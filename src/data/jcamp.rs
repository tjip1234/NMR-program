/// JCAMP-DX spectral data reader
///
/// JCAMP-DX (Joint Committee on Atomic and Molecular Physical Data — Data Exchange)
/// is a text-based format widely used for spectral data exchange.
///
/// File extensions: `.dx`, `.jdx`, `.jcamp`
///
/// Format overview:
///   - Lines starting with `##` are labeled data records (LDR)
///   - `##TITLE= ...` — spectrum title
///   - `##DATA TYPE= ...` — "NMR SPECTRUM", "NMR FID", etc.
///   - `##XUNITS= ...` — "HZ", "PPM", "1/CM", etc.
///   - `##YUNITS= ...` — "ARBITRARY UNITS", etc.
///   - `##FIRSTX= ...` — first X value
///   - `##LASTX= ...` — last X value
///   - `##NPOINTS= ...` — number of data points
///   - `##XYDATA= (X++(Y..Y))` — compressed data table (ASDF format)
///   - `##XYPOINTS= (XY..XY)` — simple X,Y pairs
///   - `##PEAK TABLE= (XY..XY)` — peak list
///
/// ASDF (ASCII Squeezed Difference Form) encoding:
///   Digits 0-9 are normal. Special characters encode compressed values:
///   - `@` through `I` (SQZ): represent 0-9 (positive)
///   - `a` through `i` (DIF): represent 1-9 differences (positive)
///   - `j` through `r` (DIF): represent -1 through -9 differences (negative)
///   - `%` through `.` (DUP): duplication counts
///
/// This reader handles the most common JCAMP-DX NMR spectral formats.

use std::io;
use std::path::Path;

use super::spectrum::*;

/// Parsed JCAMP-DX header fields
#[derive(Debug, Default)]
struct JcampHeader {
    title: String,
    data_type: String,
    x_units: String,
    y_units: String,
    first_x: f64,
    last_x: f64,
    x_factor: f64,
    y_factor: f64,
    npoints: usize,
    observe_freq: f64,   // .OBSERVE FREQUENCY
    observe_nucleus: String, // .OBSERVE NUCLEUS
    solvent: String,
    shift_reference: f64, // SHIFT REFERENCE
    data_class: String,   // XYDATA, XYPOINTS, PEAK TABLE, NTUPLES
}

/// Parse a JCAMP-DX file into a SpectrumData
pub fn read_jcamp_file(path: &Path) -> io::Result<SpectrumData> {
    let content = std::fs::read_to_string(path)?;
    parse_jcamp(&content, path)
}

