/// NMR processing operations
///
/// Each operation works on SpectrumData in-place and records itself
/// in the reproducibility log. Operations that can use NMRPipe will
/// try the subprocess first, falling back to built-in implementations.

use std::f64::consts::PI;
use std::io;
use std::path::Path;

use num_complex::Complex;
use rustfft::FftPlanner;
use serde::{Deserialize, Serialize};

use crate::data::spectrum::*;
use crate::log::reproducibility::ReproLog;
use super::command::NmrPipeCommand;

/// Available window functions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WindowFunction {
    /// Exponential multiplication: line broadening in Hz
    Exponential { lb_hz: f64 },
    /// Gaussian multiplication
    Gaussian { gb: f64, lb_hz: f64 },
    /// Sine bell: power (1=sine, 2=sine-squared), offset (0-1), end (0-1)
    SineBell { power: f64, offset: f64, end: f64 },
    /// Cosine bell (equivalent to sine bell with offset=0.5)
    CosineBell,
    /// No apodization
    None,
}

impl std::fmt::Display for WindowFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WindowFunction::Exponential { lb_hz } => write!(f, "EM (LB={:.1} Hz)", lb_hz),
            WindowFunction::Gaussian { gb, lb_hz } => write!(f, "GM (GB={:.3}, LB={:.1} Hz)", gb, lb_hz),
            WindowFunction::SineBell { power, offset, end } => {
                write!(f, "Sine Bell (pow={:.1}, off={:.2}, end={:.2})", power, offset, end)
            }
            WindowFunction::CosineBell => write!(f, "Cosine Bell"),
            WindowFunction::None => write!(f, "None"),
        }
    }
}

/// Processing operation descriptor (for undo/redo)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessingOp {
    Apodization(WindowFunction),
    ZeroFill { target_size: usize },
    FourierTransform { use_imaginary: bool },
    FourierTransform2D,
    PhaseCorrection { ph0: f64, ph1: f64 },
    AutoPhase,
    BaselineCorrection,
    ManualBaselineCorrection { num_points: usize },
    SolventSuppression { center_ppm: f64, width_ppm: f64 },
}

impl std::fmt::Display for ProcessingOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessingOp::Apodization(wf) => write!(f, "Apodization: {}", wf),
            ProcessingOp::ZeroFill { target_size } => write!(f, "Zero Fill → {} points", target_size),
            ProcessingOp::FourierTransform { use_imaginary } => {
                if *use_imaginary {
                    write!(f, "Fourier Transform (Complex)")
                } else {
                    write!(f, "Fourier Transform (Real-only)")
                }
            }
            ProcessingOp::FourierTransform2D => write!(f, "2D Fourier Transform (Magnitude)"),
            ProcessingOp::PhaseCorrection { ph0, ph1 } => {
                write!(f, "Phase Correction (PH0={:.1}°, PH1={:.1}°)", ph0, ph1)
            }
            ProcessingOp::AutoPhase => write!(f, "Automatic Phase Correction"),
            ProcessingOp::BaselineCorrection => write!(f, "Baseline Correction"),
            ProcessingOp::ManualBaselineCorrection { num_points } => {
                write!(f, "Manual Baseline Correction ({} points)", num_points)
            }
            ProcessingOp::SolventSuppression { center_ppm, width_ppm } => {
                write!(f, "Solvent Suppression ({:.2} ± {:.2} ppm)", center_ppm, width_ppm)
            }
        }
    }
}

// =========================================================================
//  Apodization / Window Functions
// =========================================================================

