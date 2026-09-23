//! The `.simpa` file: canonical writer, exact reader, load and save.

use std::fmt;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::ser::{CompactFormatter, Formatter};
use serde_json::{Map, Number, Value};

use super::integrity::IntegrityError;
use super::model::{FORMAT_VERSION, Project};

/// Why a project file could not be loaded. [`LoadError::code`] is a stable reason code.
#[derive(Debug)]
pub enum LoadError {
    /// The file does not exist.
    NotFound(PathBuf),
    /// Any other I/O failure.
    Io(io::Error),
    /// The bytes are not UTF-8.
    Utf8 { valid_up_to: usize },
    /// Not well-formed JSON (RFC 8259), or a number out of the f64 range, or a key repeated in
    /// one object. Line and column are 1-based; the column counts characters.
    Syntax {
        line: usize,
        column: usize,
        message: String,
    },
    /// The top level is not an object, or has no `format_version` key.
    MissingVersion,
    /// `format_version` is not the version this build reads (checked before anything else, so
    /// a newer file gets this error rather than a schema error). `found` is its JSON text.
    Version { found: String, supported: u32 },
    /// Well-formed JSON that does not match the schema: an unknown or missing key, a wrong type,
    /// an unknown enum value.
    Schema(String),
    /// Matches the schema but breaks a structural invariant.
    Integrity(IntegrityError),
}

impl LoadError {
    pub fn code(&self) -> &'static str {
        match self {
            LoadError::NotFound(_) => "not_found",
            LoadError::Io(_) => "io",
            LoadError::Utf8 { .. } => "utf8",
            LoadError::Syntax { .. } => "syntax",
            LoadError::MissingVersion => "missing_version",
            LoadError::Version { .. } => "version",
            LoadError::Schema(_) => "schema",
            LoadError::Integrity(_) => "integrity",
        }
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::NotFound(p) => write!(f, "project file not found: {}", p.display()),
            LoadError::Io(e) => write!(f, "i/o error: {e}"),
            LoadError::Utf8 { valid_up_to } => {
                write!(f, "not UTF-8: invalid byte after offset {valid_up_to}")
            }
            LoadError::Syntax {
                line,
                column,
                message,
            } => write!(f, "invalid JSON at line {line}, column {column}: {message}"),
            LoadError::MissingVersion => {
                write!(f, "not a project file: no format_version at the top level")
            }
            LoadError::Version { found, supported } => write!(
                f,
                "project file format version {found} is not supported (this build reads {supported})"
            ),
            LoadError::Schema(msg) => write!(f, "project file does not match the schema: {msg}"),
            LoadError::Integrity(e) => write!(f, "project is inconsistent: {e}"),
        }
    }
}

impl std::error::Error for LoadError {}

/// The canonical text of a project (see the `schema` module docs for the layout). The same
/// project always gives the same bytes. It writes any project, but only one that passes
/// [`Project::check_integrity`] loads back: for those, `to_json(&from_json(&to_json(p))?)` is
/// `to_json(p)`, byte for byte.
pub fn to_json(project: &Project) -> String {
    let mut out = Vec::with_capacity(4096);
    let mut serializer =
        serde_json::Serializer::with_formatter(&mut out, CanonicalFormatter::new());
    project
        .serialize(&mut serializer)
        .expect("a Project serialises without error: no map keys, every float encodable");
    out.push(b'\n');
    String::from_utf8(out).expect("serde_json writes UTF-8")
}

/// Parses a project from its JSON text: exact JSON reader, version check, schema, integrity.
pub fn from_json(text: &str) -> Result<Project, LoadError> {
    let value = parse_json(text)?;
    let version = match &value {
        Value::Object(map) => map.get("format_version"),
        _ => None,
    };
    match version {
        None => return Err(LoadError::MissingVersion),
        Some(Value::Number(n)) if n.as_u64() == Some(u64::from(FORMAT_VERSION)) => {}
        Some(v) => {
            return Err(LoadError::Version {
                found: v.to_string(),
                supported: FORMAT_VERSION,
            });
        }
    }
    let project: Project =
        serde_json::from_value(value).map_err(|e| LoadError::Schema(e.to_string()))?;
    project.check_integrity().map_err(LoadError::Integrity)?;
    Ok(project)
}

