//! Records a small scene into a `DisplayList` and prints each recorded operation.
//! Useful for checking what a drawing routine actually sends to a backend.

use tpt_canvas::path;
use tpt_canvas::prelude::*;

fn main() {
    let mut ctx = Context::new(DisplayList::new());
    ctx.save();
    ctx.translate(10.0, 10.0);
    ctx.clip(&path! { M 0 0 L 50 0 L 50 50 L 0 50 Z });
    ctx.set_fill_paint(Paint::solid(named::RED));
    ctx.fill_circle(25.0, 25.0, 20.0);
    ctx.restore();

    for (i, op) in ctx.into_backend().ops().iter().enumerate() {
        println!("{i:>3}  {op:?}");
    }
}
