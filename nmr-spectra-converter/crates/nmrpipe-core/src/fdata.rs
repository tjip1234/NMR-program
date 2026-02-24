//! NMRPipe FDATA header: 512-float array defining spectral data parameters.
//!
//! Ported from `fdatap.h`. The header is 2048 bytes (512 × 4-byte floats).
//! Parameter access uses named constants and the generalized ND parameter system.

use crate::enums::*;
use byteorder::{ByteOrder, NativeEndian};
use std::fmt;

// ─── Constants ──────────────────────────────────────────────────────────────

/// Number of 4-byte float values in the FDATA header.
pub const FDATA_SIZE: usize = 512;
/// Header size in bytes.
pub const FDATA_BYTES: usize = FDATA_SIZE * 4; // 2048
/// IEEE floating-point format constant.
pub const FD_IEEE_CONS: u32 = 0xEEEEEEEE;
/// VAX floating-point format constant.
pub const FD_VAX_CONS: u32 = 0x11111111;
/// Byte-order test constant.
pub const FD_ORDER_CONS: f32 = 2.345;
/// Equivalent for zero in some legacy contexts.
pub const ZERO_EQUIV: f32 = -666.0;
/// Maximum number of points in a given dimension (limited by f32 precision).
pub const MAX_NMR_SIZE: i32 = 16_777_216;

// ─── General parameter locations ────────────────────────────────────────────

pub const FDMAGIC: usize = 0;
pub const FDFLTFORMAT: usize = 1;
pub const FDFLTORDER: usize = 2;
pub const FDID: usize = 3;

pub const FDSIZE: usize = 99;
pub const FDREALSIZE: usize = 97;
pub const FDSPECNUM: usize = 219;
pub const FDQUADFLAG: usize = 106;
pub const FD2DPHASE: usize = 256;

// ─── Dimension order ────────────────────────────────────────────────────────

pub const FDTRANSPOSED: usize = 221;
pub const FDDIMCOUNT: usize = 9;
pub const FDDIMORDER: usize = 24;
pub const FDDIMORDER1: usize = 24;
pub const FDDIMORDER2: usize = 25;
pub const FDDIMORDER3: usize = 26;
pub const FDDIMORDER4: usize = 27;

pub const FDNUSDIM: usize = 45;

// ─── Pipeline / stream parameters ──────────────────────────────────────────

pub const FDPIPEFLAG: usize = 57;
pub const FDCUBEFLAG: usize = 447;
pub const FDPIPECOUNT: usize = 75;
pub const FDSLICECOUNT0: usize = 443;
pub const FDSLICECOUNT1: usize = 446;
pub const FDFILECOUNT: usize = 442;

pub const FDTHREADCOUNT: usize = 444;
pub const FDTHREADID: usize = 445;

pub const FDFIRSTPLANE: usize = 77;
pub const FDLASTPLANE: usize = 78;
pub const FDPARTITION: usize = 65;

pub const FDPLANELOC: usize = 14;

// ─── Min/Max ────────────────────────────────────────────────────────────────

pub const FDMAX: usize = 247;
pub const FDMIN: usize = 248;
pub const FDSCALEFLAG: usize = 250;
pub const FDDISPMAX: usize = 251;
pub const FDDISPMIN: usize = 252;
pub const FDPTHRESH: usize = 253;
pub const FDNTHRESH: usize = 254;

// ─── User ───────────────────────────────────────────────────────────────────

pub const FDUSER1: usize = 70;
pub const FDUSER2: usize = 71;
pub const FDUSER3: usize = 72;
pub const FDUSER4: usize = 73;
pub const FDUSER5: usize = 74;
pub const FDUSER6: usize = 76;

// ─── Footer / block locations ───────────────────────────────────────────────

pub const FDLASTBLOCK: usize = 359;
pub const FDCONTBLOCK: usize = 360;
pub const FDBASEBLOCK: usize = 361;
pub const FDPEAKBLOCK: usize = 362;
pub const FDBMAPBLOCK: usize = 363;
pub const FDHISTBLOCK: usize = 364;
pub const FD1DBLOCK: usize = 365;

// ─── Date/time ──────────────────────────────────────────────────────────────

pub const FDMONTH: usize = 294;
pub const FDDAY: usize = 295;
pub const FDYEAR: usize = 296;
pub const FDHOURS: usize = 283;
pub const FDMINS: usize = 284;
pub const FDSECS: usize = 285;

