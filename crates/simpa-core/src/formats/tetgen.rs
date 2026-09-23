//! TetGen's mesh files, read only: `.node`, `.ele`, `.face` and `.neigh`, and the
//! `_skipped.node` / `_skipped.face` pair TetGen writes when a run fails on intersecting facets.
//!
//! Spec, receipts and the canonical dump: `docs/formats/tetgen.md`. The reference reader is
//! TetGen's own `tetgenio::load_node`, `load_tet` and `load_face` (TetGen 1.6.0, vendored at
//! upstream `src/tetgen/tetgen.cxx`); TetGen has no `.neigh` loader.
//!
//! **The rule this reader keeps:** whenever it accepts a file, it holds exactly the values
//! TetGen's loader holds for that file. Where TetGen would crash, guess or read something other
//! than what the file says (a missing field defaulted to 0, junk skipped, a leading `0` read as
//! octal, a line longer than its 2 047-byte `fgets` buffer split in two, extra records ignored),
//! this reader refuses with a reason instead. Each such case is listed in the spec.
//!
//! Each file is read on its own, and a file's kind comes from its extension, never its bytes:
//! an `.ele` and a `.neigh` can hold identical text. [`check_mesh`] checks one run's files
//! against each other.

use std::fmt::Write as _;
use std::path::Path;

use super::{FormatError, Result, f64_hex};

/// Names this format in truncation errors.
const WHAT: &str = "tetgen";

/// Longest line content TetGen reads in one piece. `readnumberline` calls
/// `fgets(buf, INPUTLINESIZE = 2048)`, which holds 2 047 bytes including the `\n`; a longer line
/// comes back in pieces, each read as a line of its own.
pub const MAX_LINE: usize = 2046;

/// Point count TetGen's corner check is run against when no `.node` is at hand: `INT_MAX - 1`,
/// the largest for which TetGen's `numberofpoints + firstnumber` cannot overflow.
pub const STANDALONE_POINTS: i64 = i32::MAX as i64 - 1;

/// The neighbour number `.neigh` holds for a face on the hull.
pub const HULL: i32 = -1;

/// Decoded records may take at most `2 * len + STORAGE_SLACK` bytes, which keeps a reader inside
/// the allocation budget of gate M2(c) (`2 * len + 64 KiB`) with room for everything else.
const STORAGE_SLACK: usize = 32 * 1024;

/// Which TetGen file a path holds. TetGen names its files by extension; the bytes do not say.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Node,
    Ele,
    Face,
    Neigh,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Node, Kind::Ele, Kind::Face, Kind::Neigh];

    /// The kind named by `path`'s extension (ASCII case ignored, as Windows opens files).
    pub fn from_path(path: &Path) -> Option<Kind> {
        let ext = path.extension()?.to_str()?;
        Kind::ALL
            .into_iter()
            .find(|k| ext.eq_ignore_ascii_case(k.extension()))
    }

    /// The extension TetGen writes for this kind, without the dot.
    pub fn extension(self) -> &'static str {
        match self {
            Kind::Node => "node",
            Kind::Ele => "ele",
            Kind::Face => "face",
            Kind::Neigh => "neigh",
        }
    }
}

/// A `.node` file: `tetgenio`'s point fields after `load_node`.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeFile {
    /// `firstnumber`: the first point's index, 0 or 1; 0 for a file with no points.
    pub first: i32,
    /// `mesh_dim`. Always 3: TetGen writes 3, and this reader refuses anything else.
    pub dim: i32,
    /// `pointlist`: x, y, z of each point, in file order.
    pub points: Vec<[f64; 3]>,
    /// `numberofpointattributes`.
    pub attributes_per_point: usize,
    /// `pointattributelist`: `attributes_per_point` values per point, point after point.
    pub attributes: Vec<f64>,
    /// `pointmarkerlist`: one marker per point, present when the header's marker flag is nonzero.
    pub markers: Option<Vec<i32>>,
}

