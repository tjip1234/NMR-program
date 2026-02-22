/// Bruker TopSpin / XWIN-NMR data conversion via bruk2pipe
///
/// Uses NMRPipe's `bruk2pipe` tool to convert Bruker NMR data to
/// NMRPipe format. This mirrors how jdf.rs handles JEOL Delta files
/// via delta2pipe — the NMRPipe converter handles all the gnarly
/// binary details correctly.
///
/// The `acqus` parameter file is parsed to extract the correct
/// arguments for bruk2pipe (SW, OBS, TD, DECIM, DSPFVS, GRPDLY, etc.)
/// so the conversion doesn't use hardcoded garbage values.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;

use super::spectrum::*;

// ────────────────────────────────────────────────────────────────
//  Locate bruk2pipe
// ────────────────────────────────────────────────────────────────

/// Locate the bruk2pipe executable.
///
/// Checks PATH first, then falls back to common NMRPipe installation directories.
pub fn find_bruk2pipe() -> Option<PathBuf> {
    // Check PATH via `which`
    if let Ok(output) = Command::new("which").arg("bruk2pipe").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }

    // Check common NMRPipe installation paths relative to $HOME
    if let Ok(home) = std::env::var("HOME") {
        let home_paths = [
            format!("{}/Documents/NMRpipe/nmrbin.linux239_64/bruk2pipe", home),
            format!("{}/NMRPipe/nmrbin.linux239_64/bruk2pipe", home),
            format!("{}/nmrpipe/bin/bruk2pipe", home),
        ];
        for p in &home_paths {
            if Path::new(p).exists() {
                return Some(PathBuf::from(p));
            }
        }
    }

    // Check NMR_BASE environment variable (NMRPipe convention)
    if let Ok(nmr_base) = std::env::var("NMR_BASE") {
        let p = format!("{}/bin/bruk2pipe", nmr_base);
        if Path::new(&p).exists() {
            return Some(PathBuf::from(p));
        }
    }

    // System-wide paths
    let system_paths = [
        "/usr/local/nmrpipe/bin/bruk2pipe",
        "/opt/nmrpipe/bin/bruk2pipe",
    ];
    for p in &system_paths {
        if Path::new(p).exists() {
            return Some(PathBuf::from(*p));
        }
    }

    // Hard-coded fallback for this system
    let fallback = PathBuf::from("/home/raaf/Documents/NMRpipe/nmrbin.linux239_64/bruk2pipe");
    if fallback.exists() {
        return Some(fallback);
    }

    None
}

// ────────────────────────────────────────────────────────────────
//  acqus parameter parsing
// ────────────────────────────────────────────────────────────────

/// Parsed Bruker acquisition parameters (from acqus / acqu2s)
#[derive(Debug, Default)]
pub struct BrukerParams {
    /// Spectral width in Hz
    pub sw_h: f64,
    /// Observe frequency in MHz (SFO1)
    pub sfo1: f64,
    /// Base frequency in MHz (BF1)
    pub bf1: f64,
    /// Offset frequency in Hz (O1)
    pub o1: f64,
    /// Total data points (TD) — includes real + imaginary
    pub td: usize,
    /// Data type: 0 = int32, 2 = float64
    pub dtypa: i32,
    /// Byte order: 0 = little-endian, 1 = big-endian
    pub bytorda: i32,
    /// Number of scans
    pub ns: i32,
    /// Nucleus name (e.g. "1H", "13C")
    pub nuc1: String,
    /// Pulse program name
    pub pulprog: String,
    /// Solvent
    pub solvent: String,
    /// Digital filter group delay (grpdly) — modern TopSpin sets this
    pub grpdly: f64,
    /// Decimation factor
    pub decim: i32,
    /// DSP firmware version
    pub dspfvs: i32,
    /// Acquisition mode (AQ_mod)
    pub aq_mod: i32,
    /// Indirect dimension TD
    pub td_f1: usize,
    /// Indirect dimension SW (Hz)
    pub sw_h_f1: f64,
    /// Indirect dimension observe freq (MHz)
    pub sfo1_f1: f64,
    /// Indirect dimension nucleus
    pub nuc1_f1: String,
    /// FnMODE (indirect dim acquisition mode for 2D)
    pub fnmode: i32,
}

