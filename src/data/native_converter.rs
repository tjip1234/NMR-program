/// Native NMR format converter bridge.
///
/// Converts the output of the `delta2pipe` and `bruk2pipe` library crates
/// (NMRPipe FDATA header + f32 data planes) into our GUI's `SpectrumData`.
///
/// This eliminates the need for external NMRPipe tools — all conversion
/// happens in-process using pure Rust.

use std::io::{self, BufReader};
use std::path::Path;

use nmrpipe_core::fdata::*;
use nmrpipe_core::params::*;

use super::spectrum::*;

// ────────────────────────────────────────────────────────────────
//  FDATA → SpectrumData mapping
// ────────────────────────────────────────────────────────────────

/// Map nucleus label string (from FDATA header) to our `Nucleus` enum.
fn nucleus_from_label(label: &str) -> Nucleus {
    let upper = label.to_uppercase();
    if upper.contains("1H") || upper == "H1" || upper == "H" {
        Nucleus::H1
    } else if upper.contains("13C") || upper == "C13" || upper == "C" {
        Nucleus::C13
    } else if upper.contains("15N") || upper == "N15" || upper == "N" {
        Nucleus::N15
    } else if upper.contains("19F") || upper == "F19" {
        Nucleus::F19
    } else if upper.contains("31P") || upper == "P31" {
        Nucleus::P31
    } else if label.is_empty() {
        Nucleus::H1
    } else {
        Nucleus::Other(label.to_string())
    }
}

/// Extract `AxisParams` for a given dimension from the FDATA header.
fn axis_from_fdata(fdata: &Fdata, dim: i32) -> AxisParams {
    let size = fdata.get_size(dim) as usize;
    let sw = fdata.get_sw(dim);
    let obs = fdata.get_obs(dim);
    let orig = fdata.get_orig(dim);
    let label = fdata.get_parm_str(NDLABEL, dim);
    let nucleus = nucleus_from_label(&label);

    // NMRPipe convention: ORIG is the frequency (Hz) of the rightmost
    // (lowest-ppm) data point.  For our GUI, reference_ppm is the ppm
    // of the FIRST point (index 0 = highest ppm):
    //     reference_ppm = (ORIG + SW) / OBS
    let reference_ppm = if obs > 0.0 {
        (orig + sw) / obs
    } else {
        0.0
    };

    AxisParams {
        nucleus,
        num_points: size,
        spectral_width_hz: sw,
        observe_freq_mhz: obs,
        reference_ppm,
        label: if label.is_empty() {
            format!("F{}", dim)
        } else {
            label
        },
    }
}

