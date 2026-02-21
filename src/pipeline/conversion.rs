/// Format detection and automatic conversion to NMRPipe format
///
/// Detects vendor formats (Bruker, Varian, JEOL) and invokes the
/// appropriate NMRPipe conversion tool transparently.

use std::path::{Path, PathBuf};
use std::io;
use std::fs;

use crate::data::spectrum::*;
use crate::data::jdf;
use crate::data::nmrpipe_format;
use crate::gui::conversion_dialog::ConversionSettings;
use crate::log::reproducibility::ReproLog;
use super::command::NmrPipeCommand;

/// Detect the vendor format from a path (file or directory)
pub fn detect_format(path: &Path) -> VendorFormat {
    if path.is_file() {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        match ext.as_str() {
            "jdf" => return VendorFormat::Jeol,
            "fid" | "ft1" | "ft2" | "ft3" => {
                // Could be NMRPipe or Varian — check header
                if let Ok(data) = fs::read(path) {
                    if data.len() >= 8 && &data[0..8] == b"JEOL.NMR" {
                        return VendorFormat::Jeol;
                    }
                }
                return VendorFormat::NMRPipe;
            }
            _ => {}
        }
    }

    if path.is_dir() {
        // Bruker: look for acqus, acqu, ser, fid
        if path.join("acqus").exists() || path.join("acqu").exists() {
            return VendorFormat::Bruker;
        }
        // Also check parent for Bruker (data might be in pdata/1/)
        if path.join("fid").exists() && path.join("procpar").exists() {
            return VendorFormat::Varian;
        }
        // Varian/Agilent: look for fid + procpar
        if path.join("fid").exists() || path.join("procpar").exists() {
            return VendorFormat::Varian;
        }
        // Check for .jdf files inside directory
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                if entry
                    .path()
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase() == "jdf")
                    .unwrap_or(false)
                {
                    return VendorFormat::Jeol;
                }
            }
        }
    }

    VendorFormat::Unknown
}

/// Conversion output directory
fn conversion_output_dir(source: &Path) -> PathBuf {
    let parent = source.parent().unwrap_or(Path::new("."));
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());
    parent.join(format!("{}_nmrpipe", stem))
}

/// Convert a JEOL .jdf file to NMRPipe format using delta2pipe.
///
/// Shells out to NMRPipe's `delta2pipe` tool which correctly handles
/// the proprietary JEOL Delta binary format.
fn convert_jeol(path: &Path, log: &mut ReproLog, settings: &ConversionSettings) -> io::Result<SpectrumData> {
    log.add_entry(
        "Format Detection",
        &format!("Detected JEOL Delta format: {}", path.display()),
        "",
    );

    // Check delta2pipe availability
    if jdf::find_delta2pipe().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "delta2pipe not found. Ensure NMRPipe is installed and in PATH.\n\
             delta2pipe is part of NMRPipe and converts JEOL Delta (.jdf) files.",
        ));
    }

    let out_dir = conversion_output_dir(path);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "data".to_string());

    // Use experiment type to hint dimensionality (unless user overrides ndim)
    let filename = stem.clone();
    let experiment_type = crate::data::spectrum::detect_experiment_type(&filename);
    let dim_hint = if settings.override_ndim {
        Some(settings.ndim)
    } else {
        match crate::data::spectrum::experiment_dimensionality(&experiment_type) {
            crate::data::spectrum::Dimensionality::TwoD => Some(2usize),
            crate::data::spectrum::Dimensionality::OneD => Some(1usize),
        }
    };

    // Build extra args from settings
    let extra_args = settings.to_args();

    // Run delta2pipe
    let result = jdf::convert_jdf(path, &out_dir, &stem, dim_hint, &extra_args)?;

    log.add_entry(
        "Conversion (delta2pipe)",
        &format!(
            "Converted JEOL Delta to NMRPipe format\n# Source: {}\n# Output: {}\n# Files: {}",
            path.display(),
            result.primary_file.display(),
            result.output_files.len(),
        ),
        &result.command_string,
    );

    // Read back the converted NMRPipe data.
    // read_nmrpipe_file handles both 1D and 2D (single-file) formats via the header.
    // Only use multi-plane reader if delta2pipe actually split across multiple files.
    let mut spectrum = if result.output_files.len() > 1 {
        nmrpipe_format::read_nmrpipe_2d_planes(&result.output_files)?
    } else {
        nmrpipe_format::read_nmrpipe_file(&result.primary_file)?
    };

    // Restore original source metadata
    spectrum.source_path = path.to_path_buf();
    spectrum.vendor_format = VendorFormat::Jeol;
    spectrum.experiment_type = experiment_type;
    spectrum.nmrpipe_path = Some(result.primary_file);
    spectrum.sample_name = stem;

    // Fix dimensionality based on actual data
    if !spectrum.data_2d.is_empty() {
        spectrum.dimensionality = crate::data::spectrum::Dimensionality::TwoD;
    }

    Ok(spectrum)
}