/// Parse a Bruker `acqus` or `acqu2s` parameter file.
///
/// These files use a JCAMP-DX–like format with `##$PARAM= value` lines.
pub fn parse_acqus(content: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
    let mut current_key = String::new();
    let mut current_val = String::new();
    let mut in_multiline = false;

    for line in content.lines() {
        if line.starts_with("##$") {
            if !current_key.is_empty() {
                params.insert(current_key.clone(), current_val.trim().to_string());
            }
            if let Some(eq_pos) = line.find('=') {
                current_key = line[3..eq_pos].trim().to_string();
                current_val = line[eq_pos + 1..].trim().to_string();
                in_multiline = current_val.starts_with('(');
            } else {
                current_key.clear();
                current_val.clear();
                in_multiline = false;
            }
        } else if line.starts_with("##") {
            if !current_key.is_empty() {
                params.insert(current_key.clone(), current_val.trim().to_string());
                current_key.clear();
                current_val.clear();
                in_multiline = false;
            }
        } else if in_multiline || !current_key.is_empty() {
            current_val.push(' ');
            current_val.push_str(line.trim());
            if line.contains(')') {
                in_multiline = false;
            }
        }
    }
    if !current_key.is_empty() {
        params.insert(current_key, current_val.trim().to_string());
    }

    params
}

fn get_f64(params: &HashMap<String, String>, key: &str) -> f64 {
    params
        .get(key)
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0)
}

fn get_i32(params: &HashMap<String, String>, key: &str) -> i32 {
    params
        .get(key)
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0)
}

fn get_str(params: &HashMap<String, String>, key: &str) -> String {
    params
        .get(key)
        .map(|v| v.trim_matches(|c| c == '<' || c == '>').to_string())
        .unwrap_or_default()
}

/// Extract typed parameters from parsed acqus map
pub fn extract_params(acq: &HashMap<String, String>, acq2: Option<&HashMap<String, String>>) -> BrukerParams {
    let mut p = BrukerParams::default();
    p.sw_h = get_f64(acq, "SW_h");
    p.sfo1 = get_f64(acq, "SFO1");
    p.bf1 = get_f64(acq, "BF1");
    p.o1 = get_f64(acq, "O1");
    p.td = get_i32(acq, "TD") as usize;
    p.dtypa = get_i32(acq, "DTYPA");
    p.bytorda = get_i32(acq, "BYTORDA");
    p.ns = get_i32(acq, "NS");
    p.nuc1 = get_str(acq, "NUC1");
    p.pulprog = get_str(acq, "PULPROG");
    p.solvent = get_str(acq, "SOLVENT");
    p.grpdly = get_f64(acq, "GRPDLY");
    p.decim = get_i32(acq, "DECIM");
    p.dspfvs = get_i32(acq, "DSPFVS");
    p.aq_mod = get_i32(acq, "AQ_mod");

    if let Some(a2) = acq2 {
        p.td_f1 = get_i32(a2, "TD") as usize;
        p.sw_h_f1 = get_f64(a2, "SW_h");
        p.sfo1_f1 = {
            let v = get_f64(a2, "SFO1");
            if v > 0.0 { v } else { get_f64(acq, "SFO2") }
        };
        p.nuc1_f1 = {
            let s = get_str(a2, "NUC1");
            if s.is_empty() { get_str(acq, "NUC2") } else { s }
        };
        p.fnmode = get_i32(acq, "FnMODE");
    }

    p
}

// ────────────────────────────────────────────────────────────────
//  bruk2pipe conversion
// ────────────────────────────────────────────────────────────────

/// Result of a bruk2pipe conversion (mirrors jdf::Delta2PipeResult)
#[derive(Debug)]
pub struct Bruk2PipeResult {
    /// Output files created by bruk2pipe
    pub output_files: Vec<PathBuf>,
    /// The first (or only) output file
    pub primary_file: PathBuf,
    /// Whether the data is 2D
    pub is_2d: bool,
    /// The full command string for reproducibility logging
    pub command_string: String,
    /// Combined stdout+stderr from bruk2pipe
    pub log_output: String,
}

/// Map FnMODE (indirect dimension) to bruk2pipe -yMODE string
fn fnmode_string(fnmode: i32) -> &'static str {
    match fnmode {
        0 => "QF",
        1 => "QF",
        2 => "QSEQ",
        3 => "TPPI",
        4 => "States",
        5 => "States-TPPI",
        6 => "Echo-Antiecho",
        _ => "States-TPPI",
    }
}