/// Convert an NMRPipe FDATA header + data planes into a `SpectrumData`.
///
/// Works for both 1D (single plane) and 2D (one or more planes).
/// The `planes` vector is produced by `delta_to_pipe()` / `bruker_to_pipe()`.
///
/// Data layout in a plane:
/// - 1D complex: interleaved R,I,R,I,... with FDSIZE = number of complex pairs
/// - 1D real: sequential R,R,R,...
/// - 2D: one plane = y_size rows × x_row_width, sequential row-major;
///   each row may be complex-interleaved if FDQUADFLAG=0
fn fdata_planes_to_spectrum(
    source_path: &Path,
    fdata: &Fdata,
    planes: &[Vec<f32>],
) -> SpectrumData {
    let dim_count = fdata.dim_count().max(1);
    let is_pipe = fdata.is_pipe();

    let x_size = fdata.get_size(CUR_XDIM) as usize;
    let y_size = if dim_count >= 2 {
        fdata.get_size(CUR_YDIM) as usize
    } else {
        1
    };

    let is_complex = fdata.is_complex(CUR_XDIM);
    let is_freq = fdata.is_freq(CUR_XDIM);

    // Flatten all planes into one big f32 buffer
    let all_data: Vec<f32> = planes.iter().flat_map(|p| p.iter().copied()).collect();

    let filename = source_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let experiment_type = detect_experiment_type(&filename);

    let is_2d = dim_count >= 2 && y_size > 1;
    let dimensionality = if is_2d {
        Dimensionality::TwoD
    } else {
        Dimensionality::OneD
    };

    // Build axis info
    let axis_x = axis_from_fdata(fdata, CUR_XDIM);
    let mut axes = vec![axis_x];
    if is_2d {
        let axis_y = axis_from_fdata(fdata, CUR_YDIM);
        axes.push(axis_y);
    }

    let mut spectrum = SpectrumData {
        source_path: source_path.to_path_buf(),
        vendor_format: VendorFormat::Unknown, // caller will set
        experiment_type,
        dimensionality,
        sample_name: filename,
        axes,
        real: Vec::new(),
        imag: Vec::new(),
        data_2d: Vec::new(),
        data_2d_imag: Vec::new(),
        is_frequency_domain: is_freq,
        nmrpipe_path: None,
        conversion_method_used: String::new(),
    };

    if is_2d {
        // 2D data: split into y_size rows, each of x_row_width floats.
        let x_row_width = if y_size > 0 && !all_data.is_empty() {
            all_data.len() / y_size
        } else {
            x_size * if is_complex { 2 } else { 1 }
        };

        for row_idx in 0..y_size {
            let start = row_idx * x_row_width;
            let end = (start + x_row_width).min(all_data.len());
            if start >= all_data.len() {
                break;
            }
            let row_data = &all_data[start..end];

            if is_complex && row_data.len() >= x_size * 2 {
                let real_row: Vec<f64> = row_data.iter().step_by(2).map(|&v| v as f64).collect();
                let imag_row: Vec<f64> = row_data.iter().skip(1).step_by(2).map(|&v| v as f64).collect();
                spectrum.data_2d.push(real_row);
                spectrum.data_2d_imag.push(imag_row);
            } else {
                let real_row: Vec<f64> = row_data.iter().map(|&v| v as f64).collect();
                let len = real_row.len();
                spectrum.data_2d.push(real_row);
                spectrum.data_2d_imag.push(vec![0.0; len]);
            }
        }

        // Update x-axis num_points from actual data
        if let Some(first_row) = spectrum.data_2d.first() {
            if let Some(ax) = spectrum.axes.first_mut() {
                ax.num_points = first_row.len();
            }
            // Store first row as 1D fallback
            spectrum.real = first_row.clone();
        }
    } else {
        // 1D data
        if is_complex {
            if is_pipe {
                // Pipe mode: interleaved R,I,R,I,...
                spectrum.real = all_data.iter().step_by(2).map(|&v| v as f64).collect();
                spectrum.imag = all_data.iter().skip(1).step_by(2).map(|&v| v as f64).collect();
            } else {
                // File mode: sequential R...R then I...I
                let n = x_size.min(all_data.len());
                spectrum.real = all_data[..n].iter().map(|&v| v as f64).collect();
                if all_data.len() >= 2 * n {
                    spectrum.imag = all_data[n..2 * n].iter().map(|&v| v as f64).collect();
                }
            }
        } else {
            spectrum.real = all_data.iter().map(|&v| v as f64).collect();
        }

        // Update x-axis num_points from actual data
        if let Some(ax) = spectrum.axes.first_mut() {
            ax.num_points = spectrum.real.len();
        }
    }

    spectrum
}

// ────────────────────────────────────────────────────────────────
//  JEOL Delta (.jdf) native conversion
// ────────────────────────────────────────────────────────────────

/// Options for native JEOL conversion, derived from ConversionSettings.
pub struct NativeJeolOptions {
    pub real_only: bool,
    pub apply_df: bool,
    pub df_val: Option<f32>,
    pub verbose: bool,
}

impl Default for NativeJeolOptions {
    fn default() -> Self {
        Self {
            real_only: false,
            apply_df: false,
            df_val: None,
            verbose: false,
        }
    }
}

/// Convert a JEOL Delta .jdf file to SpectrumData using the native
/// `delta2pipe` library crate (no external tools needed).
pub fn convert_jdf_native(path: &Path, opts: &NativeJeolOptions) -> io::Result<SpectrumData> {
    let file = std::fs::File::open(path)?;
    let mut reader = BufReader::new(file);

    let delta_opts = delta2pipe::DeltaOptions {
        real_only: opts.real_only,
        apply_df: opts.apply_df,
        df_val: opts.df_val,
        tr_val: None,
        verbose: opts.verbose,
    };

    let result = delta2pipe::delta_to_pipe(&mut reader, &delta_opts)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let mut spectrum = fdata_planes_to_spectrum(path, &result.fdata, &result.planes);
    spectrum.vendor_format = VendorFormat::Jeol;
    spectrum.conversion_method_used = "Built-in (native delta2pipe)".to_string();

    // Fix dimensionality if data turns out to be 2D
    if !spectrum.data_2d.is_empty() {
        spectrum.dimensionality = Dimensionality::TwoD;
    }

    log::info!(
        "Native JEOL conversion: {} real pts, {}D, {} (df_stored={:.2}, df_applied={:.2})",
        spectrum.real.len(),
        if spectrum.is_2d() { 2 } else { 1 },
        if spectrum.is_frequency_domain { "freq" } else { "time" },
        result.stored_df_val,
        result.applied_df_val,
    );

    Ok(spectrum)
}

