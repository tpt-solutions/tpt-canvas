//! PDF backend for tpt-canvas.
//!
//! Paths are written as `m`/`l`/`c`/`h` operators. Quadratics are degree-elevated to
//! cubics, and nothing is flattened, so output stays vector and small.
//!
//! - Solid fills and strokes use `rg`/`RG`, with alpha through an ExtGState.
//! - Linear and radial gradients are shading dictionaries (`sh`) with Type 2
//!   or stitched Type 3 functions. A gradient fill clips to the path and then shades.
//! - Gradient strokes clip to the stroker's outline, then shade.
//! - The content stream flips the y axis once, so coordinates follow the same
//!   y-down convention as the SVG backend.
//!
//! Limitations, counted by [`PdfBackend::unsupported`]: `Repeat`/`Reflect` spreads are
//! drawn as `Pad`, per-stop alpha is ignored, and `Pattern` paints, glyphs and images
//! emit placeholder comments until image decoding and text outlines are wired in.
#![warn(missing_docs)]

use std::fmt::Write as _;

use tpt_canvas::color::interpolate;
use tpt_canvas::path::{quad_to_cubic, Path, PathCommand};
use tpt_canvas::stroke::stroke_outline;
use tpt_canvas::text::ShapedRun;
use tpt_canvas::{
    Affine2, CanvasBackend, Color, ColorSpace, FillRule, ImageRef, LineCap, LineJoin, Paint, Rect,
    SpreadMode, Stop, StrokeStyle, Vec2,
};

/// Decimal places written for every number.
const PRECISION: usize = 4;
/// Samples per segment used to emulate linear-light interpolation as sRGB stops.
const LINEAR_SAMPLES: usize = 8;
/// Tolerance used when flattening paths for gradient-stroke outlines (user units).
const STROKE_TOLERANCE: f32 = 0.05;

/// A `CanvasBackend` that writes a PDF with one or more pages.
pub struct PdfBackend {
    width: f32,
    height: f32,
    content: String,
    /// Pages already ended by `new_page`: width, height and content stream.
    finished: Vec<(f32, f32, String)>,
    /// ExtGState dictionaries; resource name is `GS{index}`. Shared by every page.
    ext_gstates: Vec<String>,
    /// Shading dictionaries; resource name is `Sh{index}`. Shared by every page.
    shadings: Vec<String>,
    /// Open `q` levels from clips and transforms on the current page.
    depth: usize,
    unsupported: u32,
}

impl PdfBackend {
    /// Creates a backend whose first page has the given size.
    pub fn new(width: f32, height: f32) -> Self {
        PdfBackend {
            width,
            height,
            content: page_header(height),
            finished: Vec::new(),
            ext_gstates: Vec::new(),
            shadings: Vec::new(),
            depth: 1,
            unsupported: 0,
        }
    }

    /// Ends the current page and starts a new one of the given size. Drawing calls
    /// after this go to the new page.
    pub fn new_page(&mut self, width: f32, height: f32) {
        self.end_page();
        self.width = width;
        self.height = height;
        self.content = page_header(height);
        self.depth = 1;
    }

    /// Number of calls written as placeholder comments or approximated because they are
    /// not supported yet (pattern paints, glyph runs, images, and some gradient features).
    pub fn unsupported(&self) -> u32 {
        self.unsupported
    }

    /// Closes the current page's open state and moves its content into `finished`.
    fn end_page(&mut self) {
        for _ in 0..self.depth {
            self.content.push_str("Q\n");
        }
        self.depth = 0;
        let content = core::mem::take(&mut self.content);
        self.finished.push((self.width, self.height, content));
    }

