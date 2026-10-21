//! Binary encoding for [`DisplayList`]: `"TPTD"`, a version byte, an op count, then tagged ops.
//! Paths reuse [`Path::serialize`]. All numbers are little-endian.

use alloc::vec::Vec;
use core::fmt;

use super::{DisplayList, DrawOp};
use crate::image::{ImageId, ImageRef};
use crate::path::Path;
use crate::style::{BlendMode, FillRule, LineCap, LineJoin, Paint, SpreadMode, StrokeStyle};
use crate::text::ShapedRun;
use crate::{Affine2, Color, ColorSpace, Rect, Stop, Vec2};

/// Errors from [`DisplayList::deserialize`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DisplayDecodeError {
    /// The input does not start with the `TPTD` magic bytes.
    BadMagic,
    /// The format version is not supported.
    UnsupportedVersion(u8),
    /// The input ended early.
    Truncated,
    /// An unknown operation or enum tag.
    BadTag(u8),
    /// An embedded path failed to decode.
    Path(crate::path::DecodeError),
}

impl fmt::Display for DisplayDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DisplayDecodeError::BadMagic => write!(f, "bad magic"),
            DisplayDecodeError::UnsupportedVersion(v) => write!(f, "unsupported version {v}"),
            DisplayDecodeError::Truncated => write!(f, "truncated buffer"),
            DisplayDecodeError::BadTag(t) => write!(f, "unknown tag {t}"),
            DisplayDecodeError::Path(e) => write!(f, "path: {e}"),
        }
    }
}

const MAGIC: &[u8; 4] = b"TPTD";
const VERSION: u8 = 1;

impl DisplayList {
    /// Encodes the list as bytes.
    pub fn serialize(&self) -> Vec<u8> {
        let mut w = Writer(Vec::new());
        w.0.extend_from_slice(MAGIC);
        w.u8(VERSION);
        w.u32(self.ops().len() as u32);
        for op in self.ops() {
            match op {
                DrawOp::FillPath { path, paint, rule } => {
                    w.u8(0);
                    w.path(path);
                    w.paint(paint);
                    w.u8(match rule {
                        FillRule::NonZero => 0,
                        FillRule::EvenOdd => 1,
                    });
                }
                DrawOp::StrokePath { path, paint, style } => {
                    w.u8(1);
                    w.path(path);
                    w.paint(paint);
                    w.stroke(style);
                }
                DrawOp::DrawGlyphs { run, paint } => {
                    w.u8(2);
                    w.u32(run.font_id);
                    w.f32(run.font_size);
                    w.u32(run.glyphs.len() as u32);
                    for g in &run.glyphs {
                        w.u32(g.id);
                        w.vec2(g.pos);
                    }
                    w.paint(paint);
                }
                DrawOp::DrawImage { image, src, dst } => {
                    w.u8(3);
                    w.u32(image.id.0);
                    w.u32(image.width);
                    w.u32(image.height);
                    w.rect(*src);
                    w.rect(*dst);
                }
                DrawOp::PushClip(path) => {
                    w.u8(4);
                    w.path(path);
                }
                DrawOp::PopClip => w.u8(5),
                DrawOp::PushTransform(m) => {
                    w.u8(6);
                    w.affine(m);
                }
                DrawOp::PopTransform => w.u8(7),
                DrawOp::PushLayer(alpha) => {
                    w.u8(9);
                    w.f32(*alpha);
                }
                DrawOp::PopLayer => w.u8(10),
                DrawOp::SetBlendMode(mode) => {
                    w.u8(12);
                    w.u8(blend_code(*mode));
                }
                DrawOp::ClearPath(path) => {
                    w.u8(8);
                    w.path(path);
                }
                DrawOp::FillPathBlurred {
                    path,
                    paint,
                    rule,
                    sigma,
                } => {
                    w.u8(11);
                    w.path(path);
                    w.paint(paint);
                    w.u8(match rule {
                        FillRule::NonZero => 0,
                        FillRule::EvenOdd => 1,
                    });
                    w.f32(*sigma);
                }
            }
        }
        w.0
    }

