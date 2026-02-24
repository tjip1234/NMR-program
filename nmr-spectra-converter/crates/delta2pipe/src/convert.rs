//! Top-level JEOL Delta → NMRPipe conversion.
//!
//! Reads a complete Delta `.jdf` file (header + parameters + data),
//! converts submatrix layout to sequential, interleaves R/I channels,
//! and produces NMRPipe FDATA header + float data.

use crate::header::*;
use crate::submatrix;
use nmrpipe_core::enums::*;
use nmrpipe_core::fdata::*;
use nmrpipe_io::dfcorrect::DFCorrector;
use std::io::{self, Read};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DeltaError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Invalid Delta header: {0}")]
    InvalidHeader(String),
    #[error("Submatrix conversion error: {0:?}")]
    Smx(submatrix::SmxError),
    #[error("Unsupported: {0}")]
    Unsupported(String),
}

/// Options for Delta → NMRPipe conversion.
#[derive(Debug, Clone)]
pub struct DeltaOptions {
    /// Convert only real (no imaginary) data.
    pub real_only: bool,
    /// Apply digital filter correction.
    pub apply_df: bool,
    /// Digital filter correction value override (None = auto).
    pub df_val: Option<f32>,
    /// Transition ratio override (None = auto).
    pub tr_val: Option<f32>,
    /// Verbose output to stderr.
    pub verbose: bool,
}

impl Default for DeltaOptions {
    fn default() -> Self {
        Self {
            real_only: false,
            apply_df: false,
            df_val: None,
            tr_val: None,
            verbose: false,
        }
    }
}

/// Result of a Delta conversion: NMRPipe header + data planes.
#[derive(Debug)]
pub struct DeltaResult {
    /// The NMRPipe FDATA header.
    pub fdata: Fdata,
    /// Converted data as f32, organized as sequential planes.
    /// For 2D data: one plane, size = x_size * y_size.
    /// For 3D/4D: multiple planes, each x_size * y_size.
    pub planes: Vec<Vec<f32>>,
    /// Digital filter value stored in header (auto-computed, 0 if none).
    pub stored_df_val: f32,
    /// Digital filter value actually applied (0 if correction not applied).
    pub applied_df_val: f32,
    /// Transition ratio.
    pub tr_val: f32,
}

/// Stored parameters extracted from the Delta parameter section.
struct ExtractedParams {
    sw: [f32; JMAXDIM],
    obs: [f32; JMAXDIM],
    car: [f32; JMAXDIM],
    temperature: f32,
    df_flag: bool,
    df_orders: [i32; JMAXDIM],
    df_factors: [i32; JMAXDIM],
    tr_val: f32,
    nmrpipe_info: Option<String>,
}

impl Default for ExtractedParams {
    fn default() -> Self {
        Self {
            sw: [0.0; JMAXDIM],
            obs: [0.0; JMAXDIM],
            car: [0.0; JMAXDIM],
            temperature: 0.0,
            df_flag: false,
            df_orders: [0; JMAXDIM],
            df_factors: [0; JMAXDIM],
            tr_val: 0.0,
            nmrpipe_info: None,
        }
    }
}