    /// Closes open state, then returns a complete PDF document with every page.
    pub fn finish(mut self) -> Vec<u8> {
        self.end_page();
        let resources = self.resources();
        let page_count = self.finished.len();

        // Object 1 is the catalog and 2 the page tree. Page i uses objects 3 + 2i (page)
        // and 4 + 2i (content stream).
        let kids: Vec<String> = (0..page_count)
            .map(|i| format!("{} 0 R", 3 + 2 * i))
            .collect();
        let mut objects = vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            format!(
                "<< /Type /Pages /Kids [{}] /Count {page_count} >>",
                kids.join(" ")
            ),
        ];
        for (i, (w, h, content)) in self.finished.iter().enumerate() {
            let content_id = 4 + 2 * i;
            objects.push(format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents {content_id} 0 R /Resources {resources} >>",
                fmt_num(*w),
                fmt_num(*h)
            ));
            objects.push(format!(
                "<< /Length {} >>\nstream\n{content}endstream",
                content.len()
            ));
        }

        let mut out: Vec<u8> = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let mut offsets = Vec::with_capacity(objects.len());
        for (i, body) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
        }
        let xref_at = out.len();
        let mut xref = format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1);
        for off in &offsets {
            let _ = writeln!(xref, "{off:010} 00000 n ");
        }
        let _ = write!(
            xref,
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            objects.len() + 1
        );
        out.extend_from_slice(xref.as_bytes());
        out
    }

    fn resources(&self) -> String {
        let mut r = String::from("<< ");
        if !self.ext_gstates.is_empty() {
            r.push_str("/ExtGState << ");
            for (i, gs) in self.ext_gstates.iter().enumerate() {
                let _ = write!(r, "/GS{i} {gs} ");
            }
            r.push_str(">> ");
        }
        if !self.shadings.is_empty() {
            r.push_str("/Shading << ");
            for (i, sh) in self.shadings.iter().enumerate() {
                let _ = write!(r, "/Sh{i} {sh} ");
            }
            r.push_str(">> ");
        }
        r.push_str(">>");
        r
    }

    /// Returns the ExtGState resource name for an opacity, registering it on first use.
    fn alpha_state(&mut self, alpha: f32) -> String {
        let dict = format!(
            "<< /Type /ExtGState /CA {} /ca {} >>",
            fmt_num(alpha),
            fmt_num(alpha)
        );
        let idx = match self.ext_gstates.iter().position(|d| *d == dict) {
            Some(i) => i,
            None => {
                self.ext_gstates.push(dict);
                self.ext_gstates.len() - 1
            }
        };
        format!("/GS{idx} gs")
    }

    fn register_shading(&mut self, dict: String) -> String {
        self.shadings.push(dict);
        format!("/Sh{} sh", self.shadings.len() - 1)
    }

    /// Emits a gradient shading over whatever the current clip allows. `extent` is the
    /// clip's user-space bounding box, used to choose which tiles a repeated gradient needs.
    fn shade_gradient(&mut self, paint: &Paint, extent: Rect) -> bool {
        if gradient_shading(paint).is_none() {
            return false;
        }
        if has_translucent_stops(paint) {
            self.unsupported += 1;
        }
        if let Paint::LinearGradient {
            start,
            end,
            stops,
            spread,
            space,
        } = paint
        {
            if *spread != SpreadMode::Pad {
                let tiles = linear_tiles(*start, *end, stops, *space, *spread, extent);
                if let Some(tiles) = tiles {
                    let alpha = self.alpha_state(1.0);
                    for dict in tiles {
                        let op = self.register_shading(dict);
                        let _ = writeln!(self.content, "{alpha} {op}");
                    }
                    return true;
                }
            }
        }
        if spread_of(paint) != SpreadMode::Pad {
            // Radial repeats, and linear repeats too long to tile, are drawn as Pad.
            self.unsupported += 1;
        }
        let Some(dict) = gradient_shading(paint) else {
            return false;
        };
        let op = self.register_shading(dict);
        let alpha = self.alpha_state(1.0);
        let _ = writeln!(self.content, "{alpha} {op}");
        true
    }

    /// Writes the pattern placeholder used for `Paint::Pattern`.
    fn pattern_placeholder(&mut self, what: &str) {
        self.unsupported += 1;
        let _ = writeln!(self.content, "% {what}: pattern paint not yet supported");
    }
}