/// An `.ele` file: `tetgenio`'s tetrahedron fields after `load_tet`.
#[derive(Clone, Debug, PartialEq)]
pub struct EleFile {
    /// The first record's index, 0 or 1 (TetGen takes `firstnumber` from the `.node`).
    pub first: i32,
    /// `numberofcorners`: 4, or 10 for a second-order (`-o2`) mesh.
    pub corners: usize,
    /// `tetrahedronlist`: `corners` point numbers per tetrahedron, as written (base `first`).
    pub tets: Vec<i32>,
    /// `numberoftetrahedronattributes`: 1 with `-A`, whose attribute is the region number.
    pub attributes_per_tet: usize,
    /// `tetrahedronattributelist`: `attributes_per_tet` values per tetrahedron.
    pub attributes: Vec<f64>,
}

impl EleFile {
    /// Number of tetrahedra.
    pub fn len(&self) -> usize {
        self.tets.len().checked_div(self.corners).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.tets.is_empty()
    }

    /// Corners of tetrahedron `i` (0-based position in the file).
    pub fn tet(&self, i: usize) -> &[i32] {
        &self.tets[i * self.corners..(i + 1) * self.corners]
    }

    /// Attributes of tetrahedron `i`.
    pub fn tet_attributes(&self, i: usize) -> &[f64] {
        &self.attributes[i * self.attributes_per_tet..(i + 1) * self.attributes_per_tet]
    }
}

/// A `.face` or `_skipped.face` file: `tetgenio`'s triangle fields after `load_face`.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceFile {
    /// The first record's index, 0 or 1; 0 for a file with no faces.
    pub first: i32,
    /// `trifacelist`: three point numbers per triangle, as written (base `first`).
    pub faces: Vec<[i32; 3]>,
    /// `trifacemarkerlist`: present when the header's marker flag is nonzero and the file holds
    /// at least one face (TetGen allocates no marker list for an empty file). In a `.face` it is
    /// the facet marker; in a `_skipped.face` it is `(int) badface::key`, -1 in every row seen.
    pub markers: Option<Vec<i32>>,
}

/// A `.neigh` file: four neighbours per tetrahedron, neighbour `j` across the face opposite
/// corner `j`, [`HULL`] (-1) where that face is on the hull.
#[derive(Clone, Debug, PartialEq)]
pub struct NeighFile {
    /// The first record's index, 0 or 1; neighbour numbers use the same base.
    pub first: i32,
    /// `neighborlist`, one row per tetrahedron in `.ele` order.
    pub neighbors: Vec<[i32; 4]>,
}

/// One TetGen file of any kind.
#[derive(Clone, Debug, PartialEq)]
pub enum TetgenFile {
    Node(NodeFile),
    Ele(EleFile),
    Face(FaceFile),
    Neigh(NeighFile),
}

