//! Per-file extraction from FreeArc DATA solid blocks.
//!
//! A solid block is a single compressed stream containing N files
//! laid end-to-end. To pull file `k` out we decompress the entire
//! block, then slice the decompressed bytes by cumulative file size.
//! For archives where decompression is a single shot (lzma, zstd,
//! etc.) this is fine; streaming-by-file is a future optimisation.
//!
//! `origsize` for a solid block is NOT stored in the directory entry
//! (FreeArc's C++ code literally has `data_block[i].origsize = 0;
//! // do we need origsize here?`). We compute it as the sum of the
//! file sizes the directory block assigns to that solid block. The
//! `lzma:*` decoder needs that number, so we cannot skip this step.

use std::fs::create_dir_all;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::decompress::{crc32, decompress};
use crate::dir::DirBlock;
use crate::error::{ArcError, Result};
use crate::footer::LocalDescriptor;

/// What happened to one file during extraction.
#[derive(Debug, Clone)]
pub enum FileOutcome {
    /// Wrote the file (or created the directory).
    Wrote(PathBuf),
    /// Skipped because the file's solid block uses a codec we do not
    /// have a native decoder for. `detail` is the verbatim error
    /// string from the decompressor — for the CLS hybrid path this
    /// is where you find out whether the env was unset, wine was
    /// missing, the DLL failed to load, or ClsMain returned an error.
    SkippedUnsupported {
        path: PathBuf,
        compressor: String,
        detail: String,
    },
    /// Wrote the file but its CRC did not match the directory entry.
    /// The bytes are still on disk; the caller decides whether to
    /// keep or delete them.
    CrcMismatch {
        path: PathBuf,
        expected: u32,
        actual: u32,
    },
}

#[derive(Debug, Clone, Default)]
pub struct ExtractStats {
    pub wrote: usize,
    pub skipped_unsupported: usize,
    pub crc_mismatch: usize,
}

