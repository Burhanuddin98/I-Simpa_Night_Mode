//! GABE v2 tables ("Generic Array Binary Exchange"): the column tables I-Simpa's solvers write
//! for SPPS statistics, TCR main results, punctual receivers (`.recp`, `.recps`, `.gap`) and
//! intensity (`.rpi`). Read only.
//!
//! Spec, receipts into upstream's `gabe.cpp` and the canonical dump: `docs/formats/gabe.md`.
//!
//! Upstream's `formatGABE::GABE::Load` ignores every stream failure, so a truncated table loads
//! as "success" with zero-filled or stale columns. This reader reports truncation instead. The
//! padding bytes upstream's writer seeks over are the one exception: at the very end of the file
//! they may be absent, because upstream's writer never materialises a trailing seek (a table
//! with no columns is 17 bytes, and one ending in an empty integer or string column ends 2
//! bytes early).

use std::path::Path;

use super::{Cursor, FormatError, Result, f32_hex, str_token};

/// The only `formatVersion` this reader, and upstream's writer, implement (`gabe.cpp:43`).
pub const VERSION: i32 = 2;
/// `GABE_OBJECTTYPE_FLOAT` (`gabe.h:53`).
pub const TYPE_FLOAT: u16 = 50;
/// `GABE_OBJECTTYPE_INT` (`gabe.h:54`).
pub const TYPE_INT: u16 = 51;
/// `GABE_OBJECTTYPE_SHORTSTRING` (`gabe.h:55`).
pub const TYPE_SHORTSTRING: u16 = 52;
/// `STRING_LABEL_LENGTH`: bytes reserved for a column label (`gabe.h:58`).
pub const LABEL_LEN: usize = 255;
/// `sizeof(t_StringShort)`: bytes per cell of a short-string column (`gabe.h:230`).
pub const SHORT_STRING_LEN: usize = 50;
/// File header: four `int32`, the `readOnly` byte and 3 padding bytes.
pub const FILE_HEADER_LEN: usize = 20;
/// Column header: `uint16` type, 2 padding, `int32` rows, two `int64`, the label, 1 padding.
pub const COL_HEADER_LEN: usize = 280;
/// Fewest bytes a column can occupy: a last column whose trailing padding upstream's writer
/// never materialised. Used to bound the column count before reserving.
const MIN_COL_BYTES: usize = COL_HEADER_LEN - 1;

/// A GABE table, as upstream's `GABE` object holds it after `Load`.
#[derive(Debug, Clone, PartialEq)]
pub struct Gabe {
    /// `formatVersion` from the file header. Always [`VERSION`] after a successful read.
    pub version: i32,
    /// `readOnly`: whether I-Simpa's GUI lets the user edit the table.
    pub read_only: bool,
    /// The columns of a known type, in file order. Columns of any other type are skipped, as
    /// upstream skips them.
    pub columns: Vec<Column>,
}

/// One column: `GABE_Object` (label, type) and `GABE_Data` (header, rows).
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    /// The label up to its first NUL (at most [`LABEL_LEN`] bytes). Solvers write
    /// `name\runit`; old files are Latin-1, new ones UTF-8, so it is kept as bytes.
    pub label: Vec<u8>,
    pub data: ColumnData,
}

/// A column's type, type-specific header and rows.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnData {
    /// `GABE_Data_Float`: `t_HeaderFloat::numOfDigits`, then one `f32` per row.
    Float {
        num_of_digits: i32,
        values: Vec<f32>,
    },
    /// `GABE_Data_Integer`: one `i32` per row.
    Int(Vec<i32>),
    /// `GABE_Data_ShortString`: one 50-byte cell per row, each kept up to its first NUL.
    ShortString(Vec<Vec<u8>>),
}

impl ColumnData {
    /// Upstream's `GABE_OBJECTTYPE` code for this column.
    pub fn type_code(&self) -> u16 {
        match self {
            ColumnData::Float { .. } => TYPE_FLOAT,
            ColumnData::Int(_) => TYPE_INT,
            ColumnData::ShortString(_) => TYPE_SHORTSTRING,
        }
    }

