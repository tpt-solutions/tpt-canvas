//! A tiny scene language for the browser playground.
//!
//! A script is one command per line. Blank lines and lines starting with `#` are ignored.
//!
//! | Command | Meaning |
//! |---|---|
//! | `fill-color #rrggbb` | Sets the fill colour. |
//! | `stroke-color #rrggbb` | Sets the stroke colour. |
//! | `line-width w` | Sets the stroke width. |
//! | `alpha a` | Sets the global alpha, from 0 to 1. |
//! | `blend <mode>` | Sets the blend mode, for example `multiply` or `hue`. |
//! | `shadow <blur> <dx> <dy> [#rrggbb]` | Sets a drop shadow. The colour defaults to black at 60%. |
//! | `shadow off` | Removes the shadow. |
//! | `rect x y w h` | Fills a rectangle. |
//! | `stroke-rect x y w h` | Strokes a rectangle. |
//! | `circle cx cy r` | Fills a circle. |
//! | `text size x y words...` | Draws a label with the built-in stroke font. |
//! | `translate x y`, `rotate radians`, `scale sx sy` | Changes the transform. |
//! | `save`, `restore` | Pushes and pops the drawing state. |
//!
//! [`parse`] reports the first error with its line number. [`run`] draws parsed commands
//! into any `Context`. The browser entry point is `draw_script`, which exists only on wasm.

#![warn(missing_docs)]

use tpt_canvas::prelude::*;
use tpt_canvas::text::StrokeFont;
use tpt_canvas::{BlendMode, Shadow};

/// One drawing command, after parsing.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Sets the fill colour.
    FillColor(Color),
    /// Sets the stroke colour.
    StrokeColor(Color),
    /// Sets the stroke width.
    LineWidth(f32),
    /// Sets the global alpha.
    Alpha(f32),
    /// Sets the blend mode.
    Blend(BlendMode),
    /// Sets a shadow.
    Shadow(Shadow),
    /// Removes the shadow.
    ShadowOff,
    /// Fills a rectangle.
    Rect(f32, f32, f32, f32),
    /// Strokes a rectangle.
    StrokeRect(f32, f32, f32, f32),
    /// Fills a circle.
    Circle(f32, f32, f32),
    /// Draws text at a baseline point, with a size in pixels.
    Text {
        /// Font size in user units.
        size: f32,
        /// Left edge of the baseline.
        x: f32,
        /// Baseline y.
        y: f32,
        /// The words to draw.
        text: String,
    },
    /// Translates the user space.
    Translate(f32, f32),
    /// Rotates the user space by radians.
    Rotate(f32),
    /// Scales the user space.
    Scale(f32, f32),
    /// Pushes the drawing state.
    Save,
    /// Pops the drawing state.
    Restore,
}

/// A parse failure: the 1-based line number and what was wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// The line that failed, counting from 1.
    pub line: usize,
    /// What was wrong with it.
    pub message: String,
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

/// Parses a script into commands. Stops at the first error.
pub fn parse(script: &str) -> Result<Vec<Command>, ParseError> {
    let mut out = Vec::new();
    for (i, raw) in script.lines().enumerate() {
        let line = i + 1;
        let text = raw.trim();
        if text.is_empty() || text.starts_with('#') {
            continue;
        }
        let cmd = parse_line(text).map_err(|message| ParseError { line, message })?;
        out.push(cmd);
    }
    Ok(out)
}

fn parse_line(text: &str) -> Result<Command, String> {
    let mut parts = text.split_whitespace();
    let name = parts.next().ok_or("empty command")?;
    let rest: Vec<&str> = parts.collect();
    let args = |n: usize| -> Result<Vec<f32>, String> {
        if rest.len() != n {
            return Err(format!("{name} takes {n} numbers, got {}", rest.len()));
        }
        rest.iter().map(|s| number(s)).collect()
    };
    let none = |cmd: Command| -> Result<Command, String> {
        if rest.is_empty() {
            Ok(cmd)
        } else {
            Err(format!("{name} takes no arguments"))
        }
    };
    match name {
        "fill-color" => Ok(Command::FillColor(one_colour(&rest, name)?)),
        "stroke-color" => Ok(Command::StrokeColor(one_colour(&rest, name)?)),
        "line-width" => {
            let a = args(1)?;
            Ok(Command::LineWidth(a[0]))
        }
        "alpha" => {
            let a = args(1)?;
            Ok(Command::Alpha(a[0]))
        }
        "blend" => {
            if rest.len() != 1 {
                return Err("blend takes one mode name".into());
            }
            Ok(Command::Blend(blend_mode(rest[0])?))
        }
        "shadow" => shadow(&rest),
        "rect" => {
            let a = args(4)?;
            Ok(Command::Rect(a[0], a[1], a[2], a[3]))
        }
        "stroke-rect" => {
            let a = args(4)?;
            Ok(Command::StrokeRect(a[0], a[1], a[2], a[3]))
        }
        "circle" => {
            let a = args(3)?;
            Ok(Command::Circle(a[0], a[1], a[2]))
        }
        "text" => {
            if rest.len() < 4 {
                return Err("text takes a size, x, y and words".into());
            }
            let size = number(rest[0])?;
            let x = number(rest[1])?;
            let y = number(rest[2])?;
            Ok(Command::Text {
                size,
                x,
                y,
                text: rest[3..].join(" "),
            })
        }
        "translate" => {
            let a = args(2)?;
            Ok(Command::Translate(a[0], a[1]))
        }
        "rotate" => {
            let a = args(1)?;
            Ok(Command::Rotate(a[0]))
        }
        "scale" => {
            let a = args(2)?;
            Ok(Command::Scale(a[0], a[1]))
        }
        "save" => none(Command::Save),
        "restore" => none(Command::Restore),
        other => Err(format!("unknown command '{other}'")),
    }
}

