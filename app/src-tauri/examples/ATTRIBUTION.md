# The example projects: sources and licences

These six `.simpa` files ship inside the app (`src/examples.rs`, `include_bytes!`) and are
offered on the landing page while no project is open. Opening one writes a copy into the user's
`Documents\Night Mode\Examples` folder. The files here are never modified at run time.

| file | room | source | licence |
|---|---|---|---|
| `elmia_hall.simpa` | Elmia hall | Upstream I-Simpa's tutorial 2 (`tutorial_2.proj`) | GPL-3.0 |
| `industrial_hall.simpa` | Industrial hall | Upstream I-Simpa's tutorial 3 (`tutorial_3.proj`, `Industrial_hall.ply`) | GPL-3.0 |
| `bras_cr1.simpa` | BRAS CR1, coupled rooms: laboratory and reverberation chamber (BRAS scene 8) | The BRAS database | CC BY-SA 4.0 |
| `bras_cr2.simpa` | BRAS CR2, seminar room (BRAS scene 9) | The BRAS database | CC BY-SA 4.0 |
| `bras_cr3.simpa` | BRAS CR3, chamber music hall (BRAS scene 10) | The BRAS database | CC BY-SA 4.0 |
| `bras_cr4.simpa` | BRAS CR4, auditorium (BRAS scene 11) | The BRAS database | CC BY-SA 4.0 |

## Elmia hall

A byte-for-byte copy of `tests/fixtures/rooms/elmia_corrected.simpa`
(sha256 `70439d85fc202377207d2a9d0539a439713f4f74b5f81a26115f20aa9690a9d4`). It holds the
settings of decision-log row 49. The geometry, groups, materials, sources and receivers come from
upstream I-Simpa's second tutorial project, `src/isimpa/resources/doc/tutorial/tutorial 2/tutorial_2.proj`
(UniversitÃ© Gustave Eiffel, <https://github.com/Universite-Gustave-Eiffel/I-Simpa>), imported by
`simpa_core::geometry::import::import_proj`. `tests/fixtures/rooms/PROVENANCE.md` documents the
import and every edit. I-Simpa is GPL-3.0, as is this app.

## Industrial hall

Upstream I-Simpa's third tutorial project, `src/isimpa/resources/doc/tutorial/tutorial 3/tutorial_3.proj` with
`Industrial_hall.ply` (UniversitÃ© Gustave Eiffel, GPL-3.0), imported by `simpa import-proj` (88 faces, 10 surface
groups, 2 fitting zones, 6 sources on two milling machines, 5 receivers, upstream's cutting plane; closed, no self-intersection), then edited
by `build_industrial.py` beside this file: Receiver 1, which upstream placed on a wall (`simpa validate`:
receiver_on_surface), moved along its own line to (0.5, 1.5, 1.6); the sources renamed per machine; SPPS at the
app's new-project defaults. sha256 `f8fca0216b9c485e8e3316d3314f7a60df5ea276e24b61b5af5cad2e579adefe`.

## BRAS CR2 and BRAS CR4

Both rooms are derived from the Benchmark for Room Acoustical Simulation (BRAS):

> L. AspÃ¶ck, M. VorlÃ¤nder, F. Brinkmann, D. Ackermann and S. Weinzierl, *Benchmark for Room
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

## BRAS CR1 and BRAS CR3

Derived from BRAS as CR2 is (the citation and licence above), built on 2026-10-06 by
`.out\examples\build_bras_room.py` (generalised from CR2's `build_cr2.py`): the sealed geometry of
`B:\data\m12\pearl-geom\cr1-clean` (BRAS `CR1_RIR_DoorAngle3_Dodecahedron.skp`, the door at 30.4 degrees) and
`cr3-clean` (scene 10), with each room's `fitted_estimates` materials reduced to octaves as for CR2.
CR3's sources (LS1, LS2) and receivers (MP1 to MP5) are BRAS's `positions.json`. CR1 has no entry there;
its positions are read from `CR1_RIRs_DoorAngle3_Dodecahedron.sofa` as `positions.json` was made from the
other rooms' SOFA files: the emitters by EmitterID (LS1, LS2, the dodecahedron's mid-frequency driver) and
the receivers by ReceiverID (MP3, MP4). Every source and receiver of both rooms was checked to lie inside
the room. Both files pass `simpa validate` with no issue.

- `bras_cr1.simpa` sha256 `e558fb6f5c494395c3ed959074a3db2870ffc9e464d92174b761600f241b89a2`.
- `bras_cr3.simpa` sha256 `bc0843f5fbe2ba90dfe2b297831aec24be27595ca964aff2b2db75b51e66bb8c`.

**Share-alike.** These four BRAS files are adaptations of BRAS. They are shipped, and may be passed on,
only under CC BY-SA 4.0, with this attribution. The share-alike term covers these data files. It
does not cover the app's code, which is GPL-3.0.

## Dry clips (C5, decision 75)

Three short recordings ship in `clips/` (`src/aural.rs`, `include_bytes!`) for the Results step's Listen
window. Each has a provenance file beside it: the source page, the file fetched, the author, the licence
quoted from its page, the date fetched, both sha256s and the processing (trim, mono, 48 kHz, fades, peak
-1 dBFS, 16-bit). None is a measured anechoic recording; each is close-miked and dry, and its file says so.

| file | what | author | licence |
|---|---|---|---|
| `clips/speech-lv-hislastbow.wav` | Speech, 7.95 s, from LibriVox's "His Last Bow" | Zachary Brewster-Geisz (reader), Arthur Conan Doyle (text) | Public domain (LibriVox: "all our recordings are public domain") |
| `clips/tenorsax-vcsl-c3.wav` | Tenor saxophone, one note, 7.37 s | Versilian Studios / Sam Gossner, VCSL | CC0 1.0 |
| `clips/harp-vcsl-c5.wav` | Concert harp, one note, 7.01 s | Versilian Studios / Sam Gossner, VCSL | CC0 1.0 |
