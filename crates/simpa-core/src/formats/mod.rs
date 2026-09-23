//! Readers and writers for the files exchanged with I-Simpa's solvers.
//!
//! One module per format; each is specified in `docs/formats/<format>.md`, which also defines its
//! canonical dump. The canonical dump is the text the C++ oracle (`oracle/`, built from upstream's
//! own `lib_interface`) and these readers must produce identically. Readers never panic on bad
//! input and never allocate more than the input justifies: every count is checked against the
//! bytes remaining before anything is reserved.

use std::fmt;
use std::path::{Path, PathBuf};

pub mod cbin;
pub mod csbin;
pub mod gabe;
pub mod mbin;
pub mod pbin;
pub mod poly;
pub mod tetgen;

/// Why a file could not be read or written.
#[derive(Debug)]
pub enum FormatError {
    /// The file does not exist. Upstream's RSBIN reader reports success here; we never do.
    NotFound(PathBuf),
    /// The data ends before a field that must be present.
    Truncated { what: &'static str, offset: usize },
    /// A version field holds a value this reader does not support.
    Version {
        what: &'static str,
        found: String,
        expected: &'static str,
    },
    /// The data is present but violates the format (bad count, index out of range, bad text).
    Invalid(String),
    /// Any other I/O failure.
    Io(std::io::Error),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::NotFound(p) => write!(f, "file not found: {}", p.display()),
            FormatError::Truncated { what, offset } => {
                write!(f, "truncated {what}: data ends at byte {offset}")
            }
            FormatError::Version {
                what,
                found,
                expected,
            } => {
                write!(
                    f,
                    "unsupported {what} version {found} (expected {expected})"
                )
            }
            FormatError::Invalid(msg) => write!(f, "invalid data: {msg}"),
            FormatError::Io(e) => write!(f, "i/o error: {e}"),
        }
    }
}

impl FormatError {
    /// The failure kind as spelled in a canonical dump's `error <kind>` line.
    pub fn kind(&self) -> &'static str {
        match self {
            FormatError::NotFound(_) => "notfound",
            FormatError::Truncated { .. } => "truncated",
            FormatError::Version { .. } => "version",
            FormatError::Invalid(_) => "invalid",
            FormatError::Io(_) => "io",
        }
    }
}

impl std::error::Error for FormatError {}

impl From<std::io::Error> for FormatError {
    fn from(e: std::io::Error) -> Self {
        FormatError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, FormatError>;

/// Reads a whole file, reporting a missing file as [`FormatError::NotFound`].
pub fn read_file(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => FormatError::NotFound(path.to_path_buf()),
        _ => FormatError::Io(e),
    })
}

/// Bounds-checked little-endian reader over a byte slice.
pub struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
    what: &'static str,
}

impl<'a> Cursor<'a> {
    /// `what` names the format in truncation errors.
    pub fn new(buf: &'a [u8], what: &'static str) -> Self {
        Cursor { buf, pos: 0, what }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > self.remaining() {
            return Err(FormatError::Truncated {
                what: self.what,
                offset: self.buf.len(),
            });
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.bytes(n).map(|_| ())
    }

    pub fn seek(&mut self, pos: usize) -> Result<()> {
        if pos > self.buf.len() {
            return Err(FormatError::Truncated {
                what: self.what,
                offset: self.buf.len(),
            });
        }
        self.pos = pos;
        Ok(())
    }

    /// Validates that `count` records of `record_size` bytes fit in what is left, so a caller can
    /// reserve `count` elements without trusting a corrupt header.
    pub fn check_count(&self, count: usize, record_size: usize) -> Result<usize> {
        match count.checked_mul(record_size) {
            Some(total) if total <= self.remaining() => Ok(count),
            _ => Err(FormatError::Truncated {
                what: self.what,
                offset: self.buf.len(),
            }),
        }
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut a = [0u8; N];
        a.copy_from_slice(self.bytes(N)?);
        Ok(a)
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    pub fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.array()?))
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }
    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    pub fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_le_bytes(self.array()?))
    }
    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.array()?))
    }
    pub fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.array()?))
    }
}

/// Canonical-dump spelling of an `f32`: its raw bits as 8 lowercase hex digits.
/// Bit patterns make the Rust and C++ dumps comparable exactly, NaN and -0.0 included.
pub fn f32_hex(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}

/// Canonical-dump spelling of an `f64`: its raw bits as 16 lowercase hex digits.
pub fn f64_hex(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

/// Canonical-dump spelling of a string: bytes outside printable ASCII, backslash and space are
/// written as `\xHH`, so every dump token is a single whitespace-free word.
pub fn str_token(s: &[u8]) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s {
        if b.is_ascii_graphic() && b != b'\\' {
            out.push(b as char);
        } else {
            out.push_str(&format!("\\x{b:02x}"));
        }
    }
    if out.is_empty() {
        "\\x".to_string()
    } else {
        out
    }
}
