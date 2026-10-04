//! The room of a run, read from the run's own inputs (M12 P2, the Acoustics tab:
//! `docs/investigations/2026-10-03-m12/PLAN.md`). Gate M12(a) holds every number that tab shows
//! to `simpa results --json`, so what it shows beside the solver's values is computed here, in
//! the report, never in the UI:
//! - the room's volume, its air's ([`air_tetrahedra`]: the `.mbin`'s tetrahedra outside every
//!   closed obstacle, results version 14), the obstacles' inside beside it, and its area (the
//!   `.cbin`'s faces);
//! - DIN 18041's five group-A targets at that volume ([`din18041::target_s`]), each refused
//!   where the volume is outside the group's range;
//! - the absorption by surface group: config.xml declares one material per surface group, so the
//!   `.cbin` faces grouped by their material id are the surface groups as the solver saw them,
//!   each with its faces, its area, and per computed band its absorption `α` and `S·α`; and each
//!   band's total `Σ S·α`, the Sabine absorption area without the air term.
//!
//! The room is read as TCR's analytic references read it ([`super::tcr::RoomInputs`]), for an
//! SPPS run as for a TCR one; a scene with fitting faces, or one that does not read, is
//! [`Room::NotComputed`] with why. Nothing of the solver's output is read.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::tcr::RoomInputs;
use crate::formats::mbin;
use crate::params::ParamError;
use crate::params::din18041::{self, Group};
use crate::run::expect::Expectation;

/// Said beside the DIN targets wherever they are shown: what a target applies to
/// ([`din18041`]'s module docs).
pub const DIN18041_NOTE: &str = "DIN 18041:2016-03 group-A target T_soll = a lg(V) + b (formulas \
     as quoted by BNB V 2020, criterion 3.1.4; the standard itself was not read): for the room \
     80 % occupied, at mid frequencies; an empty room's model meets it only with the audience's \
     absorption in the model";

/// One surface group as the solver saw it: the faces of one material id.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupAbsorption {
    /// `type_surface@id`, the `.cbin` faces' `idMat`.
    pub material_id: u32,
    /// How many `.cbin` faces carry it.
    pub faces: usize,
    /// Their area, m².
    pub area_m2: f64,
    /// Per computed band, ascending: `(freq_hz, α)`, the absorption config.xml declares.
    pub absorption: Vec<(i32, f64)>,
}

/// The room of a run, or why there is none.
#[derive(Clone, Debug, PartialEq)]
pub enum Room {
    Computed {
        /// The air's volume, m³: the `.mbin`'s tetrahedra that [`air_tetrahedra`] keeps.
        volume_m3: f64,
        /// The rest of the `.mbin`'s tetrahedra, m³: the inside of the closed obstacles; 0 in a
        /// room without one.
        obstacle_volume_m3: f64,
        /// The `.cbin` faces' total area, m².
        area_m2: f64,
        /// Every group, A1 to A5, with its target at `volume_m3`, s, or the refusal.
        din18041: Vec<(Group, Result<f64, ParamError>)>,
        /// By material id, ascending; only ids a face carries.
        groups: Vec<GroupAbsorption>,
    },
    NotComputed {
        why: String,
    },
}

/// Which of `mesh`'s tetrahedra hold the room's air (backlog 85), in its order: those of every
/// region (`idVolume`) that reaches the mesh's outer surface. A closed shell nested in the room, a
/// radiator or a stage panel, is meshed as a region of its own that touches only the room around
/// it, so its inside is left out; the rooms themselves, one region or several split by partition
/// walls, each reach the outer surface. These are `geometry::check`'s depth-1 cells (its
/// `air_volume_m3`), found here from the mesh alone: a face of the outer surface is a face only
/// one tetrahedron has (the same three nodes), so neither TetGen's region numbering nor the
/// `.mbin`'s neighbour fields are trusted. A mesh of one region, as upstream's meshes without
/// region attributes are, is all air: the volume every tetrahedron gives, as before version 14.
///
/// Its limit: a solid that touches the outer surface, a box pushed against a wall, reaches it too,
/// and is counted as air, as the check counts it a room. A fitting zone nested in the room is air
/// with scattering objects in it but would be left out; [`super::tcr::RoomInputs::read`] refuses a
/// scene with fitting faces before this is asked.
pub fn air_tetrahedra(mesh: &mbin::Mesh) -> Vec<bool> {
    let mut uses: HashMap<[i32; 3], u32> = HashMap::with_capacity(2 * mesh.tetrahedra.len());
    let faces = |t: &mbin::Tetrahedron| {
        let v = t.vertices;
        [
            [v[1], v[2], v[3]],
            [v[0], v[2], v[3]],
            [v[0], v[1], v[3]],
            [v[0], v[1], v[2]],
        ]
        .map(|mut f| {
            f.sort_unstable();
            f
        })
    };
    for t in &mesh.tetrahedra {
        for f in faces(t) {
            *uses.entry(f).or_insert(0) += 1;
        }
    }
    let outer: HashSet<i32> = mesh
        .tetrahedra
        .iter()
        .filter(|t| faces(t).iter().any(|f| uses[f] == 1))
        .map(|t| t.id_volume)
        .collect();
    mesh.tetrahedra
        .iter()
        .map(|t| outer.contains(&t.id_volume))
        .collect()
}

/// The room of the run whose solver folder is `solve` ([module docs](self)).
pub fn room(solve: &Path, exp: &Expectation) -> Room {
    inner(solve, exp).unwrap_or_else(|why| Room::NotComputed { why })
}

fn inner(solve: &Path, exp: &Expectation) -> Result<Room, String> {
    let room = RoomInputs::read(solve, exp)?;
    let area_m2: f64 = room.faces.iter().map(|f| f.0).sum();
    if !(area_m2.is_finite() && area_m2 > 0.0) {
        return Err(format!("the scene's faces have an area of {area_m2} m²"));
    }
    let mut ids: Vec<u32> = room.faces.iter().map(|f| f.1).collect();
    ids.sort_unstable();
    ids.dedup();
    let mut groups = Vec::with_capacity(ids.len());
    for id in ids {
        let alphas = room
            .materials
            .iter()
            .find(|(m, _)| *m == id)
            .map(|(_, a)| a)
            .ok_or_else(|| format!("a face's material {id} is not declared"))?;
        let absorption = room
            .bands
            .iter()
            .map(|b| {
                alphas
                    .get(b.index)
                    .map(|&a| (b.freq_hz, a))
                    .ok_or_else(|| format!("material {id} has no band {}", b.index))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let of_id = room.faces.iter().filter(|f| f.1 == id);
        groups.push(GroupAbsorption {
            material_id: id,
            faces: of_id.clone().count(),
            area_m2: of_id.map(|f| f.0).sum(),
            absorption,
        });
    }
    Ok(Room::Computed {
        volume_m3: room.volume_m3,
        obstacle_volume_m3: room.obstacle_volume_m3,
        area_m2,
        din18041: Group::ALL
            .iter()
            .map(|&g| (g, din18041::target_s(g, room.volume_m3)))
            .collect(),
        groups,
    })
}
