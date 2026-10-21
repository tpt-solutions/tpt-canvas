//! A PNG decoder, with its own inflate.
//!
//! Supported: all colour types and bit depths in the PNG spec, plain or interlaced (Adam7),
//! with the five row filters, and palette transparency (`tRNS`). 16-bit samples keep their
//! high byte, so the result is 8-bit.
//! CRCs are checked on every chunk.

use core::fmt;

use crate::Pixmap;

/// Why a PNG could not be decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The data ended before the image was complete.
    Truncated,
    /// The file does not start with the PNG signature.
    BadSignature,
    /// A chunk failed its CRC check, or was malformed.
    BadChunk(&'static str),
    /// The compressed data is invalid.
    BadData(&'static str),
    /// The file is valid PNG but uses a feature this decoder does not support.
    Unsupported(&'static str),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::Truncated => write!(f, "PNG data ended early"),
            DecodeError::BadSignature => write!(f, "not a PNG file"),
            DecodeError::BadChunk(what) => write!(f, "bad PNG chunk: {what}"),
            DecodeError::BadData(what) => write!(f, "bad compressed data: {what}"),
            DecodeError::Unsupported(what) => write!(f, "unsupported PNG: {what}"),
        }
    }
}

/// Decodes a PNG file into a premultiplied pixmap.
pub(crate) fn decode_png(bytes: &[u8]) -> Result<Pixmap, DecodeError> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.len() < 8 || bytes[..8] != SIGNATURE {
        return Err(DecodeError::BadSignature);
    }

    let mut header: Option<Header> = None;
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut palette_alpha: Vec<u8> = Vec::new();
    let mut idat: Vec<u8> = Vec::new();
    let mut pos = 8;
    loop {
        if pos + 8 > bytes.len() {
            return Err(DecodeError::Truncated);
        }
        let len = u32::from_be_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]])
            as usize;
        let kind = &bytes[pos + 4..pos + 8];
        let data_start = pos + 8;
        let data_end = data_start.checked_add(len).ok_or(DecodeError::Truncated)?;
        if data_end + 4 > bytes.len() {
            return Err(DecodeError::Truncated);
        }
        let data = &bytes[data_start..data_end];
        let crc = u32::from_be_bytes([
            bytes[data_end],
            bytes[data_end + 1],
            bytes[data_end + 2],
            bytes[data_end + 3],
        ]);
        if crc32(&bytes[pos + 4..data_end]) != crc {
            return Err(DecodeError::BadChunk("CRC mismatch"));
        }
        match kind {
            b"IHDR" => header = Some(Header::parse(data)?),
            b"PLTE" => {
                if data.len() % 3 != 0 || data.is_empty() {
                    return Err(DecodeError::BadChunk("PLTE length"));
                }
                palette = data.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
            }
            b"tRNS" => palette_alpha = data.to_vec(),
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {}
        }
        pos = data_end + 4;
    }

    let header = header.ok_or(DecodeError::BadChunk("missing IHDR"))?;
    if header.color_type == 3 && palette.is_empty() {
        return Err(DecodeError::BadChunk("palette image without PLTE"));
    }

    // A plain image is one pass over every pixel. An interlaced image is seven passes, each a
    // smaller image with its own filter bytes, and the passes are stored one after another.
    let (w, h) = (header.width as usize, header.height as usize);
    let passes: &[Pass] = if header.interlaced {
        &ADAM7
    } else {
        &[Pass {
            x0: 0,
            y0: 0,
            dx: 1,
            dy: 1,
        }]
    };
    let sizes: Vec<(usize, usize, usize)> = passes
        .iter()
        .map(|p| {
            let pw = pass_size(w, p.x0, p.dx);
            let ph = pass_size(h, p.y0, p.dy);
            let stride = (pw * header.channels * header.depth as usize).div_ceil(8);
            (pw, ph, stride)
        })
        .collect();
    let expected: usize = sizes
        .iter()
        .filter(|(pw, ph, _)| *pw > 0 && *ph > 0)
        .map(|(_, ph, stride)| ph * (stride + 1))
        .sum();
    let raw = zlib_decompress(&idat, expected)?;
    if raw.len() != expected {
        return Err(DecodeError::BadData("image data has the wrong length"));
    }

    let mut pixmap = Pixmap::new(header.width, header.height);
    let mut offset = 0;
    for (pass, &(pw, ph, stride)) in passes.iter().zip(&sizes) {
        if pw == 0 || ph == 0 {
            continue;
        }
        let len = ph * (stride + 1);
        let rows = unfilter(
            &raw[offset..offset + len],
            stride,
            ph,
            header.bytes_per_pixel(),
        )?;
        offset += len;
        for j in 0..ph {
            let row = &rows[j * stride..(j + 1) * stride];
            for i in 0..pw {
                let rgba = header.pixel(row, i, &palette, &palette_alpha)?;
                let a = rgba[3] as f32 / 255.0;
                let x = pass.x0 + i * pass.dx;
                let y = pass.y0 + j * pass.dy;
                pixmap.data[y * w + x] = [
                    rgba[0] as f32 / 255.0 * a,
                    rgba[1] as f32 / 255.0 * a,
                    rgba[2] as f32 / 255.0 * a,
                    a,
                ];
            }
        }
    }
    Ok(pixmap)
}

