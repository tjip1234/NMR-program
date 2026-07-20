//! Temporary diagnostic for the 2D pipeline.
//! Run with: cargo test --release twod_diag -- --nocapture

use std::path::Path;

use crate::data::native_converter::{convert_jdf_native, NativeJeolOptions};
use crate::data::spectrum::SpectrumData;
use crate::log::reproducibility::ReproLog;
use crate::pipeline::processing;

const COSY: &str = "test-files/example-2d/G4 Unknown Sample J_COSY-1-1.jdf";
const TOCSY: &str = "test-files/example-2d/G4 Unknown Sample J_TOCSY-1-1.jdf";
const HMQC: &str = "test-files/example-2d/G4 Unknown Sample J_HMQC_NUS-1-1.jdf";

/// Find local maxima of |data| above `frac`*max, deduped within `min_sep` points.
fn find_peaks(spec: &SpectrumData, frac: f64, max_peaks: usize) -> Vec<(f64, f64, f64)> {
    let data = &spec.data_2d;
    let n_rows = data.len();
    if n_rows == 0 {
        return Vec::new();
    }
    let n_cols = data[0].len();
    let max_val = data
        .iter()
        .flat_map(|r| r.iter())
        .map(|v| v.abs())
        .fold(0.0f64, f64::max);
    let thresh = frac * max_val;

    let mut peaks: Vec<(usize, usize, f64)> = Vec::new();
    for r in 1..n_rows.saturating_sub(1) {
        for c in 1..n_cols.saturating_sub(1) {
            let v = data[r][c];
            let a = v.abs();
            if a < thresh {
                continue;
            }
            // local max in 3x3 neighbourhood
            let mut is_max = true;
            'nb: for dr in -1i64..=1 {
                for dc in -1i64..=1 {
                    if dr == 0 && dc == 0 {
                        continue;
                    }
                    let rr = (r as i64 + dr) as usize;
                    let cc = (c as i64 + dc) as usize;
                    if data[rr][cc].abs() > a {
                        is_max = false;
                        break 'nb;
                    }
                }
            }
            if is_max {
                peaks.push((r, c, v));
            }
        }
    }
    peaks.sort_by(|a, b| b.2.abs().partial_cmp(&a.2.abs()).unwrap());

    // Dedup: keep peaks at least 8 points apart
    let min_sep = 8usize;
    let mut kept: Vec<(usize, usize, f64)> = Vec::new();
    for p in peaks {
        if kept
            .iter()
            .all(|k| k.0.abs_diff(p.0) > min_sep || k.1.abs_diff(p.1) > min_sep)
        {
            kept.push(p);
            if kept.len() >= max_peaks {
                break;
            }
        }
    }

    kept.iter()
        .map(|&(r, c, v)| {
            let f2 = spec.axes[0].index_to_ppm(c);
            let f1 = if spec.axes.len() > 1 {
                spec.axes[1].index_to_ppm(r)
            } else {
                r as f64
            };
            (f2, f1, v / max_val)
        })
        .collect()
}

const CARBON: &str = "test-files/example-2d/G4 Unknown Sample J_CARBON-1-1.jdf";

/// Find 1D peaks above frac*max as (ppm, rel_intensity).
fn find_peaks_1d(spec: &SpectrumData, frac: f64, max_peaks: usize) -> Vec<(f64, f64)> {
    let data = &spec.real;
    let max_val = data.iter().map(|v| v.abs()).fold(0.0f64, f64::max);
    let thresh = frac * max_val;
    let mut peaks: Vec<(usize, f64)> = Vec::new();
    for i in 1..data.len().saturating_sub(1) {
        let v = data[i];
        if v.abs() >= thresh && v.abs() >= data[i - 1].abs() && v.abs() > data[i + 1].abs() {
            peaks.push((i, v));
        }
    }
    peaks.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());
    let mut kept: Vec<(usize, f64)> = Vec::new();
    for p in peaks {
        if kept.iter().all(|k| k.0.abs_diff(p.0) > 4) {
            kept.push(p);
            if kept.len() >= max_peaks {
                break;
            }
        }
    }
    kept.iter()
        .map(|&(i, v)| (spec.axes[0].index_to_ppm(i), v / max_val))
        .collect()
}

#[test]
fn oned_diag_carbon() {
    // Vendor reference (JEOL PDF): 190.93, 164.69, 132.08, 130.03, 114.39,
    // 77.1 (CDCl3), 55.68 ppm.
    let path = Path::new(CARBON);
    assert!(path.exists(), "missing test file: {}", CARBON);
    let mut spec =
        convert_jdf_native(path, &NativeJeolOptions::default()).expect("conversion failed");
    let mut log = ReproLog::new();
    if !spec.is_frequency_domain {
        processing::fourier_transform(&mut spec, true, &mut log);
    }
    eprintln!(
        "\n== 13C 1D: npts={} ppm range [{:.3} .. {:.3}] ==",
        spec.axes[0].num_points,
        spec.axes[0].index_to_ppm(0),
        spec.axes[0].index_to_ppm(spec.axes[0].num_points.saturating_sub(1)),
    );
    let peaks = find_peaks_1d(&spec, 0.05, 12);
    for &(ppm, rel) in &peaks {
        eprintln!("  {:9.3} ppm   rel {:+.3}", ppm, rel);
    }
    for expected in [190.93, 164.69, 132.08, 114.39, 77.1, 55.68] {
        assert!(
            peaks.iter().any(|&(p, _)| (p - expected).abs() < 0.5),
            "no 13C peak near {} ppm (found {:?})",
            expected,
            peaks
        );
    }
}

