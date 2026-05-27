//! Find and parse the FreeArc footer descriptor.
//!
//! The footer block sits at the end of every FreeArc archive. To find
//! it we scan the last 4 KiB of the file backwards for the 4-byte
//! signature `ArC\x01`. The bytes from that position onwards are the
//! FOOTER's local descriptor, which tells us where the actual footer
//! block lives and how it is compressed.
//!
//! Ported from `FindFooterDescriptor` and `LOCAL_BLOCK_DESCRIPTOR` in
//! FreeArc's `ArcStructure.h`.

use std::io::{Read, Seek, SeekFrom};

use crate::error::{ArcError, Result};
use crate::varint::read_varint;

pub const SIGNATURE: [u8; 4] = *b"ArC\x01";
pub const SIGNATURE_LE: u32 = u32::from_le_bytes(SIGNATURE);
pub const MAX_FOOTER_DESCRIPTOR_SIZE: u64 = 4096;

/// One control block's local descriptor.
///
/// Found at the END of each control block (the C++ ref calls this
/// `LOCAL_BLOCK_DESCRIPTOR`). The block's compressed bytes start
/// `compsize` bytes BEFORE the descriptor offset.
#[derive(Debug, Clone)]
pub struct LocalDescriptor {
    /// Absolute file offset of the `ArC\x01` signature at the start
    /// of this descriptor.
    pub descriptor_pos: u64,
    pub block_type: u8,
    pub compressor: String,
    pub origsize: u64,
    pub compsize: u64,
    pub crc: u32,
    /// Absolute file offset where the block's compressed bytes start.
    /// Equal to `descriptor_pos - compsize`.
    pub block_pos: u64,
}

/// Standard control block type codes. From `BLOCKTYPE` enum
/// (`ArcStructure.h:49`).
pub mod block_type {
    pub const DESCR: u8 = 0;
    pub const HEADER: u8 = 1;
    pub const DATA: u8 = 2;
    pub const DIR: u8 = 3;
    pub const FOOTER: u8 = 4;
    pub const RECOVERY: u8 = 5;

    pub fn name(b: u8) -> &'static str {
        match b {
            0 => "DESCR",
            1 => "HEADER",
            2 => "DATA",
            3 => "DIR",
            4 => "FOOTER",
            5 => "RECOVERY",
            _ => "?",
        }
    }
}

/// Scan the last `MAX_FOOTER_DESCRIPTOR_SIZE` bytes for the FreeArc
/// signature and return the absolute file offset of the LAST one
/// found (the footer descriptor sits at the end).
pub fn find_footer_descriptor<R: Read + Seek>(r: &mut R) -> Result<u64> {
    let total = r.seek(SeekFrom::End(0))?;
    let win = total.min(MAX_FOOTER_DESCRIPTOR_SIZE);
    let start = total - win;
    r.seek(SeekFrom::Start(start))?;

    let mut buf = vec![0u8; win as usize];
    r.read_exact(&mut buf)?;

    // Walk backwards from the last position where a full 4-byte
    // signature still fits, looking for ArC\x01.
    if buf.len() < 4 {
        return Err(ArcError::NoFooterSignature);
    }
    for i in (0..=buf.len() - 4).rev() {
        if buf[i..i + 4] == SIGNATURE {
            return Ok(start + i as u64);
        }
    }
    Err(ArcError::NoFooterSignature)
}

/// Read a NUL-terminated ASCII string starting at `buf[*pos..]`.
fn read_cstring(buf: &[u8], pos: &mut usize) -> Result<String> {
    let start = *pos;
    while *pos < buf.len() && buf[*pos] != 0 {
        *pos += 1;
    }
    if *pos >= buf.len() {
        return Err(ArcError::Truncated);
    }
    let s = std::str::from_utf8(&buf[start..*pos])
        .map(|s| s.to_owned())
        .unwrap_or_else(|_| {
            // Tolerant decode: replace any non-UTF-8 bytes with U+FFFD.
            String::from_utf8_lossy(&buf[start..*pos]).into_owned()
        });
    *pos += 1; // consume the NUL
    Ok(s)
}

fn read_u32_le(buf: &[u8], pos: &mut usize) -> Result<u32> {
    if *pos + 4 > buf.len() {
        return Err(ArcError::Truncated);
    }
    let v = u32::from_le_bytes(buf[*pos..*pos + 4].try_into().unwrap());
    *pos += 4;
    Ok(v)
}

/// Parse a local descriptor starting at the given file offset. Reads
/// up to `MAX_FOOTER_DESCRIPTOR_SIZE` bytes from `descr_pos` forward.
pub fn read_local_descriptor<R: Read + Seek>(
    r: &mut R,
    descr_pos: u64,
) -> Result<LocalDescriptor> {
    let total = r.seek(SeekFrom::End(0))?;
    let avail = total - descr_pos;
    let want = avail.min(MAX_FOOTER_DESCRIPTOR_SIZE);
    r.seek(SeekFrom::Start(descr_pos))?;
    let mut buf = vec![0u8; want as usize];
    r.read_exact(&mut buf)?;

    let mut p = 0usize;
    let sign = read_u32_le(&buf, &mut p)?;
    if sign != SIGNATURE_LE {
        return Err(ArcError::NoFooterSignature);
    }

    let block_type_u = read_varint(&buf, &mut p)?;
    let block_type = block_type_u as u8;
    let compressor = read_cstring(&buf, &mut p)?;
    let origsize = read_varint(&buf, &mut p)?;
    let compsize = read_varint(&buf, &mut p)?;
    let crc = read_u32_le(&buf, &mut p)?;

    if compsize > descr_pos {
        return Err(ArcError::Truncated);
    }

    Ok(LocalDescriptor {
        descriptor_pos: descr_pos,
        block_type,
        compressor,
        origsize,
        compsize,
        crc,
        block_pos: descr_pos - compsize,
    })
}

/// Read the raw compressed bytes of the block this descriptor points
/// at.
pub fn read_block_bytes<R: Read + Seek>(
    r: &mut R,
    desc: &LocalDescriptor,
) -> Result<Vec<u8>> {
    r.seek(SeekFrom::Start(desc.block_pos))?;
    let mut buf = vec![0u8; desc.compsize as usize];
    r.read_exact(&mut buf)?;
    Ok(buf)
}