impl CanvasBackend for PdfBackend {
    fn fill_path(&mut self, path: &Path, paint: &Paint, rule: FillRule) {
        let fill_op = match rule {
            FillRule::NonZero => "f",
            FillRule::EvenOdd => "f*",
        };
        match paint {
            Paint::Solid(c) => {
                let gs = self.alpha_state(c.a);
                let _ = writeln!(self.content, "{gs} {} rg", rgb_ops(*c));
                path_ops(&mut self.content, path);
                let _ = writeln!(self.content, "{fill_op}");
            }
            Paint::LinearGradient { .. } | Paint::RadialGradient { .. } => {
                self.content.push_str("q\n");
                path_ops(&mut self.content, path);
                let clip = match rule {
                    FillRule::NonZero => "W n",
                    FillRule::EvenOdd => "W* n",
                };
                let _ = writeln!(self.content, "{clip}");
                self.shade_gradient(paint, path.bounding_box());
                self.content.push_str("Q\n");
            }
            Paint::Pattern { .. } => self.pattern_placeholder("fill"),
        }
    }

    fn stroke_path(&mut self, path: &Path, paint: &Paint, style: &StrokeStyle) {
        match paint {
            Paint::Solid(c) => {
                let gs = self.alpha_state(c.a);
                let _ = writeln!(self.content, "{gs} {} RG", rgb_ops(*c));
                let _ = writeln!(self.content, "{} w", fmt_num(style.width));
                let cap = match style.cap {
                    LineCap::Butt => 0,
                    LineCap::Round => 1,
                    LineCap::Square => 2,
                };
                let join = match style.join {
                    LineJoin::Miter => 0,
                    LineJoin::Round => 1,
                    LineJoin::Bevel => 2,
                };
                let _ = writeln!(
                    self.content,
                    "{cap} J {join} j {} M",
                    fmt_num(style.miter_limit)
                );
                if !style.dash.is_empty() {
                    let dashes: Vec<String> = style.dash.iter().map(|d| fmt_num(*d)).collect();
                    let _ = writeln!(
                        self.content,
                        "[{}] {} d",
                        dashes.join(" "),
                        fmt_num(style.dash_offset)
                    );
                }
                path_ops(&mut self.content, path);
                self.content.push_str("S\n");
            }
            Paint::LinearGradient { .. } | Paint::RadialGradient { .. } => {
                // Clip to the stroke outline, then shade it.
                let flat = path.flatten(STROKE_TOLERANCE);
                let outline = stroke_outline(&flat, style, STROKE_TOLERANCE);
                self.content.push_str("q\n");
                for poly in &outline {
                    polygon_ops(&mut self.content, poly);
                }
                self.content.push_str("W n\n");
                let extent = points_bbox(outline.iter().flatten().copied());
                self.shade_gradient(paint, extent);
                self.content.push_str("Q\n");
            }
            Paint::Pattern { .. } => self.pattern_placeholder("stroke"),
        }
    }

    fn draw_glyphs(&mut self, run: &ShapedRun, _paint: &Paint) {
        self.unsupported += 1;
        let _ = writeln!(
            self.content,
            "% glyph run: font {}, size {}, {} glyphs (outlines pending tpt-text)",
            run.font_id,
            fmt_num(run.font_size),
            run.glyphs.len()
        );
    }

    fn push_layer(&mut self, _alpha: f32) {
        // Group opacity needs a transparency group XObject, which is not written yet.
        self.unsupported += 1;
        self.content
            .push_str("% layer: group opacity not supported\n");
    }

    fn pop_layer(&mut self) {}

    fn clear_path(&mut self, _path: &Path) {
        // PDF content streams only append, so earlier marks cannot be erased.
        self.unsupported += 1;
        self.content.push_str("% clear path: not supported\n");
    }

    fn draw_image(&mut self, image: &ImageRef, _src_rect: Rect, _dst_rect: Rect) {
        self.unsupported += 1;
        let _ = writeln!(self.content, "% image {}: decoding pending", image.id.0);
    }

