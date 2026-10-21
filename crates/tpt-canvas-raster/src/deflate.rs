//! zlib compression for PNG and PDF streams.
//!
//! A greedy LZ77 pass with a hash chain finds repeated bytes. The result is written as a single
//! block with fixed Huffman codes (RFC 1951, section 3.2.6). Fixed codes are less compact than
//! dynamic ones, but they need no code-length tables, and they still shrink the flat, repetitive
//! pixel data that images produce.

/// How far back a match may reach. Deflate's window is 32 KiB.
const WINDOW: usize = 32_768;
/// Longest match deflate can encode.
const MAX_MATCH: usize = 258;
/// Shortest match worth encoding.
const MIN_MATCH: usize = 3;
/// Hash chain entries examined per position. More finds longer matches, but costs time.
const MAX_CHAIN: usize = 64;
const HASH_BITS: u32 = 15;

/// Length codes 257..=285: the base length and the number of extra bits for each.
const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
/// Distance codes 0..=29: the base distance and the number of extra bits for each.
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// Compresses `data` into a zlib stream (RFC 1950) that any inflate decoder reads.
pub fn zlib_compress(data: &[u8]) -> Vec<u8> {
    let mut bits = BitWriter::default();
    bits.bits(1, 1); // BFINAL: this is the only block.
    bits.bits(1, 2); // BTYPE 01: fixed Huffman codes.
    lz77_tokens(data, &mut bits);
    bits.fixed_code(256); // End of block.

    let mut out = vec![0x78, 0x9C];
    out.extend_from_slice(&bits.finish());
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// Writes the literals and matches of a greedy LZ77 parse of `data`.
fn lz77_tokens(data: &[u8], bits: &mut BitWriter) {
    let n = data.len();
    let mut head = vec![usize::MAX; 1 << HASH_BITS];
    // prev[i] is the previous position with the same hash as position i.
    let mut prev = vec![usize::MAX; n];

    let mut i = 0;
    while i < n {
        let (mut best_len, mut best_dist) = (0, 0);
        if i + MIN_MATCH <= n {
            let h = hash(data, i);
            let mut cand = head[h];
            let mut steps = 0;
            while cand != usize::MAX && i - cand <= WINDOW && steps < MAX_CHAIN {
                let limit = MAX_MATCH.min(n - i);
                let mut len = 0;
                while len < limit && data[cand + len] == data[i + len] {
                    len += 1;
                }
                if len > best_len {
                    best_len = len;
                    best_dist = i - cand;
                    if len == limit {
                        break;
                    }
                }
                cand = prev[cand];
                steps += 1;
            }
        }

        if best_len >= MIN_MATCH {
            length_code(bits, best_len);
            distance_code(bits, best_dist);
            // Every position inside the match goes into the hash chains too.
            for p in i..i + best_len {
                insert(data, &mut head, &mut prev, p);
            }
            i += best_len;
        } else {
            bits.fixed_code(data[i] as u16);
            insert(data, &mut head, &mut prev, i);
            i += 1;
        }
    }
}

fn insert(data: &[u8], head: &mut [usize], prev: &mut [usize], p: usize) {
    if p + MIN_MATCH <= data.len() {
        let h = hash(data, p);
        prev[p] = head[h];
        head[h] = p;
    }
}

fn hash(data: &[u8], p: usize) -> usize {
    let v = ((data[p] as u32) << 16) | ((data[p + 1] as u32) << 8) | data[p + 2] as u32;
    (v.wrapping_mul(2_654_435_761) >> (32 - HASH_BITS)) as usize
}

/// Writes the length symbol and its extra bits for a match of `len` bytes (3 to 258).
fn length_code(bits: &mut BitWriter, len: usize) {
    let index = (0..LENGTH_BASE.len())
        .rev()
        .find(|&k| len >= LENGTH_BASE[k] as usize)
        .expect("lengths start at 3");
    bits.fixed_code(257 + index as u16);
    bits.bits(
        (len - LENGTH_BASE[index] as usize) as u32,
        LENGTH_EXTRA[index] as u32,
    );
}

/// Writes the distance symbol and its extra bits for a match `dist` bytes back (1 to 32768).
fn distance_code(bits: &mut BitWriter, dist: usize) {
    let index = (0..DIST_BASE.len())
        .rev()
        .find(|&k| dist >= DIST_BASE[k] as usize)
        .expect("distances start at 1");
    // Distance codes are five bits, written most significant bit first.
    bits.huffman(index as u32, 5);
    bits.bits(
        (dist - DIST_BASE[index] as usize) as u32,
        DIST_EXTRA[index] as u32,
    );
}

/// Packs values least significant bit first, as deflate does.
#[derive(Default)]
struct BitWriter {
    out: Vec<u8>,
    acc: u32,
    count: u32,
}

impl BitWriter {
    /// Writes the low `n` bits of `value`, least significant first.
    fn bits(&mut self, value: u32, n: u32) {
        self.acc |= value << self.count;
        self.count += n;
        while self.count >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.count -= 8;
        }
    }

    /// Writes a Huffman code of `len` bits. Codes go most significant bit first, so the bits
    /// are reversed before they are packed.
    fn huffman(&mut self, code: u32, len: u32) {
        let mut reversed = 0;
        for k in 0..len {
            reversed |= ((code >> k) & 1) << (len - 1 - k);
        }
        self.bits(reversed, len);
    }

    /// Writes a literal or length symbol with the fixed literal/length code of RFC 1951.
    fn fixed_code(&mut self, sym: u16) {
        match sym {
            0..=143 => self.huffman(0x30 + sym as u32, 8),
            144..=255 => self.huffman(0x190 + (sym as u32 - 144), 9),
            256..=279 => self.huffman(sym as u32 - 256, 7),
            _ => self.huffman(0xC0 + (sym as u32 - 280), 8),
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.count > 0 {
            self.out.push(self.acc as u8);
        }
        self.out
    }
}

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + u32::from(byte)) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::zlib_decompress;

    fn round_trip(data: &[u8]) -> usize {
        let packed = zlib_compress(data);
        let back = zlib_decompress(&packed, data.len()).expect("decodes");
        assert_eq!(back, data, "round trip changed the data");
        packed.len()
    }

    #[test]
    fn empty_and_tiny_inputs_round_trip() {
        round_trip(&[]);
        round_trip(&[7]);
        round_trip(b"ab");
        round_trip(b"abc");
    }

    #[test]
    fn repeated_data_shrinks_and_round_trips() {
        let data = vec![42u8; 10_000];
        let size = round_trip(&data);
        assert!(
            size < 200,
            "a run of one byte should compress hard, got {size}"
        );
    }

    #[test]
    fn long_matches_and_far_distances_round_trip() {
        // A 300-byte pattern repeated with noise: matches longer than 258, and distances past
        // 256, so the extra-bit tables are exercised.
        let mut seed = 12_345u32;
        let pattern: Vec<u8> = (0..300)
            .map(|_| {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                (seed >> 16) as u8
            })
            .collect();
        let mut data = Vec::new();
        for _ in 0..40 {
            data.extend_from_slice(&pattern);
            data.push(data.len() as u8);
        }
        round_trip(&data);
    }

    #[test]
    fn distances_near_the_window_limit_round_trip() {
        let mut seed = 99u32;
        let chunk: Vec<u8> = (0..1_000)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 24) as u8
            })
            .collect();
        // Copy the chunk back about 32 KiB later, so some matches sit at the window's edge.
        let mut data = chunk.clone();
        data.extend(std::iter::repeat(0u8).take(31_500));
        data.extend_from_slice(&chunk);
        round_trip(&data);
    }

    #[test]
    fn adler32_matches_the_reference_value() {
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn incompressible_data_round_trips_with_bounded_growth() {
        let mut seed = 7u32;
        let data: Vec<u8> = (0..20_000)
            .map(|_| {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                (seed >> 16) as u8
            })
            .collect();
        let size = round_trip(&data);
        // Fixed codes cost at most about 9 bits per literal, so growth stays bounded.
        assert!(size < data.len() * 9 / 8 + 16, "grew too much: {size}");
    }
}