/// Compute the Bruker digital filter group delay from DECIM and DSPFVS.
///
/// Lookup table from NMRPipe documentation and Bruker manuals.
/// Only used as fallback when GRPDLY is not set in acqus.
fn compute_grpdly(decim: i32, dspfvs: i32) -> f64 {
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

/// Parse nucleus string to Nucleus enum
fn parse_nucleus(nuc: &str) -> Nucleus {
    match nuc.trim().to_uppercase().as_str() {
        "1H" | "H1" => Nucleus::H1,
        "13C" | "C13" => Nucleus::C13,
        "15N" | "N15" => Nucleus::N15,
        "19F" | "F19" => Nucleus::F19,
        "31P" | "P31" => Nucleus::P31,
        "" | "OFF" => Nucleus::Other("Unknown".into()),
        other => Nucleus::Other(other.to_string()),
    }
}

/// Detect experiment type from Bruker pulse program name
pub fn detect_experiment_from_pulprog(pulprog: &str) -> ExperimentType {
    let upper = pulprog.to_uppercase();
    if upper.contains("HSQC") {
        ExperimentType::Hsqc
    } else if upper.contains("HMBC") {
        ExperimentType::Hmbc
    } else if upper.contains("COSY") {
        ExperimentType::Cosy
    } else if upper.contains("DEPT") || upper.contains("135") {
        ExperimentType::Dept135
    } else if upper.contains("ZGPG") || upper.contains("C13") || upper.contains("CARBON") {
        ExperimentType::Carbon
    } else if upper.contains("ZG") {
        ExperimentType::Proton
    } else {
        ExperimentType::Other(pulprog.to_string())
    }
}

/// Read acqus parameters from a Bruker experiment directory.
pub fn read_bruker_params(dir: &Path) -> io::Result<(BrukerParams, bool)> {
    let acqus_path = if dir.join("acqus").exists() {
        dir.join("acqus")
    } else if dir.join("acqu").exists() {
        dir.join("acqu")
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("No acqus or acqu file found in {}", dir.display()),
        ));
    };

    let acqus_content = fs::read_to_string(&acqus_path)?;
    let acq_map = parse_acqus(&acqus_content);

    let acq2_map = if dir.join("acqu2s").exists() {
        Some(parse_acqus(&fs::read_to_string(dir.join("acqu2s"))?))
    } else {
        None
    };

    let params = extract_params(&acq_map, acq2_map.as_ref());
    let is_2d = acq2_map.is_some() && params.td_f1 > 1;

    Ok((params, is_2d))
}