// ─── Miscellaneous ──────────────────────────────────────────────────────────

pub const FDMCFLAG: usize = 135;
pub const FDNOISE: usize = 153;
pub const FDRANK: usize = 180;
pub const FDTEMPERATURE: usize = 157;
pub const FDPRESSURE: usize = 158;
pub const FD2DVIRGIN: usize = 399;
pub const FDTAU: usize = 199;
pub const FDDOMINFO: usize = 266;
pub const FDMETHINFO: usize = 267;

pub const FDSCALE: usize = 478;
pub const FDSCORE: usize = 370;
pub const FDSCANS: usize = 371;

pub const FDSRCNAME: usize = 286;
pub const FDUSERNAME: usize = 290;
pub const FDOPERNAME: usize = 464;
pub const FDTITLE: usize = 297;
pub const FDCOMMENT: usize = 312;

// ─── DMX ────────────────────────────────────────────────────────────────────

pub const FDDMXVAL: usize = 40;
pub const FDDMXFLAG: usize = 41;
pub const FDDELTATR: usize = 42;

// ─── F2 (dimension 2, often direct detect / X-axis) ─────────────────────────

pub const FDF2LABEL: usize = 16;
pub const FDF2APOD: usize = 95;
pub const FDF2SW: usize = 100;
pub const FDF2OBS: usize = 119;
pub const FDF2OBSMID: usize = 378;
pub const FDF2ORIG: usize = 101;
pub const FDF2UNITS: usize = 152;
pub const FDF2QUADFLAG: usize = 56;
pub const FDF2FTFLAG: usize = 220;
pub const FDF2AQSIGN: usize = 64;
pub const FDF2CAR: usize = 66;
pub const FDF2CENTER: usize = 79;
pub const FDF2OFFPPM: usize = 480;
pub const FDF2P0: usize = 109;
pub const FDF2P1: usize = 110;
pub const FDF2APODCODE: usize = 413;
pub const FDF2APODQ1: usize = 415;
pub const FDF2APODQ2: usize = 416;
pub const FDF2APODQ3: usize = 417;
pub const FDF2LB: usize = 111;
pub const FDF2GB: usize = 374;
pub const FDF2GOFF: usize = 382;
pub const FDF2C1: usize = 418;
pub const FDF2APODDF: usize = 419;
pub const FDF2ZF: usize = 108;
pub const FDF2X1: usize = 257;
pub const FDF2XN: usize = 258;
pub const FDF2FTSIZE: usize = 96;
pub const FDF2TDSIZE: usize = 386;

// ─── F1 (dimension 1, first indirect / Y-axis) ─────────────────────────────

pub const FDF1LABEL: usize = 18;
pub const FDF1APOD: usize = 428;
pub const FDF1SW: usize = 229;
pub const FDF1OBS: usize = 218;
pub const FDF1OBSMID: usize = 379;
pub const FDF1ORIG: usize = 249;
pub const FDF1UNITS: usize = 234;
pub const FDF1FTFLAG: usize = 222;
pub const FDF1AQSIGN: usize = 475;
pub const FDF1QUADFLAG: usize = 55;
pub const FDF1CAR: usize = 67;
pub const FDF1CENTER: usize = 80;
pub const FDF1OFFPPM: usize = 481;
pub const FDF1P0: usize = 245;
pub const FDF1P1: usize = 246;
pub const FDF1APODCODE: usize = 414;
pub const FDF1APODQ1: usize = 420;
pub const FDF1APODQ2: usize = 421;
pub const FDF1APODQ3: usize = 422;
pub const FDF1LB: usize = 243;
pub const FDF1GB: usize = 375;
pub const FDF1GOFF: usize = 383;
pub const FDF1C1: usize = 423;
pub const FDF1ZF: usize = 437;
pub const FDF1X1: usize = 259;
pub const FDF1XN: usize = 260;
pub const FDF1FTSIZE: usize = 98;
pub const FDF1TDSIZE: usize = 387;

// ─── F3 (Z-axis) ───────────────────────────────────────────────────────────

