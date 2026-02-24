//! Top-level Bruker → NMRPipe conversion.
//!
//! Reads a Bruker SER/FID binary file together with pre‑populated FDATA
//! parameters (from acqus/acqu2s, typically supplied via command-line args),
//! converts each 2D plane chunk by chunk, and produces NMRPipe output.

use crate::dmx;
use crate::ser2fid;
use nmrpipe_core::fdata::*;
use nmrpipe_core::params::*;
use std::io::{self, Read};
use thiserror::Error;

// ─── Types ──────────────────────────────────────────────────────────────────

/// Bruker instrument type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrukerType {
    /// AMX format: standard 4-byte integer data.
    Amx,
    /// DMX format: 4-byte integer + digital oversampling correction.
    Dmx,
    /// AM format: 3-byte integer data.
    Am,
}

#[derive(Error, Debug)]
pub enum BrukerError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Invalid configuration: {0}")]
    Config(String),
    #[error("DMX error: {0}")]
    Dmx(String),
}

/// Options for Bruker → NMRPipe conversion.
///
/// These mirror the command-line flags from bruk2pipe.
#[derive(Debug, Clone)]
pub struct BrukerOptions {
    /// Bruker data type (AMX / DMX / AM).
    pub bruk_type: BrukerType,
    /// Pre-populated FDATA header (from parseHdr / acqus parsing).
    pub fdata: Fdata,
    /// Byte-swap flag.
    pub swap: bool,
    /// Perform int-to-float conversion (default true).
    pub i2f: bool,
    /// Input word size in bytes (4 for AMX/DMX, 3 for AM, 8 for double).
    pub word_size: usize,
    /// Byte offset to skip at start of input.
    pub byte_offset: usize,
    /// Bad point threshold (0 = no clipping).
    pub bad_thresh: f32,
    /// Extract valid points only (reduce x-axis to NDAPOD size).
    pub ext_flag: bool,
    /// DMX digital-filter parameters.
    pub decim: i32,
    pub dspfvs: i32,
    pub grpdly: f32,
    pub aq_mod: i32,
    /// First/last point scale for DMX correction.
    pub fc: f32,
    /// Skip size for DMX head correction.
    pub skip_size: i32,
    /// Temporary zero-fill for DMX speed.
    pub zf_flag: bool,
    /// Verbose output.
    pub verbose: bool,
}

impl Default for BrukerOptions {
    fn default() -> Self {
        Self {
            bruk_type: BrukerType::Amx,
            fdata: Fdata::new(),
            swap: cfg!(target_endian = "little"),
            i2f: true,
            word_size: 4,
            byte_offset: 0,
            bad_thresh: 8_000_000.0,
            ext_flag: false,
            decim: 0,
            dspfvs: 10,
            grpdly: 0.0,
            aq_mod: AQ_MOD_QSIM,
            fc: 1.0,
            skip_size: 4,
            zf_flag: true,
            verbose: false,
        }
    }
}

/// Bruker AQ_MOD constants.
pub const AQ_MOD_QF: i32 = 0;
pub const AQ_MOD_QSIM: i32 = 1;
pub const AQ_MOD_QSEQ: i32 = 2;
pub const AQ_MOD_DQD: i32 = 3;

/// Result of a Bruker conversion.
#[derive(Debug)]
pub struct BrukerResult {
    /// The NMRPipe FDATA header.
    pub fdata: Fdata,
    /// Converted data planes. For pipe-mode output (dim > 2), each
    /// entry is a single 2D plane. For file mode, a single entry
    /// contains the entire dataset.
    pub planes: Vec<Vec<f32>>,
}

// ─── Main conversion ────────────────────────────────────────────────────────

