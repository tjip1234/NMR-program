//! JEOL Delta binary header parsing.
//!
//! Ported from `delta.h` and the `getDeltaHdrItem()` function in `delta2pipe.c`.
//! The Delta header is 1360 bytes, parsed via a declarative field-mapping table.

// byte-order helpers use stdlib from_ne_bytes + swap_bytes instead of crate

/// Maximum number of JEOL dimensions.
pub const JMAXDIM: usize = 8;
/// Standard JEOL header size in bytes.
pub const DELTA_HDR_SIZE: usize = 1360;
/// Time fraction for JEOL date conversion.
pub const JEOL_DAYFRAC: f64 = 86400.0 / 65535.0;

// ─── Data type constants ────────────────────────────────────────────────────

pub const JEOL_DATATYPE_DOUBLE: i32 = 0;
pub const JEOL_DATATYPE_FLOAT: i32 = 1;

// ─── Data format constants ──────────────────────────────────────────────────

pub const JEOL_FORMAT_1D: i32 = 1;
pub const JEOL_FORMAT_2D: i32 = 2;
pub const JEOL_FORMAT_3D: i32 = 3;
pub const JEOL_FORMAT_4D: i32 = 4;
pub const JEOL_FORMAT_5D: i32 = 5;
pub const JEOL_FORMAT_6D: i32 = 6;
pub const JEOL_FORMAT_7D: i32 = 7;
pub const JEOL_FORMAT_8D: i32 = 8;
pub const JEOL_FORMAT_SMALL2D: i32 = 12;
pub const JEOL_FORMAT_SMALL3D: i32 = 13;
pub const JEOL_FORMAT_SMALL4D: i32 = 14;

// ─── Axis type constants ────────────────────────────────────────────────────

pub const JEOL_AXISTYPE_NONE: i32 = 0;
pub const JEOL_AXISTYPE_REAL: i32 = 1;
pub const JEOL_AXISTYPE_TPPI: i32 = 2;
pub const JEOL_AXISTYPE_COMPLEX: i32 = 3;
pub const JEOL_AXISTYPE_REAL_COMPLEX: i32 = 4;
pub const JEOL_AXISTYPE_ENVELOPE: i32 = 5;

// ─── Endian mode ────────────────────────────────────────────────────────────

pub const JEOL_BIG_ENDIAN: i32 = 0;
pub const JEOL_LITTLE_ENDIAN: i32 = 1;

// ─── SI unit type constants ─────────────────────────────────────────────────

pub const JEOL_SIUNIT_NONE: i32 = 0;
pub const JEOL_SIUNIT_HZ: i32 = 13;
pub const JEOL_SIUNIT_PPM: i32 = 26;
pub const JEOL_SIUNIT_SECONDS: i32 = 28;
pub const JEOL_SIUNIT_CELSIUS: i32 = 4;

// ─── Scale type constants ───────────────────────────────────────────────────

pub const JEOL_SCALE_YOTTA: i32 = -8;
pub const JEOL_SCALE_ZETTA: i32 = -7;
pub const JEOL_SCALE_EXA: i32 = -6;
pub const JEOL_SCALE_PETA: i32 = -5;
pub const JEOL_SCALE_TERA: i32 = -4;
pub const JEOL_SCALE_GIGA: i32 = -3;
pub const JEOL_SCALE_MEGA: i32 = -2;
pub const JEOL_SCALE_KILO: i32 = -1;
pub const JEOL_SCALE_NONE: i32 = 0;
pub const JEOL_SCALE_MILLI: i32 = 1;
pub const JEOL_SCALE_MICRO: i32 = 2;
pub const JEOL_SCALE_NANO: i32 = 3;
pub const JEOL_SCALE_PICO: i32 = 4;
pub const JEOL_SCALE_FEMTO: i32 = 5;
pub const JEOL_SCALE_ATTO: i32 = 6;
pub const JEOL_SCALE_ZEPTO: i32 = 7;

// ─── Parameter value types ──────────────────────────────────────────────────

pub const JEOL_PARMVAL_NONE: i32 = -1;
pub const JEOL_PARMVAL_STR: i32 = 0;
pub const JEOL_PARMVAL_INT: i32 = 1;
pub const JEOL_PARMVAL_FLT: i32 = 2;
pub const JEOL_PARMVAL_Z: i32 = 3;
pub const JEOL_PARMVAL_INF: i32 = 4;

