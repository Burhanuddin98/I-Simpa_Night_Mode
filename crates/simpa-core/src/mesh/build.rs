//! The `.mbin` built from one TetGen output set (`.node`, `.ele`, `.face`, `.neigh`), as
//! upstream's GUI builds it: `CObjet3D::LoadMaillage` reads the four files
//! (`isimpa/3dengine/Core/Objet3D_maillage.cpp:54-488`) and `CObjet3D::SaveMaillage` writes the
//! `.mbin` from what it read (`:830-898`, called with `toRealCoords = true` by
//! `data_manager/projet.cpp:769`). Two departures, `docs/m5-m6-design.md` decisions 1 and 5,
//! and nothing else.
//!
//! Evidence (`tests/mesh_mbin_parity.rs`): upstream's 2019 tutorial-1 `tetramesh.mbin` is rebuilt
//! from the TetGen output it was made from byte for byte, `idVolume` apart (decision 1), and so is
//! tutorial 3's, given upstream's region ids.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::formats::cbin;
use crate::formats::mbin::{self, NO_MARKER, NO_NEIGHBOR, TetraFace, Tetrahedron};
use crate::formats::tetgen::{self, EleFile, FaceFile, HULL, NeighFile, NodeFile};

/// The `simpa-core` package folder this builder was compiled from. `tests/mesh_mbin_parity.rs`
/// checks it against the package cargo runs the tests for: cargo judges a path package up to date
/// by file times alone, so a build folder that two checkouts share can hand one of them a library
/// compiled from the other's sources, and the byte comparisons would then pass or fail on code
/// that is not in the tree under test.
#[doc(hidden)]
pub const COMPILED_FROM: &str = env!("CARGO_MANIFEST_DIR");

/// Face `i` of a tetrahedron `(a, b, c, d)` is the face opposite corner `i`, wound as upstream
/// winds it: `(b,d,c)`, `(c,d,a)`, `(a,d,b)`, `(b,c,a)` (`Objet3D_maillage.cpp:182-185`).
pub const FACE_CORNERS: [[usize; 3]; 4] = [[1, 3, 2], [2, 3, 0], [0, 3, 1], [1, 2, 0]];

/// Where each `.mbin` corner comes from in its `.ele` row: `.mbin` corner `k` is `.ele` corner
/// `UPSTREAM_CORNERS[k]`, so TetGen's `(a, b, c, d)` is written `(d, c, b, a)`. See
/// [`upstream_order`].
pub const UPSTREAM_CORNERS: [usize; 4] = [3, 2, 1, 0];

/// One `.ele` or `.neigh` row in the order upstream's GUI stores it: TetGen's `(a, b, c, d)`
/// becomes `(d, c, b, a)`.
///
/// `LoadEleFile` builds the corners as `ivec4 sommets(ToInt(GetNextToken()) - 1, ...)`, four
/// token reads as the four arguments of one constructor call (`Objet3D_maillage.cpp:161-164`).
/// C++ leaves the order in which a call's arguments are evaluated unspecified, and MSVC, which
/// builds upstream's Windows releases, evaluates them right to left: the first corner read lands
/// in `d`. `LoadNeighFile` reads the four neighbours the same way (`:435-438`), so the neighbour
/// list is reversed with the corners and neighbour `i` stays the tetrahedron across the face
/// opposite corner `i`.
///
/// The cause is inferred from the code; the effect is measured. With this order the 2019
/// tutorial-1 `.mbin` is rebuilt byte for byte; in TetGen's own order all 2,257 of its
/// tetrahedron records differ (`tests/mesh_mbin_parity.rs`).
///
/// `(a,b,c,d) -> (d,c,b,a)` is the product of two swaps, `(a d)(b c)`, an even permutation, so the
/// orientation determinant `(A-D)·((B-D)×(C-D))` keeps its sign: a mesh in which every
/// tetrahedron has the `.mbin`'s negative orientation in TetGen's order has it in upstream's.
pub fn upstream_order<T: Copy>(row: [T; 4]) -> [T; 4] {
    UPSTREAM_CORNERS.map(|k| row[k])
}