/// Apply a window function to the FID data
pub fn apply_apodization(
    spectrum: &mut SpectrumData,
    window: &WindowFunction,
    log: &mut ReproLog,
) {
    let n = spectrum.real.len();
    if n == 0 {
        return;
    }

    let sw = spectrum
        .axes
        .first()
        .map(|a| a.spectral_width_hz)
        .unwrap_or(1.0);
    let dwell = if sw > 0.0 { 1.0 / sw } else { 1.0 / n as f64 };

    let nmrpipe_fn: String;

    match window {
        WindowFunction::Exponential { lb_hz } => {
            let lb = *lb_hz;
            for i in 0..n {
                let t = i as f64 * dwell;
                let factor = (-PI * lb * t).exp();
                spectrum.real[i] *= factor;
                if i < spectrum.imag.len() {
                    spectrum.imag[i] *= factor;
                }
            }
            nmrpipe_fn = format!("nmrPipe -fn EM -lb {:.3}", lb);
        }
        WindowFunction::Gaussian { gb, lb_hz } => {
            let lb = *lb_hz;
            let g = *gb;
            let tmax = n as f64 * dwell;
            for i in 0..n {
                let t = i as f64 * dwell;
                let factor =
                    (-PI * lb * t).exp() * (-(t / (2.0 * g * tmax)).powi(2)).exp();
                spectrum.real[i] *= factor;
                if i < spectrum.imag.len() {
                    spectrum.imag[i] *= factor;
                }
            }
            nmrpipe_fn = format!("nmrPipe -fn GM -g1 {:.6} -g2 {:.3} -g3 {:.6}", g, lb, 0.0);
        }
        WindowFunction::SineBell { power, offset, end } => {
            for i in 0..n {
                let frac = i as f64 / n as f64;
                let angle = PI * (*offset + frac * (*end - *offset));
                let factor = angle.sin().powf(*power);
                spectrum.real[i] *= factor;
                if i < spectrum.imag.len() {
                    spectrum.imag[i] *= factor;
                }
            }
            nmrpipe_fn = format!(
                "nmrPipe -fn SP -off {:.3} -end {:.3} -pow {:.1}",
                offset, end, power
            );
        }
        WindowFunction::CosineBell => {
            for i in 0..n {
                let frac = i as f64 / n as f64;
                let factor = (PI * frac / 2.0).cos();
                spectrum.real[i] *= factor;
                if i < spectrum.imag.len() {
                    spectrum.imag[i] *= factor;
                }
            }
            nmrpipe_fn = "nmrPipe -fn SP -off 0.5 -end 1.0 -pow 1.0".to_string();
        }
        WindowFunction::None => {
            return;
        }
    }

    log.add_entry(
        &format!("Apodization: {}", window),
        &format!("Applied {} to {} points", window, n),
        &nmrpipe_fn,
    );
}

// =========================================================================
//  Zero Filling
// =========================================================================

/// Zero-fill the FID to the target size (must be >= current size)
pub fn zero_fill(
    spectrum: &mut SpectrumData,
    target_size: usize,
    log: &mut ReproLog,
) {
    let current = spectrum.real.len();
    if target_size <= current {
        return;
    }

    spectrum.real.resize(target_size, 0.0);
    if !spectrum.imag.is_empty() {
        spectrum.imag.resize(target_size, 0.0);
    }

    if let Some(ax) = spectrum.axes.first_mut() {
        ax.num_points = target_size;
    }

    let nmrpipe_cmd = format!("nmrPipe -fn ZF -size {}", target_size);
    log.add_entry(
        "Zero Fill",
        &format!("Zero-filled from {} to {} points", current, target_size),
        &nmrpipe_cmd,
    );
}

/// Next power of two >= n
pub fn next_power_of_two(n: usize) -> usize {
    let mut p = 1;
    while p < n {
        p <<= 1;
    }
    p
}

// =========================================================================
//  Fourier Transform
// =========================================================================