    /// Decodes a list written by `serialize`.
    pub fn deserialize(bytes: &[u8]) -> Result<DisplayList, DisplayDecodeError> {
        let mut r = Reader { buf: bytes, pos: 0 };
        if r.take(4)? != MAGIC {
            return Err(DisplayDecodeError::BadMagic);
        }
        let version = r.u8()?;
        if version != VERSION {
            return Err(DisplayDecodeError::UnsupportedVersion(version));
        }
        let count = r.u32()?;
        let mut list = DisplayList::new();
        for _ in 0..count {
            let op = match r.u8()? {
                0 => DrawOp::FillPath {
                    path: r.path()?,
                    paint: r.paint()?,
                    rule: match r.u8()? {
                        0 => FillRule::NonZero,
                        1 => FillRule::EvenOdd,
                        t => return Err(DisplayDecodeError::BadTag(t)),
                    },
                },
                1 => DrawOp::StrokePath {
                    path: r.path()?,
                    paint: r.paint()?,
                    style: r.stroke()?,
                },
                2 => {
                    let mut run = ShapedRun::new(r.u32()?, r.f32()?);
                    let n = r.u32()?;
                    for _ in 0..n {
                        let id = r.u32()?;
                        run.push(id, r.vec2()?);
                    }
                    DrawOp::DrawGlyphs {
                        run,
                        paint: r.paint()?,
                    }
                }
                3 => DrawOp::DrawImage {
                    image: ImageRef {
                        id: ImageId(r.u32()?),
                        width: r.u32()?,
                        height: r.u32()?,
                    },
                    src: r.rect()?,
                    dst: r.rect()?,
                },
                4 => DrawOp::PushClip(r.path()?),
                5 => DrawOp::PopClip,
                6 => DrawOp::PushTransform(r.affine()?),
                7 => DrawOp::PopTransform,
                8 => DrawOp::ClearPath(r.path()?),
                9 => DrawOp::PushLayer(r.f32()?),
                10 => DrawOp::PopLayer,
                12 => DrawOp::SetBlendMode(blend_from_code(r.u8()?)?),
                11 => DrawOp::FillPathBlurred {
                    path: r.path()?,
                    paint: r.paint()?,
                    rule: match r.u8()? {
                        0 => FillRule::NonZero,
                        1 => FillRule::EvenOdd,
                        t => return Err(DisplayDecodeError::BadTag(t)),
                    },
                    sigma: r.f32()?,
                },
                t => return Err(DisplayDecodeError::BadTag(t)),
            };
            list.push_op(op);
        }
        Ok(list)
    }
}

struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn vec2(&mut self, p: Vec2) {
        self.f32(p.x);
        self.f32(p.y);
    }
    fn rect(&mut self, r: Rect) {
        for v in [r.x, r.y, r.w, r.h] {
            self.f32(v);
        }
    }
    fn affine(&mut self, m: &Affine2) {
        for v in [m.a, m.b, m.c, m.d, m.e, m.f] {
            self.f32(v);
        }
    }
    fn color(&mut self, c: Color) {
        for v in [c.r, c.g, c.b, c.a] {
            self.f32(v);
        }
    }
    fn path(&mut self, p: &Path) {
        let bytes = p.serialize();
        self.u32(bytes.len() as u32);
        self.0.extend_from_slice(&bytes);
    }
    fn stops(&mut self, stops: &[Stop]) {
        self.u32(stops.len() as u32);
        for s in stops {
            self.f32(s.offset);
            self.color(s.color);
        }
    }
    fn paint(&mut self, p: &Paint) {
        match p {
            Paint::Solid(c) => {
                self.u8(0);
                self.color(*c);
            }
            Paint::LinearGradient {
                start,
                end,
                stops,
                spread,
                space,
            } => {
                self.u8(1);
                self.vec2(*start);
                self.vec2(*end);
                self.u8(spread_tag(*spread));
                self.u8(space_tag(*space));
                self.stops(stops);
            }
            Paint::RadialGradient {
                center,
                radius,
                stops,
                spread,
                space,
            } => {
                self.u8(2);
                self.vec2(*center);
                self.f32(*radius);
                self.u8(spread_tag(*spread));
                self.u8(space_tag(*space));
                self.stops(stops);
            }
            Paint::Pattern { image, transform } => {
                self.u8(3);
                self.u32(image.0);
                self.affine(transform);
            }
        }
    }
    fn stroke(&mut self, s: &StrokeStyle) {
        self.f32(s.width);
        self.u8(match s.cap {
            LineCap::Butt => 0,
            LineCap::Round => 1,
            LineCap::Square => 2,
        });
        self.u8(match s.join {
            LineJoin::Miter => 0,
            LineJoin::Round => 1,
            LineJoin::Bevel => 2,
        });
        self.f32(s.miter_limit);
        self.u32(s.dash.len() as u32);
        for d in &s.dash {
            self.f32(*d);
        }
        self.f32(s.dash_offset);
    }
}