/// Reads any schema type from JSON text exactly: [`parse_json`], then serde. Use it, never
/// `serde_json::from_str`, for anything that holds an [`F64`](super::F64) and arrives as text,
/// such as an [`Op`](super::Op) or a [`Material`](super::Material) sent by the UI. The error is
/// [`LoadError::Syntax`] or [`LoadError::Schema`]. Unlike [`from_json`] it checks no version and
/// no integrity: [`Op::apply`](super::Op::apply) does that for an op.
pub fn from_json_exact<T: DeserializeOwned>(text: &str) -> Result<T, LoadError> {
    serde_json::from_value(parse_json(text)?).map_err(|e| LoadError::Schema(e.to_string()))
}

/// Reads and parses a project file.
pub fn load(path: &Path) -> Result<Project, LoadError> {
    let bytes = std::fs::read(path).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => LoadError::NotFound(path.to_path_buf()),
        _ => LoadError::Io(e),
    })?;
    let text = std::str::from_utf8(&bytes).map_err(|e| LoadError::Utf8 {
        valid_up_to: e.valid_up_to(),
    })?;
    from_json(text)
}

/// Writes the project's canonical text to `path`, so that `path` always holds either the old
/// project or the new one, whole:
///
/// 1. the text goes to `<path>.tmp` beside it, which is flushed to disk (`sync_all`) before
///    anything else, so a crash or power cut cannot leave a renamed but empty file;
/// 2. `<path>.tmp` is renamed over `path`.
///
/// On Windows another process that holds `path` open without delete sharing (an antivirus
/// scan, the search indexer, a sync client such as OneDrive) makes either step fail for a
/// moment with "access denied" or a sharing violation. Each step is then retried for about
/// 1.2 s ([`RETRY_DELAYS_MS`]) before the error is returned. On error `path` is untouched,
/// and if the text was written, `<path>.tmp` is left holding it.
pub fn save(project: &Project, path: &Path) -> io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let text = to_json(project);
    retry_transient(|| {
        let mut file = File::create(&tmp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()
    })?;
    retry_transient(|| std::fs::rename(&tmp, path))
}

/// Waits between the attempts [`save`] makes at each step, in milliseconds: 11 attempts over
/// 1,188 ms in all.
pub const RETRY_DELAYS_MS: [u64; 10] = [1, 2, 5, 10, 20, 50, 100, 200, 300, 500];

/// Runs `step`, retrying it after each of [`RETRY_DELAYS_MS`] while it fails transiently.
fn retry_transient<T>(mut step: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut delays = RETRY_DELAYS_MS.iter();
    loop {
        match step() {
            Err(e) if is_transient(&e) => match delays.next() {
                Some(&ms) => std::thread::sleep(Duration::from_millis(ms)),
                None => return Err(e),
            },
            result => return result,
        }
    }
}

/// Windows reports a file held open by another process as access denied (5), a sharing
/// violation (32) or a lock violation (33). Elsewhere a rename replaces an open file, so
/// nothing is retried.
#[cfg(windows)]
fn is_transient(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::PermissionDenied || matches!(e.raw_os_error(), Some(5 | 32 | 33))
}

#[cfg(not(windows))]
fn is_transient(_: &io::Error) -> bool {
    false
}

// ---------------------------------------------------------------------------------------------
// Canonical formatter

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    None,
    First,
    Next,
}

/// serde_json formatter for the canonical layout. Objects: one key per line, two-space indent.
/// Arrays: on one line (`[1, 2, 3]`) while their elements are scalars; each element on its own
/// line once an element is an object or an array. The separator before an array element is held
/// back until the element's first token shows which it is.
struct CanonicalFormatter {
    /// Per open container: has it gone multi-line.
    multiline: Vec<bool>,
    pending: Pending,
}

impl CanonicalFormatter {
    fn new() -> Self {
        CanonicalFormatter {
            multiline: Vec::new(),
            pending: Pending::None,
        }
    }

    fn newline<W: ?Sized + io::Write>(&self, w: &mut W, depth: usize) -> io::Result<()> {
        w.write_all(b"\n")?;
        for _ in 0..depth {
            w.write_all(b"  ")?;
        }
        Ok(())
    }

