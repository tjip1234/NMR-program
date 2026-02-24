//! High-level parameter helpers for FDATA headers.
//!
//! These wrap `Fdata::get_parm()` / `Fdata::set_parm()` with common
//! compound operations like computing Hz/PPM conversions, setting up
//! sweep-width / carrier / size blocks in one call, etc.

use crate::enums::*;
use crate::fdata::*;

/// Dimension identifier (1-based).
pub const CUR_XDIM: i32 = 1;
pub const CUR_YDIM: i32 = 2;
pub const CUR_ZDIM: i32 = 3;
pub const CUR_ADIM: i32 = 4;
pub const NULL_DIM: i32 = 0;

impl Fdata {
    // ─── Convenience getters ────────────────────────────────────────────

    /// Get the size (number of real+imag points) for a dimension.
    pub fn get_size(&self, dim: i32) -> i32 {
        self.get_parm_i(NDSIZE, dim)
    }

    /// Set the size for a dimension.
    pub fn set_size(&mut self, dim: i32, size: i32) {
        self.set_parm(NDSIZE, size as f32, dim);
    }

    /// Get the spectral width in Hz.
    pub fn get_sw(&self, dim: i32) -> f64 {
        self.get_parm(NDSW, dim) as f64
    }

    /// Get the observe frequency in MHz.
    pub fn get_obs(&self, dim: i32) -> f64 {
        self.get_parm(NDOBS, dim) as f64
    }

    /// Get the spectral origin in Hz.
    pub fn get_orig(&self, dim: i32) -> f64 {
        self.get_parm(NDORIG, dim) as f64
    }

    /// Get the carrier frequency in Hz (offset from obs).
    pub fn get_car(&self, dim: i32) -> f64 {
        self.get_parm(NDCAR, dim) as f64
    }

    /// Is this dimension in the frequency domain?
    pub fn is_freq(&self, dim: i32) -> bool {
        self.get_parm_i(NDFTFLAG, dim) != 0
    }

    /// Is this dimension complex?
    pub fn is_complex(&self, dim: i32) -> bool {
        let qf = self.get_parm_i(NDQUADFLAG, dim);
        qf == 0 // COMPLEX = 0 in NMRPipe convention
    }

    // ─── Dimension setup helpers ────────────────────────────────────────

    /// Set up a dimension's spectral parameters in one call.
    pub fn set_dim_spectral(
        &mut self,
        dim: i32,
        size: i32,
        sw: f64,
        obs: f64,
        orig: f64,
        car: f64,
        label: &str,
        is_complex: bool,
    ) {
        self.set_parm(NDSIZE, size as f32, dim);
        self.set_parm(NDSW, sw as f32, dim);
        self.set_parm(NDOBS, obs as f32, dim);
        self.set_parm(NDORIG, orig as f32, dim);
        self.set_parm(NDCAR, car as f32, dim);
        self.set_parm_str(NDLABEL, label, dim);

        if is_complex {
            self.set_parm(NDQUADFLAG, QuadFlag::Complex as i32 as f32, dim);
        } else {
            self.set_parm(NDQUADFLAG, QuadFlag::Real as i32 as f32, dim);
        }

        // Time domain by default
        self.set_parm(NDFTFLAG, 0.0, dim);
    }

    /// Compute and set the spectral origin from carrier, sw, obs, and size.
    ///
    /// NMRPipe convention:
    ///   orig = car * obs - sw/2 + sw/size (first-point correction)
    pub fn compute_orig(&mut self, dim: i32) {
        let sw = self.get_sw(dim);
        let obs = self.get_obs(dim);
        let car = self.get_car(dim);
        let size = self.get_size(dim) as f64;
        let center = self.get_parm(NDCENTER, dim) as f64;

        if obs > 0.0 && sw > 0.0 && size > 0.0 {
            let orig = if center > 0.0 {
                car * obs - sw * (center - 1.0) / size
            } else {
                car * obs - sw / 2.0 + sw / (2.0 * size)
            };
            self.set_parm(NDORIG, orig as f32, dim);
        }
    }

    /// Set the acquisition sign (quadrature detection method).
    pub fn set_aqsign(&mut self, dim: i32, aqsign: AqSign) {
        self.set_parm(NDAQSIGN, aqsign as i32 as f32, dim);
    }

    /// Set the 2D phase method (for indirect dims).
    pub fn set_phase2d(&mut self, phase: Phase2D) {
        self.data[FD2DPHASE] = phase as i32 as f32;
    }

    /// Get the 2D phase method.
    pub fn get_phase2d(&self) -> Phase2D {
        Phase2D::from_i32(self.data[FD2DPHASE] as i32).unwrap_or(Phase2D::Magnitude)
    }

    /// Set whether data is transposed.
    pub fn set_transposed(&mut self, transposed: bool) {
        self.data[FDTRANSPOSED] = if transposed { 1.0 } else { 0.0 };
    }

    /// Is data transposed?
    pub fn is_transposed(&self) -> bool {
        self.data[FDTRANSPOSED] as i32 != 0
    }

    /// Set pipe/stream mode.
    pub fn set_pipe_flag(&mut self, is_pipe: bool) {
        self.data[FDPIPEFLAG] = if is_pipe { 1.0 } else { 0.0 };
    }