pub const JEOL_JVAL_STRLEN: usize = 16;

// ─── Unit structures ────────────────────────────────────────────────────────

/// JEOL SI unit triplet.
#[derive(Debug, Clone, Copy, Default)]
pub struct JUnit {
    pub unit_type: i32,
    pub unit_exp: i32,
    pub scale_type: i32,
}

/// JEOL time structure (packed into 4 bytes).
#[derive(Debug, Clone, Copy, Default)]
pub struct JTime {
    pub year: i32,
    pub month: i32,
    pub day: i32,
    pub day_frac: i32,
    pub hour: i32,
    pub min: i32,
    pub sec: i32,
}

/// JEOL parameter value.
#[derive(Debug, Clone)]
pub enum JVal {
    None,
    Str(String),
    Int(i32),
    Float(f64),
    Complex(f64, f64),
    Inf(i32),
}

impl Default for JVal {
    fn default() -> Self {
        JVal::None
    }
}

// ─── Delta header structure ─────────────────────────────────────────────────

/// Parsed JEOL Delta file header (1360 bytes).
#[derive(Debug, Clone)]
pub struct DeltaHeader {
    pub file_id: String,
    pub endian_mode: i32,
    pub major_version: i32,
    pub minor_version: i32,
    pub dim_count: i32,
    pub dim_exists: [i32; JMAXDIM],
    pub data_type: i32,
    pub data_format: i32,
    pub instrument: i32,
    pub translate: [i32; JMAXDIM],
    pub axis_type: [i32; JMAXDIM],
    pub unit_list: [JUnit; JMAXDIM],
    pub title: String,
    pub axis_ranged: [i32; JMAXDIM],
    pub size_list: [i32; JMAXDIM],
    pub offset_start: [i32; JMAXDIM],
    pub offset_stop: [i32; JMAXDIM],
    pub axis_start: [f64; JMAXDIM],
    pub axis_stop: [f64; JMAXDIM],
    pub creation_time: JTime,
    pub revision_time: JTime,
    pub node_name: String,
    pub site: String,
    pub author: String,
    pub comment: String,
    pub axis_titles: [String; JMAXDIM],
    pub base_freq: [f64; JMAXDIM],
    pub zero_point: [f64; JMAXDIM],
    pub reversed: [i32; JMAXDIM],
    pub annotation_flag: i32,
    pub history_used: i32,
    pub history_length: i32,
    pub param_start: i32,
    pub param_length: i32,
    pub list_start: [i32; JMAXDIM],
    pub list_length: [i32; JMAXDIM],
    pub data_start: i32,
    pub data_length: i64,
    pub context_start: i64,
    pub context_length: i32,
    pub annote_start: i64,
    pub annote_length: i32,
    pub total_size: i64,
    pub unit_loc: [i32; JMAXDIM],
}

impl Default for DeltaHeader {
    fn default() -> Self {
        Self {
            file_id: String::new(),
            endian_mode: 0,
            major_version: 0,
            minor_version: 0,
            dim_count: 0,
            dim_exists: [0; JMAXDIM],
            data_type: 0,
            data_format: 0,
            instrument: 0,
            translate: [0; JMAXDIM],
            axis_type: [0; JMAXDIM],
            unit_list: [JUnit::default(); JMAXDIM],
            title: String::new(),
            axis_ranged: [0; JMAXDIM],
            size_list: [0; JMAXDIM],
            offset_start: [0; JMAXDIM],
            offset_stop: [0; JMAXDIM],
            axis_start: [0.0; JMAXDIM],
            axis_stop: [0.0; JMAXDIM],
            creation_time: JTime::default(),
            revision_time: JTime::default(),
            node_name: String::new(),
            site: String::new(),
            author: String::new(),
            comment: String::new(),
            axis_titles: Default::default(),
            base_freq: [0.0; JMAXDIM],
            zero_point: [0.0; JMAXDIM],
            reversed: [0; JMAXDIM],
            annotation_flag: 0,
            history_used: 0,
            history_length: 0,
            param_start: 0,
            param_length: 0,
            list_start: [0; JMAXDIM],
            list_length: [0; JMAXDIM],
            data_start: 0,
            data_length: 0,
            context_start: 0,
            context_length: 0,
            annote_start: 0,
            annote_length: 0,
            total_size: 0,
            unit_loc: [0; JMAXDIM],
        }
    }
}