    /// Called before a scalar token.
    fn scalar<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        match std::mem::replace(&mut self.pending, Pending::None) {
            Pending::None | Pending::First => Ok(()),
            Pending::Next => {
                if self.multiline.last() == Some(&true) {
                    w.write_all(b",")?;
                    self.newline(w, self.multiline.len())
                } else {
                    w.write_all(b", ")
                }
            }
        }
    }

    /// Called before `[` or `{`.
    fn container<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        match std::mem::replace(&mut self.pending, Pending::None) {
            Pending::None => Ok(()),
            p => {
                if p == Pending::Next {
                    w.write_all(b",")?;
                }
                if let Some(m) = self.multiline.last_mut() {
                    *m = true;
                }
                self.newline(w, self.multiline.len())
            }
        }
    }

    fn close<W: ?Sized + io::Write>(&mut self, w: &mut W, bracket: &[u8]) -> io::Result<()> {
        if self.multiline.pop() == Some(true) {
            self.newline(w, self.multiline.len())?;
        }
        w.write_all(bracket)
    }
}

macro_rules! scalar_methods {
    ($($name:ident($ty:ty)),* $(,)?) => {
        $(
            fn $name<W: ?Sized + io::Write>(&mut self, w: &mut W, value: $ty) -> io::Result<()> {
                self.scalar(w)?;
                CompactFormatter.$name(w, value)
            }
        )*
    };
}

impl Formatter for CanonicalFormatter {
    scalar_methods!(
        write_bool(bool),
        write_i8(i8),
        write_i16(i16),
        write_i32(i32),
        write_i64(i64),
        write_i128(i128),
        write_u8(u8),
        write_u16(u16),
        write_u32(u32),
        write_u64(u64),
        write_u128(u128),
        write_f32(f32),
        write_f64(f64),
        write_number_str(&str),
        write_byte_array(&[u8]),
        write_raw_fragment(&str),
    );

    fn write_null<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.scalar(w)?;
        CompactFormatter.write_null(w)
    }

    fn begin_string<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.scalar(w)?;
        CompactFormatter.begin_string(w)
    }

    fn begin_array<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.container(w)?;
        self.multiline.push(false);
        w.write_all(b"[")
    }

    fn end_array<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.close(w, b"]")
    }

    fn begin_array_value<W: ?Sized + io::Write>(
        &mut self,
        _: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.pending = if first { Pending::First } else { Pending::Next };
        Ok(())
    }

    fn end_array_value<W: ?Sized + io::Write>(&mut self, _: &mut W) -> io::Result<()> {
        Ok(())
    }

    fn begin_object<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.container(w)?;
        self.multiline.push(false);
        w.write_all(b"{")
    }

    fn end_object<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.close(w, b"}")
    }

    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        w: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if !first {
            w.write_all(b",")?;
        }
        if let Some(m) = self.multiline.last_mut() {
            *m = true;
        }
        self.newline(w, self.multiline.len())
    }

    fn end_object_key<W: ?Sized + io::Write>(&mut self, _: &mut W) -> io::Result<()> {
        Ok(())
    }

    fn begin_object_value<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        w.write_all(b": ")
    }

    fn end_object_value<W: ?Sized + io::Write>(&mut self, _: &mut W) -> io::Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Exact reader

/// Deepest nesting the reader accepts, as serde_json's own default.
const MAX_DEPTH: usize = 128;

/// Parses JSON text (RFC 8259, strict) into a `serde_json::Value`, converting every number
/// exactly: an integer literal that fits becomes a `u64` or `i64`, and every other number is
/// parsed with `str::parse::<f64>`, which rounds correctly. A number beyond the f64 range, a key
/// repeated in one object, a raw control character in a string, a lone surrogate escape,
/// trailing commas and trailing content are refused. A leading byte-order mark is skipped. The
/// literal `-0` becomes the float -0.0.
pub fn parse_json(text: &str) -> Result<Value, LoadError> {
    let body = text.strip_prefix('\u{feff}').unwrap_or(text);
    let offset = text.len() - body.len();
    let mut p = Parser {
        s: body.as_bytes(),
        pos: 0,
        depth: 0,
    };
    let result = p.value().and_then(|v| {
        p.ws();
        if p.pos == p.s.len() {
            Ok(v)
        } else {
            Err(p.err("trailing content after the top-level value"))
        }
    });
    result.map_err(|(pos, message)| {
        let (line, column) = line_column(text, offset + pos);
        LoadError::Syntax {
            line,
            column,
            message,
        }
    })
}