/// One pass of an interlaced image: where its first pixel sits, and the step between pixels.
struct Pass {
    x0: usize,
    y0: usize,
    dx: usize,
    dy: usize,
}

/// The seven Adam7 passes, in the order they are stored.
const ADAM7: [Pass; 7] = [
    Pass {
        x0: 0,
        y0: 0,
        dx: 8,
        dy: 8,
    },
    Pass {
        x0: 4,
        y0: 0,
        dx: 8,
        dy: 8,
    },
    Pass {
        x0: 0,
        y0: 4,
        dx: 4,
        dy: 8,
    },
    Pass {
        x0: 2,
        y0: 0,
        dx: 4,
        dy: 4,
    },
    Pass {
        x0: 0,
        y0: 2,
        dx: 2,
        dy: 4,
    },
    Pass {
        x0: 1,
        y0: 0,
        dx: 2,
        dy: 2,
    },
    Pass {
        x0: 0,
        y0: 1,
        dx: 1,
        dy: 2,
    },
];

/// How many pixels a pass covers along one axis: the pixels from `start` every `step`.
fn pass_size(size: usize, start: usize, step: usize) -> usize {
    if size <= start {
        0
    } else {
        (size - start).div_ceil(step)
    }
}

/// The fields of the IHDR chunk that decoding needs.
struct Header {
    width: u32,
    height: u32,
    depth: u8,
    color_type: u8,
    channels: usize,
    interlaced: bool,
}

impl Header {
    fn parse(data: &[u8]) -> Result<Header, DecodeError> {
        if data.len() != 13 {
            return Err(DecodeError::BadChunk("IHDR length"));
        }
        let width = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let height = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let depth = data[8];
        let color_type = data[9];
        if data[10] != 0 || data[11] != 0 {
            return Err(DecodeError::BadChunk(
                "unknown compression or filter method",
            ));
        }
        let interlaced = match data[12] {
            0 => false,
            1 => true,
            _ => return Err(DecodeError::BadChunk("interlace method")),
        };
        if width == 0 || height == 0 {
            return Err(DecodeError::BadChunk("zero size"));
        }
        let (channels, depths): (usize, &[u8]) = match color_type {
            0 => (1, &[1, 2, 4, 8, 16]),
            2 => (3, &[8, 16]),
            3 => (1, &[1, 2, 4, 8]),
            4 => (2, &[8, 16]),
            6 => (4, &[8, 16]),
            _ => return Err(DecodeError::BadChunk("colour type")),
        };
        if !depths.contains(&depth) {
            return Err(DecodeError::BadChunk("bit depth for colour type"));
        }
        Ok(Header {
            width,
            height,
            depth,
            color_type,
            channels,
            interlaced,
        })
    }

