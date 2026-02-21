/// NMRPipe format reader/writer
///
/// NMRPipe uses a 2048-byte (512 float32) header followed by spectral data.
/// This module can read NMRPipe .ft1/.ft2/.fid files and also write them.

use byteorder::{LittleEndian, BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Cursor, Seek, SeekFrom};
use std::path::Path;

use super::spectrum::*;

/// NMRPipe header size: 512 float32 values = 2048 bytes
const HEADER_FLOATS: usize = 512;
const HEADER_BYTES: usize = HEADER_FLOATS * 4;

/// Key header indices (0-based, each is a float32 slot)
mod idx {
    pub const FDMAGIC: usize = 0;       // Magic number: 0.0
    pub const FDFLTFORMAT: usize = 1;   // Float format (IEEE = 0xeeeeeeee as f32)
    pub const FDFLTORDER: usize = 2;    // Byte order
    pub const FDDIMCOUNT: usize = 9;    // Number of dimensions
    pub const FDSIZE: usize = 99;       // Number of real points in current dim
    pub const FDREALSIZE: usize = 97;   // Total real data size
    pub const FDSPECNUM: usize = 219;    // Number of spectra (Y size for 2D)
    pub const FDQUADFLAG: usize = 106;  // 0=complex, 1=real
    pub const FDF2SW: usize = 100;      // Spectral width F2 (Hz)
    pub const FDF2OBS: usize = 119;     // Observe freq F2 (MHz)
    pub const FDF2ORIG: usize = 101;    // Origin F2 (Hz)
    pub const FDF2FTFLAG: usize = 220;  // 1=freq domain, 0=time domain
    pub const FDF2LABEL: usize = 16;    // F2 label (4 chars as float)
    pub const FDF1SW: usize = 229;      // Spectral width F1 (Hz)
    pub const FDF1OBS: usize = 218;     // Observe freq F1 (MHz)
    pub const FDF1ORIG: usize = 249;    // Origin F1 (Hz)
    pub const FDF1FTFLAG: usize = 222;  // 1=freq domain F1
    pub const FDF1LABEL: usize = 18;    // F1 label
    pub const FDPIPEFLAG: usize = 57;   // Pipe mode flag
    pub const FDTRANSPOSED: usize = 221; // 1=transposed
}

