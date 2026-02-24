//! DMX digital‑filter correction for Bruker digital oversampling.
//!
//! Bruker digital receivers (DMX, Avance, etc.) apply a multi-stage
//! decimation filter that introduces a deterministic group delay in the
//! FID.  The delay is reported as DECIM / DSPFVS parameters (or directly
//! as GRPDLY on newer TopSpin systems).
//!
//! The correction removes the group delay via an FFT-based fractional
//! circular shift, then extracts the valid region.

use nmrpipe_io::dfcorrect::DFCorrector;

/// Look up the expected group delay value for a given DECIM / DSPFVS / GRPDLY
/// combination.
///
/// Returns the digital‑filter correction value to store in `FDDMXVAL`.
/// When `grpdly > 0`, it is used directly (newer Bruker TopSpin convention).
pub fn get_dmx_val(_aq_mod: i32, decim: i32, dspfvs: i32, grpdly: f32) -> f32 {
    if grpdly > 0.0 {
        return grpdly;
    }

    // Look‑up table from NMRPipe's dspList for common DECIM / DSPFVS pairs.
    // This is the publicly documented table used by many NMR packages.
    let table: &[(i32, i32, f32)] = &[
        // (decim, dspfvs, value)
        (2,   10, 44.7500),
        (2,   11, 33.5000),
        (2,   12, 66.6250),
        (2,   13, 59.0833),
        (3,   10, 33.5000),
        (3,   11, 25.1667),
        (3,   12, 50.0000),
        (3,   13, 44.3750),
        (4,   10, 66.6250),
        (4,   11, 50.0000),
        (4,   12, 100.0000),
        (4,   13, 88.5000),
        (6,   10, 59.0833),
        (6,   11, 44.3750),
        (6,   12, 88.5000),
        (6,   13, 78.2500),
        (8,   10, 68.5625),
        (8,   11, 51.3750),
        (8,   12, 102.6250),
        (8,   13, 91.0000),
        (12,  10, 60.3750),
        (12,  11, 45.2500),
        (12,  12, 90.5000),
        (12,  13, 80.3750),
        (16,  10, 69.5313),
        (16,  11, 52.1250),
        (16,  12, 104.1250),
        (16,  13, 92.4375),
        (24,  10, 61.0208),
        (24,  11, 45.7500),
        (24,  12, 91.3750),
        (24,  13, 81.0625),
        (32,  10, 70.0156),
        (32,  11, 52.4375),
        (32,  12, 104.8125),
        (32,  13, 93.0000),
        (48,  10, 61.3438),
        (48,  11, 46.0000),
        (48,  12, 91.8125),
        (48,  13, 81.5000),
        (64,  10, 70.2578),
        (64,  11, 52.5625),
        (64,  12, 105.0625),
        (64,  13, 93.2813),
        (96,  10, 61.5052),
        (96,  11, 46.1250),
        (96,  12, 92.0000),
        (96,  13, 81.6875),
        (128, 10, 70.3789),
        (128, 11, 52.6250),
        (128, 12, 105.1875),
        (128, 13, 93.4219),
        (192, 10, 61.5859),
        (192, 11, 46.1875),
        (192, 12, 92.0938),
        (192, 13, 81.7813),
        (256, 10, 70.4395),
        (256, 11, 52.6563),
        (256, 12, 105.2500),
        (256, 13, 93.4922),
        (384, 10, 61.6263),
        (384, 11, 46.2188),
        (384, 12, 92.1406),
        (384, 13, 81.8281),
        (512, 10, 70.4697),
        (512, 11, 52.6719),
        (512, 12, 105.2813),
        (512, 13, 93.5273),
        (768, 10, 61.6465),
        (768, 11, 46.2344),
        (768, 12, 92.1641),
        (768, 13, 81.8516),
        (1024, 10, 70.4849),
        (1024, 11, 52.6797),
        (1024, 12, 105.2969),
        (1024, 13, 93.5449),
        (1536, 10, 61.6566),
        (1536, 11, 46.2422),
        (1536, 12, 92.1758),
        (1536, 13, 81.8633),
        (2048, 10, 70.4924),
        (2048, 11, 52.6836),
        (2048, 12, 105.3047),
        (2048, 13, 93.5537),
        // DSPFVS 20-23 (newer firmware)
        (2,   20, 46.0000),
        (2,   21, 36.5000),
        (2,   22, 48.0000),
        (2,   23, 36.5000),
        (4,   20, 48.0000),
        (4,   21, 36.5000),
        (4,   22, 53.0000),
        (4,   23, 36.5000),
        (8,   20, 49.0000),
        (8,   21, 36.5000),
        (8,   22, 55.5000),
        (8,   23, 36.5000),
        (16,  20, 49.5000),
        (16,  21, 36.5000),
        (16,  22, 56.7500),
        (16,  23, 36.5000),
        (32,  20, 49.7500),
        (32,  21, 36.5000),
        (32,  22, 57.3750),
        (32,  23, 36.5000),
        (64,  20, 49.8750),
        (64,  21, 36.5000),
        (64,  22, 57.6875),
        (64,  23, 36.5000),
        (128, 20, 49.9375),
        (128, 21, 36.5000),
        (128, 22, 57.8438),
        (128, 23, 36.5000),
        (256, 20, 49.9688),
        (256, 21, 36.5000),
        (256, 22, 57.9219),
        (256, 23, 36.5000),
        (512, 20, 49.9844),
        (512, 21, 36.5000),
        (512, 22, 57.9609),
        (512, 23, 36.5000),
        (1024, 20, 49.9921),
        (1024, 21, 36.5000),
        (1024, 22, 57.9805),
        (1024, 23, 36.5000),
        (2048, 20, 49.9961),
        (2048, 21, 36.5000),
        (2048, 22, 57.9902),
        (2048, 23, 36.5000),
    ];

    for &(d, f, v) in table {
        if d == decim && f == dspfvs {
            return v;
        }
    }

    0.0
}

