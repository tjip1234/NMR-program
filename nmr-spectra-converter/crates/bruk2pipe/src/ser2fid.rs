//! Bruker serial → float conversion routines.
//!
//! Ports the six `ser2quad*` / `ser2real*` variants from bruk2pipe.c.
//! Bruker data is stored as big-endian integers (AMX/DMX) or 3-byte
//! integers (AM).  These routines byte-swap and convert to native f32.

use byteorder::{BigEndian, LittleEndian, ByteOrder};

/// Convert 4-byte complex (alternating R/I) Bruker data to float pairs.
///
/// Each input "point" is two 4-byte words: R then I.
/// Output: `rdata[0..x_size]` = real, `idata[0..x_size]` = imaginary.
///
/// `i2f`: if true, treat input words as big-endian i32 → f32 cast.
///        if false, treat as big-endian f32 → f32 (identity after swap).
pub fn ser2quad4(
    bdata: &[u8],
    rdata: &mut [f32],
    idata: &mut [f32],
    x_size: usize,
    swap: bool,
    i2f: bool,
) {
    for i in 0..x_size {
        let r_off = i * 8;
        let i_off = r_off + 4;
        if i_off + 4 > bdata.len() {
            break;
        }

        let r_word = &bdata[r_off..r_off + 4];
        let i_word = &bdata[i_off..i_off + 4];

        rdata[i] = word4_to_float(r_word, swap, i2f);
        idata[i] = word4_to_float(i_word, swap, i2f);
    }
}

/// Convert 8-byte complex (alternating R/I) Bruker data to float pairs.
pub fn ser2quad8(
    bdata: &[u8],
    rdata: &mut [f32],
    idata: &mut [f32],
    x_size: usize,
    swap: bool,
    i2f: bool,
) {
    for i in 0..x_size {
        let r_off = i * 16;
        let i_off = r_off + 8;
        if i_off + 8 > bdata.len() {
            break;
        }

        let r_word = &bdata[r_off..r_off + 8];
        let i_word = &bdata[i_off..i_off + 8];

        rdata[i] = word8_to_float(r_word, swap, i2f);
        idata[i] = word8_to_float(i_word, swap, i2f);
    }
}

/// Convert 3-byte complex (alternating R/I) Bruker AM data to float pairs.
///
/// Each 3-byte word is packed into the upper 3 bytes of a 4-byte int,
/// then divided by 256 to restore the original magnitude with sign.
pub fn ser2quad3(
    bdata: &[u8],
    rdata: &mut [f32],
    idata: &mut [f32],
    x_size: usize,
    swap: bool,
) {
    for i in 0..x_size {
        let r_off = i * 6;
        let i_off = r_off + 3;
        if i_off + 3 > bdata.len() {
            break;
        }

        rdata[i] = word3_to_float(&bdata[r_off..r_off + 3], swap);
        idata[i] = word3_to_float(&bdata[i_off..i_off + 3], swap);
    }
}

/// Convert 4-byte real-only Bruker data to float.
pub fn ser2real4(
    bdata: &[u8],
    rdata: &mut [f32],
    x_size: usize,
    swap: bool,
    i2f: bool,
) {
    for i in 0..x_size {
        let off = i * 4;
        if off + 4 > bdata.len() {
            break;
        }
        rdata[i] = word4_to_float(&bdata[off..off + 4], swap, i2f);
    }
}

/// Convert 8-byte real-only Bruker data to float.
pub fn ser2real8(
    bdata: &[u8],
    rdata: &mut [f32],
    x_size: usize,
    swap: bool,
    i2f: bool,
) {
    for i in 0..x_size {
        let off = i * 8;
        if off + 8 > bdata.len() {
            break;
        }
        rdata[i] = word8_to_float(&bdata[off..off + 8], swap, i2f);
    }
}

/// Convert 3-byte real-only Bruker AM data to float.
pub fn ser2real3(
    bdata: &[u8],
    rdata: &mut [f32],
    x_size: usize,
    swap: bool,
) {
    for i in 0..x_size {
        let off = i * 3;
        if off + 3 > bdata.len() {
            break;
        }
        rdata[i] = word3_to_float(&bdata[off..off + 3], swap);
    }
}

/// Convert a 2D matrix of vectors from Bruker SER to NMRPipe float format.
///
/// * `bdata` — raw byte input (Bruker serial format)
/// * `rdata` — output float buffer, arranged as NMRPipe complex vectors:
///   for complex data: `[R0..Rn, I0..In, R0..Rn, I0..In, …]`
/// * `x_size` — number of complex points per row
/// * `y_size` — number of rows
/// * `quad_state` — 1 = real, 2 = complex
/// * `quad_type` — 0 = complex (alternating R/I), 1/2 = real-only
/// * `word_size` — bytes per input word (3, 4, or 8)
/// * `swap` — byte-swap flag
/// * `i2f` — int-to-float conversion flag
/// * `bad_thresh` — clip values above this magnitude (0 = no clip)
pub fn ser2fid2d(
    bdata: &[u8],
    rdata: &mut [f32],
    x_size: usize,
    y_size: usize,
    quad_state: usize,
    quad_type: i32,
    word_size: usize,
    swap: bool,
    i2f: bool,
    bad_thresh: f32,
) {
    let stride = x_size * quad_state;

    match quad_type {
        0 => {
            // Complex: alternating R/I in input, separated R/I in output
            for row in 0..y_size {
                let b_off = row * word_size * x_size * quad_state;
                let r_off = row * stride;
                let i_off = r_off + x_size;

                if b_off + word_size * x_size * quad_state > bdata.len() {
                    break;
                }
                if i_off + x_size > rdata.len() {
                    break;
                }

                let b_slice = &bdata[b_off..];
                let (left, right) = rdata.split_at_mut(i_off);
                let r_slice = &mut left[r_off..r_off + x_size];
                let i_slice = &mut right[..x_size];

                match word_size {
                    8 => ser2quad8(b_slice, r_slice, i_slice, x_size, swap, i2f),
                    3 => ser2quad3(b_slice, r_slice, i_slice, x_size, swap),
                    _ => ser2quad4(b_slice, r_slice, i_slice, x_size, swap, i2f),
                }
            }
        }
        _ => {
            // Real-only (or pseudo-quad): no R/I interleave
            for row in 0..y_size {
                let b_off = row * word_size * x_size;
                let r_off = row * x_size;

                if b_off + word_size * x_size > bdata.len() {
                    break;
                }
                if r_off + x_size > rdata.len() {
                    break;
                }

                let b_slice = &bdata[b_off..];
                let r_slice = &mut rdata[r_off..r_off + x_size];

                match word_size {
                    8 => ser2real8(b_slice, r_slice, x_size, swap, i2f),
                    3 => ser2real3(b_slice, r_slice, x_size, swap),
                    _ => ser2real4(b_slice, r_slice, x_size, swap, i2f),
                }
            }
        }
    }

    // Clip bad points
    if bad_thresh > 0.0 {
        let length = x_size * y_size * quad_state;
        bad_clip(rdata, bad_thresh, length);
    }
}

