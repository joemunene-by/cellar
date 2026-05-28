//! Minimal FreeArc archive writer.
//!
//! Writes single-solid-block archives using the `storing` codec only.
//! Enough to round-trip-test the reader without depending on FitGirl
//! bins or any compressor — `cargo test` can build an archive in
//! memory, hand it to `extract_dir_block`, and verify the bytes come
//! back identical.
//!
//! Scope today:
//!   - one HEADER block (8 bytes of magic + version)
//!   - one DATA solid block (storing: files concatenated verbatim)
//!   - one DIR block (storing, single solid block, all files in one
//!     flat directory)
//!   - one FOOTER block (storing, AOS list of HEADER + DIR)
//!   - 4-byte CRC at the end of every local block descriptor
//!
//! Out of scope: compression on output blocks, multiple solid blocks,
//! nested directories with non-empty dir-name list (always one empty
//! dir entry for now), arbitrary file mtimes (set to 0).

use std::io::Write;

use crate::decompress::crc32;
use crate::error::Result;
use crate::footer::{block_type, SIGNATURE};
use crate::varint::write_varint;

/// One file going into the archive.
pub struct InputFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Build a complete FreeArc archive into `w`. Layout:
///
///   HEADER body (8 bytes) | HEADER descriptor
///   DATA bytes (concat of all file bytes)
///   DIR body | DIR descriptor
///   FOOTER body | FOOTER descriptor
pub fn write_archive<W: Write>(w: &mut W, files: &[InputFile]) -> Result<()> {
    // --- HEADER block ---
    // "ArC\x01" + 4 bytes of version/flags (we use FreeArc's typical
    // 00 00 06 07 — taken from the fg-05.bin reference archive).
    let mut header_body = Vec::with_capacity(8);
    header_body.extend_from_slice(&SIGNATURE);
    header_body.extend_from_slice(&[0x00, 0x00, 0x06, 0x07]);
    let header_descriptor = build_local_descriptor(
        block_type::HEADER,
        "storing",
        header_body.len() as u64,
        header_body.len() as u64,
        crc32(&header_body),
    );
    w.write_all(&header_body)?;
    w.write_all(&header_descriptor)?;
    let header_end = (header_body.len() + header_descriptor.len()) as u64;

    // --- DATA solid block ---
    // storing: just concat the files. The DIR block describes how
    // many files live in this solid block + where it starts.
    let data_start = header_end;
    let mut data_bytes: Vec<u8> = Vec::new();
    for f in files {
        data_bytes.extend_from_slice(&f.bytes);
    }
    let data_compsize = data_bytes.len() as u64;
    w.write_all(&data_bytes)?;

    // --- DIR block ---
    // The DIR sits at file offset data_start + data_compsize.
    let dir_start = data_start + data_compsize;
    let dir_body = build_dir_body(files, dir_start - data_start);
    let dir_descriptor = build_local_descriptor(
        block_type::DIR,
        "storing",
        dir_body.len() as u64,
        dir_body.len() as u64,
        crc32(&dir_body),
    );
    w.write_all(&dir_body)?;
    w.write_all(&dir_descriptor)?;
    let dir_end = dir_start + (dir_body.len() + dir_descriptor.len()) as u64;

    // --- FOOTER block ---
    // AOS-encoded list of two control blocks (HEADER + DIR), then
    // the trailing 4 bytes (lock flag, recovery cmtlen, comment len, recovery cmtlen).
    // Positions are encoded as distance back from the FOOTER block start.
    let footer_start = dir_end;
    let mut footer_body: Vec<u8> = Vec::new();
    // n_blocks
    write_varint(&mut footer_body, 2);
    // block 0: HEADER
    write_varint(&mut footer_body, block_type::HEADER as u64);
    write_cstring(&mut footer_body, "storing");
    let header_rel = footer_start; // HEADER is at file offset 0
    write_varint(&mut footer_body, header_rel);
    write_varint(&mut footer_body, header_body.len() as u64);
    write_varint(&mut footer_body, header_body.len() as u64);
    footer_body.extend_from_slice(&crc32(&header_body).to_le_bytes());
    // block 1: DIR
    write_varint(&mut footer_body, block_type::DIR as u64);
    write_cstring(&mut footer_body, "storing");
    let dir_rel = footer_start - dir_start;
    write_varint(&mut footer_body, dir_rel);
    write_varint(&mut footer_body, dir_body.len() as u64);
    write_varint(&mut footer_body, dir_body.len() as u64);
    footer_body.extend_from_slice(&crc32(&dir_body).to_le_bytes());
    // trailer: arcLocked(1) + cmtlen-ucs4(varint=0) + rr_cstr("") + cmtlen(varint=0)
    footer_body.push(0); // not locked
    write_varint(&mut footer_body, 0); // comment length 0 (UCS4)
    write_cstring(&mut footer_body, ""); // empty recovery-record settings
    write_varint(&mut footer_body, 0); // trailing comment length 0

    let footer_descriptor = build_local_descriptor(
        block_type::FOOTER,
        "storing",
        footer_body.len() as u64,
        footer_body.len() as u64,
        crc32(&footer_body),
    );
    w.write_all(&footer_body)?;
    w.write_all(&footer_descriptor)?;
    Ok(())
}

