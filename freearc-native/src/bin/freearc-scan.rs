//! CLI: scan a FreeArc archive and report every block header found.
//!
//! Usage:
//!   freearc-scan path/to/fg-05.bin
//!
//! Prints one line per block (offset, type byte, method string) plus
//! a summary histogram. This is the diagnostic precursor to a real
//! decoder: before we can extract anything, we have to know what
//! algorithms each block uses.

use std::fs::File;
use std::path::PathBuf;
use std::process::ExitCode;

use cellar_freearc_native::{method_histogram, scan_blocks};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the FreeArc archive (.arc / fg-*.bin / etc.).
    archive: PathBuf,

    /// Print every block, not just the summary histogram.
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

    let size = f.metadata().map(|m| m.len()).unwrap_or(0);
    println!("scanning {} ({} bytes)", args.archive.display(), size);

    let blocks = match scan_blocks(&mut f) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("scan failed: {}", e);
            return ExitCode::from(3);
        }
    };

    println!("found {} blocks", blocks.len());

    if args.verbose {
        for b in &blocks {
            println!(
                "  {:>10}  kind=0x{:02x}  method={:?}",
                b.offset, b.kind, b.method
            );
        }
    }

    let histo = method_histogram(&blocks);
    if histo.is_empty() {
        println!("(no method strings found; archive may be unusual or empty)");
    } else {
        println!("\nmethod histogram (sorted by name):");
        let name_width = histo.iter().map(|(n, _)| n.len()).max().unwrap_or(20);
        for (name, count) in &histo {
            println!("  {:<width$}  {} block(s)", name, count, width = name_width);
        }
    }

    // Block-kind histogram is also useful for understanding the
    // archive structure.
    let mut kind_counts: std::collections::BTreeMap<u8, usize> = Default::default();
    for b in &blocks {
        *kind_counts.entry(b.kind).or_default() += 1;
    }
    println!("\nblock-kind histogram:");
    for (kind, n) in kind_counts {
        let label = match kind {
            0x00 => "archive header",
            0x02 => "method declaration",
            0x06 => "compressed data",
            0x07 => "solid block",
            0x08 => "alt compressed data",
            _ => "(unknown)",
        };
        println!("  0x{:02x} {:<22}  {} block(s)", kind, label, n);
    }

    ExitCode::SUCCESS
}