pub const FDF3LABEL: usize = 20;
pub const FDF3APOD: usize = 50;
pub const FDF3OBS: usize = 10;
pub const FDF3OBSMID: usize = 380;
pub const FDF3SW: usize = 11;
pub const FDF3ORIG: usize = 12;
pub const FDF3FTFLAG: usize = 13;
pub const FDF3AQSIGN: usize = 476;
pub const FDF3SIZE: usize = 15;
pub const FDF3QUADFLAG: usize = 51;
pub const FDF3UNITS: usize = 58;
pub const FDF3P0: usize = 60;
pub const FDF3P1: usize = 61;
pub const FDF3CAR: usize = 68;
pub const FDF3CENTER: usize = 81;
pub const FDF3OFFPPM: usize = 482;
pub const FDF3APODCODE: usize = 400;
pub const FDF3APODQ1: usize = 401;
pub const FDF3APODQ2: usize = 402;
pub const FDF3APODQ3: usize = 403;
pub const FDF3LB: usize = 372;
pub const FDF3GB: usize = 376;
pub const FDF3GOFF: usize = 384;
pub const FDF3C1: usize = 404;
pub const FDF3ZF: usize = 438;
pub const FDF3X1: usize = 261;
pub const FDF3XN: usize = 262;
pub const FDF3FTSIZE: usize = 200;
pub const FDF3TDSIZE: usize = 388;

// ─── F4 (A-axis) ───────────────────────────────────────────────────────────

pub const FDF4LABEL: usize = 22;
pub const FDF4APOD: usize = 53;
pub const FDF4OBS: usize = 28;
pub const FDF4OBSMID: usize = 381;
pub const FDF4SW: usize = 29;
pub const FDF4ORIG: usize = 30;
pub const FDF4FTFLAG: usize = 31;
pub const FDF4AQSIGN: usize = 477;
pub const FDF4SIZE: usize = 32;
pub const FDF4QUADFLAG: usize = 54;
pub const FDF4UNITS: usize = 59;
pub const FDF4P0: usize = 62;
pub const FDF4P1: usize = 63;
pub const FDF4CAR: usize = 69;
pub const FDF4CENTER: usize = 82;
pub const FDF4OFFPPM: usize = 483;
pub const FDF4APODCODE: usize = 405;
pub const FDF4APODQ1: usize = 406;
pub const FDF4APODQ2: usize = 407;
pub const FDF4APODQ3: usize = 408;
pub const FDF4LB: usize = 373;
pub const FDF4GB: usize = 377;
pub const FDF4GOFF: usize = 385;
pub const FDF4C1: usize = 409;
pub const FDF4ZF: usize = 439;
pub const FDF4X1: usize = 263;
pub const FDF4XN: usize = 264;
pub const FDF4FTSIZE: usize = 201;
pub const FDF4TDSIZE: usize = 389;

// ─── Label sizes ────────────────────────────────────────────────────────────

pub const SIZE_NDLABEL: usize = 8;
pub const SIZE_F2LABEL: usize = 8;
pub const SIZE_F1LABEL: usize = 8;
pub const SIZE_F3LABEL: usize = 8;
pub const SIZE_F4LABEL: usize = 8;
pub const SIZE_SRCNAME: usize = 16;
pub const SIZE_USERNAME: usize = 16;
pub const SIZE_OPERNAME: usize = 32;
pub const SIZE_COMMENT: usize = 160;
pub const SIZE_TITLE: usize = 60;

// ─── Generalized ND parameters ─────────────────────────────────────────────

pub const NDPARM: i32 = 1000;

pub const NDSIZE: i32 = 1 + NDPARM;
pub const NDAPOD: i32 = 2 + NDPARM;
pub const NDSW: i32 = 3 + NDPARM;
pub const NDORIG: i32 = 4 + NDPARM;
pub const NDOBS: i32 = 5 + NDPARM;
pub const NDFTFLAG: i32 = 6 + NDPARM;
pub const NDQUADFLAG: i32 = 7 + NDPARM;
pub const NDUNITS: i32 = 8 + NDPARM;
pub const NDLABEL: i32 = 9 + NDPARM;
pub const NDLABEL1: i32 = 9 + NDPARM;
pub const NDLABEL2: i32 = 10 + NDPARM;
pub const NDP0: i32 = 11 + NDPARM;
pub const NDP1: i32 = 12 + NDPARM;
pub const NDCAR: i32 = 13 + NDPARM;
pub const NDCENTER: i32 = 14 + NDPARM;
pub const NDAQSIGN: i32 = 15 + NDPARM;
pub const NDAPODCODE: i32 = 16 + NDPARM;
pub const NDAPODQ1: i32 = 17 + NDPARM;
pub const NDAPODQ2: i32 = 18 + NDPARM;
pub const NDAPODQ3: i32 = 19 + NDPARM;
pub const NDC1: i32 = 20 + NDPARM;
pub const NDZF: i32 = 21 + NDPARM;
pub const NDX1: i32 = 22 + NDPARM;
pub const NDXN: i32 = 23 + NDPARM;
pub const NDOFFPPM: i32 = 24 + NDPARM;
pub const NDFTSIZE: i32 = 25 + NDPARM;
pub const NDTDSIZE: i32 = 26 + NDPARM;
pub const NDACQMETHOD: i32 = 27 + NDPARM;
pub const NDFTDOMAIN: i32 = 28 + NDPARM;
pub const NDLB: i32 = 29 + NDPARM;
pub const NDGB: i32 = 30 + NDPARM;
pub const NDGOFF: i32 = 31 + NDPARM;
pub const NDOBSMID: i32 = 32 + NDPARM;
pub const MAX_NDPARM: i32 = 32;