/// Upstream's `UnitizeVar`: the centre and scale `CObjet3D::Unitize` fits to the scene
/// (`isimpa/3dengine/Core/Objet3D.cpp:527-571`), in the GUI's GL axes, which are the scene's
/// `(x, z, -y)`. The GUI holds every TetGen node in the frame this defines and converts it back
/// when it writes the `.mbin`, so each node's `f32` coordinates go through
/// [`Unitize::round_trip`].
///
/// **Which vertices.** Upstream fits `_pVertices`, the GUI's list of scene vertices, each time it
/// replaces that list: after loading a scene file or a project's `sceneMesh.bin`
/// (`Objet3D.cpp:441`), building a cuboid (`:480`), reloading a corrected `.poly` (`:393`) or a
/// boundary mesh (`Objet3D_maillage.cpp:1100`); nothing else writes the list. The GUI hands the
/// same list, verbatim and in order, to the solvers as the scene part of the `.cbin`
/// (`ToCBINFormat`, `Objet3D_maillage.cpp:775-779`; the fitting zones' triangles follow it) and to
/// TetGen as the first nodes of the `.poly` (`_SavePOLY`, `:938-942`). We fit the vertices of our
/// own `.cbin` ([`Unitize::of_scene`]): the same rule on the same file's list, so when our `.cbin`
/// is upstream's, the frame is upstream's.
///
/// Where the two lists differ, the frames can. Upstream keeps one copy of a vertex per face
/// corner for an STL (`stl.cpp:274-287`) or a cuboid it builds (`Objet3D.cpp:462-471`), and so
/// does tutorial 1's `.cbin`, 36 vertices for 12 faces, where our import welds them to 8. In such
/// a list of a closed surface every point appears at least three times, so leaving the last copy
/// out changes nothing, and the frames differ exactly when **our** last vertex is the scene's
/// only vertex at its minimum or maximum on some axis (a box has none). In a list upstream keeps
/// one copy of each point in (a `.ply`, tutorial 3's `.cbin`), its last vertex is left out as
/// ours is. Tutorials 1 and 3 fit the same frame from upstream's list as from ours; no upstream
/// file at hand has a last vertex that decides the frame (`tests/mesh_mbin_parity.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Unitize {
    /// `(cx, cy, cz)`: the middle of the scene's box, GL axes.
    pub centre: [f32; 3],
    /// 2 divided by the box's longest side.
    pub scale: f32,
}

impl Unitize {
    /// Fits a list of scene vertices, in scene axes and in order, as `CObjet3D::Unitize` fits
    /// `_pVertices`:
    /// - each vertex in GL axes, `(x, z, -y)`: the project loader sets exactly that
    ///   (`3dengine/Core/bin.cpp:78`), and so does `CommonCoordsToGlCoords` with the identity
    ///   `(0, 0, 0, 1)` in the `.poly`, `.stl` and `.ply` loaders (`Objet3D.cpp:347, 1017, 1055,
    ///   1094`);
    /// - the box of every vertex **but the last**: the loop runs `v < size() - 1` (`:542`), with
    ///   strict comparisons (`:544-549`);
    /// - `w, h, d = |hi - lo|` in `f32` (`:552-554`, `Abs` at `:114-119`);
    /// - the centre `(lo + hi) / 2.0`: the sum in `f32`, the halving in `f64`, narrowed back to
    ///   `f32` (`:556-558`);
    /// - `scale = 2.0 / max(w, h, d)`, divided in `f64` and narrowed to `f32` (`:560`, `Max` at
    ///   `:107-112`).
    ///
    /// Refuses a list with no vertex, and one whose box (last vertex left out) has no extent or is
    /// not finite: upstream would divide by zero.
    pub fn fit(vertices: &[[f32; 3]]) -> Result<Self, String> {
        let gl = |v: &[f32; 3]| [v[0], v[2], -v[1]];
        let Some(first) = vertices.first() else {
            return Err("the scene has no vertex to fit upstream's UnitizeVar to".to_string());
        };
        let (mut lo, mut hi) = (gl(first), gl(first));
        for v in &vertices[..vertices.len() - 1] {
            let g = gl(v);
            for k in 0..3 {
                if g[k] < lo[k] {
                    lo[k] = g[k];
                }
                if g[k] > hi[k] {
                    hi[k] = g[k];
                }
            }
        }
        let abs = |f: f32| if f < 0.0 { -f } else { f };
        let max = |a: f32, b: f32| if a > b { a } else { b };
        let [w, h, d] = [0, 1, 2].map(|k| abs(hi[k] - lo[k]));
        let centre = [0, 1, 2].map(|k| (f64::from(lo[k] + hi[k]) / 2.0) as f32);
        let longest = max(max(w, h), d);
        let scale = (2.0 / f64::from(longest)) as f32;
        if !(longest > 0.0
            && scale.is_finite()
            && scale > 0.0
            && centre.iter().all(|c| c.is_finite()))
        {
            return Err(format!(
                "the scene's box, last vertex left out as upstream's Unitize leaves it out, is \
                 {w} x {h} x {d} (GL axes): no UnitizeVar fits it"
            ));
        }
        Ok(Unitize { centre, scale })
    }