    fn push_clip(&mut self, path: &Path) {
        self.content.push_str("q\n");
        path_ops(&mut self.content, path);
        self.content.push_str("W n\n");
        self.depth += 1;
    }

    fn pop_clip(&mut self) {
        self.pop_level();
    }

    fn push_transform(&mut self, t: &Affine2) {
        let _ = writeln!(
            self.content,
            "q {} {} {} {} {} {} cm",
            fmt_num(t.a),
            fmt_num(t.b),
            fmt_num(t.c),
            fmt_num(t.d),
            fmt_num(t.e),
            fmt_num(t.f)
        );
        self.depth += 1;
    }

    fn pop_transform(&mut self) {
        self.pop_level();
    }
}

impl PdfBackend {
    fn pop_level(&mut self) {
        // Never pop the base level that holds the y-flip.
        if self.depth > 1 {
            self.depth -= 1;
            self.content.push_str("Q\n");
        }
    }
}

/// Most tiles a repeated linear gradient may use before it falls back to Pad.
const MAX_TILES: i64 = 4096;

/// Whether any stop of a gradient has alpha below 1. PDF shadings ignore per-stop alpha.
fn has_translucent_stops(paint: &Paint) -> bool {
    match paint {
        Paint::LinearGradient { stops, .. } | Paint::RadialGradient { stops, .. } => {
            stops.iter().any(|s| s.color.a < 1.0)
        }
        _ => false,
    }
}

fn spread_of(paint: &Paint) -> SpreadMode {
    match paint {
        Paint::LinearGradient { spread, .. } | Paint::RadialGradient { spread, .. } => *spread,
        _ => SpreadMode::Pad,
    }
}

/// Shadings for a `Repeat` or `Reflect` linear gradient, one per unit of the parameter
/// that overlaps `extent`. Each tile has `Extend false`, so tiles meet without overlap.
/// `None` when the gradient is degenerate or would need more than `MAX_TILES` tiles.
fn linear_tiles(
    start: Vec2,
    end: Vec2,
    stops: &[Stop],
    space: ColorSpace,
    spread: SpreadMode,
    extent: Rect,
) -> Option<Vec<String>> {
    let d = end - start;
    let len2 = d.dot(d);
    if len2 <= 0.0 || !len2.is_finite() {
        return None;
    }
    // Gradient parameter t at each corner of the extent; the tiles must cover [tmin, tmax].
    let corners = [
        Vec2::new(extent.x, extent.y),
        Vec2::new(extent.x + extent.w, extent.y),
        Vec2::new(extent.x, extent.y + extent.h),
        Vec2::new(extent.x + extent.w, extent.y + extent.h),
    ];
    let mut tmin = f32::INFINITY;
    let mut tmax = f32::NEG_INFINITY;
    for c in corners {
        let t = (c - start).dot(d) / len2;
        tmin = tmin.min(t);
        tmax = tmax.max(t);
    }
    if !tmin.is_finite() || !tmax.is_finite() {
        return None;
    }
    let k0 = tmin.floor() as i64;
    let k1 = tmax.ceil() as i64;
    if k1 - k0 > MAX_TILES {
        return None;
    }
    let func = stop_function(&prepare_stops(stops, space));
    let mut out = Vec::new();
    for k in k0..k1.max(k0 + 1) {
        let at = |t: i64| start + d * (t as f32);
        // Odd Reflect tiles run backwards, which mirrors the stops across the tile.
        let (p0, p1) = match spread {
            SpreadMode::Reflect if k.rem_euclid(2) == 1 => (at(k + 1), at(k)),
            _ => (at(k), at(k + 1)),
        };
        out.push(format!(
            "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [{} {} {} {}] /Function {func} /Extend [false false] >>",
            fmt_num(p0.x),
            fmt_num(p0.y),
            fmt_num(p1.x),
            fmt_num(p1.y),
        ));
    }
    Some(out)
}

