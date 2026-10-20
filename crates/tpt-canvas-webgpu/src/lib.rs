//! WebGPU backend for tpt-canvas, headless half.
//!
//! This crate turns drawing calls into the buffers and commands a `wgpu` adapter
//! would submit, without needing a GPU. The strategy is **stencil-then-cover**:
//!
//! 1. Every contour is fanned into triangles. Each triangle adds `±1` to a
//!    per-pixel winding count, following its orientation. This is exact for
//!    self-intersecting and overlapping contours, which is why no polygon
//!    tessellator needs to resolve them first.
//! 2. A cover pass draws the path's bounding box. A pixel is inside when the winding
//!    is nonzero (`NonZero`) or odd (`EvenOdd`), and it takes the paint there.
//!
//! Clips are mask layers. Each `push_clip` writes a new layer, computed the same way
//! and AND-ed with the parent layer. Fills test the active layer. Layer ids are
//! allocated per frame, so a GPU adapter can back them with a texture array.
//!
//! [`reference::execute`] runs the same command list on the CPU, using identical
//! winding and tie-breaking rules. Tests use it to check the algorithm without a GPU.
//!
//! Not yet supported: glyphs, images and pattern paints. Those calls are counted in
//! [`DrawList::unsupported`] and otherwise ignored.

pub mod reference;

use tpt_canvas::path::Path;
use tpt_canvas::stroke::stroke_outline;
use tpt_canvas::tessellate::fan;

use tpt_canvas::{Affine2, CanvasBackend, FillRule, ImageRef, Paint, Rect, StrokeStyle, Vec2};
use tpt_text_stub::ShapedRun;

/// Range of triangle indices (`first` into `DrawList::indices`, `count` indices, a multiple of 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriRange {
    pub first: u32,
    pub count: u32,
}

/// Paint plus the device-to-user transform the shader uses to evaluate it per pixel.
#[derive(Clone, Debug, PartialEq)]
pub struct PaintEntry {
    pub paint: Paint,
    pub device_to_user: Affine2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    /// Writes the inside/outside mask of `tris` into `layer`, AND-ed with `parent`.
    ClipMask {
        layer: u32,
        parent: Option<u32>,
        tris: TriRange,
        rule: FillRule,
        bounds: Rect,
    },
    /// Stencil-winding over `tris`, then cover `bounds` with `paint` where inside and
    /// where the clip layer (if any) is set.
    Fill {
        tris: TriRange,
        rule: FillRule,
        paint: u32,
        clip: Option<u32>,
        bounds: Rect,
    },
}

/// Everything a GPU adapter needs for one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DrawList {
    pub width: u32,
    pub height: u32,
    /// Device-space vertex positions, shared by all triangles.
    pub positions: Vec<Vec2>,
    /// Triangle indices into `positions`.
    pub indices: Vec<u32>,
    pub commands: Vec<Command>,
    pub paints: Vec<PaintEntry>,
    /// Number of clip mask layers the frame uses.
    pub clip_layers: u32,
    /// Calls skipped because the feature is not implemented yet.
    pub unsupported: u32,
}

pub struct WebGpuBackend {
    list: DrawList,
    transforms: Vec<Affine2>,
    clips: Vec<u32>,
    tolerance: f32,
}

impl WebGpuBackend {
    pub fn new(width: u32, height: u32) -> Self {
        WebGpuBackend {
            list: DrawList {
                width,
                height,
                ..DrawList::default()
            },
            transforms: vec![Affine2::IDENTITY],
            clips: Vec::new(),
            tolerance: 0.1,
        }
    }

    pub fn draw_list(&self) -> &DrawList {
        &self.list
    }

    pub fn into_draw_list(self) -> DrawList {
        self.list
    }

    fn ctm(&self) -> Affine2 {
        *self
            .transforms
            .last()
            .expect("base transform is always present")
    }

    /// Fans each polygon into triangles and returns the range they occupy.
    fn add_polygons(&mut self, polys: &[Vec<Vec2>]) -> TriRange {
        let first = self.list.indices.len() as u32;
        for poly in polys {
            let t = fan(poly);
            let base = self.list.positions.len() as u32;
            self.list.positions.extend_from_slice(&t.positions);
            for tri in t.indices.chunks_exact(3) {
                let (a, b, c) = (
                    t.positions[tri[0] as usize],
                    t.positions[tri[1] as usize],
                    t.positions[tri[2] as usize],
                );
                // Zero-area triangles would match the top-left tie rule on their edges.
                if (b - a).cross(c - a).abs() <= 1e-9 {
                    continue;
                }
                self.list
                    .indices
                    .extend_from_slice(&[base + tri[0], base + tri[1], base + tri[2]]);
            }
        }
        TriRange {
            first,
            count: self.list.indices.len() as u32 - first,
        }
    }

    fn paint_index(&mut self, paint: &Paint) -> u32 {
        let device_to_user = self.ctm().invert().unwrap_or(Affine2::IDENTITY);
        self.list.paints.push(PaintEntry {
            paint: paint.clone(),
            device_to_user,
        });
        (self.list.paints.len() - 1) as u32
    }

