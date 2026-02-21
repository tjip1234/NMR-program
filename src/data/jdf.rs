/// JEOL Delta (.jdf) file conversion via delta2pipe
///
/// Uses NMRPipe's `delta2pipe` tool to convert JEOL Delta .jdf files
/// to NMRPipe format. This is the correct and reliable approach —
/// delta2pipe handles all the proprietary JDF binary details.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;

/// Locate the delta2pipe executable.
///
/// Checks PATH first, then falls back to common NMRPipe installation directories.
pub fn find_delta2pipe() -> Option<PathBuf> {
    // Check PATH via `which`
    if let Ok(output) = Command::new("which").arg("delta2pipe").output() {
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
            format!("{}/Documents/NMRpipe/nmrbin.linux239_64/delta2pipe", home),
            format!("{}/NMRPipe/nmrbin.linux239_64/delta2pipe", home),
            format!("{}/nmrpipe/bin/delta2pipe", home),
        ];
        for p in &home_paths {
            if Path::new(p).exists() {
                return Some(PathBuf::from(p));
            }
        }
    }

    // Check NMR_BASE environment variable (NMRPipe convention)
    if let Ok(nmr_base) = std::env::var("NMR_BASE") {
        let p = format!("{}/bin/delta2pipe", nmr_base);
        if Path::new(&p).exists() {
            return Some(PathBuf::from(p));
        }
    }

    // System-wide paths
    let system_paths = [
        "/usr/local/nmrpipe/bin/delta2pipe",
        "/opt/nmrpipe/bin/delta2pipe",
    ];
    for p in &system_paths {
        if Path::new(p).exists() {
            return Some(PathBuf::from(*p));
        }
    }

    // Hard-coded fallback for this system
    let fallback = PathBuf::from("/home/raaf/Documents/NMRpipe/nmrbin.linux239_64/delta2pipe");
    if fallback.exists() {
        return Some(fallback);
    }

    None
}

/// Run `delta2pipe -in <file> -all -info` and return the raw output text.
pub fn get_jdf_info(jdf_path: &Path) -> io::Result<String> {
    let exe = find_delta2pipe().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "delta2pipe not found. Ensure NMRPipe is installed and in PATH.",
        )
    })?;

    let output = Command::new(&exe)
        .args(["-in", &jdf_path.to_string_lossy(), "-all", "-info"])
        .output()?;

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    Ok(combined)
}

/// Determine dimensionality of a JDF file by parsing delta2pipe -info output.
///
/// Looks for `y_data_points` > 1 as indicator of 2D data.
/// Falls back to 1D if unable to determine.
pub fn detect_jdf_dimensionality(jdf_path: &Path) -> usize {
    let info = match get_jdf_info(jdf_path) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("Could not run delta2pipe -info: {}", e);
            return 1;
        }
    };

    for line in info.lines() {
        let trimmed = line.trim();
        // delta2pipe -info prints lines like:  y_data_points  256
        if trimmed.starts_with("y_data_points") || trimmed.contains("y_data_points") {
            if let Some(val_str) = trimmed.split_whitespace().last() {
                if let Ok(n) = val_str.parse::<usize>() {
                    if n > 1 {
                        log::info!("JDF dimensionality: 2D (y_data_points = {})", n);
                        return 2;
                    }
                }
            }
        }
    }

    log::info!("JDF dimensionality: 1D");
    1
}

/// Result of a delta2pipe conversion.
#[derive(Debug)]
pub struct Delta2PipeResult {
    /// Output files created by delta2pipe (one for 1D, many for 2D).
    pub output_files: Vec<PathBuf>,
    /// The first (or only) output file — use this to read metadata.
    pub primary_file: PathBuf,
    /// Whether the data is 2D (multiple plane files).
    pub is_2d: bool,
    /// The full command string for reproducibility logging.
    pub command_string: String,
    /// Combined stdout+stderr from delta2pipe.
    pub log_output: String,
}

/// Convert a JEOL Delta .jdf file to NMRPipe format using delta2pipe.
///
/// For 1D data, creates a single `<stem>.fid` file.
/// For 2D data, creates a series `<stem>001.fid`, `<stem>002.fid`, etc.
///
/// `extra_args` are additional command-line arguments (e.g. `-xN 26214 -xMODE Complex`).
///
/// Returns the list of output files and the command used.
pub fn convert_jdf(
    jdf_path: &Path,
    output_dir: &Path,
    stem: &str,
    ndim_hint: Option<usize>,
    extra_args: &[String],
) -> io::Result<Delta2PipeResult> {
    let exe = find_delta2pipe().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "delta2pipe not found. Ensure NMRPipe is installed and in PATH.",
        )
    })?;

    fs::create_dir_all(output_dir)?;

    // Determine dimensionality: use hint if provided, else probe the file
    let ndim = ndim_hint.unwrap_or_else(|| detect_jdf_dimensionality(jdf_path));
    let is_2d = ndim >= 2;

    // Build output path pattern
    let out_pattern = if is_2d {
        output_dir.join(format!("{}%03d.fid", stem))
    } else {
        output_dir.join(format!("{}.fid", stem))
    };

    let exe_str = exe.to_string_lossy().to_string();
    let in_str = jdf_path.to_string_lossy().to_string();
    let out_str = out_pattern.to_string_lossy().to_string();

    let mut all_args = vec![
        "-in".to_string(), in_str.clone(),
        "-out".to_string(), out_str.clone(),
        "-ov".to_string(),
    ];
    all_args.extend_from_slice(extra_args);

    let cmd_string = {
        let mut parts = vec![exe_str.clone(), "-in".to_string(), in_str, "-out".to_string(), out_str, "-ov".to_string()];
        parts.extend_from_slice(extra_args);
        parts.join(" ")
    };

    log::info!("Running: {}", cmd_string);

    let output = Command::new(&exe)
        .args(&all_args)
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
                "delta2pipe conversion failed (exit {}):\n{}",
                output.status.code().unwrap_or(-1),
                log_output,
            ),
        ));
    }

    log::info!("delta2pipe output: {}", log_output.trim());

    // Collect the output files
    let mut output_files = Vec::new();

    if is_2d {
        // delta2pipe creates stem001.fid, stem002.fid, ...
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
                "delta2pipe produced no output files. Command: {}\nOutput: {}",
                cmd_string, log_output
            ),
        ));
    }

    let primary_file = output_files[0].clone();

    Ok(Delta2PipeResult {
        output_files,
        primary_file,
        is_2d,
        command_string: cmd_string,
        log_output,
    })
}
