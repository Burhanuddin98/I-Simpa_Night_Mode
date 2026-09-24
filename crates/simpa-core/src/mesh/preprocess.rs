//! Upstream's scene correction, `preprocess.exe` (`src/preprocess/`), run on the `.poly` before
//! TetGen when the project's mesh settings ask for it, as upstream's GUI runs it
//! (`projet_maillage.cpp:206-213`): `preprocess.exe scene_mesh.poly`, which rewrites the file in
//! place. What it does, from its source (`Preprocess.cpp:39-121`, `computations.cpp`):
//! deletes scene facets whose `float` area is at most 1e-4 m² (`DestroyNoAreaFaces`); merges
//! bit-equal vertices (`mergeVertices`); deletes user facets (the `.poly`'s Part 5) lying in a
//! scene facet's plane and inside it (`MeshDestroyCoplanarFaces`); appends the user facets to the
//! scene facets; splits facets where they cross until they meet at shared vertices
//! (`MeshReconstruction`); and saves, all facets in Part 2 and the regions unchanged.
//!
//! Three things it does not say, which this module finds out:
//! - **It never fails by its exit code.** `main` returns 0 whatever happened (`:112-121`). When a
//!   repair loop runs out of its 100 passes it prints `Mesh reparation has been aborted` and saves
//!   nothing; when it cannot read the file it prints `The mesh file cant be found !`. Both are
//!   `preprocess_aborted`, a recorded outcome, not a refusal: the mesher then meshes the `.poly`
//!   as written, as upstream's GUI does, and the geometry check on it is the gate. A saved file
//!   that still holds user facets (its coplanar step ran out of passes, and TetGen never reads
//!   Part 5) is `preprocess_output_invalid`.
//! - **What it changed.** It prints only three counts. [`account`] compares the file before and
//!   after, facet by facet: every facet it wrote must lie inside the facet it came from, and every
//!   input facet is kept, split into pieces of the same total area, or deleted, the deletions
//!   agreeing with its count.
//! - **Its markers for user facets are not the facets they lie in** (measured on tutorial 3).
//!   Its reader gives every Part 5 facet the first one's marker (`poly.cpp:418-423`: the loop
//!   compares `parsedFaces`, never reset after Part 2, with the Part 2 count), and split pieces
//!   inherit it (`computations.cpp:610`). [`account`] finds
//!   each such facet's true marker, the user facet it lies inside; the mesher writes it back
//!   ([`Markers::Restored`], the default) or keeps preprocess's bytes ([`Markers::Parity`]).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::manifest::TetgenCall;
use super::tetgen::{Mesher, run_logged};
use super::verify::point_triangle_distance;
use crate::formats::poly;
use crate::process::{CancelToken, Line, Outcome};

/// `preprocess.exe`, as `solvers/build.ps1` builds it from upstream's pinned sources.
pub const PREPROCESS_EXE_NAME: &str = "preprocess.exe";
/// Log file for `preprocess.exe`'s stdout, in the mesh folder.
pub const PREPROCESS_STDOUT_LOG: &str = "preprocess.stdout.txt";
/// Log file for `preprocess.exe`'s stderr, in the mesh folder.
pub const PREPROCESS_STDERR_LOG: &str = "preprocess.stderr.txt";
/// The `.poly` as the mesher wrote it, kept beside `scene_mesh.poly` once `preprocess.exe` has
/// rewritten that.
pub const INPUT_POLY: &str = "scene_mesh.input.poly";

/// The line `preprocess.exe` prints when a repair loop runs out of passes; it then saves nothing
/// (`Preprocess.cpp:100-105`).
pub const ABORTED_LINE: &str = "Mesh reparation has been aborted";
/// The line it prints when it cannot read the file (`Preprocess.cpp:65`).
pub const NOT_FOUND_LINE: &str = "The mesh file cant be found !";
/// The first line of its statistics, printed only when it saves (`computations.cpp:578`).
pub const STATUS_LINE: &str = "Remesh status :";

/// Upstream's `preprocess.exe`, run with the mesh folder as its working folder and the relative
/// argument `scene_mesh.poly` (it reads and writes the file with narrow `fopen`s, as TetGen does),
/// its streams logged to [`PREPROCESS_STDOUT_LOG`] and [`PREPROCESS_STDERR_LOG`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreprocessProgram {
    pub exe: PathBuf,
}

impl PreprocessProgram {
    pub fn new(exe: impl Into<PathBuf>) -> Self {
        PreprocessProgram { exe: exe.into() }
    }
}

