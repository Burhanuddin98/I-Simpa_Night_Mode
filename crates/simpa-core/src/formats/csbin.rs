//! `.csbin`: surface receivers and cutting planes, upstream's `formatRSBIN` (read only).
//!
//! Spec, upstream receipts and the canonical dump: `docs/formats/csbin.md`.
//!
//! The file is a header followed by whole C structs written raw. The header stores the length of
//! each struct; this reader steps records by those stored lengths rather than by hard-coded sizes,
//! and reads fields only at their C offsets, so struct padding (uninitialised memory in upstream's
//! writers, different between two runs of the same solver) is never read into the model.
//!
//! Reading is two passes over the bytes: the first validates every count, index and time step
//! without allocating, the second allocates exactly what the file holds. A corrupt header can
//! therefore never make the reader reserve more than the data behind it justifies.

use std::borrow::Cow;
use std::fmt::Write as _;
use std::path::Path;

use super::{Cursor, FormatError, Result, f32_hex, str_token};

/// The only version this reader accepts (`formatRSBIN::VERSION`).
pub const VERSION: i32 = 3;

/// Size of the receiver name field, `char recepteurSName[STRING_SIZE]`.
pub const NAME_SIZE: usize = 255;

const WHAT: &str = "csbin";

/// Byte extent of the fields read from each struct: a stored length below this cannot hold them.
const MIN_HEADER: u32 = 44;
const MIN_NODE: u32 = 12;
const MIN_RECEIVER: u32 = 8 + NAME_SIZE as u32;
const MIN_FACE: u32 = 16;
const MIN_VALUE: u32 = 8;

/// The struct lengths stored in the header, in file order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructLengths {
    /// `t_FileHeader_Length`: the header itself.
    pub header: u32,
    /// `t_nodesPosition_Length`.
    pub node: u32,
    /// `t_RecepteurS_Length`.
    pub receiver: u32,
    /// `t_FaceRS_Length`.
    pub face: u32,
    /// `t_faceValue_Length`.
    pub value: u32,
}

impl StructLengths {
    /// `sizeof` of each struct in upstream's builds, padding included (`t_RecepteurS` 263 + 1,
    /// `t_faceValue` 6 + 2): what every fixture stores.
    pub const NATIVE: StructLengths = StructLengths {
        header: 44,
        node: 12,
        receiver: 264,
        face: 16,
        value: 8,
    };
}

/// `formatRSBIN::RECEPTEURS_RECORD_TYPE`: what the face values mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum RecordType {
    /// Sound level, energy per face (dB once converted).
    SplStandard = 0,
    /// Difference between two standard recordings (dB).
    SplGain = 1,
    /// Reverberation time map (s).
    Tr = 2,
    /// Early decay time map (s).
    Edt = 3,
    /// Pressure map, negative values allowed (Pa).
    Pressure = 4,
    /// Clarity C (dB).
    Clarity = 5,
    /// Definition D (%).
    Definition = 6,
    /// Centre time Ts (s).
    Ts = 7,
    /// Support ST (dB).
    St = 8,
    /// Speech transmission index.
    Sti = 9,
}

impl RecordType {
    /// The enum for a stored value, or `None` outside upstream's ten values.
    pub fn from_i32(v: i32) -> Option<RecordType> {
        use RecordType::*;
        Some(match v {
            0 => SplStandard,
            1 => SplGain,
            2 => Tr,
            3 => Edt,
            4 => Pressure,
            5 => Clarity,
            6 => Definition,
            7 => Ts,
            8 => St,
            9 => Sti,
            _ => return None,
        })
    }

    /// The stored value.
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

/// One stored value of a face (`t_faceValue`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Record {
    /// Index of the time step (`bCourt timeStep`); always below [`Csbin::time_step_count`].
    pub time_step: u16,
    /// Energy, level or time, per [`Csbin::record_type`] (`Floatb energy`).
    pub energy: f32,
}

/// One triangle of a surface receiver (`t_FaceRS` and its `tabTimeStep`).
#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    /// `sommetsIndex`: indices into [`Csbin::nodes`], each checked in range.
    pub vertices: [u32; 3],
    /// The `nbRecords` values, one per time step at which something was recorded.
    pub records: Box<[Record]>,
}

/// One surface receiver or cutting plane (`t_RecepteurS` and its `dataFaces`).
#[derive(Debug, Clone, PartialEq)]
pub struct Receiver {
    /// `xmlIndex`: the receiver's id in the project XML.
    pub xml_index: i32,
    /// `recepteurSName` up to its first NUL (all 255 bytes when it has none). The bytes are the
    /// solver's narrow encoding (cp1252 in upstream's own fixture), not necessarily UTF-8.
    pub name: Vec<u8>,
    /// `quantFaces` faces.
    pub faces: Vec<Face>,
}

impl Receiver {
    /// The name decoded as UTF-8, with invalid bytes replaced.
    pub fn name_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.name)
    }
}