/// Apply complex FFT to the FID data, converting to frequency domain
pub fn fourier_transform(
    spectrum: &mut SpectrumData,
    use_imaginary: bool,
    log: &mut ReproLog,
) {
    if spectrum.is_frequency_domain {
        log::warn!("Data is already in frequency domain, skipping FT");
        return;
    }

    let n = spectrum.real.len();
    if n == 0 {
        return;
    }

    // Ensure power of 2
    let fft_size = next_power_of_two(n);
    spectrum.real.resize(fft_size, 0.0);
    spectrum.imag.resize(fft_size, 0.0);

    // Build complex buffer
    let mut buffer: Vec<Complex<f64>> = if use_imaginary && !spectrum.imag.is_empty() {
        spectrum
            .real
            .iter()
            .zip(spectrum.imag.iter())
            .map(|(&r, &i)| Complex::new(r, i))
            .collect()
    } else {
        spectrum
            .real
            .iter()
            .map(|&r| Complex::new(r, 0.0))
            .collect()
    };

    // First-point correction: multiply the first complex point by 0.5
    // This removes the DC-offset artifact that appears at the edges of the
    // spectrum (standard NMR convention, equivalent to NMRPipe FT -auto).
    if !buffer.is_empty() {
        buffer[0] *= 0.5;
    }

    // Execute FFT
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_size);
    fft.process(&mut buffer);

    // FFT shift (swap halves so 0 Hz is in the center)
    let half = fft_size / 2;
    let mut shifted = vec![Complex::new(0.0, 0.0); fft_size];
    for i in 0..fft_size {
        shifted[i] = buffer[(i + half) % fft_size];
    }

    // Reverse so that index 0 = highest frequency (downfield / high ppm)
    // This matches the ppm_scale convention: index_to_ppm(0) = reference_ppm
    shifted.reverse();

    // Extract real and imaginary
    spectrum.real = shifted.iter().map(|c| c.re).collect();
    spectrum.imag = shifted.iter().map(|c| c.im).collect();

    // Auto-sign correction: if the spectrum is predominantly negative,
    // apply a 180° phase flip so absorption peaks point upward.
    let pos_sum: f64 = spectrum.real.iter().filter(|&&v| v > 0.0).sum();
    let neg_sum: f64 = spectrum.real.iter().filter(|&&v| v < 0.0).map(|v| v.abs()).sum();
    if neg_sum > pos_sum * 1.5 {
        for v in spectrum.real.iter_mut() {
            *v = -*v;
        }
        for v in spectrum.imag.iter_mut() {
            *v = -*v;
        }
    }
    spectrum.is_frequency_domain = true;

    if let Some(ax) = spectrum.axes.first_mut() {
        ax.num_points = fft_size;
    }

    let nmrpipe_cmd = if use_imaginary {
        "nmrPipe -fn FT -auto".to_string()
    } else {
        "nmrPipe -fn FT -real".to_string()
    };
    log.add_entry(
        "Fourier Transform",
        &format!(
            "{} FFT ({} → {} points, with FFT shift)",
            if use_imaginary { "Complex" } else { "Real-only" },
            n,
            fft_size
        ),
        &nmrpipe_cmd,
    );
}

// =========================================================================
//  2D Fourier Transform
// =========================================================================