/// Parse JCAMP-DX content string
fn parse_jcamp(content: &str, source_path: &Path) -> io::Result<SpectrumData> {
    // NTUPLES format (used by Bruker TopSpin JCAMP-DX export) needs
    // specialized handling — detect it early and dispatch.
    for line in content.lines() {
        let upper = line.trim().to_uppercase();
        if upper.starts_with("##DATA CLASS") && upper.contains("NTUPLES") {
            return parse_jcamp_ntuples(content, source_path);
        }
    }

    let mut header = JcampHeader::default();
    header.x_factor = 1.0;
    header.y_factor = 1.0;

    let mut data_lines: Vec<String> = Vec::new();
    let mut in_data_block = false;
    let mut data_format = String::new(); // "(X++(Y..Y))" or "(XY..XY)"

    // First pass: extract all header fields and data lines
    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("##") {
            // This is a labeled data record
            in_data_block = false;

            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[2..eq_pos].trim().to_uppercase();
                let value = trimmed[eq_pos + 1..].trim();

                match key.as_str() {
                    "TITLE" => header.title = value.to_string(),
                    "DATA TYPE" | "DATATYPE" => header.data_type = value.to_uppercase(),
                    "XUNITS" => header.x_units = value.to_uppercase(),
                    "YUNITS" => header.y_units = value.to_uppercase(),
                    "FIRSTX" => header.first_x = parse_jcamp_float(value),
                    "LASTX" => header.last_x = parse_jcamp_float(value),
                    "XFACTOR" => header.x_factor = parse_jcamp_float(value),
                    "YFACTOR" => header.y_factor = parse_jcamp_float(value),
                    "NPOINTS" | "NUMPOINTS" => {
                        header.npoints = parse_jcamp_float(value) as usize;
                    }
                    ".OBSERVE FREQUENCY" | "$REFERENCEPOINT" => {
                        header.observe_freq = parse_jcamp_float(value);
                    }
                    ".OBSERVE NUCLEUS" => {
                        header.observe_nucleus =
                            value.trim_matches(|c: char| c == '^' || c == ' ').to_string();
                    }
                    ".SOLVENT NAME" | "SOLVENT" => {
                        header.solvent = value.to_string();
                    }
                    ".SHIFT REFERENCE" => {
                        header.shift_reference = parse_shift_reference(value);
                    }
                    "DATA CLASS" | "DATACLASS" => {
                        header.data_class = value.to_uppercase();
                    }
                    "XYDATA" => {
                        data_format = value.to_string();
                        in_data_block = true;
                        header.data_class = "XYDATA".to_string();
                    }
                    "XYPOINTS" => {
                        data_format = value.to_string();
                        in_data_block = true;
                        header.data_class = "XYPOINTS".to_string();
                    }
                    "PEAK TABLE" | "PEAKTABLE" => {
                        data_format = value.to_string();
                        in_data_block = true;
                        header.data_class = "PEAK TABLE".to_string();
                    }
                    "END" => {
                        in_data_block = false;
                    }
                    _ => {}
                }
            }
        } else if in_data_block {
            // Data line — accumulate
            if !trimmed.is_empty() {
                data_lines.push(trimmed.to_string());
            }
        }
    }

    // Parse the data block
    let (_x_data, y_data) = if data_format.contains("X++(Y..Y)") || data_format.contains("X++") {
        parse_asdf_data(&data_lines, &header)?
    } else if data_format.contains("XY..XY") || header.data_class == "XYPOINTS" {
        parse_xy_pairs(&data_lines, &header)?
    } else if header.data_class == "PEAK TABLE" {
        parse_xy_pairs(&data_lines, &header)?
    } else if !data_lines.is_empty() {
        // Try to auto-detect: if first line has ASDF chars, use ASDF
        let first = &data_lines[0];
        if first.chars().any(|c| "ABCDEFGHIJabcdefghijklmnopqr@%STUV".contains(c)
            && !c.is_ascii_digit()
            && c != '.'
            && c != '-'
            && c != '+'
            && c != 'E'
            && c != 'e')
        {
            parse_asdf_data(&data_lines, &header)?
        } else {
            parse_xy_pairs(&data_lines, &header)?
        }
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "No data block found in JCAMP-DX file",
        ));
    };

    // Determine if the X axis is in Hz or ppm
    let is_ppm = header.x_units.contains("PPM");
    let is_hz = header.x_units.contains("HZ");
    let is_frequency_domain = is_ppm || is_hz || header.data_type.contains("SPECTRUM");

    // Build the spectrum
    let obs_mhz = if header.observe_freq > 0.0 {
        header.observe_freq
    } else {
        400.0 // default fallback
    };

    let (sw_hz, ref_ppm) = if is_ppm {
        let sw_ppm = (header.first_x - header.last_x).abs();
        let sw_hz = sw_ppm * obs_mhz;
        // In JCAMP, FIRSTX is usually the highest ppm (left edge)
        let ref_ppm = header.first_x.max(header.last_x);
        (sw_hz, ref_ppm)
    } else if is_hz {
        let sw_hz = (header.first_x - header.last_x).abs();
        let ref_ppm = if obs_mhz > 0.0 {
            header.first_x.max(header.last_x) / obs_mhz
        } else {
            0.0
        };
        (sw_hz, ref_ppm)
    } else {
        // Unknown units — just use raw values
        let sw = (header.first_x - header.last_x).abs();
        (sw, header.first_x.max(header.last_x))
    };

    let nucleus = parse_jcamp_nucleus(&header.observe_nucleus);
    let npoints = y_data.len();

    let axis = AxisParams {
        nucleus: nucleus.clone(),
        num_points: npoints,
        spectral_width_hz: sw_hz,
        observe_freq_mhz: obs_mhz,
        reference_ppm: ref_ppm,
        label: header.observe_nucleus.clone(),
    };

    let filename = source_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let experiment_type = if !header.observe_nucleus.is_empty() {
        match nucleus {
            Nucleus::C13 => ExperimentType::Carbon,
            Nucleus::H1 => ExperimentType::Proton,
            _ => detect_experiment_type(&filename),
        }
    } else {
        detect_experiment_type(&filename)
    };

    // JCAMP data order: if first_x > last_x, data goes from high to low ppm
    // We want data ordered high ppm → low ppm (index 0 = highest ppm)
    let real = if header.first_x < header.last_x {
        // Data goes low → high; reverse it
        y_data.into_iter().rev().collect()
    } else {
        y_data
    };

    Ok(SpectrumData {
        source_path: source_path.to_path_buf(),
        vendor_format: VendorFormat::Jcamp,
        experiment_type,
        dimensionality: Dimensionality::OneD,
        sample_name: if header.title.is_empty() {
            filename
        } else {
            header.title
        },
        axes: vec![axis],
        real,
        imag: Vec::new(),
        data_2d: Vec::new(),
        data_2d_imag: Vec::new(),
        is_frequency_domain,
        nmrpipe_path: None,
        conversion_method_used: "Built-in (JCAMP-DX reader)".to_string(),
    })
}