// ─── Parameter record ───────────────────────────────────────────────────────

/// Parsed parameter from the parameter section.
#[derive(Debug, Clone)]
pub struct DeltaParam {
    pub name: String,
    pub val_type: i32,
    pub val: JVal,
    pub unit_scale: i32,
    pub units: [JUnit; 2],
}

/// Parameter section header.
#[derive(Debug, Clone, Default)]
pub struct DeltaParamHeader {
    pub parm_size: i32,
    pub lo_id: i32,
    pub hi_id: i32,
    pub total_size: i32,
}

// ─── Byte-level readers with swap support ───────────────────────────────────
//
// `swap = true` means the file bytes are in the OPPOSITE of the platform's
// native byte order and must be swapped.  We use `from_ne_bytes` followed
// by `swap_bytes` so the logic is correct on both LE and BE hosts.

fn read_u8(buf: &[u8], off: usize) -> u8 {
    buf[off]
}

fn read_u16(buf: &[u8], off: usize, swap: bool) -> u16 {
    let b: [u8; 2] = buf[off..off + 2].try_into().unwrap();
    let raw = u16::from_ne_bytes(b);
    if swap { raw.swap_bytes() } else { raw }
}

#[allow(dead_code)]
fn read_i16(buf: &[u8], off: usize, swap: bool) -> i16 {
    read_u16(buf, off, swap) as i16
}

fn read_u32(buf: &[u8], off: usize, swap: bool) -> u32 {
    let b: [u8; 4] = buf[off..off + 4].try_into().unwrap();
    let raw = u32::from_ne_bytes(b);
    if swap { raw.swap_bytes() } else { raw }
}

fn read_i32(buf: &[u8], off: usize, swap: bool) -> i32 {
    read_u32(buf, off, swap) as i32
}

fn read_u64(buf: &[u8], off: usize, swap: bool) -> u64 {
    let b: [u8; 8] = buf[off..off + 8].try_into().unwrap();
    let raw = u64::from_ne_bytes(b);
    if swap { raw.swap_bytes() } else { raw }
}

#[allow(dead_code)]
fn read_f32(buf: &[u8], off: usize, swap: bool) -> f32 {
    f32::from_bits(read_u32(buf, off, swap))
}

fn read_f64(buf: &[u8], off: usize, swap: bool) -> f64 {
    f64::from_bits(read_u64(buf, off, swap))
}

fn read_text(buf: &[u8], off: usize, len: usize) -> String {
    let end = off + len;
    let slice = &buf[off..end.min(buf.len())];
    let s: Vec<u8> = slice.iter().copied().take_while(|&b| b != 0).collect();
    String::from_utf8_lossy(&s).trim().to_string()
}

fn read_junit(buf: &[u8], off: usize) -> JUnit {
    let b0 = buf[off];
    let b1 = buf[off + 1];

    let mut unit_exp = (b0 & 0x0F) as i32;
    if unit_exp > 7 {
        unit_exp -= 16;
    }

    let mut scale_type = ((b0 >> 4) & 0x0F) as i32;
    if scale_type > 7 {
        scale_type -= 16;
    }

    let unit_type = b1 as i32;

    JUnit {
        unit_type,
        unit_exp,
        scale_type,
    }
}

fn read_jtime(buf: &[u8], off: usize, swap: bool) -> JTime {
    let n = read_u16(buf, off, swap) as i32;
    let day_frac = read_u16(buf, off + 2, swap) as i32;

    let day = n & 31;
    let month = (n >> 5) & 15;
    let year = 1990 + ((n >> 9) & 127);

    let total_secs = (day_frac as f64 * JEOL_DAYFRAC) as i32;
    let hour = (total_secs / 3600).clamp(0, 23);
    let remaining = total_secs - hour * 3600;
    let min = (remaining / 60).clamp(0, 59);
    let sec = (remaining - min * 60).clamp(0, 59);

    JTime {
        year,
        month,
        day,
        day_frac,
        hour,
        min,
        sec,
    }
}

