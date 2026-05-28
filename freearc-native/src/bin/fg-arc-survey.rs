//! CLI: walk a directory tree, find every FreeArc archive, classify
//! each by how cellar would handle it today.
//!
//! Usage:
//!   fg-arc-survey path/to/games-source
//!
//! For every file with the ArC\x01 footer signature, prints one
//! line summarising:
//!
//!   STATUS   classification, see below
//!   bytes    archive size on disk
//!   files    file count from the directory block
//!   codecs   the unique set of codecs used across DIR + DATA blocks
//!   path     relative path
//!
//! Classifications:
//!   NATIVE     every codec in the archive has a pure-Rust decoder
//!              shipped by this crate. fg-arc-x extracts fully on
//!              any platform without wine.
//!   HYBRID     at least one codec needs the wine + cls-*.dll path.
//!              On wine-on-Mac this currently deadlocks (see README
//!              "known issues"); on wine-on-Linux it usually works.
//!   BLOCKED    at least one codec is unknown to us entirely (no
//!              native decoder, not in the CLS plugin list).
//!   BROKEN     archive header parsed but the file table could not
//!              be read (probably truncated or corrupted).
//!
//! Ends with a one-line summary tally per status.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cellar_freearc_native::{
    decompress::{support_level, SupportLevel},
    dir,
    error::ArcError,
    footer::block_type,
    open as fa_open,
    read_control_block,
};
use clap::Parser;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Directory to walk (recursive).
    root: PathBuf,

    /// Print one line per file (default just prints a summary).
    #[arg(long)]
    verbose: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Native,
    Hybrid,
    Blocked,
    Broken,
}

impl Status {
    fn tag(&self) -> &'static str {
        match self {
            Status::Native => "NATIVE ",
            Status::Hybrid => "HYBRID ",
            Status::Blocked => "BLOCKED",
            Status::Broken => "BROKEN ",
        }
    }
}

fn classify_archive(path: &Path) -> Result<(Status, u64, Vec<String>), ArcError> {
    let mut f = File::open(path)?;
    let summary = fa_open(&mut f)?;

    let mut codecs: Vec<String> = Vec::new();
    let push_unique = |list: &mut Vec<String>, s: &str| {
        if !list.iter().any(|x| x == s) {
            list.push(s.to_owned());
        }
    };
    push_unique(&mut codecs, &summary.footer.compressor);
    for entry in &summary.control_blocks {
        push_unique(&mut codecs, &entry.compressor);
    }

    let mut file_count: u64 = 0;
    let mut broke_at_dir = false;
    for entry in &summary.control_blocks {
        if entry.block_type != block_type::DIR {
            continue;
        }
        let raw = match read_control_block(&mut f, &summary.footer, entry) {
            Ok(v) => v,
            Err(_) => {
                broke_at_dir = true;
                continue;
            }
        };
        let parsed = match dir::parse(&raw) {
            Ok(d) => d,
            Err(_) => {
                broke_at_dir = true;
                continue;
            }
        };
        file_count += parsed.files.len() as u64;
        for sb in &parsed.solid_blocks {
            push_unique(&mut codecs, &sb.compressor);
        }
    }

    if broke_at_dir && file_count == 0 {
        return Ok((Status::Broken, 0, codecs));
    }

    // Combine support levels across every codec.
    let mut status = Status::Native;
    for c in &codecs {
        match support_level(c) {
            SupportLevel::Native => {}
            SupportLevel::Hybrid => {
                if status == Status::Native {
                    status = Status::Hybrid;
                }
            }
            SupportLevel::Unsupported => {
                status = Status::Blocked;
            }
        }
    }
    Ok((status, file_count, codecs))
}

/// Quick "is this a FreeArc archive at all?" check: scan last 4 KiB
/// for ArC\x01. Fast, avoids invoking the full parser on every
/// non-archive .bin / .arc-named file in a games folder.
fn looks_like_freearc(path: &Path) -> bool {
    let mut f = match File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let size = match f.metadata() {
        Ok(m) => m.len(),
        Err(_) => return false,
    };
    if size < 4 {
        return false;
    }
    let window = size.min(4096);
    let skip = size - window;
    if f.seek(SeekFrom::Start(skip)).is_err() {
        return false;
    }
    let mut buf = vec![0u8; window as usize];
    if f.read_exact(&mut buf).is_err() {
        return false;
    }
    // Find ArC\x01 anywhere in the trailing window.
    buf.windows(4).any(|w| w == b"ArC\x01")
}

fn main() -> ExitCode {
    let args = Args::parse();
    if !args.root.exists() {
        eprintln!("no such path: {}", args.root.display());
        return ExitCode::from(2);
    }

    let mut native = 0usize;
    let mut hybrid = 0usize;
    let mut blocked = 0usize;
    let mut broken = 0usize;
    let mut total_native_bytes: u64 = 0;
    let mut total_hybrid_bytes: u64 = 0;
    let mut scanned = 0usize;

    println!(
        "{:<8}  {:>11}  {:>7}  {:<40}  path",
        "status", "bytes", "files", "codecs"
    );

    for entry in WalkDir::new(&args.root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let ext_ok = name.ends_with(".bin")
            || name.ends_with(".BIN")
            || name.ends_with(".arc")
            || name.ends_with(".ARC");
        if !ext_ok {
            continue;
        }
        if !looks_like_freearc(path) {
            continue;
        }

        scanned += 1;
        let size = path.metadata().map(|m| m.len()).unwrap_or(0);
        let rel = path.strip_prefix(&args.root).unwrap_or(path);

        match classify_archive(path) {
            Ok((status, files, codecs)) => {
                match status {
                    Status::Native => {
                        native += 1;
                        total_native_bytes += size;
                    }
                    Status::Hybrid => {
                        hybrid += 1;
                        total_hybrid_bytes += size;
                    }
                    Status::Blocked => blocked += 1,
                    Status::Broken => broken += 1,
                }
                let codecs_str = if codecs.is_empty() {
                    "(none)".to_owned()
                } else {
                    let mut s = codecs.join(",");
                    if s.len() > 40 {
                        s.truncate(37);
                        s.push_str("...");
                    }
                    s
                };
                println!(
                    "{}  {:>11}  {:>7}  {:<40}  {}",
                    status.tag(),
                    size,
                    files,
                    codecs_str,
                    rel.display()
                );
            }
            Err(e) => {
                broken += 1;
                println!(
                    "BROKEN   {:>11}  {:>7}  parser error: {}  {}",
                    size,
                    0,
                    e,
                    rel.display()
                );
            }
        }
    }

    println!();
    println!("=== survey ===");
    println!("scanned:  {} FreeArc archives", scanned);
    println!(
        "NATIVE:   {} archive(s), {} bytes  (fg-arc-x extracts these today)",
        native, total_native_bytes
    );
    println!(
        "HYBRID:   {} archive(s), {} bytes  (need wine + cls-*.dll; blocked on Mac)",
        hybrid, total_hybrid_bytes
    );
    println!("BLOCKED:  {} archive(s)  (unknown codec)", blocked);
    println!("BROKEN:   {} archive(s)  (corrupt or truncated)", broken);

    ExitCode::SUCCESS
}
