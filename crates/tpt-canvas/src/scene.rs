//! A retained scene: items are recorded display lists, drawn in order.
//!
//! Each item must be self-contained: its transforms, clips and layers are balanced within its
//! own list, as they are when recorded through [`Context`](crate::Context) with `save` and
//! `restore`. Changes mark the area they touch as dirty. [`Scene::render_dirty`] then redraws
//! just that area, and gives the same pixels as a full [`Scene::render`] on a backend that can
//! erase (the raster backend, or Canvas2D).
//!
//! An item's bounds are conservative: they include a margin for stroke joins, blur and
//! anti-aliasing. Items whose bounds are unknown (glyph runs and images, which are not
//! turned into paths) count as covering the whole canvas whenever they change.

use alloc::vec::Vec;

use crate::backend::CanvasBackend;
use crate::display::{DisplayList, DrawOp};
use crate::math::{Affine2, Rect};
use crate::path::{Path, PathBuilder};
use crate::style::BlendMode;

/// Identifies an item in a [`Scene`]. Ids are not reused after an item is removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemId(pub u32);

/// Stands in for "everywhere" when an item's bounds are unknown.
const EVERYWHERE: f32 = 1.0e7;
/// Extra device pixels around every item, for anti-aliased edges.
const EDGE: f32 = 1.0;

struct Item {
    id: ItemId,
    list: DisplayList,
    /// Device-space bounds, or `None` when they cannot be bounded.
    bounds: Option<Rect>,
    visible: bool,
}

/// An ordered list of items, drawn back to front. Later items paint over earlier ones.
#[derive(Default)]
pub struct Scene {
    items: Vec<Item>,
    next_id: u32,
    dirty: Option<Rect>,
}

impl Scene {
    /// An empty scene.
    pub fn new() -> Self {
        Scene::default()
    }

    /// Adds an item on top of the others and returns its id. The new item is visible.
    pub fn add(&mut self, list: DisplayList) -> ItemId {
        let id = ItemId(self.next_id);
        self.next_id += 1;
        let bounds = item_bounds(&list);
        self.mark(bounds);
        self.items.push(Item {
            id,
            list,
            bounds,
            visible: true,
        });
        id
    }

    /// Replaces an item's contents, keeping its place and visibility. Returns false when the
    /// id is unknown.
    pub fn replace(&mut self, id: ItemId, list: DisplayList) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        let old = self.items[index].bounds;
        let visible = self.items[index].visible;
        let bounds = item_bounds(&list);
        if visible {
            self.mark(old);
            self.mark(bounds);
        }
        let item = &mut self.items[index];
        item.list = list;
        item.bounds = bounds;
        true
    }

    /// Shows or hides an item. Returns false when the id is unknown.
    pub fn set_visible(&mut self, id: ItemId, visible: bool) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        let item = &mut self.items[index];
        if item.visible != visible {
            item.visible = visible;
            let bounds = item.bounds;
            self.mark(bounds);
        }
        true
    }

    /// Removes an item. Returns false when the id is unknown.
    pub fn remove(&mut self, id: ItemId) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        let item = self.items.remove(index);
        if item.visible {
            self.mark(item.bounds);
        }
        true
    }

    /// Number of items, visible or not.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the scene has no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns the area changed since the last call, or `None` when nothing changed, and
    /// clears it. Pass the result to [`Scene::render_dirty`].
    pub fn take_dirty(&mut self) -> Option<Rect> {
        self.dirty.take()
    }

    /// Draws every visible item, in order, onto `backend`.
    pub fn render<B: CanvasBackend>(&self, backend: &mut B) {
        for item in self.items.iter().filter(|i| i.visible) {
            draw_item(backend, &item.list);
        }
    }

    /// Redraws `area`: erases it, then draws the visible items that reach it, clipped to it.
    /// The area is rounded outwards to whole pixels first, so the result matches a full render
    /// exactly inside the area. Requires a backend that can erase.
    pub fn render_dirty<B: CanvasBackend>(&self, backend: &mut B, area: Rect) {
        let rect = pixel_rect(area);
        let path = rect_path(rect);
        backend.clear_path(&path);
        backend.push_clip(&path);
        for item in self.items.iter().filter(|i| i.visible) {
            if reaches(item.bounds, rect) {
                draw_item(backend, &item.list);
            }
        }
        backend.pop_clip();
    }

    fn index_of(&self, id: ItemId) -> Option<usize> {
        self.items.iter().position(|i| i.id == id)
    }

    fn mark(&mut self, bounds: Option<Rect>) {
        let r = bounds.unwrap_or(Rect {
            x: -EVERYWHERE,
            y: -EVERYWHERE,
            w: 2.0 * EVERYWHERE,
            h: 2.0 * EVERYWHERE,
        });
        self.dirty = Some(match self.dirty {
            None => r,
            Some(d) => d.union(&r),
        });
    }
}