impl Mesher for PreprocessProgram {
    fn program(&self) -> Option<&Path> {
        Some(&self.exe)
    }

    fn run(
        &self,
        dir: &Path,
        args: &[String],
        cancel: &CancelToken,
        on_line: &mut dyn FnMut(&Line),
    ) -> std::io::Result<Outcome> {
        run_logged(
            &self.exe,
            dir,
            args,
            cancel,
            on_line,
            (PREPROCESS_STDOUT_LOG, PREPROCESS_STDERR_LOG),
        )
    }
}

/// What becomes of the facet markers `preprocess.exe` writes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Markers {
    /// Each facet's marker is the facet it lies inside ([`account`]); the corrected `.poly` is
    /// what TetGen reads. The default.
    #[default]
    Restored,
    /// `preprocess.exe`'s bytes are kept, its user-facet markers with them, as upstream's GUI
    /// meshes them: the parity bed's mode. `mesh::verify` still reports what those markers do
    /// (marker mismatches, uncovered faces), so such a mesh never passes; its `.mbin` is written
    /// for byte comparison only.
    Parity,
}

/// What `preprocess.exe` printed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Printed {
    /// It printed [`STATUS_LINE`], which it does only when it saves.
    pub status: bool,
    /// It printed [`ABORTED_LINE`].
    pub aborted: bool,
    /// It printed [`NOT_FOUND_LINE`].
    pub not_found: bool,
    /// `Vertices merged : N`.
    pub vertices_merged: Option<u64>,
    /// `Coplanar Faces destroyed : N`: the tiny scene facets and the coplanar user facets it
    /// deleted, one counter for both (`computations.cpp:215-290`).
    pub faces_destroyed: Option<u64>,
    /// `Face splitted : N`.
    pub faces_split: Option<u64>,
    /// Lines `Split Triangle for the N times. [x;y;z]`.
    pub split_lines: usize,
}

/// Reads `preprocess.exe`'s stdout lines.
pub fn parse_stdout(lines: &[String]) -> Printed {
    let count = |line: &str, label: &str| -> Option<u64> {
        let rest = line.trim().strip_prefix(label)?;
        rest.trim().strip_prefix(':')?.trim().parse().ok()
    };
    let mut p = Printed::default();
    for l in lines {
        let t = l.trim();
        p.status |= t.starts_with(STATUS_LINE);
        p.aborted |= t.starts_with(ABORTED_LINE);
        p.not_found |= t.starts_with(NOT_FOUND_LINE);
        if t.starts_with("Split Triangle for the ") {
            p.split_lines += 1;
        }
        if let Some(n) = count(t, "Vertices merged") {
            p.vertices_merged = Some(n);
        }
        if let Some(n) = count(t, "Coplanar Faces destroyed") {
            p.faces_destroyed = Some(n);
        }
        if let Some(n) = count(t, "Face splitted") {
            p.faces_split = Some(n);
        }
    }
    p
}

/// Sizes of a `.poly`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolyCounts {
    pub vertices: usize,
    pub facets: usize,
    pub user_facets: usize,
    pub regions: usize,
}

impl PolyCounts {
    pub fn of(m: &poly::Model) -> Self {
        PolyCounts {
            vertices: m.model_vertices.len(),
            facets: m.model_faces.len(),
            user_facets: m.user_defined_faces.len(),
            regions: m.model_regions.len(),
        }
    }
}

/// An input facet `preprocess.exe` split, by its marker.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SplitFacet {
    pub marker: u32,
    /// Output facets inside it.
    pub pieces: usize,
    pub area_m2: f64,
    pub pieces_area_m2: f64,
}

/// An output facet whose marker `preprocess.exe` wrote is not the facet it lies inside.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkerChange {
    /// Its position in the output facet list (0-based).
    pub facet: u32,
    /// The marker `preprocess.exe` wrote.
    pub written: u32,
    /// The marker of the input facet it lies inside.
    pub restored: u32,
}

