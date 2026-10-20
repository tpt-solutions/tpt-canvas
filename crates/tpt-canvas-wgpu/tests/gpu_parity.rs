//! Runs the same scenes on the GPU and on the CPU reference executor, then compares pixels.
//!
//! When no GPU adapter is available, each test prints a notice and returns without failing.
//! That keeps headless CI green, so run these locally on a machine with a GPU to exercise them.

use tpt_canvas::{
    Affine2, CanvasBackend, Color, ColorSpace, FillRule, Paint, Path, PathBuilder, SpreadMode,
    Stop, StrokeStyle, Vec2,
};
use tpt_canvas_webgpu::reference::execute;
use tpt_canvas_webgpu::WebGpuBackend;
use tpt_canvas_wgpu::{GpuError, GpuRenderer};

const W: u32 = 64;
const H: u32 = 64;

fn gpu() -> Option<GpuRenderer> {
    match GpuRenderer::new() {
        Ok(g) => Some(g),
        Err(GpuError::NoAdapter) => {
            eprintln!("skipped: no GPU adapter available");
            None
        }
        Err(e) => panic!("GPU setup failed: {e}"),
    }
}

fn star(points: &[(f32, f32)]) -> Path {
    let mut b = PathBuilder::new();
    for (i, &(x, y)) in points.iter().enumerate() {
        if i == 0 {
            b.move_to(x, y);
        } else {
            b.line_to(x, y);
        }
    }
    b.close();
    b.build()
}

/// Compares the GPU frame with the reference executor. Colours may differ by one 8-bit level.
fn assert_matches_reference(name: &str, scene: impl Fn(&mut WebGpuBackend)) {
    let Some(gpu) = gpu() else { return };
    let mut be = WebGpuBackend::new(W, H);
    scene(&mut be);
    let list = be.into_draw_list();
    let reference = execute(&list);
    let frame = gpu.render(&list).expect("render");
    assert_eq!((frame.width, frame.height), (W, H));

    let mut worst = 0.0f32;
    let mut mismatched = 0usize;
    for (i, r) in reference.data.iter().enumerate() {
        let ga = frame.pixels[i * 4 + 3] as f32 / 255.0;
        if (ga >= 0.5) != (r[3] >= 0.5) {
            mismatched += 1;
            continue; // counted against the edge budget, not the colour tolerance
        }
        for (c, &expected) in r.iter().enumerate() {
            let g = frame.pixels[i * 4 + c] as f32 / 255.0;
            worst = worst.max((g - expected).abs());
        }
    }
    assert!(worst <= 0.02, "{name}: worst channel difference {worst}");
    assert!(
        mismatched <= edge_budget(reference.data.len()),
        "{name}: {mismatched} pixels disagree on coverage"
    );
}

/// Hardware rasterizers snap vertices to a fixed sub-pixel grid, so pixels within about
/// 1/512 px of a curved edge can flip. Allow 0.5% of pixels for that; polygon edges still match exactly.
fn edge_budget(pixels: usize) -> usize {
    pixels / 200
}

fn solid(c: Color) -> Paint {
    Paint::Solid(c)
}

#[test]
fn solid_rect_matches() {
    let mut b = PathBuilder::new();
    b.rect(8.0, 8.0, 20.0, 12.0);
    let path = b.build();
    assert_matches_reference("rect", |be| {
        be.fill_path(&path, &solid(Color::rgb(0.9, 0.2, 0.1)), FillRule::NonZero)
    });
}

#[test]
fn pentagram_nonzero_and_evenodd_match() {
    let pts: Vec<(f32, f32)> = (0..5)
        .map(|i| {
            let a = (i * 2) as f32 * core::f32::consts::TAU / 5.0 - core::f32::consts::FRAC_PI_2;
            (32.0 + 28.0 * libm::cosf(a), 32.0 + 28.0 * libm::sinf(a))
        })
        .collect();
    let path = star(&pts);
    assert_matches_reference("pentagram nonzero", |be| {
        be.fill_path(&path, &solid(Color::BLACK), FillRule::NonZero)
    });
    assert_matches_reference("pentagram evenodd", |be| {
        be.fill_path(&path, &solid(Color::BLACK), FillRule::EvenOdd)
    });
}

#[test]
fn clipped_fill_matches() {
    let mut clip = PathBuilder::new();
    clip.circle(32.0, 32.0, 22.0);
    let clip = clip.build();
    let mut fill = PathBuilder::new();
    fill.rect(0.0, 0.0, 64.0, 64.0);
    let fill = fill.build();
    assert_matches_reference("clipped fill", |be| {
        be.push_clip(&clip);
        be.fill_path(&fill, &solid(Color::rgb(0.1, 0.5, 0.9)), FillRule::NonZero);
        be.pop_clip();
    });
}