/// Replays one item between blend-mode resets, so no item inherits another's blend mode.
fn draw_item<B: CanvasBackend>(backend: &mut B, list: &DisplayList) {
    backend.set_blend_mode(BlendMode::Normal);
    list.replay(backend);
    backend.set_blend_mode(BlendMode::Normal);
}

/// Whether an item with `bounds` can paint inside `rect` (a whole-pixel rectangle).
fn reaches(bounds: Option<Rect>, rect: Rect) -> bool {
    match bounds {
        None => true,
        Some(b) => {
            b.x < rect.x + rect.w
                && rect.x < b.x + b.w
                && b.y < rect.y + rect.h
                && rect.y < b.y + b.h
        }
    }
}

/// `area` rounded outwards to whole pixels.
fn pixel_rect(area: Rect) -> Rect {
    let x0 = libm::floorf(area.x);
    let y0 = libm::floorf(area.y);
    let x1 = libm::ceilf(area.x + area.w);
    let y1 = libm::ceilf(area.y + area.h);
    Rect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    }
}

fn rect_path(r: Rect) -> Path {
    let mut b = PathBuilder::new();
    b.rect(r.x, r.y, r.w, r.h);
    b.build()
}

/// Device-space bounds of everything an item paints, or `None` when a glyph run or image
/// makes that impossible to bound.
fn item_bounds(list: &DisplayList) -> Option<Rect> {
    let mut acc: Option<Rect> = None;
    let mut stack: Vec<Affine2> = Vec::new();
    let mut ctm = Affine2::IDENTITY;
    for op in list.ops() {
        match op {
            DrawOp::PushTransform(m) => {
                stack.push(ctm);
                ctm = ctm * *m;
            }
            DrawOp::PopTransform => {
                ctm = stack.pop().unwrap_or(Affine2::IDENTITY);
            }
            DrawOp::FillPath { path, .. } | DrawOp::ClearPath(path) => {
                include(&mut acc, ctm, path, 0.0);
            }
            DrawOp::StrokePath { path, style, .. } => {
                // A miter join can reach miter_limit half-widths from the path.
                let margin = style.width * style.miter_limit.max(1.0) * 0.5;
                include(&mut acc, ctm, path, margin);
            }
            DrawOp::FillPathBlurred { path, sigma, .. } => {
                include(&mut acc, ctm, path, 3.0 * sigma);
            }
            DrawOp::DrawGlyphs { .. } | DrawOp::DrawImage { .. } => return None,
            // Clips, layers and blend modes do not paint on their own.
            _ => {}
        }
    }
    acc
}

fn include(acc: &mut Option<Rect>, ctm: Affine2, path: &Path, margin: f32) {
    let local = path.bounding_box();
    let grown = Rect {
        x: local.x - margin,
        y: local.y - margin,
        w: local.w + 2.0 * margin,
        h: local.h + 2.0 * margin,
    };
    let device = ctm.transform_rect(&grown);
    let padded = Rect {
        x: device.x - EDGE,
        y: device.y - EDGE,
        w: device.w + 2.0 * EDGE,
        h: device.h + 2.0 * EDGE,
    };
    *acc = Some(match acc {
        None => padded,
        Some(a) => a.union(&padded),
    });
}
