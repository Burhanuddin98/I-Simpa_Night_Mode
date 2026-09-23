//! `.mbin`: the tetrahedral mesh the solvers read (upstream `formatMBIN::CMBIN`).
//!
//! Byte layout, failure modes, receipts and the canonical dump: `docs/formats/mbin.md`.
//!
//! [`read`] decodes exactly what upstream's `ImportBIN` decodes and accepts every file it would
//! read completely. Where upstream reads past the end of a short file into uninitialised memory,
//! this reader returns [`FormatError::Truncated`]. Index ranges are not checked on read, as
//! upstream does not check them; [`validate`] checks them, and [`write_file`] refuses a mesh that
//! fails it.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

use super::{Cursor, FormatError, Result, f32_hex};

/// Header size: `quantTetra` then `quantNodes`, both `u32`.
pub const HEADER_SIZE: usize = 8;
/// One node: three `f32`.
pub const NODE_SIZE: usize = 12;
/// One tetrahedron: 4 vertex ids, `idVolume`, then 4 faces of 5 `i32` each.
pub const TETRA_SIZE: usize = 100;
/// `marker` of a face that lies on no model face (upstream's default, `mbin.h:81`).
pub const NO_MARKER: i32 = -1;
/// `neighbor` of a face with no tetrahedron behind it (upstream's default, `mbin.h:82`).
pub const NO_NEIGHBOR: i32 = -2;

/// One face of a tetrahedron (upstream `bintetraface`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TetraFace {
    /// Node indices of the face.
    pub vertices: [i32; 3],
    /// Index of the model face (in the scene's `.cbin`) this face lies on; negative for none.
    pub marker: i32,
    /// Index of the tetrahedron across this face; negative for none (upstream writes -2).
    pub neighbor: i32,
}

impl Default for TetraFace {
    fn default() -> Self {
        TetraFace {
            vertices: [0; 3],
            marker: NO_MARKER,
            neighbor: NO_NEIGHBOR,
        }
    }
}

/// One tetrahedron (upstream `bintetrahedre`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tetrahedron {
    /// Node indices of the four corners.
    pub vertices: [i32; 4],
    /// Volume (room) the tetrahedron belongs to.
    pub id_volume: i32,
    /// The four faces, in file order. In upstream's meshes face `i` is opposite vertex `i`.
    pub faces: [TetraFace; 4],
}

/// A whole `.mbin` file (upstream `trimeshmodel`, same field order).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    pub tetrahedra: Vec<Tetrahedron>,
    /// Node coordinates `x, y, z`.
    pub nodes: Vec<[f32; 3]>,
}

fn truncated(len: usize) -> FormatError {
    FormatError::Truncated {
        what: "mbin",
        offset: len,
    }
}

/// Decodes a `.mbin` file. Bytes after the last tetrahedron are ignored, as upstream ignores them.
pub fn read(bytes: &[u8]) -> Result<Mesh> {
    let mut c = Cursor::new(bytes, "mbin");
    let n_tetra = c.u32()? as usize;
    let n_nodes = c.u32()? as usize;

    // The whole body must be present before anything is reserved.
    let body = n_nodes
        .checked_mul(NODE_SIZE)
        .zip(n_tetra.checked_mul(TETRA_SIZE))
        .and_then(|(a, b)| a.checked_add(b));
    match body {
        Some(b) if b <= c.remaining() => {}
        _ => return Err(truncated(bytes.len())),
    }

    let mut nodes = Vec::with_capacity(c.check_count(n_nodes, NODE_SIZE)?);
    for _ in 0..n_nodes {
        nodes.push([c.f32()?, c.f32()?, c.f32()?]);
    }

    let mut tetrahedra = Vec::with_capacity(c.check_count(n_tetra, TETRA_SIZE)?);
    for _ in 0..n_tetra {
        let vertices = [c.i32()?, c.i32()?, c.i32()?, c.i32()?];
        let id_volume = c.i32()?;
        let mut faces = [TetraFace::default(); 4];
        for face in &mut faces {
            face.vertices = [c.i32()?, c.i32()?, c.i32()?];
            face.marker = c.i32()?;
            face.neighbor = c.i32()?;
        }
        tetrahedra.push(Tetrahedron {
            vertices,
            id_volume,
            faces,
        });
    }

    Ok(Mesh { tetrahedra, nodes })
}

