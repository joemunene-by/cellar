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

/// Write a u64 as a FreeArc varint to `out`, choosing the smallest
/// width that fits.
pub fn write_varint(out: &mut Vec<u8>, value: u64) {
    // 1 byte: value < 2^7
    if value < (1u64 << 7) {
        out.push(((value << 1) & 0xFF) as u8);
        return;
    }
    // 2 bytes: value < 2^14
    if value < (1u64 << 14) {
        let raw = (value << 2) | 0b01;
        out.extend_from_slice(&(raw as u16).to_le_bytes());
        return;
    }
    // 3 bytes: value < 2^21
    if value < (1u64 << 21) {
        let raw = (value << 3) | 0b011;
        out.extend_from_slice(&(raw as u32).to_le_bytes()[..3]);
        return;
    }
    // 4 bytes: value < 2^28
    if value < (1u64 << 28) {
        let raw = (value << 4) | 0b0111;
        out.extend_from_slice(&(raw as u32).to_le_bytes());
        return;
    }
    // 5 bytes: value < 2^35
    if value < (1u64 << 35) {
        let raw = (value << 5) | 0b01111;
        out.extend_from_slice(&raw.to_le_bytes()[..5]);
        return;
    }
    // 6 bytes
    if value < (1u64 << 42) {
        let raw = (value << 6) | 0b011111;
        out.extend_from_slice(&raw.to_le_bytes()[..6]);
        return;
    }
    // 7 bytes
    if value < (1u64 << 49) {
        let raw = (value << 7) | 0b0111111;
        out.extend_from_slice(&raw.to_le_bytes()[..7]);
        return;
    }
    // 8 bytes: value < 2^56, marker 0b01111111
    if value < (1u64 << 56) {
        let raw = (value << 8) | 0b01111111;
        out.extend_from_slice(&raw.to_le_bytes());
        return;
    }
    // 9 bytes: marker byte 0xFF, then raw u64 LE
    out.push(0xFF);
    out.extend_from_slice(&value.to_le_bytes());
}

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

    #[test]
    fn write_read_roundtrip() {
        // Cover every width branch: 1, 2, 3, 4, 5, 6, 7, 8, 9.
        let values: &[u64] = &[
            0, 1, 63, 127,           // 1-byte boundary
            128, 200, 16383,          // 2-byte
            16384, 100_000, 2_097_151,// 3-byte
            2_097_152, 9_577_819, 268_435_455, // 4-byte (incl real fg-05.bin val)
            268_435_456, 34_359_738_367, // 5-byte
            34_359_738_368, 4_398_046_511_103, // 6-byte
            4_398_046_511_104, 562_949_953_421_311, // 7-byte
            562_949_953_421_312, 72_057_594_037_927_935, // 8-byte
            72_057_594_037_927_936, u64::MAX, // 9-byte
        ];
        for &v in values {
            let mut buf = Vec::new();
            write_varint(&mut buf, v);
            let mut p = 0;
            let got = read_varint(&buf, &mut p).unwrap();
            assert_eq!(got, v, "round-trip failed for {}: bytes={:02x?}", v, buf);
            assert_eq!(p, buf.len(), "didn't consume all bytes for {}", v);
        }
    }
}