impl TetgenFile {
    pub fn kind(&self) -> Kind {
        match self {
            TetgenFile::Node(_) => Kind::Node,
            TetgenFile::Ele(_) => Kind::Ele,
            TetgenFile::Face(_) => Kind::Face,
            TetgenFile::Neigh(_) => Kind::Neigh,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Text layer: lines and fields as TetGen's readnumberline / findnextnumber see them.

fn invalid(line: usize, msg: impl std::fmt::Display) -> FormatError {
    FormatError::Invalid(format!("tetgen line {line}: {msg}"))
}

fn truncated(len: usize) -> FormatError {
    FormatError::Truncated {
        what: WHAT,
        offset: len,
    }
}

/// Whole-file checks: bytes TetGen's text-mode C runtime reads differently, and lines TetGen's
/// line buffer would split.
fn check_text(buf: &[u8]) -> Result<()> {
    let mut line = 1;
    let mut start = 0;
    for (i, &b) in buf.iter().enumerate() {
        match b {
            0 => return Err(invalid(line, "NUL byte (TetGen's C strings end there)")),
            0x1a => return Err(invalid(line, "Ctrl-Z byte (a text-mode read stops there)")),
            b'\n' => {
                let mut len = i - start;
                if len > 0 && buf[i - 1] == b'\r' {
                    len -= 1;
                }
                if len > MAX_LINE {
                    return Err(invalid(
                        line,
                        format_args!("line longer than {MAX_LINE} bytes"),
                    ));
                }
                start = i + 1;
                line += 1;
            }
            _ => {}
        }
    }
    if buf.len() - start > MAX_LINE {
        return Err(invalid(
            line,
            format_args!("line longer than {MAX_LINE} bytes"),
        ));
    }
    Ok(())
}

/// A line holding data: `text` is the whole line, `body` its data from the first field.
struct Record<'a> {
    no: usize,
    text: &'a [u8],
    body: &'a [u8],
}

#[derive(Clone)]
struct Lines<'a> {
    buf: &'a [u8],
    pos: usize,
    no: usize,
}

impl<'a> Lines<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Lines { buf, pos: 0, no: 0 }
    }

    /// The next line without its `\n` or `\r\n` (a text-mode read drops the `\r`).
    fn next_line(&mut self) -> Option<(usize, &'a [u8])> {
        if self.pos >= self.buf.len() {
            return None;
        }
        let rest = &self.buf[self.pos..];
        let (content, advance) = match rest.iter().position(|&b| b == b'\n') {
            Some(i) => {
                let line = &rest[..i];
                (line.strip_suffix(b"\r").unwrap_or(line), i + 1)
            }
            None => (rest, rest.len()),
        };
        self.pos += advance;
        self.no += 1;
        Some((self.no, content))
    }

    /// The next line holding data. Blank lines (spaces and tabs only) and lines whose first
    /// non-blank byte is `#` are skipped; TetGen's `readnumberline` skips both.
    fn next_record(&mut self) -> Option<Record<'a>> {
        while let Some((no, text)) = self.next_line() {
            let Some(start) = text.iter().position(|&b| b != b' ' && b != b'\t') else {
                continue;
            };
            if text[start] != b'#' {
                return Some(Record {
                    no,
                    text,
                    body: &text[start..],
                });
            }
        }
        None
    }

    /// Data lines left, counted without allocating.
    fn count_records(&self) -> usize {
        let mut rest = self.clone();
        let mut n = 0;
        while rest.next_record().is_some() {
            n += 1;
        }
        n
    }
}

/// Field separators. TetGen's `findnextnumber` ends a field at a space, tab or comma.
fn is_sep(b: u8) -> bool {
    b == b' ' || b == b'\t' || b == b','
}

/// The fields of one data line, parsed strictly (see the module rule).
struct Fields<'a> {
    no: usize,
    rest: &'a [u8],
}

impl<'a> Fields<'a> {
    fn new(rec: &Record<'a>) -> Result<Self> {
        // load_node parses its header from the start of the line, where a comma is not skipped;
        // every other read skips it. Refuse it everywhere rather than track the difference.
        if rec.body.first() == Some(&b',') {
            return Err(invalid(rec.no, "line starts with a comma"));
        }
        let end = rec
            .body
            .iter()
            .position(|&b| b == b'#')
            .unwrap_or(rec.body.len());
        Ok(Fields {
            no: rec.no,
            rest: &rec.body[..end],
        })
    }

    fn next_token(&mut self) -> Option<&'a [u8]> {
        let start = self.rest.iter().position(|&b| !is_sep(b))?;
        let rest = &self.rest[start..];
        let end = rest.iter().position(|&b| is_sep(b)).unwrap_or(rest.len());
        self.rest = &rest[end..];
        Some(&rest[..end])
    }

    fn int(&mut self, what: &str) -> Result<i32> {
        match self.opt_int(what)? {
            Some(v) => Ok(v),
            None => Err(invalid(self.no, format_args!("missing {what}"))),
        }
    }

    fn opt_int(&mut self, what: &str) -> Result<Option<i32>> {
        let Some(tok) = self.next_token() else {
            return Ok(None);
        };
        parse_int(tok).map(Some).ok_or_else(|| self.bad(what, tok))
    }

    fn real(&mut self, what: &str) -> Result<f64> {
        let Some(tok) = self.next_token() else {
            return Err(invalid(self.no, format_args!("missing {what}")));
        };
        parse_real(tok).ok_or_else(|| self.bad(what, tok))
    }

    fn bad(&self, what: &str, tok: &[u8]) -> FormatError {
        let shown = &tok[..tok.len().min(24)];
        invalid(
            self.no,
            format_args!("bad {what} '{}'", super::str_token(shown)),
        )
    }

    fn end(mut self) -> Result<()> {
        match self.next_token() {
            None => Ok(()),
            Some(_) => Err(invalid(self.no, "more fields than the header declares")),
        }
    }
}