// ─── Dimension location tables ─────────────────────────────────────────────

/// Maps generalized ND parameter codes to per-dimension FDATA locations.
/// Index: [nd_parm_offset][dim_f2=0, dim_f1=1, dim_f3=2, dim_f4=3]
///
/// nd_parm_offset = parm_code - NDPARM - 1 (0-based index into the ND params)
const ND_LOC_TABLE: [[usize; 4]; MAX_NDPARM as usize] = [
    // NDSIZE
    [FDSIZE, FDSPECNUM, FDF3SIZE, FDF4SIZE],
    // NDAPOD
    [FDF2APOD, FDF1APOD, FDF3APOD, FDF4APOD],
    // NDSW
    [FDF2SW, FDF1SW, FDF3SW, FDF4SW],
    // NDORIG
    [FDF2ORIG, FDF1ORIG, FDF3ORIG, FDF4ORIG],
    // NDOBS
    [FDF2OBS, FDF1OBS, FDF3OBS, FDF4OBS],
    // NDFTFLAG
    [FDF2FTFLAG, FDF1FTFLAG, FDF3FTFLAG, FDF4FTFLAG],
    // NDQUADFLAG
    [FDF2QUADFLAG, FDF1QUADFLAG, FDF3QUADFLAG, FDF4QUADFLAG],
    // NDUNITS
    [FDF2UNITS, FDF1UNITS, FDF3UNITS, FDF4UNITS],
    // NDLABEL1
    [FDF2LABEL, FDF1LABEL, FDF3LABEL, FDF4LABEL],
    // NDLABEL2
    [FDF2LABEL + 1, FDF1LABEL + 1, FDF3LABEL + 1, FDF4LABEL + 1],
    // NDP0
    [FDF2P0, FDF1P0, FDF3P0, FDF4P0],
    // NDP1
    [FDF2P1, FDF1P1, FDF3P1, FDF4P1],
    // NDCAR
    [FDF2CAR, FDF1CAR, FDF3CAR, FDF4CAR],
    // NDCENTER
    [FDF2CENTER, FDF1CENTER, FDF3CENTER, FDF4CENTER],
    // NDAQSIGN
    [FDF2AQSIGN, FDF1AQSIGN, FDF3AQSIGN, FDF4AQSIGN],
    // NDAPODCODE
    [FDF2APODCODE, FDF1APODCODE, FDF3APODCODE, FDF4APODCODE],
    // NDAPODQ1
    [FDF2APODQ1, FDF1APODQ1, FDF3APODQ1, FDF4APODQ1],
    // NDAPODQ2
    [FDF2APODQ2, FDF1APODQ2, FDF3APODQ2, FDF4APODQ2],
    // NDAPODQ3
    [FDF2APODQ3, FDF1APODQ3, FDF3APODQ3, FDF4APODQ3],
    // NDC1
    [FDF2C1, FDF1C1, FDF3C1, FDF4C1],
    // NDZF
    [FDF2ZF, FDF1ZF, FDF3ZF, FDF4ZF],
    // NDX1
    [FDF2X1, FDF1X1, FDF3X1, FDF4X1],
    // NDXN
    [FDF2XN, FDF1XN, FDF3XN, FDF4XN],
    // NDOFFPPM
    [FDF2OFFPPM, FDF1OFFPPM, FDF3OFFPPM, FDF4OFFPPM],
    // NDFTSIZE
    [FDF2FTSIZE, FDF1FTSIZE, FDF3FTSIZE, FDF4FTSIZE],
    // NDTDSIZE
    [FDF2TDSIZE, FDF1TDSIZE, FDF3TDSIZE, FDF4TDSIZE],
    // NDACQMETHOD (no per-dim location in original, map to FDDOMINFO area)
    [FDMETHINFO, FDMETHINFO, FDMETHINFO, FDMETHINFO],
    // NDFTDOMAIN
    [FDDOMINFO, FDDOMINFO, FDDOMINFO, FDDOMINFO],
    // NDLB
    [FDF2LB, FDF1LB, FDF3LB, FDF4LB],
    // NDGB
    [FDF2GB, FDF1GB, FDF3GB, FDF4GB],
    // NDGOFF
    [FDF2GOFF, FDF1GOFF, FDF3GOFF, FDF4GOFF],
    // NDOBSMID
    [FDF2OBSMID, FDF1OBSMID, FDF3OBSMID, FDF4OBSMID],
];

