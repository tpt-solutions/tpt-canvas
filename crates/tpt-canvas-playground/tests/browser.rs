//! Browser test for the playground entry point: a script is drawn onto a real canvas.
//! Run like the Canvas2D tests, with headless Chrome and a matching chromedriver.

#![cfg(target_arch = "wasm32")]

use tpt_canvas_playground::draw_script;
use wasm_bindgen::{Clamped, JsCast};
use wasm_bindgen_test::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

wasm_bindgen_test_configure!(run_in_browser);

fn canvas(w: u32, h: u32) -> HtmlCanvasElement {
    let document = web_sys::window().unwrap().document().unwrap();
    let el: HtmlCanvasElement = document
        .create_element("canvas")
        .unwrap()
        .dyn_into()
        .unwrap();
    el.set_width(w);
    el.set_height(h);
    document.body().unwrap().append_child(&el).unwrap();
    el
}

fn pixel(el: &HtmlCanvasElement, x: f64, y: f64) -> [u8; 4] {
    let ctx: CanvasRenderingContext2d = el.get_context("2d").unwrap().unwrap().dyn_into().unwrap();
    let data = ctx.get_image_data(x, y, 1.0, 1.0).unwrap().data();
    let px: Clamped<Vec<u8>> = data;
    [px[0], px[1], px[2], px[3]]
}

#[wasm_bindgen_test]
fn script_draws_onto_the_canvas() {
    let el = canvas(32, 32);
    draw_script(&el, "fill-color #00ff00\nrect 4 4 10 10").unwrap();
    assert_eq!(pixel(&el, 8.0, 8.0), [0, 255, 0, 255]);
    assert_eq!(pixel(&el, 25.0, 25.0)[3], 0);
}

#[wasm_bindgen_test]
fn redrawing_clears_the_previous_picture() {
    let el = canvas(32, 32);
    draw_script(&el, "fill-color #ff0000\nrect 0 0 32 32").unwrap();
    draw_script(&el, "fill-color #0000ff\nrect 0 0 4 4").unwrap();
    assert_eq!(pixel(&el, 20.0, 20.0)[3], 0, "old fill should be gone");
    assert_eq!(pixel(&el, 2.0, 2.0), [0, 0, 255, 255]);
}

#[wasm_bindgen_test]
fn bad_script_returns_an_error_and_leaves_the_canvas() {
    let el = canvas(16, 16);
    let err = draw_script(&el, "rect 1 2 3").unwrap_err();
    let msg = err.as_string().unwrap_or_default();
    assert!(msg.contains("line 1"), "{msg}");
}
