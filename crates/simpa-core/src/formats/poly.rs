//! TetGen piecewise linear complex (`.poly`): the closed boundary I-Simpa hands TetGen for
//! meshing, as upstream's `formatPOLY::CPoly` writes and reads it
//! (`lib_interface/input_output/poly/poly.cpp`).
//!
//! [`write`] reproduces `CPoly::ExportPOLY` byte for byte as it runs on Windows: CRLF line ends
//! (text-mode `ofstream`), classic locale, 17 significant digits. [`read`] takes what
//! `ExportPOLY` writes, and accepts a file only when `CPoly::ImportPOLY` reads it as [`read`]
//! does, with one documented exception: upstream bug `poly.cpp:406-424` gives every user facet
//! after the first the first one's marker, and [`read`] keeps each facet's own marker.
//! [`upstream_import_view`] models exactly that collapse, and the oracle checks read against it.
//! It also refuses the spellings on which TetGen's
//! tokenizer would see different fields. Anything else is refused, where `ImportPOLY` would skip
//! it without a word. Spec, receipts and canonical dump: `docs/formats/poly.md`.

use std::fmt::Write as _;
use std::path::Path;

use super::{FormatError, Result, f32_hex, f64_hex};

const WHAT: &str = "poly";
/// `ExportPOLY` writes `\n` into a text-mode `std::ofstream`, which is `\r\n` on disk on Windows.
const EOL: &str = "\r\n";

/// A triangle of the boundary (upstream `formatPOLY::t_face`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Face {
    /// 0-based indices into [`Model::model_vertices`] (upstream `indicesSommets`; 1-based on disk).
    pub vertices: [u32; 3],
    /// The facet's boundary marker, which is I-Simpa's face index (upstream `faceIndex`).
    pub face_index: u32,
}

/// A closed volume for TetGen to tag and refine (upstream `formatPOLY::t_region`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    /// Regional attribute: the volume's element id (upstream `regionIndex`).
    pub region_index: i32,
    /// A point inside the volume, single precision (upstream `dotInRegion`, a `vec3`).
    pub dot_in_region: [f32; 3],
    /// Maximum tetrahedron volume in m³, -1 for no constraint (upstream `regionRefinement`).
    pub region_refinement: f32,
}

impl Default for Region {
    /// Upstream's `t_region()` sets only `regionRefinement = -1`.
    fn default() -> Self {
        Region {
            region_index: 0,
            dot_in_region: [0.0; 3],
            region_refinement: -1.0,
        }
    }
}

/// Everything upstream's reader keeps from a `.poly` file, in `formatPOLY::t_model`'s field
/// order. Holes are read and validated but not kept, exactly as `ImportPOLY` does, and the writer
/// always writes an empty hole list, as `ExportPOLY` does.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Model {
    /// The facet list's boundary-marker flag (upstream `saveFaceIndex`).
    pub save_face_index: bool,
    /// Facets of the I-Simpa extension "Part 5 - user facet list" (upstream `userDefinedFaces`).
    pub user_defined_faces: Vec<Face>,
    /// The facet list, a 4-corner polygon split into two triangles (upstream `modelFaces`).
    pub model_faces: Vec<Face>,
    /// Node coordinates in file order (upstream `modelVertices`, `dvec3`).
    pub model_vertices: Vec<[f64; 3]>,
    /// The region list (upstream `modelRegions`).
    pub model_regions: Vec<Region>,
}

// ---------------------------------------------------------------------------------------------
// Reading

/// Parses a `.poly` file. Never panics; allocates only the returned model, sized from records
/// actually present (a first pass counts them), never from a header's claim.
pub fn read(bytes: &[u8]) -> Result<Model> {
    let mut counts = Counts::default();
    parse(bytes, &mut counts)?;
    let mut model = Model {
        save_face_index: false,
        user_defined_faces: Vec::with_capacity(counts.user_faces),
        model_faces: Vec::with_capacity(counts.faces),
        model_vertices: Vec::with_capacity(counts.vertices),
        model_regions: Vec::with_capacity(counts.regions),
    };
    parse(bytes, &mut model)?;
    Ok(model)
}

/// Reads and parses a `.poly` file; a missing file is [`FormatError::NotFound`].
pub fn read_file(path: &Path) -> Result<Model> {
    read(&super::read_file(path)?)
}