// ─── Header parsing ─────────────────────────────────────────────────────────

impl DeltaHeader {
    /// Parse a Delta header from a 1360-byte buffer.
    ///
    /// The `swap` parameter should be `true` when the system byte order
    /// differs from the file's endianness (determined by the platform
    /// for the initial header parse, then refined using the endianMode field).
    pub fn parse(buf: &[u8], swap: bool) -> Result<Self, &'static str> {
        if buf.len() < DELTA_HDR_SIZE {
            return Err("buffer too small for Delta header");
        }

        let mut hdr = Self::default();

        // FileIdentifier (offset 0, 8 bytes)
        hdr.file_id = read_text(buf, 0, 8);

        // Endian (offset 8, 1 byte)
        hdr.endian_mode = read_u8(buf, 8) as i32;

        // MajorVersion (offset 9, 1 byte)
        hdr.major_version = read_u8(buf, 9) as i32;

        // MinorVersion (offset 10, 2 bytes)
        hdr.minor_version = read_u16(buf, 10, swap) as i32;

        // DataDimensionNumber (offset 12, 1 byte)
        hdr.dim_count = read_u8(buf, 12) as i32;

        // DataDimensionExist (offset 13, 1 byte, 8 × 1-bit)
        let exist_byte = read_u8(buf, 13);
        for i in 0..JMAXDIM {
            hdr.dim_exists[i] = ((exist_byte >> (7 - i)) & 1) as i32;
        }

        // DataType (offset 14, bits 0-1)
        let type_byte = read_u8(buf, 14);
        hdr.data_type = (type_byte >> 6) as i32; // top 2 bits
        hdr.data_format = (type_byte & 0x3F) as i32; // bottom 6 bits

        // Instrument (offset 15, 1 byte)
        hdr.instrument = read_u8(buf, 15) as i32;

        // Translate (offset 16, 8 × 1 byte)
        for i in 0..JMAXDIM {
            hdr.translate[i] = read_u8(buf, 16 + i) as i32;
        }

        // DataAxisType (offset 24, 8 × 1 byte)
        for i in 0..JMAXDIM {
            hdr.axis_type[i] = read_u8(buf, 24 + i) as i32;
        }

        // DataUnits (offset 32, 8 × 2 bytes = 16 bytes per dim unit)
        for i in 0..JMAXDIM {
            hdr.unit_list[i] = read_junit(buf, 32 + i * 2);
        }

        // Title (offset 48, 124 bytes)
        hdr.title = read_text(buf, 48, 124);

        // DataAxisRanged (offset 172, 4 bytes = 8 × 4-bit)
        for i in 0..JMAXDIM {
            if i % 2 == 0 {
                hdr.axis_ranged[i] = ((buf[172 + i / 2] >> 4) & 0x0F) as i32;
            } else {
                hdr.axis_ranged[i] = (buf[172 + i / 2] & 0x0F) as i32;
            }
        }

        // DataPoints (offset 176, 8 × 4 bytes)
        for i in 0..JMAXDIM {
            hdr.size_list[i] = read_u32(buf, 176 + i * 4, swap) as i32;
        }

        // DataOffsetStart (offset 208, 8 × 4 bytes)
        for i in 0..JMAXDIM {
            hdr.offset_start[i] = read_u32(buf, 208 + i * 4, swap) as i32;
        }

        // DataOffsetStop (offset 240, 8 × 4 bytes)
        for i in 0..JMAXDIM {
            hdr.offset_stop[i] = read_u32(buf, 240 + i * 4, swap) as i32;
        }

        // DataAxisStart (offset 272, 8 × 8 bytes)
        for i in 0..JMAXDIM {
            hdr.axis_start[i] = read_f64(buf, 272 + i * 8, swap);
        }

        // DataAxisStop (offset 336, 8 × 8 bytes)
        for i in 0..JMAXDIM {
            hdr.axis_stop[i] = read_f64(buf, 336 + i * 8, swap);
        }

        // CreationTime (offset 400, 4 bytes)
        hdr.creation_time = read_jtime(buf, 400, swap);

