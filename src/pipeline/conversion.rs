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
use crate::data::bruker;
use crate::data::jcamp;
use crate::gui::conversion_dialog::{ConversionMethod, ConversionSettings};
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
            "jdx" | "dx" | "jcamp" => return VendorFormat::Jcamp,
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
/// With BuiltIn method, returns an error since JEOL has no native reader.
fn convert_jeol(path: &Path, log: &mut ReproLog, settings: &ConversionSettings) -> io::Result<SpectrumData> {
    log.add_entry(
        "Format Detection",
        &format!("Detected JEOL Delta format: {}", path.display()),
        "",
    );

    if settings.conversion_method == ConversionMethod::BuiltIn {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "JEOL Delta (.jdf) files require NMRPipe's delta2pipe for conversion.\n\
             Switch to NMRPipe mode to load this format.",
        ));
    }

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
    spectrum.conversion_method_used = "NMRPipe (delta2pipe)".to_string();

    // Fix dimensionality based on actual data
    if !spectrum.data_2d.is_empty() {
        spectrum.dimensionality = crate::data::spectrum::Dimensionality::TwoD;
    }

    Ok(spectrum)
}

/// Convert Bruker data to NMRPipe format using bruk2pipe.
///
/// Reads the `acqus` parameter file to extract correct bruk2pipe
/// arguments (SW, OBS, TD, DECIM, DSPFVS, GRPDLY, etc.) so the
/// conversion doesn't use hard-coded garbage values.
///
/// This mirrors how `convert_jeol()` uses `delta2pipe`.
fn convert_bruker(path: &Path, log: &mut ReproLog, settings: &ConversionSettings) -> io::Result<SpectrumData> {
    log.add_entry(
        "Format Detection",
        &format!("Detected Bruker format: {}", path.display()),
        "",
    );

    // Decide which method to use
    let use_builtin = match settings.conversion_method {
        ConversionMethod::BuiltIn => true,
        ConversionMethod::NMRPipe => {
            if bruker::find_bruk2pipe().is_none() {
                log::warn!("bruk2pipe not found, falling back to built-in reader");
                true
            } else {
                false
            }
        }
    };

    if use_builtin {
        return convert_bruker_builtin(path, log);
    }

    convert_bruker_nmrpipe(path, log)
}

/// Convert Bruker data using NMRPipe's bruk2pipe
fn convert_bruker_nmrpipe(path: &Path, log: &mut ReproLog) -> io::Result<SpectrumData> {
    let out_dir = conversion_output_dir(path);
    let stem = path
        .file_name()
        .or_else(|| path.parent().and_then(|p| p.file_name()))
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "data".to_string());

    // Read acqus to get experiment metadata for the spectrum
    let (params, _is_2d) = bruker::read_bruker_params(path)?;
    let experiment_type = bruker::detect_experiment_from_pulprog(&params.pulprog);

    // Run bruk2pipe with args derived from acqus
    let result = bruker::convert_bruker_data(path, &out_dir, &stem)?;

    log.add_entry(
        "Conversion (bruk2pipe)",
        &format!(
            "Converted Bruker data to NMRPipe format\n\
             # Method: NMRPipe (bruk2pipe)\n\
             # Source: {}\n# Output: {}\n# Files: {}\n# Nucleus: {}\n# Pulse program: {}",
            path.display(),
            result.primary_file.display(),
            result.output_files.len(),
            params.nuc1,
            params.pulprog,
        ),
        &result.command_string,
    );

    // Read back the converted NMRPipe data
    let mut spectrum = if result.output_files.len() > 1 {
        nmrpipe_format::read_nmrpipe_2d_planes(&result.output_files)?
    } else {
        nmrpipe_format::read_nmrpipe_file(&result.primary_file)?
    };

    // Set metadata from acqus
    spectrum.source_path = path.to_path_buf();
    spectrum.vendor_format = VendorFormat::Bruker;
    spectrum.experiment_type = experiment_type;
    spectrum.nmrpipe_path = Some(result.primary_file);
    spectrum.sample_name = stem;
    spectrum.conversion_method_used = "NMRPipe (bruk2pipe)".to_string();

    if !spectrum.data_2d.is_empty() {
        spectrum.dimensionality = crate::data::spectrum::Dimensionality::TwoD;
    }

    Ok(spectrum)
}

/// Read Bruker data using built-in native reader
fn convert_bruker_builtin(path: &Path, log: &mut ReproLog) -> io::Result<SpectrumData> {
    // Try processed data first (pdata/1/1r), then raw FID
    let spectrum = if path.join("pdata/1/1r").exists() || path.join("pdata").join("1").join("1r").exists() {
        log.add_entry(
            "Load (built-in Bruker reader)",
            &format!("Reading Bruker processed data natively\n\
                      # Method: Built-in\n# Source: {}", path.display()),
            "# built-in reader — no NMRPipe required",
        );
        bruker::read_bruker_processed(path)?
    } else {
        log.add_entry(
            "Load (built-in Bruker reader)",
            &format!("Reading Bruker raw FID natively\n\
                      # Method: Built-in\n# Source: {}", path.display()),
            "# built-in reader — no NMRPipe required",
        );
        bruker::read_bruker_fid(path)?
    };

    log.add_entry(
        "Load (built-in Bruker reader)",
        &format!(
            "Loaded: {} points, {}, {}",
            spectrum.real.len(),
            spectrum.axes.first().map(|a| a.nucleus.to_string()).unwrap_or_default(),
            if spectrum.is_frequency_domain { "frequency domain" } else { "time domain" },
        ),
        "",
    );

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
    spectrum.conversion_method_used = "NMRPipe (var2pipe)".to_string();
    Ok(spectrum)
}

