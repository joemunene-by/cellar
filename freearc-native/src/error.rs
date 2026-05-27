//! Error types for the FreeArc reader.

use std::io;

#[derive(Debug)]
pub enum ArcError {
    Io(io::Error),
    /// The footer descriptor signature was not found in the last 4 KiB
    /// of the file. Either the file is not a FreeArc archive or it is
    /// truncated.
    NoFooterSignature,
    /// We expected more bytes than the file/buffer contained.
    Truncated,
    /// A control block claimed an unrecognised type byte.
    UnknownBlockType(u8),
    /// A control block descriptor or its body failed its CRC.
    CrcMismatch {
        what: &'static str,
        expected: u32,
        actual: u32,
    },
    /// The reader does not yet implement the compressor the archive
    /// uses for a control block (e.g. an `lzma` block before the LZMA
    /// codec is wired in, or a `lolzi` CLS block).
    UnsupportedCompressor(String),
}

impl std::fmt::Display for ArcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArcError::Io(e) => write!(f, "io: {}", e),
            ArcError::NoFooterSignature => {
                write!(f, "no FreeArc footer signature in last 4 KiB (not a FreeArc archive?)")
            }
            ArcError::Truncated => write!(f, "archive truncated (read past end)"),
            ArcError::UnknownBlockType(b) => write!(f, "unknown block type byte 0x{:02x}", b),
            ArcError::CrcMismatch { what, expected, actual } => {
                write!(f, "CRC mismatch on {}: expected 0x{:08x}, got 0x{:08x}", what, expected, actual)
            }
            ArcError::UnsupportedCompressor(s) => {
                write!(f, "unsupported compressor {:?} (no native decoder wired in yet)", s)
            }
        }
    }
}

impl std::error::Error for ArcError {}

impl From<io::Error> for ArcError {
    fn from(e: io::Error) -> Self {
        ArcError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, ArcError>;