/// Where parsed records go: counted on the first pass, stored on the second.
trait Sink {
    fn save_face_index(&mut self, on: bool);
    fn vertex(&mut self, v: [f64; 3]);
    fn face(&mut self, user: bool, face: Face);
    fn region(&mut self, region: Region);
}

#[derive(Default)]
struct Counts {
    vertices: usize,
    faces: usize,
    user_faces: usize,
    regions: usize,
}

impl Sink for Counts {
    fn save_face_index(&mut self, _: bool) {}
    fn vertex(&mut self, _: [f64; 3]) {
        self.vertices += 1;
    }
    fn face(&mut self, user: bool, _: Face) {
        if user {
            self.user_faces += 1;
        } else {
            self.faces += 1;
        }
    }
    fn region(&mut self, _: Region) {
        self.regions += 1;
    }
}

impl Sink for Model {
    fn save_face_index(&mut self, on: bool) {
        self.save_face_index = on;
    }
    fn vertex(&mut self, v: [f64; 3]) {
        self.model_vertices.push(v);
    }
    fn face(&mut self, user: bool, face: Face) {
        if user {
            self.user_defined_faces.push(face);
        } else {
            self.model_faces.push(face);
        }
    }
    fn region(&mut self, region: Region) {
        self.model_regions.push(region);
    }
}

fn parse<S: Sink>(bytes: &[u8], sink: &mut S) -> Result<()> {
    let mut lines = Lines {
        buf: bytes,
        pos: 0,
        no: 0,
    };

    // Part 1 - node list. Header: count, dimension, attribute count, optional marker flag.
    let head = lines.need()?;
    head.arity(3, 4, "node list header")?;
    let n_vertices = head.uint(0, "node count")?;
    head.expect(1, "dimension", 3)?;
    head.expect(2, "node attribute count", 0)?;
    if head.len == 4 {
        head.expect(3, "node boundary marker flag", 0)?;
    }
    for i in 0..n_vertices {
        let line = lines.need()?;
        line.arity(4, 4, "node")?;
        line.ordinal(0, u64::from(i) + 1, "node number")?;
        sink.vertex([
            line.real(1, "node x")?,
            line.real(2, "node y")?,
            line.real(3, "node z")?,
        ]);
    }

    // Part 2 - facet list. Header: count, boundary-marker flag.
    let head = lines.need()?;
    head.arity(2, 2, "facet list header")?;
    let n_facets = head.uint(0, "facet count")?;
    let flag = head.uint(1, "facet boundary marker flag")?;
    if flag > 1 {
        return Err(head.invalid(format!(
            "facet boundary marker flag is {flag}, expected 0 or 1"
        )));
    }
    sink.save_face_index(flag == 1);
    facets(&mut lines, sink, u64::from(n_facets), n_vertices, false)?;

    // Part 3 - hole list: read and dropped, as ImportPOLY does.
    let head = lines.need()?;
    head.arity(1, 1, "hole list header")?;
    let n_holes = head.count(0, "hole count")?;
    for i in 0..n_holes {
        let line = lines.need()?;
        line.arity(4, 4, "hole")?;
        line.ordinal(0, i + 1, "hole number")?;
        for f in 1..4 {
            line.real(f, "hole coordinate")?;
        }
    }

    // Part 4 - region list. Optional, as it is for TetGen.
    let Some(head) = lines.next()? else {
        return Ok(());
    };
    head.arity(1, 1, "region list header")?;
    let n_regions = head.count(0, "region count")?;
    for i in 0..n_regions {
        let line = lines.need()?;
        line.arity(6, 6, "region")?;
        line.ordinal(0, i + 1, "region number")?;
        let dot_in_region = [
            line.real32(1, "region x")?,
            line.real32(2, "region y")?,
            line.real32(3, "region z")?,
        ];
        let region_index = line.int(4, "region attribute")?;
        let region_refinement = line.real32(5, "region volume constraint")?;
        sink.region(Region {
            region_index,
            dot_in_region,
            region_refinement,
        });
    }

    // Part 5 - user facet list, I-Simpa's own extension. Optional; TetGen never reads it.
    let Some(head) = lines.next()? else {
        return Ok(());
    };
    head.arity(1, 2, "user facet list header")?;
    let n_user = head.count(0, "user facet count")?;
    if head.len == 2 {
        let flag = head.int(1, "user facet boundary marker flag")?;
        if !(0..=1).contains(&flag) {
            return Err(head.invalid(format!(
                "user facet boundary marker flag is {flag}, expected 0 or 1"
            )));
        }
    }
    facets(&mut lines, sink, n_user, n_vertices, true)?;

    match lines.next()? {
        Some(extra) => Err(extra.invalid("data after the user facet list")),
        None => Ok(()),
    }
}