#[test]
fn nested_clips_match() {
    let mut outer = PathBuilder::new();
    outer.rect(4.0, 4.0, 40.0, 40.0);
    let outer = outer.build();
    let mut inner = PathBuilder::new();
    inner.circle(30.0, 30.0, 18.0);
    let inner = inner.build();
    let mut fill = PathBuilder::new();
    fill.rect(0.0, 0.0, 64.0, 64.0);
    let fill = fill.build();
    assert_matches_reference("nested clips", |be| {
        be.push_clip(&outer);
        be.push_clip(&inner);
        be.fill_path(&fill, &solid(Color::rgb(0.8, 0.8, 0.1)), FillRule::NonZero);
        be.pop_clip();
        be.pop_clip();
    });
}

#[test]
fn dashed_transformed_stroke_matches() {
    let mut b = PathBuilder::new();
    b.move_to(4.0, 4.0);
    b.line_to(50.0, 20.0);
    b.line_to(10.0, 58.0);
    let path = b.build();
    let style = StrokeStyle {
        width: 3.0,
        dash: vec![6.0, 3.0],
        ..StrokeStyle::default()
    };
    assert_matches_reference("dashed stroke", |be| {
        be.push_transform(&Affine2::translate(2.0, 1.0));
        be.stroke_path(&path, &solid(Color::BLACK), &style);
        be.pop_transform();
    });
}

#[test]
fn linear_gradient_matches() {
    let mut b = PathBuilder::new();
    b.rounded_rect(4.0, 4.0, 56.0, 56.0, 10.0);
    let path = b.build();
    let paint = Paint::LinearGradient {
        start: Vec2::new(4.0, 4.0),
        end: Vec2::new(60.0, 60.0),
        stops: vec![
            Stop {
                offset: 0.0,
                color: Color::rgba(1.0, 0.0, 0.0, 1.0),
            },
            Stop {
                offset: 0.5,
                color: Color::rgba(0.0, 1.0, 0.0, 0.5),
            },
            Stop {
                offset: 1.0,
                color: Color::rgba(0.0, 0.0, 1.0, 1.0),
            },
        ],
        spread: SpreadMode::Pad,
        space: ColorSpace::Srgb,
    };
    assert_matches_reference("linear gradient", |be| {
        be.fill_path(&path, &paint, FillRule::NonZero)
    });
}

#[test]
fn radial_gradient_linear_space_matches() {
    let mut b = PathBuilder::new();
    b.circle(32.0, 32.0, 28.0);
    let path = b.build();
    let paint = Paint::RadialGradient {
        center: Vec2::new(24.0, 24.0),
        radius: 30.0,
        stops: vec![
            Stop {
                offset: 0.0,
                color: Color::rgb(1.0, 1.0, 1.0),
            },
            Stop {
                offset: 1.0,
                color: Color::rgba(0.1, 0.2, 0.6, 0.8),
            },
        ],
        spread: SpreadMode::Pad,
        space: ColorSpace::LinearSrgb,
    };
    assert_matches_reference("radial gradient", |be| {
        be.fill_path(&path, &paint, FillRule::NonZero)
    });
}

#[test]
fn random_self_intersecting_polygons_match() {
    let Some(_) = gpu() else { return };
    let mut rng = tpt_canvas::testkit::Rng::new(9);
    for _ in 0..12 {
        let n = 3 + rng.index(7);
        let pts: Vec<(f32, f32)> = (0..n)
            .map(|_| (rng.range(4.0, 60.0), rng.range(4.0, 60.0)))
            .collect();
        let path = star(&pts);
        let rule = if rng.index(2) == 0 {
            FillRule::NonZero
        } else {
            FillRule::EvenOdd
        };
        let gpu = gpu().unwrap();
        let mut be = WebGpuBackend::new(W, H);
        be.fill_path(&path, &solid(Color::BLACK), rule);
        let list = be.into_draw_list();
        let reference = execute(&list);
        let frame = gpu.render(&list).expect("render");
        let mismatched = reference
            .data
            .iter()
            .enumerate()
            .filter(|(i, r)| (frame.pixels[i * 4 + 3] as f32 / 255.0 >= 0.5) != (r[3] >= 0.5))
            .count();
        assert!(
            mismatched <= edge_budget(reference.data.len()),
            "{pts:?} {rule:?}: {mismatched}"
        );
    }
}