    /// Bytes per complete pixel, used by the row filters. At least one.
    fn bytes_per_pixel(&self) -> usize {
        (self.channels * self.depth as usize).div_ceil(8).max(1)
    }

    /// The RGBA of pixel `x` in an unfiltered row. Sixteen-bit samples keep their high byte.
    fn pixel(
        &self,
        row: &[u8],
        x: usize,
        palette: &[[u8; 3]],
        palette_alpha: &[u8],
    ) -> Result<[u8; 4], DecodeError> {
        let depth = self.depth as usize;
        let sample = |channel: usize| -> u32 {
            let index = x * self.channels + channel;
            match depth {
                8 => row[index] as u32,
                16 => row[index * 2] as u32,
                _ => {
                    let bit = index * depth;
                    let shift = 8 - depth - bit % 8;
                    ((row[bit / 8] >> shift) as u32) & ((1 << depth) - 1)
                }
            }
        };
        let scale_gray = |v: u32| -> u8 {
            // Scale a sub-byte gray level to the full 0 to 255 range.
            (v * 255 / ((1 << depth) - 1)) as u8
        };
        Ok(match self.color_type {
            0 => {
                let g = if depth < 8 {
                    scale_gray(sample(0))
                } else {
                    sample(0) as u8
                };
                [g, g, g, 255]
            }
            2 => [sample(0) as u8, sample(1) as u8, sample(2) as u8, 255],
            3 => {
                let index = sample(0) as usize;
                let [r, g, b] = *palette
                    .get(index)
                    .ok_or(DecodeError::BadData("palette index out of range"))?;
                let a = palette_alpha.get(index).copied().unwrap_or(255);
                [r, g, b, a]
            }
            4 => {
                let g = sample(0) as u8;
                [g, g, g, sample(1) as u8]
            }
            _ => [
                sample(0) as u8,
                sample(1) as u8,
                sample(2) as u8,
                sample(3) as u8,
            ],
        })
    }
}

/// Removes the row filters. `raw` holds one filter byte before each row.
fn unfilter(raw: &[u8], stride: usize, height: usize, bpp: usize) -> Result<Vec<u8>, DecodeError> {
    let mut out = vec![0u8; stride * height];
    for y in 0..height {
        let filter = raw[y * (stride + 1)];
        let src = &raw[y * (stride + 1) + 1..(y + 1) * (stride + 1)];
        let (before, rest) = out.split_at_mut(y * stride);
        let prev: &[u8] = if y == 0 {
            &[]
        } else {
            &before[(y - 1) * stride..]
        };
        let cur = &mut rest[..stride];
        for i in 0..stride {
            let a = (if i >= bpp { cur[i - bpp] } else { 0 }) as i32;
            let b = (if y > 0 { prev[i] } else { 0 }) as i32;
            let c = (if y > 0 && i >= bpp { prev[i - bpp] } else { 0 }) as i32;
            let predictor = match filter {
                0 => 0,
                1 => a,
                2 => b,
                3 => (a + b) / 2,
                4 => paeth(a, b, c),
                _ => return Err(DecodeError::BadData("unknown row filter")),
            };
            cur[i] = (src[i] as i32 + predictor) as u8;
        }
    }
    Ok(out)
}