    /// Is pipe/stream mode?
    pub fn is_pipe(&self) -> bool {
        self.data[FDPIPEFLAG] as i32 != 0
    }

    // ─── Title / Comment ────────────────────────────────────────────────

    /// Set the title string (up to 60 chars).
    pub fn set_title(&mut self, title: &str) {
        let end = (FDTITLE + (SIZE_TITLE + 3) / 4).min(FDATA_SIZE);
        Self::txt2flt(title, &mut self.data[FDTITLE..end], SIZE_TITLE);
    }

    /// Get the title string.
    pub fn get_title(&self) -> String {
        let end = (FDTITLE + (SIZE_TITLE + 3) / 4).min(FDATA_SIZE);
        Self::flt2txt(&self.data[FDTITLE..end], SIZE_TITLE)
    }

    /// Set the comment string (up to 160 chars).
    pub fn set_comment(&mut self, comment: &str) {
        let end = (FDCOMMENT + (SIZE_COMMENT + 3) / 4).min(FDATA_SIZE);
        Self::txt2flt(comment, &mut self.data[FDCOMMENT..end], SIZE_COMMENT);
    }

    /// Get the comment string.
    pub fn get_comment(&self) -> String {
        let end = (FDCOMMENT + (SIZE_COMMENT + 3) / 4).min(FDATA_SIZE);
        Self::flt2txt(&self.data[FDCOMMENT..end], SIZE_COMMENT)
    }

    /// Set source file name (up to 16 chars).
    pub fn set_srcname(&mut self, name: &str) {
        let end = (FDSRCNAME + (SIZE_SRCNAME + 3) / 4).min(FDATA_SIZE);
        Self::txt2flt(name, &mut self.data[FDSRCNAME..end], SIZE_SRCNAME);
    }

    /// Set user name (up to 16 chars).
    pub fn set_username(&mut self, name: &str) {
        let end = (FDUSERNAME + (SIZE_USERNAME + 3) / 4).min(FDATA_SIZE);
        Self::txt2flt(name, &mut self.data[FDUSERNAME..end], SIZE_USERNAME);
    }

    /// Set operator name (up to 32 chars).
    pub fn set_opername(&mut self, name: &str) {
        let end = (FDOPERNAME + (SIZE_OPERNAME + 3) / 4).min(FDATA_SIZE);
        Self::txt2flt(name, &mut self.data[FDOPERNAME..end], SIZE_OPERNAME);
    }

    // ─── Date/Time ──────────────────────────────────────────────────────

    /// Set date fields.
    pub fn set_date(&mut self, year: i32, month: i32, day: i32) {
        self.data[FDYEAR] = year as f32;
        self.data[FDMONTH] = month as f32;
        self.data[FDDAY] = day as f32;
    }

    /// Set time fields.
    pub fn set_time(&mut self, hours: i32, mins: i32, secs: i32) {
        self.data[FDHOURS] = hours as f32;
        self.data[FDMINS] = mins as f32;
        self.data[FDSECS] = secs as f32;
    }

    // ─── DMX / digital filter ───────────────────────────────────────────

    /// Set digital-filter related parameters.
    pub fn set_dmx(&mut self, dmx_val: f32, dmx_flag: f32) {
        self.data[FDDMXVAL] = dmx_val;
        self.data[FDDMXFLAG] = dmx_flag;
    }

    /// Get DMX value (digital-filter correction).
    pub fn get_dmx_val(&self) -> f32 {
        self.data[FDDMXVAL]
    }

    /// Get DMX flag.
    pub fn get_dmx_flag(&self) -> f32 {
        self.data[FDDMXFLAG]
    }

    // ─── Min / Max / Scale ──────────────────────────────────────────────

    pub fn set_min_max(&mut self, min: f32, max: f32) {
        self.data[FDMIN] = min;
        self.data[FDMAX] = max;
    }

    pub fn get_min(&self) -> f32 {
        self.data[FDMIN]
    }

    pub fn get_max(&self) -> f32 {
        self.data[FDMAX]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dim_spectral() {
        let mut fd = Fdata::new();
        fd.init_default();
        fd.set_dim_count(2);
        fd.set_dim_spectral(CUR_XDIM, 2048, 12000.0, 600.13, 4800.0, 4.7, "1H", true);
        fd.set_dim_spectral(CUR_YDIM, 256, 3000.0, 60.81, 1500.0, 120.0, "15N", true);

        assert_eq!(fd.get_size(CUR_XDIM), 2048);
        assert_eq!(fd.get_size(CUR_YDIM), 256);
        assert!((fd.get_sw(CUR_XDIM) - 12000.0).abs() < 0.01);
        assert!((fd.get_obs(CUR_YDIM) - 60.81).abs() < 0.01);
    }

    #[test]
    fn test_title_comment() {
        let mut fd = Fdata::new();
        fd.init_default();
        fd.set_title("Test experiment");
        assert_eq!(fd.get_title(), "Test experiment");
        fd.set_comment("This is a longer comment string");
        assert_eq!(fd.get_comment(), "This is a longer comment string");
    }
}
