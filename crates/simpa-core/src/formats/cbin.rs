//! `.cbin`: the scene mesh the solvers read. It holds the vertices and triangles of the room, each
//! triangle carrying a material id, a surface-receiver id and a fitting (encombrement) id.
//!
//! Port of `formatCoreBIN::CformatBIN` (`lib_interface/input_output/bin.cpp`, `bin.h`). The byte
//! layout, the node-walking rules and the canonical dump are specified in `docs/formats/cbin.md`.
//! This reader agrees with upstream's `ImportBIN` on every input where upstream's result is defined.
//! Where upstream reads past the end of the file and keeps going on uninitialised values, this
//! reader returns [`FormatError::Truncated`] instead. Unlike upstream, it also rejects a face whose
//! vertex index is out of range, because the solver would read past its vertex array
//! (`coreinitialisation.cpp:416-421`).

use std::fmt::Write as _;
use std::path::Path;

use super::{Cursor, FormatError, Result, f32_hex};

/// The only version upstream's reader accepts (`bin.cpp:39-40`, checked at `bin.cpp:134-137`).
pub const MAJOR: u32 = 1;
/// See [`MAJOR`].
pub const MINOR: u32 = 0;

/// `NODE_TYPE_VERTICES` (`bin.h:135`).
const NODE_VERTICES: u16 = 0;
/// `NODE_TYPE_GROUP` (`bin.h:136`).
const NODE_GROUP: u16 = 1;

/// Node header: `nodeType` u16, 2 bytes of padding, `firtSon` u32, `nextBrother` u32.
const NODE_HEADER_LEN: usize = 12;
/// `groupName` (255 bytes) and the one byte skipped after it (`bin.cpp:297-298`).
const GROUP_NAME_LEN: usize = 256;
/// One vertex on disk: `x`, `y`, `z` as f32.
const VERTEX_LEN: usize = 12;
/// One face on disk: `a`, `b`, `c`, `idMaterial` as u32, then `idRs`, `idEn` as i32.
const FACE_LEN: usize = 24;

/// A vertex, `formatCoreBIN::t_pos` (`bin.h:53-65`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// A triangle, `formatCoreBIN::ioFace` (`bin.h:69-86`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Face {
    /// Vertex indices.
    pub a: u32,
    pub b: u32,
    pub c: u32,
    /// Material id (`idMat`); 0 for none (`bin.h:83`).
    pub id_mat: u32,
    /// Surface-receiver id (`idRs`); -1 for none.
    pub id_rs: i32,
    /// Fitting (encombrement) id (`idEn`); -1 for none.
    pub id_en: i32,
}

/// The scene, `formatCoreBIN::ioModel` (`bin.h:91-94`), fields in upstream's order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Model {
    pub faces: Vec<Face>,
    pub vertices: Vec<Vertex>,
}

/// Parses a `.cbin` file held in memory.
///
/// Every node is read to its end: a vertices node or a group node appends its records to the
/// model, and the walk ends after the first node whose `nextBrother` is 0. Bytes after that node
/// are ignored, as upstream ignores them.
pub fn read(bytes: &[u8]) -> Result<Model> {
    let mut c = Cursor::new(bytes, "cbin");
    let major = c.u32()?;
    let minor = c.u32()?;
    if major != MAJOR || minor != MINOR {
        return Err(FormatError::Version {
            what: "cbin",
            found: format!("{major}.{minor}"),
            expected: "1.0",
        });
    }
    let mut model = Model::default();
    loop {
        // ProcessNode, bin.cpp:236-271.
        let node_start = c.pos();
        let node_type = c.u16()?;
        c.skip(2)?;
        let _first_son = c.u32()?;
        let next_brother = c.u32()?;
        match node_type {
            NODE_VERTICES => read_vertices(&mut c, &mut model.vertices)?,
            NODE_GROUP => read_group(&mut c, &mut model.faces)?,
            other => {
                return Err(FormatError::Invalid(format!(
                    "cbin: unknown node type {other} at byte {node_start}"
                )));
            }
        }
        if next_brother == 0 {
            break;
        }
        // Upstream jumps only forwards; a nextBrother at or behind the current position is
        // ignored and the next node is read where this one ended (bin.cpp:259-266).
        let next = next_brother as usize;
        if next > c.pos() {
            c.seek(next)?;
        }
    }
    check_indices(&model)?;
    Ok(model)
}

