//! The box-blur plan for a Gaussian, shared by the raster and WebGPU backends.
//!
//! Three box passes approximate a Gaussian (Kutskir's method). Each backend runs the passes
//! itself. This module only picks their radii, so every backend blurs with the same widths.

/// Number of box passes in a blur.
pub const BOX_PASSES: usize = 3;

/// Radius of each box pass for a Gaussian with standard deviation `sigma`, in pixels.
///
/// A pass of radius `r` averages `2r + 1` pixels. The widths are chosen so that the variance
/// of the passes matches `sigma` squared. Callers must pass a positive, finite `sigma`.
pub fn box_radii(sigma: f32) -> [usize; BOX_PASSES] {
    let n = BOX_PASSES as f32;
    let w_ideal = libm::sqrtf(12.0 * sigma * sigma / n + 1.0);
    let mut wl = libm::floorf(w_ideal) as i64;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wu = wl + 2;
    let wl_f = wl as f32;
    // How many of the passes use the narrower width, chosen so the total variance matches.
    let m_ideal =
        (12.0 * sigma * sigma - n * wl_f * wl_f - 4.0 * n * wl_f - 3.0 * n) / (-4.0 * wl_f - 4.0);
    let m = libm::roundf(m_ideal).max(0.0) as usize;
    let mut radii = [0usize; BOX_PASSES];
    for (i, r) in radii.iter_mut().enumerate() {
        let width = if i < m { wl } else { wu };
        *r = ((width - 1) / 2).max(0) as usize;
    }
    radii
}