    /// [`Unitize::fit`] on the vertices of `scene`, the `.cbin` model a run hands the solvers
    /// (`config_xml::scene_mesh`: the project's vertices in order, as `f32`). See [`Unitize`] for
    /// why that list, and when it is not upstream's.
    pub fn of_scene(scene: &cbin::Model) -> Result<Self, String> {
        let vertices: Vec<[f32; 3]> = scene.vertices.iter().map(|v| [v.x, v.y, v.z]).collect();
        Self::fit(&vertices)
    }

    /// What the GUI does to one TetGen node between the `.node` and the `.mbin`, in `f32`:
    /// `CommonCoordsToGlCoords` when `LoadNodeFile` reads it (`Objet3D_maillage.cpp:94-99`), then
    /// `GlCoordsToCommonCoords` when `GetTetraMesh(.., true)` writes it (`:855-859`), with the
    /// formulas of `3dengine/Core/Mathlib.h:50-67`. A coordinate moves by a few ulps at most (up
    /// to 4.8e-7 m on tutorial 1), and a 0 in y always comes back as -0.0.
    pub fn round_trip(&self, p: [f32; 3]) -> [f32; 3] {
        let [cx, cy, cz] = self.centre;
        let w = self.scale;
        let [x, y, z] = p;
        let (gx, gy, gz) = ((x - cx) * w, (z - cy) * w, (-y - cz) * w);
        [gx / w + cx, -(gz / w + cz), gy / w + cy]
    }
}

/// The four files of one TetGen run.
#[derive(Clone, Debug, PartialEq)]
pub struct TetgenOutput {
    pub node: NodeFile,
    pub ele: EleFile,
    pub face: FaceFile,
    pub neigh: NeighFile,
}

/// The paths of one TetGen output set: `<dir>/<base>.1.{node,ele,face,neigh}`, and the
/// `<base>_skipped.face` TetGen writes when it skips facets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputPaths {
    pub node: PathBuf,
    pub ele: PathBuf,
    pub face: PathBuf,
    pub neigh: PathBuf,
    pub skipped_face: PathBuf,
}

impl OutputPaths {
    pub fn new(dir: &Path, base: &str) -> Self {
        let at = |suffix: &str| dir.join(format!("{base}{suffix}"));
        OutputPaths {
            node: at(".1.node"),
            ele: at(".1.ele"),
            face: at(".1.face"),
            neigh: at(".1.neigh"),
            skipped_face: at("_skipped.face"),
        }
    }