/// Convert a Bruker dataset to NMRPipe format using bruk2pipe.
///
/// Reads `acqus` (and `acqu2s` for 2D) to get the correct conversion
/// parameters, then shells out to bruk2pipe with proper arguments.
///
/// For 1D data: creates a single `<stem>.fid` file.
/// For 2D data: creates a series `<stem>%03d.fid` files.
pub fn convert_bruker_data(
    dir: &Path,
    output_dir: &Path,
    stem: &str,
) -> io::Result<Bruk2PipeResult> {
    let exe = find_bruk2pipe().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "bruk2pipe not found. Ensure NMRPipe is installed and in PATH.\n\
             bruk2pipe is part of NMRPipe and converts Bruker data files.",
        )
    })?;

    fs::create_dir_all(output_dir)?;

    // Read acquisition parameters from acqus
    let (params, is_2d) = read_bruker_params(dir)?;

    // Resolve group delay: use GRPDLY if set, otherwise compute from DECIM/DSPFVS
    let grpdly = if params.grpdly > 0.0 {
        params.grpdly
    } else {
        compute_grpdly(params.decim, params.dspfvs)
    };

    // Determine input file
    let in_file = if is_2d && dir.join("ser").exists() {
        dir.join("ser")
    } else {
        dir.join("fid")
    };

    // Output pattern
    let out_pattern = if is_2d {
        output_dir.join(format!("{}%03d.fid", stem))
    } else {
        output_dir.join(format!("{}.fid", stem))
    };

    let ndim = if is_2d { 2 } else { 1 };

    // Carrier position in ppm
    let car_ppm = if params.bf1 > 0.0 {
        params.o1 / params.bf1
    } else if params.sfo1 > 0.0 {
        params.o1 / params.sfo1
    } else {
        4.7 // default to ~water for 1H
    };

    // Build bruk2pipe arguments from acqus parameters
    let mut args: Vec<String> = vec![
        "-in".into(), in_file.to_string_lossy().to_string(),
        "-bad".into(), "0.0".into(),
        "-ext".into(),
        "-apts".into(),
        "-AMX".into(),
        "-decim".into(), format!("{}", params.decim),
        "-dspfvs".into(), format!("{}", params.dspfvs),
        "-grpdly".into(), format!("{:.4}", grpdly),
        "-DMX".into(),
        "-ndim".into(), format!("{}", ndim),
        // F2 (direct / x) dimension
        "-xN".into(), format!("{}", params.td),
        "-xT".into(), format!("{}", params.td / 2),
        "-xMODE".into(), "DQD".into(),
        "-xSW".into(), format!("{:.3}", params.sw_h),
        "-xOBS".into(), format!("{:.4}", params.sfo1),
        "-xCAR".into(), format!("{:.4}", car_ppm),
        "-xLAB".into(), params.nuc1.clone(),
    ];

    // 2D indirect dimension parameters
    if is_2d {
        let y_mode = fnmode_string(params.fnmode);
        let car_f1 = if params.sfo1_f1 > 0.0 { 0.0 } else { 0.0 };

        args.extend_from_slice(&[
            "-yN".into(), format!("{}", params.td_f1),
            "-yT".into(), format!("{}", params.td_f1 / 2),
            "-yMODE".into(), y_mode.into(),
            "-ySW".into(), format!("{:.3}", params.sw_h_f1),
            "-yOBS".into(), format!("{:.4}", params.sfo1_f1),
            "-yCAR".into(), format!("{:.4}", car_f1),
            "-yLAB".into(), params.nuc1_f1.clone(),
        ]);
    }

    args.extend_from_slice(&[
        "-out".into(), out_pattern.to_string_lossy().to_string(),
        "-ov".into(),
    ]);

    // Build command string for logging / reproducibility
    let cmd_string = {
        let mut parts = vec![exe.to_string_lossy().to_string()];
        parts.extend(args.clone());
        parts.join(" \\\n  ")
    };

    log::info!("Running: {}", cmd_string);

    let output = Command::new(&exe)
        .args(&args)
        .output()?;

    let log_output = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "bruk2pipe conversion failed (exit {}):\n{}\nCommand: {}",
                output.status.code().unwrap_or(-1),
                log_output,
                cmd_string,
            ),
        ));
    }

    log::info!("bruk2pipe output: {}", log_output.trim());

    // Collect output files
    let mut output_files = Vec::new();

    if is_2d {
        for i in 1..=99999 {
            let file = output_dir.join(format!("{}{:03}.fid", stem, i));
            if file.exists() {
                output_files.push(file);
            } else {
                break;
            }
        }
    } else {
        let file = output_dir.join(format!("{}.fid", stem));
        if file.exists() {
            output_files.push(file);
        }
    }

    if output_files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "bruk2pipe produced no output files.\nCommand: {}\nOutput: {}",
                cmd_string, log_output
            ),
        ));
    }

    let primary_file = output_files[0].clone();

    Ok(Bruk2PipeResult {
        output_files,
        primary_file,
        is_2d,
        command_string: cmd_string,
        log_output,
    })
}

// ────────────────────────────────────────────────────────────────
//  Native (built-in) Bruker reader — no NMRPipe required
// ────────────────────────────────────────────────────────────────

