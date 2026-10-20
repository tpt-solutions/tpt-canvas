//! SVG backend for tpt-canvas.
//!
//! Curves are written as-is (no flattening). Gradients use `userSpaceOnUse` in
//! `<defs>`, clips become `<clipPath>` groups, and transforms become nested `<g>`
//! elements, so the output reads like the original drawing calls.
//!
//! Numbers are formatted with a fixed precision and trailing zeros removed, so
//! output is byte-identical across platforms and suitable for golden-file tests.
//!
//! Not yet supported: glyph outlines and image bytes. Both are written as
//! placeholder comments and `<image>` references, because `tpt-text` and image
//! decoding are not wired in yet.
#![warn(missing_docs)]

use std::fmt::Write as _;

use tpt_canvas::path::{Path, PathCommand};
use tpt_canvas::text::ShapedRun;
use tpt_canvas::{
    Affine2, CanvasBackend, Color, ColorSpace, FillRule, ImageRef, LineCap, LineJoin, Paint, Rect,
    SpreadMode, Stop, StrokeStyle,
};

/// Decimal places kept in every emitted number.
const PRECISION: usize = 4;

/// A `CanvasBackend` that writes an SVG document.
pub struct SvgBackend {
    width: f32,
    height: f32,
    body: String,
    defs: String,
    /// Number of open `<g>` elements (transform and clip groups).
    depth: usize,
    next_id: u32,
    unsupported: u32,
}

impl SvgBackend {
    /// Creates a backend for a canvas of the given size.
    pub fn new(width: f32, height: f32) -> Self {
        SvgBackend {
            width,
            height,
            body: String::new(),
            defs: String::new(),
            depth: 0,
            next_id: 0,
            unsupported: 0,
        }
    }

    /// Number of calls written as placeholders because they are not supported yet
    /// (pattern paints, glyph runs and images).
    pub fn unsupported(&self) -> u32 {
        self.unsupported
    }

    /// Closes any open groups and returns the complete SVG document.
    pub fn finish(mut self) -> String {
        while self.depth > 0 {
            self.depth -= 1;
            self.body.push_str("</g>\n");
        }
        let mut out = String::new();
        let _ = writeln!(
            out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">"#,
            fmt_num(self.width),
            fmt_num(self.height),
            fmt_num(self.width),
            fmt_num(self.height)
        );
        if !self.defs.is_empty() {
            out.push_str("<defs>\n");
            out.push_str(&self.defs);
            out.push_str("</defs>\n");
        }
        out.push_str(&self.body);
        out.push_str("</svg>\n");
        out
    }

    fn fresh_id(&mut self, prefix: &str) -> String {
        self.next_id += 1;
        format!("{prefix}{}", self.next_id)
    }