// ────────────────────────────────────────────────────────────────
//  Bruker native conversion
// ────────────────────────────────────────────────────────────────

/// Convert a Bruker dataset to SpectrumData using the native
/// `bruk2pipe` library crate (no external tools needed).
///
/// This reads acqus parameters, populates an FDATA header, then calls
/// `bruk2pipe::bruker_to_pipe()` for the raw binary conversion.
pub fn convert_bruker_native(dir: &Path) -> io::Result<SpectrumData> {
    use super::bruker;

    // Read acqus parameters
    let (params, is_2d) = bruker::read_bruker_params(dir)?;

    // Determine input file
    let in_file = if is_2d && dir.join("ser").exists() {
        dir.join("ser")
    } else if dir.join("fid").exists() {
        dir.join("fid")
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("No fid or ser file found in {}", dir.display()),
        ));
    };

    // Populate FDATA header from acqus parameters
    let mut fdata = Fdata::new();
    fdata.init_default();

    let dim_count = if is_2d { 2 } else { 1 };
    fdata.set_dim_count(dim_count);

    // X dimension (direct / F2)
    // TD includes both real and imaginary points for complex data
    let x_td = params.td as i32;
    let x_complex = true; // Bruker FIDs are always complex
    fdata.set_dim_spectral(
        CUR_XDIM,
        x_td,
        params.sw_h,
        params.sfo1,
        0.0, // orig computed later
        params.o1 / params.sfo1, // carrier in PPM (o1 is in Hz relative to SFO1)
        &params.nuc1,
        x_complex,
    );

    // APOD (valid points) and TDSIZE (original TD)
    fdata.set_parm(NDAPOD, x_td as f32, CUR_XDIM);
    fdata.set_parm(NDTDSIZE, x_td as f32, CUR_XDIM);

    if is_2d {
        let y_td = params.td_f1 as i32;
        let y_obs = if params.sfo1_f1 > 0.0 { params.sfo1_f1 } else { params.sfo1 };
        let y_car = if y_obs > 0.0 {
            // Try to compute carrier from O1 of indirect dim
            // In Bruker, the indirect carrier is in the acqu2s O1 field
            // but we may not have parsed it separately.
            // Fall back to 0.0 if needed — bruk2pipe will compute origin.
            0.0
        } else {
            0.0
        };

        let nuc_f1 = if params.nuc1_f1.is_empty() {
            &params.nuc1
        } else {
            &params.nuc1_f1
        };

        fdata.set_dim_spectral(
            CUR_YDIM,
            y_td,
            params.sw_h_f1,
            y_obs,
            0.0,
            y_car,
            nuc_f1,
            true,
        );
        fdata.set_parm(NDAPOD, y_td as f32, CUR_YDIM);
        fdata.set_parm(NDTDSIZE, y_td as f32, CUR_YDIM);

        // Acquisition sign from FnMODE
        // NMRPipe AqSign: None=0, Sequential=1, States=2
        // Bruker FnMODE: 0=undefined, 1=QF, 2=QSEQ, 3=TPPI, 4=States, 5=States-TPPI, 6=EA
        // For bruk2pipe, the aqsign is set as a raw value — use set_parm directly.
        let aqsign_val: f32 = match params.fnmode {
            0 | 1 => 0.0,  // QF / magnitude → no special sign
            2 => 1.0,      // QSEQ → sequential
            3 => 1.0,      // TPPI → sequential-like
            4 => 2.0,      // States → states
            5 => 2.0,      // States-TPPI → states
            6 => 2.0,      // Echo-Antiecho → states-like
            _ => 2.0,      // default to states
        };
        fdata.set_parm(NDAQSIGN, aqsign_val, CUR_YDIM);
    }

    // Determine Bruker type and group delay
    let grpdly = if params.grpdly > 0.0 {
        params.grpdly as f32
    } else {
        bruker_compute_grpdly(params.decim, params.dspfvs) as f32
    };

    let bruk_type = if params.decim > 1 || grpdly > 0.0 {
        bruk2pipe::BrukerType::Dmx
    } else {
        bruk2pipe::BrukerType::Amx
    };

    // Byte swap: Bruker data byte order from BYTORDA
    let needs_swap = if cfg!(target_endian = "little") {
        params.bytorda == 1 // big-endian data on little-endian host
    } else {
        params.bytorda == 0 // little-endian data on big-endian host
    };

    // Word size
    let word_size: usize = if params.dtypa == 2 { 8 } else { 4 };

    let bruker_opts = bruk2pipe::BrukerOptions {
        bruk_type,
        fdata,
        swap: needs_swap,
        i2f: true,
        word_size,
        byte_offset: 0,
        bad_thresh: 8_000_000.0,
        ext_flag: false,
        decim: params.decim,
        dspfvs: params.dspfvs,
        grpdly,
        aq_mod: params.aq_mod,
        fc: 1.0,
        skip_size: 4,
        zf_flag: true,
        verbose: false,
    };

    let file = std::fs::File::open(&in_file)?;
    let mut reader = BufReader::new(file);

    let result = bruk2pipe::bruker_to_pipe(&mut reader, &bruker_opts)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;

    let mut spectrum = fdata_planes_to_spectrum(dir, &result.fdata, &result.planes);
    spectrum.vendor_format = VendorFormat::Bruker;
    spectrum.conversion_method_used = "Built-in (native bruk2pipe)".to_string();

    // Use experiment type from pulse program
    spectrum.experiment_type = bruker::detect_experiment_from_pulprog(&params.pulprog);

    // Set sample name from directory
    spectrum.sample_name = dir
        .file_name()
        .or_else(|| dir.parent().and_then(|p| p.file_name()))
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "data".to_string());

    if !spectrum.data_2d.is_empty() {
        spectrum.dimensionality = Dimensionality::TwoD;
    }

    log::info!(
        "Native Bruker conversion: {} real pts, {}D, type={:?}, grpdly={:.2}",
        spectrum.real.len(),
        if spectrum.is_2d() { 2 } else { 1 },
        bruk_type,
        grpdly,
    );

    Ok(spectrum)
}