/// Apply 2D FFT to a 2D time-domain spectrum (e.g. COSY, HSQC, HMBC).
///
/// Pipeline:
///   1. FFT along F2 (direct / rows) — each row is a complex FID
///   2. FFT along F1 (indirect / columns) — transpose, FFT each column
///   3. Compute magnitude: sqrt(re² + im²) for display
///
/// After processing `data_2d` contains the magnitude spectrum and
/// `data_2d_imag` is cleared.  `is_frequency_domain` is set to `true`.
pub fn fourier_transform_2d(
    spectrum: &mut SpectrumData,
    log: &mut ReproLog,
) {
    if spectrum.is_frequency_domain {
        log::warn!("2D data is already in frequency domain, skipping FT");
        return;
    }

    let n_rows = spectrum.data_2d.len();
    if n_rows == 0 {
        return;
    }
    let n_cols = spectrum.data_2d[0].len();
    if n_cols == 0 {
        return;
    }

    let has_imag = !spectrum.data_2d_imag.is_empty()
        && spectrum.data_2d_imag.len() == n_rows;

    // ── Step 1: FFT along F2 (rows) ──
    let fft_cols = next_power_of_two(n_cols);
    let mut planner = FftPlanner::new();
    let fft_f2 = planner.plan_fft_forward(fft_cols);

    // Store complex result matrix (rows × fft_cols)
    let mut re_2d = vec![vec![0.0f64; fft_cols]; n_rows];
    let mut im_2d = vec![vec![0.0f64; fft_cols]; n_rows];

    for row_idx in 0..n_rows {
        let row_len = spectrum.data_2d[row_idx].len();
        let mut buffer: Vec<Complex<f64>> = Vec::with_capacity(fft_cols);

        for col in 0..fft_cols {
            if col < row_len {
                let re = spectrum.data_2d[row_idx][col];
                let im = if has_imag && col < spectrum.data_2d_imag[row_idx].len() {
                    spectrum.data_2d_imag[row_idx][col]
                } else {
                    0.0
                };
                buffer.push(Complex::new(re, im));
            } else {
                buffer.push(Complex::new(0.0, 0.0)); // zero-pad
            }
        }

        // First-point correction (standard NMR convention)
        if !buffer.is_empty() {
            buffer[0] *= 0.5;
        }

        fft_f2.process(&mut buffer);

        // FFT-shift (swap halves)
        let half = fft_cols / 2;
        for i in 0..fft_cols {
            let si = (i + half) % fft_cols;
            re_2d[row_idx][i] = buffer[si].re;
            im_2d[row_idx][i] = buffer[si].im;
        }
    }

    // ── Step 2: FFT along F1 (columns) ──
    let fft_rows = next_power_of_two(n_rows);
    let fft_f1 = planner.plan_fft_forward(fft_rows);

    // Extend rows if needed (zero-pad in F1 dimension)
    re_2d.resize(fft_rows, vec![0.0; fft_cols]);
    im_2d.resize(fft_rows, vec![0.0; fft_cols]);

    for col_idx in 0..fft_cols {
        // Build column vector
        let mut col_buf: Vec<Complex<f64>> = Vec::with_capacity(fft_rows);
        for row_idx in 0..fft_rows {
            col_buf.push(Complex::new(re_2d[row_idx][col_idx], im_2d[row_idx][col_idx]));
        }

        // First-point correction in F1
        if !col_buf.is_empty() {
            col_buf[0] *= 0.5;
        }

        fft_f1.process(&mut col_buf);

        // FFT-shift (swap halves)
        let half = fft_rows / 2;
        for row_idx in 0..fft_rows {
            let si = (row_idx + half) % fft_rows;
            re_2d[row_idx][col_idx] = col_buf[si].re;
            im_2d[row_idx][col_idx] = col_buf[si].im;
        }
    }

    // ── Step 3: Compute magnitude and reverse axes ──
    // Reverse each row so index 0 → highest ppm (matches 1D convention)
    let mut magnitude = vec![vec![0.0f64; fft_cols]; fft_rows];
    for row_idx in 0..fft_rows {
        for col_idx in 0..fft_cols {
            let re = re_2d[row_idx][col_idx];
            let im = im_2d[row_idx][col_idx];
            // Reverse column direction (so high ppm = left = index 0)
            magnitude[row_idx][fft_cols - 1 - col_idx] = (re * re + im * im).sqrt();
        }
    }

    // Reverse row order for F1 (so high ppm = top = index 0)
    magnitude.reverse();

    // Store result
    spectrum.data_2d = magnitude;
    spectrum.data_2d_imag.clear();
    spectrum.is_frequency_domain = true;

    // Also set the 1D projection (first row) for the status bar
    spectrum.real = spectrum.data_2d.first().cloned().unwrap_or_default();
    spectrum.imag.clear();

    // Update axis sizes
    if let Some(ax) = spectrum.axes.get_mut(0) {
        ax.num_points = fft_cols;
    }
    if let Some(ax) = spectrum.axes.get_mut(1) {
        ax.num_points = fft_rows;
    }

    log.add_entry(
        "2D Fourier Transform",
        &format!(
            "Complex 2D FFT: {}×{} → {}×{} (magnitude mode)",
            n_rows, n_cols, fft_rows, fft_cols
        ),
        &format!(
            "nmrPipe -fn FT -auto  # F2\nnmrPipe -fn FT -auto  # F1"
        ),
    );
}

// =========================================================================
//  Phase Correction
// =========================================================================

/// Apply zero-order and first-order phase correction
pub fn phase_correct(
    spectrum: &mut SpectrumData,
    ph0_degrees: f64,
    ph1_degrees: f64,
    log: &mut ReproLog,
) {
    let n = spectrum.real.len();
    if n == 0 {
        return;
    }

    let ph0 = ph0_degrees * PI / 180.0;
    let ph1 = ph1_degrees * PI / 180.0;

    for i in 0..n {
        let frac = i as f64 / n as f64;
        let phase = ph0 + ph1 * frac;
        let cos_p = phase.cos();
        let sin_p = phase.sin();
        let re = spectrum.real[i];
        let im = if i < spectrum.imag.len() {
            spectrum.imag[i]
        } else {
            0.0
        };
        spectrum.real[i] = re * cos_p - im * sin_p;
        if i < spectrum.imag.len() {
            spectrum.imag[i] = re * sin_p + im * cos_p;
        }
    }

    let nmrpipe_cmd = format!("nmrPipe -fn PS -p0 {:.2} -p1 {:.2} -di", ph0_degrees, ph1_degrees);
    log.add_entry(
        "Phase Correction",
        &format!("PH0={:.2}°, PH1={:.2}°", ph0_degrees, ph1_degrees),
        &nmrpipe_cmd,
    );
}

