//! Digital-filter (group-delay) correction matching the NMRPipe `dmx()` algorithm.
//!
//! Bruker digital receivers and JEOL Delta both introduce a group delay
//! when applying decimation filters to oversampled data.  The correction
//! removes this delay precisely — including the fractional part.
//!
//! The algorithm (reverse-engineered from NMRPipe's `dmx()` function):
//!
//! ```text
//!   1.  Zero-pad R+jI to N = next_power_of_2(in_size)
//!   2.  Inverse FFT  (unnormalized)
//!   3.  fftshift      (swap halves)
//!   4.  Phase multiply: exp(−j·2π·k·grpdly/N) for each bin k
//!   5.  fftshift      (swap halves)
//!   6.  Divide by N   (c_scale normalisation)
//!   7.  Forward FFT   (unnormalized)
//!   8.  Double first point
//!   9.  Zero trailing contaminated points
//!   10. Extract output
//! ```

use rustfft::{num_complex::Complex, num_traits::Zero, FftPlanner};
use std::f32::consts::PI;
use std::sync::Arc;

/// Next power of two ≥ `n`.
fn next_pow2(n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    1 << (usize::BITS - (n - 1).leading_zeros())
}

/// Re-usable digital-filter corrector.
///
/// Holds pre-planned FFT/IFFT and cached parameters so that many
/// vectors of the same length can be corrected without re-planning.
pub struct DFCorrector {
    /// FFT size (next power of 2 ≥ in_size).
    fft_size: usize,
    /// Input vector length (complex points).
    in_size: usize,
    /// Output vector length after correction (complex points).
    out_size: usize,
    /// Group delay in sample periods (fractional allowed).
    grpdly: f32,
    /// Number of additional trailing points to discard beyond ceil(grpdly).
    #[allow(dead_code)]
    skip_tail: usize,
    /// Forward FFT plan.
    fwd: Arc<dyn rustfft::Fft<f32>>,
    /// Inverse FFT plan.
    inv: Arc<dyn rustfft::Fft<f32>>,
}

/// Swap the first and second halves of a slice (in-place fftshift for even N).
fn fftshift(buf: &mut [Complex<f32>]) {
    let n = buf.len();
    let half = n / 2;
    for i in 0..half {
        buf.swap(i, i + half);
    }
}

impl DFCorrector {
    /// Create a new corrector.
    ///
    /// # Arguments
    ///
    /// * `in_size`    – Number of complex points per input vector.
    /// * `grpdly`     – Group delay in sample periods (fractional OK).
    /// * `skip_tail`  – Extra trailing points to discard beyond ceil(grpdly).
    ///                  Typically 1 for JEOL.
    /// * `max_out`    – If `Some(n)`, cap output size at `n`.
    pub fn new(
        in_size: usize,
        grpdly: f32,
        skip_tail: usize,
        max_out: Option<usize>,
    ) -> Self {
        let fft_size = next_pow2(in_size);

        let head_discard = grpdly.ceil() as usize;
        let mut out_size = in_size.saturating_sub(head_discard + skip_tail);

        if let Some(cap) = max_out {
            if cap > 0 && cap < out_size {
                out_size = cap;
            }
        }

        let mut planner = FftPlanner::<f32>::new();
        let fwd = planner.plan_fft_forward(fft_size);
        let inv = planner.plan_fft_inverse(fft_size);

        Self {
            fft_size,
            in_size,
            out_size,
            grpdly,
            skip_tail,
            fwd,
            inv,
        }
    }

    /// The corrected (shorter) output size in complex points.
    pub fn out_size(&self) -> usize {
        self.out_size
    }