/// Reads and decodes a `.mbin` file; a missing file is [`FormatError::NotFound`].
pub fn read_file(path: &Path) -> Result<Mesh> {
    read(&super::read_file(path)?)
}

/// Checks what the solvers index without checking (`lib_interface/coreTypes.cpp:184-241`):
/// every vertex id of a tetrahedron or face names a node, a tetrahedron's four vertices are
/// distinct (upstream's writer refuses otherwise, and the solver exits), and every non-negative
/// neighbour names a tetrahedron. It also checks that both counts fit the `u32` header.
/// Markers index the scene's `.cbin` faces, which this file does not hold, so they are not checked.
pub fn validate(mesh: &Mesh) -> Result<()> {
    let invalid = |msg: String| Err(FormatError::Invalid(format!("mbin: {msg}")));
    if u32::try_from(mesh.nodes.len()).is_err() || u32::try_from(mesh.tetrahedra.len()).is_err() {
        return invalid("more than u32::MAX nodes or tetrahedra".to_string());
    }
    let n_nodes = mesh.nodes.len();
    let n_tetra = mesh.tetrahedra.len();
    let node_ok = |v: i32| usize::try_from(v).is_ok_and(|v| v < n_nodes);
    for (t, tetra) in mesh.tetrahedra.iter().enumerate() {
        if let Some(v) = tetra.vertices.iter().find(|&&v| !node_ok(v)) {
            return invalid(format!(
                "tetrahedron {t} vertex {v} is not one of {n_nodes} nodes"
            ));
        }
        for i in 0..4 {
            for j in i + 1..4 {
                if tetra.vertices[i] == tetra.vertices[j] {
                    return invalid(format!(
                        "tetrahedron {t} repeats vertex {} ({:?})",
                        tetra.vertices[i], tetra.vertices
                    ));
                }
            }
        }
        for (f, face) in tetra.faces.iter().enumerate() {
            if let Some(v) = face.vertices.iter().find(|&&v| !node_ok(v)) {
                return invalid(format!(
                    "tetrahedron {t} face {f} vertex {v} is not one of {n_nodes} nodes"
                ));
            }
            if usize::try_from(face.neighbor).is_ok_and(|n| n >= n_tetra) {
                return invalid(format!(
                    "tetrahedron {t} face {f} neighbour {} is not one of {n_tetra} tetrahedra",
                    face.neighbor
                ));
            }
        }
    }
    Ok(())
}

