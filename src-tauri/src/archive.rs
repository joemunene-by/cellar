//! Native archive inspection commands.
//!
//! FitGirl repacks ship as `fg-*.bin` FreeArc archives that the
//! installer unpacks via `unarc.dll` at install time. The native
//! reader (sibling crate `cellar-freearc-native`) parses the same
//! container format end-to-end in pure Rust, so the GUI can peek
//! inside the archive WITHOUT running the installer.
//!
//! Today we expose one command, `archive_peek`, that returns the
//! file table and the codec list. Future commands (extract per
//! file, integrity check) can be added without touching the wine
//! pipeline.

use std::fs::File;
use std::path::PathBuf;

use cellar_freearc_native::{
    decompress::is_supported,
    dir,
    error::ArcError,
    footer::block_type,
    open as fa_open,
    read_control_block,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArchiveError {
    NotFound { path: String },
    NotFreeArc { detail: String },
    ReadFailed { detail: String },
}

#[derive(Serialize)]
pub struct PeekFile {
    pub path: String,
    pub size: u64,
    /// CRC-32 (pkzip polynomial) the directory entry claims.
    pub crc: u32,
    pub is_dir: bool,
}

#[derive(Serialize)]
pub struct PeekCodec {
    pub method: String,
    /// True when this crate has a native Rust decoder for the codec
    /// chain. When false, the file bytes can only be retrieved via
    /// the wine + cls-*.dll hybrid path.
    pub supported_natively: bool,
}

#[derive(Serialize)]
pub struct ArchivePeek {
    pub archive_path: String,
    pub archive_bytes: u64,
    pub file_count: u64,
    pub total_uncompressed_bytes: u64,
    pub codecs: Vec<PeekCodec>,
    /// Set to a non-empty value when at least one DIR block uses a
    /// codec we cannot decode. The peek itself still works because
    /// DIR blocks in real FitGirl archives use lzma, not lollypop.
    pub partial_reason: Option<String>,
    pub files: Vec<PeekFile>,
}

#[tauri::command]
pub fn archive_peek(path: String) -> Result<ArchivePeek, ArchiveError> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(ArchiveError::NotFound { path });
    }
    let mut f = File::open(&p).map_err(|e| ArchiveError::ReadFailed { detail: e.to_string() })?;
    let archive_bytes = f.metadata().map(|m| m.len()).unwrap_or(0);

    let summary = match fa_open(&mut f) {
        Ok(s) => s,
        Err(ArcError::NoFooterSignature) => {
            return Err(ArchiveError::NotFreeArc {
                detail: "missing ArC\\x01 signature in last 4 KiB".into(),
            });
        }
        Err(e) => {
            return Err(ArchiveError::ReadFailed { detail: e.to_string() });
        }
    };

    let mut files: Vec<PeekFile> = Vec::new();
    let mut codecs_seen: Vec<String> = Vec::new();
    let mut partial_reason: Option<String> = None;
    let mut total_uncompressed: u64 = 0;

    // Footer's own codec counts too.
    push_codec_unique(&mut codecs_seen, summary.footer.compressor.clone());

    for entry in &summary.control_blocks {
        push_codec_unique(&mut codecs_seen, entry.compressor.clone());
        if entry.block_type != block_type::DIR {
            continue;
        }
        let raw = match read_control_block(&mut f, &summary.footer, entry) {
            Ok(v) => v,
            Err(ArcError::UnsupportedCompressor(s)) => {
                partial_reason = Some(format!(
                    "DIR block uses unsupported codec {}; file list may be incomplete",
                    s
                ));
                continue;
            }
            Err(e) => {
                return Err(ArchiveError::ReadFailed { detail: e.to_string() });
            }
        };
        let parsed = match dir::parse(&raw) {
            Ok(d) => d,
            Err(e) => {
                return Err(ArchiveError::ReadFailed {
                    detail: format!("DIR parse: {}", e),
                });
            }
        };
        for sb in &parsed.solid_blocks {
            push_codec_unique(&mut codecs_seen, sb.compressor.clone());
        }
        for fe in &parsed.files {
            total_uncompressed += fe.size;
            files.push(PeekFile {
                path: parsed.full_path(fe),
                size: fe.size,
                crc: fe.crc,
                is_dir: fe.is_dir,
            });
        }
    }

    let codecs: Vec<PeekCodec> = codecs_seen
        .into_iter()
        .map(|m| {
            let supported_natively = is_supported(&m);
            PeekCodec { method: m, supported_natively }
        })
        .collect();

    Ok(ArchivePeek {
        archive_path: path,
        archive_bytes,
        file_count: files.len() as u64,
        total_uncompressed_bytes: total_uncompressed,
        codecs,
        partial_reason,
        files,
    })
}

fn push_codec_unique(seen: &mut Vec<String>, method: String) {
    if !seen.iter().any(|s| s == &method) {
        seen.push(method);
    }
}