/// Convert JEOL Delta data to NMRPipe format.
///
/// Reads the entire input into memory, parses the header and parameters,
/// converts SMX → sequential, interleaves R/I, and returns the result.
pub fn delta_to_pipe<R: Read>(input: &mut R, opts: &DeltaOptions) -> Result<DeltaResult, DeltaError> {
    // Read everything into memory (Delta files are typically < 2 GB)
    let mut all_data = Vec::new();
    input.read_to_end(&mut all_data)?;

    if all_data.len() < DELTA_HDR_SIZE {
        return Err(DeltaError::InvalidHeader("file too small".into()));
    }

    // ── Parse header ────────────────────────────────────────────────────

    // Initial swap: assume big-endian header on little-endian platform
    let swap_hdr = cfg!(target_endian = "little");

    let hdr = DeltaHeader::parse(&all_data[..DELTA_HDR_SIZE], swap_hdr)
        .map_err(|e| DeltaError::InvalidHeader(e.into()))?;

    let swap_data = hdr.needs_data_swap();
    let dim_count = hdr.dim_count as usize;

    if dim_count < 1 || dim_count > 4 {
        return Err(DeltaError::Unsupported(format!(
            "{} dimensions (max 4 supported)",
            dim_count
        )));
    }

    // ── Compute sizes ───────────────────────────────────────────────────

    let mut in_size = [1i32; JMAXDIM];
    let mut out_size = [1i32; JMAXDIM];
    let mut quad_size = [1i32; JMAXDIM];
    let mut channel_count = 1i32;
    let mut total_in_size: i64 = 1;
    let mut total_out_size: i64 = 1;

    for i in 0..dim_count {
        in_size[i] = hdr.size_list[i];
        out_size[i] = 1 + hdr.offset_stop[i] - hdr.offset_start[i];

        total_in_size *= in_size[i] as i64;
        total_out_size *= out_size[i] as i64;

        if hdr.is_quad(i) {
            quad_size[i] = 2;
            channel_count *= 2;
        }
    }

    let word_size_in = hdr.get_word_size(total_in_size, channel_count);
    let word_size_out = 4i32; // always f32 output
    let smx_size = hdr.get_smx_sizes();

    // ── Parse parameters ────────────────────────────────────────────────

    let mut params = ExtractedParams::default();

    if hdr.param_length > 0 && (hdr.param_start as usize) < all_data.len() {
        let parm_buf = &all_data[hdr.param_start as usize..];
        if parm_buf.len() >= 16 {
            let parm_hdr = DeltaParamHeader::parse(parm_buf, swap_data);

            let record_start = 16usize;
            for i in parm_hdr.lo_id..parm_hdr.hi_id {
                let off = record_start + (i - parm_hdr.lo_id) as usize * parm_hdr.parm_size as usize;
                if off + parm_hdr.parm_size as usize > parm_buf.len() {
                    break;
                }
                let rec = &parm_buf[off..off + parm_hdr.parm_size as usize];
                let param = parse_param_record(rec, swap_data);
                store_param(&mut params, &param);
            }
        }
    }

    // ── Compute DF value ────────────────────────────────────────────────

    // Always compute DF value for the header (FDDMXVAL), regardless of
    // whether correction is applied.
    let stored_df_val = if params.df_flag {
        compute_df_val(&params.df_orders, &params.df_factors)
    } else {
        0.0
    };

    // The applied DF value is only used when the user requests correction
    let mut df_val = if opts.apply_df {
        if let Some(v) = opts.df_val {
            v // user override
        } else {
            stored_df_val // auto-computed
        }
    } else {
        0.0 // no correction requested
    };

    let mut tr_val = params.tr_val;

    if let Some(v) = opts.tr_val {
        tr_val = v;
    }

    if !hdr.is_quad(0) || !hdr.is_time_domain(0) {
        df_val = 0.0;
    }
    if opts.real_only {
        df_val = 0.0;
    }

    // ── Read data ───────────────────────────────────────────────────────

    let data_start = hdr.data_start as usize;
    let data_len = hdr.data_length as usize;

    if data_start + data_len > all_data.len() {
        return Err(DeltaError::InvalidHeader(format!(
            "data section exceeds file: start={} len={} file_size={}",
            data_start,
            data_len,
            all_data.len()
        )));
    }

    let mut data_buf = all_data[data_start..data_start + data_len].to_vec();

    // Byte-swap if needed
    if swap_data {
        if word_size_in == 8 {
            nmrpipe_io::byteswap::bswap8(&mut data_buf);
        } else {
            nmrpipe_io::byteswap::bswap4(&mut data_buf);
        }
    }

    // Convert double→float if needed
    let float_data: Vec<f32> =
        if word_size_in == 8 {
            // Convert f64 to f32
            data_buf
                .chunks_exact(8)
                .map(|c| {
                    let d = f64::from_ne_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]);
                    d as f32
                })
                .collect()
        } else {
            data_buf
                .chunks_exact(4)
                .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
                .collect()
        };

    // ── Submatrix → sequential conversion ───────────────────────────────

    let mat_x1 = [1i32; JMAXDIM];
    let mut mat_xn = [1i32; JMAXDIM];
    let mut smx_x1 = [1i32; JMAXDIM];
    let mut smx_xn = [1i32; JMAXDIM];

    for i in 0..dim_count {
        mat_xn[i] = out_size[i];
        smx_x1[i] = 1 + hdr.offset_start[i];
        smx_xn[i] = 1 + hdr.offset_stop[i];
    }

    // Convert each channel's data from SMX to sequential
    let channel_data_in = total_in_size as usize;
    let channel_data_out = total_out_size as usize;

    // Work in byte space for the SMX conversion
    let bytes_per_channel_in = channel_data_in * word_size_out as usize;
    let bytes_per_channel_out = channel_data_out * word_size_out as usize;

    // Convert float_data to bytes for SMX conversion
    let float_bytes: Vec<u8> = float_data
        .iter()
        .flat_map(|f| f.to_ne_bytes())
        .collect();

    let mut out_bytes = vec![0u8; bytes_per_channel_out * channel_count as usize];

    for ch in 0..channel_count as usize {
        let src_start = ch * bytes_per_channel_in;
        let src_end = src_start + bytes_per_channel_in;
        let dest_start = ch * bytes_per_channel_out;

        if src_end <= float_bytes.len() {
            let _ = submatrix::smx2matrix(
                &float_bytes[src_start..src_end],
                &mut out_bytes[dest_start..dest_start + bytes_per_channel_out],
                &out_size[..dim_count],
                Some(&mat_x1[..dim_count]),
                Some(&mat_xn[..dim_count]),
                &in_size[..dim_count],
                Some(&smx_x1[..dim_count]),
                Some(&smx_xn[..dim_count]),
                &smx_size[..dim_count],
                word_size_out,
                dim_count,
            );
        }
    }

    // Convert back to f32
    let mut result_data: Vec<f32> = out_bytes
        .chunks_exact(4)
        .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    // ── Interleave R/I ──────────────────────────────────────────────────

    if channel_count > 1 && !opts.real_only {
        result_data = interleave_ri(
            &result_data,
            &out_size,
            &quad_size,
            &hdr,
            dim_count,
            channel_count,
            total_out_size as usize,
        );
    }

    // ── Digital-filter correction (X dimension, AFTER interleaving) ────
    //
    // The legacy delta2pipe applies DF correction on the interleaved data
    // (post R/I interleaving). After interleaving, each X-dimension row
    // is stored as [R(x_size), I(x_size)]. This preserves the sign
    // convention applied during interleaving (e.g. negated imaginary for
    // reversed axes).

    if df_val > 0.0 && quad_size[0] == 2 && !opts.real_only {
        let old_x = out_size[0] as usize;
        let corrector = DFCorrector::new(
            old_x,
            df_val,
            1,     // skip_tail: legacy strips ceil(df)+1 trailing points
            None,  // max_out
        );
        let new_x = corrector.out_size();

        // After interleaving, each row is [R(old_x), I(old_x)]
        let old_stride = 2 * old_x;
        let row_count = result_data.len() / old_stride;

        let new_stride = 2 * new_x;
        let mut new_data = vec![0.0f32; row_count * new_stride];

        for row in 0..row_count {
            let src_base = row * old_stride;
            let mut r_buf = result_data[src_base..src_base + old_x].to_vec();
            let mut i_buf = result_data[src_base + old_x..src_base + old_stride].to_vec();

            corrector.correct(&mut r_buf, &mut i_buf);

            let dst_base = row * new_stride;
            new_data[dst_base..dst_base + new_x].copy_from_slice(&r_buf[..new_x]);
            new_data[dst_base + new_x..dst_base + new_stride].copy_from_slice(&i_buf[..new_x]);
        }

        result_data = new_data;
        out_size[0] = new_x as i32;
        total_out_size = 1;
        for i in 0..dim_count {
            total_out_size *= out_size[i] as i64;
        }
    }

    // ── Build FDATA header ──────────────────────────────────────────────

    let mut fdata = Fdata::new();
    fdata.init_default();
    fdata.fixfdata();

    fdata.set_dim_count(dim_count as i32);

    // Quad flag for X dimension
    let quad_flag_x = if opts.real_only {
        QuadFlag::Real
    } else if quad_size[0] == 2 {
        QuadFlag::Complex
    } else {
        QuadFlag::Real
    };

    fdata.data[FDQUADFLAG] = quad_flag_x as i32 as f32;
    fdata.data[FD2DPHASE] = hdr.get_aq2d_mode() as f32;

    // Always store auto-computed DF value in header, even when not applying correction.
    // When correction IS applied, legacy convention is to zero out the DMX fields
    // (the correction has been "consumed").
    if df_val > 0.0 {
        // Correction will be applied — leave FDDMXVAL/FDDMXFLAG/FDDELTATR at 0
    } else {
        fdata.data[FDDMXVAL] = stored_df_val;
        fdata.data[FDDELTATR] = tr_val;
    }

    // ── Per-dimension parameters ────────────────────────────────────────

    let _dim_labels = ['X', 'Y', 'Z', 'A'];

    for i in 0..dim_count {
        let size_n;
        let size_t;
        let ft_flag;
        let qf;

        if opts.real_only || quad_size[i] == 1 {
            qf = QuadFlag::Real;
            size_n = out_size[i];
            size_t = out_size[i];
        } else {
            qf = QuadFlag::Complex;
            size_n = if i == 0 { out_size[i] } else { 2 * out_size[i] };
            size_t = out_size[i];
        }

        let size_f;
        if hdr.is_time_domain(i) {
            ft_flag = 0;
            size_f = next_power2(2 * size_t);
        } else {
            ft_flag = 1;
            size_f = size_t;
        }

        let mid = size_f / 2 + 1;
        let sw = get_delta_sw(&hdr, &params, i);
        let obs = get_delta_obs(&hdr, &params, i);
        let car = get_delta_car(&hdr, &params, i, sw, obs);
        let orig = get_delta_orig(&hdr, &params, i, sw, obs, car, out_size[i], size_f, mid);

        let dim_code = (i + 1) as i32;

        if i == 0 {
            fdata.data[FDREALSIZE] = size_t as f32;
        }

        fdata.set_parm(NDSIZE, size_n as f32, dim_code);
        fdata.set_parm(NDAPOD, size_t as f32, dim_code);
        fdata.set_parm(NDTDSIZE, size_t as f32, dim_code);
        fdata.set_parm(NDFTFLAG, ft_flag as f32, dim_code);
        fdata.set_parm(NDQUADFLAG, qf as i32 as f32, dim_code);
        fdata.set_parm(NDSW, sw, dim_code);
        fdata.set_parm(NDOBS, obs, dim_code);
        fdata.set_parm(NDCENTER, mid as f32, dim_code);
        fdata.set_parm(NDCAR, car, dim_code);
        fdata.set_parm(NDORIG, orig, dim_code);

        // Set axis label
        let label = format_label(&hdr.axis_titles, i, dim_count);
        fdata.set_parm_str(NDLABEL, &label, dim_code);
    }

    // ── Set defaults for unused dimensions ──────────────────────────────

    {
        let default_labels = ["", "Y", "Z", "A"]; // indexed by dim_code - 1
        for d in (dim_count + 1)..=4 {
            let dc = d as i32;
            fdata.set_parm(NDSIZE, 1.0, dc);
            fdata.set_parm(NDQUADFLAG, QuadFlag::Real as i32 as f32, dc);
            // Only set OBS=1 and SW=1 for the Y dimension (dim_code=2),
            // matching legacy behavior
            if d == 2 {
                fdata.set_parm(NDOBS, 1.0, dc);
                fdata.set_parm(NDSW, 1.0, dc);
            }
            if d >= 2 && d <= 4 {
                fdata.set_parm_str(NDLABEL, default_labels[d - 1], dc);
            }
        }
    }

    // ── Pipeline params ─────────────────────────────────────────────────

    let plane_count = if opts.real_only {
        out_size[2] * out_size[3]
    } else {
        out_size[2] * quad_size[2] * out_size[3] * quad_size[3]
    };

    if dim_count <= 2 {
        fdata.data[FDFILECOUNT] = plane_count as f32;
        fdata.data[FDPIPEFLAG] = 0.0;
    } else {
        fdata.data[FDFILECOUNT] = 1.0;
        fdata.data[FDPIPEFLAG] = 1.0;
    }

    fdata.data[FDTEMPERATURE] = params.temperature;

    // ── Organize into planes ────────────────────────────────────────────

    let x_out = if opts.real_only {
        out_size[0]
    } else {
        out_size[0] * quad_size[0]
    };

    let y_out = if opts.real_only {
        out_size[1]
    } else {
        out_size[1] * quad_size[1]
    };

    let plane_size = (x_out * y_out) as usize;
    let num_planes = plane_count.max(1) as usize;

    let mut planes = Vec::with_capacity(num_planes);
    for p in 0..num_planes {
        let start = p * plane_size;
        let end = (start + plane_size).min(result_data.len());
        if start < result_data.len() {
            planes.push(result_data[start..end].to_vec());
        } else {
            planes.push(vec![0.0f32; plane_size]);
        }
    }

    // Set FDATA size fields for the final output
    // Note: FDSIZE (index 99) is already set correctly by set_parm(NDSIZE, ..., dim_code=1)
    // as the complex point count. Do NOT overwrite it with x_out (which includes R+I doubling).
    // FDSPECNUM (index 219) is set by set_parm(NDSIZE, ..., dim_code=2) for 2D data,
    // or by the unused dimension loop for 1D data (sets it to 1).

    // Compute min/max
    if !result_data.is_empty() {
        let min_val = result_data.iter().cloned().fold(f32::INFINITY, f32::min);
        let max_val = result_data.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        fdata.set_min_max(min_val, max_val);
    }

    Ok(DeltaResult {
        fdata,
        planes,
        stored_df_val,
        applied_df_val: df_val,
        tr_val,
    })
}