/// Assert that a peak exists near (f2, f1) within the given tolerances.
fn assert_peak(peaks: &[(f64, f64, f64)], f2: f64, f1: f64, tol_f2: f64, tol_f1: f64) {
    assert!(
        peaks
            .iter()
            .any(|&(p2, p1, _)| (p2 - f2).abs() <= tol_f2 && (p1 - f1).abs() <= tol_f1),
        "no peak near F2={} F1={} (found: {:?})",
        f2,
        f1,
        peaks
    );
}

#[test]
fn twod_diag_cosy() {
    // Vendor reference (JEOL PDF): diagonal 9.85/7.8/7.0/3.87, cross 7.8<->7.0
    let peaks = run_diag(COSY);
    assert_peak(&peaks, 3.87, 3.87, 0.05, 0.15);
    assert_peak(&peaks, 6.99, 6.99, 0.05, 0.15);
    assert_peak(&peaks, 7.80, 7.80, 0.05, 0.15);
    assert_peak(&peaks, 9.87, 9.87, 0.05, 0.15);
    assert_peak(&peaks, 7.8, 7.0, 0.1, 0.15); // cross-peak
}

#[test]
fn twod_diag_tocsy() {
    let peaks = run_diag(TOCSY);
    assert_peak(&peaks, 3.87, 3.87, 0.05, 0.15);
    assert_peak(&peaks, 9.87, 9.87, 0.05, 0.15);
    assert_peak(&peaks, 7.8, 7.0, 0.1, 0.15); // cross-peak
}

#[test]
fn twod_diag_hmqc() {
    // Vendor reference: (3.87H, ~56C), (7.0, ~115), (7.8, ~131).
    // F1 grid is coarse (128 pts over 170 ppm), so tolerance is wide.
    let peaks = run_diag(HMQC);
    assert_peak(&peaks, 3.87, 56.0, 0.05, 4.0);
    assert_peak(&peaks, 7.00, 115.0, 0.05, 4.0);
    assert_peak(&peaks, 7.84, 131.0, 0.05, 4.0);
}

fn run_diag(file: &str) -> Vec<(f64, f64, f64)> {
    let path = Path::new(file);
    assert!(path.exists(), "missing test file: {}", file);

    let mut spec0 =
        convert_jdf_native(path, &NativeJeolOptions::default()).expect("conversion failed");
    // Mirror the GUI load path: attach the NUS schedule if present
    if let Some(nus) = crate::data::jdf::read_nus_schedule(path) {
        eprintln!(
            "NUS schedule: {} sampled / {} full F1 points",
            nus.indices.len(),
            nus.full_size
        );
        spec0.nus_indices = Some(nus.indices);
        spec0.nus_full_size = Some(nus.full_size);
    }
    eprintln!("\n== After conversion ==");
    eprintln!(
        "rows={} cols={} imag_rows={} is_freq_f2={} is_freq_f1={} y_is_complex={} quad={}",
        spec0.data_2d.len(),
        spec0.data_2d.first().map(|r| r.len()).unwrap_or(0),
        spec0.data_2d_imag.len(),
        spec0.is_freq_f2,
        spec0.is_freq_f1,
        spec0.y_is_complex,
        spec0.quad_mode,
    );
    for (i, ax) in spec0.axes.iter().enumerate() {
        eprintln!(
            "axis[{}] {}: npts={} sw={:.1}Hz obs={:.4}MHz ref_ppm={:.4} -> ppm range [{:.3} .. {:.3}]",
            i,
            ax.label,
            ax.num_points,
            ax.spectral_width_hz,
            ax.observe_freq_mhz,
            ax.reference_ppm,
            ax.index_to_ppm(0),
            ax.index_to_ppm(ax.num_points.saturating_sub(1)),
        );
    }

    let mut spec = spec0.clone();
    let mut log = ReproLog::new();
    if spec.is_freq_f2 && !spec.is_freq_f1 {
        eprintln!("-- using fourier_transform_f1_only --");
        processing::fourier_transform_f1_only(&mut spec, &mut log);
    } else {
        eprintln!("-- using fourier_transform_2d --");
        processing::fourier_transform_2d(&mut spec, &mut log);
    }

    eprintln!("\n== After FT ==");
    eprintln!(
        "rows={} cols={}",
        spec.data_2d.len(),
        spec.data_2d.first().map(|r| r.len()).unwrap_or(0)
    );
    for (i, ax) in spec.axes.iter().enumerate() {
        eprintln!(
            "axis[{}] {}: npts={} ppm range [{:.3} .. {:.3}]",
            i,
            ax.label,
            ax.num_points,
            ax.index_to_ppm(0),
            ax.index_to_ppm(ax.num_points.saturating_sub(1)),
        );
    }

    let peaks = find_peaks(&spec, 0.05, 25);
    eprintln!("\n== Peaks ==");
    for &(f2, f1, rel) in &peaks {
        eprintln!("  F2 {:8.3} ppm   F1 {:8.3} ppm   rel {:+.3}", f2, f1, rel);
    }
    peaks
}