/// A decimal integer as `strtol(s, &end, 0)` reads it, when that read consumes the whole token
/// and fits in a C `long` (32 bits on Windows). A leading `0` followed by more digits is refused:
/// base 0 reads it as octal.
fn parse_int(tok: &[u8]) -> Option<i32> {
    let (neg, digits) = match tok.first()? {
        b'-' => (true, &tok[1..]),
        b'+' => (false, &tok[1..]),
        _ => (false, tok),
    };
    if digits.is_empty() || digits.len() > 10 || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    if digits.len() > 1 && digits[0] == b'0' {
        return None;
    }
    let magnitude = digits
        .iter()
        .fold(0i64, |v, d| v * 10 + i64::from(d - b'0'));
    i32::try_from(if neg { -magnitude } else { magnitude }).ok()
}

/// Significant digits MSVC's `strtod` (the C runtime TetGen is built with) takes into account.
/// It drops every later digit without letting it round, so `1.00000000000000011102230246251565404236316680908203125`
/// followed by 800 zeros and a `1` reads as 1.0 there and as the next double here. Probed with the
/// oracle on 2026-09-23: 768 digits are kept, leading zeros do not count. A longer mantissa is refused.
pub const MAX_SIGNIFICANT_DIGITS: usize = 768;

/// A finite decimal floating-point number as `strtod` reads it, when that read consumes the
/// whole token: `[+-]? (digits [. digits] | . digits) ([eE] [+-]? digits)?`. Hexadecimal,
/// `inf` and `nan` spellings are refused (TetGen's field scanner would not even start on the
/// letters), and so are a value that overflows to infinity and a mantissa of more than
/// [`MAX_SIGNIFICANT_DIGITS`] significant digits.
fn parse_real(tok: &[u8]) -> Option<f64> {
    let mut i = 0;
    let n = tok.len();
    if i < n && (tok[i] == b'+' || tok[i] == b'-') {
        i += 1;
    }
    let (mut digits, mut significant) = (0usize, 0usize);
    let mut scan_digits = |i: &mut usize| {
        while *i < n && tok[*i].is_ascii_digit() {
            if significant > 0 || tok[*i] != b'0' {
                significant += 1;
            }
            digits += 1;
            *i += 1;
        }
    };
    scan_digits(&mut i);
    if i < n && tok[i] == b'.' {
        i += 1;
        scan_digits(&mut i);
    }
    if digits == 0 || significant > MAX_SIGNIFICANT_DIGITS {
        return None;
    }
    if i < n && (tok[i] == b'e' || tok[i] == b'E') {
        i += 1;
        if i < n && (tok[i] == b'+' || tok[i] == b'-') {
            i += 1;
        }
        let exp_start = i;
        while i < n && tok[i].is_ascii_digit() {
            i += 1;
        }
        if i == exp_start {
            return None;
        }
    }
    if i != n {
        return None;
    }
    let v: f64 = std::str::from_utf8(tok).ok()?.parse().ok()?;
    v.is_finite().then_some(v)
}

/// The header: the first data line.
fn header<'a>(lines: &mut Lines<'a>) -> Result<Record<'a>> {
    lines
        .next_record()
        .ok_or_else(|| truncated(lines.buf.len()))
}

/// Checks the record count against the data lines present, before anything is reserved.
fn check_count(lines: &Lines, count: usize) -> Result<()> {
    let present = lines.count_records();
    if count > present {
        return Err(truncated(lines.buf.len()));
    }
    if count < present {
        return Err(FormatError::Invalid(format!(
            "tetgen: {present} data lines after the header, which declares {count}"
        )));
    }
    Ok(())
}