/// `count` facets, each a header line `1 0 <marker>` and one polygon line `3 a b c` or
/// `4 a b c d`. A quadrilateral becomes the triangles (a,b,c) and (c,d,a), as in `ImportPOLY`.
fn facets<S: Sink>(
    lines: &mut Lines<'_>,
    sink: &mut S,
    count: u64,
    n_vertices: u32,
    user: bool,
) -> Result<()> {
    for _ in 0..count {
        let info = lines.need()?;
        info.arity(3, 3, "facet header")?;
        let polygons = info.int(0, "facet polygon count")?;
        if polygons != 1 {
            return Err(info.invalid(format!(
                "facet with {polygons} polygons: ImportPOLY reads exactly one per facet"
            )));
        }
        let holes = info.int(1, "facet hole count")?;
        if holes != 0 {
            return Err(info.invalid(format!(
                "facet with {holes} holes: ImportPOLY cannot read facet holes"
            )));
        }
        let face_index = info.uint(2, "facet boundary marker")?;

        let poly = lines.need()?;
        let corners = poly.int(0, "polygon corner count")?;
        let k = match corners {
            3 => 3,
            4 => 4,
            _ => {
                return Err(poly.invalid(format!(
                    "polygon with {corners} corners: ImportPOLY reads 3 or 4"
                )));
            }
        };
        poly.arity(k + 1, k + 1, "polygon")?;
        let mut v = [0u32; 4];
        for (j, slot) in v.iter_mut().take(k).enumerate() {
            *slot = poly.vertex(j + 1, n_vertices)?;
        }
        sink.face(
            user,
            Face {
                vertices: [v[0], v[1], v[2]],
                face_index,
            },
        );
        if k == 4 {
            sink.face(
                user,
                Face {
                    vertices: [v[2], v[3], v[0]],
                    face_index,
                },
            );
        }
    }
    Ok(())
}

/// The most fields any line may carry: a region line has six.
const MAX_FIELDS: usize = 6;

/// The longest line TetGen reads whole. `readnumberline` calls `fgets` with `INPUTLINESIZE` =
/// 2048 (`tetgen.h:70`, `tetgen.cxx:2900`), which holds 2047 characters; a longer line is split
/// and its tail read as a line of its own. A line of exactly 2047 leaves only its `\n` for the
/// next call, which TetGen skips as blank.
const MAX_LINE: usize = 2047;

/// Ctrl-Z. Text-mode reads on Windows (`ImportPOLY`'s `ifstream`, TetGen's `fopen "r"`) stop at
/// it as if the file ended there; elsewhere it is an ordinary byte.
const CTRL_Z: u8 = 0x1a;

/// A byte where TetGen's `readnumberline`/`findnextnumber` would start reading a number
/// (`tetgen.cxx:2906-2908`, `:2938-2940`).
fn starts_number(b: u8) -> bool {
    b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.')
}

/// Field separators both readers share: `ImportPOLY`'s `>>` skips `isspace`, and TetGen's
/// `findnextnumber` stops a field only at space, tab, comma or `#` (`tetgen.cxx:2932-2933`).
fn is_separator(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

/// Content lines. Blank lines and comment lines are skipped; see [`Lines::next`].
struct Lines<'a> {
    buf: &'a [u8],
    pos: usize,
    no: usize,
}

