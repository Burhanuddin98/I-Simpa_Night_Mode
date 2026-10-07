# C1 report: a blank geometry to results (2026-10-07, 13:26 to 14:35)

The spec (SPEC.md, "Report") asks for this file on `blank`, committed.

## Done when: holds

A stranger with a blank OBJ box can now do every step in the app without editing a file. They import it
and carve it into floor, ceiling and four named walls from the 3D view. They rename a group, merge two
groups, and move a picked face into a group. They give each group a library material, or one typed per
band: absorption, scattering, and transmission loss. They add a source and set its power (overall or per
band), spectrum and directivity. They add receivers and a plane, run SPPS on the CPU and on the GPU, and
read the acoustics and the plane's map.

`m13.blank` drives all of it from a raw OBJ that the test writes itself, a 6 x 10 x 3 m box. It runs once
groupless and once grouped. It passed **9 of 9** on the m11 harness:
`tools/gates/m11.ps1 -TargetDir C:\tmp\nm-target-blank -Only e2e -Spec m13.blank -SolversDir C:\tmp\nm-solvers-gpu -SkipPrior`.
The run ended 14:18; its work folder is `C:\tmp\nm-target-blank\gates\m11\20261007-141404\`. This is a
partial run of the harness, not a gate pass. Its window opened on the screen, unfocused, and the focus
watcher passed.

**Gaps.**

- A source **at the room's exact centre** still fails SPPS (see BED). The fix here is a different default
  position, not a fix in the solver.
- The **map** is checked on the CPU run only.
- The m10, m11 and m12 gates were **not run**, as the spec says.

## What the bed found, before and after (BED.md)

**Before**, on the release app at `20e8f09`:

- Import and carving worked.
- **Missing:** rename, merge, move to group, transmission editing, and the source's power, spectrum and
  directivity. The emission read "Read-only in this build".
- A groupless file imported as a group named `default`, with nothing said about it.
- **SPPS on the CPU and the GPU failed `PARTICLE_LOSS_EXCESS`** on the raw box, with 3.1 to 3.2 % of
  particles lost to loops in every band. Isolated by changing one thing at a time, the cause is the
  source's position. + Source put the source at the box's centroid, which lies on an edge every
  tetrahedron of the mesh shares. Moved, the source loses 0.00 %. Mesh size, run length, receivers, the
  plane and the materials changed nothing.
- So **no stranger's 3 m box could reach results** with the default + Source.

**After**, on `2f48212`: every step is OK on both boxes. CPU 5.5 s, GPU 1.0 s, both OK, stderr empty,
Acoustics ready, and the plane's map holds 120 faces.

## What was built

**Core** (`crates/simpa-core`):

- `Op::MoveFaces` ("Move selection to group"). The faces take the group's material.
- `Op::MergeSurfaceGroups`. The merge goes into the first group, which keeps its name and material.
  The others are removed together with their variant overrides.
- Both are exact single undo steps. Both refuse `split` when a scene receiver or zone would gain or lose
  faces, and the merge refuses a bad group list with a new code, `groups`.
- `ImportReport::ungrouped`. A file that declares no groups (OBJ, PLY or STL) gets one group named after
  the file, with its ids unchanged.
- `ReferenceSpectrum::shape_on`: upstream's eight reference spectra on the project's bands. Octave bands
  are the energy sum of their thirds.
- Tests: `tests/group_ops.rs` (9 tests), and the undo fuzz in `schema_roundtrip.rs` now covers the new
  ops (40 ops).

**Backend** (`app/src-tauri`):

- A new `spectrum_library` command, allowed in `build.rs` and the capability. The bed caught the missing
  ACL entry.
- `ProjectInfo.imported_ungrouped`, plus a Console line at import.
- `touches_geometry` now covers the new ops.
- The bindings are regenerated.

**UI** (`app/ui/src`):

- **Scene list** (`ScenePanel.tsx`, logic in `chrome/groupsModel.ts`): a face count on each surface row;
  F2 or a double-click renames a group in its row; Ctrl+click picks several groups, and a Merge bar
  appears. A blank or taken name is refused inline with `GROUP_NAME_EMPTY` or `GROUP_NAME_TAKEN`.
- **Edit menu:** Rename group (F2) and Merge groups.
- **3D view menu:** "Move to group", with one entry per group (`Viewport.tsx`).
- **Geometry step:** the "No groups in the file" note.
- **Sources step:** `chrome/EmissionEditor.tsx`, logic in `chrome/emission.ts`. It edits the power
  overall or per band, the spectrum from upstream's list (or typed per band), and the directivity (codes 0
  to 4; a balloon is kept as it is). Each change is one `replace_source`.
- **Materials grid:** a Transmission tab. An empty band does not transmit; each material's change is one
  `replace_material`.
- **+ Source** now goes to `sourceSpot`, at 0.382 by 0.414 of the box (decision 73).

**Other files:**

- e2e: `app/e2e/specs/m13.blank.e2e.ts`. `tools/gates/m11.ps1` registers `m13.blank` and copies
  `spps-gpu.exe` into the private solvers when `-SolversDir` has it.
- Docs: decision 73, and the scope ledger row.
- The bed itself is `bed.py`.

## Test counts (14:19 to 14:32)

| Suite | Result |
|---|---|
| simpa-core | 918 passed, 0 failed, 18 ignored (78 test binaries, the gate's selection) |
| simpa | 70 passed, 0 failed, 19 ignored |
| app | 95 passed |
| `npm run typecheck` | passes |
| `npm test` | 346 of 346 |
| `m13.blank` | 9 of 9 |

## What remains unproven or open

- **Default source position.** A source at a room's exact centre, or on any shared mesh edge, still loses
  particles to loops in SPPS. The verdict names it `PARTICLE_LOSS_EXCESS` and does not point at the
  source. + Source now avoids the centre, but a user who types the centre gets the same refusal.
  - A pre-run check of the source against the tetrahedral mesh would name the cause. That is a decision
    for Burhan (decision 73, revisit), and it was not built.
- **Transmission through an outer wall.** It was typed and switched off again, but never run with
  transmission on. With nothing behind the wall, what SPPS does with the transmitted particles was not
  measured.
- **Directivity in a run.** Only omni was run. Unidirectional and the plane directivities were edited and
  undone, but no run used them.
- **Spectrum in a run.** No band level was checked against `config.xml`. The e2e run used ES_VL with
  1 kHz typed, and `config.xml` writes it through the existing `band_levels_db`, which is not new.
- **Group names.** Rename does not check names in the core, only in the scene list. A `.proj` can still
  bring duplicate group names.
- **Large models.** The view's "Move to group" list scrolls past twelve groups, but no hall with many
  groups was tried.
- **The other builder's app.** At 14:19 and 14:21 the bed connected, by a port clash on 9241, to the
  `dock` builder's running `app.exe`. It drove that app: an import, a carve, and two SPPS runs saved
  into `.out\blank\bed-wrong-app-port9241\`. Its process was not stopped, and the bed now uses port 9263.

## Blockers

None.