/// Automatic phase correction using entropy minimization
pub fn auto_phase(
    spectrum: &mut SpectrumData,
    log: &mut ReproLog,
) -> (f64, f64) {
    let n = spectrum.real.len();
    if n == 0 {
        return (0.0, 0.0);
    }

    // Simple automatic phasing:
    // Search for ph0 that maximizes the integral of the real part
    // Then search for ph1 that minimizes baseline distortion
    let mut best_ph0 = 0.0f64;
    let mut best_score = f64::NEG_INFINITY;

    // Coarse search for ph0
    let mut ph0 = -180.0;
    while ph0 <= 180.0 {
        let score = evaluate_phase(spectrum, ph0, 0.0);
        if score > best_score {
            best_score = score;
            best_ph0 = ph0;
        }
        ph0 += 5.0;
    }

    // Fine search around best ph0
    let mut fine_ph0 = best_ph0 - 5.0;
    best_score = f64::NEG_INFINITY;
    while fine_ph0 <= best_ph0 + 5.0 {
        let score = evaluate_phase(spectrum, fine_ph0, 0.0);
        if score > best_score {
            best_score = score;
            best_ph0 = fine_ph0;
        }
        fine_ph0 += 0.5;
    }

    // Search for ph1
    let mut best_ph1 = 0.0f64;
    best_score = f64::NEG_INFINITY;
    let mut ph1 = -180.0;
    while ph1 <= 180.0 {
        let score = evaluate_phase(spectrum, best_ph0, ph1);
        if score > best_score {
            best_score = score;
            best_ph1 = ph1;
        }
        ph1 += 5.0;
    }

    // Fine search for ph1
    let saved_ph1 = best_ph1;
    best_score = f64::NEG_INFINITY;
    let mut fine_ph1 = saved_ph1 - 5.0;
    while fine_ph1 <= saved_ph1 + 5.0 {
        let score = evaluate_phase(spectrum, best_ph0, fine_ph1);
        if score > best_score {
            best_score = score;
            best_ph1 = fine_ph1;
        }
        fine_ph1 += 0.5;
    }

    // Apply the best phase
    phase_correct(spectrum, best_ph0, best_ph1, log);

    (best_ph0, best_ph1)
}

/// Evaluate phase quality: sum of positive real values (higher = better phased)
fn evaluate_phase(spectrum: &SpectrumData, ph0_deg: f64, ph1_deg: f64) -> f64 {
    let n = spectrum.real.len();
    let ph0 = ph0_deg * PI / 180.0;
    let ph1 = ph1_deg * PI / 180.0;

    let mut score = 0.0;
    for i in 0..n {
        let frac = i as f64 / n as f64;
        let phase = ph0 + ph1 * frac;
        let re = spectrum.real[i];
        let im = if i < spectrum.imag.len() {
            spectrum.imag[i]
        } else {
            0.0
        };
        let corrected_re = re * phase.cos() - im * phase.sin();
        // Penalize negative values (absorption mode should be mostly positive)
        if corrected_re > 0.0 {
            score += corrected_re;
        } else {
            score += corrected_re * 2.0; // Stronger penalty for negative
        }
    }
    score
}

// =========================================================================
//  Baseline Correction
// =========================================================================

/// Simple polynomial baseline correction
pub fn baseline_correct(
    spectrum: &mut SpectrumData,
    log: &mut ReproLog,
) {
    let n = spectrum.real.len();
    if n == 0 {
        return;
    }

    // Use the edge regions (first/last 10%) to estimate baseline
    let edge = (n as f64 * 0.1) as usize;
    let edge = edge.max(1);

    let left_mean: f64 = spectrum.real[..edge].iter().sum::<f64>() / edge as f64;
    let right_mean: f64 = spectrum.real[n - edge..].iter().sum::<f64>() / edge as f64;

    // Linear baseline subtraction
    for i in 0..n {
        let frac = i as f64 / n as f64;
        let baseline = left_mean + (right_mean - left_mean) * frac;
        spectrum.real[i] -= baseline;
    }

    let nmrpipe_cmd = "nmrPipe -fn POLY -auto".to_string();
    log.add_entry(
        "Baseline Correction",
        &format!(
            "Linear baseline correction (left={:.2}, right={:.2})",
            left_mean, right_mean
        ),
        &nmrpipe_cmd,
    );
}