/// Bounding box of a set of points. Empty input gives a zero rectangle.
fn points_bbox(points: impl Iterator<Item = Vec2>) -> Rect {
    let mut min = Vec2::new(f32::INFINITY, f32::INFINITY);
    let mut max = Vec2::new(f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut any = false;
    for p in points {
        any = true;
        min = Vec2::new(min.x.min(p.x), min.y.min(p.y));
        max = Vec2::new(max.x.max(p.x), max.y.max(p.y));
    }
    if !any {
        return Rect::new(0.0, 0.0, 0.0, 0.0);
    }
    Rect::new(min.x, min.y, max.x - min.x, max.y - min.y)
}

/// Builds the shading dictionary for a gradient paint, or `None` for other paints.
fn gradient_shading(paint: &Paint) -> Option<String> {
    match paint {
        // Spread modes other than Pad are drawn as Pad; see the module docs.
        Paint::LinearGradient {
            start,
            end,
            stops,
            space,
            ..
        } => {
            let func = stop_function(&prepare_stops(stops, *space));
            Some(format!(
                "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [{} {} {} {}] /Function {func} /Extend [true true] >>",
                fmt_num(start.x),
                fmt_num(start.y),
                fmt_num(end.x),
                fmt_num(end.y),
            ))
        }
        Paint::RadialGradient {
            center,
            radius,
            stops,
            space,
            ..
        } => {
            let func = stop_function(&prepare_stops(stops, *space));
            Some(format!(
                "<< /ShadingType 3 /ColorSpace /DeviceRGB /Coords [{} {} 0 {} {} {}] /Function {func} /Extend [true true] >>",
                fmt_num(center.x),
                fmt_num(center.y),
                fmt_num(center.x),
                fmt_num(center.y),
                fmt_num(*radius),
            ))
        }
        _ => None,
    }
}

/// Makes stops span `[0, 1]` and, for linear-light gradients, resamples them so that
/// sRGB-space interpolation between the samples matches the requested space.
fn prepare_stops(stops: &[Stop], space: ColorSpace) -> Vec<Stop> {
    let mut v: Vec<Stop> = Vec::new();
    if stops.is_empty() {
        return vec![
            Stop {
                offset: 0.0,
                color: Color::BLACK,
            },
            Stop {
                offset: 1.0,
                color: Color::BLACK,
            },
        ];
    }
    match space {
        ColorSpace::Srgb => v.extend_from_slice(stops),
        ColorSpace::LinearSrgb => {
            for pair in stops.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                v.push(a);
                for k in 1..LINEAR_SAMPLES {
                    let t = k as f32 / LINEAR_SAMPLES as f32;
                    let off = a.offset + (b.offset - a.offset) * t;
                    v.push(Stop {
                        offset: off,
                        color: interpolate(stops, off, ColorSpace::LinearSrgb),
                    });
                }
            }
            v.push(*stops.last().expect("checked non-empty"));
        }
    }
    if v[0].offset > 0.0 {
        v.insert(
            0,
            Stop {
                offset: 0.0,
                color: v[0].color,
            },
        );
    }
    let last = *v.last().expect("non-empty");
    if last.offset < 1.0 {
        v.push(Stop {
            offset: 1.0,
            color: last.color,
        });
    }
    v
}

/// Type 2 function for two stops, or a stitched Type 3 function for more.
fn stop_function(stops: &[Stop]) -> String {
    let c = |col: Color| format!("[{} {} {}]", fmt_num(col.r), fmt_num(col.g), fmt_num(col.b));
    let exp = |a: Color, b: Color| {
        format!(
            "<< /FunctionType 2 /Domain [0 1] /C0 {} /C1 {} /N 1 >>",
            c(a),
            c(b)
        )
    };
    // Segments with positive width become sub-functions; zero-width ones are dropped.
    let segments: Vec<(Stop, Stop)> = stops
        .windows(2)
        .filter(|w| w[1].offset > w[0].offset)
        .map(|w| (w[0], w[1]))
        .collect();
    match segments.len() {
        0 => exp(stops[0].color, stops[stops.len() - 1].color),
        1 => exp(segments[0].0.color, segments[0].1.color),
        _ => {
            let funcs: Vec<String> = segments
                .iter()
                .map(|(a, b)| exp(a.color, b.color))
                .collect();
            let bounds: Vec<String> = segments[1..]
                .iter()
                .map(|(a, _)| fmt_num(a.offset))
                .collect();
            let encode = vec!["0 1"; segments.len()].join(" ");
            format!(
                "<< /FunctionType 3 /Domain [0 1] /Functions [{}] /Bounds [{}] /Encode [{encode}] >>",
                funcs.join(" "),
                bounds.join(" ")
            )
        }
    }
}