/// ProcessNodeVertices, bin.cpp:273-289.
fn read_vertices(c: &mut Cursor<'_>, out: &mut Vec<Vertex>) -> Result<()> {
    let n = c.u32()? as usize;
    c.check_count(n, VERTEX_LEN)?;
    out.reserve_exact(n);
    for _ in 0..n {
        let x = c.f32()?;
        let y = c.f32()?;
        let z = c.f32()?;
        out.push(Vertex { x, y, z });
    }
    Ok(())
}

/// ProcessNodeGroup, bin.cpp:291-320. The group name is read and discarded by upstream.
fn read_group(c: &mut Cursor<'_>, out: &mut Vec<Face>) -> Result<()> {
    c.skip(GROUP_NAME_LEN)?;
    let n = c.u32()? as usize;
    c.check_count(n, FACE_LEN)?;
    out.reserve_exact(n);
    for _ in 0..n {
        let a = c.u32()?;
        let b = c.u32()?;
        let cc = c.u32()?;
        let id_mat = c.u32()?;
        let id_rs = c.i32()?;
        let id_en = c.i32()?;
        out.push(Face {
            a,
            b,
            c: cc,
            id_mat,
            id_rs,
            id_en,
        });
    }
    Ok(())
}

/// Every face must index an existing vertex. Upstream's reader does not check this; its solver
/// indexes `pvertices` with these values unchecked.
fn check_indices(model: &Model) -> Result<()> {
    let n = model.vertices.len();
    for (i, f) in model.faces.iter().enumerate() {
        for v in [f.a, f.b, f.c] {
            if v as usize >= n {
                return Err(FormatError::Invalid(format!(
                    "cbin: face {i} uses vertex {v} but the file holds {n} vertices"
                )));
            }
        }
    }
    Ok(())
}

/// Reads and parses a `.cbin` file. A missing file is [`FormatError::NotFound`].
pub fn read_file(path: &Path) -> Result<Model> {
    read(&super::read_file(path)?)
}

/// The canonical dump (grammar in `docs/formats/cbin.md`): the version line, the vertex list,
/// then the face list.
pub fn dump(value: &Model) -> String {
    let mut s = String::with_capacity(32 + 27 * value.vertices.len() + 40 * value.faces.len());
    let _ = writeln!(s, "cbin {MAJOR} {MINOR}");
    let _ = writeln!(s, "vertices {}", value.vertices.len());
    for v in &value.vertices {
        let _ = writeln!(s, "{} {} {}", f32_hex(v.x), f32_hex(v.y), f32_hex(v.z));
    }
    let _ = writeln!(s, "faces {}", value.faces.len());
    for f in &value.faces {
        let _ = writeln!(
            s,
            "{} {} {} {} {} {}",
            f.a, f.b, f.c, f.id_mat, f.id_rs, f.id_en
        );
    }
    s
}

