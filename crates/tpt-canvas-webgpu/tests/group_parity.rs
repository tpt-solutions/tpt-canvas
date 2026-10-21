//! Group opacity on the WebGPU reference executor must match the raster backend. Shapes sit on
//! whole pixels, so coverage is exact on both backends.

use tpt_canvas::{CanvasBackend, Color, FillRule, Paint, Path, PathBuilder};
use tpt_canvas_raster::RasterBackend;
use tpt_canvas_webgpu::reference::execute;
use tpt_canvas_webgpu::WebGpuBackend;

const W: u32 = 32;
const H: u32 = 32;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Path {
    let mut b = PathBuilder::new();
    b.rect(x, y, w, h);
    b.build()
}

fn red() -> Paint {
    Paint::Solid(Color::rgb(1.0, 0.0, 0.0))
}

fn blue() -> Paint {
    Paint::Solid(Color::rgb(0.0, 0.0, 1.0))
}

fn assert_same(name: &str, scene: impl Fn(&mut dyn CanvasBackend)) {
    let mut raster = RasterBackend::new(W, H);
    scene(&mut raster);
    let want = raster.into_pixmap().data;

    let mut gpu = WebGpuBackend::new(W, H);
    scene(&mut gpu);
    assert_eq!(
        gpu.draw_list().unsupported,
        0,
        "{name}: groups are drawn, not counted"
    );
    let got = execute(gpu.draw_list()).data;

    for (i, (w, g)) in want.iter().zip(&got).enumerate() {
        for c in 0..4 {
            assert!(
                (w[c] - g[c]).abs() < 1e-4,
                "{name} pixel {i} channel {c}: raster {w:?}, reference {g:?}"
            );
        }
    }
}

#[test]
fn overlaps_inside_a_group_are_not_faded_twice() {
    assert_same("overlap", |be| {
        be.fill_path(
            &rect(0.0, 0.0, 32.0, 32.0),
            &Paint::Solid(Color::WHITE),
            FillRule::NonZero,
        );
        be.push_layer(0.5);
        be.fill_path(&rect(4.0, 4.0, 16.0, 16.0), &red(), FillRule::NonZero);
        be.fill_path(&rect(12.0, 12.0, 16.0, 16.0), &red(), FillRule::NonZero);
        be.pop_layer();
    });
}

#[test]
fn nested_groups_compose_in_order() {
    assert_same("nested", |be| {
        be.push_layer(0.8);
        be.fill_path(&rect(0.0, 0.0, 24.0, 24.0), &blue(), FillRule::NonZero);
        be.push_layer(0.5);
        be.fill_path(&rect(8.0, 8.0, 24.0, 24.0), &red(), FillRule::NonZero);
        be.pop_layer();
        be.pop_layer();
    });
}

#[test]
fn a_group_over_a_backdrop_and_under_a_clip() {
    assert_same("clipped group", |be| {
        be.fill_path(&rect(0.0, 0.0, 32.0, 32.0), &blue(), FillRule::NonZero);
        be.push_clip(&rect(6.0, 6.0, 20.0, 20.0));
        be.push_layer(0.25);
        be.fill_path(&rect(0.0, 0.0, 32.0, 32.0), &red(), FillRule::NonZero);
        be.pop_layer();
        be.pop_clip();
    });
}
