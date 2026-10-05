# The example projects: sources and licences

These three `.simpa` files ship inside the app (`src/examples.rs`, `include_bytes!`) and are
offered on the landing page while no project is open. Opening one writes a copy into the user's
`Documents\Night Mode\Examples` folder. The files here are never modified at run time.

| file | room | source | licence |
|---|---|---|---|
| `elmia_hall.simpa` | Elmia hall | Upstream I-Simpa's tutorial 2 (`tutorial_2.proj`) | GPL-3.0 |
| `bras_cr2.simpa` | BRAS CR2, seminar room (BRAS scene 9) | The BRAS database | CC BY-SA 4.0 |
| `bras_cr4.simpa` | BRAS CR4, auditorium (BRAS scene 11) | The BRAS database | CC BY-SA 4.0 |

## Elmia hall

A byte-for-byte copy of `tests/fixtures/rooms/elmia_corrected.simpa`
(sha256 `70439d85fc202377207d2a9d0539a439713f4f74b5f81a26115f20aa9690a9d4`). It holds the
settings of decision-log row 49. The geometry, groups, materials, sources and receivers come from
upstream I-Simpa's second tutorial project, `src/isimpa/resources/doc/tutorial/tutorial 2/tutorial_2.proj`
(Université Gustave Eiffel, <https://github.com/Universite-Gustave-Eiffel/I-Simpa>), imported by
`simpa_core::geometry::import::import_proj`. `tests/fixtures/rooms/PROVENANCE.md` documents the
import and every edit. I-Simpa is GPL-3.0, as is this app.

## BRAS CR2 and BRAS CR4

Both rooms are derived from the Benchmark for Room Acoustical Simulation (BRAS):

> L. Aspöck, M. Vorländer, F. Brinkmann, D. Ackermann and S. Weinzierl, *Benchmark for Room
> Acoustical Simulation (BRAS)*, RWTH Aachen University, Institute of Technical Acoustics, and
> TU Berlin, Audio Communication Group. Documentation of the database, 15 May 2019.

The BRAS database is licensed under the Creative Commons Attribution-ShareAlike 4.0 International
licence (CC BY-SA 4.0, <https://creativecommons.org/licenses/by-sa/4.0/>).

**What was changed.** Each room's geometry is BRAS's SketchUp model (scene 9 for CR2, scene 11 for
CR4), exported to OBJ and then repaired so that SPPS can mesh it. The repairs remove SketchUp's
triangulation artefacts (T-junctions, zero-area slivers, duplicate faces) and correct face
orientation. CR2 keeps every vertex where BRAS put it. In CR4, six near-coincident vertex pairs
were merged, the largest move 0.23 mm, and every face keeps its BRAS material. The repair scripts and a full account of each change are
kept with the working data (`B:\data\m12\pearl-geom\cr2-clean\FINDINGS.md` and
`cr4-clean\FINDINGS.md`). The materials are BRAS's `fitted_estimates`, reduced to octave bands as
the arithmetic mean of each band's three third-octave values. The sources (LS1, LS2) and receivers
(MP1 to MP5) are at BRAS's measured positions (`positions.json`). The descriptions inside each
file say the same.

- `bras_cr2.simpa` is `B:\data\m12\pearl-geom\cr2-solve\CR2.simpa`
  (sha256 `445505dffc548a8e9529a2b1e163bdbb758a0613fd9cc55fd34c911c69f43dca`).
- `bras_cr4.simpa` is the working copy that the 2026-10-04 CR4 tour saved,
  `B:\data\m12\tour-cr4\work-20261004-123018\cr4\CR4.simpa`
  (sha256 `73245b893909ee937e4840d152c8b29c866e9575a32ac0f32ee379541ccd28ee`). Beyond the room it
  holds one sound-level plane, "Audience plane", and one variant, "Absorbing panels", which is
  active.

**Share-alike.** These two files are adaptations of BRAS. They are shipped, and may be passed on,
only under CC BY-SA 4.0, with this attribution. The share-alike term covers these data files. It
does not cover the app's code, which is GPL-3.0.
