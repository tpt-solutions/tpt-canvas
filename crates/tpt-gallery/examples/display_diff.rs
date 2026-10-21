//! Diffs two recorded drawings op by op. The second drawing changes one shape and adds one,
//! so the output shows which ops stayed the same and which did not.
//! Run with `cargo run -p tpt-gallery --example display_diff`.

use tpt_canvas::display::diff::{diff, DiffOp};
use tpt_canvas::prelude::*;
use tpt_canvas::{DisplayList, DrawOp};

/// Records the scene. `colour` changes the middle shape, and `extra` adds a last shape.
fn record(colour: Color, extra: bool) -> DisplayList {
    let mut ctx = Context::new(DisplayList::new());
    ctx.set_fill_paint(Paint::solid(named::BLUE));
    ctx.fill_rect(0.0, 0.0, 10.0, 10.0);
    ctx.set_fill_paint(Paint::solid(colour));
    ctx.fill_circle(30.0, 30.0, 8.0);
    ctx.set_fill_paint(Paint::solid(named::GREEN));
    ctx.fill_rect(50.0, 50.0, 10.0, 10.0);
    if extra {
        ctx.fill_rect(70.0, 10.0, 5.0, 5.0);
    }
    ctx.into_backend()
}

fn describe(op: &DrawOp) -> &'static str {
    match op {
        DrawOp::FillPath { .. } => "fill_path",
        DrawOp::StrokePath { .. } => "stroke_path",
        DrawOp::SetBlendMode(_) => "set_blend_mode",
        DrawOp::PushClip(_) => "push_clip",
        DrawOp::PopClip => "pop_clip",
        _ => "other",
    }
}

fn main() {
    let before = record(named::RED, false);
    let after = record(named::MAGENTA, true);
    let steps = diff(&before, &after).expect("small lists");

    for step in &steps {
        match *step {
            DiffOp::Same { a, .. } => println!("  {:>2}  {}", a, describe(&before.ops()[a])),
            DiffOp::Removed(a) => println!("- {:>2}  {}", a, describe(&before.ops()[a])),
            DiffOp::Added(b) => println!("+ {:>2}  {}", b, describe(&after.ops()[b])),
        }
    }
    let changed = steps
        .iter()
        .filter(|s| !matches!(s, DiffOp::Same { .. }))
        .count();
    println!("{changed} of {} steps changed", steps.len());
}