/// Parse a simple numeric value from a JCAMP field
fn parse_jcamp_float(s: &str) -> f64 {
    s.trim()
        .split_whitespace()
        .next()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0)
}

/// Parse .SHIFT REFERENCE field.
/// Format is typically: `(compound,nucleus,frequency,shift)` or just a number.
fn parse_shift_reference(s: &str) -> f64 {
    // Try to extract the last number which is the shift value
    let cleaned = s.trim().trim_matches(|c| c == '(' || c == ')');
    cleaned
        .split(',')
        .last()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(0.0)
}

/// Parse comma-separated float values (used by NTUPLES FACTOR/FIRST/LAST/VAR_DIM)
fn parse_csv_floats(s: &str) -> Vec<f64> {
    s.split(',')
        .map(|v| v.trim().parse::<f64>().unwrap_or(0.0))
        .collect()
}

/// Parse a JCAMP-DX file in NTUPLES format.
///
/// NTUPLES is a container format used by Bruker TopSpin for JCAMP-DX export.
/// It stores multiple variables (frequency, real spectrum, imaginary spectrum)
/// as separate "pages" within the file.
fn parse_jcamp_ntuples(content: &str, source_path: &Path) -> io::Result<SpectrumData> {
    let mut header = JcampHeader::default();
    header.x_factor = 1.0;
    header.y_factor = 1.0;

    // NTUPLES-specific metadata
    let mut nt_factors: Vec<f64> = Vec::new();
    let mut nt_firsts: Vec<f64> = Vec::new();
    let mut nt_lasts: Vec<f64> = Vec::new();
    let mut nt_dims: Vec<usize> = Vec::new();
    let mut nt_units: Vec<String> = Vec::new();

    // Per-page data collection
    let mut pages: Vec<Vec<String>> = Vec::new();
    let mut current_page_data: Vec<String> = Vec::new();
    let mut in_data_block = false;
    let mut in_ntuples = false;
    let mut page_format = String::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("##") {
            let was_in_data = in_data_block;
            in_data_block = false;

            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[2..eq_pos].trim().to_uppercase();
                let value = trimmed[eq_pos + 1..].trim();

                match key.as_str() {
                    "TITLE" => header.title = value.to_string(),
                    "DATA TYPE" | "DATATYPE" => header.data_type = value.to_uppercase(),
                    ".OBSERVE FREQUENCY" | "$REFERENCEPOINT" => {
                        header.observe_freq = parse_jcamp_float(value);
                    }
                    ".OBSERVE NUCLEUS" => {
                        header.observe_nucleus =
                            value.trim_matches(|c: char| c == '^' || c == ' ').to_string();
                    }
                    ".SOLVENT NAME" | "SOLVENT" => {
                        header.solvent = value.to_string();
                    }
                    ".SHIFT REFERENCE" => {
                        header.shift_reference = parse_shift_reference(value);
                    }
                    "NTUPLES" => {
                        in_ntuples = true;
                    }
                    "END NTUPLES" => {
                        if !current_page_data.is_empty() {
                            pages.push(std::mem::take(&mut current_page_data));
                        }
                        in_ntuples = false;
                    }
                    _ if in_ntuples => {
                        match key.as_str() {
                            "FACTOR" => nt_factors = parse_csv_floats(value),
                            "FIRST" => nt_firsts = parse_csv_floats(value),
                            "LAST" => nt_lasts = parse_csv_floats(value),
                            "VAR_DIM" => {
                                nt_dims = parse_csv_floats(value)
                                    .into_iter()
                                    .map(|v| v as usize)
                                    .collect();
                            }
                            "UNITS" => {
                                nt_units = value
                                    .split(',')
                                    .map(|s| s.trim().to_uppercase())
                                    .collect();
                            }
                            "PAGE" => {
                                if was_in_data && !current_page_data.is_empty() {
                                    pages.push(std::mem::take(&mut current_page_data));
                                }
                            }
                            "DATA TABLE" => {
                                page_format = value.to_string();
                                in_data_block = true;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        } else if in_data_block {
            if !trimmed.is_empty() && !trimmed.starts_with("$$") {
                current_page_data.push(trimmed.to_string());
            }
        }
    }

    // Save any remaining page data
    if !current_page_data.is_empty() {
        pages.push(current_page_data);
    }

    if pages.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "No data pages found in JCAMP-DX NTUPLES file",
        ));
    }

    // Extract NTUPLES metadata
    let npoints = nt_dims.first().copied().unwrap_or(0);
    let y_factor_real = nt_factors.get(1).copied().unwrap_or(1.0);
    let y_factor_imag = nt_factors.get(2).copied().unwrap_or(1.0);
    let first_x = nt_firsts.first().copied().unwrap_or(0.0);
    let last_x = nt_lasts.first().copied().unwrap_or(0.0);
    let x_unit = nt_units.first().cloned().unwrap_or_default();

    // Parse real data (first page)
    let real_header = JcampHeader {
        first_x,
        last_x,
        x_factor: 1.0,
        y_factor: y_factor_real,
        npoints,
        ..Default::default()
    };

    let (_x_data, real_data) = if page_format.contains("X++") {
        parse_asdf_data(&pages[0], &real_header)?
    } else {
        parse_xy_pairs(&pages[0], &real_header)?
    };

    // Parse imaginary data (second page, if present)
    let imag_data = if pages.len() > 1 {
        let imag_header = JcampHeader {
            first_x,
            last_x,
            x_factor: 1.0,
            y_factor: y_factor_imag,
            npoints,
            ..Default::default()
        };
        let (_x, imag) = if page_format.contains("X++") {
            parse_asdf_data(&pages[1], &imag_header)?
        } else {
            parse_xy_pairs(&pages[1], &imag_header)?
        };
        imag
    } else {
        Vec::new()
    };

    // Build spectrum metadata
    let is_ppm = x_unit.contains("PPM");
    let is_hz = x_unit.contains("HZ");
    let is_frequency_domain = is_ppm || is_hz || header.data_type.contains("SPECTRUM");

    let obs_mhz = if header.observe_freq > 0.0 {
        header.observe_freq
    } else {
        400.0
    };

    let (sw_hz, ref_ppm) = if is_ppm {
        let sw_ppm = (first_x - last_x).abs();
        let sw_hz = sw_ppm * obs_mhz;
        let ref_ppm = first_x.max(last_x);
        (sw_hz, ref_ppm)
    } else if is_hz {
        let sw_hz = (first_x - last_x).abs();
        let ref_ppm = if obs_mhz > 0.0 {
            first_x.max(last_x) / obs_mhz
        } else {
            0.0
        };
        (sw_hz, ref_ppm)
    } else {
        let sw = (first_x - last_x).abs();
        (sw, first_x.max(last_x))
    };

    let nucleus = parse_jcamp_nucleus(&header.observe_nucleus);
    let n = real_data.len();

    let axis = AxisParams {
        nucleus: nucleus.clone(),
        num_points: n,
        spectral_width_hz: sw_hz,
        observe_freq_mhz: obs_mhz,
        reference_ppm: ref_ppm,
        label: header.observe_nucleus.clone(),
    };

    let filename = source_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let experiment_type = if !header.observe_nucleus.is_empty() {
        match nucleus {
            Nucleus::C13 => ExperimentType::Carbon,
            Nucleus::H1 => ExperimentType::Proton,
            _ => detect_experiment_type(&filename),
        }
    } else {
        detect_experiment_type(&filename)
    };

    // Data order: if first_x > last_x, data runs high→low (what we want)
    let (real, imag) = if first_x < last_x {
        (
            real_data.into_iter().rev().collect(),
            if imag_data.is_empty() {
                Vec::new()
            } else {
                imag_data.into_iter().rev().collect()
            },
        )
    } else {
        (real_data, imag_data)
    };

    Ok(SpectrumData {
        source_path: source_path.to_path_buf(),
        vendor_format: VendorFormat::Jcamp,
        experiment_type,
        dimensionality: Dimensionality::OneD,
        sample_name: if header.title.is_empty() {
            filename
        } else {
            header.title
        },
        axes: vec![axis],
        real,
        imag,
        data_2d: Vec::new(),
        data_2d_imag: Vec::new(),
        is_frequency_domain,
        nmrpipe_path: None,
        conversion_method_used: "Built-in (JCAMP-DX NTUPLES reader)".to_string(),
    })
}