// ─── Parameter extraction ───────────────────────────────────────────────────

fn store_param(params: &mut ExtractedParams, param: &DeltaParam) {
    let dim_chars_u = ['X', 'Y', 'Z', 'A', 'B', 'C', 'D', 'E'];
    let name = param.name.to_uppercase();

    if name == "TEMP_GET" {
        let mut t = param_float_val(param);
        // Check if units are Celsius
        if param.units[0].unit_type == JEOL_SIUNIT_CELSIUS {
            t += 273.0;
        }
        params.temperature = t as f32;
        return;
    }

    if name == "TRANSITION_RATIO" {
        params.tr_val = param_float_val(param) as f32;
        return;
    }

    if name == "DIGITAL_FILTER" {
        if let JVal::Str(s) = &param.val {
            params.df_flag = s.starts_with('T') || s.starts_with('t') || s.starts_with('1');
        }
        return;
    }

    if name == "ORDERS" {
        if let JVal::Str(s) = &param.val {
            for (i, tok) in s.split_whitespace().enumerate() {
                if i >= JMAXDIM {
                    break;
                }
                params.df_orders[i] = tok.parse().unwrap_or(0);
            }
        }
        return;
    }

    if name == "FACTORS" {
        if let JVal::Str(s) = &param.val {
            for (i, tok) in s.split_whitespace().enumerate() {
                if i >= JMAXDIM {
                    break;
                }
                params.df_factors[i] = tok.parse().unwrap_or(0);
            }
        }
        return;
    }

    if name == "NMRPIPE_INFO" {
        if let JVal::Str(s) = &param.val {
            params.nmrpipe_info = Some(s.clone());
        }
        return;
    }

    // X_OFFSET, Y_OFFSET, ... / X_SWEEP, ... / X_FREQ, ...
    for (i, &ch) in dim_chars_u.iter().enumerate() {
        if i >= JMAXDIM {
            break;
        }

        if name == format!("{}_OFFSET", ch) {
            params.car[i] = param_float_val(param) as f32;
            return;
        }
        if name == format!("{}_SWEEP", ch) {
            let mut v = param_float_val(param) as f32;
            if param.unit_scale != 0 {
                v *= 10.0f32.powi(param.unit_scale);
            }
            params.sw[i] = v;
            return;
        }
        if name == format!("{}_FREQ", ch) {
            params.obs[i] = 1.0e-6 * param_float_val(param) as f32;
            return;
        }
    }
}

