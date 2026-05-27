//! CLI: open a FreeArc archive footer-first, decompress it, and list
//! every control block it points at.
//!
//! Usage:
//!   fg-arc-ls path/to/fg-05.bin
//!
//! Output looks like:
//!
//!   archive: fg-05.bin (9578531 bytes)
//!   footer:  pos=9578517 origsize=82 compsize=78 compressor=storing
//!   3 control blocks:
//!     type=HEADER  pos=...  size=... compressor=storing
//!     type=DIR     pos=...  size=... compressor=storing
//!     type=DATA    pos=...  size=... compressor=lzma:mfbt4:d1m
//!
//! Codec-agnostic for DIR/DATA blocks: we list them whether or not we
//! can decompress them. So `fg-arc-ls` works on ANY FitGirl archive,
//! including ones whose DATA blocks use the closed-source CLS plugins
//! we cannot decode natively.

use std::fs::File;
use std::path::PathBuf;
use std::process::ExitCode;

use cellar_freearc_native::{footer::block_type, open, type_label};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the FreeArc archive (.arc / fg-*.bin / etc.).
    archive: PathBuf,
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
    println!("archive: {} ({} bytes)", args.archive.display(), size);

    let summary = match open(&mut f) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("could not open archive footer: {}", e);
            return ExitCode::from(3);
        }
    };

    let footer = &summary.footer;
    let footer_type_ok = footer.block_type == block_type::FOOTER;
    println!(
        "footer:  pos={} origsize={} compsize={} compressor={:?} type=0x{:02x} ({}){}",
        footer.descriptor_pos,
        footer.origsize,
        footer.compsize,
        footer.compressor,
        footer.block_type,
        type_label(footer.block_type),
        if footer_type_ok { "" } else { "  WARN: not a FOOTER block!" },
    );

    println!("\n{} control block(s):", summary.control_blocks.len());
    println!(
        "  {:<9} {:>14} {:>14} {:>14}  compressor",
        "type", "abs_pos", "origsize", "compsize"
    );
    let footer_pos = summary.footer.block_pos;
    for b in &summary.control_blocks {
        // FreeArc encodes positions as "distance going BACK from the
        // footer" (all blocks are at smaller file offsets than the
        // footer; encoding the gap as a positive varint keeps the
        // numbers small for big archives). So the absolute position
        // is footer_pos - rel_pos. See BLOCK_DESCRIPTOR notes in the
        // C++ reference.
        let abs_pos = footer_pos.checked_sub(b.rel_pos).unwrap_or(footer_pos);
        println!(
            "  {:<9} {:>14} {:>14} {:>14}  {}",
            type_label(b.block_type),
            abs_pos,
            b.origsize,
            b.compsize,
            b.compressor
        );
    }

    if !summary.trailer.is_empty() {
        println!(
            "\nfooter trailer: {} bytes (archive metadata: lock flag, recovery settings, comment)",
            summary.trailer.len()
        );
        let preview = &summary.trailer[..summary.trailer.len().min(64)];
        println!("  first {} bytes: {:02x?}", preview.len(), preview);
    }

    ExitCode::SUCCESS
}