/// Parse JCAMP nucleus string to Nucleus enum
fn parse_jcamp_nucleus(nuc: &str) -> Nucleus {
    let upper = nuc.trim().to_uppercase();
    let cleaned = upper
        .replace('^', "")
        .replace(' ', "")
        .replace("NUC", "");
    match cleaned.as_str() {
        "1H" | "H1" | "H" => Nucleus::H1,
        "13C" | "C13" | "C" => Nucleus::C13,
        "15N" | "N15" | "N" => Nucleus::N15,
        "19F" | "F19" | "F" => Nucleus::F19,
        "31P" | "P31" | "P" => Nucleus::P31,
        _ => {
            if cleaned.is_empty() {
                Nucleus::Other("Unknown".into())
            } else {
                Nucleus::Other(cleaned)
            }
        }
    }
}

/// Parse (XY..XY) format — simple comma or space separated X,Y pairs
fn parse_xy_pairs(lines: &[String], header: &JcampHeader) -> io::Result<(Vec<f64>, Vec<f64>)> {
    let mut x_data = Vec::new();
    let mut y_data = Vec::new();

    for line in lines {
        // Split on commas, semicolons, or whitespace
        let tokens: Vec<&str> = line
            .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .collect();

        let mut i = 0;
        while i + 1 < tokens.len() {
            if let (Ok(x), Ok(y)) = (tokens[i].parse::<f64>(), tokens[i + 1].parse::<f64>()) {
                x_data.push(x * header.x_factor);
                y_data.push(y * header.y_factor);
                i += 2;
            } else {
                i += 1; // skip unparseable tokens
            }
        }
    }

    Ok((x_data, y_data))
}

