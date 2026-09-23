//! PLY: ASCII, binary little-endian and binary big-endian.
//!
//! Upstream's layered PLY convention is read (`lib_interface/input_output/ply/rply_interface.cpp`):
//! a face property `layer_id` (any integer type) names each face's layer, and an element `layer`
//! with a list property `layer_name` (a character list) names the layers, in id order. With
//! `layer_id`, each layer holding faces becomes a surface group, in id order, named by its
//! `layer_name` or `Layer <id>` when it has none; without it, the model is one group, `model`.
//!
//! Fixed against Night Mode's reader (`mesh/ply_loader.cpp:45-394`): binary files keep their
//! layers, big-endian files are read big-endian, list counts of every integer type are read at
//! their own size, integer vertex coordinates are read as integers, a face index out of range is
//! an error rather than a dropped face, and every count is checked against the bytes left before
//! anything is allocated.
//!
//! Vertex coordinates are the `vertex` element's `x`, `y` and `z` (any numeric type); face
//! indices are the `face` element's `vertex_indices` or `vertex_index` list (any integer type).
//! Other elements and properties are read and ignored.

use super::{ImportError, ImportOptions, ImportedModel, MeshFormat, RawMesh, Result};
use crate::config_xml::widen_f32;

const FMT: &str = "ply";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Encoding {
    Ascii,
    Little,
    Big,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scalar {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}

impl Scalar {
    fn parse(name: &str) -> Option<Scalar> {
        Some(match name {
            "char" | "int8" => Scalar::I8,
            "uchar" | "uint8" => Scalar::U8,
            "short" | "int16" => Scalar::I16,
            "ushort" | "uint16" => Scalar::U16,
            "int" | "int32" => Scalar::I32,
            "uint" | "uint32" => Scalar::U32,
            "float" | "float32" => Scalar::F32,
            "double" | "float64" => Scalar::F64,
            _ => return None,
        })
    }

    fn size(self) -> usize {
        match self {
            Scalar::I8 | Scalar::U8 => 1,
            Scalar::I16 | Scalar::U16 => 2,
            Scalar::I32 | Scalar::U32 | Scalar::F32 => 4,
            Scalar::F64 => 8,
        }
    }

    fn is_integer(self) -> bool {
        !matches!(self, Scalar::F32 | Scalar::F64)
    }
}

#[derive(Clone, Debug)]
enum Property {
    Scalar {
        name: String,
        ty: Scalar,
    },
    List {
        name: String,
        count: Scalar,
        item: Scalar,
    },
}

impl Property {
    fn name(&self) -> &str {
        match self {
            Property::Scalar { name, .. } | Property::List { name, .. } => name,
        }
    }

    /// The fewest bytes one value of this property takes in a binary file.
    fn min_binary_size(&self) -> usize {
        match self {
            Property::Scalar { ty, .. } => ty.size(),
            Property::List { count, .. } => count.size(),
        }
    }
}

#[derive(Clone, Debug)]
struct Element {
    name: String,
    count: usize,
    properties: Vec<Property>,
}

/// A value as read: integers exactly, reals as `f64` (a `float` widened).
#[derive(Clone, Copy, Debug)]
enum Value {
    Int(i64),
    Real(f64),
}

impl Value {
    fn as_real(self) -> f64 {
        match self {
            Value::Int(i) => i as f64,
            Value::Real(r) => r,
        }
    }
}

pub(super) fn read(bytes: &[u8], options: &ImportOptions) -> Result<ImportedModel> {
    let (encoding, elements, body) = header(bytes)?;
    let mut src = match encoding {
        Encoding::Ascii => Source::Ascii(AsciiTokens::new(body)),
        e => Source::Binary {
            data: body,
            pos: 0,
            big: e == Encoding::Big,
        },
    };

    let vertex_el = elements.iter().position(|e| e.name == "vertex");
    let face_el = elements.iter().position(|e| e.name == "face");
    let (Some(vertex_el), Some(face_el)) = (vertex_el, face_el) else {
        return Err(ImportError::invalid(
            FMT,
            "the header declares no `vertex` or no `face` element",
        ));
    };
    let vx = &elements[vertex_el];
    let coord = |axis: &str| -> Result<usize> {
        let i = vx
            .properties
            .iter()
            .position(|p| p.name() == axis)
            .ok_or_else(|| {
                ImportError::invalid(FMT, format!("the vertex element has no `{axis}` property"))
            })?;
        match &vx.properties[i] {
            Property::Scalar { .. } => Ok(i),
            Property::List { .. } => Err(ImportError::invalid(
                FMT,
                format!("vertex property `{axis}` is a list"),
            )),
        }
    };
    let (ix, iy, iz) = (coord("x")?, coord("y")?, coord("z")?);
    let fc = &elements[face_el];
    let idx_prop = fc
        .properties
        .iter()
        .position(|p| p.name() == "vertex_indices")
        .or_else(|| {
            fc.properties
                .iter()
                .position(|p| p.name() == "vertex_index")
        })
        .ok_or_else(|| {
            ImportError::invalid(
                FMT,
                "the face element has no `vertex_indices` or `vertex_index` list",
            )
        })?;
    match &fc.properties[idx_prop] {
        Property::List { item, .. } if item.is_integer() => {}
        _ => {
            return Err(ImportError::invalid(
                FMT,
                "the face vertex index property is not a list of integers",
            ));
        }
    }
    let layer_prop = fc.properties.iter().position(|p| p.name() == "layer_id");
    if let Some(i) = layer_prop {
        match &fc.properties[i] {
            Property::Scalar { ty, .. } if ty.is_integer() => {}
            _ => {
                return Err(ImportError::invalid(
                    FMT,
                    "face property `layer_id` is not an integer",
                ));
            }
        }
    }
    let layer_el = elements.iter().position(|e| e.name == "layer");

    let mut vertices: Vec<[f64; 3]> = Vec::new();
    let mut faces: Vec<(Vec<u32>, i64)> = Vec::new();
    let mut layer_names: Vec<String> = Vec::new();

    for (ei, el) in elements.iter().enumerate() {
        // Every count is checked against what is left before anything is reserved.
        src.check_room(el)?;
        if ei == vertex_el {
            vertices.reserve(el.count);
        } else if ei == face_el {
            faces.reserve(el.count);
        }
        for row in 0..el.count {
            let what = || format!("{} {row}", el.name);
            let mut point = [0.0f64; 3];
            let mut indices: Vec<u32> = Vec::new();
            let mut layer: i64 = 0;
            let mut name_bytes: Vec<u8> = Vec::new();
            for (pi, prop) in el.properties.iter().enumerate() {
                match prop {
                    Property::Scalar { ty, .. } => {
                        let v = src.value(*ty, &what)?;
                        if ei == vertex_el {
                            if pi == ix {
                                point[0] = v.as_real();
                            } else if pi == iy {
                                point[1] = v.as_real();
                            } else if pi == iz {
                                point[2] = v.as_real();
                            }
                        } else if ei == face_el
                            && Some(pi) == layer_prop
                            && let Value::Int(i) = v
                        {
                            layer = i;
                        }
                    }
                    Property::List { count, item, name } => {
                        let n = match src.value(*count, &what)? {
                            Value::Int(n) if n >= 0 => n as usize,
                            other => {
                                return Err(ImportError::invalid(
                                    FMT,
                                    format!("{}: list `{name}` has count {other:?}", what()),
                                ));
                            }
                        };
                        src.check_list_room(n, *item, &what)?;
                        let keep_indices = ei == face_el && pi == idx_prop;
                        let keep_name = Some(ei) == layer_el && name == "layer_name";
                        if keep_indices {
                            indices.reserve(n);
                        }
                        for _ in 0..n {
                            let v = src.value(*item, &what)?;
                            if keep_indices {
                                // The item type was checked to be an integer type.
                                let i = match v {
                                    Value::Int(i) => i,
                                    Value::Real(r) => r as i64,
                                };
                                let i = u32::try_from(i).map_err(|_| {
                                    ImportError::invalid(
                                        FMT,
                                        format!("{}: vertex index {i} is out of range", what()),
                                    )
                                })?;
                                indices.push(i);
                            } else if keep_name {
                                name_bytes.push(match v {
                                    Value::Int(c) => c as u8,
                                    Value::Real(r) => r as u8,
                                });
                            }
                        }
                    }
                }
            }
            if ei == vertex_el {
                vertices.push(point);
            } else if ei == face_el {
                faces.push((indices, layer));
            } else if Some(ei) == layer_el {
                layer_names.push(String::from_utf8_lossy(&name_bytes).into_owned());
            }
        }
    }
    let trailing = src.trailing();
    let mut notes = Vec::new();
    if trailing > 0 {
        notes.push(match encoding {
            Encoding::Ascii => format!("{trailing} values after the last element were ignored"),
            _ => format!("{trailing} bytes after the last element were ignored"),
        });
    }

    // Groups.
    let n = vertices.len();
    let mut polygons = Vec::with_capacity(faces.len());
    let groups: Vec<String>;
    if layer_prop.is_some() {
        let mut ids: Vec<i64> = faces.iter().map(|f| f.1).collect();
        ids.sort_unstable();
        ids.dedup();
        if let Some(&neg) = ids.first().filter(|&&i| i < 0) {
            return Err(ImportError::invalid(
                FMT,
                format!("layer_id {neg} is negative"),
            ));
        }
        groups = ids
            .iter()
            .map(|&id| {
                usize::try_from(id)
                    .ok()
                    .and_then(|i| layer_names.get(i))
                    .cloned()
                    .unwrap_or_else(|| {
                        notes.push(format!(
                            "layer_id {id} has no layer_name; its group is named `Layer {id}`"
                        ));
                        format!("Layer {id}")
                    })
            })
            .collect();
        for (i, name) in layer_names.iter().enumerate() {
            if ids.binary_search(&(i as i64)).is_err() {
                notes.push(format!("layer {i} `{name}` holds no face"));
            }
        }
        for (fi, (idx, layer)) in faces.into_iter().enumerate() {
            let g = ids.binary_search(&layer).expect("collected above") as u32;
            polygons.push((check_face(fi, idx, n)?, g));
        }
    } else {
        groups = vec!["model".to_string()];
        if !layer_names.is_empty() {
            notes.push(format!(
                "{} layer names were ignored: faces have no layer_id",
                layer_names.len()
            ));
        }
        for (fi, (idx, _)) in faces.into_iter().enumerate() {
            polygons.push((check_face(fi, idx, n)?, 0));
        }
    }
    let source_faces = polygons.len();
    RawMesh {
        vertices,
        polygons,
        groups,
        notes,
    }
    .finish(MeshFormat::Ply, options, source_faces)
}

fn check_face(fi: usize, idx: Vec<u32>, n: usize) -> Result<Vec<u32>> {
    if idx.len() < 3 {
        return Err(ImportError::invalid(
            FMT,
            format!(
                "face {fi} has {} vertices; a face needs 3 or more",
                idx.len()
            ),
        ));
    }
    if let Some(&bad) = idx.iter().find(|&&i| i as usize >= n) {
        return Err(ImportError::invalid(
            FMT,
            format!("face {fi} uses vertex {bad}, but there are {n} vertices"),
        ));
    }
    Ok(idx)
}

/// Parses the header. Returns the encoding, the elements and the body after `end_header`.
fn header(bytes: &[u8]) -> Result<(Encoding, Vec<Element>, &[u8])> {
    let mut pos = 0usize;
    let mut line_no = 0usize;
    let next_line = |pos: &mut usize| -> Option<&[u8]> {
        if *pos >= bytes.len() {
            return None;
        }
        let rest = &bytes[*pos..];
        let end = rest.iter().position(|&b| b == b'\n').unwrap_or(rest.len());
        *pos += (end + 1).min(rest.len());
        let mut line = &rest[..end];
        if line.last() == Some(&b'\r') {
            line = &line[..line.len() - 1];
        }
        Some(line)
    };
    let first = next_line(&mut pos).unwrap_or(b"");
    if first != b"ply" {
        return Err(ImportError::syntax(
            FMT,
            "line 1",
            "the file does not start with `ply`",
        ));
    }
    line_no += 1;
    let mut encoding = None;
    let mut elements: Vec<Element> = Vec::new();
    loop {
        let Some(line) = next_line(&mut pos) else {
            return Err(ImportError::truncated(FMT, "`end_header`"));
        };
        line_no += 1;
        let at = || format!("header line {line_no}");
        let text = std::str::from_utf8(line)
            .map_err(|_| ImportError::syntax(FMT, at(), "the header is not text"))?;
        let mut tok = text.split_ascii_whitespace();
        match tok.next() {
            Some("end_header") => break,
            Some("comment") | Some("obj_info") | None => {}
            Some("format") => {
                let e = match tok.next() {
                    Some("ascii") => Encoding::Ascii,
                    Some("binary_little_endian") => Encoding::Little,
                    Some("binary_big_endian") => Encoding::Big,
                    other => {
                        return Err(ImportError::syntax(
                            FMT,
                            at(),
                            format!("unknown format {other:?}"),
                        ));
                    }
                };
                match tok.next() {
                    Some("1.0") => {}
                    other => {
                        return Err(ImportError::unsupported(
                            FMT,
                            format!("format version {other:?}; only 1.0 is read"),
                        ));
                    }
                }
                if encoding.replace(e).is_some() {
                    return Err(ImportError::syntax(FMT, at(), "a second `format` line"));
                }
            }
            Some("element") => {
                let name = tok
                    .next()
                    .ok_or_else(|| ImportError::syntax(FMT, at(), "element without a name"))?;
                let count = tok
                    .next()
                    .and_then(|c| c.parse::<usize>().ok())
                    .ok_or_else(|| ImportError::syntax(FMT, at(), "element without a count"))?;
                elements.push(Element {
                    name: name.to_string(),
                    count,
                    properties: Vec::new(),
                });
            }
            Some("property") => {
                let Some(el) = elements.last_mut() else {
                    return Err(ImportError::syntax(
                        FMT,
                        at(),
                        "a property before any element",
                    ));
                };
                let ty = tok
                    .next()
                    .ok_or_else(|| ImportError::syntax(FMT, at(), "property without a type"))?;
                let scalar = |name: Option<&str>| -> Result<Scalar> {
                    name.and_then(Scalar::parse).ok_or_else(|| {
                        ImportError::syntax(FMT, at(), format!("unknown type {name:?}"))
                    })
                };
                let prop = if ty == "list" {
                    let count = scalar(tok.next())?;
                    if !count.is_integer() {
                        return Err(ImportError::syntax(
                            FMT,
                            at(),
                            "a list count type must be an integer type",
                        ));
                    }
                    let item = scalar(tok.next())?;
                    Property::List {
                        name: String::new(),
                        count,
                        item,
                    }
                } else {
                    Property::Scalar {
                        name: String::new(),
                        ty: scalar(Some(ty))?,
                    }
                };
                let name = tok
                    .next()
                    .ok_or_else(|| ImportError::syntax(FMT, at(), "property without a name"))?
                    .to_string();
                el.properties.push(match prop {
                    Property::Scalar { ty, .. } => Property::Scalar { name, ty },
                    Property::List { count, item, .. } => Property::List { name, count, item },
                });
            }
            Some(other) => {
                return Err(ImportError::syntax(
                    FMT,
                    at(),
                    format!("unknown header keyword `{other}`"),
                ));
            }
        }
    }
    let encoding =
        encoding.ok_or_else(|| ImportError::syntax(FMT, "header", "no `format` line"))?;
    Ok((encoding, elements, &bytes[pos.min(bytes.len())..]))
}

// ---------------------------------------------------------------------------------------------
// Reading values.

enum Source<'a> {
    Ascii(AsciiTokens<'a>),
    Binary {
        data: &'a [u8],
        pos: usize,
        big: bool,
    },
}

impl Source<'_> {
    /// Refuses an element whose count cannot fit in what is left: each row takes at least one
    /// token (ASCII) or its fixed minimum size (binary).
    fn check_room(&self, el: &Element) -> Result<()> {
        if el.count == 0 {
            return Ok(());
        }
        if el.properties.is_empty() {
            // Rows of nothing: meaningless, and unbounded by the data; refuse.
            return Err(ImportError::invalid(
                FMT,
                format!("element `{}` has rows but no property", el.name),
            ));
        }
        let (left, need) = match self {
            // A token and a separator per property; the last token may have no separator.
            Source::Ascii(t) => (
                t.data.len().saturating_sub(t.pos) + 1,
                el.count.checked_mul(2 * el.properties.len()),
            ),
            Source::Binary { data, pos, .. } => (
                data.len().saturating_sub(*pos),
                el.count.checked_mul(
                    el.properties
                        .iter()
                        .map(Property::min_binary_size)
                        .sum::<usize>(),
                ),
            ),
        };
        match need {
            Some(need) if need <= left => Ok(()),
            _ => Err(ImportError::truncated(
                FMT,
                format!("the {} rows of element `{}`", el.count, el.name),
            )),
        }
    }

    fn check_list_room(&self, n: usize, item: Scalar, what: &dyn Fn() -> String) -> Result<()> {
        let left = match self {
            Source::Ascii(t) => t.data.len().saturating_sub(t.pos),
            Source::Binary { data, pos, .. } => data.len().saturating_sub(*pos),
        };
        let per = match self {
            Source::Ascii(_) => 2,
            Source::Binary { .. } => item.size(),
        };
        match n.checked_mul(per) {
            Some(need) if need <= left + 1 => Ok(()),
            _ => Err(ImportError::truncated(
                FMT,
                format!("the {n} list items of {}", what()),
            )),
        }
    }

    fn value(&mut self, ty: Scalar, what: &dyn Fn() -> String) -> Result<Value> {
        match self {
            Source::Ascii(t) => {
                let tok = t
                    .next()
                    .ok_or_else(|| ImportError::truncated(FMT, what()))?;
                let text = std::str::from_utf8(tok)
                    .map_err(|_| ImportError::syntax(FMT, what(), "a value is not ASCII text"))?;
                parse_ascii(ty, text).ok_or_else(|| {
                    ImportError::syntax(FMT, what(), format!("`{text}` is not a valid {ty:?}"))
                })
            }
            Source::Binary { data, pos, big } => {
                let n = ty.size();
                let b = data
                    .get(*pos..*pos + n)
                    .ok_or_else(|| ImportError::truncated(FMT, what()))?;
                *pos += n;
                Ok(decode_binary(ty, b, *big))
            }
        }
    }

    fn trailing(&mut self) -> usize {
        match self {
            Source::Ascii(t) => {
                let mut n = 0;
                while t.next().is_some() {
                    n += 1;
                }
                // Tokens are not bytes, but any count > 0 is worth a note.
                n
            }
            Source::Binary { data, pos, .. } => data.len().saturating_sub(*pos),
        }
    }
}