/// Build a LOCAL_BLOCK_DESCRIPTOR: signature + type + compressor +
/// origsize + compsize + crc-of-data + crc-of-descriptor.
fn build_local_descriptor(
    block_ty: u8,
    compressor: &str,
    origsize: u64,
    compsize: u64,
    data_crc: u32,
) -> Vec<u8> {
    let mut d = Vec::with_capacity(64);
    d.extend_from_slice(&SIGNATURE);
    write_varint(&mut d, block_ty as u64);
    write_cstring(&mut d, compressor);
    write_varint(&mut d, origsize);
    write_varint(&mut d, compsize);
    d.extend_from_slice(&data_crc.to_le_bytes());
    // The descriptor's own CRC, computed over everything written so
    // far. unarc's openWithCRCAtEnd reads this trailing u32 and
    // checks it matches a CRC32 of all preceding bytes.
    let descr_crc = crc32(&d);
    d.extend_from_slice(&descr_crc.to_le_bytes());
    d
}

fn write_cstring(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(s.as_bytes());
    out.push(0);
}

/// Build the DIRECTORY block body. Single solid block with all input
/// files, all in the root directory (one empty dir-name entry).
fn build_dir_body(files: &[InputFile], dir_back_from_data_start: u64) -> Vec<u8> {
    let mut body: Vec<u8> = Vec::new();

    // Section 1: solid-block table (SOA, 1 block).
    write_varint(&mut body, 1); // n_solid
    write_varint(&mut body, files.len() as u64); // n_files[0]
    write_cstring(&mut body, "storing"); // compressors[0]
    write_varint(&mut body, dir_back_from_data_start); // offsets[0]
    let total_size: u64 = files.iter().map(|f| f.bytes.len() as u64).sum();
    write_varint(&mut body, total_size); // compsizes[0]

    // Section 2: directory names. Just one empty string for root.
    write_varint(&mut body, 1); // n_dirs
    write_cstring(&mut body, ""); // dirs[0] = ""

    // Section 3: file table (SOA).
    // name column
    for f in files {
        write_cstring(&mut body, &f.name);
    }
    // dir_indices column (all point at dirs[0])
    for _ in files {
        write_varint(&mut body, 0);
    }
    // sizes column
    for f in files {
        write_varint(&mut body, f.bytes.len() as u64);
    }
    // times column (u32 LE each, all zero)
    for _ in files {
        body.extend_from_slice(&0u32.to_le_bytes());
    }
    // isdir column (u8 each, all zero)
    for _ in files {
        body.push(0);
    }
    // crcs column (u32 LE each)
    for f in files {
        body.extend_from_slice(&crc32(&f.bytes).to_le_bytes());
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open;
    use std::io::Cursor;

    #[test]
    fn write_then_open_roundtrips_one_file() {
        let files = vec![InputFile {
            name: "hello.txt".into(),
            bytes: b"Hello, FreeArc!\n".to_vec(),
        }];
        let mut buf = Vec::new();
        write_archive(&mut buf, &files).expect("write");

        // Open it back through the real footer-first reader.
        let mut cursor = Cursor::new(&buf);
        let summary = open(&mut cursor).expect("open");
        assert_eq!(summary.control_blocks.len(), 2); // HEADER + DIR
        assert_eq!(summary.footer.block_type, block_type::FOOTER);
        assert_eq!(summary.footer.compressor, "storing");
    }

    #[test]
    fn write_then_extract_roundtrips_three_files() {
        let files = vec![
            InputFile { name: "a.txt".into(), bytes: b"alpha\n".to_vec() },
            InputFile { name: "b.txt".into(), bytes: b"beta beta beta\n".to_vec() },
            InputFile {
                name: "c.bin".into(),
                bytes: (0u8..=255).cycle().take(2000).collect(),
            },
        ];
        let mut buf = Vec::new();
        write_archive(&mut buf, &files).expect("write");

        let mut cursor = Cursor::new(&buf);
        let summary = open(&mut cursor).expect("open");

        // Find the DIR control block.
        let dir_entry = summary
            .control_blocks
            .iter()
            .find(|e| e.block_type == block_type::DIR)
            .expect("dir entry");

        // Read + parse DIR.
        let dir_raw = crate::read_control_block(&mut cursor, &summary.footer, dir_entry)
            .expect("read dir");
        let parsed = crate::dir::parse(&dir_raw).expect("parse dir");
        assert_eq!(parsed.solid_blocks.len(), 1);
        assert_eq!(parsed.files.len(), 3);
        assert_eq!(parsed.files[0].basename, "a.txt");
        assert_eq!(parsed.files[1].basename, "b.txt");
        assert_eq!(parsed.files[2].basename, "c.bin");
        assert_eq!(parsed.files[0].size, files[0].bytes.len() as u64);
        assert_eq!(parsed.files[2].size, files[2].bytes.len() as u64);
        // CRC the parser stored should match what we'd compute.
        for (parsed_f, input_f) in parsed.files.iter().zip(files.iter()) {
            assert_eq!(parsed_f.crc, crc32(&input_f.bytes));
        }

        // Now extract end-to-end and verify on-disk bytes.
        let dir_pos = summary
            .footer
            .block_pos
            .checked_sub(dir_entry.rel_pos)
            .expect("dir pos");
        let tmp = std::env::temp_dir().join(format!("fg-writer-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let (_outcomes, stats) =
            crate::extract::extract_dir_block(&mut cursor, &summary.footer, dir_pos, &parsed, &tmp)
                .expect("extract");
        assert_eq!(stats.wrote, 3);
        assert_eq!(stats.skipped_unsupported, 0);
        assert_eq!(stats.crc_mismatch, 0);

        for input in &files {
            let on_disk = std::fs::read(tmp.join(&input.name)).unwrap_or_else(|e| {
                panic!("read {}: {}", input.name, e);
            });
            assert_eq!(on_disk, input.bytes, "bytes diverged for {}", input.name);
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