/// Parse (X++(Y..Y)) ASDF format — compressed data with differences
///
/// In this format, each line starts with an X value, followed by Y values.
/// Y values may be encoded using ASDF characters for compression.
///
/// ASDF character classes:
///   SQZ digits (squeeze): `@`=0, `A`=1..`I`=9, `a`=-1..`i`=-9
///   DIF digits (difference): `%`=0, `J`=1..`R`=9, `j`=-1..`r`=-9
///   DUP count: `S`=1, `T`=2..`Z`=9, `s`=1 (sometimes)
fn parse_asdf_data(lines: &[String], header: &JcampHeader) -> io::Result<(Vec<f64>, Vec<f64>)> {
    let mut all_y: Vec<f64> = Vec::new();

    for line in lines {
        let decoded = decode_asdf_line(line);
        all_y.extend(decoded);
    }

    // Apply Y factor
    let y_data: Vec<f64> = all_y.iter().map(|&v| v * header.y_factor).collect();

    // Generate X values from FIRSTX, LASTX, and number of points
    let npoints = if header.npoints > 0 {
        header.npoints
    } else {
        y_data.len()
    };

    // Truncate or pad Y data to match expected npoints
    let y_final: Vec<f64> = if y_data.len() >= npoints {
        y_data[..npoints].to_vec()
    } else {
        let mut v = y_data;
        v.resize(npoints, 0.0);
        v
    };

    let x_data: Vec<f64> = if npoints > 1 {
        let dx = (header.last_x - header.first_x) * header.x_factor / (npoints - 1) as f64;
        (0..npoints)
            .map(|i| header.first_x * header.x_factor + i as f64 * dx)
            .collect()
    } else {
        vec![header.first_x * header.x_factor]
    };

    Ok((x_data, y_final))
}

