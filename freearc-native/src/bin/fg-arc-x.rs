//! CLI: extract a FreeArc archive into a directory.
//!
//! Usage:
//!   fg-arc-x <archive> <output-dir>
//!
//! Walks every DIR control block, decompresses the solid blocks it
//! points at, and writes each file to disk under `<output-dir>`,
//! preserving directory structure.
//!
//! Solid blocks whose codec we do not have a native decoder for are
//! skipped (their files are listed but not written). Files written
//! to disk are verified against the directory's CRC-32 and reported
//! separately if they mismatch.

use std::fs::File;
use std::path::PathBuf;
use std::process::ExitCode;

use cellar_freearc_native::{
    dir, error::ArcError, extract::{extract_dir_block, FileOutcome},
    footer::block_type, open, read_control_block,
};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    archive: PathBuf,
    output_dir: PathBuf,
    /// Print every wrote/skip line, not just summary.
    #[arg(long)]
    verbose: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let mut f = match File::open(&args.archive) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("could not open {}: {}", args.archive.display(), e);
            return ExitCode::from(2);
        }
    };

    let summary = match open(&mut f) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("could not open archive footer: {}", e);
            return ExitCode::from(3);
        }
    };

    if let Err(e) = std::fs::create_dir_all(&args.output_dir) {
        eprintln!("could not create {}: {}", args.output_dir.display(), e);
        return ExitCode::from(4);
    }

    let footer_pos = summary.footer.block_pos;
    let mut totals = (0usize, 0usize, 0usize); // wrote, skipped, crc-mismatch
    let mut had_any_dir = false;

    for entry in &summary.control_blocks {
        if entry.block_type != block_type::DIR {
            continue;
        }
        had_any_dir = true;

        let dir_pos = footer_pos
            .checked_sub(entry.rel_pos)
            .unwrap_or(footer_pos);

        let raw = match read_control_block(&mut f, &summary.footer, entry) {
            Ok(v) => v,
            Err(ArcError::UnsupportedCompressor(s)) => {
                eprintln!("DIR block uses unsupported codec ({}), skipping", s);
                continue;
            }
            Err(e) => {
                eprintln!("DIR block read failed: {}", e);
                continue;
            }
        };

        let parsed = match dir::parse(&raw) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("DIR block parse failed: {}", e);
                continue;
            }
        };

        let (outcomes, stats) =
            match extract_dir_block(&mut f, &summary.footer, dir_pos, &parsed, &args.output_dir)
            {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("extract failed: {}", e);
                    return ExitCode::from(5);
                }
            };

        let mut last_skip_detail: Option<String> = None;
        for o in &outcomes {
            match o {
                FileOutcome::Wrote(p) => {
                    if args.verbose {
                        println!("wrote   {}", p.display());
                    }
                }
                FileOutcome::SkippedUnsupported { path, compressor, detail } => {
                    println!("skipped {}  (codec={})", path.display(), compressor);
                    // The detail string carries the actual cause from
                    // the decompressor. Print once per change (all
                    // files in a single solid block share one cause,
                    // so this collapses to one line per block).
                    if last_skip_detail.as_ref() != Some(detail) {
                        eprintln!("  detail: {}", detail);
                        last_skip_detail = Some(detail.clone());
                    }
                }
                FileOutcome::CrcMismatch { path, expected, actual } => {
                    println!(
                        "CRC!!   {}  (expected 0x{:08x}, got 0x{:08x})",
                        path.display(),
                        expected,
                        actual
                    );
                    last_skip_detail = None;
                }
            }
        }
        totals.0 += stats.wrote;
        totals.1 += stats.skipped_unsupported;
        totals.2 += stats.crc_mismatch;
    }

    if !had_any_dir {
        eprintln!("no DIR block found in archive");
        return ExitCode::from(6);
    }

    println!(
        "\n{} wrote, {} skipped (unsupported codec), {} CRC mismatch",
        totals.0, totals.1, totals.2
    );
    if totals.2 > 0 { ExitCode::from(7) } else { ExitCode::SUCCESS }
}