fn spread_tag(s: SpreadMode) -> u8 {
    match s {
        SpreadMode::Pad => 0,
        SpreadMode::Repeat => 1,
        SpreadMode::Reflect => 2,
    }
}

fn space_tag(s: ColorSpace) -> u8 {
    match s {
        ColorSpace::Srgb => 0,
        ColorSpace::LinearSrgb => 1,
    }
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], DisplayDecodeError> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or(DisplayDecodeError::Truncated)?;
        if end > self.buf.len() {
            return Err(DisplayDecodeError::Truncated);
        }
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, DisplayDecodeError> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, DisplayDecodeError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn f32(&mut self) -> Result<f32, DisplayDecodeError> {
        let b = self.take(4)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn vec2(&mut self) -> Result<Vec2, DisplayDecodeError> {
        Ok(Vec2::new(self.f32()?, self.f32()?))
    }
    fn rect(&mut self) -> Result<Rect, DisplayDecodeError> {
        Ok(Rect::new(
            self.f32()?,
            self.f32()?,
            self.f32()?,
            self.f32()?,
        ))
    }
    fn affine(&mut self) -> Result<Affine2, DisplayDecodeError> {
        Ok(Affine2::new(
            self.f32()?,
            self.f32()?,
            self.f32()?,
            self.f32()?,
            self.f32()?,
            self.f32()?,
        ))
    }
    fn color(&mut self) -> Result<Color, DisplayDecodeError> {
        Ok(Color::rgba(
            self.f32()?,
            self.f32()?,
            self.f32()?,
            self.f32()?,
        ))
    }
    fn path(&mut self) -> Result<Path, DisplayDecodeError> {
        let n = self.u32()? as usize;
        Path::deserialize(self.take(n)?).map_err(DisplayDecodeError::Path)
    }
    fn stops(&mut self) -> Result<Vec<Stop>, DisplayDecodeError> {
        let n = self.u32()?;
        let mut v = Vec::new();
        for _ in 0..n {
            v.push(Stop {
                offset: self.f32()?,
                color: self.color()?,
            });
        }
        Ok(v)
    }
    fn spread(&mut self) -> Result<SpreadMode, DisplayDecodeError> {
        match self.u8()? {
            0 => Ok(SpreadMode::Pad),
            1 => Ok(SpreadMode::Repeat),
            2 => Ok(SpreadMode::Reflect),
            t => Err(DisplayDecodeError::BadTag(t)),
        }
    }
    fn space(&mut self) -> Result<ColorSpace, DisplayDecodeError> {
        match self.u8()? {
            0 => Ok(ColorSpace::Srgb),
            1 => Ok(ColorSpace::LinearSrgb),
            t => Err(DisplayDecodeError::BadTag(t)),
        }
    }
    fn paint(&mut self) -> Result<Paint, DisplayDecodeError> {
        match self.u8()? {
            0 => Ok(Paint::Solid(self.color()?)),
            1 => {
                let start = self.vec2()?;
                let end = self.vec2()?;
                let spread = self.spread()?;
                let space = self.space()?;
                Ok(Paint::LinearGradient {
                    start,
                    end,
                    stops: self.stops()?,
                    spread,
                    space,
                })
            }
            2 => {
                let center = self.vec2()?;
                let radius = self.f32()?;
                let spread = self.spread()?;
                let space = self.space()?;
                Ok(Paint::RadialGradient {
                    center,
                    radius,
                    stops: self.stops()?,
                    spread,
                    space,
                })
            }
            3 => Ok(Paint::Pattern {
                image: ImageId(self.u32()?),
                transform: self.affine()?,
            }),
            t => Err(DisplayDecodeError::BadTag(t)),
        }
    }
    fn stroke(&mut self) -> Result<StrokeStyle, DisplayDecodeError> {
        let width = self.f32()?;
        let cap = match self.u8()? {
            0 => LineCap::Butt,
            1 => LineCap::Round,
            2 => LineCap::Square,
            t => return Err(DisplayDecodeError::BadTag(t)),
        };
        let join = match self.u8()? {
            0 => LineJoin::Miter,
            1 => LineJoin::Round,
            2 => LineJoin::Bevel,
            t => return Err(DisplayDecodeError::BadTag(t)),
        };
        let miter_limit = self.f32()?;
        let n = self.u32()?;
        let mut dash = Vec::new();
        for _ in 0..n {
            dash.push(self.f32()?);
        }
        let dash_offset = self.f32()?;
        Ok(StrokeStyle {
            width,
            cap,
            join,
            miter_limit,
            dash,
            dash_offset,
        })
    }
}