/// Persistent state for Bruker DMX correction.
///
/// Created once by [`dmx_init`], then re-used for every vector via
/// [`DmxState::correct`] (or the convenience [`dmx2fid2d`]).
pub struct DmxState {
    corrector: DFCorrector,
}

impl DmxState {
    /// Get the corrected output size (complex points per vector).
    pub fn out_size(&self) -> usize {
        self.corrector.out_size()
    }

    /// Correct one complex vector in place.
    ///
    /// After the call, `rdata[0..out_size()]` and `idata[0..out_size()]`
    /// contain the corrected data.
    pub fn correct(&self, rdata: &mut [f32], idata: &mut [f32]) {
        self.corrector.correct(rdata, idata);
    }

    /// Correct a 2D matrix of complex vectors laid out in NMRPipe order:
    /// `[R0…Rn I0…In  R0…Rn I0…In  …]` with stride = 2 × in_size.
    pub fn correct_2d(&self, data: &mut [f32], in_size: usize, y_size: usize) {
        let stride = 2 * in_size;
        for row in 0..y_size {
            let base = row * stride;
            if base + stride > data.len() {
                break;
            }
            let (r_part, i_part) = data[base..base + stride].split_at_mut(in_size);
            self.corrector.correct(r_part, i_part);
        }
    }
}

/// Initialise the DMX correction.
///
/// Returns a `DmxState` whose `out_size()` gives the corrected
/// time-domain length that should replace `NDAPOD`.
///
/// # Arguments
///
/// * `in_size`    – Complex points in X dimension.
/// * `valid_size` – NDAPOD (valid/extract size); caps the output.
/// * `skip_size`  – Points to skip in tail correction (−1 = none).
/// * `decim`      – DECIM value from acqus.
/// * `dspfvs`     – DSPFVS value from acqus.
/// * `grpdly`     – GRPDLY value (0 = use lookup table).
/// * `aq_mod`     – Acquisition mode (QF / QSIM / QSEQ / DQD).
pub fn dmx_init(
    in_size: usize,
    valid_size: usize,
    skip_size: i32,
    decim: i32,
    dspfvs: i32,
    grpdly: f32,
    aq_mod: i32,
) -> Result<DmxState, &'static str> {
    let grp = get_dmx_val(aq_mod, decim, dspfvs, grpdly);

    if grp <= 0.0 {
        return Err("could not determine group delay (check -decim / -dspfvs / -grpdly)");
    }

    let skip = if skip_size < 0 { 0 } else { skip_size as usize };

    let max_out = if valid_size > 0 && valid_size < in_size {
        Some(valid_size)
    } else {
        None
    };

    let corrector = DFCorrector::new(in_size, grp, skip, max_out);

    Ok(DmxState { corrector })
}

/// Convenience: apply DMX correction to a 2D matrix (legacy interface).
///
/// Requires a pre-initialised `DmxState`.
pub fn dmx2fid2d(state: &DmxState, data: &mut [f32], in_size: usize, y_size: usize) {
    state.correct_2d(data, in_size, y_size);
}
