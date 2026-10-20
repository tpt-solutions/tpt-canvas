//! Parser for SVG path data (`d` attribute), and the `path!` macro built on it.

use alloc::string::String;
use core::fmt;

use super::{Path, PathBuilder};
use crate::Vec2;

/// A parse failure in [`Path::from_svg_d`], with the byte offset where it happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SvgPathError {
    /// Byte offset into the input.
    pub offset: usize,
}

impl fmt::Display for SvgPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid SVG path data at byte {}", self.offset)
    }
}

impl Path {
    /// Parses SVG path data such as `"M 0 0 L 10 10 Z"` or `"m1 1c2 0 3 4 5 5"`.
    ///
    /// All commands are supported, absolute and relative: `M L H V C S Q T A Z`. Arcs
    /// are converted to cubics the same way [`PathBuilder::elliptical_arc`] does it.
    pub fn from_svg_d(d: &str) -> Result<Path, SvgPathError> {
        let mut lex = Lexer {
            s: d.as_bytes(),
            i: 0,
        };
        let mut b = PathBuilder::new();
        let mut cur = Vec2::ZERO;
        let mut start = Vec2::ZERO;
        let mut cmd: Option<u8> = None;
        let mut started = false;
        // Last control points, for the reflected S and T commands.
        let mut ctrl_c: Option<Vec2> = None;
        let mut ctrl_q: Option<Vec2> = None;

        loop {
            lex.skip_separators();
            if lex.at_end() {
                break;
            }
            let at = lex.i;
            let c = lex.peek();
            if c.is_ascii_alphabetic() {
                cmd = Some(c);
                lex.i += 1;
            } else {
                // Numbers after a command repeat it. After M/m they mean L/l.
                match cmd {
                    Some(b'M') => cmd = Some(b'L'),
                    Some(b'm') => cmd = Some(b'l'),
                    Some(b'Z' | b'z') | None => return Err(lex.error()),
                    Some(_) => {}
                }
            }
            let c = cmd.ok_or_else(|| lex.error())?;
            if !started && !matches!(c, b'M' | b'm') {
                return Err(SvgPathError { offset: at });
            }
            started = true;

            let prev_c = ctrl_c.take();
            let prev_q = ctrl_q.take();
            let rel = c.is_ascii_lowercase();
            let origin = if rel { cur } else { Vec2::ZERO };
            match c.to_ascii_uppercase() {
                b'M' => {
                    let p = origin + lex.point()?;
                    b.move_to(p.x, p.y);
                    cur = p;
                    start = p;
                }
                b'L' => {
                    let p = origin + lex.point()?;
                    b.line_to(p.x, p.y);
                    cur = p;
                }
                b'H' => {
                    let x = lex.number()? + if rel { cur.x } else { 0.0 };
                    cur = Vec2::new(x, cur.y);
                    b.line_to(cur.x, cur.y);
                }
                b'V' => {
                    let y = lex.number()? + if rel { cur.y } else { 0.0 };
                    cur = Vec2::new(cur.x, y);
                    b.line_to(cur.x, cur.y);
                }
                b'C' => {
                    let c1 = origin + lex.point()?;
                    let c2 = origin + lex.point()?;
                    let p = origin + lex.point()?;
                    b.cubic_to(c1.x, c1.y, c2.x, c2.y, p.x, p.y);
                    ctrl_c = Some(c2);
                    cur = p;
                }
                b'S' => {
                    let c1 = prev_c.map_or(cur, |c2| cur * 2.0 - c2);
                    let c2 = origin + lex.point()?;
                    let p = origin + lex.point()?;
                    b.cubic_to(c1.x, c1.y, c2.x, c2.y, p.x, p.y);
                    ctrl_c = Some(c2);
                    cur = p;
                }
                b'Q' => {
                    let q = origin + lex.point()?;
                    let p = origin + lex.point()?;
                    b.quad_to(q.x, q.y, p.x, p.y);
                    ctrl_q = Some(q);
                    cur = p;
                }
                b'T' => {
                    let q = prev_q.map_or(cur, |q| cur * 2.0 - q);
                    let p = origin + lex.point()?;
                    b.quad_to(q.x, q.y, p.x, p.y);
                    ctrl_q = Some(q);
                    cur = p;
                }
                b'A' => {
                    let rx = lex.number()?;
                    let ry = lex.number()?;
                    let rot = lex.number()?.to_radians();
                    let large = lex.flag()?;
                    let sweep = lex.flag()?;
                    let p = origin + lex.point()?;
                    b.elliptical_arc(rx, ry, rot, large, sweep, p.x, p.y);
                    cur = p;
                }
                b'Z' => {
                    b.close();
                    cur = start;
                    // Z takes no numbers, so a following number is an error.
                    cmd = None;
                }
                _ => return Err(lex.error()),
            }
        }
        Ok(b.build())
    }
}

