//! `draw_image` in PDF: registered pixmaps are cropped to the source rectangle and embedded as
//! image XObjects. The rendering was also checked with a PDF renderer.

use tpt_canvas::{CanvasBackend, ImageId, ImageRef, Rect};
use tpt_canvas_pdf::PdfBackend;
use tpt_canvas_raster::Pixmap;

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).into_owned()
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect { x, y, w, h }
}

fn two_by_two() -> Pixmap {
    // Opaque red, green, blue and yellow, row by row.
    let rgba = [
        255, 0, 0, 255, 0, 255, 0, 255, //
        0, 0, 255, 255, 255, 255, 0, 255,
    ];
    Pixmap::from_rgba8(2, 2, &rgba).expect("valid size")
}

fn image_ref(id: u32, w: u32, h: u32) -> ImageRef {
    ImageRef {
        id: ImageId(id),
        width: w,
        height: h,
    }
}

#[test]
fn registered_image_is_embedded_and_drawn() {
    let mut backend = PdfBackend::new(100.0, 100.0);
    backend.add_image(ImageId(1), two_by_two());
    backend.draw_image(
        &image_ref(1, 2, 2),
        rect(0.0, 0.0, 2.0, 2.0),
        rect(10.0, 10.0, 40.0, 40.0),
    );
    assert_eq!(backend.unsupported(), 0);
    let pdf = text(backend.finish());
    assert!(pdf.contains("/Width 2 /Height 2"), "image size missing");
    assert!(pdf.contains("/Im0 Do"), "image not drawn");
}

#[test]
fn source_rectangle_crops_the_embedded_pixels() {
    let mut backend = PdfBackend::new(100.0, 100.0);
    backend.add_image(ImageId(1), two_by_two());
    // A source rectangle inside one pixel snaps outwards to that pixel.
    backend.draw_image(
        &image_ref(1, 2, 2),
        rect(1.0, 0.0, 0.5, 0.5),
        rect(0.0, 0.0, 10.0, 10.0),
    );
    let pdf = text(backend.finish());
    assert!(
        pdf.contains("/Width 1 /Height 1"),
        "crop did not reduce the image"
    );
}

#[test]
fn unregistered_image_is_counted_and_skipped() {
    let mut backend = PdfBackend::new(100.0, 100.0);
    backend.draw_image(
        &image_ref(9, 2, 2),
        rect(0.0, 0.0, 2.0, 2.0),
        rect(0.0, 0.0, 10.0, 10.0),
    );
    assert_eq!(backend.unsupported(), 1);
    assert!(!text(backend.finish()).contains("/Subtype /Image"));
}

#[test]
fn empty_destination_or_source_draws_nothing() {
    let mut backend = PdfBackend::new(100.0, 100.0);
    backend.add_image(ImageId(1), two_by_two());
    backend.draw_image(
        &image_ref(1, 2, 2),
        rect(0.0, 0.0, 2.0, 2.0),
        rect(0.0, 0.0, 0.0, 10.0),
    );
    backend.draw_image(
        &image_ref(1, 2, 2),
        rect(5.0, 5.0, 2.0, 2.0),
        rect(0.0, 0.0, 10.0, 10.0),
    );
    assert_eq!(backend.unsupported(), 0);
    assert!(!text(backend.finish()).contains("/Subtype /Image"));
}

#[test]
fn re_registering_replaces_the_pixels() {
    let mut backend = PdfBackend::new(100.0, 100.0);
    backend.add_image(ImageId(1), two_by_two());
    let one = Pixmap::from_rgba8(1, 1, &[0, 0, 0, 255]).unwrap();
    backend.add_image(ImageId(1), one);
    backend.draw_image(
        &image_ref(1, 1, 1),
        rect(0.0, 0.0, 1.0, 1.0),
        rect(0.0, 0.0, 10.0, 10.0),
    );
    assert!(text(backend.finish()).contains("/Width 1 /Height 1"));
}
