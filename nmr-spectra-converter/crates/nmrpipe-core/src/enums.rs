//! Enumerations for NMRPipe data types, axis modes, and unit codes.
//!
//! Ported from `fdatap.h`, `namelist.h`, and `bruker.h`.

use std::fmt;

// ─── Axis Units (NDUNITS) ───────────────────────────────────────────────────

/// Axis unit codes for NMRPipe dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum AxisUnit {
    Sec = 1,
    Hz = 2,
    Ppm = 3,
    Pts = 4,
}

impl AxisUnit {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            1 => Some(Self::Sec),
            2 => Some(Self::Hz),
            3 => Some(Self::Ppm),
            4 => Some(Self::Pts),
            _ => None,
        }
    }
}

// ─── 2D Plane Type (FD2DPHASE) ─────────────────────────────────────────────

/// 2D plane acquisition mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum Phase2D {
    Magnitude = 0,
    Tppi = 1,
    States = 2,
    Image = 3,
    Array = 4,
}

impl Phase2D {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Magnitude),
            1 => Some(Self::Tppi),
            2 => Some(Self::States),
            3 => Some(Self::Image),
            4 => Some(Self::Array),
            _ => None,
        }
    }
}

impl fmt::Display for Phase2D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Magnitude => write!(f, "Magnitude"),
            Self::Tppi => write!(f, "TPPI"),
            Self::States => write!(f, "States"),
            Self::Image => write!(f, "Image"),
            Self::Array => write!(f, "Array"),
        }
    }
}

// ─── Data Type / Quad Flag (FDQUADFLAG, NDQUADFLAG) ─────────────────────────

/// Quadrature / data type codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum QuadFlag {
    /// Complex (quad detected).
    Complex = 0,
    /// Real (singlature).
    Real = 1,
    /// Pseudo-quad (treated as real on output).
    PseudoQuad = 2,
    /// States-Echo (SE).
    StatesEcho = 3,
    /// Gradient (Rance-Kay / Echo-AntiEcho).
    Gradient = 4,
}

impl QuadFlag {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Complex),
            1 => Some(Self::Real),
            2 => Some(Self::PseudoQuad),
            3 => Some(Self::StatesEcho),
            4 => Some(Self::Gradient),
            _ => None,
        }
    }

    pub fn is_complex(self) -> bool {
        self == Self::Complex
    }
}

impl fmt::Display for QuadFlag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Complex => write!(f, "Complex"),
            Self::Real => write!(f, "Real"),
            Self::PseudoQuad => write!(f, "PseudoQuad"),
            Self::StatesEcho => write!(f, "States-Echo"),
            Self::Gradient => write!(f, "Gradient"),
        }
    }
}

// ─── Sign Alternation (NDAQSIGN) ───────────────────────────────────────────

/// Sign alternation needed for Fourier transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum AqSign {
    None = 0,
    Sequential = 1,
    States = 2,
    NoneNeg = 16,
    SequentialNeg = 17,
    StatesNeg = 18,
}

impl AqSign {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::None),
            1 => Some(Self::Sequential),
            2 => Some(Self::States),
            16 => Some(Self::NoneNeg),
            17 => Some(Self::SequentialNeg),
            18 => Some(Self::StatesNeg),
            _ => None,
        }
    }
}

// ─── DMX Mode ───────────────────────────────────────────────────────────────

/// DMX (digital oversampling) adjustment mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum DmxMode {
    Auto = 0,
    On = 1,
    Off = -1,
}

// ─── Fold Mode ──────────────────────────────────────────────────────────────

/// Folding mode for extracted data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum FoldMode {
    Invert = -1,
    Bad = 0,
    Ordinary = 1,
}

// ─── Dimension Codes ────────────────────────────────────────────────────────

/// Standard dimension identifiers used in NMRPipe.
///
/// The NMRPipe convention uses dimension codes 1-4, corresponding to
/// the historically-named t2, t1, t3, t4 dimensions. The "current"
/// axis codes (X=1, Y=2, Z=3, A=4) are used for accessing parameters
/// relative to the current data transposition state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum DimCode {
    /// Null dimension (used when dimension is irrelevant).
    Null = 0,
    /// Current X-axis (directly detected).
    X = 1,
    /// Current Y-axis.
    Y = 2,
    /// Current Z-axis.
    Z = 3,
    /// Current A-axis (4th dimension).
    A = 4,
}

impl DimCode {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Null),
            1 => Some(Self::X),
            2 => Some(Self::Y),
            3 => Some(Self::Z),
            4 => Some(Self::A),
            _ => None,
        }
    }

    /// Returns the lowercase axis letter.
    pub fn axis_char_lower(self) -> char {
        match self {
            Self::X => 'x',
            Self::Y => 'y',
            Self::Z => 'z',
            Self::A => 'a',
            Self::Null => ' ',
        }
    }

    /// Returns the uppercase axis letter.
    pub fn axis_char_upper(self) -> char {
        match self {
            Self::X => 'X',
            Self::Y => 'Y',
            Self::Z => 'Z',
            Self::A => 'A',
            Self::Null => ' ',
        }
    }
}

// ─── Bruker Acquisition Modes (from bruker.h) ──────────────────────────────

/// Bruker acquisition mode codes from `acqus` parameter `AQ_mod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum BrukerAqMod {
    /// QF mode (single-channel).
    Qf = 0,
    /// Simultaneous quadrature.
    Qsim = 1,
    /// Sequential quadrature.
    Qseq = 2,
    /// Digital quadrature detection.
    Dqd = 3,
}

impl BrukerAqMod {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Qf),
            1 => Some(Self::Qsim),
            2 => Some(Self::Qseq),
            3 => Some(Self::Dqd),
            _ => None,
        }
    }
}

impl fmt::Display for BrukerAqMod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Qf => write!(f, "QF"),
            Self::Qsim => write!(f, "QSIM"),
            Self::Qseq => write!(f, "QSEQ"),
            Self::Dqd => write!(f, "DQD"),
        }
    }
}

/// Bruker spectrometer type for conversion path selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BrukerType {
    /// AMX-style (integer data, no digital oversampling correction during conversion).
    Amx,
    /// DMX-style (apply digital oversampling correction during conversion).
    Dmx,
    /// AM-style (3-byte integer data).
    Am,
}

impl fmt::Display for BrukerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Amx => write!(f, "AMX"),
            Self::Dmx => write!(f, "DMX"),
            Self::Am => write!(f, "AM"),
        }
    }
}

// ─── Bruker ISCALE constants ────────────────────────────────────────────────

/// Scale for Bruker Cray 8-byte integer to float conversion.
pub const BRUKER_ISCALE: i64 = 4_294_967_296; // 2^32
/// Scale for Bruker Cray 3-byte data path.
pub const BRUKER_ISCALE_256: i64 = 1_099_511_627_776; // 2^40

// ─── FT Domain / Acquisition Method ────────────────────────────────────────

/// Acquisition method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum AcqMethod {
    /// FT-based acquisition.
    Ft = 0,
    /// Direct (non-FT) acquisition.
    Direct = 1,
}

/// FT domain type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum FtDomain {
    /// Spectral domain.
    Spectral = 0,
    /// Spatial domain.
    Spatial = 1,
}

// ─── Header validation ─────────────────────────────────────────────────────

/// Result of header validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdrStatus {
    Ok,
    Swapped,
    Bad,
}