/// Builds a [`Path`] from SVG path syntax at the call site: `path!{ M 0 0 L 10 10 Z }`.
///
/// The data is parsed at run time, so an invalid literal panics on first use. Tests
/// that build paths with this macro catch such mistakes.
#[macro_export]
macro_rules! path {
    ($($t:tt)*) => {
        $crate::Path::from_svg_d(stringify!($($t)*)).expect("invalid SVG path in path!")
    };
}

struct Lexer<'a> {
    s: &'a [u8],
    i: usize,
}

impl Lexer<'_> {
    fn at_end(&self) -> bool {
        self.i >= self.s.len()
    }

    fn peek(&self) -> u8 {
        self.s[self.i]
    }

    fn error(&self) -> SvgPathError {
        SvgPathError { offset: self.i }
    }

    fn skip_separators(&mut self) {
        while !self.at_end() && (self.peek().is_ascii_whitespace() || self.peek() == b',') {
            self.i += 1;
        }
    }

    fn number(&mut self) -> Result<f32, SvgPathError> {
        self.skip_separators();
        let s = self.s;
        let mut j = self.i;
        let mut text = String::new();
        if j < s.len() && (s[j] == b'+' || s[j] == b'-') {
            text.push(s[j] as char);
            j += 1;
            // Allow spaces after the sign, as stringify! can produce for `- 5`.
            while j < s.len() && s[j].is_ascii_whitespace() {
                j += 1;
            }
        }
        let mut digits = 0;
        while j < s.len() && s[j].is_ascii_digit() {
            text.push(s[j] as char);
            j += 1;
            digits += 1;
        }
        if j < s.len() && s[j] == b'.' {
            text.push('.');
            j += 1;
            while j < s.len() && s[j].is_ascii_digit() {
                text.push(s[j] as char);
                j += 1;
                digits += 1;
            }
        }
        if digits == 0 {
            return Err(self.error());
        }
        if j < s.len() && (s[j] == b'e' || s[j] == b'E') {
            let mut k = j + 1;
            let mut exp = String::from("e");
            if k < s.len() && (s[k] == b'+' || s[k] == b'-') {
                exp.push(s[k] as char);
                k += 1;
            }
            let first_digit = k;
            while k < s.len() && s[k].is_ascii_digit() {
                exp.push(s[k] as char);
                k += 1;
            }
            if k > first_digit {
                text.push_str(&exp);
                j = k;
            }
        }
        let value: f32 = text.parse().map_err(|_| self.error())?;
        self.i = j;
        Ok(value)
    }

    fn point(&mut self) -> Result<Vec2, SvgPathError> {
        let x = self.number()?;
        let y = self.number()?;
        Ok(Vec2::new(x, y))
    }

    fn flag(&mut self) -> Result<bool, SvgPathError> {
        self.skip_separators();
        if self.at_end() {
            return Err(self.error());
        }
        let v = match self.peek() {
            b'0' => false,
            b'1' => true,
            _ => return Err(self.error()),
        };
        self.i += 1;
        Ok(v)
    }
}