/// Refuses a file whose decoded records would outgrow the reader's allocation budget. Only a
/// dense file with many attributes of one or two characters each can; see the spec.
fn check_storage(len: usize, count: usize, per_record: usize) -> Result<()> {
    let need = count as u128 * per_record as u128;
    let allowed = 2 * len as u128 + STORAGE_SLACK as u128;
    if need > allowed {
        return Err(FormatError::Invalid(format!(
            "tetgen: {count} records of {per_record} bytes exceed the reader's budget for a {len}-byte file"
        )));
    }
    Ok(())
}

/// The record index: the first one sets the base (0 or 1, as `load_node_call` takes
/// `firstnumber` from the first point); every later one must continue it.
fn record_index(f: &mut Fields, i: usize, first: &mut i32, what: &str) -> Result<()> {
    let idx = f.int(what)?;
    if i == 0 {
        if idx != 0 && idx != 1 {
            return Err(invalid(
                f.no,
                format_args!("first {what} is {idx}, not 0 or 1"),
            ));
        }
        *first = idx;
    } else if i64::from(idx) != i64::from(*first) + i as i64 {
        return Err(invalid(f.no, format_args!("{what} {idx} out of sequence")));
    }
    Ok(())
}

/// A corner as `load_tet` / `load_face` check it without a `.node`.
fn corner(f: &mut Fields, first: i32) -> Result<i32> {
    let c = f.int("corner")?;
    let (c64, lo) = (i64::from(c), i64::from(first));
    if c64 < lo || c64 >= lo + STANDALONE_POINTS {
        return Err(invalid(
            f.no,
            format_args!("corner {c} outside [{first}, {first} + INT_MAX - 1)"),
        ));
    }
    Ok(c)
}

fn next_record<'a>(lines: &mut Lines<'a>) -> Result<Record<'a>> {
    // check_count has seen enough data lines; this cannot run dry.
    lines
        .next_record()
        .ok_or_else(|| truncated(lines.buf.len()))
}

// ---------------------------------------------------------------------------------------------
// Readers.

/// Reads a `.node` (or `_skipped.node`) file. Receipt: `tetgenio::load_node`,
/// `load_node_call` (tetgen.cxx:190, :69).
pub fn read_node(bytes: &[u8]) -> Result<NodeFile> {
    check_text(bytes)?;
    let mut lines = Lines::new(bytes);
    let head = header(&mut lines)?;
    if head.text.windows(4).any(|w| w == b"rbox") {
        // load_node switches to qhull's rbox layout when "rbox" appears anywhere in the line.
        return Err(invalid(
            head.no,
            "rbox (qhull) point files are not TetGen output",
        ));
    }
    let mut h = Fields::new(&head)?;
    let count = h.int("point count")?;
    let dim = h.opt_int("dimension")?.unwrap_or(3);
    let nattr = h.opt_int("attribute count")?.unwrap_or(0);
    let marker_flag = h.opt_int("marker flag")?.unwrap_or(0);
    let uv_flag = h.opt_int("uv flag")?.unwrap_or(0);
    h.end()?;
    let no = head.no;
    let count = usize::try_from(count).map_err(|_| invalid(no, "negative point count"))?;
    if dim != 3 {
        return Err(invalid(
            no,
            format_args!("dimension {dim}; TetGen's meshes are 3-D"),
        ));
    }
    let nattr = usize::try_from(nattr).map_err(|_| invalid(no, "negative attribute count"))?;
    if uv_flag != 0 {
        return Err(invalid(
            no,
            "uv parameters (TetGen allocates them but never reads them)",
        ));
    }
    let has_markers = marker_flag != 0;

    check_count(&lines, count)?;
    let per_record = nattr
        .saturating_mul(8)
        .saturating_add(if has_markers { 28 } else { 24 });
    check_storage(bytes.len(), count, per_record)?;
    let mut points = Vec::with_capacity(count);
    let mut attributes = Vec::with_capacity(count * nattr);
    let mut markers = has_markers.then(|| Vec::with_capacity(count));
    let mut first = 0;
    for i in 0..count {
        let rec = next_record(&mut lines)?;
        let mut f = Fields::new(&rec)?;
        record_index(&mut f, i, &mut first, "point index")?;
        let x = f.real("x")?;
        let y = f.real("y")?;
        let z = f.real("z")?;
        points.push([x, y, z]);
        for _ in 0..nattr {
            attributes.push(f.real("point attribute")?);
        }
        if let Some(m) = markers.as_mut() {
            m.push(f.int("point marker")?);
        }
        f.end()?;
    }
    Ok(NodeFile {
        first,
        dim,
        points,
        attributes_per_point: nattr,
        attributes,
        markers,
    })
}