fn number(s: &str) -> Result<f32, String> {
    let v: f32 = s.parse().map_err(|_| format!("'{s}' is not a number"))?;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(format!("'{s}' is not a finite number"))
    }
}

fn one_colour(rest: &[&str], name: &str) -> Result<Color, String> {
    if rest.len() != 1 {
        return Err(format!("{name} takes one colour"));
    }
    colour(rest[0])
}

fn colour(s: &str) -> Result<Color, String> {
    Color::from_hex(s).ok_or_else(|| format!("'{s}' is not a #rrggbb colour"))
}

fn blend_mode(name: &str) -> Result<BlendMode, String> {
    Ok(match name {
        "normal" => BlendMode::Normal,
        "multiply" => BlendMode::Multiply,
        "screen" => BlendMode::Screen,
        "overlay" => BlendMode::Overlay,
        "darken" => BlendMode::Darken,
        "lighten" => BlendMode::Lighten,
        "difference" => BlendMode::Difference,
        "exclusion" => BlendMode::Exclusion,
        "hue" => BlendMode::Hue,
        "saturation" => BlendMode::Saturation,
        "color" => BlendMode::Color,
        "luminosity" => BlendMode::Luminosity,
        other => return Err(format!("unknown blend mode '{other}'")),
    })
}

fn shadow(rest: &[&str]) -> Result<Command, String> {
    if rest == ["off"] {
        return Ok(Command::ShadowOff);
    }
    if rest.len() != 3 && rest.len() != 4 {
        return Err("shadow takes 'off', or a blur, dx, dy and optional colour".into());
    }
    let color = match rest.get(3) {
        Some(c) => colour(c)?,
        None => Color::rgba(0.0, 0.0, 0.0, 0.6),
    };
    Ok(Command::Shadow(Shadow {
        color,
        offset: Vec2::new(number(rest[1])?, number(rest[2])?),
        blur: number(rest[0])?,
    }))
}

