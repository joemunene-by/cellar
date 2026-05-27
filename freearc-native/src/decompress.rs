//! Dispatch table: turn a `compressor` method string + compressed
//! bytes into raw decompressed bytes.
//!
//! Native decoders covered so far:
//!   - `storing`           memcpy
//!
//! Planned next:
//!   - `lzma:*`            via lzma-rs with header synthesised from
//!                         the method-string parameters
//!   - `zstd`              via zstd crate
//!   - `srep:lzma`         via two-stage srep port + lzma
//!
//! Anything else returns `UnsupportedCompressor`. The caller can fall
//! back to the hybrid CLS-plugin-via-wine path (not in this crate).

use std::io::Cursor;

use crc32fast::Hasher;

use crate::error::{ArcError, Result};

/// Decompress `compressed` according to the FreeArc method string.
/// `orig_size` is the expected decompressed size from the descriptor.
pub fn decompress(method: &str, compressed: &[u8], orig_size: u64) -> Result<Vec<u8>> {
    if method == "storing" {
        if compressed.len() as u64 != orig_size {
            return Err(ArcError::Truncated);
        }
        return Ok(compressed.to_vec());
    }
    if let Some(params) = method.strip_prefix("lzma:") {
        return decompress_lzma(compressed, orig_size, params);
    }
    Err(ArcError::UnsupportedCompressor(method.to_owned()))
}

/// FreeArc strips the standard 13-byte LZMA header (5-byte properties
/// + 8-byte uncompressed size). We synthesise that header from the
/// method-string parameters and the descriptor-provided `orig_size`,
/// then hand the result to lzma-rs.
fn decompress_lzma(compressed: &[u8], orig_size: u64, params: &str) -> Result<Vec<u8>> {
    // Parse parameters like "mfbt4:d1m". Only the `d<size>` token
    // affects the decoder; the rest are encoder hints. Default dict
    // is 1 MiB. Default props (pb=2 lp=0 lc=3) = 0x5d.
    let mut dict_size: u32 = 1 << 20;
    let mut props: u8 = 0x5d;
    for tok in params.split(':') {
        if let Some(rest) = tok.strip_prefix('d') {
            dict_size = parse_size(rest).unwrap_or(dict_size);
        } else if let Some(rest) = tok.strip_prefix("lc") {
            if let Ok(lc) = rest.parse::<u8>() {
                props = (props - (props % 9)) + lc.min(8);
            }
        } else if let Some(rest) = tok.strip_prefix("lp") {
            if let Ok(lp) = rest.parse::<u8>() {
                // props = (pb*5 + lp)*9 + lc
                let lc = props % 9;
                let pb_lp = props / 9;
                let pb = pb_lp / 5;
                props = (pb * 5 + lp.min(4)) * 9 + lc;
            }
        } else if let Some(rest) = tok.strip_prefix("pb") {
            if let Ok(pb) = rest.parse::<u8>() {
                let lc = props % 9;
                let pb_lp = props / 9;
                let lp = pb_lp % 5;
                props = (pb.min(4) * 5 + lp) * 9 + lc;
            }
        }
        // mfbt4 / mfbt / fb / mc / etc. are encoder-only, ignored.
    }

    // Build a standard LZMA1 header in front of the compressed body.
    let mut wrapped = Vec::with_capacity(13 + compressed.len());
    wrapped.push(props);
    wrapped.extend_from_slice(&dict_size.to_le_bytes());
    wrapped.extend_from_slice(&orig_size.to_le_bytes());
    wrapped.extend_from_slice(compressed);

    let mut out = Vec::with_capacity(orig_size as usize);
    lzma_rs::lzma_decompress(&mut Cursor::new(&wrapped), &mut out)
        .map_err(|e| ArcError::UnsupportedCompressor(format!("lzma decode failed: {}", e)))?;
    Ok(out)
}

/// Parse a FreeArc size token like "1m", "256k", "16777216".
fn parse_size(s: &str) -> Option<u32> {
    if let Some(rest) = s.strip_suffix(['k', 'K']) {
        rest.parse::<u32>().ok().and_then(|n| n.checked_mul(1024))
    } else if let Some(rest) = s.strip_suffix(['m', 'M']) {
        rest.parse::<u32>().ok().and_then(|n| n.checked_mul(1024 * 1024))
    } else if let Some(rest) = s.strip_suffix(['g', 'G']) {
        rest.parse::<u32>()
            .ok()
            .and_then(|n| n.checked_mul(1024 * 1024 * 1024))
    } else {
        s.parse::<u32>().ok()
    }
}

/// CRC-32 (pkzip / IEEE 802.3 polynomial) over a byte slice. FreeArc
/// uses this everywhere it stores a CRC.
pub fn crc32(data: &[u8]) -> u32 {
    let mut h = Hasher::new();
    h.update(data);
    h.finalize()
}
