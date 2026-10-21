//! Benchmarks for the stages of the drawing pipeline. Run with `cargo bench -p tpt-canvas-bench`.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tpt_canvas::prelude::*;
use tpt_canvas::stroke::stroke_outline;
use tpt_canvas::tessellate::ear_clip;
use tpt_canvas_raster::RasterBackend;

/// A closed star with `n` points, so the curve work scales with `n`.
fn star(n: usize) -> Path {
    let mut b = PathBuilder::new();
    for i in 0..n * 2 {
        let r = if i % 2 == 0 { 100.0 } else { 40.0 };
        let a = i as f32 * core::f32::consts::PI / n as f32;
        let (x, y) = (128.0 + r * libm::cosf(a), 128.0 + r * libm::sinf(a));
        if i == 0 {
            b.move_to(x, y);
        } else {
            b.line_to(x, y);
        }
    }
    b.close();
    b.build()
}

/// A circle built from curves, so flattening has real work to do.
fn curved() -> Path {
    let mut b = PathBuilder::new();
    b.circle(128.0, 128.0, 100.0);
    b.build()
}

fn bench_flatten(c: &mut Criterion) {
    let path = curved();
    c.bench_function("flatten/circle_tol_0.01", |bench| {
        bench.iter(|| black_box(path.flatten(0.01)))
    });
}

fn bench_stroke(c: &mut Criterion) {
    let flat = star(32).flatten(0.01);
    let style = StrokeStyle {
        width: 4.0,
        dash: vec![6.0, 3.0],
        ..StrokeStyle::default()
    };
    c.bench_function("stroke/star32_dashed", |bench| {
        bench.iter(|| black_box(stroke_outline(&flat, &style, 0.01)))
    });
}

fn bench_tessellate(c: &mut Criterion) {
    let ring: Vec<Vec2> = star(64)
        .flatten(0.01)
        .polylines
        .into_iter()
        .flat_map(|p| p.points)
        .collect();
    c.bench_function("tessellate/ear_clip_star64", |bench| {
        bench.iter(|| black_box(ear_clip(&ring, &[])))
    });
}

fn bench_raster_fill(c: &mut Criterion) {
    let path = star(32);
    let paint = Paint::solid(named::BLUE);
    c.bench_function("raster/fill_star32_256", |bench| {
        bench.iter(|| {
            let mut ctx = Context::new(RasterBackend::new(256, 256));
            ctx.set_fill_paint(paint.clone());
            ctx.fill_path(&path);
            black_box(ctx.into_backend().into_pixmap())
        })
    });
}

criterion_group!(
    benches,
    bench_flatten,
    bench_stroke,
    bench_tessellate,
    bench_raster_fill
);
criterion_main!(benches);