// ─── Calibration helpers ────────────────────────────────────────────────────

fn get_delta_sw(hdr: &DeltaHeader, params: &ExtractedParams, dim: usize) -> f32 {
    let n = hdr.offset_stop[dim] - hdr.offset_start[dim];

    let s1 = apply_unit_scale(hdr.axis_start[dim], &hdr.unit_list[dim]);
    let s2 = apply_unit_scale(hdr.axis_stop[dim], &hdr.unit_list[dim]);
    let t = (s1 - s2).abs();

    let sw = if hdr.is_time_domain(dim) {
        if params.sw[dim] != 0.0 {
            params.sw[dim]
        } else if t == 0.0 {
            n as f32
        } else {
            n as f32 / t as f32
        }
    } else if hdr.is_ppm(dim) {
        t as f32 * get_delta_obs(hdr, params, dim)
    } else if hdr.is_hz(dim) {
        t as f32
    } else {
        0.0
    };

    let sw = if sw == 0.0 { params.sw[dim] } else { sw };
    let sw = if sw == 0.0 { n as f32 } else { sw };
    let sw = if sw == 0.0 { 1.0 } else { sw };

    sw
}

fn get_delta_obs(hdr: &DeltaHeader, params: &ExtractedParams, dim: usize) -> f32 {
    let obs = hdr.base_freq[dim] as f32;
    let obs = if obs == 0.0 { params.obs[dim] } else { obs };
    if obs == 0.0 {
        1.0
    } else {
        obs
    }
}