    /// Writes the paint attributes for a fill (`fill`) or stroke (`stroke`) and
    /// registers any gradient or pattern definitions it needs.
    fn paint_attr(&mut self, attr: &str, paint: &Paint) -> String {
        match paint {
            Paint::Solid(c) => {
                let opacity = if c.a < 1.0 {
                    format!(r#" {attr}-opacity="{}""#, fmt_num(c.a))
                } else {
                    String::new()
                };
                format!(r#"{attr}="{}"{opacity}"#, rgb(*c))
            }
            Paint::LinearGradient {
                start,
                end,
                stops,
                spread,
                space,
            } => {
                let id = self.fresh_id("lg");
                let mut def = format!(
                    r#"<linearGradient id="{id}" gradientUnits="userSpaceOnUse" x1="{}" y1="{}" x2="{}" y2="{}"{}>"#,
                    fmt_num(start.x),
                    fmt_num(start.y),
                    fmt_num(end.x),
                    fmt_num(end.y),
                    gradient_attrs(*spread, *space)
                );
                push_stops(&mut def, stops);
                def.push_str("</linearGradient>\n");
                self.defs.push_str(&def);
                format!(r#"{attr}="url(#{id})""#)
            }
            Paint::RadialGradient {
                center,
                radius,
                stops,
                spread,
                space,
            } => {
                let id = self.fresh_id("rg");
                let mut def = format!(
                    r#"<radialGradient id="{id}" gradientUnits="userSpaceOnUse" cx="{}" cy="{}" r="{}"{}>"#,
                    fmt_num(center.x),
                    fmt_num(center.y),
                    fmt_num(*radius),
                    gradient_attrs(*spread, *space)
                );
                push_stops(&mut def, stops);
                def.push_str("</radialGradient>\n");
                self.defs.push_str(&def);
                format!(r#"{attr}="url(#{id})""#)
            }
            Paint::Pattern { image, transform } => {
                self.unsupported += 1;
                let id = self.fresh_id("pt");
                let def = format!(
                    r#"<pattern id="{id}" patternUnits="userSpaceOnUse" width="1" height="1" patternTransform="{}"><image href="tpt-image:{}" width="1" height="1"/></pattern>
"#,
                    matrix(transform),
                    image.0
                );
                self.defs.push_str(&def);
                format!(r#"{attr}="url(#{id})""#)
            }
        }
    }

    fn clip_path_element(&mut self, path: &Path) -> String {
        let id = self.fresh_id("clip");
        let _ = writeln!(
            self.defs,
            r#"<clipPath id="{id}"><path d="{}"/></clipPath>"#,
            path_d(path)
        );
        id
    }
}

impl CanvasBackend for SvgBackend {
    fn fill_path(&mut self, path: &Path, paint: &Paint, rule: FillRule) {
        let rule = match rule {
            FillRule::NonZero => "nonzero",
            FillRule::EvenOdd => "evenodd",
        };
        let fill = self.paint_attr("fill", paint);
        let _ = writeln!(
            self.body,
            r#"<path d="{}" {fill} fill-rule="{rule}"/>"#,
            path_d(path)
        );
    }

    fn stroke_path(&mut self, path: &Path, paint: &Paint, style: &StrokeStyle) {
        let stroke = self.paint_attr("stroke", paint);
        let cap = match style.cap {
            LineCap::Butt => "butt",
            LineCap::Round => "round",
            LineCap::Square => "square",
        };
        let join = match style.join {
            LineJoin::Miter => "miter",
            LineJoin::Round => "round",
            LineJoin::Bevel => "bevel",
        };
        let mut attrs = format!(
            r#"stroke-width="{}" stroke-linecap="{cap}" stroke-linejoin="{join}" stroke-miterlimit="{}""#,
            fmt_num(style.width),
            fmt_num(style.miter_limit)
        );
        if !style.dash.is_empty() {
            let dashes: Vec<String> = style.dash.iter().map(|d| fmt_num(*d)).collect();
            let _ = write!(
                attrs,
                r#" stroke-dasharray="{}" stroke-dashoffset="{}""#,
                dashes.join(" "),
                fmt_num(style.dash_offset)
            );
        }
        let _ = writeln!(
            self.body,
            r#"<path d="{}" fill="none" {stroke} {attrs}/>"#,
            path_d(path)
        );
    }

    fn draw_glyphs(&mut self, run: &ShapedRun, _paint: &Paint) {
        // Glyph outlines are not available yet; keep the run visible in the output.
        self.unsupported += 1;
        let _ = writeln!(
            self.body,
            "<!-- glyph run: font {}, size {}, {} glyphs (outlines pending tpt-text) -->",
            run.font_id,
            fmt_num(run.font_size),
            run.glyphs.len()
        );
    }

    fn clear_path(&mut self, _path: &Path) {
        // SVG only appends elements, so earlier marks cannot be erased.
        self.unsupported += 1;
    }

    fn draw_image(&mut self, image: &ImageRef, src_rect: Rect, dst_rect: Rect) {
        self.unsupported += 1;
        let _ = writeln!(
            self.body,
            r#"<image href="tpt-image:{}" x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="none" data-src="{} {} {} {}"/>"#,
            image.id.0,
            fmt_num(dst_rect.x),
            fmt_num(dst_rect.y),
            fmt_num(dst_rect.w),
            fmt_num(dst_rect.h),
            fmt_num(src_rect.x),
            fmt_num(src_rect.y),
            fmt_num(src_rect.w),
            fmt_num(src_rect.h)
        );
    }

    fn push_clip(&mut self, path: &Path) {
        let id = self.clip_path_element(path);
        let _ = writeln!(self.body, r#"<g clip-path="url(#{id})">"#);
        self.depth += 1;
    }

    fn pop_clip(&mut self) {
        self.close_group();
    }

    fn push_transform(&mut self, transform: &Affine2) {
        let _ = writeln!(self.body, r#"<g transform="{}">"#, matrix(transform));
        self.depth += 1;
    }

    fn pop_transform(&mut self) {
        self.close_group();
    }
    fn push_layer(&mut self, alpha: f32) {
        // A group opacity on `<g>` composites the whole group once, like a raster layer.
        let _ = writeln!(self.body, r#"<g opacity="{}">"#, fmt_num(alpha));
        self.depth += 1;
    }
    fn pop_layer(&mut self) {
        self.close_group();
    }
}

impl SvgBackend {
    fn close_group(&mut self) {
        if self.depth > 0 {
            self.depth -= 1;
            self.body.push_str("</g>\n");
        }
    }
}

fn gradient_attrs(spread: SpreadMode, space: ColorSpace) -> String {
    let spread = match spread {
        SpreadMode::Pad => "pad",
        SpreadMode::Repeat => "repeat",
        SpreadMode::Reflect => "reflect",
    };
    let space = match space {
        ColorSpace::Srgb => "sRGB",
        ColorSpace::LinearSrgb => "linearRGB",
    };
    format!(r#" spreadMethod="{spread}" color-interpolation="{space}""#)
}

fn push_stops(def: &mut String, stops: &[Stop]) {
    for s in stops {
        let _ = writeln!(
            def,
            r#"<stop offset="{}" stop-color="{}" stop-opacity="{}"/>"#,
            fmt_num(s.offset),
            rgb(s.color),
            fmt_num(s.color.a)
        );
    }
}

fn rgb(c: Color) -> String {
    let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    format!("rgb({},{},{})", to8(c.r), to8(c.g), to8(c.b))
}

fn matrix(m: &Affine2) -> String {
    format!(
        "matrix({} {} {} {} {} {})",
        fmt_num(m.a),
        fmt_num(m.b),
        fmt_num(m.c),
        fmt_num(m.d),
        fmt_num(m.e),
        fmt_num(m.f)
    )
}

/// Path data with curves preserved: `M`, `L`, `Q`, `C`, `Z`.
pub fn path_d(path: &Path) -> String {
    let mut d = String::new();
    for cmd in path.commands() {
        if !d.is_empty() {
            d.push(' ');
        }
        match *cmd {
            PathCommand::MoveTo(p) => {
                let _ = write!(d, "M{} {}", fmt_num(p.x), fmt_num(p.y));
            }
            PathCommand::LineTo(p) => {
                let _ = write!(d, "L{} {}", fmt_num(p.x), fmt_num(p.y));
            }
            PathCommand::QuadTo(c, p) => {
                let _ = write!(
                    d,
                    "Q{} {} {} {}",
                    fmt_num(c.x),
                    fmt_num(c.y),
                    fmt_num(p.x),
                    fmt_num(p.y)
                );
            }
            PathCommand::CubicTo(c1, c2, p) => {
                let _ = write!(
                    d,
                    "C{} {} {} {} {} {}",
                    fmt_num(c1.x),
                    fmt_num(c1.y),
                    fmt_num(c2.x),
                    fmt_num(c2.y),
                    fmt_num(p.x),
                    fmt_num(p.y)
                );
            }
            PathCommand::Close => d.push('Z'),
        }
    }
    d
}

/// Fixed-precision formatting with trailing zeros removed. `-0` becomes `0`.
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
    fn numbers_are_trimmed_and_stable() {
        assert_eq!(fmt_num(1.0), "1");
        assert_eq!(fmt_num(0.5), "0.5");
        assert_eq!(fmt_num(-0.0), "0");
        assert_eq!(fmt_num(-0.00001), "0");
        assert_eq!(fmt_num(12.34567), "12.3457");
    }
}