/// Decode a single ASDF-encoded line.
///
/// A line looks like: `1234.5  J3K2M8j4k1` or `1234.5 100 -50 200 150`
///
/// If the line uses plain numbers, we just parse those.
/// If it uses ASDF encoding, we decode the compressed form.
fn decode_asdf_line(line: &str) -> Vec<f64> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    // The first token is always the X value (or checkpoint X); skip it
    // Then parse remaining tokens as Y values
    let mut chars = trimmed.chars().peekable();

    // Skip leading whitespace
    while chars.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
        chars.next();
    }

    // First token: X value — skip it
    // X value can be negative or have decimal points
    let mut x_consumed = false;
    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() && x_consumed {
            chars.next(); // consume the whitespace
            break;
        }
        if ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+' || ch == 'E' || ch == 'e' {
            chars.next();
            x_consumed = true;
        } else if !x_consumed {
            // First char is an ASDF char — this might be a continuation line without X
            // In that case, parse the entire line as Y values
            return decode_asdf_values(trimmed);
        } else {
            break;
        }
    }

    // Skip whitespace between X and Y data
    while chars.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
        chars.next();
    }

    // Collect remaining as Y value string
    let y_str: String = chars.collect();
    if y_str.is_empty() {
        return Vec::new();
    }

    // Check if Y values are plain numbers or ASDF encoded
    let has_asdf = y_str.chars().any(|c| is_asdf_char(c));
    if has_asdf {
        decode_asdf_values(&y_str)
    } else {
        // Plain numbers separated by whitespace or commas
        y_str
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<f64>().ok())
            .collect()
    }
}

/// Check if a character is an ASDF special character
fn is_asdf_char(c: char) -> bool {
    matches!(c,
        '@' | 'A'..='I' |       // SQZ positive
        'a'..='i' |             // SQZ negative (or DIF positive)
        'J'..='R' |             // DIF positive
        'j'..='r' |             // DIF negative
        'S'..='Z' | 's'        // DUP
    )
}

/// Decode ASDF-encoded Y values
fn decode_asdf_values(s: &str) -> Vec<f64> {
    // First, tokenize into numbers (each started by a SQZ char or a sign/digit)
    let mut values: Vec<f64> = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();

    // Track if we're in DIF mode (after first SQZ establishes a value)
    let mut last_value: f64 = 0.0;
    let mut in_dif_mode = false;

    while i < n {
        let ch = chars[i];

        // Skip whitespace and commas
        if ch.is_whitespace() || ch == ',' {
            i += 1;
            continue;
        }

        // SQZ character: starts a new absolute number
        if ch == '@' || ('A'..='I').contains(&ch) || ('a'..='i').contains(&ch) {
            let (val, new_i) = read_sqz_number(&chars, i);
            last_value = val;
            values.push(val);
            in_dif_mode = false;
            i = new_i;
            continue;
        }

        // DIF character: difference from last value
        if ('J'..='R').contains(&ch) || ('j'..='r').contains(&ch) {
            let (diff, new_i) = read_dif_number(&chars, i);
            last_value += diff;
            values.push(last_value);
            in_dif_mode = true;
            i = new_i;
            continue;
        }

        // DUP character: repeat last value
        if ('S'..='Z').contains(&ch) || ch == 's' {
            let dup_count = match ch {
                'S' | 's' => 1,
                'T' => 2,
                'U' => 3,
                'V' => 4,
                'W' => 5,
                'X' => 6,
                'Y' => 7,
                'Z' => 8,
                _ => 1,
            };
            // Read any following digits for larger dup counts
            i += 1;
            let mut count_str = String::new();
            while i < n && chars[i].is_ascii_digit() {
                count_str.push(chars[i]);
                i += 1;
            }
            let total_dup = if count_str.is_empty() {
                dup_count
            } else {
                // The DUP char gives the first digit, following chars extend it
                format!("{}{}", dup_count, count_str).parse().unwrap_or(dup_count)
            };

            // Repeat the last Y value (or last difference)
            if in_dif_mode {
                // In DIF mode, DUP means repeat the last *difference*
                let last_diff = if values.len() >= 2 {
                    values[values.len() - 1] - values[values.len() - 2]
                } else {
                    0.0
                };
                for _ in 0..total_dup {
                    last_value += last_diff;
                    values.push(last_value);
                }
            } else {
                for _ in 0..total_dup {
                    values.push(last_value);
                }
            }
            continue;
        }

        // Regular digit, sign, or decimal: parse as a plain number
        if ch.is_ascii_digit() || ch == '+' || ch == '-' || ch == '.' {
            let start = i;
            i += 1;
            while i < n
                && (chars[i].is_ascii_digit()
                    || chars[i] == '.'
                    || chars[i] == 'E'
                    || chars[i] == 'e'
                    || ((chars[i] == '+' || chars[i] == '-')
                        && i > start
                        && (chars[i - 1] == 'E' || chars[i - 1] == 'e')))
            {
                i += 1;
            }
            let num_str: String = chars[start..i].iter().collect();
            if let Ok(val) = num_str.parse::<f64>() {
                last_value = val;
                values.push(val);
                in_dif_mode = false;
            }
            continue;
        }

        // Unknown character — skip
        i += 1;
    }

    values
}