        // RevisionTime (offset 404, 4 bytes)
        hdr.revision_time = read_jtime(buf, 404, swap);

        // NodeName (offset 408, 16 bytes)
        hdr.node_name = read_text(buf, 408, 16);

        // Site (offset 424, 128 bytes)
        hdr.site = read_text(buf, 424, 128);

        // Author (offset 552, 128 bytes)
        hdr.author = read_text(buf, 552, 128);

        // Comment (offset 680, 128 bytes)
        hdr.comment = read_text(buf, 680, 128);

        // DataAxisTitles (offset 808, 256 bytes = 8 × 32 bytes)
        for i in 0..JMAXDIM {
            hdr.axis_titles[i] = read_text(buf, 808 + i * 32, 32);
        }

        // BaseFreq (offset 1064, 8 × 8 bytes)
        for i in 0..JMAXDIM {
            hdr.base_freq[i] = read_f64(buf, 1064 + i * 8, swap);
        }

        // ZeroPoint (offset 1128, 8 × 8 bytes)
        for i in 0..JMAXDIM {
            hdr.zero_point[i] = read_f64(buf, 1128 + i * 8, swap);
        }

        // Reversed (offset 1192, 1 byte = 8 × 1-bit)
        let rev_byte = read_u8(buf, 1192);
        for i in 0..JMAXDIM {
            hdr.reversed[i] = ((rev_byte >> (7 - i)) & 1) as i32;
        }

        // AnnotationValid (offset 1203, 1 bit)
        hdr.annotation_flag = ((buf[1203] >> 7) & 1) as i32;

        // HistoryUsed (offset 1204, 4 bytes)
        hdr.history_used = read_u32(buf, 1204, swap) as i32;

        // HistoryLength (offset 1208, 4 bytes)
        hdr.history_length = read_u32(buf, 1208, swap) as i32;

        // ParamStart (offset 1212, 4 bytes)
        hdr.param_start = read_u32(buf, 1212, swap) as i32;

        // ParamLength (offset 1216, 4 bytes)
        hdr.param_length = read_u32(buf, 1216, swap) as i32;

        // ListStart (offset 1220, 8 × 4 bytes)
        for i in 0..JMAXDIM {
            hdr.list_start[i] = read_u32(buf, 1220 + i * 4, swap) as i32;
        }

        // ListLength (offset 1252, 8 × 4 bytes)
        for i in 0..JMAXDIM {
            hdr.list_length[i] = read_u32(buf, 1252 + i * 4, swap) as i32;
        }

        // DataStart (offset 1284, 4 bytes)
        hdr.data_start = read_u32(buf, 1284, swap) as i32;

        // DataLength (offset 1288, 8 bytes)
        hdr.data_length = read_u64(buf, 1288, swap) as i64;

        // ContextStart (offset 1296, 8 bytes)
        hdr.context_start = read_u64(buf, 1296, swap) as i64;

        // ContextLength (offset 1304, 4 bytes)
        hdr.context_length = read_u32(buf, 1304, swap) as i32;

        // AnnoteStart (offset 1308, 8 bytes)
        hdr.annote_start = read_u64(buf, 1308, swap) as i64;

        // AnnoteLength (offset 1316, 4 bytes)
        hdr.annote_length = read_u32(buf, 1316, swap) as i32;

        // TotalSize (offset 1320, 8 bytes)
        hdr.total_size = read_u64(buf, 1320, swap) as i64;

        // UnitLocation (offset 1328, 8 × 1 byte)
        for i in 0..JMAXDIM {
            hdr.unit_loc[i] = read_u8(buf, 1328 + i) as i32;
        }

