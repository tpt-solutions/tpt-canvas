//! The command list has no blend op yet, so blended draws are counted, not hidden.

use tpt_canvas::prelude::*;
use tpt_canvas::BlendMode;
use tpt_canvas_webgpu::WebGpuBackend;

#[test]
fn blended_draws_are_counted_and_normal_draws_are_not() {
    let mut ctx = Context::new(WebGpuBackend::new(16, 16));
    ctx.fill_rect(0.0, 0.0, 4.0, 4.0);
    assert_eq!(ctx.backend().draw_list().unsupported, 0);

    ctx.set_blend_mode(BlendMode::Difference);
    ctx.fill_rect(0.0, 0.0, 4.0, 4.0);
    ctx.stroke_rect(0.0, 0.0, 4.0, 4.0);
    assert_eq!(ctx.backend().draw_list().unsupported, 2);
}
