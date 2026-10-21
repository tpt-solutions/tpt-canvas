//! Blend modes are native in PDF: they go into the ExtGState used by the drawing.

use tpt_canvas::prelude::*;
use tpt_canvas::BlendMode;
use tpt_canvas_pdf::PdfBackend;

#[test]
fn blend_mode_is_written_into_an_extgstate() {
    let mut ctx = Context::new(PdfBackend::new(20.0, 20.0));
    ctx.set_blend_mode(BlendMode::Multiply);
    ctx.fill_rect(0.0, 0.0, 5.0, 5.0);
    let backend = ctx.into_backend();
    assert_eq!(backend.unsupported(), 0, "PDF blends natively");
    let pdf = String::from_utf8_lossy(&backend.finish()).into_owned();
    assert!(pdf.contains("/BM /Multiply"), "no /BM entry in the output");
}

#[test]
fn gradient_fill_gets_the_blend_mode_too() {
    let mut ctx = Context::new(PdfBackend::new(20.0, 20.0));
    ctx.set_blend_mode(BlendMode::Screen);
    ctx.set_fill_paint(Paint::linear(
        Vec2::new(0.0, 0.0),
        Vec2::new(20.0, 0.0),
        &[(0.0, named::RED), (1.0, named::BLUE)],
    ));
    ctx.fill_rect(0.0, 0.0, 20.0, 20.0);
    let pdf = String::from_utf8_lossy(&ctx.into_backend().finish()).into_owned();
    assert!(
        pdf.contains("/BM /Screen"),
        "gradient ignored the blend mode"
    );
}

#[test]
fn normal_output_has_no_blend_entry() {
    let mut ctx = Context::new(PdfBackend::new(20.0, 20.0));
    ctx.fill_rect(0.0, 0.0, 5.0, 5.0);
    let pdf = String::from_utf8_lossy(&ctx.into_backend().finish()).into_owned();
    assert!(!pdf.contains("/BM"), "normal blending must not add /BM");
}

#[test]
fn non_separable_modes_use_their_pdf_names() {
    for (mode, name) in [
        (BlendMode::Hue, "/BM /Hue"),
        (BlendMode::Saturation, "/BM /Saturation"),
        (BlendMode::Color, "/BM /Color"),
        (BlendMode::Luminosity, "/BM /Luminosity"),
    ] {
        let mut ctx = Context::new(PdfBackend::new(10.0, 10.0));
        ctx.set_blend_mode(mode);
        ctx.fill_rect(0.0, 0.0, 5.0, 5.0);
        let pdf = String::from_utf8_lossy(&ctx.into_backend().finish()).into_owned();
        assert!(pdf.contains(name), "{mode:?}: expected {name}");
    }
}