/// Extract valid portion of X-axis from each row of a 2D matrix.
///
/// Compacts each row from `x_size` to `x_ext_size` points.
pub fn x_ext_2d(data: &mut [f32], x_size: usize, x_ext_size: usize, y_size: usize) {
    if x_ext_size >= x_size {
        return;
    }

    let mut dest = 0usize;
    for row in 0..y_size {
        let src_start = row * x_size;
        for col in 0..x_ext_size {
            data[dest] = data[src_start + col];
            dest += 1;
        }
    }
}

/// Zero out values exceeding `thresh` in magnitude.
pub fn bad_clip(data: &mut [f32], thresh: f32, length: usize) {
    for i in 0..length.min(data.len()) {
        if data[i].abs() > thresh {
            data[i] = 0.0;
        }
    }
}

// ─── Internal helpers ───────────────────────────────────────────────────────

/// Convert a 4-byte word to f32.
///
/// Bruker data on disk is big-endian.  
/// `swap=true` means do byte-swap (we're on a little-endian system reading big-endian).
/// Actually the C code logic: if `swapFlag` is true, the bytes are read in
/// original disk order (big-endian), so on a little-endian system that means
/// we interpret them in big-endian → native.
/// If `swapFlag` is false, bytes are reversed (for big-endian systems reading
/// big-endian data, but the C code reverses anyway, so no swap needed).
///
/// In practice: `swap=true` → read as big-endian, `swap=false` → read as little-endian.
fn word4_to_float(bytes: &[u8], swap: bool, i2f: bool) -> f32 {
    if i2f {
        let val = if swap {
            BigEndian::read_i32(bytes)
        } else {
            LittleEndian::read_i32(bytes)
        };
        val as f32
    } else {
        let val = if swap {
            BigEndian::read_f32(bytes)
        } else {
            LittleEndian::read_f32(bytes)
        };
        val
    }
}

/// Convert an 8-byte word to f32.
fn word8_to_float(bytes: &[u8], swap: bool, i2f: bool) -> f32 {
    if i2f {
        let val = if swap {
            BigEndian::read_i64(bytes)
        } else {
            LittleEndian::read_i64(bytes)
        };
        val as f32
    } else {
        let val = if swap {
            BigEndian::read_f64(bytes)
        } else {
            LittleEndian::read_f64(bytes)
        };
        val as f32
    }
}

/// Convert a 3-byte word to f32.
///
/// The 3 bytes represent a 24-bit signed integer.
/// The C code packs them into the upper 3 bytes of a 4-byte int, then
/// divides by 256 to recover the 24-bit value with correct sign.
fn word3_to_float(bytes: &[u8], swap: bool) -> f32 {
    let (b0, b1, b2) = (bytes[0], bytes[1], bytes[2]);

    // Pack into i32: upper 3 bytes, lower byte = 0
    let val = if swap {
        // swap mode: bytes are big-endian (MSB first)
        i32::from_be_bytes([0, b0, b1, b2]) << 8
    } else {
        // no-swap mode: bytes are little-endian
        i32::from_le_bytes([b2, b1, b0, 0])
    };

    // Divide by 256 to shift back from upper 3 bytes
    (val / 256) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_word4_int_swap() {
        // Big-endian i32 = 1000
        let bytes = 1000i32.to_be_bytes();
        let val = word4_to_float(&bytes, true, true);
        assert_eq!(val, 1000.0);
    }

    #[test]
    fn test_word4_float_no_swap() {
        // Little-endian f32 = 3.14
        let bytes = 3.14f32.to_le_bytes();
        let val = word4_to_float(&bytes, false, false);
        assert!((val - 3.14).abs() < 1e-5);
    }

    #[test]
    fn test_word3() {
        // 3-byte big-endian integer = 100
        // 100 in 24-bit big-endian: [0x00, 0x00, 0x64]
        let bytes = [0x00u8, 0x00, 0x64];
        let val = word3_to_float(&bytes, true);
        assert_eq!(val, 100.0);
    }

    #[test]
    fn test_bad_clip() {
        let mut data = vec![1.0, -5000.0, 3.0, 10000.0, -2.0];
        bad_clip(&mut data, 4000.0, 5);
        assert_eq!(data, vec![1.0, 0.0, 3.0, 0.0, -2.0]);
    }
}
