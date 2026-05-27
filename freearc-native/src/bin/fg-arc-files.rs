//! CLI: print the file table of a FreeArc archive.
//!
//! Usage:
//!   fg-arc-files path/to/fg-05.bin
//!
//! Walks every DIR control block in the archive, decompresses it,
//! parses the directory + file lists, and prints one line per file:
//!
//!   <size>  <crc>  <solid-block-index>  <path>
//!
//! When the DIR block uses a compressor we don't have a native
//! decoder for (e.g. one of the closed-source CLS plugins), the
//! tool prints that fact and continues with the next DIR block.

use std::fs::File;
use std::path::PathBuf;
use std::process::ExitCode;

use cellar_freearc_native::{
    dir, error::ArcError, footer::block_type, open, read_control_block, type_label,
};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the FreeArc archive.
    archive: PathBuf,

    /// Print the solid-block table alongside the file list.
    #[arg(long)]
    show_solids: bool,
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

    let mut total_files = 0u64;
    let mut total_orig = 0u64;
    let mut had_skip = false;

    for (i, entry) in summary.control_blocks.iter().enumerate() {
        if entry.block_type != block_type::DIR {
            continue;
        }
        println!(
            "# {} block #{}  compressor={}  origsize={}",
            type_label(entry.block_type),
            i,
            entry.compressor,
            entry.origsize
        );

        let raw = match read_control_block(&mut f, &summary.footer, entry) {
            Ok(v) => v,
            Err(ArcError::UnsupportedCompressor(s)) => {
                println!("  (skipped: {})", s);
                had_skip = true;
                continue;
            }
            Err(e) => {
                eprintln!("  (read failed: {})", e);
                had_skip = true;
                continue;
            }
        };

        let parsed = match dir::parse(&raw) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("  (parse failed: {})", e);
                had_skip = true;
                continue;
            }
        };

        if args.show_solids {
            println!("  solid blocks ({}):", parsed.solid_blocks.len());
            for (j, sb) in parsed.solid_blocks.iter().enumerate() {
                println!(
                    "    [{:>3}] n_files={:<5} compsize={:<14} compressor={}",
                    j, sb.n_files, sb.compsize, sb.compressor
                );
            }
            println!("  directories ({}):", parsed.dirs.len());
            for (j, d) in parsed.dirs.iter().enumerate() {
                println!("    [{:>3}] {}", j, d);
            }
            println!("  files ({}):", parsed.files.len());
        }

        let mut solid_idx = 0usize;
        let mut files_in_current_solid = 0u64;
        for fe in &parsed.files {
            while solid_idx < parsed.solid_blocks.len()
                && files_in_current_solid >= parsed.solid_blocks[solid_idx].n_files
            {
                solid_idx += 1;
                files_in_current_solid = 0;
            }
            let full = parsed.full_path(fe);
            let kind = if fe.is_dir { "d" } else { "-" };
            println!(
                "{} {:>14} {:08x} solid={:<3} {}",
                kind, fe.size, fe.crc, solid_idx, full
            );
            total_files += 1;
            if !fe.is_dir {
                total_orig += fe.size;
            }
            files_in_current_solid += 1;
        }
    }

    println!(
        "\n{} files, {} bytes original{}",
        total_files,
        total_orig,
        if had_skip { " (some DIR blocks skipped)" } else { "" }
    );

    ExitCode::SUCCESS
}
