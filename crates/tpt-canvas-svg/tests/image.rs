//! `draw_image` in SVG: the cropped pixels go into the file as a PNG data URI. The tests
//! decode that URI and check the pixels, so the base64 and PNG encoders are both covered.

use tpt_canvas::{CanvasBackend, ImageId, ImageRef, Rect};
use tpt_canvas_raster::Pixmap;
use tpt_canvas_svg::SvgBackend;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect { x, y, w, h }
}

/// Decodes standard base64 with padding. Test-only, so it is written for clarity.
fn base64_decode(text: &str) -> Vec<u8> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0;
    for c in text.bytes().take_while(|&c| c != b'=') {
        let v = ALPHABET.iter().position(|&a| a == c).expect("base64 digit") as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    out
}

/// The PNG inside the first data URI of the document.
fn embedded_png(svg: &str) -> Vec<u8> {
    let start =
        svg.find("data:image/png;base64,").expect("data URI") + "data:image/png;base64,".len();
    let end = start + svg[start..].find('"').expect("end of URI");
    base64_decode(&svg[start..end])
}

fn two_by_two() -> Pixmap {
    let rgba = [
        255, 0, 0, 255, 0, 255, 0, 255, //
        0, 0, 255, 255, 255, 255, 0, 255,
    ];
    Pixmap::from_rgba8(2, 2, &rgba).unwrap()
}

#[test]
fn embedded_png_decodes_to_the_source_pixels() {
    let mut svg = SvgBackend::new(100.0, 100.0);
    svg.add_image(ImageId(1), two_by_two());
    let image = ImageRef {
        id: ImageId(1),
        width: 2,
        height: 2,
    };
    svg.draw_image(&image, rect(0.0, 0.0, 2.0, 2.0), rect(0.0, 0.0, 50.0, 50.0));
    assert_eq!(svg.unsupported(), 0);
    let doc = svg.finish();
    assert!(doc.contains(r#"image-rendering="pixelated""#));

    let pm = Pixmap::from_png(&embedded_png(&doc)).expect("embedded PNG decodes");
    assert_eq!((pm.width, pm.height), (2, 2));
    assert_eq!(pm.pixel(0, 0), [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(pm.pixel(1, 0), [0.0, 1.0, 0.0, 1.0]);
    assert_eq!(pm.pixel(0, 1), [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(pm.pixel(1, 1), [1.0, 1.0, 0.0, 1.0]);
}

#[test]
fn source_rectangle_crops_before_embedding() {
    let mut svg = SvgBackend::new(100.0, 100.0);
    svg.add_image(ImageId(1), two_by_two());
    let image = ImageRef {
        id: ImageId(1),
        width: 2,
        height: 2,
    };
    // The right column only: green over yellow.
    svg.draw_image(&image, rect(1.0, 0.0, 1.0, 2.0), rect(0.0, 0.0, 10.0, 10.0));
    let pm = Pixmap::from_png(&embedded_png(&svg.finish())).unwrap();
    assert_eq!((pm.width, pm.height), (1, 2));
    assert_eq!(pm.pixel(0, 0), [0.0, 1.0, 0.0, 1.0]);
    assert_eq!(pm.pixel(0, 1), [1.0, 1.0, 0.0, 1.0]);
}

#[test]
fn unregistered_image_is_counted() {
    let mut svg = SvgBackend::new(10.0, 10.0);
    let image = ImageRef {
        id: ImageId(7),
        width: 2,
        height: 2,
    };
    svg.draw_image(&image, rect(0.0, 0.0, 2.0, 2.0), rect(0.0, 0.0, 5.0, 5.0));
    assert_eq!(svg.unsupported(), 1);
    assert!(!svg.finish().contains("data:image"));
}

#[test]
fn base64_round_trips_every_padding_case() {
    // Lengths 1, 2 and 3 mod 3 exercise one, two and no '=' padding characters.
    for len in 1..8u8 {
        let mut svg = SvgBackend::new(10.0, 10.0);
        let rgba: Vec<u8> = (0..len as usize * 4).map(|i| (i * 37) as u8).collect();
        svg.add_image(
            ImageId(1),
            Pixmap::from_rgba8(len as u32, 1, &rgba).unwrap(),
        );
        let image = ImageRef {
            id: ImageId(1),
            width: len as u32,
            height: 1,
        };
        svg.draw_image(
            &image,
            rect(0.0, 0.0, len as f32, 1.0),
            rect(0.0, 0.0, 10.0, 10.0),
        );
        let pm = Pixmap::from_png(&embedded_png(&svg.finish())).unwrap();
        assert_eq!(pm.width, len as u32);
    }
}