fn get_delta_car(
    hdr: &DeltaHeader,
    params: &ExtractedParams,
    dim: usize,
    sw: f32,
    obs: f32,
) -> f32 {
    if hdr.is_time_domain(dim) {
        let z = hdr.zero_point[dim] as f32;
        sw * z / obs
    } else {
        params.car[dim]
    }
}

fn get_delta_orig(
    hdr: &DeltaHeader,
    _params: &ExtractedParams,
    dim: usize,
    sw: f32,
    obs: f32,
    car: f32,
    out_size: i32,
    size_f: i32,
    mid: i32,
) -> f32 {
    if hdr.is_time_domain(dim) {
        // PPM calibration from time-domain info
        obs * car - sw * (size_f - mid) as f32 / size_f as f32
    } else if hdr.is_ppm(dim) {
        let s = apply_unit_scale(hdr.axis_stop[dim], &hdr.unit_list[dim]);
        let n = if out_size == 0 { 1 } else { out_size };
        s as f32 * obs + 0.5 * sw / n as f32
    } else if hdr.is_hz(dim) {
        let s = apply_unit_scale(hdr.axis_stop[dim], &hdr.unit_list[dim]);
        let n = if out_size == 0 { 1 } else { out_size };
        s as f32 + 0.5 * sw / n as f32
    } else {
        0.0
    }
}