/// The canonical dump (grammar in `docs/formats/mbin.md`).
pub fn dump(mesh: &Mesh) -> String {
    let mut s = String::with_capacity(32 + 32 * mesh.nodes.len() + 128 * mesh.tetrahedra.len());
    s.push_str("mbin\n");
    let _ = writeln!(s, "nodes {}", mesh.nodes.len());
    for n in &mesh.nodes {
        let _ = writeln!(
            s,
            "node {} {} {}",
            f32_hex(n[0]),
            f32_hex(n[1]),
            f32_hex(n[2])
        );
    }
    let _ = writeln!(s, "tetrahedra {}", mesh.tetrahedra.len());
    for t in &mesh.tetrahedra {
        let v = t.vertices;
        let _ = write!(
            s,
            "tetra {} {} {} {} {}",
            v[0], v[1], v[2], v[3], t.id_volume
        );
        for f in &t.faces {
            let fv = f.vertices;
            let _ = write!(
                s,
                " face {} {} {} {} {}",
                fv[0], fv[1], fv[2], f.marker, f.neighbor
            );
        }
        s.push('\n');
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

/// The canonical dump of a file, or `error <kind>\n` when it cannot be read.
pub fn dump_file(path: &Path) -> String {
    match read_file(path) {
        Ok(mesh) => dump(&mesh),
        Err(e) => format!("error {}\n", error_kind(&e)),
    }
}

/// Encodes a mesh exactly as upstream's `ExportBIN` does. It does not validate; see
/// [`write_file`].
///
/// # Panics
/// If the mesh has more than `u32::MAX` nodes or tetrahedra, which the header cannot count.
pub fn write(mesh: &Mesh) -> Vec<u8> {
    let n_tetra =
        u32::try_from(mesh.tetrahedra.len()).expect("mbin: tetrahedron count exceeds u32");
    let n_nodes = u32::try_from(mesh.nodes.len()).expect("mbin: node count exceeds u32");
    let mut out = Vec::with_capacity(
        HEADER_SIZE + NODE_SIZE * mesh.nodes.len() + TETRA_SIZE * mesh.tetrahedra.len(),
    );
    out.extend_from_slice(&n_tetra.to_le_bytes());
    out.extend_from_slice(&n_nodes.to_le_bytes());
    for n in &mesh.nodes {
        for x in n {
            out.extend_from_slice(&x.to_le_bytes());
        }
    }
    for t in &mesh.tetrahedra {
        for v in t.vertices.iter().chain(std::iter::once(&t.id_volume)) {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for f in &t.faces {
            for v in f.vertices.iter().chain([&f.marker, &f.neighbor]) {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
    }
    out
}

/// Writes a mesh to `path`, after [`validate`]: a mesh the solver would index out of range, or
/// that upstream's `ExportBIN` would refuse, is [`FormatError::Invalid`] and no file is written.
/// (Upstream writes the header and nodes before refusing, leaving a partial file.)
pub fn write_file(mesh: &Mesh, path: &Path) -> Result<()> {
    validate(mesh)?;
    std::fs::write(path, write(mesh))?;
    Ok(())
}

/// Deterministic xorshift64* stream for [`generate`].
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // splitmix64 of the seed, so nearby seeds start far apart; never zero.
        let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        Rng((z ^ (z >> 31)) | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    /// Uniform in `0..n` (n > 0).
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    /// Uniform in `[0, 1)`.
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// A small valid mesh, different for every seed: a jittered box of 1 to 27 cells, each split
/// into the 6 tetrahedra of the Freudenthal triangulation, so the mesh is conforming. Nodes and
/// tetrahedra are shuffled; faces follow upstream's convention (face `i` opposite vertex `i`,
/// wound as TetGen winds them); interior faces carry their neighbour and marker -1, boundary
/// faces neighbour -2 and a random marker. The result passes [`validate`].
pub fn generate(seed: u64) -> Mesh {
    let mut rng = Rng::new(seed);
    let dims = [1 + rng.below(3), 1 + rng.below(3), 1 + rng.below(3)].map(|d| d as usize);
    let origin = [
        rng.unit() * 20.0 - 10.0,
        rng.unit() * 20.0 - 10.0,
        rng.unit() * 20.0 - 10.0,
    ];
    let step = [
        0.5 + rng.unit() * 4.0,
        0.5 + rng.unit() * 4.0,
        0.5 + rng.unit() * 4.0,
    ];
    let grid = [dims[0] + 1, dims[1] + 1, dims[2] + 1];
    let grid_nodes = grid[0] * grid[1] * grid[2];
    let extra_nodes = rng.below(3) as usize; // unused nodes are legal

    // Node permutation: grid node g is stored at perm[g].
    let n_nodes = grid_nodes + extra_nodes;
    let mut perm: Vec<usize> = (0..n_nodes).collect();
    for i in (1..n_nodes).rev() {
        perm.swap(i, rng.below(i as u64 + 1) as usize);
    }
    let mut nodes = vec![[0f32; 3]; n_nodes];
    let grid_index = |x: usize, y: usize, z: usize| x + grid[0] * (y + grid[1] * z);
    for z in 0..grid[2] {
        for y in 0..grid[1] {
            for x in 0..grid[0] {
                let ijk = [x, y, z];
                let mut p = [0f32; 3];
                for a in 0..3 {
                    // Jitter stays under a tenth of a cell, so no tetrahedron inverts.
                    let jitter = (rng.unit() - 0.5) * 0.2 * step[a];
                    p[a] = origin[a] + ijk[a] as f32 * step[a] + jitter;
                }
                nodes[perm[grid_index(x, y, z)]] = p;
            }
        }
    }
    for g in grid_nodes..n_nodes {
        nodes[perm[g]] = [rng.unit() * 50.0, rng.unit() * 50.0, rng.unit() * 50.0];
    }

    // Freudenthal: one tetrahedron per axis order, walking from corner (0,0,0) to (1,1,1).
    const ORDERS: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let mut corners: Vec<[i32; 4]> = Vec::new();
    for z in 0..dims[2] {
        for y in 0..dims[1] {
            for x in 0..dims[0] {
                for order in ORDERS {
                    let mut at = [x, y, z];
                    let mut tet = [0i32; 4];
                    tet[0] = perm[grid_index(at[0], at[1], at[2])] as i32;
                    for (k, &axis) in order.iter().enumerate() {
                        at[axis] += 1;
                        tet[k + 1] = perm[grid_index(at[0], at[1], at[2])] as i32;
                    }
                    corners.push(tet);
                }
            }
        }
    }
    for i in (1..corners.len()).rev() {
        corners.swap(i, rng.below(i as u64 + 1) as usize);
    }

    // Upstream's meshes have (A-D).((B-D)x(C-D)) < 0; swap two corners where it is positive.
    for tet in &mut corners {
        let p = tet.map(|v| nodes[v as usize].map(f64::from));
        let d = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let (a, b, c) = (d(p[0], p[3]), d(p[1], p[3]), d(p[2], p[3]));
        let cross = [
            b[1] * c[2] - b[2] * c[1],
            b[2] * c[0] - b[0] * c[2],
            b[0] * c[1] - b[1] * c[0],
        ];
        if a[0] * cross[0] + a[1] * cross[1] + a[2] * cross[2] > 0.0 {
            tet.swap(0, 1);
        }
    }

    // Face i is opposite vertex i, wound as in upstream's files (mbin.md, "Conventions").
    const FACE_CORNERS: [[usize; 3]; 4] = [[1, 3, 2], [2, 3, 0], [0, 3, 1], [1, 2, 0]];
    let key = |mut f: [i32; 3]| {
        f.sort_unstable();
        f
    };
    let mut owners: HashMap<[i32; 3], Vec<usize>> = HashMap::new();
    for (t, tet) in corners.iter().enumerate() {
        for fc in FACE_CORNERS {
            owners.entry(key(fc.map(|i| tet[i]))).or_default().push(t);
        }
    }
    let n_markers = 1 + rng.below(40);
    let id_volume = rng.below(4) as i32;
    let tetrahedra = corners
        .iter()
        .enumerate()
        .map(|(t, &tet)| {
            let faces = FACE_CORNERS.map(|fc| {
                let vertices = fc.map(|i| tet[i]);
                let across = owners[&key(vertices)].iter().copied().find(|&o| o != t);
                match across {
                    Some(o) => TetraFace {
                        vertices,
                        marker: NO_MARKER,
                        neighbor: o as i32,
                    },
                    None => TetraFace {
                        vertices,
                        marker: rng.below(n_markers) as i32,
                        neighbor: NO_NEIGHBOR,
                    },
                }
            });
            Tetrahedron {
                vertices: tet,
                id_volume,
                faces,
            }
        })
        .collect();
    Mesh { tetrahedra, nodes }
}