/// Reads an `.ele` file. Receipt: `tetgenio::load_tet` (tetgen.cxx:445).
pub fn read_ele(bytes: &[u8]) -> Result<EleFile> {
    check_text(bytes)?;
    let mut lines = Lines::new(bytes);
    let head = header(&mut lines)?;
    let mut h = Fields::new(&head)?;
    let count = h.int("tetrahedron count")?;
    let corners = h.opt_int("corner count")?.unwrap_or(4);
    let nattr = h.opt_int("attribute count")?.unwrap_or(0);
    h.end()?;
    let no = head.no;
    if count <= 0 {
        return Err(invalid(
            no,
            format_args!("tetrahedron count {count}; load_tet needs at least 1"),
        ));
    }
    if corners != 4 && corners != 10 {
        return Err(invalid(
            no,
            format_args!("{corners} corners per tetrahedron, not 4 or 10"),
        ));
    }
    let nattr = usize::try_from(nattr).map_err(|_| invalid(no, "negative attribute count"))?;
    let (count, corners) = (count as usize, corners as usize);

    check_count(&lines, count)?;
    check_storage(
        bytes.len(),
        count,
        nattr.saturating_mul(8).saturating_add(4 * corners),
    )?;
    let mut tets = Vec::with_capacity(count * corners);
    let mut attributes = Vec::with_capacity(count * nattr);
    let mut first = 0;
    for i in 0..count {
        let rec = next_record(&mut lines)?;
        let mut f = Fields::new(&rec)?;
        record_index(&mut f, i, &mut first, "tetrahedron index")?;
        for _ in 0..corners {
            tets.push(corner(&mut f, first)?);
        }
        for _ in 0..nattr {
            attributes.push(f.real("tetrahedron attribute")?);
        }
        f.end()?;
    }
    Ok(EleFile {
        first,
        corners,
        tets,
        attributes_per_tet: nattr,
        attributes,
    })
}

/// Reads a `.face` or `_skipped.face` file. Receipt: `tetgenio::load_face` (tetgen.cxx:349).
pub fn read_face(bytes: &[u8]) -> Result<FaceFile> {
    check_text(bytes)?;
    let mut lines = Lines::new(bytes);
    let head = header(&mut lines)?;
    let mut h = Fields::new(&head)?;
    let count = h.int("face count")?;
    let marker_flag = h.opt_int("marker flag")?.unwrap_or(0);
    h.end()?;
    let count = usize::try_from(count).map_err(|_| invalid(head.no, "negative face count"))?;
    let has_markers = marker_flag != 0 && count > 0;

    check_count(&lines, count)?;
    check_storage(bytes.len(), count, 12 + if has_markers { 4 } else { 0 })?;
    let mut faces = Vec::with_capacity(count);
    let mut markers = has_markers.then(|| Vec::with_capacity(count));
    let mut first = 0;
    for i in 0..count {
        let rec = next_record(&mut lines)?;
        let mut f = Fields::new(&rec)?;
        record_index(&mut f, i, &mut first, "face index")?;
        let a = corner(&mut f, first)?;
        let b = corner(&mut f, first)?;
        let c = corner(&mut f, first)?;
        faces.push([a, b, c]);
        if let Some(m) = markers.as_mut() {
            m.push(f.int("face marker")?);
        }
        f.end()?;
    }
    Ok(FaceFile {
        first,
        faces,
        markers,
    })
}