    /// The `.node`, `.ele` and `.face` files that do not exist.
    pub fn missing_mesh_files(&self) -> Vec<&Path> {
        [&self.node, &self.ele, &self.face]
            .into_iter()
            .filter(|p| !p.is_file())
            .map(PathBuf::as_path)
            .collect()
    }
}

impl TetgenOutput {
    /// Reads the four files with [`crate::formats::tetgen`]'s readers.
    pub fn read(paths: &OutputPaths) -> Result<Self, String> {
        let read = |p: &Path| {
            crate::formats::read_file(p)
                .map_err(|e| format!("{}: {e}", p.display()))
                .map(|bytes| (p.to_path_buf(), bytes))
        };
        let fail = |p: &Path, e: crate::formats::FormatError| format!("{}: {e}", p.display());
        let (p, b) = read(&paths.node)?;
        let node = tetgen::read_node(&b).map_err(|e| fail(&p, e))?;
        let (p, b) = read(&paths.ele)?;
        let ele = tetgen::read_ele(&b).map_err(|e| fail(&p, e))?;
        let (p, b) = read(&paths.face)?;
        let face = tetgen::read_face(&b).map_err(|e| fail(&p, e))?;
        let (p, b) = read(&paths.neigh)?;
        let neigh = tetgen::read_neigh(&b).map_err(|e| fail(&p, e))?;
        Ok(TetgenOutput {
            node,
            ele,
            face,
            neigh,
        })
    }
}

/// What the builder saw and did, for the manifest.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BuildStats {
    pub nodes: usize,
    pub tetrahedra: usize,
    /// Rows of the `.face` file.
    pub face_rows: usize,
    /// Tetrahedron faces written with a scene-face marker (both sides of an internal facet
    /// count).
    pub marked_tet_faces: usize,
    /// Tetrahedron faces on a facet whose marker is at or above the scene's face count (a box
    /// fitting zone's), written as -1.
    pub zone_tet_faces: usize,
    /// Tetrahedron faces with no neighbour (-2).
    pub hull_tet_faces: usize,
    /// Each TetGen region attribute seen, the `idVolume` written for it, and its tetrahedra.
    pub attributes: Vec<AttributeMap>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributeMap {
    pub attribute: i64,
    pub id_volume: i32,
    pub tetrahedra: usize,
}

fn sorted(mut f: [i32; 3]) -> [i32; 3] {
    f.sort_unstable();
    f
}