/// Read Bruker processed data from `pdata/1/` (1r = real, 1i = imaginary).
///
/// This is the built-in reader that works without NMRPipe.
/// It reads the processed spectrum directly from the `1r` file.
pub fn read_bruker_processed(dir: &Path) -> io::Result<SpectrumData> {
    let (params, is_2d) = read_bruker_params(dir)?;

    // Find pdata/1/ directory
    let pdata_dir = if dir.join("pdata/1/1r").exists() {
        dir.join("pdata/1")
    } else if dir.join("pdata").join("1").join("1r").exists() {
        dir.join("pdata").join("1")
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("No processed data (pdata/1/1r) found in {}", dir.display()),
        ));
    };

    // Parse procs to get processing parameters
    let procs_path = pdata_dir.join("procs");
    let proc_params = if procs_path.exists() {
        parse_acqus(&fs::read_to_string(&procs_path)?)
    } else {
        std::collections::HashMap::new()
    };

    let si = get_i32(&proc_params, "SI") as usize;
    let nc_proc = get_i32(&proc_params, "NC_proc");
    let sw_p = get_f64(&proc_params, "SW_p");
    let sf = get_f64(&proc_params, "SF");
    let offset = get_f64(&proc_params, "OFFSET");
    let bytordp = get_i32(&proc_params, "BYTORDP");
    let dtypp = get_i32(&proc_params, "DTYPP");

    // Read the 1r binary file
    let real_path = pdata_dir.join("1r");
    let raw = fs::read(&real_path)?;

    let npoints = if si > 0 { si } else { raw.len() / 4 };
    let scale = (2.0f64).powi(nc_proc);

    // Build metadata
    let sw_hz = if sw_p > 0.0 { sw_p } else { params.sw_h };
    let obs_mhz = if sf > 0.0 { sf } else { params.sfo1 };
    let ref_ppm = if offset != 0.0 { offset } else {
        if params.bf1 > 0.0 { params.o1 / params.bf1 + sw_hz / (2.0 * params.bf1) } else { 0.0 }
    };

    let nucleus = parse_nucleus(&params.nuc1);
    let experiment_type = detect_experiment_from_pulprog(&params.pulprog);
    let sample_name = dir.file_name()
        .or_else(|| dir.parent().and_then(|p| p.file_name()))
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Bruker".to_string());

    // Check for 2D processed data (2rr file)
    let rr_path = pdata_dir.join("2rr");
    if is_2d && rr_path.exists() {
        // Parse proc2s for indirect dimension parameters
        let proc2s_path = pdata_dir.join("proc2s");
        let proc2_params = if proc2s_path.exists() {
            parse_acqus(&fs::read_to_string(&proc2s_path)?)
        } else {
            std::collections::HashMap::new()
        };

        let si2 = {
            let v = get_i32(&proc2_params, "SI") as usize;
            if v > 0 { v } else { params.td_f1 / 2 }
        };
        let nc_proc2 = get_i32(&proc2_params, "NC_proc");
        let sw_p2 = get_f64(&proc2_params, "SW_p");
        let sf2 = get_f64(&proc2_params, "SF");
        let offset2 = get_f64(&proc2_params, "OFFSET");

        let scale2 = (2.0f64).powi(nc_proc2);
        let rr_raw = fs::read(&rr_path)?;

        // Read all processed 2D data
        let total_pts = if dtypp == 0 { rr_raw.len() / 4 } else { rr_raw.len() / 8 };
        let all_vals = if dtypp == 0 {
            read_int32_data(&rr_raw, total_pts, bytordp, scale2)
        } else {
            read_float64_data(&rr_raw, total_pts, bytordp, scale2)
        };

        // Split into rows: nrows = si2, ncols = si (direct dim)
        let ncols = if si > 0 { si } else { 1024 };
        let nrows = if si2 > 0 { si2 } else if ncols > 0 { all_vals.len() / ncols } else { 0 };

        let mut data_2d = Vec::with_capacity(nrows);
        for row_idx in 0..nrows {
            let start = row_idx * ncols;
            let end = (start + ncols).min(all_vals.len());
            if start >= all_vals.len() { break; }
            data_2d.push(all_vals[start..end].to_vec());
        }

        // F2 (direct, x) axis
        let axis_x = AxisParams {
            nucleus: nucleus.clone(),
            num_points: ncols,
            spectral_width_hz: sw_hz,
            observe_freq_mhz: obs_mhz,
            reference_ppm: ref_ppm,
            label: params.nuc1.clone(),
        };

        // F1 (indirect, y) axis
        let nucleus_f1 = parse_nucleus(&params.nuc1_f1);
        let sw_hz_f1 = if sw_p2 > 0.0 { sw_p2 } else { params.sw_h_f1 };
        let obs_mhz_f1 = if sf2 > 0.0 { sf2 } else { params.sfo1_f1 };
        let ref_ppm_f1 = if offset2 != 0.0 { offset2 } else {
            if params.sfo1_f1 > 0.0 { params.sw_h_f1 / (2.0 * params.sfo1_f1) } else { 0.0 }
        };
        let axis_y = AxisParams {
            nucleus: nucleus_f1,
            num_points: nrows,
            spectral_width_hz: sw_hz_f1,
            observe_freq_mhz: obs_mhz_f1,
            reference_ppm: ref_ppm_f1,
            label: params.nuc1_f1.clone(),
        };

        let real = data_2d.first().cloned().unwrap_or_default();

        return Ok(SpectrumData {
            source_path: dir.to_path_buf(),
            vendor_format: VendorFormat::Bruker,
            experiment_type,
            dimensionality: Dimensionality::TwoD,
            sample_name,
            axes: vec![axis_x, axis_y],
            real,
            imag: Vec::new(),
            data_2d,
            data_2d_imag: Vec::new(),
            is_frequency_domain: true,
            nmrpipe_path: None,
            conversion_method_used: "Built-in (Bruker 2D processed data reader)".to_string(),
        });
    }

    // 1D processed data
    let real: Vec<f64> = if dtypp == 0 {
        // 32-bit integers
        read_int32_data(&raw, npoints, bytordp, scale)
    } else {
        // 64-bit floats
        read_float64_data(&raw, npoints, bytordp, scale)
    };

    // Read imaginary if available
    let imag_path = pdata_dir.join("1i");
    let imag = if imag_path.exists() {
        let imag_raw = fs::read(&imag_path)?;
        if dtypp == 0 {
            read_int32_data(&imag_raw, npoints, bytordp, scale)
        } else {
            read_float64_data(&imag_raw, npoints, bytordp, scale)
        }
    } else {
        Vec::new()
    };

    let axis = AxisParams {
        nucleus: nucleus.clone(),
        num_points: real.len(),
        spectral_width_hz: sw_hz,
        observe_freq_mhz: obs_mhz,
        reference_ppm: ref_ppm,
        label: params.nuc1.clone(),
    };

    Ok(SpectrumData {
        source_path: dir.to_path_buf(),
        vendor_format: VendorFormat::Bruker,
        experiment_type,
        dimensionality: Dimensionality::OneD,
        sample_name,
        axes: vec![axis],
        real,
        imag,
        data_2d: Vec::new(),
        data_2d_imag: Vec::new(),
        is_frequency_domain: true, // processed data is always in frequency domain
        nmrpipe_path: None,
        conversion_method_used: "Built-in (Bruker processed data reader)".to_string(),
    })
}