/// Extract every file the DIR block points at into `out_dir`.
///
/// `dir_block_pos` is the absolute offset of the DIR control block
/// (we need it to resolve solid-block positions, since FreeArc
/// stores those as distance back from the DIR's start).
///
/// Solid blocks whose codec we cannot decode are skipped wholesale
/// (every file inside them is reported as `SkippedUnsupported`).
pub fn extract_dir_block<R: Read + Seek>(
    r: &mut R,
    footer: &LocalDescriptor,
    dir_entry_pos_in_footer: u64,
    parsed: &DirBlock,
    out_dir: &Path,
) -> Result<(Vec<FileOutcome>, ExtractStats)> {
    let _ = footer; // kept for symmetry with read_control_block API
    let dir_block_pos = dir_entry_pos_in_footer;
    let mut outcomes = Vec::new();
    let mut stats = ExtractStats::default();
    let mut file_idx = 0usize;

    for solid in &parsed.solid_blocks {
        let n = solid.n_files as usize;
        let slice = &parsed.files[file_idx..file_idx + n];
        file_idx += n;

        // Sum file sizes in this solid block to get origsize for the
        // codec. Directories contribute 0, which is fine.
        let origsize: u64 = slice.iter().map(|f| f.size).sum();

        // Special-case: an empty solid block (compsize=0). Just emit
        // the directory entries it contains and move on; we don't
        // need to touch the codec at all.
        if solid.compsize == 0 {
            for fe in slice {
                let full = parsed.full_path(fe);
                let dest = out_dir.join(&full);
                if fe.is_dir {
                    create_dir_all(&dest)?;
                }
                outcomes.push(FileOutcome::Wrote(dest));
                stats.wrote += 1;
            }
            continue;
        }

        // Read the solid block's compressed bytes from disk.
        let abs_pos = dir_block_pos
            .checked_sub(solid.rel_pos)
            .ok_or(ArcError::Truncated)?;
        r.seek(SeekFrom::Start(abs_pos))?;
        let mut comp = vec![0u8; solid.compsize as usize];
        r.read_exact(&mut comp)?;

        // Decompress, or skip the whole block on UnsupportedCompressor.
        let raw = match decompress(&solid.compressor, &comp, origsize) {
            Ok(v) => v,
            Err(ArcError::UnsupportedCompressor(detail)) => {
                for fe in slice {
                    let full = parsed.full_path(fe);
                    outcomes.push(FileOutcome::SkippedUnsupported {
                        path: out_dir.join(&full),
                        compressor: solid.compressor.clone(),
                        detail: detail.clone(),
                    });
                    stats.skipped_unsupported += 1;
                }
                continue;
            }
            Err(e) => return Err(e),
        };

        // Walk file entries, slicing the raw buffer by file size.
        let mut cur = 0usize;
        for fe in slice {
            let full = parsed.full_path(fe);
            let dest = out_dir.join(&full);
            if fe.is_dir {
                create_dir_all(&dest)?;
                outcomes.push(FileOutcome::Wrote(dest));
                stats.wrote += 1;
                continue;
            }

            if let Some(parent) = dest.parent() {
                create_dir_all(parent)?;
            }

            let len = fe.size as usize;
            let end = cur + len;
            if end > raw.len() {
                return Err(ArcError::Truncated);
            }
            let body = &raw[cur..end];
            cur = end;

            let actual_crc = crc32(body);
            std::fs::write(&dest, body)?;

            if actual_crc != fe.crc {
                outcomes.push(FileOutcome::CrcMismatch {
                    path: dest,
                    expected: fe.crc,
                    actual: actual_crc,
                });
                stats.crc_mismatch += 1;
            } else {
                outcomes.push(FileOutcome::Wrote(dest));
                stats.wrote += 1;
            }
        }
    }

    Ok((outcomes, stats))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dir::{DirBlock, FileEntry, SolidBlock};
    use std::io::Cursor;

    /// Build an in-memory file containing one "storing" solid block
    /// with two known files glued together, then check that extract
    /// produces the right bytes and CRCs.
    #[test]
    fn extract_storing_two_files() {
        let a: &[u8] = b"hello, world\n";
        let b: &[u8] = b"goodbye, world\n";
        let crc_a = crc32(a);
        let crc_b = crc32(b);

        // Lay out: [a][b] starting at file offset 100. DIR block sits
        // at offset 200 (we never actually parse it here — extract
        // only needs the DIR block's absolute position).
        let mut file = vec![0u8; 200];
        file[100..100 + a.len()].copy_from_slice(a);
        file[100 + a.len()..100 + a.len() + b.len()].copy_from_slice(b);
        // pad file size to the dir block position
        file.resize(200, 0);

        let solid = SolidBlock {
            n_files: 2,
            compressor: "storing".to_owned(),
            rel_pos: 200 - 100, // dir_pos - abs_pos = 100
            compsize: (a.len() + b.len()) as u64,
        };
        let parsed = DirBlock {
            solid_blocks: vec![solid],
            dirs: vec![String::new()],
            files: vec![
                FileEntry {
                    basename: "a.txt".into(),
                    dir_index: 0,
                    size: a.len() as u64,
                    time: 0,
                    is_dir: false,
                    crc: crc_a,
                },
                FileEntry {
                    basename: "b.txt".into(),
                    dir_index: 0,
                    size: b.len() as u64,
                    time: 0,
                    is_dir: false,
                    crc: crc_b,
                },
            ],
        };

        let tmp = std::env::temp_dir().join(format!(
            "fg-arc-extract-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);

        // The footer is unused by extract_dir_block beyond the
        // function signature, so we just build a placeholder.
        let footer = LocalDescriptor {
            descriptor_pos: 0,
            block_type: 0,
            compressor: String::new(),
            origsize: 0,
            compsize: 0,
            crc: 0,
            block_pos: 0,
        };

        let mut cursor = Cursor::new(file);
        let (outcomes, stats) =
            extract_dir_block(&mut cursor, &footer, 200, &parsed, &tmp).expect("extract");

        assert_eq!(stats.wrote, 2);
        assert_eq!(stats.skipped_unsupported, 0);
        assert_eq!(stats.crc_mismatch, 0);

        let on_disk_a = std::fs::read(tmp.join("a.txt")).expect("read a");
        let on_disk_b = std::fs::read(tmp.join("b.txt")).expect("read b");
        assert_eq!(on_disk_a, a);
        assert_eq!(on_disk_b, b);

        // Verify outcomes are all Wrote.
        assert!(matches!(outcomes[0], FileOutcome::Wrote(_)));
        assert!(matches!(outcomes[1], FileOutcome::Wrote(_)));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
