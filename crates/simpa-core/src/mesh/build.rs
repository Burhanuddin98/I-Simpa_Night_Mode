//! The `.mbin` built from one TetGen output set (`.node`, `.ele`, `.face`, `.neigh`), as
//! upstream's `CObjet3D::LoadMaillage` builds it (`isimpa/3dengine/Core/Objet3D_maillage.cpp:54-452`)
//! with the departures of `docs/m5-m6-design.md` decisions 1, 5 and 6.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::formats::mbin::{self, NO_MARKER, NO_NEIGHBOR, TetraFace, Tetrahedron};
use crate::formats::tetgen::{self, EleFile, FaceFile, HULL, NeighFile, NodeFile};

/// Face `i` of a tetrahedron `(a, b, c, d)` is the face opposite corner `i`, wound as upstream
/// winds it: `(b,d,c)`, `(c,d,a)`, `(a,d,b)`, `(b,c,a)` (`Objet3D_maillage.cpp:182-185`).
pub const FACE_CORNERS: [[usize; 3]; 4] = [[1, 3, 2], [2, 3, 0], [0, 3, 1], [1, 2, 0]];

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

/// Builds the `.mbin` of `out`:
/// - nodes narrowed to `f32`;
/// - tetrahedra in `.ele` order, corners as written, minus the index base;
/// - faces in [`FACE_CORNERS`] order, face `i`'s neighbour = `.neigh` column `i` minus the base,
///   TetGen's -1 written as -2 (`Objet3D_maillage.cpp:435-442`). There is no fallback: the
///   neighbours come from the `.neigh` or not at all;
/// - markers from the `.face` rows, matched by unordered vertex triple to **every** tetrahedron
///   face carrying that triangle, so both sides of an internal facet get it
///   (`Objet3D_maillage.cpp:369-381`); a marker at or above `scene_faces` (a box fitting zone's
///   triangle) or below 0 is written as -1;
/// - `idVolume` = the last `.ele` attribute (`-A`'s region) when it is one of `fittings`, and 0
///   (the room) otherwise (decision 1).
///
/// Refuses, with the reason, files that disagree with each other ([`tetgen::check_mesh`]), a mesh
/// without the `-A` attribute or with a non-integral one, second-order tetrahedra, a `.face`
/// without markers, one triangle listed twice with different markers, and a `.face` row that no
/// tetrahedron has.
pub fn build_mbin(
    out: &TetgenOutput,
    scene_faces: usize,
    fittings: &[i32],
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
            let n = p.map(|c| c as f32);
            if n.iter().all(|c| c.is_finite()) {
                Ok(n)
            } else {
                Err(format!("node {i} is not finite as a 32-bit float"))
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
        let vertices = [c[0] - base, c[1] - base, c[2] - base, c[3] - base];
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
            let n = neigh.neighbors[t][i];
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

    /// Two tetrahedra sharing face (1,2,3): corners 0..4, 1-based on disk.
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
        // Tet 1 (1,2,3,4): opposite corner 0 is (2,3,4), shared with tet 2.
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
        let (mesh, stats) = build_mbin(&out, 8, &[2]).unwrap();
        let (a, b) = (&mesh.tetrahedra[0], &mesh.tetrahedra[1]);
        assert_eq!(a.vertices, [0, 1, 2, 3]);
        // Face 0 of tet 0 is (b,d,c) = (1,3,2), opposite corner 0, neighbour tet 1.
        assert_eq!(a.faces[0].vertices, [1, 3, 2]);
        assert_eq!((a.faces[0].neighbor, a.faces[0].marker), (1, 5));
        assert_eq!(a.faces[1].neighbor, NO_NEIGHBOR);
        // Both sides of the internal facet carry its marker.
        let shared = b.faces.iter().find(|f| f.neighbor == 0).unwrap();
        assert_eq!(shared.marker, 5);
        // (1,2,3) 1-based is face 3 of tet 0, (b,c,a) = (1,2,0).
        assert_eq!(a.faces[3].vertices, [1, 2, 0]);
        assert_eq!(a.faces[3].marker, 0);
        // Marker 9 >= 8 scene faces: a zone facet, written -1.
        assert!(b.faces.iter().all(|f| f.marker != 9));
        assert_eq!(stats.zone_tet_faces, 1);
        // Attribute 3 is not a fitting: room 0. Attribute 2 is.
        assert_eq!((a.id_volume, b.id_volume), (0, 2));
        assert_eq!(stats.marked_tet_faces, 3);
    }

    #[test]
    fn inconsistent_output_is_refused() {
        // A .face row no tetrahedron has.
        let out = pair(&[([1, 2, 5], 0)], [1.0, 1.0]);
        let e = build_mbin(&out, 8, &[]).unwrap_err();
        assert!(e.contains("belong to no tetrahedron"), "{e}");
        // One triangle, two markers.
        let out = pair(&[([2, 3, 4], 1), ([4, 3, 2], 2)], [1.0, 1.0]);
        assert!(build_mbin(&out, 8, &[]).unwrap_err().contains("marker"));
        // A non-integral region attribute.
        let out = pair(&[], [1.5, 1.0]);
        assert!(
            build_mbin(&out, 8, &[])
                .unwrap_err()
                .contains("not an integer")
        );
        // No -A attribute.
        let mut out = pair(&[], [1.0, 1.0]);
        out.ele = tetgen::read_ele(b"2 4 0\n1 1 2 3 4\n2 5 2 4 3\n").unwrap();
        assert!(build_mbin(&out, 8, &[]).unwrap_err().contains("-A"));
        // A .neigh that is not mutual.
        let mut out = pair(&[], [1.0, 1.0]);
        out.neigh = tetgen::read_neigh(b"2 4\n1 2 -1 -1 -1\n2 -1 -1 -1 -1\n").unwrap();
        assert!(build_mbin(&out, 8, &[]).is_err());
    }
}