// ─── FDATA structure ────────────────────────────────────────────────────────

/// The NMRPipe 512-float header array.
///
/// This wraps the raw `[f32; 512]` array and provides typed accessor methods
/// that mirror the C `getParm()` / `setParm()` API, including generalized
/// ND parameter access with dimension remapping based on transposition state.
#[derive(Clone)]
pub struct Fdata {
    pub data: [f32; FDATA_SIZE],
}

impl Default for Fdata {
    fn default() -> Self {
        Self::new()
    }
}

impl Fdata {
    /// Create a zeroed FDATA header.
    pub fn new() -> Self {
        Self {
            data: [0.0f32; FDATA_SIZE],
        }
    }

    /// Initialize with NMRPipe defaults (float format, byte order, dimension order 2 1 3 4).
    pub fn init_default(&mut self) {
        self.data.fill(0.0);
        // Match C convention: (float)FD_IEEE_CONS is integer→float numeric conversion
        self.data[FDFLTFORMAT] = FD_IEEE_CONS as f32;
        self.data[FDFLTORDER] = FD_ORDER_CONS;
        // Default dimension order: 2 1 3 4
        self.data[FDDIMORDER1] = 2.0;
        self.data[FDDIMORDER2] = 1.0;
        self.data[FDDIMORDER3] = 3.0;
        self.data[FDDIMORDER4] = 4.0;
        self.data[FD2DVIRGIN] = 1.0;
        self.data[FDDIMCOUNT as usize] = 1.0;
    }

    /// Get the actual FDATA location for a generalized ND parameter and dimension code.
    ///
    /// `dim_code`: 1-based dimension code (1=X, 2=Y, 3=Z, 4=A), or 0 for null.
    ///
    /// The dimension order array maps current axes (X,Y,Z,A) to physical
    /// dimensions (F2, F1, F3, F4). For example, dimension order [2,1,3,4]
    /// means X-axis stores dimension 2, Y-axis stores dimension 1, etc.
    fn get_loc(&self, parm: i32, dim_code: i32) -> Option<usize> {
        if parm > NDPARM {
            // Generalized ND parameter
            let nd_idx = (parm - NDPARM - 1) as usize;
            if nd_idx >= MAX_NDPARM as usize {
                return None;
            }

            if dim_code < 1 || dim_code > 4 {
                return None;
            }

            // Map current dim (1-4) to physical dim via dimension order
            let phys_dim = self.data[FDDIMORDER + (dim_code as usize - 1)] as i32;

            // Physical dimensions: 2->F2(idx 0), 1->F1(idx 1), 3->F3(idx 2), 4->F4(idx 3)
            let table_idx = match phys_dim {
                2 => 0,
                1 => 1,
                3 => 2,
                4 => 3,
                _ => return None,
            };

            Some(ND_LOC_TABLE[nd_idx][table_idx])
        } else {
            // Direct location
            let loc = parm as usize;
            if loc < FDATA_SIZE {
                Some(loc)
            } else {
                None
            }
        }
    }

    /// Get a parameter value by code and dimension.
    ///
    /// `dim_code`: 1=CUR_XDIM, 2=CUR_YDIM, 3=CUR_ZDIM, 4=CUR_ADIM, 0=NULL_DIM
    pub fn get_parm(&self, parm: i32, dim_code: i32) -> f32 {
        self.get_loc(parm, dim_code)
            .map(|loc| self.data[loc])
            .unwrap_or(0.0)
    }

