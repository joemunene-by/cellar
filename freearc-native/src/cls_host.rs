//! Hybrid path for closed-source FreeArc CLS plugins.
//!
//! When the dispatch table sees a method like `lolly:d2g` or
//! `lollypop:d1024` we cannot decode the bytes ourselves; the
//! algorithm lives inside `cls-*.dll`. We invoke the matching
//! plugin DLL via the `cellar-freearc-cls-host` PE32 helper running
//! under `wine`, piping compressed bytes in and reading decompressed
//! bytes out. This sidesteps `unarc.dll`'s wedged outer loop entirely
//! (which is the actual reason `freearc-shim` deadlocks on wine-on-Mac).
//!
//! Configuration is purely env-var driven, so callers can drop the
//! crate into any host environment:
//!
//!   CELLAR_CLS_HOST   absolute path to cellar-freearc-cls-host.exe
//!   CELLAR_CLS_DIR    directory containing the cls-*.dll plugins
//!   CELLAR_WINE       (optional) wine binary, default "wine"
//!
//! Both variables must be set; if either is missing we return an
//! `UnsupportedCompressor` error so callers can fall through to
//! whatever fallback they prefer (e.g. a UI hint to set them up).

use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::error::{ArcError, Result};

/// Codecs we know live in a `cls-*.dll` (and so are candidates for
/// the hybrid path). This is not exhaustive — any plugin that ships
/// with a `cls-<name>.dll` will work, but we only auto-dispatch the
/// ones FitGirl actually uses. List confirmed against a real
/// FitGirl install on 2026-05-28 (cls-*.dll set found in
/// drive_c/cellar-headless/).
const KNOWN_CLS_CODECS: &[&str] = &[
    "lolzi",
    "lolzx",
    "lolly",
    "lollypop",
    "lollypop2",
    "srep",
    "delta",
    "dispack",
    "dispack070",
    "msc",
];

/// True when `method` looks like a single CLS codec call (no chain)
/// for a plugin name we recognise. The caller can use this to gate
/// the more expensive `try_cls_host` invocation.
pub fn looks_like_cls(method: &str) -> bool {
    if method.contains('+') {
        // Chained method ("srep+dispack+lollypop"). Phase 6b will
        // handle these by walking the chain right-to-left; for now
        // we refuse them so callers fall back cleanly.
        return false;
    }
    let codec = split_codec(method).0;
    KNOWN_CLS_CODECS.iter().any(|c| *c == codec)
}

/// Split a FreeArc method string on its first `:` into
/// `(codec_name, params)`. `lolly:d2g:al1` -> `("lolly", "d2g:al1")`,
/// `delta` -> `("delta", "")`.
fn split_codec(method: &str) -> (&str, &str) {
    match method.split_once(':') {
        Some((codec, params)) => (codec, params),
        None => (method, ""),
    }
}

/// Run the CLS plugin host on a compressed buffer. Returns the
/// decompressed bytes. Pure spawn-and-pipe; the helper is a PE32
/// binary so it runs under `wine` (configurable).
pub fn decompress_via_host(method: &str, compressed: &[u8], _orig_size: u64) -> Result<Vec<u8>> {
    let (codec, params) = split_codec(method);

    let host_exe = env::var("CELLAR_CLS_HOST").map_err(|_| {
        ArcError::UnsupportedCompressor(format!(
            "{}: CELLAR_CLS_HOST not set (point at cellar-freearc-cls-host.exe to enable)",
            method
        ))
    })?;
    let dll_dir = env::var("CELLAR_CLS_DIR").map_err(|_| {
        ArcError::UnsupportedCompressor(format!(
            "{}: CELLAR_CLS_DIR not set (point at the directory containing cls-*.dll)",
            method
        ))
    })?;
    let wine_bin = env::var("CELLAR_WINE").unwrap_or_else(|_| "wine".to_owned());

    // FitGirl ships the cls-*.dll set with mixed casing
    // (e.g. cls-lolly.dll alongside CLS-srep.dll, CLS-MSC.dll).
    // macOS / HFS+ is case-insensitive so a lowercase lookup works,
    // but Linux is not. Try the casings we have actually seen in the
    // wild before giving up.
    let dir = PathBuf::from(&dll_dir);
    let candidates = [
        format!("cls-{}.dll", codec),
        format!("CLS-{}.dll", codec),
        format!("cls-{}.dll", codec.to_uppercase()),
        format!("CLS-{}.dll", codec.to_uppercase()),
    ];
    let dll_path = candidates
        .iter()
        .map(|name| dir.join(name))
        .find(|p| p.exists())
        .ok_or_else(|| {
            ArcError::UnsupportedCompressor(format!(
                "{}: no cls-{}.dll in CELLAR_CLS_DIR ({}), tried {:?}",
                method,
                codec,
                dll_dir,
                candidates
            ))
        })?;

    let mut child = Command::new(&wine_bin)
        .arg(&host_exe)
        .arg("--dll")
        .arg(&dll_path)
        .arg("--params")
        .arg(params)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            ArcError::UnsupportedCompressor(format!(
                "{}: failed to spawn {} {}: {}",
                method, wine_bin, host_exe, e
            ))
        })?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .expect("piped stdin should be present");
        stdin.write_all(compressed).map_err(|e| {
            ArcError::UnsupportedCompressor(format!("{}: pipe write failed: {}", method, e))
        })?;
    }
    // Closing stdin signals EOF to the plugin's read callback.
    drop(child.stdin.take());

    let output = child.wait_with_output().map_err(|e| {
        ArcError::UnsupportedCompressor(format!("{}: wait failed: {}", method, e))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(ArcError::UnsupportedCompressor(format!(
            "{}: cls host exited {} ({})",
            method,
            output.status.code().unwrap_or(-1),
            if stderr.is_empty() { "no stderr".to_owned() } else { stderr }
        )));
    }

    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_codec_basic() {
        assert_eq!(split_codec("lolly:d2g:al1"), ("lolly", "d2g:al1"));
        assert_eq!(split_codec("delta"), ("delta", ""));
        assert_eq!(split_codec(""), ("", ""));
    }

    #[test]
    fn looks_like_cls_recognises_fitgirl_codecs() {
        assert!(looks_like_cls("lolzi"));
        assert!(looks_like_cls("lolzx:al1"));
        assert!(looks_like_cls("lolly:d2g"));
        assert!(looks_like_cls("lollypop:d1024:al1"));
        assert!(looks_like_cls("srep:m3f:l256"));
    }

    #[test]
    fn looks_like_cls_rejects_open_codecs() {
        assert!(!looks_like_cls("storing"));
        assert!(!looks_like_cls("lzma:mfbt4:d1m"));
        assert!(!looks_like_cls("zstd:22"));
    }

    #[test]
    fn looks_like_cls_rejects_chains() {
        // Phase 6a handles single codec calls only.
        assert!(!looks_like_cls("srep+dispack+lollypop"));
        assert!(!looks_like_cls("lollypop+lzma"));
    }
}
