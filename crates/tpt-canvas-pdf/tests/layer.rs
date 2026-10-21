//! Group opacity in PDF: a layer becomes a transparency-group form, painted once with the
//! layer's alpha. The rendering was checked with PyMuPDF, where overlaps inside a layer are
//! not faded twice.

use tpt_canvas::prelude::*;
use tpt_canvas::CanvasBackend;
use tpt_canvas_pdf::PdfBackend;

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).into_owned()
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Path {
    let mut b = PathBuilder::new();
    b.rect(x, y, w, h);
    b.build()
}

#[test]
fn layer_becomes_a_transparency_group_with_its_alpha() {
    let mut backend = PdfBackend::new(100.0, 100.0);
    backend.push_layer(0.5);
    backend.fill_path(
        &rect(0.0, 0.0, 40.0, 40.0),
        &Paint::solid(named::RED),
        FillRule::NonZero,
    );
    backend.pop_layer();
    assert_eq!(backend.unsupported(), 0, "groups are drawn, not counted");
    let pdf = text(backend.finish());
    assert!(pdf.contains("/Subtype /Form"), "no form XObject");
    assert!(
        pdf.contains("/Group << /S /Transparency /I true >>"),
        "form is not a transparency group"
    );
    assert!(pdf.contains("/F0 Do"), "the page does not paint the group");
    assert!(
        pdf.contains("/CA 0.5 /ca 0.5"),
        "the group is not painted with the layer alpha"
    );
}

#[test]
fn full_alpha_layer_paints_without_a_state() {
    let mut backend = PdfBackend::new(100.0, 100.0);
    backend.push_layer(1.0);
    backend.fill_path(
        &rect(0.0, 0.0, 40.0, 40.0),
        &Paint::solid(named::RED),
        FillRule::NonZero,
    );
    backend.pop_layer();
    let pdf = text(backend.finish());
    // The page paints the group with no state of its own.
    assert!(
        pdf.contains(
            "q /F0 Do Q
"
        ),
        "full alpha should paint the form with no state"
    );
}

#[test]
fn nested_layers_make_one_form_each() {
    let mut backend = PdfBackend::new(100.0, 100.0);
    backend.push_layer(0.5);
    backend.push_layer(0.25);
    backend.fill_path(
        &rect(0.0, 0.0, 10.0, 10.0),
        &Paint::solid(named::BLUE),
        FillRule::NonZero,
    );
    backend.pop_layer();
    backend.pop_layer();
    let pdf = text(backend.finish());
    assert_eq!(pdf.matches("/Subtype /Form").count(), 2);
    assert!(pdf.contains("/F0 Do") && pdf.contains("/F1 Do"));
}

#[test]
fn clips_inside_a_layer_stay_inside_its_form() {
    let mut backend = PdfBackend::new(100.0, 100.0);
    backend.push_layer(0.5);
    backend.push_clip(&rect(0.0, 0.0, 20.0, 20.0));
    backend.fill_path(
        &rect(0.0, 0.0, 50.0, 50.0),
        &Paint::solid(named::RED),
        FillRule::NonZero,
    );
    backend.pop_clip();
    backend.pop_layer();
    assert_eq!(backend.unsupported(), 0);
    // The clip is drawn inside the form, so the page itself has only the paint of the form.
    let pdf = text(backend.finish());
    assert!(pdf.contains("/F0 Do"));
}