// ─── Interleave R/I ─────────────────────────────────────────────────────────

fn interleave_ri(
    data: &[f32],
    out_size: &[i32; JMAXDIM],
    quad_size: &[i32; JMAXDIM],
    hdr: &DeltaHeader,
    dim_count: usize,
    channel_count: i32,
    total_out: usize,
) -> Vec<f32> {
    let mut result = data.to_vec();
    let mut pair_count = channel_count / 2;

    // Adjust reversed flags (unless envelope mode)
    let mut reversed = hdr.reversed;
    if hdr.axis_type[0] != JEOL_AXISTYPE_ENVELOPE {
        for i in 0..dim_count {
            reversed[i] = if reversed[i] != 0 { 0 } else { 1 };
        }
    }

    // Dimension 0 interleave
    if dim_count >= 1 && quad_size[0] == 2 {
        let mut work = vec![0.0f32; result.len()];
        let v_size = out_size[0] as usize;
        let v_count = total_out / v_size;
        let rev_flag = reversed[0] != 0;

        let mut dest_off = 0usize;
        for _ in 0..pair_count {
            let src_r_base = dest_off;
            let src_i_base = src_r_base + total_out;

            for j in 0..v_count {
                let r_off = src_r_base + j * v_size;
                let i_off = src_i_base + j * v_size;

                for k in 0..v_size {
                    work[dest_off + j * 2 * v_size + k] =
                        if r_off + k < result.len() { result[r_off + k] } else { 0.0 };
                }
                for k in 0..v_size {
                    let val = if i_off + k < result.len() { result[i_off + k] } else { 0.0 };
                    work[dest_off + j * 2 * v_size + v_size + k] =
                        if rev_flag { -val } else { val };
                }
            }

            dest_off += total_out * 2;
        }

        result = work;
        pair_count /= 2;
    }

    // Higher dimensions interleave
    for dim in 1..dim_count {
        if quad_size[dim] != 2 {
            continue;
        }

        let mut quad_size_n = 1i64;
        let mut out_size_n = 1i64;
        for i in 0..dim {
            quad_size_n *= quad_size[i] as i64;
            out_size_n *= out_size[i] as i64;
        }

        let mut work = vec![0.0f32; result.len()];
        let v_size = (out_size_n * quad_size_n) as usize;
        let v_count = total_out / out_size_n as usize;
        let rev_flag = reversed[dim] != 0;

        let mut dest_off = 0usize;
        for _ in 0..pair_count {
            let src_r_base = dest_off;
            let src_i_base = src_r_base + total_out * quad_size_n as usize;

            for j in 0..v_count {
                let r_start = src_r_base + j * v_size;
                let i_start = src_i_base + j * v_size;

                for k in 0..v_size {
                    let dest_idx = dest_off + j * 2 * v_size + k;
                    if dest_idx < work.len() && r_start + k < result.len() {
                        work[dest_idx] = result[r_start + k];
                    }
                }
                for k in 0..v_size {
                    let dest_idx = dest_off + j * 2 * v_size + v_size + k;
                    if dest_idx < work.len() && i_start + k < result.len() {
                        let val = result[i_start + k];
                        work[dest_idx] = if rev_flag { -val } else { val };
                    }
                }
            }

            dest_off += total_out * quad_size_n as usize * 2;
        }

        result = work;
        pair_count /= 2;
    }

    result
}