/// Read an NMRPipe format file
pub fn read_nmrpipe_file(path: &Path) -> io::Result<SpectrumData> {
    let data = std::fs::read(path)?;
    if data.len() < HEADER_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "File too small for NMRPipe format",
        ));
    }

    // Parse header as 512 little-endian floats first
    let mut header = vec![0.0f32; HEADER_FLOATS];
    let mut cursor = Cursor::new(&data[..HEADER_BYTES]);
    for h in header.iter_mut() {
        *h = cursor.read_f32::<LittleEndian>()?;
    }

    // NMRPipe byte order check: FDFLTORDER (index 2) should be ≈ 2.345
    // If it's not, the file is big-endian and we re-read.
    let order_val = header[idx::FDFLTORDER];
    let is_big_endian = (order_val - 2.345).abs() > 0.01;
    if is_big_endian {
        cursor.seek(SeekFrom::Start(0))?;
        for h in header.iter_mut() {
            *h = cursor.read_f32::<BigEndian>()?;
        }
    }

    let ndim = header[idx::FDDIMCOUNT] as usize;
    let npts_x = header[idx::FDSIZE] as usize;
    let npts_y = if ndim >= 2 {
        header[idx::FDSPECNUM] as usize
    } else {
        1
    };
    let is_complex = header[idx::FDQUADFLAG] as i32 == 0;
    let is_freq_domain = header[idx::FDF2FTFLAG] as i32 == 1;

    let sw_x = header[idx::FDF2SW] as f64;
    let obs_x = header[idx::FDF2OBS] as f64;
    let orig_x = header[idx::FDF2ORIG] as f64;

    // NMRPipe convention: FDF2ORIG is the frequency (Hz) of the RIGHT edge
    // (lowest ppm). The LEFT edge (highest ppm, index 0) is at ORIG + SW.
    // reference_ppm = ppm of index 0 = (ORIG + SW) / OBS
    let ref_ppm_x = if obs_x > 0.0 {
        (orig_x + sw_x) / obs_x
    } else {
        0.0
    };

    let filename = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let experiment_type = detect_experiment_type(&filename);
    let dimensionality = if ndim >= 2 && npts_y > 1 {
        Dimensionality::TwoD
    } else {
        Dimensionality::OneD
    };

    let mut spectrum = SpectrumData {
        source_path: path.to_path_buf(),
        vendor_format: VendorFormat::NMRPipe,
        experiment_type,
        dimensionality: dimensionality.clone(),
        sample_name: filename,
        axes: Vec::new(),
        real: Vec::new(),
        imag: Vec::new(),
        data_2d: Vec::new(),
        is_frequency_domain: is_freq_domain,
        nmrpipe_path: Some(path.to_path_buf()),
    };

    let axis_x = AxisParams {
        nucleus: Nucleus::H1,
        num_points: npts_x,
        spectral_width_hz: sw_x,
        observe_freq_mhz: obs_x,
        reference_ppm: ref_ppm_x,
        label: "F2".to_string(),
    };
    spectrum.axes.push(axis_x);

    if dimensionality == Dimensionality::TwoD {
        let sw_y = header[idx::FDF1SW] as f64;
        let obs_y = header[idx::FDF1OBS] as f64;
        let orig_y = header[idx::FDF1ORIG] as f64;
        let ref_ppm_y = if obs_y > 0.0 { (orig_y + sw_y) / obs_y } else { 0.0 };

        let axis_y = AxisParams {
            nucleus: Nucleus::C13,
            num_points: npts_y,
            spectral_width_hz: sw_y,
            observe_freq_mhz: obs_y,
            reference_ppm: ref_ppm_y,
            label: "F1".to_string(),
        };
        spectrum.axes.push(axis_y);
    }

    // Read spectral data (after header)
    let data_slice = &data[HEADER_BYTES..];
    let num_floats = data_slice.len() / 4;
    let mut cursor = Cursor::new(data_slice);
    let mut values = Vec::with_capacity(num_floats);

    for _ in 0..num_floats {
        let v = if is_big_endian {
            cursor.read_f32::<BigEndian>()?
        } else {
            cursor.read_f32::<LittleEndian>()?
        };
        values.push(v as f64);
    }

    if dimensionality == Dimensionality::OneD {
        if is_complex {
            let is_pipe_mode = header[idx::FDPIPEFLAG] as i32 == 1;
            if is_pipe_mode {
                // Pipe/stream mode: interleaved R, I, R, I, ...
                spectrum.real = values.iter().step_by(2).copied().collect();
                spectrum.imag = values.iter().skip(1).step_by(2).copied().collect();
            } else {
                // Standard file mode: sequential blocks R...R then I...I
                let n = npts_x.min(values.len());
                spectrum.real = values[..n].to_vec();
                if values.len() >= 2 * n {
                    spectrum.imag = values[n..2 * n].to_vec();
                }
            }
        } else {
            spectrum.real = values;
        }
        if let Some(ax) = spectrum.axes.first_mut() {
            ax.num_points = spectrum.real.len();
        }
    } else {
        let points_per_row = if npts_y > 0 {
            values.len() / npts_y
        } else {
            values.len()
        };
        for row in 0..npts_y {
            let start = row * points_per_row;
            let end = (start + points_per_row).min(values.len());
            if start < values.len() {
                spectrum.data_2d.push(values[start..end].to_vec());
            }
        }
    }

    Ok(spectrum)
}