/// What `preprocess.exe` changed, facet by facet: [`account`]'s result.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Accounting {
    /// Per output facet, the marker of the input facet it lies inside.
    pub true_markers: Vec<u32>,
    /// Markers of the input facets no output facet lies inside, ascending: deleted.
    pub deleted: Vec<u32>,
    /// Input facets now more than one output facet.
    pub split: Vec<SplitFacet>,
    /// Output facets whose written marker is not their true one.
    pub marker_changes: Vec<MarkerChange>,
    /// Output vertices at no input vertex's position, in output order.
    pub new_vertices: Vec<[f64; 3]>,
    /// The largest distance from an output facet's vertex to the input facet it lies inside.
    pub max_distance_m: f64,
    /// The largest `|pieces' area - facet's area|` over the kept input facets, m², and the same
    /// as a fraction of the bound it is held to (perimeter times the tolerance).
    pub max_area_error_m2: f64,
    pub max_area_error_of_bound: f64,
}

/// The containment tolerance for a scene whose largest |coordinate| is `r`: the verifier's own,
/// `16 · 2⁻²⁴ · r` (`mesh::verify`'s marker tolerance).
pub fn tolerance(r: f64) -> f64 {
    16.0 * (f32::EPSILON as f64 / 2.0) * r
}

fn tri(m: &poly::Model, f: &poly::Face) -> Option<[[f64; 3]; 3]> {
    let v = |i: u32| m.model_vertices.get(i as usize).copied();
    Some([v(f.vertices[0])?, v(f.vertices[1])?, v(f.vertices[2])?])
}

fn perimeter(t: &[[f64; 3]; 3]) -> f64 {
    let d = |a: [f64; 3], b: [f64; 3]| {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    };
    d(t[0], t[1]) + d(t[1], t[2]) + d(t[2], t[0])
}

fn area(t: &[[f64; 3]; 3]) -> f64 {
    let u = [0, 1, 2].map(|k| t[1][k] - t[0][k]);
    let v = [0, 1, 2].map(|k| t[2][k] - t[0][k]);
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
}

/// The largest distance from a vertex of `t` to the triangle `parent`.
fn distance_into(t: &[[f64; 3]; 3], parent: &[[f64; 3]; 3]) -> f64 {
    t.iter()
        .map(|&p| point_triangle_distance(p, *parent))
        .fold(
            0.0,
            |a: f64, d| if d.is_nan() { f64::INFINITY } else { a.max(d) },
        )
}