impl<'a> Lines<'a> {
    /// The next content line. A line is split at `\n`, and one `\r` before it (or at the end of
    /// the file) is the line end, as text mode reads it. Then:
    /// - Ctrl-Z anywhere, or more than [`MAX_LINE`] bytes, is `Invalid`;
    /// - a line holding `#` is a comment when nothing before its first `#` can start a number:
    ///   `ImportPOLY` skips every line holding `#` (`poly.cpp:297`), and TetGen skips exactly
    ///   these as comments. With a number before the `#` it is `Invalid`, because TetGen would
    ///   read that number and `ImportPOLY` would not;
    /// - otherwise fields are separated by spaces and tabs only. Any other whitespace byte
    ///   (`\r` inside the line, `\v`, `\f`) is `Invalid`: `ImportPOLY` separates fields at it
    ///   and TetGen does not. A line of no fields is blank.
    fn next(&mut self) -> Result<Option<Line<'a>>> {
        while self.pos < self.buf.len() {
            let rest = &self.buf[self.pos..];
            let (raw, used) = match rest.iter().position(|&b| b == b'\n') {
                Some(i) => (&rest[..i], i + 1),
                None => (rest, rest.len()),
            };
            self.pos += used;
            self.no += 1;
            let no = self.no;
            let invalid = |msg: &str| FormatError::Invalid(format!("line {no}: {msg}"));
            let text = raw.strip_suffix(b"\r").unwrap_or(raw);
            if text.len() > MAX_LINE {
                return Err(invalid(&format!(
                    "{} bytes long: TetGen reads at most {MAX_LINE} bytes of a line and the rest \
                     as another line",
                    text.len()
                )));
            }
            if text.contains(&CTRL_Z) {
                return Err(invalid(
                    "Ctrl-Z (0x1a): text-mode reads on Windows end the file there, other \
                     platforms read on",
                ));
            }
            if let Some(hash) = text.iter().position(|&b| b == b'#') {
                if text[..hash].iter().copied().any(starts_number) {
                    return Err(invalid(
                        "'#' after data: ImportPOLY drops such a line whole, TetGen reads the \
                         numbers before the '#'",
                    ));
                }
                continue;
            }
            // The classic-locale `isspace` set, less the separators and the `\n` that ends a line.
            if let Some(&b) = text
                .iter()
                .find(|&&b| matches!(b, b'\r' | b'\x0b' | b'\x0c'))
            {
                return Err(invalid(&format!(
                    "whitespace byte {b:#04x} inside a line: ImportPOLY separates fields at it, \
                     TetGen does not"
                )));
            }
            let mut fields = text.split(|&b| is_separator(b)).filter(|f| !f.is_empty());
            let Some(first) = fields.next() else { continue };
            let mut line = Line {
                no,
                fields: [first; MAX_FIELDS],
                len: 1,
            };
            for field in fields {
                if line.len == MAX_FIELDS {
                    return Err(line.invalid(format!("more than {MAX_FIELDS} fields")));
                }
                line.fields[line.len] = field;
                line.len += 1;
            }
            return Ok(Some(line));
        }
        Ok(None)
    }

    /// The next content line, which must exist.
    fn need(&mut self) -> Result<Line<'a>> {
        self.next()?.ok_or(FormatError::Truncated {
            what: WHAT,
            offset: self.buf.len(),
        })
    }
}

struct Line<'a> {
    no: usize,
    fields: [&'a [u8]; MAX_FIELDS],
    len: usize,
}

impl<'a> Line<'a> {
    fn invalid(&self, msg: impl std::fmt::Display) -> FormatError {
        FormatError::Invalid(format!("line {}: {msg}", self.no))
    }

    fn arity(&self, min: usize, max: usize, what: &str) -> Result<()> {
        if self.len < min || self.len > max {
            let want = if min == max {
                min.to_string()
            } else {
                format!("{min} to {max}")
            };
            return Err(self.invalid(format!("{what} has {} fields, expected {want}", self.len)));
        }
        Ok(())
    }