/// Write spectrum data to NMRPipe format
pub fn write_nmrpipe_file(spectrum: &SpectrumData, path: &Path) -> io::Result<()> {
    let mut file = std::fs::File::create(path)?;

    // Create header
    let mut header = vec![0.0f32; HEADER_FLOATS];

    // Magic / format
    header[idx::FDMAGIC] = 0.0;
    header[idx::FDFLTFORMAT] = f32::from_bits(0x4f6eeeef); // NMRPipe IEEE marker
    header[idx::FDFLTORDER] = 2.345f32; // Little-endian byte order marker

    // Dimensions
    let ndim = if spectrum.is_2d() { 2.0 } else { 1.0 };
    header[idx::FDDIMCOUNT] = ndim;

    // X axis
    let npts = spectrum.real.len().max(1);
    header[idx::FDSIZE] = npts as f32;
    header[idx::FDREALSIZE] = npts as f32;

    if let Some(ax) = spectrum.axes.first() {
        header[idx::FDF2SW] = ax.spectral_width_hz as f32;
        header[idx::FDF2OBS] = ax.observe_freq_mhz as f32;
        header[idx::FDF2ORIG] = (ax.reference_ppm * ax.observe_freq_mhz) as f32;
    }

    header[idx::FDF2FTFLAG] = if spectrum.is_frequency_domain {
        1.0
    } else {
        0.0
    };
    header[idx::FDQUADFLAG] = if spectrum.imag.is_empty() {
        1.0
    } else {
        0.0
    };

    if spectrum.is_2d() {
        let ny = spectrum.data_2d.len();
        header[idx::FDSPECNUM] = ny as f32;
        if let Some(ax) = spectrum.axes.get(1) {
            header[idx::FDF1SW] = ax.spectral_width_hz as f32;
            header[idx::FDF1OBS] = ax.observe_freq_mhz as f32;
            header[idx::FDF1ORIG] = (ax.reference_ppm * ax.observe_freq_mhz) as f32;
        }
    }

    // Write header
    for &h in &header {
        file.write_f32::<LittleEndian>(h)?;
    }

    // Write data
    if spectrum.is_2d() {
        for row in &spectrum.data_2d {
            for &v in row {
                file.write_f32::<LittleEndian>(v as f32)?;
            }
        }
    } else if !spectrum.imag.is_empty() {
        // Interleaved complex
        for i in 0..spectrum.real.len() {
            file.write_f32::<LittleEndian>(spectrum.real[i] as f32)?;
            let im = spectrum.imag.get(i).copied().unwrap_or(0.0);
            file.write_f32::<LittleEndian>(im as f32)?;
        }
    } else {
        for &v in &spectrum.real {
            file.write_f32::<LittleEndian>(v as f32)?;
        }
    }

    Ok(())
}