    /// Get a parameter as integer.
    pub fn get_parm_i(&self, parm: i32, dim_code: i32) -> i32 {
        self.get_parm(parm, dim_code) as i32
    }

    /// Set a parameter value by code and dimension.
    pub fn set_parm(&mut self, parm: i32, val: f32, dim_code: i32) {
        if let Some(loc) = self.get_loc(parm, dim_code) {
            self.data[loc] = val;
        }
    }

    /// Get dimension count.
    pub fn dim_count(&self) -> i32 {
        self.data[FDDIMCOUNT as usize] as i32
    }

    /// Set dimension count.
    pub fn set_dim_count(&mut self, n: i32) {
        self.data[FDDIMCOUNT as usize] = n as f32;
    }

    // ─── Text packing ───────────────────────────────────────────────────

    /// Pack a string into FDATA locations (4 bytes per float slot).
    pub fn txt2flt(text: &str, dest: &mut [f32], max_bytes: usize) {
        let bytes = text.as_bytes();
        let n = bytes.len().min(max_bytes);

        // Zero out destination as u8 slice
        let dest_bytes: &mut [u8] = unsafe {
            std::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u8, dest.len() * 4)
        };
        for b in dest_bytes.iter_mut() {
            *b = 0;
        }

        // Copy text bytes
        dest_bytes[..n].copy_from_slice(&bytes[..n]);
    }

    /// Unpack a string from FDATA locations.
    pub fn flt2txt(src: &[f32], max_bytes: usize) -> String {
        let src_bytes: &[u8] =
            unsafe { std::slice::from_raw_parts(src.as_ptr() as *const u8, src.len() * 4) };
        let n = src_bytes.len().min(max_bytes);
        let s: Vec<u8> = src_bytes[..n]
            .iter()
            .copied()
            .take_while(|&b| b != 0)
            .collect();
        String::from_utf8_lossy(&s).to_string()
    }

    /// Set a label string for a dimension.
    ///
    /// `dim_code`: 1-4 (X, Y, Z, A)
    pub fn set_parm_str(&mut self, parm: i32, text: &str, dim_code: i32) {
        if let Some(loc) = self.get_loc(parm, dim_code) {
            let max_bytes = SIZE_NDLABEL;
            let end = (loc + (max_bytes + 3) / 4).min(FDATA_SIZE);
            Self::txt2flt(text, &mut self.data[loc..end], max_bytes);
        }
    }

    /// Get a label string for a dimension.
    pub fn get_parm_str(&self, parm: i32, dim_code: i32) -> String {
        if let Some(loc) = self.get_loc(parm, dim_code) {
            let max_bytes = SIZE_NDLABEL;
            let end = (loc + (max_bytes + 3) / 4).min(FDATA_SIZE);
            Self::flt2txt(&self.data[loc..end], max_bytes)
        } else {
            String::new()
        }
    }

    // ─── Header I/O ─────────────────────────────────────────────────────

    /// Serialize to bytes (native endian).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = vec![0u8; FDATA_BYTES];
        for (i, &val) in self.data.iter().enumerate() {
            NativeEndian::write_f32(&mut buf[i * 4..(i + 1) * 4], val);
        }
        buf
    }

    /// Deserialize from bytes, auto-detecting byte order.
    pub fn from_bytes(buf: &[u8]) -> Result<(Self, HdrStatus), &'static str> {
        if buf.len() < FDATA_BYTES {
            return Err("buffer too small for FDATA header");
        }

        // Try native endian first
        let mut fdata = Self::new();
        for i in 0..FDATA_SIZE {
            fdata.data[i] = NativeEndian::read_f32(&buf[i * 4..(i + 1) * 4]);
        }

        // Check byte order constant
        let order_val = fdata.data[FDFLTORDER];
        if (order_val - FD_ORDER_CONS).abs() < 0.001 {
            return Ok((fdata, HdrStatus::Ok));
        }

        // Try swapped endian
        let mut fdata_swap = Self::new();
        for i in 0..FDATA_SIZE {
            let b = &buf[i * 4..(i + 1) * 4];
            let swapped = [b[3], b[2], b[1], b[0]];
            fdata_swap.data[i] = f32::from_ne_bytes(swapped);
        }

        let order_val_swap = fdata_swap.data[FDFLTORDER];
        if (order_val_swap - FD_ORDER_CONS).abs() < 0.001 {
            return Ok((fdata_swap, HdrStatus::Swapped));
        }

        Err("invalid FDATA header: byte order check failed")
    }

    /// Test if this is a valid FDATA header.
    pub fn test_header(&self) -> HdrStatus {
        let order_val = self.data[FDFLTORDER];
        if (order_val - FD_ORDER_CONS).abs() < 0.001 {
            HdrStatus::Ok
        } else {
            HdrStatus::Bad
        }
    }

    /// Apply fixfdata adjustments (ensure dimension order is valid, etc.).
    pub fn fixfdata(&mut self) {
        // Ensure valid dimension order
        if self.data[FDDIMORDER1] == 0.0 {
            self.data[FDDIMORDER1] = 2.0;
        }
        if self.data[FDDIMORDER2] == 0.0 {
            self.data[FDDIMORDER2] = 1.0;
        }
        if self.data[FDDIMORDER3] == 0.0 {
            self.data[FDDIMORDER3] = 3.0;
        }
        if self.data[FDDIMORDER4] == 0.0 {
            self.data[FDDIMORDER4] = 4.0;
        }

        // Ensure dimension count is at least 1
        if self.data[FDDIMCOUNT as usize] < 1.0 {
            self.data[FDDIMCOUNT as usize] = 1.0;
        }

        // Set float format (match C: (float)FD_IEEE_CONS)
        self.data[FDFLTFORMAT] = FD_IEEE_CONS as f32;
        self.data[FDFLTORDER] = FD_ORDER_CONS;
    }
}