/// Compares the `.poly` `preprocess.exe` was given (`input`, markers as the mesher wrote them)
/// with the one it saved (`output`), within `tolerance`:
/// - the output holds no user facet, and the input's regions unchanged;
/// - each output facet lies inside the facet it came from: a facet marked as an input scene
///   facet (Part 2) inside that facet; a facet carrying a user facet's marker, which may be the
///   first user facet's (as its reader assigns them), inside exactly one input user facet, whose
///   marker is its true one;
/// - each input facet is deleted (nothing lies inside it), or kept, alone or split, its pieces'
///   total area its own within its perimeter times `tolerance`: the pieces lie within
///   `tolerance` of it, so they can leave uncovered, or cover twice, at most a strip that wide
///   along its edges (`preprocess.exe` splits at points it takes as on an edge within an angle of
///   1e-4, `SplitTriangleByThree`, `computations.cpp:599-601`, so its pieces tile a facet only to
///   rounding: tutorial 3's floor facet 55 is 18.574098657 m², its 9 pieces 18.574098218 m²);
/// - the deletions are as many as `preprocess.exe` counted (`faces_destroyed`).
///
/// `Err` lists every way the output fails that, at most 20 of each.
pub fn account(
    input: &poly::Model,
    output: &poly::Model,
    faces_destroyed: Option<u64>,
    tolerance: f64,
) -> Result<Accounting, Vec<String>> {
    let mut errors: Vec<String> = Vec::new();
    let more = |errors: &mut Vec<String>, e: String| {
        if errors.len() < 60 {
            errors.push(e);
        }
    };
    if !output.user_defined_faces.is_empty() {
        more(
            &mut errors,
            format!(
                "the output still holds {} user facets (Part 5), which TetGen never reads",
                output.user_defined_faces.len()
            ),
        );
    }
    if output.model_regions != input.model_regions {
        more(
            &mut errors,
            "the output's regions are not the input's".to_string(),
        );
    }
    // Input facets: (marker, triangle, user?).
    let mut inputs: Vec<(u32, [[f64; 3]; 3], bool)> = Vec::new();
    for (f, user) in input
        .model_faces
        .iter()
        .map(|f| (f, false))
        .chain(input.user_defined_faces.iter().map(|f| (f, true)))
    {
        match tri(input, f) {
            Some(t) => inputs.push((f.face_index, t, user)),
            None => more(
                &mut errors,
                format!(
                    "input facet {} names a node that does not exist",
                    f.face_index
                ),
            ),
        }
    }
    // The first scene facet with each marker (a project's are its face indices, one each).
    let mut by_marker: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for (i, (k, _, user)) in inputs.iter().enumerate() {
        if !user {
            by_marker.entry(*k).or_insert(i);
        }
    }
    let scene_facet = |m: u32| by_marker.get(&m).copied();
    let user_markers: Vec<u32> = inputs
        .iter()
        .filter(|(_, _, user)| *user)
        .map(|(k, _, _)| *k)
        .collect();

    let mut acc = Accounting::default();
    let mut pieces: Vec<(usize, f64)> = vec![(0, 0.0); inputs.len()];
    for (j, f) in output.model_faces.iter().enumerate() {
        let Some(t) = tri(output, f) else {
            more(
                &mut errors,
                format!("output facet {j} names a node that does not exist"),
            );
            acc.true_markers.push(f.face_index);
            continue;
        };
        let written = f.face_index;
        let parent = if let Some(i) = scene_facet(written) {
            let d = distance_into(&t, &inputs[i].1);
            if d <= tolerance {
                acc.max_distance_m = acc.max_distance_m.max(d);
                Some(i)
            } else {
                more(
                    &mut errors,
                    format!(
                        "output facet {j} carries scene facet {written}'s marker and lies {d:.3e} \
                         m outside it (tolerance {tolerance:.3e} m)"
                    ),
                );
                None
            }
        } else if user_markers.contains(&written) {
            let inside: Vec<(usize, f64)> = inputs
                .iter()
                .enumerate()
                .filter(|(_, (_, _, user))| *user)
                .map(|(i, (_, p, _))| (i, distance_into(&t, p)))
                .filter(|(_, d)| *d <= tolerance)
                .collect();
            match inside.as_slice() {
                [(i, d)] => {
                    acc.max_distance_m = acc.max_distance_m.max(*d);
                    Some(*i)
                }
                [] => {
                    more(
                        &mut errors,
                        format!(
                            "output facet {j} carries user facet marker {written} and lies inside \
                             no input user facet"
                        ),
                    );
                    None
                }
                many => {
                    let ks: Vec<u32> = many.iter().map(|(i, _)| inputs[*i].0).collect();
                    more(
                        &mut errors,
                        format!(
                            "output facet {j} carries user facet marker {written} and lies inside \
                             the user facets {ks:?}: its true marker is ambiguous"
                        ),
                    );
                    None
                }
            }
        } else {
            more(
                &mut errors,
                format!("output facet {j} carries marker {written}, which no input facet has"),
            );
            None
        };
        let true_marker = parent.map_or(written, |i| inputs[i].0);
        if let Some(i) = parent {
            pieces[i].0 += 1;
            pieces[i].1 += area(&t);
        }
        if true_marker != written {
            acc.marker_changes.push(MarkerChange {
                facet: j as u32,
                written,
                restored: true_marker,
            });
        }
        acc.true_markers.push(true_marker);
    }
    for (i, (marker, t, _)) in inputs.iter().enumerate() {
        let (n, a) = pieces[i];
        let own = area(t);
        if n == 0 {
            acc.deleted.push(*marker);
            continue;
        }
        let err = (a - own).abs();
        let bound = perimeter(t) * tolerance;
        acc.max_area_error_m2 = acc.max_area_error_m2.max(err);
        acc.max_area_error_of_bound = acc.max_area_error_of_bound.max(if bound > 0.0 {
            err / bound
        } else {
            f64::INFINITY
        });
        if err.is_nan() || err > bound {
            more(
                &mut errors,
                format!(
                    "input facet {marker}'s {n} output pieces cover {a:.9} m², the facet {own:.9} \
                     m² (at most {bound:.3e} m² apart)"
                ),
            );
        }
        if n > 1 {
            acc.split.push(SplitFacet {
                marker: *marker,
                pieces: n,
                area_m2: own,
                pieces_area_m2: a,
            });
        }
    }
    acc.deleted.sort_unstable();
    match faces_destroyed {
        Some(n) if n as usize == acc.deleted.len() => {}
        Some(n) => more(
            &mut errors,
            format!(
                "{} input facets have no output facet inside them ({:?}), and preprocess.exe \
                 counted {n} destroyed",
                acc.deleted.len(),
                acc.deleted
            ),
        ),
        None => more(
            &mut errors,
            "preprocess.exe printed no count of destroyed facets".to_string(),
        ),
    }
    let at = |p: &[f64; 3]| p.map(f64::to_bits);
    let known: std::collections::HashSet<[u64; 3]> = input.model_vertices.iter().map(at).collect();
    acc.new_vertices = output
        .model_vertices
        .iter()
        .filter(|p| !known.contains(&at(p)))
        .copied()
        .collect();
    if errors.is_empty() {
        Ok(acc)
    } else {
        Err(errors)
    }
}