/// Draws parsed commands into `ctx`. A `restore` without a matching `save` is ignored,
/// as in `Context`.
pub fn run<B: CanvasBackend>(ctx: &mut Context<B>, commands: &[Command]) {
    ctx.set_fill_paint(Paint::solid(Color::BLACK));
    ctx.set_stroke_paint(Paint::solid(Color::BLACK));
    ctx.set_glyph_source(Box::new(StrokeFont));
    for cmd in commands {
        match cmd {
            Command::FillColor(c) => ctx.set_fill_paint(Paint::solid(*c)),
            Command::StrokeColor(c) => ctx.set_stroke_paint(Paint::solid(*c)),
            Command::LineWidth(w) => ctx.set_stroke_style(StrokeStyle {
                width: *w,
                ..StrokeStyle::default()
            }),
            Command::Alpha(a) => ctx.set_global_alpha(*a),
            Command::Blend(m) => ctx.set_blend_mode(*m),
            Command::Shadow(s) => ctx.set_shadow(*s),
            Command::ShadowOff => ctx.clear_shadow(),
            Command::Rect(x, y, w, h) => ctx.fill_rect(*x, *y, *w, *h),
            Command::StrokeRect(x, y, w, h) => ctx.stroke_rect(*x, *y, *w, *h),
            Command::Circle(cx, cy, r) => ctx.fill_circle(*cx, *cy, *r),
            Command::Text { size, x, y, text } => {
                // The font lays glyphs out from the origin, so move them to the label's place.
                let mut run = StrokeFont::layout(text, *size);
                for g in &mut run.glyphs {
                    g.pos.x += x;
                    g.pos.y += y;
                }
                ctx.draw_glyphs(&run);
            }
            Command::Translate(x, y) => ctx.translate(*x, *y),
            Command::Rotate(t) => ctx.rotate(*t),
            Command::Scale(x, y) => ctx.scale(*x, *y),
            Command::Save => ctx.save(),
            Command::Restore => ctx.restore(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_canvas_raster::RasterBackend;

    #[test]
    fn parses_each_command_kind() {
        let cmds = parse(
            "# comment\n\
             fill-color #ff0000\n\
             stroke-color #00ff00\n\
             line-width 3\n\
             alpha 0.5\n\
             blend multiply\n\
             shadow 4 2 3\n\
             shadow off\n\
             rect 1 2 3 4\n\
             stroke-rect 1 2 3 4\n\
             circle 5 5 2\n\
             text 12 3 9 HELLO WORLD\n\
             translate 1 1\n\
             rotate 0.5\n\
             scale 2 2\n\
             save\n\
             restore\n",
        )
        .unwrap();
        assert_eq!(cmds.len(), 16);
        assert_eq!(cmds[0], Command::FillColor(Color::rgb(1.0, 0.0, 0.0)));
        assert_eq!(cmds[4], Command::Blend(BlendMode::Multiply));
        assert_eq!(
            cmds[10],
            Command::Text {
                size: 12.0,
                x: 3.0,
                y: 9.0,
                text: "HELLO WORLD".into()
            }
        );
    }

    #[test]
    fn shadow_defaults_to_translucent_black() {
        let cmds = parse("shadow 6 1 1").unwrap();
        match &cmds[0] {
            Command::Shadow(s) => {
                assert_eq!(s.color, Color::rgba(0.0, 0.0, 0.0, 0.6));
                assert_eq!(s.blur, 6.0);
            }
            other => panic!("expected a shadow, got {other:?}"),
        }
    }

    #[test]
    fn errors_name_the_line() {
        let err = parse("rect 1 2 3 4\nrect 1 two 3 4").unwrap_err();
        assert_eq!(err.line, 2);
        assert!(err.message.contains("'two' is not a number"), "{err}");
    }

    #[test]
    fn unknown_commands_and_modes_are_rejected() {
        assert!(parse("wobble 1")
            .unwrap_err()
            .message
            .contains("unknown command"));
        assert!(parse("blend sparkle")
            .unwrap_err()
            .message
            .contains("unknown blend mode"));
        assert!(parse("fill-color red")
            .unwrap_err()
            .message
            .contains("#rrggbb"));
        assert!(parse("rect 1 2 3")
            .unwrap_err()
            .message
            .contains("takes 4 numbers"));
        assert!(parse("rect 1 inf 3 4").is_err());
    }

    #[test]
    fn run_draws_a_filled_rectangle() {
        let cmds = parse("fill-color #0000ff\nrect 2 2 4 4").unwrap();
        let mut ctx = Context::new(RasterBackend::new(8, 8));
        run(&mut ctx, &cmds);
        let p = ctx.backend().pixmap().pixel(3, 3);
        assert!(p[2] > 0.99 && p[3] > 0.99, "pixel was {p:?}");
    }

    #[test]
    fn text_draws_glyph_pixels() {
        let cmds = parse("fill-color #000000\ntext 20 2 18 I").unwrap();
        let mut ctx = Context::new(RasterBackend::new(40, 24));
        run(&mut ctx, &cmds);
        let ink: f32 = ctx.backend().pixmap().data.iter().map(|p| p[3]).sum();
        assert!(ink > 10.0, "no glyph ink: {ink}");
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use tpt_canvas::prelude::*;
    use tpt_canvas_canvas2d::Canvas2dBackend;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

    /// Parses `script` and draws it onto `canvas`, clearing the canvas first.
    ///
    /// Returns an error message for a parse failure or a missing 2D context. The page
    /// shows the message instead of drawing.
    #[wasm_bindgen]
    pub fn draw_script(canvas: &HtmlCanvasElement, script: &str) -> Result<(), JsValue> {
        let commands = super::parse(script).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let ctx: CanvasRenderingContext2d = canvas
            .get_context("2d")?
            .ok_or_else(|| JsValue::from_str("canvas has no 2d context"))?
            .dyn_into()?;
        let (w, h) = (canvas.width() as f32, canvas.height() as f32);
        let mut ctx = Context::new(Canvas2dBackend::new(ctx));
        ctx.clear_rect(0.0, 0.0, w, h);
        super::run(&mut ctx, &commands);
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::draw_script;

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_examples {}
