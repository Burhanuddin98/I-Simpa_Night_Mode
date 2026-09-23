# Provenance: `tg_bad`, a self-intersecting cube

**Source.** The contract survey's scratch folder `libif\tg_bad\` (2026-09-23). It was backed up
to `target/scr-libif-backup/tg_bad/` and copied from there unmodified. `scene_mesh.1.edge` and
the empty `tg.stderr` were left out.

**How it was made:**
1. The survey's probe exported upstream's `lib_interface/tests/cube.cbin` (a 5 m cube, 12
   faces) through upstream's own `CPoly::ExportPOLY`, with face `i` given facet marker `i`:
   - the command was `probe_formats.exe poly cube.cbin <out>`
   - the code is `probe.cpp:84-91` in the backup
   - the result is `tg_ok/scene_mesh.poly`
2. `mkpolybad.py` (in the same backup) copied that poly and added three nodes, (2.5, 2.5, 2.5),
   (7.5, 2.5, 2.0) and (7.5, 2.5, 3.0), and a 13th facet `1 0 12` / `3 9 10 11`. The new facet
   pierces the x = 5 wall.
3. The survey ran upstream's pinned TetGen on it with the command in the trailer of every `.1.*`
   file:
   `B:\repos\I-Simpa-upstream\build_solvers\src\tetgen\Release\tetgen.exe -pq5 -A -n -Y scene_mesh.poly`.
   `tg.stdout` is its output.

The survey row is `docs/rebuild-plan-raw-2026-09-23.json`, line 1187, in the TetGen format row
under "Failure signature". It records exit 3 and "The input surface mesh contain
self-intersections. Program stopped." on stdout with stderr empty. It also records partial
`.1.node`, `.1.ele`, `.1.face` and `.1.edge` output, no `.1.neigh`, and
`scene_mesh_skipped.face` listing facet markers 8, 9 and 12.

**What it is expected to show** (`crates/simpa-core/tests/mesh_verify.rs`, `tg_bad_folder`):
`verify_dir` finds basename `scene_mesh`, 3 skipped facets with markers `[8, 9, 12]`, and exactly
the codes `tetgen_skipped_facets` and `neigh_missing`. There is no `.mbin`, so no mesh report.

The markers are the `.poly` facet markers, TetGen's `shellmark`, written as the last column
(`tetgen.cxx:20462, 21338`). Markers 8 and 9 are faces of `cube.cbin`. Marker 12 is the added
facet, so a scene that maps it back needs 13 faces. It is also gate M5(c)'s input for
`simpa mesh <file.poly>` (`docs/m5-m6-design.md`, "Gate amendments").

| file | bytes | sha256 |
|---|---:|---|
| `scene_mesh.poly` | 533 | `0872259eae74fd2393279b1f3bce87022afa4fe249a3ad333b6b69baaafe7fd8` |
| `scene_mesh.1.node` | 335 | `ac9af3ec576aa1235a7fdbc10a8e108b5106e69f0cce3bb5d70a906977b22370` |
| `scene_mesh.1.ele` | 773 | `5f5a31057522d31847649db8b5f79d75b4b621d41b884e63fe79d9e8b6e236de` |
| `scene_mesh.1.face` | 436 | `ffbd1efce5d93f2ff4294d0efc5ece744e1d8f8e593e2cd81860a9c47087a177` |
| `scene_mesh_skipped.face` | 47 | `be0181c0dc01f873d7c2c6b7dc962b67b51ad8a6206d4faf3be013dc644f1806` |
| `scene_mesh_skipped.node` | 335 | `ac9af3ec576aa1235a7fdbc10a8e108b5106e69f0cce3bb5d70a906977b22370` |
| `tg.stdout` | 814 | `43a0f68c513501692b888d0049fd5fed04d35d70bd47a8d522c323e807f0db61` |