impl fmt::Debug for Fdata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fdata")
            .field("dim_count", &self.dim_count())
            .field(
                "dim_order",
                &[
                    self.data[FDDIMORDER1] as i32,
                    self.data[FDDIMORDER2] as i32,
                    self.data[FDDIMORDER3] as i32,
                    self.data[FDDIMORDER4] as i32,
                ],
            )
            .field("x_size", &self.get_parm(NDSIZE, 1))
            .field("y_size", &self.get_parm(NDSIZE, 2))
            .field("z_size", &self.get_parm(NDSIZE, 3))
            .field("a_size", &self.get_parm(NDSIZE, 4))
            .finish()
    }
}

/// Compute the next power of 2 >= n.
pub fn next_power2(n: i32) -> i32 {
    let mut v = n;
    if v <= 0 {
        return 1;
    }
    v -= 1;
    v |= v >> 1;
    v |= v >> 2;
    v |= v >> 4;
    v |= v >> 8;
    v |= v >> 16;
    v + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_header() {
        let mut fd = Fdata::new();
        fd.init_default();
        assert_eq!(fd.dim_count(), 1);
        assert_eq!(fd.data[FDDIMORDER1] as i32, 2);
        assert_eq!(fd.data[FDDIMORDER2] as i32, 1);
    }

    #[test]
    fn test_get_set_parm() {
        let mut fd = Fdata::new();
        fd.init_default();
        fd.set_dim_count(2);
        fd.set_parm(NDSIZE, 1024.0, 1); // X-axis size
        fd.set_parm(NDSIZE, 256.0, 2); // Y-axis size
        assert_eq!(fd.get_parm(NDSIZE, 1) as i32, 1024);
        assert_eq!(fd.get_parm(NDSIZE, 2) as i32, 256);
    }

    #[test]
    fn test_label() {
        let mut fd = Fdata::new();
        fd.init_default();
        fd.set_parm_str(NDLABEL, "1H", 1);
        let lab = fd.get_parm_str(NDLABEL, 1);
        assert_eq!(lab, "1H");
    }

    #[test]
    fn test_next_power2() {
        assert_eq!(next_power2(1), 1);
        assert_eq!(next_power2(3), 4);
        assert_eq!(next_power2(1024), 1024);
        assert_eq!(next_power2(1025), 2048);
    }

    #[test]
    fn test_roundtrip_bytes() {
        let mut fd = Fdata::new();
        fd.init_default();
        fd.set_parm(NDSW, 10000.0, 1);
        let bytes = fd.to_bytes();
        let (fd2, status) = Fdata::from_bytes(&bytes).unwrap();
        assert_eq!(status, HdrStatus::Ok);
        assert!((fd2.get_parm(NDSW, 1) - 10000.0).abs() < 0.01);
    }
}
