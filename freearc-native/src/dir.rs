//! Parser for the FreeArc DIRECTORY block.
//!
//! A DIR block sits immediately after the solid blocks it describes
//! and is split into three struct-of-arrays sections (see
//! `FreeArc-archive-format.md`):
//!
//!   1. Solid-block table: how many files, compressor, position,
//!      compressed size — one column per field.
//!   2. Directory name list: parent directory paths the file entries
//!      will reference by index.
//!   3. File table: base name, dir index, size, mtime, is-dir flag,
//!      CRC — one column per field.
//!
//! All numeric fields are varint except for `time` and `crc`, which
//! the format spec pins to 4 bytes each.

use crate::error::{ArcError, Result};
use crate::varint::read_varint;

#[derive(Debug, Clone)]
pub struct SolidBlock {
    pub n_files: u64,
    pub compressor: String,
    /// Offset relative to the start of the DIR block. Subtract from
    /// the DIR block's absolute position to get the solid block's
    /// absolute file offset (DIR sits AFTER its solid blocks).
    pub rel_pos: u64,
    pub compsize: u64,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub basename: String,
    /// Index into the parent DIR's `dirs` list. May be sentinel
    /// values that the C++ reference uses for "root" entries; we keep
    /// the raw number and let the caller decide.
    pub dir_index: u64,
    pub size: u64,
    /// FILETIME-ish 32-bit timestamp as FreeArc writes it. We pass it
    /// through; callers that need real time can decode it.
    pub time: u32,
    pub is_dir: bool,
    pub crc: u32,
}

#[derive(Debug, Clone)]
pub struct DirBlock {
    pub solid_blocks: Vec<SolidBlock>,
    pub dirs: Vec<String>,
    pub files: Vec<FileEntry>,
}

impl DirBlock {
    /// Resolve a file entry's full path by joining its parent dir
    /// (if any) with its basename. Returns just the basename when
    /// `dir_index` is out of range, which FreeArc uses for root-level
    /// entries.
    pub fn full_path(&self, f: &FileEntry) -> String {
        if (f.dir_index as usize) < self.dirs.len() {
            let parent = &self.dirs[f.dir_index as usize];
            if parent.is_empty() {
                f.basename.clone()
            } else {
                format!("{}/{}", parent, f.basename)
            }
        } else {
            f.basename.clone()
        }
    }
}

pub fn parse(raw: &[u8]) -> Result<DirBlock> {
    let mut p = 0usize;

    // --- Section 1: solid-block table (SOA) ---
    // Order matches DIRECTORY_BLOCK::DIRECTORY_BLOCK in ArcStructure.h:
    //   read(&num_of_blocks);
    //   read(num_of_blocks, &num_of_files);
    //   read(num_of_blocks, &compressors);
    //   read(num_of_blocks, &offsets);
    //   read(num_of_blocks, &compsizes);
    let n_solid = read_varint(raw, &mut p)? as usize;
    let n_files_per_solid: Vec<u64> = read_varint_column(raw, &mut p, n_solid)?;
    let compressors: Vec<String> = read_cstring_column(raw, &mut p, n_solid)?;
    let rel_pos: Vec<u64> = read_varint_column(raw, &mut p, n_solid)?;
    let compsize: Vec<u64> = read_varint_column(raw, &mut p, n_solid)?;

    let solid_blocks: Vec<SolidBlock> = (0..n_solid)
        .map(|i| SolidBlock {
            n_files: n_files_per_solid[i],
            compressor: compressors[i].clone(),
            rel_pos: rel_pos[i],
            compsize: compsize[i],
        })
        .collect();

    // total_files is NOT in the stream; it's the sum of per-solid counts.
    let total_files: usize = n_files_per_solid.iter().sum::<u64>() as usize;

    // --- Section 2: directory names (ARRAY: count + strings) ---
    let n_dirs = read_varint(raw, &mut p)? as usize;
    let dirs = read_cstring_column(raw, &mut p, n_dirs)?;

    // --- Section 3: file table (SOA) ---
    // Order: name, dir_numbers, size, time(u32 LE), isdir(u8), crc(u32 LE).
    let basenames = read_cstring_column(raw, &mut p, total_files)?;
    let dir_indices = read_varint_column(raw, &mut p, total_files)?;
    let sizes = read_varint_column(raw, &mut p, total_files)?;
    let times = read_u32_column(raw, &mut p, total_files)?;
    let is_dirs = read_u8_column(raw, &mut p, total_files)?;
    let crcs = read_u32_column(raw, &mut p, total_files)?;

    let files: Vec<FileEntry> = (0..total_files)
        .map(|i| FileEntry {
            basename: basenames[i].clone(),
            dir_index: dir_indices[i],
            size: sizes[i],
            time: times[i],
            is_dir: is_dirs[i] != 0,
            crc: crcs[i],
        })
        .collect();

    Ok(DirBlock { solid_blocks, dirs, files })
}

