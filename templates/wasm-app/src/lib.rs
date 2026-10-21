//! A browser drawing with the Canvas2D backend. Build with
//! `wasm-pack build --target web`, then open `index.html` through a local web server.

use tpt_canvas::prelude::*;
use tpt_canvas_canvas2d::Canvas2dBackend;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

/// Draws the scene onto `canvas`, clearing it first.
#[wasm_bindgen]
pub fn draw(canvas: &HtmlCanvasElement) -> Result<(), JsValue> {
    let ctx2d: CanvasRenderingContext2d = canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("no 2d context"))?
        .dyn_into()?;
    let (w, h) = (canvas.width() as f32, canvas.height() as f32);
    let mut ctx = Context::new(Canvas2dBackend::new(ctx2d));
    ctx.clear_rect(0.0, 0.0, w, h);

    ctx.set_fill_paint(Paint::linear(
        Vec2::new(0.0, 0.0),
        Vec2::new(w, 0.0),
        &[(0.0, named::BLUE), (1.0, named::CYAN)],
    ));
    let mut card = PathBuilder::new();
    card.rounded_rect(16.0, 16.0, w - 32.0, h - 32.0, 16.0);
    ctx.fill_path(&card.build());
    Ok(())
}
