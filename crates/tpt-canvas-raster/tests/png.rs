//! PNG output: header, chunk layout, and a stream that decodes back to the same pixels.

use tpt_canvas::{Color, Context, Paint, PathBuilder};
use tpt_canvas_raster::{Pixmap, RasterBackend};

fn chunks(png: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut i = 8;
    while i < png.len() {
        let len = u32::from_be_bytes(png[i..i + 4].try_into().unwrap()) as usize;
        let kind = String::from_utf8(png[i + 4..i + 8].to_vec()).unwrap();
        out.push((kind, png[i + 8..i + 8 + len].to_vec()));
        i += 12 + len;
    }
    out
}

#[test]
fn png_has_ihdr_idat_iend_and_a_valid_zlib_header() {
    let mut ctx = Context::new(RasterBackend::new(5, 3));
    let mut b = PathBuilder::new();
    b.rect(0.0, 0.0, 5.0, 3.0);
    ctx.set_fill_paint(Paint::Solid(Color::rgb(1.0, 0.0, 0.0)));
    ctx.fill_path(&b.build());
    let pixmap: Pixmap = ctx.into_backend().into_pixmap();

    let png = pixmap.to_png();
    let parts = chunks(&png);
    let kinds: Vec<&str> = parts.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(kinds, ["IHDR", "IDAT", "IEND"]);

    let ihdr = &parts[0].1;
    assert_eq!(u32::from_be_bytes(ihdr[0..4].try_into().unwrap()), 5);
    assert_eq!(u32::from_be_bytes(ihdr[4..8].try_into().unwrap()), 3);
    assert_eq!(&ihdr[8..13], &[8, 6, 0, 0, 0]);

    // zlib header: deflate with a 32 KiB window, and the two header bytes must be a multiple of 31.
    let idat = &parts[1].1;
    assert_eq!(idat[0] & 0x0F, 8, "compression method is deflate");
    assert_eq!(
        (idat[0] as u16 * 256 + idat[1] as u16) % 31,
        0,
        "header check bits"
    );

    // The stream decodes back to the same five by three pixels.
    let back = Pixmap::from_png(&png).expect("our own PNG decodes");
    assert_eq!((back.width, back.height), (5, 3));
    for y in 0..3 {
        for x in 0..5 {
            assert_eq!(back.pixel(x, y), [1.0, 0.0, 0.0, 1.0], "pixel ({x}, {y})");
        }
    }
}

#[test]
fn png_of_a_single_pixel_round_trips_its_bytes() {
    let mut p = Pixmap::new(1, 1);
    p.data[0] = [0.0, 1.0, 0.0, 1.0];
    let back = Pixmap::from_png(&p.to_png()).expect("decodes");
    assert_eq!(back.pixel(0, 0), [0.0, 1.0, 0.0, 1.0]);
}

#[test]
fn repetitive_images_compress_well() {
    // A flat image has long runs, so the PNG should be far smaller than its raw pixels.
    let mut p = Pixmap::new(256, 256);
    for px in p.data.iter_mut() {
        *px = [0.2, 0.4, 0.6, 1.0];
    }
    let png = p.to_png();
    let raw = 256 * 256 * 4;
    assert!(
        png.len() < raw / 20,
        "PNG is {} bytes for {raw} raw",
        png.len()
    );
    assert!(Pixmap::from_png(&png).is_ok());
}