    fn field(&self, i: usize, what: &str) -> Result<&'a [u8]> {
        if i < self.len {
            Ok(self.fields[i])
        } else {
            Err(self.invalid(format!("{what} missing")))
        }
    }

    /// Quotes at most 32 bytes of the field, so a huge field cannot inflate the error message.
    fn bad(&self, i: usize, what: &str, problem: &str) -> FormatError {
        let field = self.fields.get(i).copied().unwrap_or_default();
        let shown = String::from_utf8_lossy(&field[..field.len().min(32)]);
        let more = if field.len() > 32 { "..." } else { "" };
        self.invalid(format!("{what} '{}{more}' {problem}", shown.escape_debug()))
    }

    /// An integer field within `lo..=hi`; `signed` allows a minus sign.
    fn integer(&self, i: usize, what: &str, signed: bool, lo: i64, hi: i64) -> Result<i64> {
        let range = if signed {
            "is not a 32-bit integer"
        } else {
            "is not an unsigned 32-bit integer"
        };
        match parse_integer(self.field(i, what)?, signed) {
            Ok(v) if (lo..=hi).contains(&v) => Ok(v),
            Ok(_) | Err(IntError::Syntax) => Err(self.bad(i, what, range)),
            Err(IntError::LeadingZero) => Err(self.bad(
                i,
                what,
                "has a leading zero: TetGen's strtol would read it as octal",
            )),
        }
    }

    /// A C++ `int` (or MSVC's 32-bit `long`): `[+-]?[0-9]+` within `i32`, no leading zero.
    fn int(&self, i: usize, what: &str) -> Result<i32> {
        let v = self.integer(i, what, true, i32::MIN.into(), i32::MAX.into())?;
        Ok(v as i32)
    }

    /// A C++ `unsigned int`: `+?[0-9]+` within `u32`, no leading zero. A minus sign is refused,
    /// where MSVC's `>> unsigned` would wrap `-1` to 4294967295.
    fn uint(&self, i: usize, what: &str) -> Result<u32> {
        let v = self.integer(i, what, false, 0, u32::MAX.into())?;
        Ok(v as u32)
    }

    /// A list length stored in a C++ `int`: non-negative.
    fn count(&self, i: usize, what: &str) -> Result<u64> {
        let v = self.int(i, what)?;
        u64::try_from(v).map_err(|_| self.invalid(format!("{what} is {v}, expected at least 0")))
    }

    fn expect(&self, i: usize, what: &str, want: i32) -> Result<()> {
        let v = self.int(i, what)?;
        if v != want {
            return Err(self.invalid(format!("{what} is {v}, expected {want}")));
        }
        Ok(())
    }

    /// A record number, which must count up from 1: `ImportPOLY` ignores it and takes records
    /// in file order, so any other numbering would be read differently by TetGen.
    fn ordinal(&self, i: usize, want: u64, what: &str) -> Result<()> {
        let v = self.int(i, what)?;
        if i64::from(v) != want as i64 {
            return Err(self.invalid(format!("{what} is {v}, expected {want}")));
        }
        Ok(())
    }

    /// A 1-based node reference, returned 0-based.
    fn vertex(&self, i: usize, n_vertices: u32) -> Result<u32> {
        let v = self.int(i, "polygon corner")?;
        match u32::try_from(v) {
            Ok(v) if (1..=n_vertices).contains(&v) => Ok(v - 1),
            _ => Err(self.invalid(format!(
                "polygon corner {v} is not a node (1 to {n_vertices})"
            ))),
        }
    }

    fn real(&self, i: usize, what: &str) -> Result<f64> {
        const NOT: &str = "is not a decimal number";
        let (negative, magnitude, nonzero) =
            decimal(self.field(i, what)?).ok_or_else(|| self.bad(i, what, NOT))?;
        let v: f64 = magnitude.parse().map_err(|_| self.bad(i, what, NOT))?;
        check_range(v.is_finite(), v == 0.0, nonzero).map_err(|m| self.bad(i, what, m))?;
        Ok(if negative { -v } else { v })
    }

    fn real32(&self, i: usize, what: &str) -> Result<f32> {
        const NOT: &str = "is not a decimal number";
        let (negative, magnitude, nonzero) =
            decimal(self.field(i, what)?).ok_or_else(|| self.bad(i, what, NOT))?;
        let v: f32 = magnitude.parse().map_err(|_| self.bad(i, what, NOT))?;
        check_range(v.is_finite(), v == 0.0, nonzero).map_err(|m| self.bad(i, what, m))?;
        Ok(if negative { -v } else { v })
    }
}

/// MSVC's `>> double` / `>> float` fail on a literal that overflows to infinity or a non-zero
/// literal that underflows to zero (subnormal results are accepted); so does this reader.
fn check_range(
    finite: bool,
    zero: bool,
    nonzero_digits: bool,
) -> std::result::Result<(), &'static str> {
    if !finite {
        Err("is not finite in range")
    } else if zero && nonzero_digits {
        Err("is not representable (it underflows to zero)")
    } else {
        Ok(())
    }
}

