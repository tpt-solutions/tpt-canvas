//! Colour helpers layered over the colour type: hex and 8-bit parsing, mixing and named colours.
//!
//! These live in this crate, not on `Color` itself, because `Color` comes from the
//! `tpt-color` stub and will be swapped for the real crate. A trait import keeps the
//! same call sites working after the swap.

mod base;
pub use base::*;

/// Constructors and adjustments for [`Color`]. Import the trait to use them.
pub trait ColorExt: Sized {
    /// Builds an opaque colour from 8-bit channels.
    fn from_rgb8(r: u8, g: u8, b: u8) -> Self;
    /// Builds a colour from 8-bit channels and 8-bit alpha.
    fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self;
    /// Parses `#rgb`, `#rrggbb` or `#rrggbbaa`. The leading `#` is optional.
    fn from_hex(s: &str) -> Option<Self>;
    /// Linear blend from `self` (`t = 0`) to `other` (`t = 1`), applied to all four channels in sRGB encoding.
    fn mix(self, other: Self, t: f32) -> Self;
    /// Blends towards white by `t`, keeping alpha.
    fn lighten(self, t: f32) -> Self;
    /// Blends towards black by `t`, keeping alpha.
    fn darken(self, t: f32) -> Self;
}

fn channel(v: u8) -> f32 {
    f32::from(v) / 255.0
}

impl ColorExt for Color {
    fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        Color::rgb(channel(r), channel(g), channel(b))
    }

    fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color::rgba(channel(r), channel(g), channel(b), channel(a))
    }

    fn from_hex(s: &str) -> Option<Self> {
        let digits = s.strip_prefix('#').unwrap_or(s);
        if !digits.bytes().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        // Expand `#rgb` to `#rrggbb` by doubling each digit.
        let expanded;
        let digits = match digits.len() {
            3 | 4 => {
                let mut out = [0u8; 8];
                for (i, c) in digits.bytes().enumerate() {
                    out[2 * i] = c;
                    out[2 * i + 1] = c;
                }
                expanded = out;
                core::str::from_utf8(&expanded[..2 * digits.len()]).ok()?
            }
            6 | 8 => digits,
            _ => return None,
        };
        let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
        let r = byte(0)?;
        let g = byte(2)?;
        let b = byte(4)?;
        let a = if digits.len() == 8 { byte(6)? } else { 255 };
        Some(Color::from_rgba8(r, g, b, a))
    }

    fn mix(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let lerp = |a: f32, b: f32| a + (b - a) * t;
        Color::rgba(
            lerp(self.r, other.r),
            lerp(self.g, other.g),
            lerp(self.b, other.b),
            lerp(self.a, other.a),
        )
    }

    fn lighten(self, t: f32) -> Self {
        let a = self.a;
        Color {
            a,
            ..self.mix(Color::WHITE, t)
        }
    }

    fn darken(self, t: f32) -> Self {
        let a = self.a;
        Color {
            a,
            ..self.mix(Color::BLACK, t)
        }
    }
}

/// Named colours in sRGB.
pub mod named {
    use crate::Color;

    /// Fully transparent black.
    pub const TRANSPARENT: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);
    /// `#000000`.
    pub const BLACK: Color = Color::rgb(0.0, 0.0, 0.0);
    /// `#ffffff`.
    pub const WHITE: Color = Color::rgb(1.0, 1.0, 1.0);
    /// `#808080`.
    pub const GRAY: Color = Color::rgb(0.5019608, 0.5019608, 0.5019608);
    /// `#ff0000`.
    pub const RED: Color = Color::rgb(1.0, 0.0, 0.0);
    /// `#00ff00`.
    pub const GREEN: Color = Color::rgb(0.0, 1.0, 0.0);
    /// `#0000ff`.
    pub const BLUE: Color = Color::rgb(0.0, 0.0, 1.0);
    /// `#ffff00`.
    pub const YELLOW: Color = Color::rgb(1.0, 1.0, 0.0);
    /// `#00ffff`.
    pub const CYAN: Color = Color::rgb(0.0, 1.0, 1.0);
    /// `#ff00ff`.
    pub const MAGENTA: Color = Color::rgb(1.0, 0.0, 1.0);
}