// ─── Digital filter ─────────────────────────────────────────────────────────

fn compute_df_val(orders: &[i32; JMAXDIM], factors: &[i32; JMAXDIM]) -> f32 {
    let stages = orders[0];
    if stages <= 0 {
        return 0.0;
    }

    let mut s = 0.0f64;

    for i in 0..stages as usize {
        let mut p = 1.0f64;
        for j in i..stages as usize - 1 {
            p *= factors[j] as f64;
        }
        if p == 0.0 {
            p = 1.0;
        }
        s += (orders[i + 1] - 1) as f64 / p;
    }

    let last_factor = factors[stages as usize - 1] as f64;
    if last_factor == 0.0 {
        (0.5 * s) as f32
    } else {
        (0.5 * s / last_factor) as f32
    }
}

// ─── Label formatting ───────────────────────────────────────────────────────

fn space2us(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .collect()
}

fn format_label(axis_titles: &[String; JMAXDIM], dim: usize, dim_count: usize) -> String {
    let dim_chars_l = ['x', 'y', 'z', 'a', 'b', 'c', 'd', 'e'];

    let mut lab = space2us(&axis_titles[dim]);

    // Canonical label replacements
    match lab.to_lowercase().as_str() {
        "proton" => lab = "1H".to_string(),
        "nitrogen" => lab = "15N".to_string(),
        "carbon" | "carbon13" => lab = "13C".to_string(),
        "phosphorus" | "phosphorus31" => lab = "31P".to_string(),
        _ => {}
    }

    // For dimension 0, if label is 1H and there's a 15N axis, rename to HN
    if dim == 0 && (lab == "1H" || lab == "H1") {
        for j in 0..dim_count {
            if j == dim {
                continue;
            }
            let other = space2us(&axis_titles[j]).to_lowercase();
            if other == "15n" || other == "n15" || other == "nitrogen" {
                lab = "HN".to_string();
                break;
            }
        }
    }

    // Deduplicate: if same label appears for another dim, add suffix
    for j in 0..dim_count {
        if j == dim {
            continue;
        }
        let other = space2us(&axis_titles[j]);
        if lab.eq_ignore_ascii_case(&other) {
            lab.push(dim_chars_l[dim]);
            break;
        }
    }

    lab
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_df_val() {
        // Example: orders=[3, 44, 46, 0, ...], factors=[2, 2, 0, ...]
        let mut orders = [0i32; JMAXDIM];
        let mut factors = [0i32; JMAXDIM];
        orders[0] = 3;
        orders[1] = 44;
        orders[2] = 46;
        factors[0] = 2;
        factors[1] = 2;

        let val = compute_df_val(&orders, &factors);
        assert!(val > 0.0);
    }

    #[test]
    fn test_format_label() {
        let mut titles: [String; JMAXDIM] = Default::default();
        titles[0] = "Proton".to_string();
        titles[1] = "Nitrogen".to_string();

        let lab = format_label(&titles, 0, 2);
        assert_eq!(lab, "HN"); // 1H with 15N present → HN
    }
}