/// The Paeth predictor from the PNG spec.
fn paeth(a: i32, b: i32, c: i32) -> i32 {
    let p = a + b - c;
    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// CRC-32 as used by PNG chunks.
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

/// Decompresses a zlib stream (RFC 1950), refusing more than `limit` bytes of output.
pub(crate) fn zlib_decompress(data: &[u8], limit: usize) -> Result<Vec<u8>, DecodeError> {
    if data.len() < 2 {
        return Err(DecodeError::Truncated);
    }
    let (cmf, flg) = (data[0], data[1]);
    if cmf & 0x0F != 8 || (cmf as u16 * 256 + flg as u16) % 31 != 0 {
        return Err(DecodeError::BadData("zlib header"));
    }
    if flg & 0x20 != 0 {
        return Err(DecodeError::Unsupported("zlib preset dictionary"));
    }
    inflate(&data[2..], limit)
}

/// The inflate algorithm (RFC 1951), written after zlib's reference decoder.
fn inflate(data: &[u8], limit: usize) -> Result<Vec<u8>, DecodeError> {
    let mut bits = Bits {
        data,
        pos: 0,
        buf: 0,
        count: 0,
    };
    let mut out = Vec::new();
    loop {
        let last = bits.read(1)? == 1;
        match bits.read(2)? {
            0 => stored(&mut bits, &mut out, limit)?,
            1 => {
                let (lit, dist) = fixed_tables();
                codes(&mut bits, &mut out, &lit, &dist, limit)?;
            }
            2 => {
                let (lit, dist) = dynamic_tables(&mut bits)?;
                codes(&mut bits, &mut out, &lit, &dist, limit)?;
            }
            _ => return Err(DecodeError::BadData("reserved block type")),
        }
        if last {
            break;
        }
    }
    Ok(out)
}

/// Reads the bits of the stream, least significant bit first.
struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    buf: u32,
    count: u32,
}

impl Bits<'_> {
    fn read(&mut self, n: u32) -> Result<u32, DecodeError> {
        while self.count < n {
            let byte = *self.data.get(self.pos).ok_or(DecodeError::Truncated)?;
            self.pos += 1;
            self.buf |= (byte as u32) << self.count;
            self.count += 8;
        }
        let value = self.buf & ((1u32 << n) - 1);
        self.buf >>= n;
        self.count -= n;
        Ok(value)
    }

    /// Discards bits up to the next byte boundary.
    fn align(&mut self) {
        self.buf = 0;
        self.count = 0;
    }
}

/// A canonical Huffman code: how many codes have each length, and the symbols in order.
struct Huffman {
    count: [u16; 16],
    symbol: Vec<u16>,
}

impl Huffman {
    fn new(lengths: &[u8]) -> Result<Huffman, DecodeError> {
        let mut count = [0u16; 16];
        for &len in lengths {
            count[len as usize] += 1;
        }
        // Reject over-subscribed codes. Incomplete codes are allowed, as zlib allows them.
        let mut left: i32 = 1;
        for &n in &count[1..16] {
            left <<= 1;
            left -= n as i32;
            if left < 0 {
                return Err(DecodeError::BadData("over-subscribed Huffman code"));
            }
        }
        let mut offsets = [0u16; 16];
        for len in 1..15 {
            offsets[len + 1] = offsets[len] + count[len];
        }
        let mut symbol = vec![0u16; lengths.len()];
        for (sym, &len) in lengths.iter().enumerate() {
            if len != 0 {
                symbol[offsets[len as usize] as usize] = sym as u16;
                offsets[len as usize] += 1;
            }
        }
        Ok(Huffman { count, symbol })
    }