/// Reads a `.neigh` file. TetGen writes it (`outneighbors`, tetgen.cxx:34924) but has no loader;
/// the header is read as `load_tet` reads an `.ele` header.
pub fn read_neigh(bytes: &[u8]) -> Result<NeighFile> {
    check_text(bytes)?;
    let mut lines = Lines::new(bytes);
    let head = header(&mut lines)?;
    let mut h = Fields::new(&head)?;
    let count = h.int("tetrahedron count")?;
    let per = h.opt_int("neighbours per tetrahedron")?.unwrap_or(4);
    h.end()?;
    let no = head.no;
    if count <= 0 {
        return Err(invalid(
            no,
            format_args!("tetrahedron count {count}; a mesh has at least 1"),
        ));
    }
    if per != 4 {
        return Err(invalid(
            no,
            format_args!("{per} neighbours per tetrahedron, not 4"),
        ));
    }
    let count = count as usize;

    check_count(&lines, count)?;
    check_storage(bytes.len(), count, 16)?;
    let mut neighbors = Vec::with_capacity(count);
    let mut first = 0;
    for i in 0..count {
        let rec = next_record(&mut lines)?;
        let mut f = Fields::new(&rec)?;
        record_index(&mut f, i, &mut first, "tetrahedron index")?;
        let mut row = [0i32; 4];
        for slot in &mut row {
            let n = f.int("neighbour")?;
            let rel = i64::from(n) - i64::from(first);
            if n != HULL && !(0..count as i64).contains(&rel) {
                return Err(invalid(
                    f.no,
                    format_args!("neighbour {n} is neither -1 nor a tetrahedron"),
                ));
            }
            *slot = n;
        }
        f.end()?;
        neighbors.push(row);
    }
    Ok(NeighFile { first, neighbors })
}

/// Reads a TetGen file of the given kind. The kind is a parameter because TetGen files do not
/// name themselves: an `.ele` without attributes and a `.neigh` can hold identical text.
pub fn read(kind: Kind, bytes: &[u8]) -> Result<TetgenFile> {
    Ok(match kind {
        Kind::Node => TetgenFile::Node(read_node(bytes)?),
        Kind::Ele => TetgenFile::Ele(read_ele(bytes)?),
        Kind::Face => TetgenFile::Face(read_face(bytes)?),
        Kind::Neigh => TetgenFile::Neigh(read_neigh(bytes)?),
    })
}

/// Reads a TetGen file, taking its kind from the extension (`.node`, `.ele`, `.face`, `.neigh`).
pub fn read_file(path: &Path) -> Result<TetgenFile> {
    let bytes = super::read_file(path)?;
    let kind = Kind::from_path(path).ok_or_else(|| {
        FormatError::Invalid(format!(
            "{}: not a TetGen file name (.node, .ele, .face or .neigh)",
            path.display()
        ))
    })?;
    read(kind, &bytes)
}

