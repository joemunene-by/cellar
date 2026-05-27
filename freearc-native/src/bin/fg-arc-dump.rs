//! Debug CLI: decompress a chosen control block and hexdump its raw
//! bytes. Useful while bringing up parsers for new block layouts.

use std::fs::File;
use std::path::PathBuf;
use std::process::ExitCode;

use cellar_freearc_native::{footer::block_type, open, read_control_block, type_label};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    archive: PathBuf,
    /// Block type to dump: header, dir, footer.
    #[arg(long, default_value = "dir")]
    kind: String,
}

fn kind_to_byte(s: &str) -> Option<u8> {
    match s.to_ascii_lowercase().as_str() {
        "header" => Some(block_type::HEADER),
        "data" => Some(block_type::DATA),
        "dir" => Some(block_type::DIR),
        "footer" => Some(block_type::FOOTER),
        _ => None,
    }
}

fn main() -> ExitCode {
    let args = Args::parse();
    let want = match kind_to_byte(&args.kind) {
        Some(b) => b,
        None => {
            eprintln!("unknown --kind value (use header/data/dir/footer)");
            return ExitCode::from(2);
        }
    };

    let mut f = File::open(&args.archive).expect("open archive");
    let summary = open(&mut f).expect("open footer");

    if want == block_type::FOOTER {
        // Special-case: footer block is not in the control_blocks list
        // (it's the source of that list). Read it directly.
        let raw = cellar_freearc_native::footer::read_block_bytes(&mut f, &summary.footer)
            .expect("read footer block bytes");
        let dec = cellar_freearc_native::decompress::decompress(
            &summary.footer.compressor,
            &raw,
            summary.footer.origsize,
        )
        .expect("decompress footer");
        dump("FOOTER", &dec);
        return ExitCode::SUCCESS;
    }

    let mut found_any = false;
    for entry in &summary.control_blocks {
        if entry.block_type != want {
            continue;
        }
        found_any = true;
        let dec = read_control_block(&mut f, &summary.footer, entry)
            .expect("read+decompress control block");
        dump(type_label(entry.block_type), &dec);
    }
    if !found_any {
        eprintln!("no {} block found", args.kind);
        return ExitCode::from(4);
    }
    ExitCode::SUCCESS
}

fn dump(label: &str, raw: &[u8]) {
    println!("# {}: {} bytes decompressed", label, raw.len());
    for (i, chunk) in raw.chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{:02x}", b)).collect();
        let ascii: String = chunk
            .iter()
            .map(|&b| if (32..127).contains(&b) { b as char } else { '.' })
            .collect();
        println!("{:08x}  {:<48}  {}", i * 16, hex.join(" "), ascii);
    }
}