/// Compute the Bruker digital filter group delay from DECIM and DSPFVS.
/// (Fallback lookup table when GRPDLY is not set in acqus.)
fn bruker_compute_grpdly(decim: i32, dspfvs: i32) -> f64 {
    if decim <= 1 {
        return 0.0;
    }

    match dspfvs {
        10 => match decim {
            2 => 44.75, 3 => 33.5, 4 => 66.625, 6 => 59.0833,
            8 => 68.5625, 12 => 60.375, 16 => 69.5313, 24 => 61.0208,
            32 => 70.0156, 48 => 61.3438, 64 => 70.2578, 96 => 61.5052,
            128 => 70.3789, 192 => 61.5859, 256 => 70.4395, 384 => 61.6263,
            512 => 70.4697, 768 => 61.6465, 1024 => 70.4849, 1536 => 61.6566,
            2048 => 70.4924, _ => 0.0,
        },
        11 => match decim {
            2 => 46.0, 3 => 36.5, 4 => 48.0, 6 => 50.1667,
            8 => 53.25, 12 => 69.5, 16 => 72.25, 24 => 70.1667,
            32 => 72.75, 48 => 70.5, 64 => 73.0, 96 => 70.6667,
            128 => 72.5, 192 => 71.3333, 256 => 72.25, 384 => 71.6667,
            512 => 72.125, 768 => 71.8333, 1024 => 72.0625, 1536 => 71.9167,
            2048 => 72.0313, _ => 0.0,
        },
        12 => match decim {
            2 => 46.311, 3 => 36.530, 4 => 47.870, 6 => 50.229,
            8 => 53.289, 12 => 69.551, 16 => 71.600, 24 => 70.184,
            32 => 72.138, 48 => 70.528, 64 => 72.348, 96 => 70.700,
            128 => 72.524, _ => 0.0,
        },
        _ => 0.0,
    }
}
