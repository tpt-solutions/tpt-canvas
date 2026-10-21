//! Minimal PNG encoder: 8-bit RGBA, with the image data compressed by [`crate::zlib_compress`].

/// Encodes straight-alpha RGBA8 pixels (`rgba.len() == width * height * 4`) as a PNG file.
pub(crate) fn encode_rgba8(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let row_len = width as usize * 4;

    // Scanlines, each prefixed with filter type 0 (None).
    let mut raw = Vec::with_capacity((row_len + 1) * height as usize);
    for row in rgba.chunks_exact(row_len.max(1)).take(height as usize) {
        raw.push(0);
        raw.extend_from_slice(row);
    }

    let mut out = Vec::new();
    out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    // Bit depth 8, colour type 6 (RGBA), compression 0, filter 0, no interlace.
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    write_chunk(&mut out, b"IHDR", &ihdr);

    write_chunk(&mut out, b"IDAT", &crate::zlib_compress(&raw));
    write_chunk(&mut out, b"IEND", &[]);
    out
}

fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_match_reference_values() {
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    }

    #[test]
    fn empty_image_is_still_a_valid_stream() {
        let png = encode_rgba8(0, 0, &[]);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert!(png.ends_with(&[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]));
    }
}