fn parse_ascii(ty: Scalar, text: &str) -> Option<Value> {
    if ty.is_integer() {
        let v: i64 = text.parse().ok()?;
        let ok = match ty {
            Scalar::I8 => i8::try_from(v).is_ok(),
            Scalar::U8 => u8::try_from(v).is_ok(),
            Scalar::I16 => i16::try_from(v).is_ok(),
            Scalar::U16 => u16::try_from(v).is_ok(),
            Scalar::I32 => i32::try_from(v).is_ok(),
            Scalar::U32 => u32::try_from(v).is_ok(),
            Scalar::F32 | Scalar::F64 => unreachable!(),
        };
        ok.then_some(Value::Int(v))
    } else if ty == Scalar::F32 {
        let v: f32 = text.parse().ok()?;
        Some(Value::Real(widen_f32(v)))
    } else {
        Some(Value::Real(text.parse::<f64>().ok()?))
    }
}

fn decode_binary(ty: Scalar, b: &[u8], big: bool) -> Value {
    macro_rules! get {
        ($t:ty, $n:expr) => {{
            let mut a = [0u8; $n];
            a.copy_from_slice(b);
            if big {
                <$t>::from_be_bytes(a)
            } else {
                <$t>::from_le_bytes(a)
            }
        }};
    }
    match ty {
        Scalar::I8 => Value::Int(i64::from(b[0] as i8)),
        Scalar::U8 => Value::Int(i64::from(b[0])),
        Scalar::I16 => Value::Int(i64::from(get!(i16, 2))),
        Scalar::U16 => Value::Int(i64::from(get!(u16, 2))),
        Scalar::I32 => Value::Int(i64::from(get!(i32, 4))),
        Scalar::U32 => Value::Int(i64::from(get!(u32, 4))),
        Scalar::F32 => Value::Real(widen_f32(get!(f32, 4))),
        Scalar::F64 => Value::Real(get!(f64, 8)),
    }
}

/// Whitespace-separated tokens of an ASCII body.
struct AsciiTokens<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> AsciiTokens<'a> {
    fn new(data: &'a [u8]) -> Self {
        AsciiTokens { data, pos: 0 }
    }

    fn next(&mut self) -> Option<&'a [u8]> {
        let d = self.data;
        while self.pos < d.len() && d[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
        if self.pos >= d.len() {
            return None;
        }
        let start = self.pos;
        while self.pos < d.len() && !d[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
        Some(&d[start..self.pos])
    }
}
