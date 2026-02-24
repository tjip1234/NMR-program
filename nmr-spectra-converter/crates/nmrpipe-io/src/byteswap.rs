//! Byte-swap and int→float conversion helpers.
//!
//! Ported from `bswap.c`. Handles in-place 4-byte swap and 3-byte/4-byte
//! integer-to-float conversion used by Bruker format readers.

/// Swap bytes of 4-byte words in place.
pub fn bswap4(buf: &mut [u8]) {
    debug_assert!(buf.len() % 4 == 0, "bswap4: buffer length must be multiple of 4");
    for chunk in buf.chunks_exact_mut(4) {
        chunk.swap(0, 3);
        chunk.swap(1, 2);
    }
}

/// Swap bytes of 2-byte words in place.
pub fn bswap2(buf: &mut [u8]) {
    debug_assert!(buf.len() % 2 == 0, "bswap2: buffer length must be multiple of 2");
    for chunk in buf.chunks_exact_mut(2) {
        chunk.swap(0, 1);
    }
}

/// Swap bytes of 8-byte words in place.
pub fn bswap8(buf: &mut [u8]) {
    debug_assert!(buf.len() % 8 == 0, "bswap8: buffer length must be multiple of 8");
    for chunk in buf.chunks_exact_mut(8) {
        chunk.swap(0, 7);
        chunk.swap(1, 6);
        chunk.swap(2, 5);
        chunk.swap(3, 4);
    }
}

/// Convert a slice of big-endian 4-byte signed integers to f32.
/// Swaps bytes first if `swap` is true, then interprets as i32 → f32.
pub fn int4_to_float(buf: &mut [u8], swap: bool) -> Vec<f32> {
    debug_assert!(buf.len() % 4 == 0);
    if swap {
        bswap4(buf);
    }
    buf.chunks_exact(4)
        .map(|c| {
            let val = i32::from_ne_bytes([c[0], c[1], c[2], c[3]]);
            val as f32
        })
        .collect()
}

/// Convert a slice of 3-byte signed integers (big-endian) to f32.
///
/// Each 3-byte group is sign-extended to 32 bits, then cast to f32.
/// Used by Bruker AM-format data.
pub fn int3_to_float(buf: &[u8], swap: bool) -> Vec<f32> {
    debug_assert!(buf.len() % 3 == 0);
    buf.chunks_exact(3)
        .map(|c| {
            // 3-byte big-endian: MSB is c[0], c[1], c[2]
            let (b0, b1, b2) = if swap { (c[2], c[1], c[0]) } else { (c[0], c[1], c[2]) };

            // Sign extend from 24-bit to 32-bit
            let v = ((b0 as i32) << 16) | ((b1 as i32) << 8) | (b2 as i32);
            let v = if v & 0x800000 != 0 {
                v | !0xFF_FFFF // sign extend
            } else {
                v
            };
            v as f32
        })
        .collect()
}

/// Detect platform byte order: returns `true` if big-endian.
pub fn is_big_endian() -> bool {
    cfg!(target_endian = "big")
}

/// Check if two byte buffers have different endianness based on the
/// NMRPipe FDFLTORDER constant.
pub fn needs_swap(flt_order_bytes: &[u8; 4]) -> bool {
    let val = f32::from_ne_bytes(*flt_order_bytes);
    let target = nmrpipe_core::fdata::FD_ORDER_CONS;
    (val - target).abs() > 0.001
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bswap4() {
        let mut buf = [0x01u8, 0x02, 0x03, 0x04];
        bswap4(&mut buf);
        assert_eq!(buf, [0x04, 0x03, 0x02, 0x01]);
    }

    #[test]
    fn test_int4_to_float_no_swap() {
        let val: i32 = 42;
        let bytes = val.to_ne_bytes();
        let mut buf = bytes.to_vec();
        let result = int4_to_float(&mut buf, false);
        assert_eq!(result[0], 42.0);
    }

    #[test]
    fn test_int3_to_float() {
        // +1 in big-endian 3-byte: [0x00, 0x00, 0x01]
        let buf = [0x00u8, 0x00, 0x01];
        let result = int3_to_float(&buf, false);
        assert_eq!(result[0], 1.0);

        // -1 in big-endian 3-byte: [0xFF, 0xFF, 0xFF]
        let buf = [0xFFu8, 0xFF, 0xFF];
        let result = int3_to_float(&buf, false);
        assert_eq!(result[0], -1.0);
    }
}