/// Stable byte for each blend mode in the display list format. Do not reorder.
fn blend_code(mode: BlendMode) -> u8 {
    match mode {
        BlendMode::Normal => 0,
        BlendMode::Multiply => 1,
        BlendMode::Screen => 2,
        BlendMode::Overlay => 3,
        BlendMode::Darken => 4,
        BlendMode::Lighten => 5,
        BlendMode::Difference => 6,
        BlendMode::Exclusion => 7,
    }
}

fn blend_from_code(code: u8) -> Result<BlendMode, DisplayDecodeError> {
    Ok(match code {
        0 => BlendMode::Normal,
        1 => BlendMode::Multiply,
        2 => BlendMode::Screen,
        3 => BlendMode::Overlay,
        4 => BlendMode::Darken,
        5 => BlendMode::Lighten,
        6 => BlendMode::Difference,
        7 => BlendMode::Exclusion,
        t => return Err(DisplayDecodeError::BadTag(t)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Context, PathBuilder};
    use alloc::vec;

    #[test]
    fn round_trips_every_op_kind() {
        let mut ctx = Context::new(DisplayList::new());
        let mut b = PathBuilder::new();
        b.rounded_rect(1.0, 2.0, 30.0, 20.0, 4.0);
        let path = b.build();
        ctx.set_fill_paint(Paint::RadialGradient {
            center: Vec2::new(3.0, 4.0),
            radius: 9.0,
            stops: vec![
                Stop {
                    offset: 0.0,
                    color: Color::WHITE,
                },
                Stop {
                    offset: 1.0,
                    color: Color::rgba(0.0, 0.0, 0.0, 0.5),
                },
            ],
            spread: SpreadMode::Reflect,
            space: ColorSpace::LinearSrgb,
        });
        ctx.save();
        ctx.translate(5.0, 6.0);
        ctx.clip(&path);
        ctx.set_fill_rule(FillRule::EvenOdd);
        ctx.fill_path(&path);
        ctx.restore();
        ctx.set_stroke_style(StrokeStyle {
            width: 2.0,
            cap: LineCap::Square,
            join: LineJoin::Bevel,
            dash: vec![1.0, 2.0, 3.0],
            dash_offset: 0.5,
            ..StrokeStyle::default()
        });
        ctx.stroke_path(&path);
        ctx.draw_image(
            &ImageRef {
                id: ImageId(7),
                width: 2,
                height: 2,
            },
            Rect::new(0.0, 0.0, 2.0, 2.0),
            Rect::new(1.0, 1.0, 4.0, 4.0),
        );
        let mut run = ShapedRun::new(3, 12.0);
        run.push(65, Vec2::new(0.0, 1.0));
        ctx.draw_glyphs(&run);
        let list = ctx.into_backend();
        assert_eq!(DisplayList::deserialize(&list.serialize()).unwrap(), list);
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(
            DisplayList::deserialize(b"nope"),
            Err(DisplayDecodeError::BadMagic)
        );
        assert_eq!(
            DisplayList::deserialize(b"TPT"),
            Err(DisplayDecodeError::Truncated)
        );
        assert_eq!(
            DisplayList::deserialize(b"XXXX\x01\0\0\0\0"),
            Err(DisplayDecodeError::BadMagic)
        );
        let mut bytes = DisplayList::new().serialize();
        bytes[4] = 9;
        assert_eq!(
            DisplayList::deserialize(&bytes),
            Err(DisplayDecodeError::UnsupportedVersion(9))
        );
        let mut bad = DisplayList::new().serialize();
        bad[5..9].copy_from_slice(&1u32.to_le_bytes());
        bad.push(42);
        assert_eq!(
            DisplayList::deserialize(&bad),
            Err(DisplayDecodeError::BadTag(42))
        );
    }
}