        Ok(hdr)
    }

    /// Determine if byte swap is needed for data section.
    /// The header itself is parsed with platform-dependent swap;
    /// data swap depends on the `endian_mode` field.
    pub fn needs_data_swap(&self) -> bool {
        if cfg!(target_endian = "big") {
            self.endian_mode == JEOL_LITTLE_ENDIAN
        } else {
            self.endian_mode == JEOL_BIG_ENDIAN
        }
    }

    /// Returns true if this dimension is quadrature (complex).
    pub fn is_quad(&self, dim: usize) -> bool {
        self.axis_type[dim] == JEOL_AXISTYPE_COMPLEX
            || self.axis_type[dim] == JEOL_AXISTYPE_ENVELOPE
            || (dim == 0 && self.axis_type[dim] == JEOL_AXISTYPE_REAL_COMPLEX)
    }

    /// Returns true if this dimension is in the time domain.
    pub fn is_time_domain(&self, dim: usize) -> bool {
        let u = &self.unit_list[dim];
        if u.unit_type == JEOL_SIUNIT_SECONDS {
            return u.unit_exp == 0 || u.unit_exp == 1;
        }
        if u.unit_type == JEOL_SIUNIT_HZ && u.unit_exp == -1 {
            return true;
        }
        if u.unit_type == JEOL_SIUNIT_PPM && u.unit_exp == -1 {
            return true;
        }
        false
    }

    /// Returns true if this dimension's units are PPM.
    pub fn is_ppm(&self, dim: usize) -> bool {
        let u = &self.unit_list[dim];
        u.unit_type == JEOL_SIUNIT_PPM && (u.unit_exp == 0 || u.unit_exp == 1)
    }

    /// Returns true if this dimension's units are Hz.
    pub fn is_hz(&self, dim: usize) -> bool {
        let u = &self.unit_list[dim];
        if u.unit_type == JEOL_SIUNIT_HZ && (u.unit_exp == 0 || u.unit_exp == 1) {
            return true;
        }
        if u.unit_type == JEOL_SIUNIT_SECONDS && u.unit_exp == -1 {
            return true;
        }
        false
    }

    /// Compute the SMX (submatrix) tile sizes based on data format.
    pub fn get_smx_sizes(&self) -> [i32; JMAXDIM] {
        let mut smx = [1i32; JMAXDIM];
        let d = self.dim_count as usize;
        match self.data_format {
            JEOL_FORMAT_1D => {
                for i in 0..d {
                    smx[i] = 8;
                }
            }
            JEOL_FORMAT_2D => {
                for i in 0..d {
                    smx[i] = 32;
                }
            }
            JEOL_FORMAT_3D | JEOL_FORMAT_4D => {
                for i in 0..d {
                    smx[i] = 8;
                }
            }
            JEOL_FORMAT_5D | JEOL_FORMAT_6D => {
                for i in 0..d {
                    smx[i] = 4;
                }
            }
            JEOL_FORMAT_7D | JEOL_FORMAT_8D => {
                for i in 0..d {
                    smx[i] = 2;
                }
            }
            JEOL_FORMAT_SMALL2D | JEOL_FORMAT_SMALL3D | JEOL_FORMAT_SMALL4D => {
                for i in 0..d {
                    smx[i] = 4;
                }
            }
            _ => {}
        }
        smx
    }

    /// Get the word size (in bytes) for input data.
    pub fn get_word_size(&self, total_in_size: i64, channel_count: i32) -> i32 {
        if self.data_type == JEOL_DATATYPE_DOUBLE {
            return 8;
        }
        if self.data_type == JEOL_DATATYPE_FLOAT {
            return 4;
        }
        // Fallback: derive from data length
        if total_in_size > 0 && channel_count > 0 {
            let ws = self.data_length / (total_in_size * channel_count as i64);
            match ws {
                3..=5 => 4,
                7..=9 => 8,
                _ => ws as i32,
            }
        } else {
            8
        }
    }

    /// Get the 2D acquisition mode.
    pub fn get_aq2d_mode(&self) -> i32 {
        let dc = self.dim_count as usize;
        if dc < 2 {
            return 0;
        }

        let q0 = if self.is_quad(0) { 2 } else { 1 };
        let q1 = if self.is_quad(1) { 2 } else { 1 };

        if q0 == 2 && q1 == 2 {
            3 // FD_STATES
        } else if self.axis_type[1] == JEOL_AXISTYPE_TPPI {
            1 // FD_TPPI
        } else if self.axis_type[0] == JEOL_AXISTYPE_REAL_COMPLEX {
            0 // FD_MAGNITUDE
        } else {
            4 // FD_REAL
        }
    }
}

// ─── Parameter section parsing ──────────────────────────────────────────────