/// A whole `.csbin` file, in file order. Counts are the lengths of the vectors.
#[derive(Debug, Clone, PartialEq)]
pub struct Csbin {
    /// `formatVersion`; always [`VERSION`] after a successful read.
    pub version: i32,
    /// The stored struct lengths the records were stepped by.
    pub lengths: StructLengths,
    /// `nbTimeStep`.
    pub time_step_count: u32,
    /// `timeStep`, the time step in seconds.
    pub time_step: f32,
    /// `recordType`.
    pub record_type: RecordType,
    /// `quantNodes` node positions (x, y, z), in metres.
    pub nodes: Vec<[f32; 3]>,
    /// `quantRS` receivers.
    pub receivers: Vec<Receiver>,
}

/// The header, validated.
struct Header {
    lengths: StructLengths,
    quant_nodes: usize,
    quant_rs: usize,
    time_step_count: u32,
    time_step: f32,
    record_type: RecordType,
}

fn invalid(msg: String) -> FormatError {
    FormatError::Invalid(msg)
}

/// Reads the header and leaves the cursor at the first node.
fn header(c: &mut Cursor<'_>) -> Result<Header> {
    let version = c.i32()?;
    if version != VERSION {
        return Err(FormatError::Version {
            what: WHAT,
            found: version.to_string(),
            expected: "3",
        });
    }
    let lengths = StructLengths {
        header: c.u32()?,
        node: c.u32()?,
        receiver: c.u32()?,
        face: c.u32()?,
        value: c.u32()?,
    };
    let quant_nodes = c.i32()?;
    let quant_rs = c.i32()?;
    let nb_time_step = c.i32()?;
    let time_step = c.f32()?;
    let record_type = c.i32()?;

    for (name, stored, min) in [
        ("header", lengths.header, MIN_HEADER),
        ("node", lengths.node, MIN_NODE),
        ("receiver", lengths.receiver, MIN_RECEIVER),
        ("face", lengths.face, MIN_FACE),
        ("face value", lengths.value, MIN_VALUE),
    ] {
        if stored < min {
            return Err(invalid(format!(
                "stored {name} struct length {stored} is below the {min} bytes its fields need"
            )));
        }
    }
    let quant_nodes = usize::try_from(quant_nodes)
        .map_err(|_| invalid(format!("negative node count {quant_nodes}")))?;
    let quant_rs = usize::try_from(quant_rs)
        .map_err(|_| invalid(format!("negative receiver count {quant_rs}")))?;
    let time_step_count = u32::try_from(nb_time_step)
        .map_err(|_| invalid(format!("negative time step count {nb_time_step}")))?;
    let record_type = RecordType::from_i32(record_type)
        .ok_or_else(|| invalid(format!("unknown record type {record_type}")))?;

    // Honour the stored header length: anything past the fields we know is skipped.
    c.seek(lengths.header as usize)?;
    Ok(Header {
        lengths,
        quant_nodes,
        quant_rs,
        time_step_count,
        time_step,
        record_type,
    })
}

/// Takes one record of `stride` bytes and returns a cursor over it.
fn record<'a>(c: &mut Cursor<'a>, stride: u32) -> Result<Cursor<'a>> {
    Ok(Cursor::new(c.bytes(stride as usize)?, WHAT))
}

/// A receiver record's `xmlIndex`, `quantFaces` and name.
fn receiver_fields<'a>(c: &mut Cursor<'a>, h: &Header) -> Result<(i32, usize, &'a [u8])> {
    let mut r = record(c, h.lengths.receiver)?;
    let xml_index = r.i32()?;
    let quant_faces = r.i32()?;
    let raw = r.bytes(NAME_SIZE)?;
    let name = &raw[..raw.iter().position(|&b| b == 0).unwrap_or(NAME_SIZE)];
    let quant_faces = usize::try_from(quant_faces)
        .map_err(|_| invalid(format!("negative face count {quant_faces}")))?;
    c.check_count(quant_faces, h.lengths.face as usize)?;
    Ok((xml_index, quant_faces, name))
}

/// A face record's vertices (checked against the node count) and `nbRecords`.
fn face_fields(c: &mut Cursor<'_>, h: &Header) -> Result<([u32; 3], usize)> {
    let mut r = record(c, h.lengths.face)?;
    let mut vertices = [0u32; 3];
    for v in &mut vertices {
        let i = r.i32()?;
        *v = match u32::try_from(i) {
            Ok(u) if (u as usize) < h.quant_nodes => u,
            _ => {
                return Err(invalid(format!(
                    "face vertex {i} outside the {} nodes",
                    h.quant_nodes
                )));
            }
        };
    }
    let nb_records = r.i32()?;
    let nb_records = usize::try_from(nb_records)
        .map_err(|_| invalid(format!("negative record count {nb_records}")))?;
    c.check_count(nb_records, h.lengths.value as usize)?;
    Ok((vertices, nb_records))
}

/// A face value record, its time step checked against the time step count.
fn value_fields(c: &mut Cursor<'_>, h: &Header) -> Result<Record> {
    let mut r = record(c, h.lengths.value)?;
    let time_step = r.u16()?;
    r.skip(2)?; // padding: uninitialised in upstream's writer, never read
    let energy = r.f32()?;
    if u32::from(time_step) >= h.time_step_count {
        return Err(invalid(format!(
            "record at time step {time_step} beyond the {} time steps",
            h.time_step_count
        )));
    }
    Ok(Record { time_step, energy })
}