/// Read raw Bruker FID data natively (built-in reader).
///
/// Reads the `fid` or `ser` binary file using parameters from `acqus`.
/// For 2D data (ser file with acqu2s), reads all rows as a 2D matrix.
pub fn read_bruker_fid(dir: &Path) -> io::Result<SpectrumData> {
    let (params, is_2d) = read_bruker_params(dir)?;

    let fid_path = if is_2d && dir.join("ser").exists() {
        dir.join("ser")
    } else if dir.join("fid").exists() {
        dir.join("fid")
    } else if dir.join("ser").exists() {
        dir.join("ser")
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("No fid or ser file found in {}", dir.display()),
        ));
    };

    let raw = fs::read(&fid_path)?;
    let npoints = if params.td > 0 { params.td } else {
        if params.dtypa == 0 { raw.len() / 4 } else { raw.len() / 8 }
    };

    let all_vals = if params.dtypa == 0 {
        read_int32_data(&raw, npoints, params.bytorda, 1.0)
    } else {
        read_float64_data(&raw, npoints, params.bytorda, 1.0)
    };

    let nucleus = parse_nucleus(&params.nuc1);
    let experiment_type = detect_experiment_from_pulprog(&params.pulprog);
    let sample_name = dir.file_name()
        .or_else(|| dir.parent().and_then(|p| p.file_name()))
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Bruker".to_string());

    let ref_ppm = if params.bf1 > 0.0 {
        params.o1 / params.bf1 + params.sw_h / (2.0 * params.bf1)
    } else {
        0.0
    };

    if is_2d && params.td_f1 > 1 {
        // 2D data: ser file contains multiple FIDs (rows)
        // Each row has TD (direct dim) points, complex interleaved
        let row_len = params.td; // points per row (complex interleaved)
        let nrows = if row_len > 0 { all_vals.len() / row_len } else { 0 };

        if nrows == 0 || row_len == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("2D ser file has invalid dimensions: TD={}, total points={}", row_len, all_vals.len()),
            ));
        }

        // Extract real and imaginary parts from each row (deinterleave complex)
        let npts_real = row_len / 2;
        let mut data_2d = Vec::with_capacity(nrows);
        let mut data_2d_imag = Vec::with_capacity(nrows);
        for row_idx in 0..nrows {
            let start = row_idx * row_len;
            let end = (start + row_len).min(all_vals.len());
            let row_data = &all_vals[start..end];
            // Deinterleave: even indices = real, odd indices = imaginary
            let real_row: Vec<f64> = row_data.iter().step_by(2).copied().collect();
            let imag_row: Vec<f64> = row_data.iter().skip(1).step_by(2).copied().collect();
            data_2d.push(real_row);
            data_2d_imag.push(imag_row);
        }

        // F2 (direct, x) axis
        let axis_x = AxisParams {
            nucleus: nucleus.clone(),
            num_points: npts_real,
            spectral_width_hz: params.sw_h,
            observe_freq_mhz: params.sfo1,
            reference_ppm: ref_ppm,
            label: params.nuc1.clone(),
        };

        // F1 (indirect, y) axis
        let nucleus_f1 = parse_nucleus(&params.nuc1_f1);
        let ref_ppm_f1 = if params.sfo1_f1 > 0.0 {
            // Carrier position in ppm for indirect dim
            params.sw_h_f1 / (2.0 * params.sfo1_f1)
        } else {
            0.0
        };
        let axis_y = AxisParams {
            nucleus: nucleus_f1,
            num_points: nrows,
            spectral_width_hz: params.sw_h_f1,
            observe_freq_mhz: params.sfo1_f1,
            reference_ppm: ref_ppm_f1,
            label: params.nuc1_f1.clone(),
        };

        // Use first row as the 1D projection
        let real = data_2d.first().cloned().unwrap_or_default();

        Ok(SpectrumData {
            source_path: dir.to_path_buf(),
            vendor_format: VendorFormat::Bruker,
            experiment_type,
            dimensionality: Dimensionality::TwoD,
            sample_name,
            axes: vec![axis_x, axis_y],
            real,
            imag: Vec::new(),
            data_2d,
            data_2d_imag,
            is_frequency_domain: false,
            nmrpipe_path: None,
            conversion_method_used: "Built-in (Bruker raw 2D FID reader)".to_string(),
        })
    } else {
        // 1D data: deinterleave real/imaginary
        let mut real = Vec::with_capacity(npoints / 2);
        let mut imag = Vec::with_capacity(npoints / 2);
        for pair in all_vals.chunks(2) {
            if pair.len() == 2 {
                real.push(pair[0]);
                imag.push(pair[1]);
            }
        }

        let axis = AxisParams {
            nucleus,
            num_points: real.len(),
            spectral_width_hz: params.sw_h,
            observe_freq_mhz: params.sfo1,
            reference_ppm: ref_ppm,
            label: params.nuc1.clone(),
        };

        Ok(SpectrumData {
            source_path: dir.to_path_buf(),
            vendor_format: VendorFormat::Bruker,
            experiment_type,
            dimensionality: Dimensionality::OneD,
            sample_name,
            axes: vec![axis],
            real,
            imag,
            data_2d: Vec::new(),
            data_2d_imag: Vec::new(),
            is_frequency_domain: false,
            nmrpipe_path: None,
            conversion_method_used: "Built-in (Bruker raw FID reader)".to_string(),
        })
    }
}