enum IntError {
    /// Not `[+-]?[0-9]+`, or beyond `i64`.
    Syntax,
    /// Digits starting with `0`, other than `0` itself. `ImportPOLY` reads them as decimal, but
    /// TetGen's `strtol(.., 0)` (`tetgen.cxx:902`, `:993`, `:1034`, `:1147`, `:1189`) reads
    /// `010` as octal 8 and `08` as 0.
    LeadingZero,
}

/// `[+-]?[0-9]+` (sign `-` only when `signed`) without a leading zero, in `i64`.
fn parse_integer(tok: &[u8], signed: bool) -> std::result::Result<i64, IntError> {
    let (negative, digits) = match tok.split_first() {
        Some((b'+', rest)) => (false, rest),
        Some((b'-', rest)) if signed => (true, rest),
        _ => (false, tok),
    };
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return Err(IntError::Syntax);
    }
    if digits.len() > 1 && digits[0] == b'0' {
        return Err(IntError::LeadingZero);
    }
    let mut v: i64 = 0;
    for &d in digits {
        v = v
            .checked_mul(10)
            .and_then(|v| v.checked_add(i64::from(d - b'0')))
            .ok_or(IntError::Syntax)?;
    }
    Ok(if negative { -v } else { v })
}

/// Splits a decimal literal `[+-]?(D+(.D*)?|.D+)([eE][+-]?D+)?` into its sign and magnitude, and
/// says whether the mantissa holds a non-zero digit. Hexadecimal (which MSVC's `>> double`
/// accepts), `inf` and `nan` are refused.
fn decimal(tok: &[u8]) -> Option<(bool, &str, bool)> {
    let (negative, mag) = match tok.split_first()? {
        (b'-', rest) => (true, rest),
        (b'+', rest) => (false, rest),
        _ => (false, tok),
    };
    let mut i = 0;
    let mut digits = 0;
    let mut nonzero = false;
    while let Some(&d) = mag.get(i).filter(|d| d.is_ascii_digit()) {
        nonzero |= d != b'0';
        digits += 1;
        i += 1;
    }
    if mag.get(i) == Some(&b'.') {
        i += 1;
        while let Some(&d) = mag.get(i).filter(|d| d.is_ascii_digit()) {
            nonzero |= d != b'0';
            digits += 1;
            i += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if matches!(mag.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(mag.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let start = i;
        while mag.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    if i != mag.len() {
        return None;
    }
    Some((negative, std::str::from_utf8(mag).ok()?, nonzero))
}

// ---------------------------------------------------------------------------------------------
// Canonical dump

/// The canonical dump (grammar in `docs/formats/poly.md`), `t_model`'s fields in its order.
pub fn dump(value: &Model) -> String {
    let mut s = String::from("poly\n");
    let _ = writeln!(s, "save_face_index {}", u8::from(value.save_face_index));
    dump_faces(&mut s, "user_faces", &value.user_defined_faces);
    dump_faces(&mut s, "faces", &value.model_faces);
    let _ = writeln!(s, "vertices {}", value.model_vertices.len());
    for v in &value.model_vertices {
        let _ = writeln!(s, "{} {} {}", f64_hex(v[0]), f64_hex(v[1]), f64_hex(v[2]));
    }
    let _ = writeln!(s, "regions {}", value.model_regions.len());
    for r in &value.model_regions {
        let [x, y, z] = r.dot_in_region;
        let _ = writeln!(
            s,
            "{} {} {} {} {}",
            r.region_index,
            f32_hex(x),
            f32_hex(y),
            f32_hex(z),
            f32_hex(r.region_refinement)
        );
    }
    s
}

fn dump_faces(s: &mut String, name: &str, faces: &[Face]) {
    let _ = writeln!(s, "{name} {}", faces.len());
    for f in faces {
        let [a, b, c] = f.vertices;
        let _ = writeln!(s, "{a} {b} {c} {}", f.face_index);
    }
}

/// The canonical dump of a file, or `error <kind>` when it cannot be read.
pub fn dump_file(path: &Path) -> String {
    match read_file(path) {
        Ok(model) => dump(&model),
        Err(e) => format!("error {}\n", error_kind(&e)),
    }
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

// ---------------------------------------------------------------------------------------------
// Writing

/// The bytes `CPoly::ExportPOLY` writes for `value` on Windows, spacing and all.
pub fn write(value: &Model) -> Vec<u8> {
    let mut s = String::new();
    let _ = write!(
        s,
        "# Part 1 - node list{EOL}{}  3  0  0{EOL}",
        value.model_vertices.len()
    );
    for (i, v) in value.model_vertices.iter().enumerate() {
        let _ = write!(s, "{}", i + 1);
        for &c in v {
            s.push(' ');
            push_g17(&mut s, c);
        }
        s.push_str(EOL);
    }

    let _ = write!(
        s,
        "{EOL}# Part 2 - facet list{EOL}{} {}{EOL}",
        value.model_faces.len(),
        u8::from(value.save_face_index)
    );
    for f in &value.model_faces {
        let [a, b, c] = f.vertices.map(|v| u64::from(v) + 1);
        let _ = write!(s, "1  0  {}{EOL}3  {a} {b} {c}{EOL}", f.face_index);
    }

    let _ = write!(
        s,
        "{EOL}# Part 3 - hole list{EOL}0{EOL}{EOL}# Part 4 - region list{EOL}{}{EOL}",
        value.model_regions.len()
    );
    for (i, r) in value.model_regions.iter().enumerate() {
        let _ = write!(s, "{}", i + 1);
        for &c in &r.dot_in_region {
            s.push_str("  ");
            push_g17(&mut s, f64::from(c));
        }
        let _ = write!(s, "  {}  ", r.region_index);
        push_g17(&mut s, f64::from(r.region_refinement));
        s.push_str(EOL);
    }

    let _ = write!(
        s,
        "{EOL}# Part 5 - user facet list{EOL}{}  1{EOL}",
        value.user_defined_faces.len()
    );
    for f in &value.user_defined_faces {
        let [a, b, c] = f.vertices.map(|v| u64::from(v) + 1);
        let _ = write!(s, "1  0  {}{EOL}3  {a}  {b}  {c}{EOL}", f.face_index);
    }
    s.into_bytes()
}

/// The model upstream's `ImportPOLY` returns for the file [`write`] makes of `model`, when the
/// model's values are finite and its node indices are below 2^31, and likewise for any file
/// [`read`] accepts. That model is `model` itself, except that every user facet after the first
/// carries the first one's marker. The cause is an upstream bug: the user-facet loop at
/// `poly.cpp:406-424` compares its counter with the main facet count. Our [`read`] keeps each
/// marker.
///
/// An oracle differential over generated files must compare upstream's reading with
/// `dump(&upstream_import_view(&model))`: against `dump(&model)` the bug alone makes about half
/// of [`generate`]'s models differ, which is deliberate, since a generator avoiding the bug
/// would hide it.
pub fn upstream_import_view(model: &Model) -> Model {
    let mut view = model.clone();
    if let Some(first) = view.user_defined_faces.first().map(|f| f.face_index) {
        for f in &mut view.user_defined_faces {
            f.face_index = first;
        }
    }
    view
}

/// Writes [`write`]'s bytes to `path`.
pub fn write_file(value: &Model, path: &Path) -> Result<()> {
    std::fs::write(path, write(value)).map_err(FormatError::from)
}

/// `v` as MSVC's classic-locale `ostream << double` prints it at precision 17 (`%.17g`): 17
/// significant digits of the exact binary value rounded half to even (verified against the
/// UCRT, e.g. 2^-25 -> `2.9802322387695312e-08`), trailing zeros dropped, a two-digit minimum
/// exponent, and the UCRT's spellings of infinities and NaNs.
fn push_g17(out: &mut String, v: f64) {
    let bits = v.to_bits();
    let negative = bits >> 63 != 0;
    if v.is_nan() {
        out.push_str(if negative { "-nan" } else { "nan" });
        if bits & (1 << 51) == 0 {
            out.push_str("(snan)");
        } else if negative && bits & ((1 << 51) - 1) == 0 {
            out.push_str("(ind)");
        }
        return;
    }
    if negative {
        out.push('-');
    }
    let a = v.abs();
    if a.is_infinite() {
        out.push_str("inf");
        return;
    }
    if a == 0.0 {
        out.push('0');
        return;
    }
    // Rust's exact formatting rounds half to even, as the UCRT's printf does.
    let sci = format!("{a:.16e}");
    let (mantissa, exponent) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exponent.parse().unwrap_or(0);
    let mut d = [b'0'; 17];
    for (slot, b) in d
        .iter_mut()
        .zip(mantissa.bytes().filter(u8::is_ascii_digit))
    {
        *slot = b;
    }
    let trim = |s: &[u8]| -> usize { s.iter().rposition(|&b| b != b'0').map_or(0, |p| p + 1) };
    let push_digits = |out: &mut String, s: &[u8]| s.iter().for_each(|&b| out.push(char::from(b)));
    if (-4..17).contains(&exp) {
        if exp >= 0 {
            let int_len = exp as usize + 1;
            push_digits(out, &d[..int_len]);
            let frac = &d[int_len..];
            let n = trim(frac);
            if n > 0 {
                out.push('.');
                push_digits(out, &frac[..n]);
            }
        } else {
            out.push_str("0.");
            for _ in 0..(-exp - 1) {
                out.push('0');
            }
            push_digits(out, &d[..trim(&d)]);
        }
    } else {
        push_digits(out, &d[..1]);
        let n = trim(&d[1..]);
        if n > 0 {
            out.push('.');
            push_digits(out, &d[1..1 + n]);
        }
        let _ = write!(
            out,
            "e{}{:02}",
            if exp < 0 { '-' } else { '+' },
            exp.unsigned_abs()
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Random valid models

/// A small valid model, different for every seed, whose coordinates exercise every branch of
/// the float spelling: integers, full-precision values, single-precision values, exponents in
/// both notations, exact binary fractions (which include rounding ties) and signed zeros.
pub fn generate(seed: u64) -> Model {
    let mut rng = Rng::new(seed);
    let n_vertices = 3 + rng.below(10) as u32;
    let model_vertices = (0..n_vertices)
        .map(|_| [rng.coord(), rng.coord(), rng.coord()])
        .collect();
    let save_face_index = rng.next() & 1 == 1;
    let n_faces = 1 + rng.below(12);
    let model_faces = (0..n_faces).map(|_| rng.face(n_vertices)).collect();
    let n_regions = rng.below(4);
    let model_regions = (0..n_regions)
        .map(|_| Region {
            region_index: rng.region_index(),
            dot_in_region: [rng.coord() as f32, rng.coord() as f32, rng.coord() as f32],
            region_refinement: if rng.below(2) == 0 {
                -1.0
            } else {
                (rng.coord().abs() as f32).max(f32::MIN_POSITIVE)
            },
        })
        .collect();
    let n_user = rng.below(4);
    let user_defined_faces = (0..n_user).map(|_| rng.face(n_vertices)).collect();
    Model {
        save_face_index,
        user_defined_faces,
        model_faces,
        model_vertices,
        model_regions,
    }
}

/// xorshift64* seeded through splitmix64, so seed 0 and neighbouring seeds all work.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        Rng(if z == 0 { 0x2545_F491_4F6C_DD1D } else { z })
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn coord(&mut self) -> f64 {
        let sign = if self.below(2) == 0 { 1.0 } else { -1.0 };
        match self.below(7) {
            0 => sign * self.below(21) as f64,
            1 => self.unit() * 100.0 - 50.0,
            2 => f64::from((self.unit() * 40.0 - 20.0) as f32),
            3 => sign * (1.0 + self.unit()) * 10f64.powi(self.below(40) as i32 - 15),
            4 => {
                let odd = (self.next() >> (11 + self.below(40))) | 1;
                sign * odd as f64 * 2f64.powi(-(1 + self.below(40) as i32))
            }
            5 => -0.0,
            _ => sign * f64::from(self.below(1000) as u32) / 8.0,
        }
    }

    fn face(&mut self, n_vertices: u32) -> Face {
        let n = u64::from(n_vertices);
        Face {
            vertices: [
                self.below(n) as u32,
                self.below(n) as u32,
                self.below(n) as u32,
            ],
            face_index: match self.below(4) {
                0 => u32::MAX - self.below(3) as u32,
                1 => self.next() as u32,
                _ => self.below(40) as u32,
            },
        }
    }

    fn region_index(&mut self) -> i32 {
        match self.below(4) {
            0 => self.next() as i32,
            1 => [i32::MIN, i32::MAX, -1][self.below(3) as usize],
            _ => self.below(5000) as i32,
        }
    }
}