    /// Apply the correction to one complex vector.
    ///
    /// `rdata` and `idata` must each have at least `in_size` elements.
    /// On return the first `out_size()` elements of each contain the
    /// corrected data.
    pub fn correct(&self, rdata: &mut [f32], idata: &mut [f32]) {
        let n = self.fft_size;
        let inv_n = 1.0 / n as f32;

        // ── 1. Build zero-padded complex buffer ─────────────────────────
        let mut buf: Vec<Complex<f32>> = Vec::with_capacity(n);
        for i in 0..n {
            if i < self.in_size {
                buf.push(Complex::new(rdata[i], idata[i]));
            } else {
                buf.push(Complex::zero());
            }
        }

        // ── 2. Inverse FFT (unnormalized, like FFTPACK cfftb) ───────────
        let mut scratch = vec![Complex::zero(); self.inv.get_inplace_scratch_len()];
        self.inv.process_with_scratch(&mut buf, &mut scratch);

        // ── 3. fftshift (swap halves) ───────────────────────────────────
        fftshift(&mut buf);

        // ── 4. Phase rotation: exp(−j·2π·k·grpdly/N) ───────────────────
        let neg_two_pi_grp = -2.0 * PI * self.grpdly;
        for k in 0..n {
            let angle = neg_two_pi_grp * k as f32 * inv_n;
            let (sin_a, cos_a) = angle.sin_cos();
            let rot = Complex::new(cos_a, sin_a);
            buf[k] = buf[k] * rot;
        }

        // ── 5. fftshift (swap halves) ───────────────────────────────────
        fftshift(&mut buf);

        // ── 6. Divide by N (c_scale normalisation) ──────────────────────
        for z in buf.iter_mut() {
            z.re *= inv_n;
            z.im *= inv_n;
        }

        // ── 7. Forward FFT (unnormalized, like FFTPACK cfftf) ───────────
        scratch.resize(self.fwd.get_inplace_scratch_len(), Complex::zero());
        self.fwd.process_with_scratch(&mut buf, &mut scratch);

        // ── 8. Double first point ───────────────────────────────────────
        buf[0].re *= 2.0;
        buf[0].im *= 2.0;

        // ── 9. Zero trailing contaminated region ────────────────────────
        for z in buf[self.out_size..].iter_mut() {
            *z = Complex::zero();
        }

        // ── 10. Extract output ──────────────────────────────────────────
        for i in 0..self.out_size {
            rdata[i] = buf[i].re;
            idata[i] = buf[i].im;
        }
    }

    /// Apply correction to a 2D matrix of complex vectors laid out as
    /// `[R0…Rn I0…In  R0…Rn I0…In  …]` (NMRPipe convention).
    ///
    /// Each row pair (real + imag, stride = 2 × `in_size`) is corrected
    /// in-place. After the call, the valid data in each row occupies
    /// `out_size()` points.
    pub fn correct_2d(&self, data: &mut [f32], y_size: usize) {
        let stride = 2 * self.in_size;
        for row in 0..y_size {
            let base = row * stride;
            if base + stride > data.len() {
                break;
            }
            let (r_part, i_part) = data[base..base + stride].split_at_mut(self.in_size);
            self.correct(r_part, i_part);
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_pow2() {
        assert_eq!(next_pow2(1), 1);
        assert_eq!(next_pow2(2), 2);
        assert_eq!(next_pow2(3), 4);
        assert_eq!(next_pow2(5), 8);
        assert_eq!(next_pow2(1024), 1024);
        assert_eq!(next_pow2(1025), 2048);
    }

    #[test]
    fn test_out_size_with_cap() {
        let corr = DFCorrector::new(1024, 70.0, 4, Some(900));
        assert_eq!(corr.out_size(), 900);

        let corr2 = DFCorrector::new(1024, 70.0, 4, None);
        assert_eq!(corr2.out_size(), 1024 - 70 - 4); // 950
    }

    #[test]
    fn test_skip_tail() {
        let corr = DFCorrector::new(256, 10.0, 1, None);
        // 256 - 10(ceil) - 1(skip_tail) = 245
        assert_eq!(corr.out_size(), 245);
    }

    #[test]
    fn test_fftshift() {
        let mut buf = vec![
            Complex::new(1.0, 0.0),
            Complex::new(2.0, 0.0),
            Complex::new(3.0, 0.0),
            Complex::new(4.0, 0.0),
        ];
        fftshift(&mut buf);
        assert_eq!(buf[0].re, 3.0);
        assert_eq!(buf[1].re, 4.0);
        assert_eq!(buf[2].re, 1.0);
        assert_eq!(buf[3].re, 2.0);
    }

    /// Round-trip identity: with grpdly=0 the output should match input.
    #[test]
    fn test_identity_grpdly_zero() {
        let n = 64usize;
        let mut rdata: Vec<f32> = (0..n).map(|i| (i as f32 * 0.1).sin()).collect();
        let mut idata: Vec<f32> = (0..n).map(|i| (i as f32 * 0.1).cos()).collect();
        let orig_r = rdata.clone();
        let orig_i = idata.clone();

        let corr = DFCorrector::new(n, 0.0, 0, None);
        let out_n = corr.out_size();
        corr.correct(&mut rdata, &mut idata);

        // With grpdly=0, the first point is doubled, rest should be unchanged
        assert!((rdata[0] - orig_r[0] * 2.0).abs() < 1e-4);
        assert!((idata[0] - orig_i[0] * 2.0).abs() < 1e-4);
        for i in 1..out_n {
            assert!(
                (rdata[i] - orig_r[i]).abs() < 1e-4,
                "real mismatch at {}: {} vs {}",
                i,
                rdata[i],
                orig_r[i]
            );
        }
    }
}