/// Read a SQZ-encoded number starting at position i.
/// SQZ chars: `@`=0, `A`=1..`I`=9, `a`=-1..`i`=-9
/// Followed by regular digits.
fn read_sqz_number(chars: &[char], start: usize) -> (f64, usize) {
    let ch = chars[start];
    let (first_digit, negative) = match ch {
        '@' => (0i64, false),
        'A'..='I' => ((ch as i64 - 'A' as i64 + 1), false),
        'a'..='i' => ((ch as i64 - 'a' as i64 + 1), true),
        _ => (0, false),
    };

    let mut i = start + 1;
    let mut num_str = first_digit.to_string();

    // Consume following digits
    while i < chars.len() && chars[i].is_ascii_digit() {
        num_str.push(chars[i]);
        i += 1;
    }

    let value: f64 = num_str.parse().unwrap_or(0.0);
    let value = if negative { -value } else { value };
    (value, i)
}

/// Read a DIF-encoded number starting at position i.
/// DIF chars: `J`=1..`R`=9 (positive), `j`=-1..`r`=-9 (negative)
/// Followed by regular digits.
fn read_dif_number(chars: &[char], start: usize) -> (f64, usize) {
    let ch = chars[start];
    let (first_digit, negative) = match ch {
        'J'..='R' => ((ch as i64 - 'J' as i64 + 1), false),
        'j'..='r' => ((ch as i64 - 'j' as i64 + 1), true),
        _ => (0, false),
    };

    let mut i = start + 1;
    let mut num_str = first_digit.to_string();

    while i < chars.len() && chars[i].is_ascii_digit() {
        num_str.push(chars[i]);
        i += 1;
    }

    let value: f64 = num_str.parse().unwrap_or(0.0);
    let value = if negative { -value } else { value };
    (value, i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_jcamp_float() {
        assert!((parse_jcamp_float("123.456") - 123.456).abs() < 0.001);
        assert!((parse_jcamp_float("  -1.5E2  ") - -150.0).abs() < 0.001);
        assert!((parse_jcamp_float("0") - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_jcamp_nucleus() {
        assert_eq!(parse_jcamp_nucleus("1H"), Nucleus::H1);
        assert_eq!(parse_jcamp_nucleus("^1H"), Nucleus::H1);
        assert_eq!(parse_jcamp_nucleus("13C"), Nucleus::C13);
        assert_eq!(parse_jcamp_nucleus("^13C"), Nucleus::C13);
    }

    #[test]
    fn test_sqz_decode() {
        // A = 1, B = 2, etc.
        let (val, _) = read_sqz_number(&['A'], 0);
        assert!((val - 1.0).abs() < 0.001);

        let (val, _) = read_sqz_number(&['E', '5'], 0);
        assert!((val - 55.0).abs() < 0.001);

        // Negative: a = -1
        let (val, _) = read_sqz_number(&['a', '0', '0'], 0);
        assert!((val - -100.0).abs() < 0.001);
    }

    #[test]
    fn test_dif_decode() {
        let (val, _) = read_dif_number(&['J'], 0);
        assert!((val - 1.0).abs() < 0.001);

        let (val, _) = read_dif_number(&['j'], 0);
        assert!((val - -1.0).abs() < 0.001);

        let (val, _) = read_dif_number(&['K', '5'], 0);
        assert!((val - 25.0).abs() < 0.001);
    }

    #[test]
    fn test_decode_plain_numbers() {
        let result = decode_asdf_values("100 200 300");
        assert_eq!(result.len(), 3);
        assert!((result[0] - 100.0).abs() < 0.001);
        assert!((result[1] - 200.0).abs() < 0.001);
        assert!((result[2] - 300.0).abs() < 0.001);
    }

    #[test]
    fn test_decode_sqz_values() {
        // A00 = 100, B00 = 200
        let result = decode_asdf_values("A00B00");
        assert_eq!(result.len(), 2);
        assert!((result[0] - 100.0).abs() < 0.001);
        assert!((result[1] - 200.0).abs() < 0.001);
    }

    #[test]
    fn test_xy_pairs() {
        let header = JcampHeader {
            x_factor: 1.0,
            y_factor: 1.0,
            ..Default::default()
        };
        let lines = vec![
            "1.0, 100.0".to_string(),
            "2.0, 200.0".to_string(),
            "3.0, 300.0".to_string(),
        ];
        let (x, y) = parse_xy_pairs(&lines, &header).unwrap();
        assert_eq!(x.len(), 3);
        assert_eq!(y.len(), 3);
        assert!((x[0] - 1.0).abs() < 0.001);
        assert!((y[2] - 300.0).abs() < 0.001);
    }

    #[test]
    fn test_ntuples_parse() {
        let content = r#"##TITLE= Test NTUPLES Spectrum
##JCAMP-DX= 6.0
##DATA TYPE= NMR SPECTRUM
##DATA CLASS= NTUPLES
##ORIGIN= BRUKER
##.OBSERVE FREQUENCY= 400.13
##.OBSERVE NUCLEUS= ^1H
##NTUPLES= NMR SPECTRUM
##VAR_NAME= FREQUENCY, SPECTRUM/REAL, SPECTRUM/IMAG
##SYMBOL= X, R, I
##VAR_FORM= AFFN, ASDF, ASDF
##VAR_DIM= 5, 5, 5
##UNITS= HZ, ARBITRARY UNITS, ARBITRARY UNITS
##FACTOR= 1.0, 1.0, 1.0
##FIRST= 2000.0, 100, 50
##LAST= 0.0, 500, 250
##PAGE= N=1
##DATA TABLE= (X++(R..R)), XYDATA
2000.0 100 200 300 400 500
##PAGE= N=2
##DATA TABLE= (X++(I..I)), XYDATA
2000.0 50 100 150 200 250
##END NTUPLES= NMR SPECTRUM
##END=
"#;
        let spectrum = parse_jcamp(content, Path::new("test_ntuples.jdx")).unwrap();
        assert_eq!(spectrum.vendor_format, VendorFormat::Jcamp);
        assert_eq!(spectrum.real.len(), 5);
        assert_eq!(spectrum.imag.len(), 5);
        assert!(spectrum.is_frequency_domain);
        assert!((spectrum.axes[0].observe_freq_mhz - 400.13).abs() < 0.01);
        assert!((spectrum.axes[0].spectral_width_hz - 2000.0).abs() < 1.0);
    }

    #[test]
    fn test_full_jcamp_parse() {
        let content = r#"##TITLE= Test Spectrum
##JCAMP-DX= 5.01
##DATA TYPE= NMR SPECTRUM
##XUNITS= PPM
##YUNITS= ARBITRARY UNITS
##.OBSERVE FREQUENCY= 400.13
##.OBSERVE NUCLEUS= ^1H
##FIRSTX= 12.0
##LASTX= -1.0
##NPOINTS= 5
##XFACTOR= 1.0
##YFACTOR= 1.0
##XYDATA= (X++(Y..Y))
12.0 100 200 300 400 500
##END=
"#;
        let spectrum = parse_jcamp(content, Path::new("test.jdx")).unwrap();
        assert_eq!(spectrum.vendor_format, VendorFormat::Jcamp);
        assert_eq!(spectrum.real.len(), 5);
        assert!(spectrum.is_frequency_domain);
        assert!((spectrum.axes[0].observe_freq_mhz - 400.13).abs() < 0.01);
    }
}