/// Convert Bruker SER/FID data to NMRPipe format.
///
/// The caller must have already populated `opts.fdata` with the correct
/// NMRPipe header parameters (dimension count, sizes, SW, OBS, CAR, labels,
/// quad flags, etc.).  This function handles the byte-level conversion and
/// data reorganisation.
pub fn bruker_to_pipe<R: Read>(input: &mut R, opts: &BrukerOptions) -> Result<BrukerResult, BrukerError> {
    let mut fdata = opts.fdata.clone();
    fdata.fixfdata();

    let dim_count = fdata.dim_count();
    let word_size = if opts.bruk_type == BrukerType::Am { 3 } else { opts.word_size };

    // ── Sizes ───────────────────────────────────────────────────────────

    let quad_type = fdata.data[FDQUADFLAG] as i32;
    let x_size_raw = if dim_count >= 1 { fdata.get_parm(NDSIZE, CUR_XDIM) as i32 } else { 1 };
    let y_size = if dim_count >= 2 { fdata.get_parm(NDSIZE, CUR_YDIM) as i32 } else { 1 };
    let z_size = if dim_count >= 3 { fdata.get_parm(NDSIZE, CUR_ZDIM) as i32 } else { 1 };
    let a_size = if dim_count >= 4 { fdata.get_parm(NDSIZE, CUR_ADIM) as i32 } else { 1 };

    let x_ext_size_raw = fdata.get_parm(NDAPOD, CUR_XDIM) as i32;

    let (quad_state, x_size, x_mid, x_freq_size) = match quad_type {
        0 => {
            // Complex
            let xs = x_size_raw / 2;
            fdata.set_parm(NDSIZE, xs as f32, CUR_XDIM);
            (2, xs, 1 + xs / 2, xs)
        }
        1 => {
            // Real
            (1, x_size_raw, 1 + x_size_raw / 4, x_size_raw / 2)
        }
        2 => {
            // Pseudo-quad → convert to real
            fdata.set_parm(NDQUADFLAG, 1.0, CUR_XDIM);
            fdata.data[FDQUADFLAG] = 1.0;
            (1, x_size_raw, 1 + x_size_raw / 4, x_size_raw / 2)
        }
        _ => return Err(BrukerError::Config(format!("bad quad type {}", quad_type))),
    };

    let mut x_ext_size = x_ext_size_raw;
    let plane_count = z_size * a_size;

    // ── Pipeline flags ──────────────────────────────────────────────────

    if dim_count > 2 {
        fdata.data[FDPIPEFLAG] = 1.0;
        fdata.data[FDFILECOUNT] = 1.0;
    } else {
        fdata.data[FDPIPEFLAG] = 0.0;
        fdata.data[FDFILECOUNT] = plane_count as f32;
    }
    fdata.data[FDCUBEFLAG] = 0.0;

    // ── Center / origin ─────────────────────────────────────────────────

    let y_freq_size = if y_size / 2 != 0 { y_size / 2 } else { 1 };
    let z_freq_size = if z_size / 2 != 0 { z_size / 2 } else { 1 };
    let a_freq_size = if a_size / 2 != 0 { a_size / 2 } else { 1 };

    let x_mid_f = x_mid as f32;
    let y_mid = (1 + y_freq_size / 2) as f32;
    let z_mid = (1 + z_freq_size / 2) as f32;
    let a_mid = (1 + a_freq_size / 2) as f32;

    fdata.set_parm(NDCENTER, x_mid_f, CUR_XDIM);
    fdata.set_parm(NDCENTER, y_mid, CUR_YDIM);
    fdata.set_parm(NDCENTER, z_mid, CUR_ZDIM);
    fdata.set_parm(NDCENTER, a_mid, CUR_ADIM);

    // Compute origins: orig = obs*car - sw*(freqSize - mid)/freqSize
    let compute_orig = |dim_code: i32, freq_size: i32, mid: f32| -> f32 {
        let obs = fdata.get_parm(NDOBS, dim_code);
        let sw = fdata.get_parm(NDSW, dim_code);
        let car = fdata.get_parm(NDCAR, dim_code);
        obs * car - sw * (freq_size as f32 - mid) / freq_size as f32
    };

    let x_orig = compute_orig(CUR_XDIM, x_freq_size, x_mid_f);
    let y_orig = compute_orig(CUR_YDIM, y_freq_size, y_mid);
    let z_orig = compute_orig(CUR_ZDIM, z_freq_size, z_mid);
    let a_orig = compute_orig(CUR_ADIM, a_freq_size, a_mid);

    fdata.set_parm(NDORIG, x_orig, CUR_XDIM);
    fdata.set_parm(NDORIG, y_orig, CUR_YDIM);
    fdata.set_parm(NDORIG, z_orig, CUR_ZDIM);
    fdata.set_parm(NDORIG, a_orig, CUR_ADIM);

    // ── DMX setup ───────────────────────────────────────────────────────

    let dmx_state: Option<dmx::DmxState>;
    if opts.bruk_type == BrukerType::Dmx {
        if quad_state != 2 {
            return Err(BrukerError::Config("DMX conversion must be complex".into()));
        }
        let state = dmx::dmx_init(
            x_size as usize,
            x_ext_size as usize,
            opts.skip_size,
            opts.decim,
            opts.dspfvs,
            opts.grpdly,
            opts.aq_mod,
        )
        .map_err(|e| BrukerError::Dmx(e.to_string()))?;

        x_ext_size = state.out_size() as i32;
        fdata.set_parm(NDAPOD, x_ext_size as f32, CUR_XDIM);
        dmx_state = Some(state);
    } else if opts.decim > 0 {
        let val = dmx::get_dmx_val(opts.aq_mod, opts.decim, opts.dspfvs, opts.grpdly);
        fdata.data[FDDMXVAL] = val;
        fdata.data[FDDMXFLAG] = 0.0;
        dmx_state = None;
    } else {
        dmx_state = None;
    }

    if opts.ext_flag {
        fdata.set_parm(NDSIZE, x_ext_size as f32, CUR_XDIM);
    }

    // ── TDSIZE ──────────────────────────────────────────────────────────

    fdata.set_parm(NDTDSIZE, fdata.get_parm(NDAPOD, CUR_XDIM), CUR_XDIM);
    fdata.set_parm(NDTDSIZE, fdata.get_parm(NDAPOD, CUR_YDIM), CUR_YDIM);
    fdata.set_parm(NDTDSIZE, fdata.get_parm(NDAPOD, CUR_ZDIM), CUR_ZDIM);
    fdata.set_parm(NDTDSIZE, fdata.get_parm(NDAPOD, CUR_ADIM), CUR_ADIM);

    // ── FDATA size fields ───────────────────────────────────────────────

    let out_x = if opts.ext_flag && x_ext_size < x_size {
        x_ext_size
    } else {
        x_size
    };

    fdata.data[FDSIZE] = out_x as f32;
    fdata.data[FDSPECNUM] = y_size as f32;
    fdata.data[FDREALSIZE] = fdata.get_parm(NDAPOD, CUR_XDIM);

    // ── Read and convert ────────────────────────────────────────────────

    // Read entire input into memory
    let mut raw = Vec::new();
    input.read_to_end(&mut raw)?;

    // Skip byte offset
    let data_start = opts.byte_offset.min(raw.len());
    let raw = &raw[data_start..];

    let pts_per_slice = x_size as usize * quad_state as usize;
    let bytes_per_slice = word_size * pts_per_slice;
    let out_pts_per_slice = if opts.ext_flag && (x_ext_size as usize) < (x_size as usize) {
        x_ext_size as usize * quad_state as usize
    } else {
        pts_per_slice
    };

    let mut planes: Vec<Vec<f32>> = Vec::with_capacity(plane_count as usize);
    let mut raw_offset = 0usize;

    for _plane_idx in 0..plane_count {
        let plane_pts = out_pts_per_slice * y_size as usize;
        let mut plane_data = vec![0.0f32; plane_pts];

        // Process y_size slices for this plane
        let mut dest_offset = 0usize;

        for _row in 0..y_size {
            if raw_offset + bytes_per_slice > raw.len() {
                break;
            }

            let chunk = &raw[raw_offset..raw_offset + bytes_per_slice];
            let mut row_buf = vec![0.0f32; pts_per_slice];

            ser2fid::ser2fid2d(
                chunk,
                &mut row_buf,
                x_size as usize,
                1,
                quad_state as usize,
                quad_type,
                word_size,
                opts.swap,
                opts.i2f,
                opts.bad_thresh,
            );

            raw_offset += bytes_per_slice;

            // DMX correction
            if opts.bruk_type == BrukerType::Dmx && quad_state == 2 {
                if let Some(ref state) = dmx_state {
                    dmx::dmx2fid2d(state, &mut row_buf, x_size as usize, 1);
                }
            }

            // Extract valid points if needed
            if opts.ext_flag && (x_ext_size as usize) < (x_size as usize) {
                ser2fid::x_ext_2d(
                    &mut row_buf,
                    x_size as usize,
                    x_ext_size as usize,
                    quad_state as usize,
                );
            }

            let copy_len = out_pts_per_slice.min(row_buf.len());
            if dest_offset + copy_len <= plane_data.len() {
                plane_data[dest_offset..dest_offset + copy_len]
                    .copy_from_slice(&row_buf[..copy_len]);
            }
            dest_offset += out_pts_per_slice;
        }

        planes.push(plane_data);
    }

    // Compute global min/max
    let mut g_min = f32::INFINITY;
    let mut g_max = f32::NEG_INFINITY;
    for plane in &planes {
        for &v in plane {
            if v < g_min { g_min = v; }
            if v > g_max { g_max = v; }
        }
    }
    if g_min.is_finite() && g_max.is_finite() {
        fdata.set_min_max(g_min, g_max);
    }

    Ok(BrukerResult { fdata, planes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_options() {
        let opts = BrukerOptions::default();
        assert_eq!(opts.bruk_type, BrukerType::Amx);
        assert_eq!(opts.word_size, 4);
        assert!(opts.i2f);
    }
}