/// Manual baseline correction using user-picked anchor points.
/// Performs piecewise-linear interpolation between sorted anchor points
/// and subtracts the resulting baseline from the spectrum.
pub fn manual_baseline_correct(
    spectrum: &mut SpectrumData,
    anchor_points: &[[f64; 2]], // (ppm, intensity) pairs
    log: &mut ReproLog,
) {
    let n = spectrum.real.len();
    if n == 0 || anchor_points.len() < 2 {
        return;
    }

    // Sort anchors by ppm
    let mut anchors = anchor_points.to_vec();
    anchors.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap());

    // Build the ppm scale
    let ppm_scale = if spectrum.is_frequency_domain && !spectrum.axes.is_empty() {
        spectrum.axes[0].ppm_scale()
    } else {
        (0..n).map(|i| i as f64).collect::<Vec<_>>()
    };

    // For each data point, interpolate baseline from anchors
    for i in 0..n {
        let ppm = ppm_scale[i];

        // Find surrounding anchors
        let baseline_val = if ppm <= anchors[0][0] {
            // Extrapolate from first two points
            let (x0, y0) = (anchors[0][0], anchors[0][1]);
            let (x1, y1) = (anchors[1][0], anchors[1][1]);
            if (x1 - x0).abs() > 1e-12 {
                y0 + (ppm - x0) * (y1 - y0) / (x1 - x0)
            } else {
                y0
            }
        } else if ppm >= anchors[anchors.len() - 1][0] {
            // Extrapolate from last two points
            let len = anchors.len();
            let (x0, y0) = (anchors[len - 2][0], anchors[len - 2][1]);
            let (x1, y1) = (anchors[len - 1][0], anchors[len - 1][1]);
            if (x1 - x0).abs() > 1e-12 {
                y0 + (ppm - x0) * (y1 - y0) / (x1 - x0)
            } else {
                y1
            }
        } else {
            // Interpolate between surrounding anchors
            let mut val = 0.0;
            for j in 0..anchors.len() - 1 {
                if ppm >= anchors[j][0] && ppm <= anchors[j + 1][0] {
                    let (x0, y0) = (anchors[j][0], anchors[j][1]);
                    let (x1, y1) = (anchors[j + 1][0], anchors[j + 1][1]);
                    let frac = if (x1 - x0).abs() > 1e-12 {
                        (ppm - x0) / (x1 - x0)
                    } else {
                        0.5
                    };
                    val = y0 + frac * (y1 - y0);
                    break;
                }
            }
            val
        };

        spectrum.real[i] -= baseline_val;
    }

    let ppm_list: Vec<String> = anchors.iter().map(|a| format!("{:.2}", a[0])).collect();
    log.add_entry(
        "Manual Baseline Correction",
        &format!(
            "Piecewise-linear baseline from {} anchor points at ppm: [{}]",
            anchors.len(),
            ppm_list.join(", ")
        ),
        &format!(
            "# Manual baseline correction with {} user-defined anchor points",
            anchors.len()
        ),
    );
}

// =========================================================================
//  Peak Detection
// =========================================================================

/// Simple peak detection: find local maxima above a noise threshold.
/// Returns peaks as `[ppm, intensity]` pairs sorted by ppm descending.
pub fn detect_peaks(
    spectrum: &SpectrumData,
    threshold_fraction: f64, // 0.0–1.0, fraction of max intensity
    min_distance: usize,     // minimum index distance between accepted peaks
) -> Vec<[f64; 2]> {
    let n = spectrum.real.len();
    if n < 3 {
        return vec![];
    }

    let max_val = spectrum
        .real
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);
    if max_val <= 0.0 {
        return vec![];
    }
    let threshold = max_val * threshold_fraction;

    // Collect local-maxima candidates above threshold
    let mut candidates: Vec<(usize, f64)> = Vec::new();
    for i in 1..n - 1 {
        let val = spectrum.real[i];
        if val > threshold
            && val >= spectrum.real[i - 1]
            && val >= spectrum.real[i + 1]
            && val > 0.0
        {
            candidates.push((i, val));
        }
    }

    // Keep strongest first, enforce minimum distance
    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let mut selected: Vec<usize> = Vec::new();
    for &(idx, _) in &candidates {
        let too_close = selected
            .iter()
            .any(|&s| (idx as i64 - s as i64).unsigned_abs() as usize <= min_distance);
        if !too_close {
            selected.push(idx);
        }
    }

    // Build ppm scale
    let ppm_scale = if spectrum.is_frequency_domain && !spectrum.axes.is_empty() {
        spectrum.axes[0].ppm_scale()
    } else {
        (0..n).map(|i| i as f64).collect()
    };

    let mut peaks: Vec<[f64; 2]> = selected
        .iter()
        .filter_map(|&i| {
            if i < ppm_scale.len() {
                Some([ppm_scale[i], spectrum.real[i]])
            } else {
                None
            }
        })
        .collect();

    // Sort by ppm descending (NMR convention: high ppm first)
    peaks.sort_by(|a, b| b[0].partial_cmp(&a[0]).unwrap());
    peaks
}

