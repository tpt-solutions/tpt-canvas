//! Gaussian blur of a coverage mask, built from three box blurs.
//!
//! Three box passes are a standard approximation of a Gaussian (Kutskir's method). The
//! box widths are chosen so that the variance matches `sigma`. The mask is treated as zero
//! outside its bounds, so a shadow near an edge fades out there.

use tpt_canvas::raster::Coverage;

const PASSES: usize = 3;

/// Returns a copy of `src` blurred with standard deviation `sigma` (in pixels).
///
/// A non-positive or non-finite `sigma` returns the mask unchanged.
pub(crate) fn blur_coverage(src: &Coverage, sigma: f32) -> Coverage {
    let (w, h) = (src.width as usize, src.height as usize);
    if !sigma.is_finite() || sigma <= 0.0 || w == 0 || h == 0 {
        return Coverage {
            width: src.width,
            height: src.height,
            data: src.data.clone(),
        };
    }
    let mut buf: Vec<f32> = src.data.iter().map(|&v| v as f32).collect();
    let mut tmp = vec![0.0f32; w * h];
    for r in box_radii(sigma) {
        box_pass_horizontal(&buf, &mut tmp, w, h, r);
        box_pass_vertical(&tmp, &mut buf, w, h, r);
    }
    Coverage {
        width: src.width,
        height: src.height,
        data: buf
            .iter()
            .map(|v| (v + 0.5).clamp(0.0, 255.0) as u8)
            .collect(),
    }
}

/// Radius of each box pass for a Gaussian with standard deviation `sigma`.
fn box_radii(sigma: f32) -> [usize; PASSES] {
    let n = PASSES as f32;
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
    let mut radii = [0usize; PASSES];
    for (i, r) in radii.iter_mut().enumerate() {
        let width = if i < m { wl } else { wu };
        *r = ((width - 1) / 2).max(0) as usize;
    }
    radii
}

/// Averages each row over a window of `2r + 1` pixels.
fn box_pass_horizontal(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    let norm = 1.0 / (2 * r + 1) as f32;
    for y in 0..h {
        let row = &src[y * w..(y + 1) * w];
        let out = &mut dst[y * w..(y + 1) * w];
        let mut sum: f32 = row[..=r.min(w - 1)].iter().sum();
        for x in 0..w {
            out[x] = sum * norm;
            if x + r + 1 < w {
                sum += row[x + r + 1];
            }
            if x >= r {
                sum -= row[x - r];
            }
        }
    }
}

/// Averages each column over a window of `2r + 1` pixels.
fn box_pass_vertical(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    let norm = 1.0 / (2 * r + 1) as f32;
    for x in 0..w {
        let mut sum: f32 = (0..=r.min(h - 1)).map(|y| src[y * w + x]).sum();
        for y in 0..h {
            dst[y * w + x] = sum * norm;
            if y + r + 1 < h {
                sum += src[(y + r + 1) * w + x];
            }
            if y >= r {
                sum -= src[(y - r) * w + x];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(w: u32, h: u32, x: u32, y: u32) -> Coverage {
        let mut c = Coverage::new(w, h);
        c.data[(y * w + x) as usize] = 255;
        c
    }

    #[test]
    fn zero_sigma_is_identity() {
        let src = point(9, 9, 4, 4);
        assert_eq!(blur_coverage(&src, 0.0).data, src.data);
    }

    #[test]
    fn blur_conserves_mass_away_from_edges() {
        let src = point(41, 41, 20, 20);
        let total_in: u32 = src.data.iter().map(|&v| v as u32).sum();
        let out = blur_coverage(&src, 2.0);
        let total_out: u32 = out.data.iter().map(|&v| v as u32).sum();
        // Rounding loses a little per pixel; the mass must stay close.
        let diff = (total_in as i64 - total_out as i64).abs();
        assert!(
            diff * 100 < total_in as i64 * 5,
            "mass {total_in} -> {total_out}"
        );
    }

    #[test]
    fn blur_spreads_symmetrically() {
        let out = blur_coverage(&point(41, 41, 20, 20), 3.0);
        let at = |x: u32, y: u32| out.data[(y * 41 + x) as usize];
        assert!(at(20, 20) > at(23, 20));
        assert_eq!(at(17, 20), at(23, 20));
        assert_eq!(at(20, 17), at(20, 23));
    }

    #[test]
    fn larger_sigma_spreads_further() {
        let src = point(61, 61, 30, 30);
        let narrow = blur_coverage(&src, 1.0);
        let wide = blur_coverage(&src, 4.0);
        assert!(wide.data[(30 * 61 + 30) as usize] < narrow.data[(30 * 61 + 30) as usize]);
        assert!(wide.data[(30 * 61 + 36) as usize] > narrow.data[(30 * 61 + 36) as usize]);
    }
}