fn line_column(text: &str, pos: usize) -> (usize, usize) {
    let mut pos = pos.min(text.len());
    while !text.is_char_boundary(pos) {
        pos -= 1;
    }
    let before = &text[..pos];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    (line, before[line_start..].chars().count() + 1)
}

type PResult<T> = Result<T, (usize, String)>;

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
    depth: usize,
}

impl Parser<'_> {
    fn err(&self, message: impl Into<String>) -> (usize, String) {
        (self.pos, message.into())
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    fn ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.peek() {
            self.pos += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> PResult<()> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.err(format!("expected '{}'", byte as char)))
        }
    }

    fn value(&mut self) -> PResult<Value> {
        self.ws();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Value::String(self.string()?)),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.err("expected a value")),
            None => Err(self.err("unexpected end of input")),
        }
    }

    fn literal(&mut self, word: &str, value: Value) -> PResult<Value> {
        if self.s[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(self.err("expected a value"))
        }
    }

    fn enter(&mut self) -> PResult<()> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            Err(self.err(format!("nested deeper than {MAX_DEPTH} levels")))
        } else {
            Ok(())
        }
    }

    fn object(&mut self) -> PResult<Value> {
        self.enter()?;
        self.pos += 1; // '{'
        let mut map = Map::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            self.depth -= 1;
            return Ok(Value::Object(map));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return Err(self.err("expected a string key"));
            }
            let key_at = self.pos;
            let key = self.string()?;
            self.ws();
            self.expect(b':')?;
            let value = self.value()?;
            if map.contains_key(&key) {
                return Err((key_at, format!("key \"{key}\" repeated in one object")));
            }
            map.insert(key, value);
            self.ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    self.depth -= 1;
                    return Ok(Value::Object(map));
                }
                _ => return Err(self.err("expected ',' or '}'")),
            }
        }
    }

    fn array(&mut self) -> PResult<Value> {
        self.enter()?;
        self.pos += 1; // '['
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            self.depth -= 1;
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.value()?);
            self.ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    self.depth -= 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.err("expected ',' or ']'")),
            }
        }
    }

    fn hex4(&mut self) -> PResult<u32> {
        let digits = self
            .s
            .get(self.pos..self.pos + 4)
            .filter(|d| d.iter().all(u8::is_ascii_hexdigit))
            .ok_or_else(|| self.err("expected 4 hex digits after \\u"))?;
        // The digits are ASCII hex, so both conversions succeed.
        let v = u32::from_str_radix(std::str::from_utf8(digits).unwrap_or("0"), 16).unwrap_or(0);
        self.pos += 4;
        Ok(v)
    }

    fn string(&mut self) -> PResult<String> {
        self.pos += 1; // '"'
        let mut out = String::new();
        loop {
            let start = self.pos;
            while let Some(b) = self.peek() {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            // `start` and `pos` sit on ASCII bytes or the ends, so this is a char boundary slice
            // of a `str`.
            out.push_str(
                std::str::from_utf8(&self.s[start..self.pos])
                    .map_err(|_| self.err("invalid UTF-8"))?,
            );
            match self.peek() {
                None => return Err(self.err("unterminated string")),
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let escape = self.peek().ok_or_else(|| self.err("unterminated string"))?;
                    self.pos += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let at = self.pos - 2;
                            let unit = self.hex4()?;
                            let code = match unit {
                                0xd800..=0xdbff => {
                                    if !self.s[self.pos..].starts_with(b"\\u") {
                                        return Err((at, "unpaired surrogate escape".into()));
                                    }
                                    self.pos += 2;
                                    let low = self.hex4()?;
                                    if !(0xdc00..=0xdfff).contains(&low) {
                                        return Err((at, "unpaired surrogate escape".into()));
                                    }
                                    0x10000 + ((unit - 0xd800) << 10) + (low - 0xdc00)
                                }
                                0xdc00..=0xdfff => {
                                    return Err((at, "unpaired surrogate escape".into()));
                                }
                                _ => unit,
                            };
                            out.push(
                                char::from_u32(code)
                                    .ok_or_else(|| (at, "invalid \\u escape".to_string()))?,
                            );
                        }
                        _ => return Err((self.pos - 1, "invalid escape".into())),
                    }
                }
                Some(_) => return Err(self.err("control character in a string")),
            }
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.pos;
        while let Some(b'0'..=b'9') = self.peek() {
            self.pos += 1;
        }
        self.pos - start
    }

    fn number(&mut self) -> PResult<Value> {
        let start = self.pos;
        let negative = self.peek() == Some(b'-');
        if negative {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => {
                self.pos += 1;
                if let Some(b'0'..=b'9') = self.peek() {
                    return Err(self.err("leading zero in a number"));
                }
            }
            Some(b'1'..=b'9') => {
                self.digits();
            }
            _ => return Err(self.err("expected a digit")),
        }
        let mut integer = true;
        if self.peek() == Some(b'.') {
            self.pos += 1;
            integer = false;
            if self.digits() == 0 {
                return Err(self.err("expected a digit after '.'"));
            }
        }
        if let Some(b'e' | b'E') = self.peek() {
            self.pos += 1;
            integer = false;
            if let Some(b'+' | b'-') = self.peek() {
                self.pos += 1;
            }
            if self.digits() == 0 {
                return Err(self.err("expected a digit in the exponent"));
            }
        }
        // ASCII only, so valid UTF-8.
        let text = std::str::from_utf8(&self.s[start..self.pos]).unwrap_or("");
        if integer && text != "-0" {
            let exact = if negative {
                text.parse::<i64>().ok().map(Number::from)
            } else {
                text.parse::<u64>().ok().map(Number::from)
            };
            if let Some(n) = exact {
                return Ok(Value::Number(n));
            }
        }
        let v: f64 = text
            .parse()
            .map_err(|_| (start, format!("unreadable number {text}")))?;
        Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| (start, format!("number {text} is beyond the f64 range")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical(v: &Value) -> String {
        let mut out = Vec::new();
        let mut s = serde_json::Serializer::with_formatter(&mut out, CanonicalFormatter::new());
        v.serialize(&mut s).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn layout() {
        let v = serde_json::json!({"a": [1, 2, [3, 4]], "b": {}, "c": [], "d": [[1], {"e": null}], "f": "x"});
        assert_eq!(
            canonical(&v),
            "{\n  \"a\": [1, 2,\n    [3, 4]\n  ],\n  \"b\": {},\n  \"c\": [],\n  \"d\": [\n    [1],\n    {\n      \"e\": null\n    }\n  ],\n  \"f\": \"x\"\n}"
        );
    }

    #[test]
    fn reader_refuses() {
        for bad in [
            "",
            "{",
            "[1,]",
            "{\"a\":1,}",
            "01",
            "1.",
            ".5",
            "-",
            "1e",
            "+1",
            "NaN",
            "\"\\x\"",
            "\"a\nb\"",
            "\"\\ud800\"",
            "\"\\udc00\"",
            "\"\\ud800\\u0041\"",
            "{\"a\":1,\"a\":2}",
            "1 2",
            "1e400",
            "-1e400",
            "tru",
            "nul",
            "[1 2]",
            "{\"a\" 1}",
            "{1:2}",
        ] {
            assert!(parse_json(bad).is_err(), "accepted {bad:?}");
        }
        let deep = "[".repeat(MAX_DEPTH + 1) + &"]".repeat(MAX_DEPTH + 1);
        assert!(parse_json(&deep).is_err());
        let ok = "[".repeat(MAX_DEPTH) + &"]".repeat(MAX_DEPTH);
        assert!(parse_json(&ok).is_ok());
    }

    #[test]
    fn reader_accepts() {
        let v = parse_json("\u{feff} {\"a\": [true, false, null, -0, 0, 18446744073709551615, -9223372036854775808, 18446744073709551616, \"\\u00e9\\ud83d\\ude00\\/\"]} ").unwrap();
        let a = v["a"].as_array().unwrap();
        assert_eq!(a[3].as_f64().unwrap().to_bits(), (-0.0f64).to_bits());
        assert_eq!(a[4].as_u64(), Some(0));
        assert_eq!(a[5].as_u64(), Some(u64::MAX));
        assert_eq!(a[6].as_i64(), Some(i64::MIN));
        assert_eq!(a[7].as_f64(), Some(18446744073709551616.0));
        assert_eq!(a[8].as_str(), Some("\u{e9}\u{1f600}/"));
    }

    #[test]
    fn syntax_error_position() {
        match parse_json("{\n  \"é\": x\n}") {
            Err(LoadError::Syntax { line, column, .. }) => assert_eq!((line, column), (2, 8)),
            other => panic!("{other:?}"),
        }
    }
}