// =========================================================================
//  Multiplet Detection
// =========================================================================

/// A detected multiplet group
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Multiplet {
    /// Center ppm of the multiplet
    pub center_ppm: f64,
    /// Coupling constant J in Hz (average spacing between lines)
    pub j_hz: f64,
    /// Number of lines in the multiplet
    pub num_lines: usize,
    /// Classification label
    pub label: String,
    /// The peaks that form this multiplet: [ppm, intensity]
    pub peaks: Vec<[f64; 2]>,
}

impl std::fmt::Display for Multiplet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.j_hz > 0.0 {
            write!(f, "{:.2} ppm ({}, J={:.1} Hz)", self.center_ppm, self.label, self.j_hz)
        } else {
            write!(f, "{:.2} ppm ({})", self.center_ppm, self.label)
        }
    }
}

fn multiplet_label(n: usize) -> &'static str {
    match n {
        1 => "s",
        2 => "d",
        3 => "t",
        4 => "q",
        5 => "quint",
        6 => "sext",
        7 => "sept",
        _ => "m",
    }
}

/// Group detected peaks into multiplets based on coupling patterns.
///
/// `max_j_hz`: maximum coupling constant to consider (typically ~20 Hz for ¹H).
/// `obs_mhz`: observe frequency in MHz (needed to convert ppm spacing → Hz).
pub fn detect_multiplets(
    peaks: &[[f64; 2]],
    max_j_hz: f64,
    obs_mhz: f64,
) -> Vec<Multiplet> {
    if peaks.is_empty() || obs_mhz <= 0.0 {
        return vec![];
    }

    // Sort peaks by ppm ascending for grouping
    let mut sorted = peaks.to_vec();
    sorted.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap());

    // Convert max J from Hz to ppm
    let max_j_ppm = max_j_hz / obs_mhz;

    // Greedy grouping: walk through sorted peaks, group if gap ≤ max_j_ppm
    let mut groups: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut current_group: Vec<[f64; 2]> = vec![sorted[0]];

    for i in 1..sorted.len() {
        let gap = (sorted[i][0] - sorted[i - 1][0]).abs();
        if gap <= max_j_ppm {
            current_group.push(sorted[i]);
        } else {
            groups.push(std::mem::take(&mut current_group));
            current_group = vec![sorted[i]];
        }
    }
    if !current_group.is_empty() {
        groups.push(current_group);
    }

    // Build multiplets from groups
    let mut multiplets: Vec<Multiplet> = Vec::new();
    for group in &groups {
        let n = group.len();
        // Center ppm: intensity-weighted average
        let total_int: f64 = group.iter().map(|p| p[1].abs()).sum();
        let center = if total_int > 0.0 {
            group.iter().map(|p| p[0] * p[1].abs()).sum::<f64>() / total_int
        } else {
            group.iter().map(|p| p[0]).sum::<f64>() / n as f64
        };

        // Average J: mean spacing between consecutive lines (in Hz)
        let j_hz = if n >= 2 {
            let mut spacings = Vec::new();
            for i in 1..n {
                spacings.push((group[i][0] - group[i - 1][0]).abs() * obs_mhz);
            }
            spacings.iter().sum::<f64>() / spacings.len() as f64
        } else {
            0.0
        };

        multiplets.push(Multiplet {
            center_ppm: center,
            j_hz,
            num_lines: n,
            label: multiplet_label(n).to_string(),
            peaks: group.clone(),
        });
    }

    // Sort by ppm descending (NMR convention)
    multiplets.sort_by(|a, b| b.center_ppm.partial_cmp(&a.center_ppm).unwrap());
    multiplets
}