/// Read a 2D NMRPipe dataset stored as a series of plane files.
///
/// delta2pipe and other NMRPipe tools store 2D data as numbered files:
/// `name001.fid`, `name002.fid`, etc. Each file has the same NMRPipe header
/// (with full 2D metadata) and contains one plane of data.
///
/// This function reads all plane files, extracts metadata from the first,
/// and assembles the full 2D data matrix.
pub fn read_nmrpipe_2d_planes(plane_files: &[std::path::PathBuf]) -> io::Result<SpectrumData> {
    if plane_files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "No plane files provided for 2D read",
        ));
    }

    // Read the first file to get metadata
    let first_data = std::fs::read(&plane_files[0])?;
    if first_data.len() < HEADER_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "First plane file too small for NMRPipe format",
        ));
    }

    // Parse header
    let mut header = vec![0.0f32; HEADER_FLOATS];
    let mut cursor = Cursor::new(&first_data[..HEADER_BYTES]);
    for h in header.iter_mut() {
        *h = cursor.read_f32::<LittleEndian>()?;
    }

    let order_val = header[idx::FDFLTORDER];
    let is_big_endian = (order_val - 2.345).abs() > 0.01;
    if is_big_endian {
        cursor.seek(SeekFrom::Start(0))?;
        for h in header.iter_mut() {
            *h = cursor.read_f32::<BigEndian>()?;
        }
    }

    let npts_x = header[idx::FDSIZE] as usize;
    let is_complex_x = header[idx::FDQUADFLAG] as i32 == 0;
    let is_freq_domain = header[idx::FDF2FTFLAG] as i32 == 1;

    let sw_x = header[idx::FDF2SW] as f64;
    let obs_x = header[idx::FDF2OBS] as f64;
    let orig_x = header[idx::FDF2ORIG] as f64;
    let ref_ppm_x = if obs_x > 0.0 { orig_x / obs_x } else { 0.0 };

    let sw_y = header[idx::FDF1SW] as f64;
    let obs_y = header[idx::FDF1OBS] as f64;
    let orig_y = header[idx::FDF1ORIG] as f64;
    let ref_ppm_y = if obs_y > 0.0 { orig_y / obs_y } else { 0.0 };

    // Detect nucleus labels from header
    let label_f2 = decode_label(&header, idx::FDF2LABEL);
    let label_f1 = decode_label(&header, idx::FDF1LABEL);

    let nucleus_x = nucleus_from_label(&label_f2);
    let nucleus_y = nucleus_from_label(&label_f1);

    let npts_y = plane_files.len();

    let filename = plane_files[0]
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let experiment_type = super::spectrum::detect_experiment_type(&filename);

    let mut spectrum = SpectrumData {
        source_path: plane_files[0].to_path_buf(),
        vendor_format: super::spectrum::VendorFormat::NMRPipe,
        experiment_type,
        dimensionality: super::spectrum::Dimensionality::TwoD,
        sample_name: filename,
        axes: vec![
            super::spectrum::AxisParams {
                nucleus: nucleus_x,
                num_points: npts_x,
                spectral_width_hz: sw_x,
                observe_freq_mhz: obs_x,
                reference_ppm: ref_ppm_x,
                label: if label_f2.is_empty() { "F2".to_string() } else { label_f2 },
            },
            super::spectrum::AxisParams {
                nucleus: nucleus_y,
                num_points: npts_y,
                spectral_width_hz: sw_y,
                observe_freq_mhz: obs_y,
                reference_ppm: ref_ppm_y,
                label: if label_f1.is_empty() { "F1".to_string() } else { label_f1 },
            },
        ],
        real: Vec::new(),
        imag: Vec::new(),
        data_2d: Vec::new(),
        is_frequency_domain: is_freq_domain,
        nmrpipe_path: Some(plane_files[0].to_path_buf()),
    };

    // Read data from each plane file
    for plane_path in plane_files {
        let plane_data = std::fs::read(plane_path)?;
        if plane_data.len() < HEADER_BYTES {
            log::warn!("Skipping short plane file: {}", plane_path.display());
            continue;
        }

        let data_slice = &plane_data[HEADER_BYTES..];
        let num_floats = data_slice.len() / 4;
        let mut pcursor = Cursor::new(data_slice);
        let mut row = Vec::with_capacity(num_floats);

        for _ in 0..num_floats {
            let v = if is_big_endian {
                pcursor.read_f32::<BigEndian>()?
            } else {
                pcursor.read_f32::<LittleEndian>()?
            };
            row.push(v as f64);
        }

        // For complex data, take only the real part (every other value)
        if is_complex_x && row.len() >= npts_x * 2 {
            let real_row: Vec<f64> = row.iter().step_by(2).copied().collect();
            spectrum.data_2d.push(real_row);
        } else {
            spectrum.data_2d.push(row);
        }
    }

    log::info!(
        "Read 2D NMRPipe series: {} planes × {} points",
        spectrum.data_2d.len(),
        npts_x,
    );

    Ok(spectrum)
}

/// Decode a 4-char label stored in two consecutive float slots
fn decode_label(header: &[f32], start_idx: usize) -> String {
    if start_idx + 1 >= header.len() {
        return String::new();
    }
    let bytes1 = header[start_idx].to_bits().to_be_bytes();
    let bytes2 = header[start_idx + 1].to_bits().to_be_bytes();
    let combined: Vec<u8> = bytes1
        .iter()
        .chain(bytes2.iter())
        .copied()
        .filter(|&b| b.is_ascii_alphanumeric() || b == b' ')
        .collect();
    String::from_utf8_lossy(&combined).trim().to_string()
}

/// Map a label like "1H" or "13C" to a Nucleus enum
fn nucleus_from_label(label: &str) -> super::spectrum::Nucleus {
    let upper = label.to_uppercase();
    if upper.contains("1H") || upper == "H1" || upper == "H" {
        super::spectrum::Nucleus::H1
    } else if upper.contains("13C") || upper == "C13" || upper == "C" {
        super::spectrum::Nucleus::C13
    } else if upper.contains("15N") || upper == "N15" || upper == "N" {
        super::spectrum::Nucleus::N15
    } else if upper.contains("19F") || upper == "F19" {
        super::spectrum::Nucleus::F19
    } else if upper.contains("31P") || upper == "P31" {
        super::spectrum::Nucleus::P31
    } else if label.is_empty() {
        super::spectrum::Nucleus::H1
    } else {
        super::spectrum::Nucleus::Other(label.to_string())
    }
}
