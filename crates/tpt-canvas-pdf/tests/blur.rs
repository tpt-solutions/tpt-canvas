//! Blurred fills are rendered by the raster backend and embedded as image XObjects with an
//! alpha mask. These tests check the structure. The rendering was checked by hand with a PDF
//! renderer (PyMuPDF) against the raster output of the hero example.

use tpt_canvas::prelude::*;
use tpt_canvas::{BlendMode, CanvasBackend};
use tpt_canvas_pdf::PdfBackend;

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).into_owned()
}

fn square(x: f32, y: f32, w: f32, h: f32) -> Path {
    let mut b = PathBuilder::new();
    b.rect(x, y, w, h);
    b.build()
}

/// A 100 by 100 page with one blurred red fill of `path`.
fn blurred(path: &Path, sigma: f32) -> PdfBackend {
    let mut backend = PdfBackend::new(100.0, 100.0);
    backend.fill_path_blurred(path, &Paint::solid(named::RED), FillRule::NonZero, sigma);
    backend
}

#[test]
fn blurred_fill_is_an_image_with_an_alpha_mask() {
    let backend = blurred(&square(10.0, 10.0, 20.0, 20.0), 2.0);
    assert_eq!(backend.unsupported(), 0, "blur is drawn, not counted");
    let pdf = text(backend.finish());
    assert!(pdf.contains("/Subtype /Image"), "no image XObject");
    assert!(pdf.contains("/ColorSpace /DeviceRGB"));
    assert!(pdf.contains("/ColorSpace /DeviceGray"), "no alpha mask");
    assert!(pdf.contains("/SMask "), "image does not refer to its mask");
    assert!(pdf.contains("/Im0 Do"), "content does not draw the image");
}

#[test]
fn image_covers_the_shape_plus_three_sigmas() {
    // The 20-unit square grows by 3 * 2 = 6 on each side, so the image is 32 by 32.
    let pdf = text(blurred(&square(10.0, 10.0, 20.0, 20.0), 2.0).finish());
    assert!(
        pdf.contains("/Width 32 /Height 32"),
        "unexpected image size"
    );
}

#[test]
fn zero_sigma_is_a_plain_fill() {
    let pdf = text(blurred(&square(10.0, 10.0, 20.0, 20.0), 0.0).finish());
    assert!(
        !pdf.contains("/Subtype /Image"),
        "zero sigma must not make an image"
    );
}

#[test]
fn oversized_blur_is_counted_and_skipped() {
    let backend = blurred(&square(0.0, 0.0, 5000.0, 10.0), 1.0);
    assert_eq!(backend.unsupported(), 1);
    assert!(!text(backend.finish()).contains("/Subtype /Image"));
}

#[test]
fn normal_fill_after_a_blend_says_it_is_normal() {
    // A state without /BM keeps the mode in force before it, so once a blend mode has been
    // used, every later state names its mode. Here the fill after the multiply must reset.
    let mut ctx = Context::new(PdfBackend::new(100.0, 100.0));
    ctx.set_blend_mode(BlendMode::Multiply);
    ctx.fill_rect(0.0, 0.0, 10.0, 10.0);
    ctx.set_blend_mode(BlendMode::Normal);
    ctx.fill_rect(20.0, 20.0, 10.0, 10.0);
    let pdf = text(ctx.into_backend().finish());
    assert!(pdf.contains("/BM /Multiply"));
    assert!(
        pdf.contains("/BM /Normal"),
        "normal fill does not reset the blend mode"
    );
}

#[test]
fn documents_without_blending_have_no_blend_entries() {
    let mut ctx = Context::new(PdfBackend::new(100.0, 100.0));
    ctx.fill_rect(0.0, 0.0, 10.0, 10.0);
    let pdf = text(ctx.into_backend().finish());
    assert!(!pdf.contains("/BM"), "no blend mode was used, so no /BM");
}