/// Checks one run's files against each other: one index base, every corner a point of the
/// `.node`, one neighbour row per tetrahedron, and every neighbour relation mutual. TetGen's
/// `load_tetmesh` makes the corner checks; the neighbour checks are what `outneighbors` writes.
pub fn check_mesh(
    nodes: &NodeFile,
    ele: &EleFile,
    faces: Option<&FaceFile>,
    neigh: Option<&NeighFile>,
) -> Result<()> {
    let first = i64::from(nodes.first);
    let npoints = nodes.points.len() as i64;
    let is_point = |c: i32| (first..first + npoints).contains(&i64::from(c));
    let mismatch = |what: &str, base: i32| {
        FormatError::Invalid(format!(
            "tetgen: {what} counts from {base}, the .node from {}",
            nodes.first
        ))
    };
    if ele.first != nodes.first {
        return Err(mismatch(".ele", ele.first));
    }
    if let Some(i) = ele.tets.iter().position(|&c| !is_point(c)) {
        return Err(FormatError::Invalid(format!(
            "tetgen: tetrahedron {} has corner {}, not one of the {npoints} points",
            i / ele.corners.max(1),
            ele.tets[i]
        )));
    }
    if let Some(faces) = faces {
        if !faces.faces.is_empty() && faces.first != nodes.first {
            return Err(mismatch(".face", faces.first));
        }
        if let Some(i) = faces
            .faces
            .iter()
            .position(|f| !f.iter().all(|&c| is_point(c)))
        {
            return Err(FormatError::Invalid(format!(
                "tetgen: face {i} has a corner that is not one of the {npoints} points"
            )));
        }
    }
    if let Some(neigh) = neigh {
        if neigh.neighbors.len() != ele.len() {
            return Err(FormatError::Invalid(format!(
                "tetgen: .neigh has {} rows for {} tetrahedra",
                neigh.neighbors.len(),
                ele.len()
            )));
        }
        if neigh.first != ele.first {
            return Err(mismatch(".neigh", neigh.first));
        }
        for (t, row) in neigh.neighbors.iter().enumerate() {
            let me = i64::from(neigh.first) + t as i64;
            for &n in row.iter().filter(|&&n| n != HULL) {
                let back = &neigh.neighbors[(i64::from(n) - i64::from(neigh.first)) as usize];
                if !back.iter().any(|&m| i64::from(m) == me) {
                    return Err(FormatError::Invalid(format!(
                        "tetgen: tetrahedron {me} lists {n} as a neighbour, but {n} does not list {me}"
                    )));
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Canonical dump (grammar in docs/formats/tetgen.md).

/// The canonical dump of a TetGen file.
pub fn dump(value: &TetgenFile) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "tetgen {}", value.kind().extension());
    match value {
        TetgenFile::Node(n) => {
            let _ = writeln!(s, "first {}\ndim {}", n.first, n.dim);
            let _ = writeln!(s, "attributes {}", n.attributes_per_point);
            let _ = writeln!(s, "markers {}", u8::from(n.markers.is_some()));
            let _ = writeln!(s, "points {}", n.points.len());
            for (i, p) in n.points.iter().enumerate() {
                let _ = write!(s, "{} {} {}", f64_hex(p[0]), f64_hex(p[1]), f64_hex(p[2]));
                let k = n.attributes_per_point;
                for &a in &n.attributes[i * k..(i + 1) * k] {
                    let _ = write!(s, " {}", f64_hex(a));
                }
                if let Some(m) = &n.markers {
                    let _ = write!(s, " {}", m[i]);
                }
                s.push('\n');
            }
        }
        TetgenFile::Ele(e) => {
            let _ = writeln!(s, "first {}\ncorners {}", e.first, e.corners);
            let _ = writeln!(s, "attributes {}\ntets {}", e.attributes_per_tet, e.len());
            for i in 0..e.len() {
                let corners: Vec<String> = e.tet(i).iter().map(i32::to_string).collect();
                s.push_str(&corners.join(" "));
                for &a in e.tet_attributes(i) {
                    let _ = write!(s, " {}", f64_hex(a));
                }
                s.push('\n');
            }
        }
        TetgenFile::Face(f) => {
            let _ = writeln!(s, "first {}", f.first);
            let _ = writeln!(
                s,
                "markers {}\nfaces {}",
                u8::from(f.markers.is_some()),
                f.faces.len()
            );
            for (i, [a, b, c]) in f.faces.iter().enumerate() {
                let _ = write!(s, "{a} {b} {c}");
                if let Some(m) = &f.markers {
                    let _ = write!(s, " {}", m[i]);
                }
                s.push('\n');
            }
        }
        TetgenFile::Neigh(n) => {
            let _ = writeln!(s, "first {}\ntets {}", n.first, n.neighbors.len());
            for [a, b, c, d] in &n.neighbors {
                let _ = writeln!(s, "{a} {b} {c} {d}");
            }
        }
    }
    s
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

/// The canonical dump of the file at `path`, or `error <kind>` when it cannot be read.
pub fn dump_file(path: &Path) -> String {
    match read_file(path) {
        Ok(v) => dump(&v),
        Err(e) => format!("error {}\n", error_kind(&e)),
    }
}