fn read_varint_column(buf: &[u8], pos: &mut usize, n: usize) -> Result<Vec<u64>> {
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(read_varint(buf, pos)?);
    }
    Ok(out)
}

fn read_cstring_column(buf: &[u8], pos: &mut usize, n: usize) -> Result<Vec<String>> {
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(read_cstring(buf, pos)?);
    }
    Ok(out)
}

fn read_u32_column(buf: &[u8], pos: &mut usize, n: usize) -> Result<Vec<u32>> {
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        if *pos + 4 > buf.len() {
            return Err(ArcError::Truncated);
        }
        let v = u32::from_le_bytes(buf[*pos..*pos + 4].try_into().unwrap());
        *pos += 4;
        out.push(v);
    }
    Ok(out)
}

fn read_u8_column(buf: &[u8], pos: &mut usize, n: usize) -> Result<Vec<u8>> {
    if *pos + n > buf.len() {
        return Err(ArcError::Truncated);
    }
    let out = buf[*pos..*pos + n].to_vec();
    *pos += n;
    Ok(out)
}

fn read_cstring(buf: &[u8], pos: &mut usize) -> Result<String> {
    let start = *pos;
    while *pos < buf.len() && buf[*pos] != 0 {
        *pos += 1;
    }
    if *pos >= buf.len() {
        return Err(ArcError::Truncated);
    }
    let s = String::from_utf8_lossy(&buf[start..*pos]).into_owned();
    *pos += 1;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal DIR block by hand: 1 solid block (storing,
    /// 2 files, rel_pos=10, compsize=20), 1 directory (empty root),
    /// 2 file entries.
    fn synth() -> Vec<u8> {
        let mut v = Vec::new();
        // n_solid = 1
        v.push(0x02); // varint 1
        // n_files_per_solid[0] = 2
        v.push(0x04); // varint 2
        // compressors[0] = "storing\0"
        v.extend_from_slice(b"storing\0");
        // offsets[0] = 10
        v.push(0x14); // varint 10
        // compsizes[0] = 20
        v.push(0x28); // varint 20
        // n_dirs = 1
        v.push(0x02);
        // dirs[0] = ""
        v.push(0x00);
        // basenames
        v.extend_from_slice(b"a.txt\0");
        v.extend_from_slice(b"b.txt\0");
        // dir_indices = [0, 0]
        v.push(0x00);
        v.push(0x00);
        // sizes = [100, 200]
        v.push(0xc8); // varint 100 = (100<<1)|0 = 0xc8
        v.push(0x90);
        v.push(0x01); // 200 = (200<<2)|0b01 = 0x321 → 0xc8 0x01? Let me redo
        // Actually 200 needs 2 bytes: value 200 << 2 | 0b01 = 801 = 0x321 → bytes 0x21 0x03
        // Fix sizes encoding:
        // (rewrite from start of sizes column)
        let sizes_start = v.len() - 3;
        v.truncate(sizes_start);
        v.push(0xc8); // 100 → 1 byte (200 << 1 in low bit form? 100<<1 = 200 = 0xc8). low bit 0 → value = 0xc8>>1 = 100. ✓
        v.push(0x21);
        v.push(0x03); // 200: (200<<2)|0b01 = 0x321 → LE bytes 0x21, 0x03
        // times[0..1] = u32 LE
        v.extend_from_slice(&1_000_000u32.to_le_bytes());
        v.extend_from_slice(&2_000_000u32.to_le_bytes());
        // isdir[0..1]
        v.push(0);
        v.push(0);
        // crcs[0..1]
        v.extend_from_slice(&0xdeadbeefu32.to_le_bytes());
        v.extend_from_slice(&0xcafebabeu32.to_le_bytes());
        v
    }

    #[test]
    fn parses_minimal_dir() {
        let raw = synth();
        let d = parse(&raw).expect("parse");
        assert_eq!(d.solid_blocks.len(), 1);
        assert_eq!(d.solid_blocks[0].n_files, 2);
        assert_eq!(d.solid_blocks[0].compressor, "storing");
        assert_eq!(d.solid_blocks[0].rel_pos, 10);
        assert_eq!(d.solid_blocks[0].compsize, 20);
        assert_eq!(d.dirs, vec![String::new()]);
        assert_eq!(d.files.len(), 2);
        assert_eq!(d.files[0].basename, "a.txt");
        assert_eq!(d.files[0].size, 100);
        assert_eq!(d.files[0].time, 1_000_000);
        assert_eq!(d.files[0].crc, 0xdeadbeef);
        assert_eq!(d.files[1].basename, "b.txt");
        assert_eq!(d.files[1].size, 200);
        assert_eq!(d.files[1].time, 2_000_000);
        assert_eq!(d.files[1].crc, 0xcafebabe);
    }
}