/// Builds the `.mbin` of `out` as upstream's GUI builds it:
/// - each node read as `f64`, narrowed to `f32` (`Convertor::ToFloat` parses a `double` and
///   returns a `float`, `manager/sppsString.cpp:43-50`), then put through `unitize`'s round trip
///   ([`Unitize::round_trip`]);
/// - tetrahedra in `.ele` order, corners in [`upstream_order`], minus the index base;
/// - faces in [`FACE_CORNERS`] order of those corners; face `i`'s neighbour is `.neigh` row in
///   [`upstream_order`], column `i`, minus the base, and TetGen's -1 is written as -2
///   (`Objet3D_maillage.cpp:433-442`). There is no fallback: the neighbours come from the
///   `.neigh` or not at all;
/// - markers from the `.face` rows, matched by unordered vertex triple to **every** tetrahedron
///   face carrying that triangle, so both sides of an internal facet get it
///   (`Objet3D_maillage.cpp:369-381`).
///
/// The two departures from upstream:
/// - `idVolume` = the last `.ele` attribute (`-A`'s region) when it is one of `fittings`, and 0
///   (the room) otherwise (decision 1). Upstream writes the attribute unchanged
///   (`Objet3D_maillage.cpp:165, 868`), so its room is 1;
/// - a marker at or above `scene_faces` (a box fitting zone's triangle, decision 5) or below 0
///   is written as -1.
///
/// Refuses, with the reason, files that disagree with each other ([`tetgen::check_mesh`]), a mesh
/// without the `-A` attribute or with a non-integral one, second-order tetrahedra, a `.face`
/// without markers, one triangle listed twice with different markers, a `.face` row that no
/// tetrahedron has, and a node that is not finite after the round trip.
pub fn build_mbin(
    out: &TetgenOutput,
    scene_faces: usize,
    fittings: &[i32],
    unitize: &Unitize,
) -> Result<(mbin::Mesh, BuildStats), String> {
    tetgen::check_mesh(&out.node, &out.ele, Some(&out.face), Some(&out.neigh))
        .map_err(|e| e.to_string())?;
    let (ele, face, neigh) = (&out.ele, &out.face, &out.neigh);
    if ele.corners != 4 {
        return Err(format!(
            ".ele has {} corners per tetrahedron; the solvers read 4",
            ele.corners
        ));
    }
    if ele.attributes_per_tet == 0 {
        return Err(".ele has no region attribute: TetGen ran without -A".to_string());
    }
    let base = out.node.first;
    let nodes = out
        .node
        .points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let n = unitize.round_trip(p.map(|c| c as f32));
            if n.iter().all(|c| c.is_finite()) {
                Ok(n)
            } else {
                Err(format!(
                    "node {i} is not finite as a 32-bit float after upstream's GL round trip"
                ))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    let markers: &[i32] = match &face.markers {
        Some(m) => m,
        // TetGen allocates no marker list for an empty .face.
        None if face.faces.is_empty() => &[],
        None => return Err(".face has no marker column".to_string()),
    };
    // Triangle (sorted, 0-based) -> (marker, matched by some tetrahedron face).
    let mut by_triangle: HashMap<[i32; 3], (i32, bool)> = HashMap::with_capacity(face.faces.len());
    for (row, (f, &m)) in face.faces.iter().zip(markers).enumerate() {
        let key = sorted(f.map(|v| v - base));
        if let Some(&(other, _)) = by_triangle.get(&key) {
            if other != m {
                return Err(format!(
                    ".face row {row} lists triangle {f:?} with marker {m}, and an earlier row \
                     with marker {other}"
                ));
            }
        } else {
            by_triangle.insert(key, (m, false));
        }
    }

    let mut stats = BuildStats {
        nodes: nodes.len(),
        tetrahedra: ele.len(),
        face_rows: face.faces.len(),
        ..BuildStats::default()
    };
    let mut attributes: BTreeMap<i64, (i32, usize)> = BTreeMap::new();
    let mut tetrahedra = Vec::with_capacity(ele.len());
    for t in 0..ele.len() {
        let c = ele.tet(t);
        let vertices = upstream_order([c[0] - base, c[1] - base, c[2] - base, c[3] - base]);
        let neighbours = upstream_order(neigh.neighbors[t]);
        let attr = *ele
            .tet_attributes(t)
            .last()
            .expect("attributes_per_tet > 0");
        if attr.fract() != 0.0 || !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&attr) {
            return Err(format!(
                "tetrahedron {t} has region attribute {attr}, not an integer"
            ));
        }
        let attr = attr as i64;
        let id_volume = if fittings.iter().any(|&f| i64::from(f) == attr) {
            attr as i32
        } else {
            0
        };
        attributes.entry(attr).or_insert((id_volume, 0)).1 += 1;
        let faces = std::array::from_fn(|i| {
            let fv = FACE_CORNERS[i].map(|k| vertices[k]);
            let n = neighbours[i];
            let neighbor = if n == HULL { NO_NEIGHBOR } else { n - base };
            let marker = match by_triangle.get_mut(&sorted(fv)) {
                Some((m, matched)) => {
                    *matched = true;
                    if *m < 0 {
                        NO_MARKER
                    } else if *m as usize >= scene_faces {
                        stats.zone_tet_faces += 1;
                        NO_MARKER
                    } else {
                        stats.marked_tet_faces += 1;
                        *m
                    }
                }
                None => NO_MARKER,
            };
            if neighbor == NO_NEIGHBOR {
                stats.hull_tet_faces += 1;
            }
            TetraFace {
                vertices: fv,
                marker,
                neighbor,
            }
        });
        tetrahedra.push(Tetrahedron {
            vertices,
            id_volume,
            faces,
        });
    }
    let unmatched: Vec<&[i32; 3]> = by_triangle
        .iter()
        .filter(|(_, (_, matched))| !matched)
        .map(|(k, _)| k)
        .collect();
    if let Some(first) = unmatched.first() {
        return Err(format!(
            "{} .face triangles belong to no tetrahedron, for example {first:?} (0-based)",
            unmatched.len()
        ));
    }
    stats.attributes = attributes
        .into_iter()
        .map(|(attribute, (id_volume, tetrahedra))| AttributeMap {
            attribute,
            id_volume,
            tetrahedra,
        })
        .collect();
    Ok((mbin::Mesh { tetrahedra, nodes }, stats))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::check::predicates::orient3d;

    /// The unit tests' frame: the nodes of [`pair`] fit (0..1 on each axis).
    fn unit_frame() -> Unitize {
        Unitize::fit(&[[0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [0.5, 0.5, 0.5]]).unwrap()
    }

    /// Two tetrahedra sharing face (2,3,4), 1-based on disk.
    fn pair(face_rows: &[([i32; 3], i32)], attrs: [f64; 2]) -> TetgenOutput {
        let node =
            tetgen::read_node(b"5 3 0 0\n1 0 0 0\n2 1 0 0\n3 0 1 0\n4 0 0 1\n5 1 1 1\n").unwrap();
        let ele_text = format!("2 4 1\n1 1 2 3 4 {}\n2 5 2 4 3 {}\n", attrs[0], attrs[1]);
        let ele = tetgen::read_ele(ele_text.as_bytes()).unwrap();
        let mut face_text = format!("{} 1\n", face_rows.len());
        for (i, (f, m)) in face_rows.iter().enumerate() {
            face_text.push_str(&format!("{} {} {} {} {m}\n", i + 1, f[0], f[1], f[2]));
        }
        let face = tetgen::read_face(face_text.as_bytes()).unwrap();
        // Tet 1 (1,2,3,4): opposite its first corner, node 1, is (2,3,4), shared with tet 2.
        let neigh = tetgen::read_neigh(b"2 4\n1 2 -1 -1 -1\n2 1 -1 -1 -1\n").unwrap();
        TetgenOutput {
            node,
            ele,
            face,
            neigh,
        }
    }

    #[test]
    fn neighbours_markers_and_regions_follow_the_rules() {
        // (2,3,4) is an internal facet with marker 5; (1,2,3) a scene face; (5,2,4) a zone facet.
        let out = pair(
            &[([2, 3, 4], 5), ([1, 2, 3], 0), ([5, 2, 4], 9)],
            [3.0, 2.0],
        );
        let (mesh, stats) = build_mbin(&out, 8, &[2], &unit_frame()).unwrap();
        let (a, b) = (&mesh.tetrahedra[0], &mesh.tetrahedra[1]);
        // .ele row (1,2,3,4) is written (4,3,2,1), 0-based (3,2,1,0).
        assert_eq!(a.vertices, [3, 2, 1, 0]);
        // Face 3 is (b,c,a) = (2,1,3), opposite corner 3 = node 1 (0-based 0), so its neighbour
        // is the .neigh row's first column, tet 2 (0-based 1), across the internal facet.
        assert_eq!(a.faces[3].vertices, [2, 1, 3]);
        assert_eq!((a.faces[3].neighbor, a.faces[3].marker), (1, 5));
        // Face 0 is (b,d,c) = (2,0,1), opposite node 4: the .neigh's last column, the hull.
        assert_eq!(a.faces[0].vertices, [2, 0, 1]);
        assert_eq!((a.faces[0].neighbor, a.faces[0].marker), (NO_NEIGHBOR, 0));
        // Both sides of the internal facet carry its marker, and point at each other.
        assert_eq!(b.vertices, [2, 3, 1, 4]);
        assert_eq!((b.faces[3].neighbor, b.faces[3].marker), (0, 5));
        // Marker 9 >= 8 scene faces: a zone facet, written -1.
        assert!(b.faces.iter().all(|f| f.marker != 9));
        assert_eq!(stats.zone_tet_faces, 1);
        // Attribute 3 is not a fitting: room 0. Attribute 2 is.
        assert_eq!((a.id_volume, b.id_volume), (0, 2));
        assert_eq!(stats.marked_tet_faces, 3);
        // The nodes are the .node's, through the round trip: here every coordinate survives it,
        // and each 0 in y comes back as -0.0.
        assert_eq!(mesh.nodes[1], [1.0, -0.0, 0.0]);
        assert_eq!(mesh.nodes[1][1].to_bits(), (-0.0f32).to_bits());
        assert_eq!(mesh.nodes[4], [1.0, 1.0, 1.0]);
    }

    #[test]
    fn inconsistent_output_is_refused() {
        let u = unit_frame();
        // A .face row no tetrahedron has.
        let out = pair(&[([1, 2, 5], 0)], [1.0, 1.0]);
        let e = build_mbin(&out, 8, &[], &u).unwrap_err();
        assert!(e.contains("belong to no tetrahedron"), "{e}");
        // One triangle, two markers.
        let out = pair(&[([2, 3, 4], 1), ([4, 3, 2], 2)], [1.0, 1.0]);
        assert!(build_mbin(&out, 8, &[], &u).unwrap_err().contains("marker"));
        // A non-integral region attribute.
        let out = pair(&[], [1.5, 1.0]);
        assert!(
            build_mbin(&out, 8, &[], &u)
                .unwrap_err()
                .contains("not an integer")
        );
        // No -A attribute.
        let mut out = pair(&[], [1.0, 1.0]);
        out.ele = tetgen::read_ele(b"2 4 0\n1 1 2 3 4\n2 5 2 4 3\n").unwrap();
        assert!(build_mbin(&out, 8, &[], &u).unwrap_err().contains("-A"));
        // A .neigh that is not mutual.
        let mut out = pair(&[], [1.0, 1.0]);
        out.neigh = tetgen::read_neigh(b"2 4\n1 2 -1 -1 -1\n2 -1 -1 -1 -1\n").unwrap();
        assert!(build_mbin(&out, 8, &[], &u).is_err());
        // A node the round trip takes out of f32's range.
        let mut out = pair(&[], [1.0, 1.0]);
        out.node = tetgen::read_node(b"5 3 0 0\n1 0 0 0\n2 1 0 0\n3 0 1 0\n4 0 0 1\n5 3e38 1 1\n")
            .unwrap();
        let far = Unitize {
            centre: [-3e38, 0.0, 0.0],
            scale: 1.0,
        };
        let e = build_mbin(&out, 8, &[], &far).unwrap_err();
        assert!(e.contains("node 4 is not finite"), "{e}");
    }

    /// The tutorial-1 box as its project holds it (`tests/fixtures/rooms/tutorial1_box.simpa`),
    /// vertex order included.
    const BOX: [[f32; 3]; 8] = [
        [6.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
        [0.0, 10.0, 0.0],
        [6.0, 10.0, 0.0],
        [0.0, 10.0, 3.0],
        [6.0, 10.0, 3.0],
        [0.0, 0.0, 3.0],
        [6.0, 0.0, 3.0],
    ];

    #[test]
    fn unitize_fits_the_box_as_upstream_did() {
        // (3, 1.5, -5, 0.2f): the frame that reproduces all 732 nodes of the 2019 tutorial-1
        // .mbin bit for bit (target/investigate/report-investigate-pipeline.md, finding 6).
        let u = Unitize::fit(&BOX).unwrap();
        assert_eq!(u.centre, [3.0, 1.5, -5.0]);
        assert_eq!(u.scale.to_bits(), 0.2f32.to_bits());
    }

    #[test]
    fn unitize_leaves_the_last_vertex_out_as_upstream_does() {
        // A vertex far outside the box, last: upstream's loop never reaches it (Objet3D.cpp:542),
        // so the frame is the box's.
        let mut far_last = BOX.to_vec();
        far_last.push([100.0, 0.0, 0.0]);
        assert_eq!(
            Unitize::fit(&far_last).unwrap(),
            Unitize::fit(&BOX).unwrap()
        );
        // The same vertex anywhere else is counted: this is what the check above can refuse.
        let mut far_first = far_last.clone();
        far_first.rotate_right(1);
        let u = Unitize::fit(&far_first).unwrap();
        assert_eq!(u.centre[0], 50.0);
        assert_eq!(u.scale.to_bits(), 0.02f32.to_bits());
    }

    #[test]
    fn unitize_refuses_a_scene_with_no_extent() {
        assert!(Unitize::fit(&[]).is_err());
        // One vertex, or all but the last at one point: max(w, h, d) = 0.
        assert!(Unitize::fit(&[[1.0, 2.0, 3.0]]).is_err());
        assert!(Unitize::fit(&[[1.0, 2.0, 3.0], [1.0, 2.0, 3.0], [9.0, 9.0, 9.0]]).is_err());
        assert!(Unitize::fit(&[[f32::NAN, 0.0, 0.0], [1.0, 1.0, 1.0], [0.0; 3]]).is_err());
    }

    #[test]
    fn the_round_trip_is_upstreams() {
        let u = Unitize::fit(&BOX).unwrap();
        // Node 22 (1-based) of the 2019 .node, `5.3097315100373663 1.1504474832710558 3`: y
        // moves one ulp down, to the 0x3f9341dc the 2019 .mbin holds.
        let p = ["5.3097315100373663", "1.1504474832710558", "3"]
            .map(|s| s.parse::<f64>().unwrap() as f32);
        let q = u.round_trip(p);
        assert_eq!(q[0].to_bits(), p[0].to_bits());
        assert_eq!(q[1].to_bits(), 0x3f93_41dc);
        assert_ne!(q[1].to_bits(), p[1].to_bits());
        assert_eq!(q[2].to_bits(), p[2].to_bits());
        // A 0 in y, of either sign, comes back as -0.0; x and z keep their zeros' signs.
        for y in [0.0f32, -0.0] {
            let q = u.round_trip([0.0, y, 0.0]);
            assert_eq!(q.map(f32::to_bits), [0, (-0.0f32).to_bits(), 0]);
        }
    }

    #[test]
    fn reversing_the_corners_keeps_every_orientation_sign() {
        // The permutation's parity: (d,c,b,a) has 6 inversions, an even number.
        let inversions = (0..4)
            .flat_map(|i| (i + 1..4).map(move |j| (i, j)))
            .filter(|&(i, j)| UPSTREAM_CORNERS[i] > UPSTREAM_CORNERS[j])
            .count();
        assert_eq!(inversions, 6);
        // And on points, with the exact predicate: every ordering of four points in general
        // position, reversed, keeps its orientation; one swap flips it (the check can say no).
        let pts: [[f64; 3]; 4] = [
            [0.1, 0.2, 0.3],
            [1.7, -0.4, 0.9],
            [-0.6, 1.3, 0.2],
            [0.4, 0.5, 2.1],
        ];
        let mut orderings = 0;
        for a in 0..4 {
            for b in 0..4 {
                for c in 0..4 {
                    for d in 0..4 {
                        let o = [a, b, c, d];
                        if (0..4).any(|i| (i + 1..4).any(|j| o[i] == o[j])) {
                            continue;
                        }
                        orderings += 1;
                        let s =
                            |o: [usize; 4]| orient3d(pts[o[0]], pts[o[1]], pts[o[2]], pts[o[3]]);
                        assert_ne!(s(o), 0);
                        assert_eq!(s(upstream_order(o)), s(o), "{o:?}");
                        assert_eq!(s([o[1], o[0], o[2], o[3]]), -s(o), "{o:?}");
                    }
                }
            }
        }
        assert_eq!(orderings, 24);
    }
}