/// The record of one `preprocess.exe` run, in the mesh manifest's `preprocess`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PreprocessReport {
    /// The call, as the manifest records TetGen's (`argv` `["scene_mesh.poly"]`, `cwd` `.`).
    pub call: TetgenCall,
    pub markers: Markers,
    pub printed: Printed,
    /// sha256 of the `.poly` it was given ([`INPUT_POLY`]) and of the one it saved (`None` when it
    /// saved none).
    pub input_sha256: String,
    pub output_sha256: Option<String>,
    pub input: PolyCounts,
    pub output: Option<PolyCounts>,
    /// The facet-by-facet comparison, when the output could be read and accounted for.
    pub accounting: Option<Accounting>,
    /// The deleted facets, mapped to the scene as TetGen's skipped facets are.
    pub deleted_facets: Vec<super::manifest::SkippedFacet>,
    /// The containment tolerance used, m.
    pub tolerance_m: f64,
    /// Whether the markers in `scene_mesh.poly` are the restored ones (`restored`) or
    /// `preprocess.exe`'s (`parity`, or when nothing needed restoring).
    pub markers_rewritten: bool,
    /// One line: what it changed.
    pub summary: String,
    /// What TetGen was given: `corrected`, what `preprocess.exe` saved, accounted for; or
    /// `aborted`, when it saved nothing (it gave up, could not read the file, or printed no
    /// statistics) and the `.poly` as written, uncorrected, was meshed, as upstream's GUI meshes
    /// it then (`projet_maillage.cpp:206-213`: the GUI does not look at the result). `None` when
    /// the run failed (its codes say why). Read as `None` when absent.
    #[serde(default)]
    pub outcome: Option<PreprocessOutcome>,
    /// Why it was `aborted`, in words, with its last line; `None` otherwise. Read as `None` when
    /// absent.
    #[serde(default)]
    pub aborted_reason: Option<String>,
}

