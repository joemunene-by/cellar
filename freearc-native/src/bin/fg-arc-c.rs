//! CLI: build a FreeArc archive from a directory using the `storing`
//! codec. Pair to fg-arc-x. Useful for round-trip testing and as a
//! reference implementation of the format.
//!
//! Usage:
//!   fg-arc-c <input-dir> <output.arc>
//!
//! Walks the input directory at one level (no recursion), stuffs
//! every regular file into a single solid `storing` block, writes a
//! valid FreeArc archive readable by `fg-arc-ls`, `fg-arc-files`,
//! and `fg-arc-x`.

use std::fs::File;
use std::io::{BufWriter, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use cellar_freearc_native::writer::{write_archive, InputFile};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Directory to archive (top-level files only — no recursion).
    input_dir: PathBuf,
    /// Output archive path.
    output: PathBuf,
}

fn main() -> ExitCode {
    let args = Args::parse();
    if !args.input_dir.is_dir() {
        eprintln!("not a directory: {}", args.input_dir.display());
        return ExitCode::from(2);
    }

    let mut files: Vec<InputFile> = Vec::new();
    let entries = match std::fs::read_dir(&args.input_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("could not read {}: {}", args.input_dir.display(), e);
            return ExitCode::from(3);
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_owned(),
            None => continue,
        };
        let mut bytes = Vec::new();
        if let Err(e) = File::open(&path).and_then(|mut f| f.read_to_end(&mut bytes)) {
            eprintln!("read {}: {}", path.display(), e);
            return ExitCode::from(4);
        }
        files.push(InputFile { name, bytes });
    }

    if files.is_empty() {
        eprintln!("no regular files in {}", args.input_dir.display());
        return ExitCode::from(5);
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));

    let out = match File::create(&args.output) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("could not create {}: {}", args.output.display(), e);
            return ExitCode::from(6);
        }
    };
    let mut w = BufWriter::new(out);
    if let Err(e) = write_archive(&mut w, &files) {
        eprintln!("write_archive: {}", e);
        return ExitCode::from(7);
    }

    let total_bytes: u64 = files.iter().map(|f| f.bytes.len() as u64).sum();
    println!(
        "wrote {} ({} files, {} original bytes)",
        args.output.display(),
        files.len(),
        total_bytes
    );
    ExitCode::SUCCESS
}
