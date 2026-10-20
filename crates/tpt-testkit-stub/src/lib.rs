//! Stub for `tpt-testkit`: golden-file comparison and deterministic random input
//! generation for fuzz and property tests.
//!
//! Set `TPT_UPDATE_GOLDEN=1` to (re)write golden files instead of comparing.
// Stub crate: replaced by the real ecosystem crate, so its items are not documented individually.
#![allow(missing_docs)]

use std::fs;
use std::path::Path;

/// SplitMix64 generator: small, fast, and identical on every platform.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }

    /// Uniform in `[lo, hi)`.
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    /// Uniform index in `0..n` (`n > 0`).
    pub fn index(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// Runs `f` `iterations` times with a generator seeded by `seed`.
pub fn fuzz<F: FnMut(&mut Rng)>(seed: u64, iterations: usize, mut f: F) {
    let mut rng = Rng::new(seed);
    for _ in 0..iterations {
        f(&mut rng);
    }
}

/// Compares `actual` against the golden file at `path`.
///
/// With `TPT_UPDATE_GOLDEN=1` the file is written instead. Returns an error
/// message describing the first mismatch, or the missing-file condition.
pub fn check_golden(path: &Path, actual: &[u8]) -> Result<(), String> {
    if std::env::var_os("TPT_UPDATE_GOLDEN").is_some() {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        return fs::write(path, actual).map_err(|e| e.to_string());
    }
    let expected = fs::read(path).map_err(|e| {
        format!(
            "golden {} unreadable ({e}); run with TPT_UPDATE_GOLDEN=1",
            path.display()
        )
    })?;
    if expected == actual {
        return Ok(());
    }
    let first = expected
        .iter()
        .zip(actual.iter())
        .position(|(a, b)| a != b)
        .unwrap_or(expected.len().min(actual.len()));
    Err(format!(
        "golden {} differs at byte {first} (expected {} bytes, got {})",
        path.display(),
        expected.len(),
        actual.len()
    ))
}
