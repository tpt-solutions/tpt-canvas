//! PNG output: header, chunk layout, and a decodable zlib payload.

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
fn png_has_ihdr_idat_iend_and_correct_size() {
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

    // zlib stream: 2-byte header, one stored block, then adler32. Payload is filter byte + row per line.
    let idat = &parts[1].1;
    assert_eq!(&idat[..2], &[0x78, 0x01]);
    let len = u16::from_le_bytes([idat[3], idat[4]]) as usize;
    assert_eq!(len, 3 * (1 + 5 * 4));
    assert_eq!(idat.len(), 2 + 5 + len + 4);
}

#[test]
fn png_of_a_drawn_pixel_carries_the_pixel_bytes() {
    let mut p = Pixmap::new(1, 1);
    p.data[0] = [1.0, 0.0, 0.0, 1.0];
    let png = p.to_png();
    let idat = &chunks(&png)[1].1;
    // Raw scanline starts after the 2-byte zlib header and 5-byte block header.
    assert_eq!(&idat[7..12], &[0, 255, 0, 0, 255]);
}