    /// Number of rows (`GetSize()`).
    pub fn len(&self) -> usize {
        match self {
            ColumnData::Float { values, .. } => values.len(),
            ColumnData::Int(v) => v.len(),
            ColumnData::ShortString(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Column {
    /// The label before its `\r`, which solvers use to separate a name from its unit.
    pub fn name(&self) -> &[u8] {
        match self.label.iter().position(|&b| b == b'\r') {
            Some(i) => &self.label[..i],
            None => &self.label,
        }
    }

    /// The unit after the label's `\r`, if there is one.
    pub fn unit(&self) -> Option<&[u8]> {
        self.label
            .iter()
            .position(|&b| b == b'\r')
            .map(|i| &self.label[i + 1..])
    }

    pub fn floats(&self) -> Option<&[f32]> {
        match &self.data {
            ColumnData::Float { values, .. } => Some(values),
            _ => None,
        }
    }

    pub fn ints(&self) -> Option<&[i32]> {
        match &self.data {
            ColumnData::Int(v) => Some(v),
            _ => None,
        }
    }

    pub fn strings(&self) -> Option<&[Vec<u8>]> {
        match &self.data {
            ColumnData::ShortString(v) => Some(v),
            _ => None,
        }
    }
}

/// A per-band float column of a table whose first column labels the rows with the bands and a
/// final `Global` row, as TCR's main results do. `global` is kept apart: for a quantity with no
/// energetic sum TCR writes NaN there by design (`ctr/input_output/reportmanager.cpp:208`).
#[derive(Debug, Clone, PartialEq)]
pub struct BandSeries {
    pub bands: Vec<f32>,
    pub global: Option<f32>,
}

impl Gabe {
    /// The first column whose [`Column::name`] is `name`.
    pub fn column(&self, name: &[u8]) -> Option<&Column> {
        self.columns.iter().find(|c| c.name() == name)
    }

    /// The row labels: the first column, when it is a short-string column.
    pub fn row_labels(&self) -> Option<&[Vec<u8>]> {
        self.columns.first().and_then(Column::strings)
    }

    /// Index of the row labelled `label` in [`Gabe::row_labels`].
    pub fn row_index(&self, label: &[u8]) -> Option<usize> {
        self.row_labels()?.iter().position(|l| l == label)
    }

    /// The float column `name`, with the row labelled `Global` split off from the band rows.
    pub fn band_series(&self, name: &[u8]) -> Option<BandSeries> {
        let values = self.column(name)?.floats()?;
        let labels = self.row_labels()?;
        let mut bands = Vec::with_capacity(values.len());
        let mut global = None;
        for (i, &v) in values.iter().enumerate() {
            if labels.get(i).map(Vec::as_slice) == Some(b"Global".as_slice()) {
                global = Some(v);
            } else {
                bands.push(v);
            }
        }
        Some(BandSeries { bands, global })
    }
}

/// Skips `n` padding bytes that upstream's writer seeks over. A seek at the end of the file is
/// never written, so padding may run past the end; anything read after it then reports the
/// truncation.
fn skip_padding(c: &mut Cursor, n: usize) {
    let n = n.min(c.remaining());
    // Cannot fail: n is within what remains.
    let _ = c.skip(n);
}

/// Bytes up to the first NUL.
fn c_string(bytes: &[u8]) -> &[u8] {
    match bytes.iter().position(|&b| b == 0) {
        Some(i) => &bytes[..i],
        None => bytes,
    }
}

fn row_count(rows: i32) -> Result<usize> {
    usize::try_from(rows).map_err(|_| FormatError::Invalid(format!("gabe column has {rows} rows")))
}

/// Reads a table from the start of `bytes` and returns it with the number of bytes it occupies.
/// Bytes after the last column are ignored, as upstream ignores them.
pub fn read_prefix(bytes: &[u8]) -> Result<(Gabe, usize)> {
    let mut c = Cursor::new(bytes, "gabe");
    let version = c.i32()?;
    if version != VERSION {
        return Err(FormatError::Version {
            what: "gabe",
            found: version.to_string(),
            expected: "2",
        });
    }
    let _file_header_length = c.i32()?; // written 0 (or 20 by old writers); unused upstream
    let _col_header_length = c.i32()?; // written 0 (or 280); unused upstream
    let cols = c.i32()?;
    let read_only = match c.u8()? {
        0 => false,
        1 => true,
        b => return Err(FormatError::Invalid(format!("gabe readOnly byte is {b}"))),
    };
    skip_padding(&mut c, 3);

    let cols = usize::try_from(cols)
        .map_err(|_| FormatError::Invalid(format!("gabe header declares {cols} columns")))?;
    c.check_count(cols, MIN_COL_BYTES)?;
    let mut columns = Vec::with_capacity(cols);
    for _ in 0..cols {
        let col_type = c.u16()?;
        skip_padding(&mut c, 2);
        let rows = c.i32()?;
        let size_of_col = c.i64()?;
        let _size_of_header = c.i64()?; // sizeof(t_HeaderFloat)=4 or sizeof(t_NoStruct)=1; unused
        let label = c_string(c.bytes(LABEL_LEN)?).to_vec();
        skip_padding(&mut c, 1);
        let data = match col_type {
            TYPE_FLOAT => {
                let rows = row_count(rows)?;
                let num_of_digits = c.i32()?;
                let raw = c.bytes(c.check_count(rows, 4)? * 4)?;
                let values = raw
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|b| f32::from_le_bytes(*b))
                    .collect();
                ColumnData::Float {
                    num_of_digits,
                    values,
                }
            }
            TYPE_INT => {
                let rows = row_count(rows)?;
                skip_padding(&mut c, 1); // sizeof(t_NoStruct) header, never read
                let raw = c.bytes(c.check_count(rows, 4)? * 4)?;
                let values = raw
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|b| i32::from_le_bytes(*b))
                    .collect();
                ColumnData::Int(values)
            }
            TYPE_SHORTSTRING => {
                let rows = row_count(rows)?;
                skip_padding(&mut c, 1); // sizeof(t_NoStruct) header, never read
                let raw = c.bytes(c.check_count(rows, SHORT_STRING_LEN)? * SHORT_STRING_LEN)?;
                let values = raw
                    .as_chunks::<SHORT_STRING_LEN>()
                    .0
                    .iter()
                    .map(|s| c_string(s).to_vec())
                    .collect();
                ColumnData::ShortString(values)
            }
            _ => {
                // Upstream seeks over sizeofCol bytes and keeps nothing (gabe.cpp:426-434).
                let n = usize::try_from(size_of_col).map_err(|_| {
                    FormatError::Invalid(format!(
                        "gabe column of type {col_type} has size {size_of_col}"
                    ))
                })?;
                c.skip(n)?;
                continue;
            }
        };
        columns.push(Column { label, data });
    }
    Ok((
        Gabe {
            version,
            read_only,
            columns,
        },
        c.pos(),
    ))
}

/// Reads a table. Bytes after the last column are ignored, as upstream ignores them.
pub fn read(bytes: &[u8]) -> Result<Gabe> {
    read_prefix(bytes).map(|(g, _)| g)
}

pub fn read_file(path: &Path) -> Result<Gabe> {
    read(&super::read_file(path)?)
}

/// Canonical dump (grammar in `docs/formats/gabe.md`).
pub fn dump(value: &Gabe) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let _ = writeln!(out, "gabe {}", value.version);
    let _ = writeln!(out, "readonly {}", u8::from(value.read_only));
    let _ = writeln!(out, "columns {}", value.columns.len());
    for col in &value.columns {
        let label = str_token(&col.label);
        match &col.data {
            ColumnData::Float {
                num_of_digits,
                values,
            } => {
                let _ = writeln!(out, "column float {label}");
                let _ = writeln!(out, "digits {num_of_digits}");
                let _ = writeln!(out, "rows {}", values.len());
                for v in values {
                    let _ = writeln!(out, "{}", f32_hex(*v));
                }
            }
            ColumnData::Int(values) => {
                let _ = writeln!(out, "column int {label}");
                let _ = writeln!(out, "rows {}", values.len());
                for v in values {
                    let _ = writeln!(out, "{v}");
                }
            }
            ColumnData::ShortString(values) => {
                let _ = writeln!(out, "column shortstring {label}");
                let _ = writeln!(out, "rows {}", values.len());
                for v in values {
                    let _ = writeln!(out, "{}", str_token(v));
                }
            }
        }
    }
    out
}

fn error_kind(e: &FormatError) -> &'static str {
    match e {
        FormatError::NotFound(_) => "notfound",
        FormatError::Truncated { .. } => "truncated",
        FormatError::Version { .. } => "version",
        FormatError::Invalid(_) => "invalid",
        FormatError::Io(_) => "io",
    }
}

/// The canonical dump of a file, or `error <kind>\n`.
pub fn dump_file(path: &Path) -> String {
    match read_file(path) {
        Ok(g) => dump(&g),
        Err(e) => format!("error {}\n", error_kind(&e)),
    }
}