fn rgb_ops(c: Color) -> String {
    format!("{} {} {}", fmt_num(c.r), fmt_num(c.g), fmt_num(c.b))
}

fn path_ops(out: &mut String, path: &Path) {
    let mut cur = Vec2::ZERO;
    let mut start = Vec2::ZERO;
    for cmd in path.commands() {
        match *cmd {
            PathCommand::MoveTo(p) => {
                let _ = writeln!(out, "{} {} m", fmt_num(p.x), fmt_num(p.y));
                cur = p;
                start = p;
            }
            PathCommand::LineTo(p) => {
                let _ = writeln!(out, "{} {} l", fmt_num(p.x), fmt_num(p.y));
                cur = p;
            }
            PathCommand::QuadTo(c, p) => {
                let (c1, c2, p3) = quad_to_cubic(cur, c, p);
                cubic_ops(out, c1, c2, p3);
                cur = p;
            }
            PathCommand::CubicTo(c1, c2, p) => {
                cubic_ops(out, c1, c2, p);
                cur = p;
            }
            PathCommand::Close => {
                out.push_str("h\n");
                cur = start;
            }
        }
    }
}

fn cubic_ops(out: &mut String, c1: Vec2, c2: Vec2, p: Vec2) {
    let _ = writeln!(
        out,
        "{} {} {} {} {} {} c",
        fmt_num(c1.x),
        fmt_num(c1.y),
        fmt_num(c2.x),
        fmt_num(c2.y),
        fmt_num(p.x),
        fmt_num(p.y)
    );
}

fn polygon_ops(out: &mut String, poly: &[Vec2]) {
    for (i, p) in poly.iter().enumerate() {
        let op = if i == 0 { "m" } else { "l" };
        let _ = writeln!(out, "{} {} {op}", fmt_num(p.x), fmt_num(p.y));
    }
    out.push_str("h\n");
}

/// Opening of a page's content stream. It flips y so user space matches the y-down
/// convention of the other backends, and its `q` is closed when the page ends.
fn page_header(height: f32) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "q 1 0 0 -1 0 {} cm", fmt_num(height));
    s
}

/// Fixed-precision number formatting with trailing zeros removed. `-0` becomes `0`.
pub fn fmt_num(v: f32) -> String {
    let mut s = format!("{:.*}", PRECISION, v);
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_trimmed() {
        assert_eq!(fmt_num(2.0), "2");
        assert_eq!(fmt_num(-0.25), "-0.25");
    }

    #[test]
    fn two_stop_gradient_uses_type2_function() {
        let stops = [
            Stop {
                offset: 0.0,
                color: Color::BLACK,
            },
            Stop {
                offset: 1.0,
                color: Color::WHITE,
            },
        ];
        let f = stop_function(&prepare_stops(&stops, ColorSpace::Srgb));
        assert!(f.contains("/FunctionType 2"));
        assert!(!f.contains("/FunctionType 3"));
    }

    #[test]
    fn multi_stop_gradient_uses_stitching_function() {
        let stops = [
            Stop {
                offset: 0.0,
                color: Color::BLACK,
            },
            Stop {
                offset: 0.5,
                color: Color::WHITE,
            },
            Stop {
                offset: 1.0,
                color: Color::BLACK,
            },
        ];
        let f = stop_function(&prepare_stops(&stops, ColorSpace::Srgb));
        assert!(f.contains("/FunctionType 3"));
        assert!(f.contains("/Bounds [0.5]"));
    }
}
