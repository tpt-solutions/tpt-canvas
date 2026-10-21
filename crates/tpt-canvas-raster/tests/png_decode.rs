//! PNG decoding against fixtures written by a real encoder. The pixel formulas match
//! `tests/fixtures/make_png_fixtures.py`, and were also checked with PyMuPDF.

use tpt_canvas_raster::{DecodeError, Pixmap};

fn load(bytes: &[u8]) -> Pixmap {
    Pixmap::from_png(bytes).expect("fixture decodes")
}

/// The premultiplied value of a straight colour with an 8-bit alpha, as the pixmap stores it.
fn premul(rgb: [u8; 3], a: u8) -> [f32; 4] {
    let a = a as f32 / 255.0;
    [
        rgb[0] as f32 / 255.0 * a,
        rgb[1] as f32 / 255.0 * a,
        rgb[2] as f32 / 255.0 * a,
        a,
    ]
}

fn assert_pixel(p: [f32; 4], want: [f32; 4], at: (u32, u32)) {
    for i in 0..4 {
        assert!(
            (p[i] - want[i]).abs() < 1e-4,
            "pixel {at:?} channel {i}: got {p:?}, want {want:?}"
        );
    }
}

#[test]
fn rgba8_with_all_five_filters() {
    let pm = load(include_bytes!("fixtures/rgba8_filters.png"));
    assert_eq!((pm.width, pm.height), (16, 16));
    for y in 0..16u32 {
        for x in 0..16u32 {
            let rgb = [(17 * x) as u8, (17 * y) as u8, (x * y) as u8];
            let a = (255 - 8 * x) as u8;
            assert_pixel(pm.pixel(x, y), premul(rgb, a), (x, y));
        }
    }
}

#[test]
fn rgb8_stored_blocks() {
    let pm = load(include_bytes!("fixtures/rgb8_stored.png"));
    for y in 0..8u32 {
        for x in 0..8u32 {
            let rgb = [(x * 10) as u8, (y * 10) as u8, 200];
            assert_pixel(pm.pixel(x, y), premul(rgb, 255), (x, y));
        }
    }
}

#[test]
fn gray_1_2_and_16_bit() {
    let g1 = load(include_bytes!("fixtures/gray1.png"));
    for y in 0..4u32 {
        for x in 0..16u32 {
            let v = if (x / 4 + y) % 2 == 1 { 255 } else { 0 };
            assert_pixel(g1.pixel(x, y), premul([v; 3], 255), (x, y));
        }
    }

    let g2 = load(include_bytes!("fixtures/gray2.png"));
    for y in 0..4u32 {
        for x in 0..16u32 {
            let v = (((x + y) % 4) * 85) as u8;
            assert_pixel(g2.pixel(x, y), premul([v; 3], 255), (x, y));
        }
    }

    // Sixteen-bit gray keeps the high byte.
    let g16 = load(include_bytes!("fixtures/gray16.png"));
    for y in 0..4u32 {
        for x in 0..16u32 {
            let v = ((16 * x + y) & 0xFF) as u8;
            assert_pixel(g16.pixel(x, y), premul([v; 3], 255), (x, y));
        }
    }
}

#[test]
fn palette_with_transparency() {
    let pm = load(include_bytes!("fixtures/palette4.png"));
    let palette = [
        [255, 0, 0],
        [0, 255, 0],
        [0, 0, 255],
        [255, 255, 0],
        [0, 255, 255],
    ];
    let alpha = [0u8, 128];
    for y in 0..16u32 {
        for x in 0..16u32 {
            let i = ((x + y) % 5) as usize;
            let a = alpha.get(i).copied().unwrap_or(255);
            assert_pixel(pm.pixel(x, y), premul(palette[i], a), (x, y));
        }
    }
}

#[test]
fn gray_with_alpha_and_sixteen_bit_rgb() {
    let ga = load(include_bytes!("fixtures/graya8.png"));
    for y in 0..4u32 {
        for x in 0..8u32 {
            let g = (20 * x + 5 * y) as u8;
            let a = (16 * y + 15) as u8;
            assert_pixel(ga.pixel(x, y), premul([g; 3], a), (x, y));
        }
    }

    let rgb16 = load(include_bytes!("fixtures/rgb16.png"));
    for y in 0..4u32 {
        for x in 0..8u32 {
            let rgb = [(16 * x) as u8, (16 * y) as u8, 128];
            assert_pixel(rgb16.pixel(x, y), premul(rgb, 255), (x, y));
        }
    }
}