/// First pass: walks and validates everything after the header, allocating nothing.
fn validate(c: &mut Cursor<'_>, h: &Header) -> Result<()> {
    c.check_count(h.quant_nodes, h.lengths.node as usize)?;
    c.skip(h.quant_nodes * h.lengths.node as usize)?;
    c.check_count(h.quant_rs, h.lengths.receiver as usize)?;
    for _ in 0..h.quant_rs {
        let (_, quant_faces, _) = receiver_fields(c, h)?;
        for _ in 0..quant_faces {
            let (_, nb_records) = face_fields(c, h)?;
            for _ in 0..nb_records {
                value_fields(c, h)?;
            }
        }
    }
    Ok(())
}

/// Second pass over already validated bytes: builds the model with exact allocations.
fn build(c: &mut Cursor<'_>, h: Header) -> Result<Csbin> {
    let mut nodes = Vec::with_capacity(h.quant_nodes);
    for _ in 0..h.quant_nodes {
        let mut r = record(c, h.lengths.node)?;
        nodes.push([r.f32()?, r.f32()?, r.f32()?]);
    }
    let mut receivers = Vec::with_capacity(h.quant_rs);
    for _ in 0..h.quant_rs {
        let (xml_index, quant_faces, name) = receiver_fields(c, &h)?;
        let mut faces = Vec::with_capacity(quant_faces);
        for _ in 0..quant_faces {
            let (vertices, nb_records) = face_fields(c, &h)?;
            let mut records = Vec::with_capacity(nb_records);
            for _ in 0..nb_records {
                records.push(value_fields(c, &h)?);
            }
            faces.push(Face {
                vertices,
                records: records.into_boxed_slice(),
            });
        }
        receivers.push(Receiver {
            xml_index,
            name: name.to_vec(),
            faces,
        });
    }
    Ok(Csbin {
        version: VERSION,
        lengths: h.lengths,
        time_step_count: h.time_step_count,
        time_step: h.time_step,
        record_type: h.record_type,
        nodes,
        receivers,
    })
}

/// Parses a `.csbin` file held in memory. Bytes after the last record are ignored, as upstream
/// ignores them.
pub fn read(bytes: &[u8]) -> Result<Csbin> {
    let mut c = Cursor::new(bytes, WHAT);
    let h = header(&mut c)?;
    let body = c.pos();
    validate(&mut c, &h)?;
    c.seek(body)?;
    build(&mut c, h)
}

/// Reads a `.csbin` file. A missing file is [`FormatError::NotFound`]; upstream's `ImportBIN`
/// reports success there, with an uninitialised header.
pub fn read_file(path: &Path) -> Result<Csbin> {
    read(&super::read_file(path)?)
}

/// The canonical dump (grammar in `docs/formats/csbin.md`).
pub fn dump(value: &Csbin) -> String {
    let mut s = String::new();
    // Writing to a String cannot fail.
    let _ = write_dump(&mut s, value);
    s
}

fn write_dump(s: &mut String, v: &Csbin) -> std::fmt::Result {
    let l = &v.lengths;
    writeln!(s, "csbin {}", v.version)?;
    writeln!(
        s,
        "lengths {} {} {} {} {}",
        l.header, l.node, l.receiver, l.face, l.value
    )?;
    writeln!(
        s,
        "timesteps {} {}",
        v.time_step_count,
        f32_hex(v.time_step)
    )?;
    writeln!(s, "recordtype {}", v.record_type.as_i32())?;
    writeln!(s, "nodes {}", v.nodes.len())?;
    for [x, y, z] in &v.nodes {
        writeln!(s, "node {} {} {}", f32_hex(*x), f32_hex(*y), f32_hex(*z))?;
    }
    writeln!(s, "receivers {}", v.receivers.len())?;
    for r in &v.receivers {
        writeln!(s, "receiver {} {}", r.xml_index, str_token(&r.name))?;
        writeln!(s, "faces {}", r.faces.len())?;
        for f in &r.faces {
            let [a, b, c] = f.vertices;
            writeln!(s, "face {a} {b} {c}")?;
            writeln!(s, "records {}", f.records.len())?;
            for rec in f.records.iter() {
                writeln!(s, "record {} {}", rec.time_step, f32_hex(rec.energy))?;
            }
        }
    }
    Ok(())
}

/// The canonical dump of a file, or `error <kind>` when it cannot be read.
pub fn dump_file(path: &Path) -> String {
    match read_file(path) {
        Ok(v) => dump(&v),
        Err(e) => format!("error {}\n", error_kind(&e)),
    }
}

/// The `error <kind>` token of the canonical dump.
fn error_kind(e: &FormatError) -> &'static str {
    match e {
        FormatError::NotFound(_) => "notfound",
        FormatError::Truncated { .. } => "truncated",
        FormatError::Version { .. } => "version",
        FormatError::Invalid(_) => "invalid",
        FormatError::Io(_) => "io",
    }
}
