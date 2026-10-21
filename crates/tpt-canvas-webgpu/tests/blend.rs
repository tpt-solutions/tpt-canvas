//! Blended fills are recorded on their command, so the executors can draw them. Only the
//! patterns and the features that have no command yet are still counted.

use tpt_canvas::prelude::*;
use tpt_canvas::BlendMode;
use tpt_canvas_webgpu::{Command, WebGpuBackend};

#[test]
fn blended_fills_and_strokes_carry_their_mode_and_are_not_counted() {
    let mut ctx = Context::new(WebGpuBackend::new(16, 16));
    ctx.fill_rect(0.0, 0.0, 4.0, 4.0);
    ctx.set_blend_mode(BlendMode::Difference);
    ctx.fill_rect(0.0, 0.0, 4.0, 4.0);
    ctx.stroke_rect(0.0, 0.0, 4.0, 4.0);
    let list = ctx.backend().draw_list();
    assert_eq!(list.unsupported, 0);
    let modes: Vec<BlendMode> = list
        .commands
        .iter()
        .map(|c| match c {
            Command::Fill { blend, .. } => *blend,
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(
        modes,
        [
            BlendMode::Normal,
            BlendMode::Difference,
            BlendMode::Difference
        ]
    );
}