/// The canonical dump of a file, or `error <kind>` when it cannot be read.
pub fn dump_file(path: &Path) -> String {
    match read_file(path) {
        Ok(m) => dump(&m),
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

/// Offset of the group node, which the vertices node's `nextBrother` holds, or `None` when it
/// does not fit the field's 32 bits.
fn group_offset(n_vertices: usize) -> Option<u32> {
    let bytes = n_vertices
        .checked_mul(VERTEX_LEN)?
        .checked_add(8 + NODE_HEADER_LEN + 4)?;
    u32::try_from(bytes).ok()
}

/// Size in bytes of the file [`write`] produces for this model.
fn written_len(value: &Model) -> Option<usize> {
    let faces = value.faces.len().checked_mul(FACE_LEN)?;
    (group_offset(value.vertices.len())? as usize)
        .checked_add(NODE_HEADER_LEN + GROUP_NAME_LEN + 4)?
        .checked_add(faces)
}

/// Checks that [`write`] can represent the model: the face count and the group node's offset fit
/// the file's 32-bit fields, and every face indexes an existing vertex so [`read`] accepts it.
fn check_writable(value: &Model) -> Result<()> {
    if u32::try_from(value.faces.len()).is_err() || written_len(value).is_none() {
        return Err(FormatError::Invalid(format!(
            "cbin: {} vertices and {} faces do not fit the format's 32-bit fields",
            value.vertices.len(),
            value.faces.len()
        )));
    }
    check_indices(value)
}

/// Serialises the model exactly as upstream's `ExportBIN` does (`bin.cpp:149-229`): header 1.0,
/// one vertices node whose `nextBrother` points at the group node, then one group node with an
/// all-zero name, `nextBrother` 0. Padding and `firtSon` are written as zeros.
///
/// Does not validate face indices; [`write_file`] does. Panics if the face count or the group
/// node's offset does not fit in 32 bits: 2^32 faces or about 357 million vertices.
pub fn write(value: &Model) -> Vec<u8> {
    let group_at =
        group_offset(value.vertices.len()).expect("cbin: group node offset exceeds u32::MAX");
    // The group node's offset fits in 32 bits, so the vertex count does too.
    let n_vertices = value.vertices.len() as u32;
    let n_faces = u32::try_from(value.faces.len()).expect("cbin: more than u32::MAX faces");
    let mut out = Vec::with_capacity(written_len(value).unwrap_or(0));
    out.extend_from_slice(&MAJOR.to_le_bytes());
    out.extend_from_slice(&MINOR.to_le_bytes());
    write_node_header(&mut out, NODE_VERTICES, group_at);
    out.extend_from_slice(&n_vertices.to_le_bytes());
    for v in &value.vertices {
        out.extend_from_slice(&v.x.to_le_bytes());
        out.extend_from_slice(&v.y.to_le_bytes());
        out.extend_from_slice(&v.z.to_le_bytes());
    }
    write_node_header(&mut out, NODE_GROUP, 0);
    out.extend_from_slice(&[0u8; GROUP_NAME_LEN]);
    out.extend_from_slice(&n_faces.to_le_bytes());
    for f in &value.faces {
        out.extend_from_slice(&f.a.to_le_bytes());
        out.extend_from_slice(&f.b.to_le_bytes());
        out.extend_from_slice(&f.c.to_le_bytes());
        out.extend_from_slice(&f.id_mat.to_le_bytes());
        out.extend_from_slice(&f.id_rs.to_le_bytes());
        out.extend_from_slice(&f.id_en.to_le_bytes());
    }
    out
}

/// writeNode, bin.cpp:322-336, with `nodeHeadSize` 0 as every upstream caller passes it.
fn write_node_header(out: &mut Vec<u8>, node_type: u16, next_brother: u32) {
    out.extend_from_slice(&node_type.to_le_bytes());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&next_brother.to_le_bytes());
}

/// Writes the model to `path`. Refuses, before creating the file, a model [`read`] would reject
/// or the format cannot hold.
pub fn write_file(value: &Model, path: &Path) -> Result<()> {
    check_writable(value)?;
    std::fs::write(path, write(value))?;
    Ok(())
}

/// A small valid model, different per seed: 1 to 40 vertices with finite coordinates (some -0.0
/// and subnormal), 0 to 60 faces indexing them, and ids spanning the sentinels (-1, and
/// `idMat` 0xFFFFFFFF as upstream's own test writes it).
pub fn generate(seed: u64) -> Model {
    let mut rng = XorShift::new(seed);
    let n_vertices = 1 + rng.below(40) as usize;
    let n_faces = rng.below(61) as usize;
    let vertices = (0..n_vertices)
        .map(|_| Vertex {
            x: rng.coord(),
            y: rng.coord(),
            z: rng.coord(),
        })
        .collect();
    let faces = (0..n_faces)
        .map(|_| Face {
            a: rng.below(n_vertices as u64) as u32,
            b: rng.below(n_vertices as u64) as u32,
            c: rng.below(n_vertices as u64) as u32,
            id_mat: match rng.below(4) {
                0 => 0,
                1 => u32::MAX,
                2 => rng.below(200) as u32,
                _ => rng.next() as u32,
            },
            id_rs: rng.id(),
            id_en: rng.id(),
        })
        .collect();
    Model { faces, vertices }
}

/// xorshift64* with a splitmix64-scrambled seed, so seeds 0, 1, 2... give unrelated streams.
struct XorShift(u64);

impl XorShift {
    fn new(seed: u64) -> Self {
        let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        XorShift(if z == 0 { 0x2545_f491_4f6c_dd1d } else { z })
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// A finite coordinate: usually a room-sized value, sometimes an edge case of the bit layout.
    fn coord(&mut self) -> f32 {
        match self.below(8) {
            0 => -0.0,
            1 => f32::from_bits(self.next() as u32 & 0x807f_ffff), // subnormal or zero
            2 => loop {
                let v = f32::from_bits(self.next() as u32);
                if v.is_finite() {
                    break v;
                }
            },
            _ => (self.next() as u32 as f32 / u32::MAX as f32 - 0.5) * 100.0,
        }
    }

    /// A receiver or fitting id: -1 (none), a small id, or any i32.
    fn id(&mut self) -> i32 {
        match self.below(3) {
            0 => -1,
            1 => self.below(5000) as i32,
            _ => self.next() as i32,
        }
    }
}
