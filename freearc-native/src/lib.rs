//! Pure-Rust reader for the FreeArc archive container format.
//!
//! Phase 1 (done) shipped a naive `ArC\x01` scanner. That turned out
//! to misread the format: the kind bytes I saw at scan time were
//! actually varint width markers, not block-type codes. Real FreeArc
//! parsing goes footer-first: find the footer's local descriptor in
//! the last 4 KiB of the file, decode it, follow it back to the
//! footer block, decompress that block to get a list of every other
//! control block in the archive (header + dir blocks).
//!
//! This module implements that footer-first reader. The full spec is
//! in `FreeArc-archive-format.md` (kept next to this crate's
//! `Cargo.toml`) and the canonical C++ reference is `ArcStructure.h`
//! from the `xredor/unarc` project.
//!
//! Honest scope: the parser shape is universal, but the decompressor
//! dispatch table only covers `storing` today. LZMA, zstd, srep are
//! the next codecs. The closed-source CLS plugins (lolzi, lolzx,
//! lolly, lollypop) will route through a wine-side `cls-*.dll`
//! invocation in a separate crate when we get there.

pub mod decompress;
pub mod dir;
pub mod error;
pub mod footer;
pub mod varint;

use std::io::{Read, Seek};

use crate::error::Result;
use crate::footer::{block_type, find_footer_descriptor, read_local_descriptor, LocalDescriptor};

/// One control-block entry read out of the footer block.
#[derive(Debug, Clone)]
pub struct BlockEntry {
    pub block_type: u8,
    pub compressor: String,
    /// Distance (in bytes) BACK from the footer block's start to this
    /// block's compressed payload. Absolute offset is
    /// `footer.block_pos - rel_pos`. FreeArc encodes positions this
    /// way so the varint stays small even for multi-GB archives.
    pub rel_pos: u64,
    pub origsize: u64,
    pub compsize: u64,
    pub crc: u32,
}

/// What we found in an archive at the footer level: the footer's own
/// descriptor and the list of control blocks it points at.
#[derive(Debug, Clone)]
pub struct ArchiveSummary {
    pub footer: LocalDescriptor,
    pub control_blocks: Vec<BlockEntry>,
    /// Raw decompressed footer bytes after the control-block list,
    /// containing the archive-locked flag, recovery-record settings,
    /// and any UTF-8 comment. Returned verbatim for now.
    pub trailer: Vec<u8>,
}

/// First step: find and parse the footer descriptor. Cheap, never fails
/// on compressor support (we are not decompressing yet).
pub fn open_descriptor<R: Read + Seek>(r: &mut R) -> Result<LocalDescriptor> {
    let descr_pos = find_footer_descriptor(r)?;
    read_local_descriptor(r, descr_pos)
}

/// Open a FreeArc archive, find the footer, and return what it points at.
///
/// This works for ANY FreeArc archive regardless of the compressors
/// used in DIR / DATA blocks (we do not decompress those here). It
/// only fails when the FOOTER block itself uses a compressor we have
/// not wired in yet.
pub fn open<R: Read + Seek>(r: &mut R) -> Result<ArchiveSummary> {
    let footer = open_descriptor(r)?;

    // Pull the footer block's compressed bytes, decompress.
    let comp = crate::footer::read_block_bytes(r, &footer)?;
    let raw = crate::decompress::decompress(&footer.compressor, &comp, footer.origsize)?;

    // Parse the decompressed footer as: list of BLOCK_DESCRIPTOR
    // records, then trailing archive-metadata bytes.
    let mut p = 0usize;
    let n_blocks = varint::read_varint(&raw, &mut p)? as usize;
    let mut control_blocks = Vec::with_capacity(n_blocks);
    for _ in 0..n_blocks {
        let block_type = varint::read_varint(&raw, &mut p)? as u8;
        let compressor = read_cstring(&raw, &mut p)?;
        let rel_pos = varint::read_varint(&raw, &mut p)?;
        let origsize = varint::read_varint(&raw, &mut p)?;
        let compsize = varint::read_varint(&raw, &mut p)?;
        let crc = read_u32_le(&raw, &mut p)?;
        control_blocks.push(BlockEntry {
            block_type,
            compressor,
            rel_pos,
            origsize,
            compsize,
            crc,
        });
    }

    let trailer = raw[p..].to_vec();

    Ok(ArchiveSummary { footer, control_blocks, trailer })
}

fn read_cstring(buf: &[u8], pos: &mut usize) -> Result<String> {
    let start = *pos;
    while *pos < buf.len() && buf[*pos] != 0 {
        *pos += 1;
    }
    if *pos >= buf.len() {
        return Err(crate::error::ArcError::Truncated);
    }
    let s = String::from_utf8_lossy(&buf[start..*pos]).into_owned();
    *pos += 1;
    Ok(s)
}

fn read_u32_le(buf: &[u8], pos: &mut usize) -> Result<u32> {
    if *pos + 4 > buf.len() {
        return Err(crate::error::ArcError::Truncated);
    }
    let v = u32::from_le_bytes(buf[*pos..*pos + 4].try_into().unwrap());
    *pos += 4;
    Ok(v)
}

/// Convenience: render the block-type code as a 6-character label.
pub fn type_label(t: u8) -> &'static str {
    block_type::name(t)
}

/// Read a control block's compressed bytes from disk and decompress
/// them per its declared compressor.
pub fn read_control_block<R: Read + Seek>(
    r: &mut R,
    footer: &LocalDescriptor,
    entry: &BlockEntry,
) -> Result<Vec<u8>> {
    let abs_pos = footer
        .block_pos
        .checked_sub(entry.rel_pos)
        .ok_or(crate::error::ArcError::Truncated)?;
    use std::io::SeekFrom;
    r.seek(SeekFrom::Start(abs_pos))?;
    let mut buf = vec![0u8; entry.compsize as usize];
    r.read_exact(&mut buf)?;
    crate::decompress::decompress(&entry.compressor, &buf, entry.origsize)
}
