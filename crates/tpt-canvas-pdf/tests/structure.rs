//! Structural checks on the PDF output: header, xref offsets, balanced graphics state,
//! and a golden file for a scene with gradients, dashes, clips and transforms.

use std::path::PathBuf;

use tpt_canvas::{
    Affine2, CanvasBackend, Color, ColorSpace, Context, FillRule, LineCap, Paint, PathBuilder,
    SpreadMode, Stop, StrokeStyle, Vec2,
};
use tpt_canvas_pdf::PdfBackend;
use tpt_testkit_stub::check_golden;

fn scene<B: CanvasBackend>(ctx: &mut Context<B>) {
    let mut bg = PathBuilder::new();
    bg.rounded_rect(10.0, 10.0, 180.0, 120.0, 12.0);
    ctx.set_fill_paint(Paint::LinearGradient {
        start: Vec2::new(10.0, 10.0),
        end: Vec2::new(190.0, 130.0),
        stops: vec![
            Stop {
                offset: 0.0,
                color: Color::rgb(0.1, 0.4, 0.9),
            },
            Stop {
                offset: 0.5,
                color: Color::rgb(0.9, 0.9, 0.2),
            },
            Stop {
                offset: 1.0,
                color: Color::rgb(0.9, 0.2, 0.2),
            },
        ],
        spread: SpreadMode::Pad,
        space: ColorSpace::Srgb,
    });
    ctx.fill_path(&bg.build());

    ctx.save();
    let mut clip = PathBuilder::new();
    clip.circle(100.0, 70.0, 50.0);
    ctx.clip(&clip.build());
    ctx.translate(100.0, 70.0);
    ctx.rotate(0.3);
    let mut inner = PathBuilder::new();
    inner.rect(-40.0, -40.0, 80.0, 80.0);
    ctx.set_fill_rule(FillRule::EvenOdd);
    ctx.set_fill_paint(Paint::Solid(Color::rgba(0.0, 0.0, 0.0, 0.5)));
    ctx.fill_path(&inner.build());
    ctx.restore();

    let mut curve = PathBuilder::new();
    curve.move_to(10.0, 150.0);
    curve.quad_to(100.0, 110.0, 190.0, 150.0);
    ctx.set_stroke_style(StrokeStyle {
        width: 3.0,
        cap: LineCap::Round,
        dash: vec![8.0, 4.0],
        ..StrokeStyle::default()
    });
    ctx.set_stroke_paint(Paint::Solid(Color::rgb(0.0, 0.0, 0.0)));
    ctx.stroke_path(&curve.build());

    ctx.set_stroke_paint(Paint::LinearGradient {
        start: Vec2::new(0.0, 0.0),
        end: Vec2::new(200.0, 0.0),
        stops: vec![
            Stop {
                offset: 0.0,
                color: Color::rgb(1.0, 0.0, 0.0),
            },
            Stop {
                offset: 1.0,
                color: Color::rgb(0.0, 0.0, 1.0),
            },
        ],
        spread: SpreadMode::Pad,
        space: ColorSpace::Srgb,
    });
    ctx.transform(Affine2::translate(0.0, 2.0));
    let mut base = PathBuilder::new();
    base.move_to(10.0, 180.0);
    base.line_to(190.0, 180.0);
    ctx.stroke_path(&base.build());
}

fn render() -> Vec<u8> {
    let mut ctx = Context::new(PdfBackend::new(200.0, 200.0));
    scene(&mut ctx);
    ctx.into_backend().finish()
}

/// Byte offset of `needle`'s first occurrence at or after `from`.
fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

#[test]
fn xref_offsets_point_at_their_objects() {
    let pdf = render();
    assert!(pdf.starts_with(b"%PDF-1.7\n"));

    let sx = find(&pdf, b"startxref\n", 0).expect("startxref");
    let num_start = sx + b"startxref\n".len();
    let num_end = find(&pdf, b"\n", num_start).unwrap();
    let xref_at: usize = std::str::from_utf8(&pdf[num_start..num_end])
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(&pdf[xref_at..xref_at + 4], b"xref");

    // Each in-use entry is exactly 20 bytes and must point at "N 0 obj".
    // Subsection header "0 N" follows "xref\n"; entry 0 starts on the next line.
    let header_end = find(&pdf, b"\n", xref_at + 5).unwrap();
    let count: usize = std::str::from_utf8(&pdf[xref_at + 5..header_end])
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    // Entry 0 is the free head; objects 1..count follow it.
    let mut pos = header_end + 1 + 20;
    for obj in 1..count {
        let entry = &pdf[pos..pos + 20];
        let off: usize = std::str::from_utf8(&entry[0..10]).unwrap().parse().unwrap();
        assert_eq!(&entry[17..18], b"n", "entry {obj}");
        let expected = format!("{obj} 0 obj");
        assert_eq!(
            &pdf[off..off + expected.len()],
            expected.as_bytes(),
            "object {obj}"
        );
        pos += 20;
    }
}

#[test]
fn graphics_state_is_balanced_and_shadings_are_used() {
    let pdf = String::from_utf8_lossy(&render()).into_owned();
    let stream_start = pdf.find("stream\n").unwrap() + "stream\n".len();
    let stream_end = pdf.find("endstream").unwrap();
    let content = &pdf[stream_start..stream_end];

    let mut depth: i32 = 0;
    for line in content.lines() {
        if line == "q" || line.starts_with("q ") {
            depth += 1;
        } else if line == "Q" {
            depth -= 1;
        }
        assert!(depth >= 0, "unbalanced Q");
    }
    assert_eq!(depth, 0, "unclosed q");
    assert!(content.contains(" sh\n"), "gradients should use sh");
    assert!(content.contains("W n\n"), "clips should use W n");
    assert!(
        content.contains("[8 4] 0 d"),
        "dash pattern should be emitted"
    );
    assert!(
        pdf.contains("/ShadingType 2"),
        "shading dictionary in resources"
    );
    assert!(
        pdf.contains("/FunctionType 3"),
        "three-stop gradient uses stitching"
    );
}

#[test]
fn scene_matches_golden_pdf() {
    let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/scene.pdf");
    check_golden(&golden, &render()).unwrap();
}

#[test]
fn rendering_is_deterministic() {
    assert_eq!(render(), render());
}