impl DeltaParamHeader {
    /// Parse the 16-byte parameter header.
    pub fn parse(buf: &[u8], swap: bool) -> Self {
        Self {
            parm_size: read_u32(buf, 0, swap) as i32,
            lo_id: read_u32(buf, 4, swap) as i32,
            hi_id: read_u32(buf, 8, swap) as i32,
            total_size: read_u32(buf, 12, swap) as i32,
        }
    }
}

/// Parse a single parameter record from the parameter section.
///
/// Record layout (64 bytes):
///   0-3:   class (4 bytes, text)
///   4-5:   unit_scale (2 bytes, i16)
///   6-15:  units (10 bytes: 5 × JUnit)
///   16-31: value (16 bytes, JVAL)
///   32-35: val_type (4 bytes, u32)
///   36-63: name (28 bytes, text)
pub fn parse_param_record(buf: &[u8], swap: bool) -> DeltaParam {
    let name = read_text(buf, 36, 28);
    let val_type = read_u32(buf, 32, swap) as i32;

    let unit_scale = read_i16(buf, 4, swap) as i32;
    let units = [read_junit(buf, 6), read_junit(buf, 8)];

    let val = match val_type {
        JEOL_PARMVAL_STR => {
            let s = read_text(buf, 16, JEOL_JVAL_STRLEN);
            JVal::Str(s)
        }
        JEOL_PARMVAL_INT => {
            let i = read_i32(buf, 16, swap);
            JVal::Int(i)
        }
        JEOL_PARMVAL_FLT => {
            let d = read_f64(buf, 16, swap);
            JVal::Float(d)
        }
        JEOL_PARMVAL_Z => {
            let re = read_f64(buf, 16, swap);
            let im = read_f64(buf, 24, swap);
            JVal::Complex(re, im)
        }
        JEOL_PARMVAL_INF => {
            let i = read_i32(buf, 16, swap);
            JVal::Inf(i)
        }
        _ => JVal::None,
    };

    DeltaParam {
        name,
        val_type,
        val,
        unit_scale,
        units,
    }
}

// ─── Unit value conversion ──────────────────────────────────────────────────

/// Apply SI unit scale factor to a value.
pub fn apply_unit_scale(val: f64, unit: &JUnit) -> f64 {
    let scale_factor = match unit.scale_type {
        JEOL_SCALE_YOTTA => 1.0e+24,
        JEOL_SCALE_ZETTA => 1.0e+21,
        JEOL_SCALE_EXA => 1.0e+18,
        JEOL_SCALE_PETA => 1.0e+15,
        JEOL_SCALE_TERA => 1.0e+12,
        JEOL_SCALE_GIGA => 1.0e+9,
        JEOL_SCALE_MEGA => 1.0e+6,
        JEOL_SCALE_KILO => 1.0e+3,
        JEOL_SCALE_NONE => 1.0,
        JEOL_SCALE_MILLI => 1.0e-3,
        JEOL_SCALE_MICRO => 1.0e-6,
        JEOL_SCALE_NANO => 1.0e-9,
        JEOL_SCALE_PICO => 1.0e-12,
        JEOL_SCALE_FEMTO => 1.0e-15,
        JEOL_SCALE_ATTO => 1.0e-18,
        JEOL_SCALE_ZEPTO => 1.0e-21,
        _ => 1.0,
    };

    let mut result = val * scale_factor;

    if unit.unit_exp != 0 && unit.unit_exp != 1 {
        result = result.powf(unit.unit_exp as f64);
    }

    result
}

/// Get a float value from a parameter, applying unit scaling.
pub fn param_float_val(param: &DeltaParam) -> f64 {
    let raw = match &param.val {
        JVal::Int(i) => *i as f64,
        JVal::Float(d) => *d,
        _ => 0.0,
    };
    let mut val = apply_unit_scale(raw, &param.units[0]);
    if param.unit_scale != 0 {
        val *= 10.0f64.powi(param.unit_scale);
    }
    val
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unit_scale() {
        let unit = JUnit {
            unit_type: JEOL_SIUNIT_HZ,
            unit_exp: 1,
            scale_type: JEOL_SCALE_MEGA,
        };
        let val = apply_unit_scale(600.13, &unit);
        assert!((val - 600.13e6).abs() < 1.0);
    }
}