#[test]
fn fixed_huffman_block_decodes() {
    // This fixture's first deflate block uses fixed codes (block type 1).
    let pm = load(include_bytes!("fixtures/tiny_rgb.png"));
    assert_pixel(pm.pixel(0, 0), premul([255, 0, 0], 255), (0, 0));
    assert_pixel(pm.pixel(1, 0), premul([0, 255, 0], 255), (1, 0));
    assert_pixel(pm.pixel(0, 1), premul([0, 0, 255], 255), (0, 1));
    assert_pixel(pm.pixel(1, 1), premul([255, 255, 0], 255), (1, 1));
}

#[test]
fn rejects_a_file_that_is_not_png() {
    assert_eq!(
        Pixmap::from_png(b"GIF89a....").unwrap_err(),
        DecodeError::BadSignature
    );
}

#[test]
fn rejects_truncated_files() {
    let bytes = include_bytes!("fixtures/rgb8_stored.png");
    let err = Pixmap::from_png(&bytes[..bytes.len() / 2]).unwrap_err();
    assert_eq!(err, DecodeError::Truncated);
}

#[test]
fn a_corrupted_chunk_fails_its_crc() {
    let mut bytes = include_bytes!("fixtures/rgb8_stored.png").to_vec();
    // Flip a byte inside the IDAT data, which comes after the 33-byte IHDR chunk.
    let idat = bytes.windows(4).position(|w| w == b"IDAT").unwrap();
    bytes[idat + 6] ^= 0xFF;
    assert_eq!(
        Pixmap::from_png(&bytes).unwrap_err(),
        DecodeError::BadChunk("CRC mismatch")
    );
}

/// CRC-32 as PNG uses it, for rebuilding a chunk after editing it in a test.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[test]
fn interlaced_images_are_reported_as_unsupported() {
    let mut bytes = include_bytes!("fixtures/rgb8_stored.png").to_vec();
    // IHDR starts at offset 8: length (4), type (4), then 13 data bytes. Interlace is the last
    // data byte. Set it to 1 and recompute the chunk's CRC, so only the interlace rule fails.
    let data_start = 16;
    bytes[data_start + 12] = 1;
    let crc = crc32(&bytes[12..data_start + 13]);
    bytes[data_start + 13..data_start + 17].copy_from_slice(&crc.to_be_bytes());
    assert_eq!(
        Pixmap::from_png(&bytes).unwrap_err(),
        DecodeError::Unsupported("interlaced (Adam7) images")
    );
}

#[test]
fn libpng_written_rgba_matches_an_independent_decode() {
    // The PNG was written by libpng (through PyMuPDF). The expected bytes were computed with
    // Python's zlib and a separate unfilter, see fixtures/make_libpng_fixture.py.
    let pm = load(include_bytes!("fixtures/libpng_rgba.png"));
    let expected = include_bytes!("fixtures/libpng_rgba.expected");
    assert_eq!(expected.len(), (pm.width * pm.height * 4) as usize);
    for (i, p) in pm.data.iter().enumerate() {
        let a = p[3];
        for c in 0..3 {
            // Un-premultiply, then compare with the straight 8-bit value.
            let straight = if a > 0.0 { p[c] / a } else { 0.0 };
            let got = (straight * 255.0 + 0.5) as i32;
            let want = expected[i * 4 + c] as i32;
            assert!(
                (got - want).abs() <= 1,
                "pixel {i} channel {c}: got {got}, want {want}"
            );
        }
        assert_eq!((a * 255.0 + 0.5) as u8, expected[i * 4 + 3]);
    }
}

#[test]
fn decoded_png_draws_through_draw_image() {
    use tpt_canvas::prelude::*;
    use tpt_canvas::{ImageId, ImageRef, Rect};
    use tpt_canvas_raster::RasterBackend;

    let image = load(include_bytes!("fixtures/tiny_rgb.png"));
    let mut backend = RasterBackend::new(4, 4);
    backend.add_image(ImageId(1), image);
    let mut ctx = Context::new(backend);
    // A 2 by 2 image stretched to 4 by 4: each source pixel covers a 2 by 2 block.
    ctx.draw_image(
        &ImageRef {
            id: ImageId(1),
            width: 2,
            height: 2,
        },
        Rect {
            x: 0.0,
            y: 0.0,
            w: 2.0,
            h: 2.0,
        },
        Rect {
            x: 0.0,
            y: 0.0,
            w: 4.0,
            h: 4.0,
        },
    );
    let pm = ctx.backend().pixmap();
    assert_pixel(pm.pixel(0, 0), premul([255, 0, 0], 255), (0, 0));
    assert_pixel(pm.pixel(3, 0), premul([0, 255, 0], 255), (3, 0));
    assert_pixel(pm.pixel(0, 3), premul([0, 0, 255], 255), (0, 3));
    assert_pixel(pm.pixel(3, 3), premul([255, 255, 0], 255), (3, 3));
}