/// Read JCAMP-DX spectral data file natively
fn convert_jcamp(path: &Path, log: &mut ReproLog) -> io::Result<SpectrumData> {
    log.add_entry(
        "Format Detection",
        &format!("Detected JCAMP-DX format: {}", path.display()),
        "",
    );

    let spectrum = jcamp::read_jcamp_file(path)?;

    log.add_entry(
        "Load (native JCAMP-DX reader)",
        &format!(
            "Read JCAMP-DX file: {} points, {}, {}\n# Method: Built-in",
            spectrum.real.len(),
            spectrum.axes.first().map(|a| a.nucleus.to_string()).unwrap_or_default(),
            if spectrum.is_frequency_domain { "frequency domain" } else { "time domain" },
        ),
        "# native JCAMP-DX reader — no conversion needed",
    );

    let mut spectrum = spectrum;
    spectrum.conversion_method_used = "Built-in (JCAMP-DX reader)".to_string();

    Ok(spectrum)
}

/// Discover sibling NMRPipe plane files for a numbered plane file.
/// Given e.g. `/path/to/data001.fid`, finds `data002.fid`, `data003.fid`, etc.
/// Returns sorted list of all discovered plane files, or just the original file if
/// no numbered pattern is detected.
fn discover_nmrpipe_planes(path: &Path) -> Vec<PathBuf> {
    let stem = match path.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s.to_string(),
        None => return vec![path.to_path_buf()],
    };
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let parent = match path.parent() {
        Some(p) => p,
        None => return vec![path.to_path_buf()],
    };

    // Check if stem ends with digits (e.g., "data001", "test_2d003")
    // Find the split point between the base name and the numeric suffix
    let num_suffix_start = stem.rfind(|c: char| !c.is_ascii_digit())
        .map(|i| i + 1)
        .unwrap_or(0);

    if num_suffix_start >= stem.len() {
        // No numeric suffix found (or entire stem is digits — unlikely for NMR files)
        return vec![path.to_path_buf()];
    }

    let base_name = &stem[..num_suffix_start];
    let digit_len = stem.len() - num_suffix_start;

    // Scan directory for files matching this pattern
    let mut planes: Vec<(u32, PathBuf)> = Vec::new();
    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            let file_ext = p
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if file_ext != ext {
                continue;
            }
            if let Some(fstem) = p.file_stem().and_then(|s| s.to_str()) {
                if fstem.starts_with(base_name) {
                    let suffix = &fstem[base_name.len()..];
                    // Check suffix is all digits with matching length
                    if suffix.len() == digit_len && suffix.chars().all(|c| c.is_ascii_digit()) {
                        if let Ok(num) = suffix.parse::<u32>() {
                            planes.push((num, p));
                        }
                    }
                }
            }
        }
    }

    if planes.len() > 1 {
        planes.sort_by_key(|(n, _)| *n);
        planes.into_iter().map(|(_, p)| p).collect()
    } else {
        vec![path.to_path_buf()]
    }
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
        VendorFormat::Bruker => convert_bruker(path, log, settings),
        VendorFormat::Varian => convert_varian(path, log),
        VendorFormat::Jcamp => convert_jcamp(path, log),
        VendorFormat::NMRPipe => {
            log.add_entry(
                "Format Detection",
                &format!("File is already in NMRPipe format: {}", path.display()),
                "",
            );

            // Check if this is a numbered plane file (e.g., name001.fid)
            // If so, discover sibling planes and use read_nmrpipe_2d_planes
            let plane_files = discover_nmrpipe_planes(path);
            if plane_files.len() > 1 {
                log.add_entry(
                    "2D Plane Discovery",
                    &format!("Found {} plane files for 2D dataset", plane_files.len()),
                    "",
                );
                let mut spectrum = nmrpipe_format::read_nmrpipe_2d_planes(&plane_files)?;
                spectrum.conversion_method_used = "Direct (NMRPipe 2D planes)".to_string();
                Ok(spectrum)
            } else {
                let mut spectrum = nmrpipe_format::read_nmrpipe_file(path)?;
                spectrum.conversion_method_used = "Direct (NMRPipe format)".to_string();
                Ok(spectrum)
            }
        }
        VendorFormat::Unknown => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Unknown NMR data format for: {}. \
                 Supported: Bruker, Varian/Agilent, JEOL Delta (.jdf), JCAMP-DX (.jdx/.dx), NMRPipe",
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
                    "jdf" | "fid" | "ft1" | "ft2" | "jdx" | "dx" | "jcamp" => {
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