/// [`PreprocessReport::outcome`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreprocessOutcome {
    /// TetGen read what `preprocess.exe` saved, accounted for facet by facet.
    Corrected,
    /// `preprocess.exe` saved nothing; TetGen read the `.poly` as the mesher wrote it, which the
    /// geometry check gates. The mesh folder's `scene_mesh.poly` is that file, byte for byte
    /// `scene_mesh.input.poly`.
    Aborted,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(str::to_string).collect()
    }

    #[test]
    fn stdout_is_read_as_preprocess_prints_it() {
        let saved = parse_stdout(&lines(
            "Split Triangle for the 1 times. [18;3.5;0]\nSplit Triangle for the 2 times. [18;4;0]\n\
             Coplanar correction step and vertices merging step has finished.\nRemesh status : \n\
             Vertices merged : 28\nCoplanar Faces destroyed : 2\nFace splitted : 31\n",
        ));
        assert_eq!(
            saved,
            Printed {
                status: true,
                aborted: false,
                not_found: false,
                vertices_merged: Some(28),
                faces_destroyed: Some(2),
                faces_split: Some(31),
                split_lines: 2,
            }
        );
        // Says no: the abort line, and no statistics.
        let aborted = parse_stdout(&lines(
            "Split Triangle for the 104 times. [41.1013;-0.651135;8.55302]\nMesh reparation has \
             been aborted. The algorithm enter into an infinite loop. Try to stick coplanar faces \
             or destroy manually.\n",
        ));
        assert!(aborted.aborted && !aborted.status);
        assert_eq!(aborted.faces_destroyed, None);
        assert!(parse_stdout(&lines("The mesh file cant be found !")).not_found);
    }

    /// A unit square in z = 0 as two scene facets 0 and 1, and a user triangle 2 standing on it.
    fn square_and_fin() -> poly::Model {
        poly::Model {
            save_face_index: true,
            model_vertices: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.2, 0.5, 0.0],
                [0.8, 0.5, 0.0],
                [0.5, 0.5, 1.0],
            ],
            model_faces: vec![
                poly::Face {
                    vertices: [0, 1, 2],
                    face_index: 0,
                },
                poly::Face {
                    vertices: [0, 2, 3],
                    face_index: 1,
                },
            ],
            user_defined_faces: vec![poly::Face {
                vertices: [4, 5, 6],
                face_index: 2,
            }],
            model_regions: Vec::new(),
        }
    }

    #[test]
    fn a_split_facet_and_a_user_facet_are_accounted_for() {
        let input = square_and_fin();
        // Facet 0 split in two at its edge's midpoint (1, 0.5, 0); the user facet appended with
        // the reader's marker for it, 2 (the first user facet's: here its own).
        let mut out = input.clone();
        out.user_defined_faces.clear();
        out.model_vertices.push([1.0, 0.5, 0.0]);
        out.model_faces = vec![
            poly::Face {
                vertices: [0, 1, 7],
                face_index: 0,
            },
            poly::Face {
                vertices: [0, 7, 2],
                face_index: 0,
            },
            poly::Face {
                vertices: [0, 2, 3],
                face_index: 1,
            },
            poly::Face {
                vertices: [4, 5, 6],
                face_index: 2,
            },
        ];
        let acc = account(&input, &out, Some(0), 1e-9).unwrap();
        assert_eq!(acc.true_markers, [0, 0, 1, 2]);
        assert_eq!(acc.deleted, Vec::<u32>::new());
        assert_eq!(acc.split.len(), 1);
        assert_eq!((acc.split[0].marker, acc.split[0].pieces), (0, 2));
        assert_eq!(acc.new_vertices, [[1.0, 0.5, 0.0]]);
        assert!(acc.marker_changes.is_empty());

        // Says no: a piece missing (the split facet's pieces no longer cover it).
        let mut lost = out.clone();
        lost.model_faces.remove(1);
        let e = account(&input, &lost, Some(0), 1e-9).unwrap_err();
        assert!(
            e.iter()
                .any(|m| m.contains("input facet 0's 1 output pieces")),
            "{e:?}"
        );

        // Says no: a facet deleted that preprocess did not count.
        let mut deleted = out.clone();
        deleted.model_faces.remove(2);
        let e = account(&input, &deleted, Some(0), 1e-9).unwrap_err();
        assert!(e.iter().any(|m| m.contains("counted 0 destroyed")), "{e:?}");
        // Counted, it is accepted, and named.
        let acc = account(&input, &deleted, Some(1), 1e-9).unwrap();
        assert_eq!(acc.deleted, [1]);

        // Says no: a piece carrying a scene facet's marker outside that facet.
        let mut moved = out.clone();
        moved.model_faces[2].face_index = 0;
        let e = account(&input, &moved, Some(0), 1e-9).unwrap_err();
        assert!(e.iter().any(|m| m.contains("lies")), "{e:?}");

        // Says no: user facets left in Part 5.
        let mut kept = out.clone();
        kept.user_defined_faces = input.user_defined_faces.clone();
        let e = account(&input, &kept, Some(0), 1e-9).unwrap_err();
        assert!(e.iter().any(|m| m.contains("Part 5")), "{e:?}");
    }

    #[test]
    fn the_readers_user_facet_marker_is_restored() {
        // Two user facets 2 and 3; preprocess's reader gives both the first one's marker, 2.
        let mut input = square_and_fin();
        input
            .model_vertices
            .extend([[0.2, 0.2, 0.0], [0.8, 0.2, 0.0], [0.5, 0.2, 1.0]]);
        input.user_defined_faces.push(poly::Face {
            vertices: [7, 8, 9],
            face_index: 3,
        });
        let mut out = input.clone();
        out.user_defined_faces.clear();
        out.model_faces.extend([
            poly::Face {
                vertices: [4, 5, 6],
                face_index: 2,
            },
            poly::Face {
                vertices: [7, 8, 9],
                face_index: 2,
            },
        ]);
        let acc = account(&input, &out, Some(0), 1e-9).unwrap();
        assert_eq!(acc.true_markers, [0, 1, 2, 3]);
        assert_eq!(
            acc.marker_changes,
            [MarkerChange {
                facet: 3,
                written: 2,
                restored: 3
            }]
        );
        // Says no: two user facets in one place, so the true marker is ambiguous.
        let mut twin = input.clone();
        twin.user_defined_faces[1].vertices = [4, 5, 6];
        let e = account(&twin, &out, Some(0), 1e-9).unwrap_err();
        assert!(e.iter().any(|m| m.contains("ambiguous")), "{e:?}");
    }
}