    /// Device-space bounding box of `polys`, clipped to the canvas.
    fn canvas_bounds(&self, polys: &[Vec<Vec2>]) -> Option<Rect> {
        let mut min = Vec2::new(f32::INFINITY, f32::INFINITY);
        let mut max = Vec2::new(f32::NEG_INFINITY, f32::NEG_INFINITY);
        for p in polys.iter().flatten() {
            min = Vec2::new(min.x.min(p.x), min.y.min(p.y));
            max = Vec2::new(max.x.max(p.x), max.y.max(p.y));
        }
        let x0 = min.x.max(0.0);
        let y0 = min.y.max(0.0);
        let x1 = max.x.min(self.list.width as f32);
        let y1 = max.y.min(self.list.height as f32);
        if !(x1 > x0 && y1 > y0) {
            return None;
        }
        Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
    }

    fn device_polygons(&self, path: &Path) -> Vec<Vec<Vec2>> {
        path.transform(&self.ctm())
            .flatten(self.tolerance)
            .polylines
            .into_iter()
            .map(|p| p.points)
            .collect()
    }

    fn fill_polygons(&mut self, polys: Vec<Vec<Vec2>>, rule: FillRule, paint: &Paint) {
        let Some(bounds) = self.canvas_bounds(&polys) else {
            return;
        };
        let tris = self.add_polygons(&polys);
        if tris.count == 0 {
            return;
        }
        let paint = self.paint_index(paint);
        let clip = self.clips.last().copied();
        self.list.commands.push(Command::Fill {
            tris,
            rule,
            paint,
            clip,
            bounds,
        });
    }
}

impl CanvasBackend for WebGpuBackend {
    fn fill_path(&mut self, path: &Path, paint: &Paint, rule: FillRule) {
        if matches!(paint, Paint::Pattern { .. }) {
            self.list.unsupported += 1;
            return;
        }
        let polys = self.device_polygons(path);
        self.fill_polygons(polys, rule, paint);
    }

    fn stroke_path(&mut self, path: &Path, paint: &Paint, style: &StrokeStyle) {
        if matches!(paint, Paint::Pattern { .. }) {
            self.list.unsupported += 1;
            return;
        }
        // Stroke in user space, then map the outline. Its pieces overlap, so nonzero is used.
        let ctm = self.ctm();
        let scale = libm::sqrtf(libm::fabsf(ctm.determinant())).max(1e-6);
        let user_tol = self.tolerance / scale;
        let outline = stroke_outline(&path.flatten(user_tol), style, user_tol);
        let polys: Vec<Vec<Vec2>> = outline
            .into_iter()
            .map(|poly| poly.into_iter().map(|p| ctm.apply(p)).collect())
            .collect();
        self.fill_polygons(polys, FillRule::NonZero, paint);
    }

    fn draw_glyphs(&mut self, _run: &ShapedRun, _paint: &Paint) {
        self.list.unsupported += 1;
    }

    fn draw_image(&mut self, _image: &ImageRef, _src_rect: Rect, _dst_rect: Rect) {
        self.list.unsupported += 1;
    }

    fn push_clip(&mut self, path: &Path) {
        let polys = self.device_polygons(path);
        let layer = self.list.clip_layers;
        self.list.clip_layers += 1;
        let parent = self.clips.last().copied();
        let bounds = self.canvas_bounds(&polys).unwrap_or(Rect::ZERO);
        let tris = self.add_polygons(&polys);
        self.list.commands.push(Command::ClipMask {
            layer,
            parent,
            tris,
            rule: FillRule::NonZero,
            bounds,
        });
        self.clips.push(layer);
    }

    fn pop_clip(&mut self) {
        self.clips.pop();
    }

    fn push_transform(&mut self, transform: &Affine2) {
        let next = self.ctm() * *transform;
        self.transforms.push(next);
    }

    fn pop_transform(&mut self) {
        if self.transforms.len() > 1 {
            self.transforms.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_canvas::PathBuilder;

    #[test]
    fn rect_fill_records_one_fill_command() {
        let mut b = PathBuilder::new();
        b.rect(1.0, 1.0, 4.0, 4.0);
        let mut be = WebGpuBackend::new(10, 10);
        be.fill_path(
            &b.build(),
            &Paint::Solid(tpt_canvas::Color::BLACK),
            FillRule::NonZero,
        );
        let list = be.into_draw_list();
        assert_eq!(list.commands.len(), 1);
        assert_eq!(list.indices.len() % 3, 0);
        assert_eq!(list.indices.len(), 6);
    }

    #[test]
    fn offscreen_fill_is_dropped() {
        let mut b = PathBuilder::new();
        b.rect(50.0, 50.0, 4.0, 4.0);
        let mut be = WebGpuBackend::new(10, 10);
        be.fill_path(
            &b.build(),
            &Paint::Solid(tpt_canvas::Color::BLACK),
            FillRule::NonZero,
        );
        assert!(be.into_draw_list().commands.is_empty());
    }

    #[test]
    fn clip_layers_chain_to_their_parent() {
        let mut b = PathBuilder::new();
        b.rect(0.0, 0.0, 8.0, 8.0);
        let p = b.build();
        let mut be = WebGpuBackend::new(10, 10);
        be.push_clip(&p);
        be.push_clip(&p);
        be.fill_path(
            &p,
            &Paint::Solid(tpt_canvas::Color::BLACK),
            FillRule::NonZero,
        );
        let list = be.into_draw_list();
        assert_eq!(list.clip_layers, 2);
        match list.commands[1] {
            Command::ClipMask { parent, .. } => assert_eq!(parent, Some(0)),
            other => panic!("unexpected {other:?}"),
        }
        match list.commands[2] {
            Command::Fill { clip, .. } => assert_eq!(clip, Some(1)),
            other => panic!("unexpected {other:?}"),
        }
    }
}