/// Read binary data as 32-bit integers, scaled
fn read_int32_data(raw: &[u8], npoints: usize, bytorda: i32, scale: f64) -> Vec<f64> {
    let mut data = Vec::with_capacity(npoints);
    let little_endian = bytorda == 0;
    for i in 0..npoints {
        let offset = i * 4;
        if offset + 4 > raw.len() { break; }
        let val = if little_endian {
            i32::from_le_bytes([raw[offset], raw[offset+1], raw[offset+2], raw[offset+3]])
        } else {
            i32::from_be_bytes([raw[offset], raw[offset+1], raw[offset+2], raw[offset+3]])
        };
        data.push(val as f64 * scale);
    }
    data
}

/// Read binary data as 64-bit floats, scaled
fn read_float64_data(raw: &[u8], npoints: usize, bytorda: i32, scale: f64) -> Vec<f64> {
    let mut data = Vec::with_capacity(npoints);
    let little_endian = bytorda == 0;
    for i in 0..npoints {
        let offset = i * 8;
        if offset + 8 > raw.len() { break; }
        let val = if little_endian {
            f64::from_le_bytes([raw[offset], raw[offset+1], raw[offset+2], raw[offset+3],
                               raw[offset+4], raw[offset+5], raw[offset+6], raw[offset+7]])
        } else {
            f64::from_be_bytes([raw[offset], raw[offset+1], raw[offset+2], raw[offset+3],
                               raw[offset+4], raw[offset+5], raw[offset+6], raw[offset+7]])
        };
        data.push(val * scale);
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_acqus_basic() {
        let content = r#"##TITLE= Parameter file
##JCAMP-DX= 5.00
##$SW_h= 8012.820
##$TD= 65536
##$SFO1= 400.13
##$BF1= 400.13
##$O1= 2400.39
##$DTYPA= 0
##$BYTORDA= 0
##$NS= 16
##$NUC1= <1H>
##$PULPROG= <zg30>
##$SOLVENT= <CDCl3>
##$GRPDLY= 76.0
##$DECIM= 2
##$DSPFVS= 12
##END=
"#;
        let params = parse_acqus(content);
        assert_eq!(params.get("SW_h").unwrap(), "8012.820");
        assert_eq!(params.get("TD").unwrap(), "65536");
        assert_eq!(params.get("NUC1").unwrap(), "<1H>");
        assert_eq!(params.get("PULPROG").unwrap(), "<zg30>");
    }

    #[test]
    fn test_extract_params() {
        let content = r#"##$SW_h= 8012.820
##$TD= 65536
##$SFO1= 400.130
##$BF1= 400.130
##$O1= 2400.390
##$DTYPA= 0
##$BYTORDA= 0
##$NS= 16
##$NUC1= <1H>
##$PULPROG= <zg30>
##$DECIM= 2
##$DSPFVS= 12
##$GRPDLY= 76.0
"#;
        let map = parse_acqus(content);
        let params = extract_params(&map, None);
        assert!((params.sw_h - 8012.82).abs() < 0.01);
        assert_eq!(params.td, 65536);
        assert!((params.sfo1 - 400.13).abs() < 0.01);
        assert_eq!(params.nuc1, "1H");
        assert_eq!(params.pulprog, "zg30");
        assert!((params.grpdly - 76.0).abs() < 0.01);
        assert_eq!(params.decim, 2);
        assert_eq!(params.dspfvs, 12);
    }

    #[test]
    fn test_parse_nucleus() {
        assert_eq!(parse_nucleus("1H"), Nucleus::H1);
        assert_eq!(parse_nucleus("13C"), Nucleus::C13);
        assert_eq!(parse_nucleus("15N"), Nucleus::N15);
        assert_eq!(parse_nucleus("19F"), Nucleus::F19);
        assert_eq!(parse_nucleus("31P"), Nucleus::P31);
        assert_eq!(parse_nucleus("OFF"), Nucleus::Other("Unknown".into()));
    }

    #[test]
    fn test_detect_experiment_from_pulprog() {
        assert_eq!(detect_experiment_from_pulprog("zg30"), ExperimentType::Proton);
        assert_eq!(detect_experiment_from_pulprog("zgpg30"), ExperimentType::Carbon);
        assert_eq!(detect_experiment_from_pulprog("cosygpqf"), ExperimentType::Cosy);
        assert_eq!(detect_experiment_from_pulprog("hsqcetgpsi2"), ExperimentType::Hsqc);
        assert_eq!(detect_experiment_from_pulprog("hmbcgplpndqf"), ExperimentType::Hmbc);
        assert_eq!(detect_experiment_from_pulprog("dept135"), ExperimentType::Dept135);
    }

    #[test]
    fn test_compute_grpdly() {
        assert!((compute_grpdly(2, 12) - 46.311).abs() < 0.001);
        assert!((compute_grpdly(4, 12) - 47.870).abs() < 0.001);
        assert!((compute_grpdly(1, 10) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_find_bruk2pipe() {
        // Just verifies the function doesn't panic.
        // On systems with NMRPipe it finds something, otherwise None.
        let result = find_bruk2pipe();
        if let Some(path) = &result {
            assert!(path.exists());
        }
    }

    #[test]
    fn test_fnmode_string() {
        assert_eq!(fnmode_string(0), "QF");
        assert_eq!(fnmode_string(5), "States-TPPI");
        assert_eq!(fnmode_string(6), "Echo-Antiecho");
    }
}
