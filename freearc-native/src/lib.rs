//! Pure-Rust reader for the FreeArc archive container format.
//!
//! FreeArc files (`.arc`, FitGirl's `fg-*.bin`) are split into blocks.
//! Each block starts with the 4-byte magic `ArC\x01` followed by a
//! type byte and (for compressed-data blocks) a NUL-terminated method
//! string describing the algorithm chain used on that block's
//! payload (e.g. `lzma:mfbt4:d1m`, `storing`, `srep:lzma:max`,
//! `lolzi`, `lolzx`, ...).
//!
//! This module implements a streaming SCAN (phase 1): it walks the
//! file from start to finish, finds every block header, and emits a
//! `BlockHeader` describing its type and method string. No
//! decompression yet.
//!
//! Phase 2 will add decoders for the open algorithms (`storing`,
//! `lzma`, `lz4`, `zstd`, `srep`) via Rust crates.
//!
//! Phase 3 will add a hybrid path that calls the closed-source
//! `cls-*.dll` plugins under wine for FitGirl-custom algorithms
//! (`lolzi`, `lolzx`, `lolly`, `lollypop`, `lollypop2`), bypassing
//! `unarc.dll`'s wedged state machine.

use std::io::{self, Read, Seek, SeekFrom};

pub const MAGIC: &[u8; 4] = b"ArC\x01";

/// One block header found in the archive.
#[derive(Debug, Clone)]
pub struct BlockHeader {
    /// Absolute byte offset of the leading `ArC\x01` magic.
    pub offset: u64,
    /// The byte immediately after the magic. Known values include:
    ///   0x00 archive header
    ///   0x02 method-declaration block ("storing", etc.)
    ///   0x06 compressed data block (method tag follows)
    ///   0x07 solid block
    ///   0x08 alternative compressed data
    /// Values outside this set are reported as-is; we do not refuse
    /// to scan unknown types.
    pub kind: u8,
    /// The NUL-terminated method string immediately after the kind
    /// byte. Empty when no string is present (e.g. archive-header
    /// blocks). UTF-8 decoded with replacement for non-UTF-8 bytes.
    pub method: String,
}

/// Scan the whole reader and return every `ArC\x01` block header
/// found, in file order. Brute-force linear scan; FreeArc archives
/// have no index telling us where the headers live, so this is the
/// honest first pass.
///
/// For a 10 MB archive this completes in milliseconds. For a 4 GB
/// archive it's I/O-bound on the read.
pub fn scan_blocks<R: Read + Seek>(reader: &mut R) -> io::Result<Vec<BlockHeader>> {
    reader.seek(SeekFrom::Start(0))?;

    // Read the whole file into memory. Reasonable up to a few GB on
    // modern hardware; for very large archives we will switch to a
    // sliding-window read in phase 2.
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;

    let mut out = Vec::new();
    let mut i = 0usize;
    while i + MAGIC.len() < buf.len() {
        if &buf[i..i + MAGIC.len()] == MAGIC {
            let kind = buf[i + 4];
            let method = read_cstring_at(&buf, i + 5);
            out.push(BlockHeader {
                offset: i as u64,
                kind,
                method,
            });
            // Skip past the magic; we still want to find further
            // magic markers inside data (the data itself never
            // contains `ArC\x01` because it would have been escaped
            // at compression time, so a naive scan is safe here).
            i += MAGIC.len();
        } else {
            i += 1;
        }
    }
    Ok(out)
}

/// Read a NUL-terminated string starting at offset `start` in `buf`.
/// Returns up to 256 bytes; if no NUL is found inside that window we
/// truncate. Non-UTF-8 bytes are replaced with `?`.
fn read_cstring_at(buf: &[u8], start: usize) -> String {
    let end = (start + 256).min(buf.len());
    let slice = &buf[start..end];
    let nul = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    let bytes = &slice[..nul];
    // Methods are restricted to ASCII printable in the FreeArc spec.
    // We do a tolerant decode so we surface anything that's clearly
    // not a real method string for the caller to inspect.
    bytes
        .iter()
        .map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '?' })
        .collect()
}

/// Group block headers by method string and return counts. Useful for
/// answering "what algorithms does this archive use".
pub fn method_histogram(blocks: &[BlockHeader]) -> Vec<(String, usize)> {
    let mut map: std::collections::BTreeMap<String, usize> = Default::default();
    for b in blocks {
        // Skip empty methods (archive-header blocks have no method).
        if b.method.is_empty() {
            continue;
        }
        *map.entry(b.method.clone()).or_default() += 1;
    }
    map.into_iter().collect()
}
