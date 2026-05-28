//! Dispatch table: turn a `compressor` method string + compressed
//! bytes into raw decompressed bytes.
//!
//! Native decoders covered so far:
//!   - `storing`           memcpy
//!   - `lzma:*`            via lzma-rs with header synthesised from
//!                         the method-string parameters
//!   - `zstd[:N]`          via the zstd crate
//!
//! Planned next:
//!   - `srep:lzma`         via two-stage srep port + lzma
//!   - `lz4[:N]`           via lz4_flex
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
    if method == "zstd" || method.starts_with("zstd:") {
        return decompress_zstd(compressed, orig_size);
    }
    // Closed-source CLS plugins (lolzi, lolzx, lolly, lollypop, ...)
    // get routed through the wine-side helper if the caller has the
    // CELLAR_CLS_HOST + CELLAR_CLS_DIR env vars pointed at the host
    // exe and the cls-*.dll directory. The host invocation itself
    // returns UnsupportedCompressor when the env is missing, so the
    // caller still sees a clean "unsupported" outcome in that case.
    if crate::cls_host::looks_like_cls(method) {
        return crate::cls_host::decompress_via_host(method, compressed, orig_size);
    }
    Err(ArcError::UnsupportedCompressor(method.to_owned()))
}

/// FreeArc stores zstd-compressed blocks as standard zstd frames, so
/// we just hand the bytes to the zstd crate. The level token after
/// the colon (e.g. `zstd:22`) is encoder-only and irrelevant here.
fn decompress_zstd(compressed: &[u8], orig_size: u64) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(orig_size as usize);
    zstd::stream::copy_decode(compressed, &mut out)
        .map_err(|e| ArcError::UnsupportedCompressor(format!("zstd decode failed: {}", e)))?;
    Ok(out)
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

/// How `decompress` will handle a method string. Cheap predicate
/// (no decoding) suitable for UI badges that want to tell users
/// "we can decode this natively" vs "we can decode this if you set
/// up the wine helper" vs "no chance".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportLevel {
    /// Pure-Rust decoder, always works locally.
    Native,
    /// Needs the wine-side CLS plugin host plus a matching
    /// `cls-*.dll`. The user must set CELLAR_CLS_HOST and
    /// CELLAR_CLS_DIR for `decompress` to actually try this path.
    Hybrid,
    /// No path. `decompress` returns `UnsupportedCompressor`.
    Unsupported,
}

/// Classify a method string by how it would be handled. Cheap, no
/// decoding.
pub fn support_level(method: &str) -> SupportLevel {
    if method == "storing"
        || method == "lzma"
        || method.starts_with("lzma:")
        || method == "zstd"
        || method.starts_with("zstd:")
    {
        return SupportLevel::Native;
    }
    if crate::cls_host::looks_like_cls(method) {
        return SupportLevel::Hybrid;
    }
    SupportLevel::Unsupported
}

/// True when `decompress` will at least attempt this method string
/// (Native or Hybrid). Wrapper over `support_level` kept for
/// existing callers; new code should prefer the explicit enum.
pub fn is_supported(method: &str) -> bool {
    !matches!(support_level(method), SupportLevel::Unsupported)
}

/// CRC-32 (pkzip / IEEE 802.3 polynomial) over a byte slice. FreeArc
/// uses this everywhere it stores a CRC.
pub fn crc32(data: &[u8]) -> u32 {
    let mut h = Hasher::new();
    h.update(data);
    h.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storing_roundtrip() {
        let body = b"the quick brown fox";
        let out = decompress("storing", body, body.len() as u64).unwrap();
        assert_eq!(&out, body);
    }

    #[test]
    fn storing_size_mismatch_is_truncated() {
        let body = b"abc";
        let err = decompress("storing", body, 99).err().unwrap();
        assert!(matches!(err, ArcError::Truncated));
    }

    #[test]
    fn zstd_roundtrip() {
        let body = b"hello hello hello hello hello world world world world".repeat(8);
        let comp = zstd::stream::encode_all(body.as_slice(), 3).unwrap();
        let out = decompress("zstd", &comp, body.len() as u64).unwrap();
        assert_eq!(out, body);
        // method-string variants with level tokens should still work
        let out2 = decompress("zstd:22", &comp, body.len() as u64).unwrap();
        assert_eq!(out2, body);
    }

    #[test]
    fn unknown_codec_returns_unsupported() {
        let err = decompress("lolzi", b"", 0).err().unwrap();
        assert!(matches!(err, ArcError::UnsupportedCompressor(_)));
    }

    #[test]
    fn support_level_classifies_correctly() {
        assert_eq!(support_level("storing"), SupportLevel::Native);
        assert_eq!(support_level("lzma"), SupportLevel::Native);
        assert_eq!(support_level("lzma:mfbt4:d1m"), SupportLevel::Native);
        assert_eq!(support_level("zstd"), SupportLevel::Native);
        assert_eq!(support_level("zstd:22"), SupportLevel::Native);

        assert_eq!(support_level("lolzi"), SupportLevel::Hybrid);
        assert_eq!(support_level("lollypop:d1024"), SupportLevel::Hybrid);

        // Chains are not handled by the single-codec hybrid path yet.
        assert_eq!(support_level("srep+dispack+lollypop"), SupportLevel::Unsupported);
        assert_eq!(support_level(""), SupportLevel::Unsupported);
    }

    #[test]
    fn is_supported_matches_dispatch() {
        // Native codecs.
        assert!(is_supported("storing"));
        assert!(is_supported("lzma:mfbt4:d1m"));
        assert!(is_supported("zstd:22"));
        // Hybrid codecs are reported supported even if the host env
        // isn't set; runtime will surface the missing env later.
        assert!(is_supported("lolzi"));
        // Truly unknown.
        assert!(!is_supported(""));
        assert!(!is_supported("totally_made_up_codec_xyz"));
    }
}