// =========================================================================
//  Integration
// =========================================================================

/// Integrate the spectrum between two ppm values (trapezoidal sum).
/// Returns the raw integral value — ratios between regions are what matter.
pub fn integrate_region(spectrum: &SpectrumData, start_ppm: f64, end_ppm: f64) -> f64 {
    if spectrum.axes.is_empty() || spectrum.real.is_empty() {
        return 0.0;
    }

    let ppm_scale = if spectrum.is_frequency_domain && !spectrum.axes.is_empty() {
        spectrum.axes[0].ppm_scale()
    } else {
        (0..spectrum.real.len()).map(|i| i as f64).collect()
    };

    let lo = start_ppm.min(end_ppm);
    let hi = start_ppm.max(end_ppm);

    let mut integral = 0.0;
    for i in 0..spectrum.real.len().min(ppm_scale.len()) {
        if ppm_scale[i] >= lo && ppm_scale[i] <= hi {
            integral += spectrum.real[i];
        }
    }

    integral
}

// =========================================================================
//  Solvent Suppression
// =========================================================================

/// Suppress solvent signal by zeroing a region around the specified ppm
pub fn solvent_suppress(
    spectrum: &mut SpectrumData,
    center_ppm: f64,
    width_ppm: f64,
    log: &mut ReproLog,
) {
    if !spectrum.is_frequency_domain {
        log::warn!("Solvent suppression should be applied in frequency domain");
        return;
    }

    let n = spectrum.real.len();
    if n == 0 {
        return;
    }

    if let Some(ax) = spectrum.axes.first() {
        let low_ppm = center_ppm - width_ppm / 2.0;
        let high_ppm = center_ppm + width_ppm / 2.0;

        for i in 0..n {
            let ppm = ax.index_to_ppm(i);
            if ppm >= low_ppm && ppm <= high_ppm {
                // Smooth transition using cosine window at edges
                let dist_from_center = (ppm - center_ppm).abs();
                let half_width = width_ppm / 2.0;
                if dist_from_center > half_width * 0.8 {
                    let edge_frac = (dist_from_center - half_width * 0.8) / (half_width * 0.2);
                    let factor = (edge_frac * PI / 2.0).sin();
                    spectrum.real[i] *= factor;
                    if i < spectrum.imag.len() {
                        spectrum.imag[i] *= factor;
                    }
                } else {
                    spectrum.real[i] = 0.0;
                    if i < spectrum.imag.len() {
                        spectrum.imag[i] = 0.0;
                    }
                }
            }
        }
    }

    let nmrpipe_cmd = format!(
        "nmrPipe -fn SOL -fl {} -fs {}",
        (width_ppm * 100.0) as i32,
        16
    );
    log.add_entry(
        "Solvent Suppression",
        &format!("Suppressed region: {:.2} ± {:.2} ppm", center_ppm, width_ppm / 2.0),
        &nmrpipe_cmd,
    );
}

// =========================================================================
//  NMRPipe Subprocess Execution
// =========================================================================

/// Execute a processing operation via NMRPipe subprocess
/// This is used when NMRPipe is available and the user prefers it
pub fn execute_via_nmrpipe(
    input_path: &Path,
    output_path: &Path,
    function_name: &str,
    params: &[(&str, &str)],
    log: &mut ReproLog,
) -> io::Result<()> {
    let mut cmd = NmrPipeCommand::new("nmrPipe")
        .arg("-in")
        .arg(&input_path.to_string_lossy())
        .arg("-fn")
        .arg(function_name);

    for (key, val) in params {
        cmd = cmd.arg(key).arg(val);
    }

    cmd = cmd
        .arg("-out")
        .arg(&output_path.to_string_lossy())
        .arg("-ov");

    log.add_entry(
        &format!("NMRPipe: {}", function_name),
        &format!("Executing via NMRPipe subprocess"),
        &cmd.to_command_string(),
    );

    let result = cmd.execute()?;
    if !result.success {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("NMRPipe execution failed: {}", result.stderr),
        ));
    }
    Ok(())
}
