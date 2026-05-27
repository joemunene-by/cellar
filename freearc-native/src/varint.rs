//! FreeArc's variable-length integer encoding.
//!
//! Numbers in control blocks use a 1-to-9 byte format where the low
//! bits of the first byte indicate the width:
//!
//!   xxxxxxx0           — 1 byte,  value = byte >> 1
//!   xxxxxx01 xxxxxxxx  — 2 bytes, value = (u16 LE) >> 2
//!   xxxxx011 ...       — 3 bytes, value = (u24 LE) >> 3
//!   xxxx0111 ...       — 4 bytes, value = (u32 LE) >> 4
//!   xxx01111 ...       — 5 bytes, value = (u40 LE) >> 5
//!   xx011111 ...       — 6 bytes, value = (u48 LE) >> 6
//!   x0111111 ...       — 7 bytes, value = (u56 LE) >> 7
//!   01111111 ...       — 8 bytes, value =  u64 LE  >> 8
//!   11111111 ...       — 9 bytes, value =  u64 LE  (raw, no shift)
//!
//! Ported from `MEMORY_BUFFER::readInteger` in FreeArc's
//! `ArcStructure.h`.

use crate::error::{ArcError, Result};

/// Read one FreeArc varint from `buf[*pos..]` and advance `*pos`.
pub fn read_varint(buf: &[u8], pos: &mut usize) -> Result<u64> {
    if *pos >= buf.len() {
        return Err(ArcError::Truncated);
    }
    let first = buf[*pos];

    // 1-byte case: low bit clear.
    if first & 1 == 0 {
        *pos += 1;
        return Ok((first as u64) >> 1);
    }

    // Determine width by counting trailing 1-bits in `first`.
    let trailing = (!first).trailing_zeros() as usize;
    let width = trailing + 1; // bytes to read

    if *pos + width > buf.len() {
        return Err(ArcError::Truncated);
    }

    // Special case: first byte is 0xFF means "next 8 bytes are the
    // raw u64 value".
    if first == 0xFF {
        let mut le = [0u8; 8];
        le.copy_from_slice(&buf[*pos + 1..*pos + 9]);
        *pos += 9;
        return Ok(u64::from_le_bytes(le));
    }

    // Generic case: read `width` little-endian bytes into a u64,
    // then shift right by `width` to drop the width-marker bits.
    let mut le = [0u8; 8];
    le[..width].copy_from_slice(&buf[*pos..*pos + width]);
    let raw = u64::from_le_bytes(le);
    *pos += width;
    Ok(raw >> width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_byte() {
        let mut p = 0;
        assert_eq!(read_varint(&[0x06], &mut p).unwrap(), 3);
        assert_eq!(p, 1);
    }

    #[test]
    fn two_byte() {
        // value 100 encoded in 2 bytes: (100 << 2) | 0b01 = 0x191
        let mut p = 0;
        assert_eq!(read_varint(&[0x91, 0x01], &mut p).unwrap(), 100);
        assert_eq!(p, 2);
    }

    #[test]
    fn zero() {
        let mut p = 0;
        assert_eq!(read_varint(&[0x00], &mut p).unwrap(), 0);
        assert_eq!(p, 1);
    }
}
