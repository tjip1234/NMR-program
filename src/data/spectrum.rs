use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Supported vendor formats for NMR data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VendorFormat {
    Bruker,
    Varian,
    Jeol,
    NMRPipe,
    Unknown,
}

impl std::fmt::Display for VendorFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VendorFormat::Bruker => write!(f, "Bruker"),
            VendorFormat::Varian => write!(f, "Varian/Agilent"),
            VendorFormat::Jeol => write!(f, "JEOL Delta"),
            VendorFormat::NMRPipe => write!(f, "NMRPipe"),
            VendorFormat::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Nucleus type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Nucleus {
    H1,
    C13,
    N15,
    F19,
    P31,
    Other(String),
}

impl std::fmt::Display for Nucleus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Nucleus::H1 => write!(f, "1H"),
            Nucleus::C13 => write!(f, "13C"),
            Nucleus::N15 => write!(f, "15N"),
            Nucleus::F19 => write!(f, "19F"),
            Nucleus::P31 => write!(f, "31P"),
            Nucleus::Other(s) => write!(f, "{}", s),
        }
    }
}

/// Experiment dimensionality
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Dimensionality {
    OneD,
    TwoD,
}

/// Experiment type detected from filename/metadata
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExperimentType {
    Proton,
    Carbon,
    Dept135,
    Cosy,
    Hsqc,
    Hmbc,
    Other(String),
}

impl std::fmt::Display for ExperimentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExperimentType::Proton => write!(f, "1H"),
            ExperimentType::Carbon => write!(f, "13C"),
            ExperimentType::Dept135 => write!(f, "DEPT-135"),
            ExperimentType::Cosy => write!(f, "COSY"),
            ExperimentType::Hsqc => write!(f, "HSQC"),
            ExperimentType::Hmbc => write!(f, "HMBC"),
            ExperimentType::Other(s) => write!(f, "{}", s),
        }
    }
}

/// Axis parameters for a spectral dimension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisParams {
    pub nucleus: Nucleus,
    pub num_points: usize,
    pub spectral_width_hz: f64,
    pub observe_freq_mhz: f64,
    pub reference_ppm: f64,
    pub label: String,
}

impl Default for AxisParams {
    fn default() -> Self {
        Self {
            nucleus: Nucleus::H1,
            num_points: 0,
            spectral_width_hz: 0.0,
            observe_freq_mhz: 400.0,
            reference_ppm: 0.0,
            label: String::new(),
        }
    }
}

impl AxisParams {
    /// Convert a point index to ppm
    pub fn index_to_ppm(&self, index: usize) -> f64 {
        if self.num_points == 0 || self.observe_freq_mhz == 0.0 {
            return 0.0;
        }
        let sw_ppm = self.spectral_width_hz / self.observe_freq_mhz;
        let frac = index as f64 / self.num_points as f64;
        // NMRPipe convention: reference_ppm is the ppm of the first point (index 0).
        // Spectrum runs from reference_ppm down to (reference_ppm - sw_ppm).
        self.reference_ppm - frac * sw_ppm
    }

    /// Generate a ppm scale array
    pub fn ppm_scale(&self) -> Vec<f64> {
        (0..self.num_points)
            .map(|i| self.index_to_ppm(i))
            .collect()
    }
}

/// Spectrum data container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumData {
    pub source_path: PathBuf,
    pub vendor_format: VendorFormat,
    pub experiment_type: ExperimentType,
    pub dimensionality: Dimensionality,
    pub sample_name: String,
    /// Axis parameters (1 for 1D, 2 for 2D)
    pub axes: Vec<AxisParams>,
    /// Real data for 1D spectrum
    pub real: Vec<f64>,
    /// Imaginary data for 1D spectrum (if present)
    pub imag: Vec<f64>,
    /// 2D data stored as flattened row-major (f2 is fast axis)
    pub data_2d: Vec<Vec<f64>>,
    /// Whether the data has been Fourier-transformed
    pub is_frequency_domain: bool,
    /// NMRPipe format file path after conversion
    pub nmrpipe_path: Option<PathBuf>,
}

impl Default for SpectrumData {
    fn default() -> Self {
        Self {
            source_path: PathBuf::new(),
            vendor_format: VendorFormat::Unknown,
            experiment_type: ExperimentType::Other("Unknown".into()),
            dimensionality: Dimensionality::OneD,
            sample_name: String::new(),
            axes: vec![AxisParams::default()],
            real: Vec::new(),
            imag: Vec::new(),
            data_2d: Vec::new(),
            is_frequency_domain: false,
            nmrpipe_path: None,
        }
    }
}

impl SpectrumData {
    /// Get the maximum absolute value for normalization
    pub fn max_abs(&self) -> f64 {
        self.real
            .iter()
            .map(|v| v.abs())
            .fold(0.0f64, f64::max)
    }

    /// Check if this is a 2D experiment
    pub fn is_2d(&self) -> bool {
        self.dimensionality == Dimensionality::TwoD
    }
}

/// Detect experiment type from filename
pub fn detect_experiment_type(filename: &str) -> ExperimentType {
    let upper = filename.to_uppercase();
    if upper.contains("PROTON") || upper.contains("1H") {
        ExperimentType::Proton
    } else if upper.contains("135") || upper.contains("DEPT") {
        ExperimentType::Dept135
    } else if upper.contains("HSQC") {
        ExperimentType::Hsqc
    } else if upper.contains("HMBC") {
        ExperimentType::Hmbc
    } else if upper.contains("COSY") {
        ExperimentType::Cosy
    } else if upper.contains("CARBON") || upper.contains("13C") {
        ExperimentType::Carbon
    } else {
        ExperimentType::Other(filename.to_string())
    }
}

/// Detect if an experiment is 2D from the experiment type
pub fn experiment_dimensionality(exp: &ExperimentType) -> Dimensionality {
    match exp {
        ExperimentType::Cosy | ExperimentType::Hsqc | ExperimentType::Hmbc => Dimensionality::TwoD,
        _ => Dimensionality::OneD,
    }
}