    fn decode(&self, bits: &mut Bits) -> Result<u16, DecodeError> {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: i32 = 0;
        for len in 1..16 {
            code |= bits.read(1)? as i32;
            let count = self.count[len] as i32;
            if code - count < first {
                return Ok(self.symbol[(index + (code - first)) as usize]);
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err(DecodeError::BadData("invalid Huffman code"))
    }
}

const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

fn fixed_tables() -> (Huffman, Huffman) {
    let mut lengths = [0u8; 288];
    lengths[..144].fill(8);
    lengths[144..256].fill(9);
    lengths[256..280].fill(7);
    lengths[280..].fill(8);
    let lit = Huffman::new(&lengths).expect("fixed literal code is valid");
    let dist = Huffman::new(&[5u8; 30]).expect("fixed distance code is valid");
    (lit, dist)
}

fn dynamic_tables(bits: &mut Bits) -> Result<(Huffman, Huffman), DecodeError> {
    const ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let nlen = bits.read(5)? as usize + 257;
    let ndist = bits.read(5)? as usize + 1;
    let ncode = bits.read(4)? as usize + 4;
    if nlen > 286 || ndist > 30 {
        return Err(DecodeError::BadData("too many length or distance codes"));
    }
    let mut code_lengths = [0u8; 19];
    for &index in ORDER.iter().take(ncode) {
        code_lengths[index] = bits.read(3)? as u8;
    }
    let code_code = Huffman::new(&code_lengths)?;

    let mut lengths = vec![0u8; nlen + ndist];
    let mut i = 0;
    while i < nlen + ndist {
        let sym = code_code.decode(bits)?;
        let (value, repeat) = match sym {
            0..=15 => (sym as u8, 1),
            16 => {
                if i == 0 {
                    return Err(DecodeError::BadData("repeat with no previous length"));
                }
                (lengths[i - 1], 3 + bits.read(2)? as usize)
            }
            17 => (0, 3 + bits.read(3)? as usize),
            _ => (0, 11 + bits.read(7)? as usize),
        };
        if i + repeat > nlen + ndist {
            return Err(DecodeError::BadData("code lengths overflow"));
        }
        lengths[i..i + repeat].fill(value);
        i += repeat;
    }
    if lengths[256] == 0 {
        return Err(DecodeError::BadData("no end-of-block code"));
    }
    let lit = Huffman::new(&lengths[..nlen])?;
    let dist = Huffman::new(&lengths[nlen..])?;
    Ok((lit, dist))
}

fn stored(bits: &mut Bits, out: &mut Vec<u8>, limit: usize) -> Result<(), DecodeError> {
    bits.align();
    let header = bits
        .data
        .get(bits.pos..bits.pos + 4)
        .ok_or(DecodeError::Truncated)?;
    let len = u16::from_le_bytes([header[0], header[1]]) as usize;
    let nlen = u16::from_le_bytes([header[2], header[3]]) as usize;
    if len != !nlen & 0xFFFF {
        return Err(DecodeError::BadData("stored block length check"));
    }
    bits.pos += 4;
    let body = bits
        .data
        .get(bits.pos..bits.pos + len)
        .ok_or(DecodeError::Truncated)?;
    if out.len() + len > limit {
        return Err(DecodeError::BadData("output larger than the image"));
    }
    out.extend_from_slice(body);
    bits.pos += len;
    Ok(())
}

fn codes(
    bits: &mut Bits,
    out: &mut Vec<u8>,
    lit: &Huffman,
    dist: &Huffman,
    limit: usize,
) -> Result<(), DecodeError> {
    loop {
        let sym = lit.decode(bits)? as usize;
        if sym < 256 {
            if out.len() >= limit {
                return Err(DecodeError::BadData("output larger than the image"));
            }
            out.push(sym as u8);
        } else if sym == 256 {
            return Ok(());
        } else {
            let index = sym - 257;
            if index >= 29 {
                return Err(DecodeError::BadData("length code out of range"));
            }
            let len = LENGTH_BASE[index] as usize + bits.read(LENGTH_EXTRA[index] as u32)? as usize;
            let dsym = dist.decode(bits)? as usize;
            if dsym >= 30 {
                return Err(DecodeError::BadData("distance code out of range"));
            }
            let d = DIST_BASE[dsym] as usize + bits.read(DIST_EXTRA[dsym] as u32)? as usize;
            if d > out.len() {
                return Err(DecodeError::BadData(
                    "distance before the start of the data",
                ));
            }
            if out.len() + len > limit {
                return Err(DecodeError::BadData("output larger than the image"));
            }
            let start = out.len() - d;
            for i in 0..len {
                let byte = out[start + i];
                out.push(byte);
            }
        }
    }
}