/// Convert Bruker data to NMRPipe format using bruk2pipe
fn convert_bruker(path: &Path, log: &mut ReproLog) -> io::Result<SpectrumData> {
    log.add_entry(
        "Format Detection",
        &format!("Detected Bruker format: {}", path.display()),
        "",
    );

    let out_dir = conversion_output_dir(path);
    fs::create_dir_all(&out_dir)?;
    let out_file = out_dir.join("test.fid");

    let cmd = NmrPipeCommand::new("bruk2pipe")
        .arg("-in").arg(&path.to_string_lossy())
        .arg("-bad").arg("0.0")
        .arg("-apts")
        .arg("-DMX")
        .arg("-decim").arg("1")
        .arg("-dspfvs").arg("0")
        .arg("-grpdly").arg("0")
        .arg("-out").arg(&out_file.to_string_lossy())
        .describe("Convert Bruker data to NMRPipe format");

    log.add_entry(
        "Conversion (bruk2pipe)",
        &format!("Converting Bruker data to NMRPipe format"),
        &cmd.to_command_string(),
    );

    let result = cmd.execute()?;
    if !result.success {
        // If bruk2pipe is not available, return an error with guidance
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "bruk2pipe conversion failed. Ensure NMRPipe is installed.\nstderr: {}",
                result.stderr
            ),
        ));
    }

    // Read back the converted file
    let mut spectrum = nmrpipe_format::read_nmrpipe_file(&out_file)?;
    spectrum.source_path = path.to_path_buf();
    spectrum.vendor_format = VendorFormat::Bruker;
    Ok(spectrum)
}

/// Convert Varian/Agilent data to NMRPipe format using var2pipe
fn convert_varian(path: &Path, log: &mut ReproLog) -> io::Result<SpectrumData> {
    log.add_entry(
        "Format Detection",
        &format!("Detected Varian/Agilent format: {}", path.display()),
        "",
    );

    let out_dir = conversion_output_dir(path);
    fs::create_dir_all(&out_dir)?;
    let out_file = out_dir.join("test.fid");

    let cmd = NmrPipeCommand::new("var2pipe")
        .arg("-in").arg(&path.to_string_lossy())
        .arg("-out").arg(&out_file.to_string_lossy())
        .arg("-noaswap")
        .describe("Convert Varian/Agilent data to NMRPipe format");

    log.add_entry(
        "Conversion (var2pipe)",
        &format!("Converting Varian/Agilent data to NMRPipe format"),
        &cmd.to_command_string(),
    );

    let result = cmd.execute()?;
    if !result.success {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "var2pipe conversion failed. Ensure NMRPipe is installed.\nstderr: {}",
                result.stderr
            ),
        ));
    }

    let mut spectrum = nmrpipe_format::read_nmrpipe_file(&out_file)?;
    spectrum.source_path = path.to_path_buf();
    spectrum.vendor_format = VendorFormat::Varian;
    Ok(spectrum)
}

/// Load spectrum from any supported format, converting if needed.
/// For JEOL files, `settings` controls delta2pipe parameters; pass `None` for defaults.
pub fn load_spectrum(
    path: &Path,
    log: &mut ReproLog,
    settings: Option<&ConversionSettings>,
) -> io::Result<SpectrumData> {
    let format = detect_format(path);
    log::info!("Detected format: {:?} for {}", format, path.display());

    let default_settings = ConversionSettings::default();
    let settings = settings.unwrap_or(&default_settings);

    match format {
        VendorFormat::Jeol => convert_jeol(path, log, settings),
        VendorFormat::Bruker => convert_bruker(path, log),
        VendorFormat::Varian => convert_varian(path, log),
        VendorFormat::NMRPipe => {
            log.add_entry(
                "Format Detection",
                &format!("File is already in NMRPipe format: {}", path.display()),
                "",
            );
            nmrpipe_format::read_nmrpipe_file(path)
        }
        VendorFormat::Unknown => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Unknown NMR data format for: {}. \
                 Supported: Bruker, Varian/Agilent, JEOL Delta (.jdf), NMRPipe",
                path.display()
            ),
        )),
    }
}

/// List all loadable NMR files in a directory
pub fn list_nmr_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                let ext = p
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                match ext.as_str() {
                    "jdf" | "fid" | "ft1" | "ft2" => {
                        files.push(p);
                    }
                    _ => {}
                }
            }
        }
    }
    files.sort();
    files
}
