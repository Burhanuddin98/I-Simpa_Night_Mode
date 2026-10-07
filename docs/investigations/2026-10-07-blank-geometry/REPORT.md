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

## Fixes after audit (verdict SHIP-WITH-FIXES, 14:35; fixed 14:36 to 14:58)

1. **Group names are unique in the core** (`5322081`).
   - The core compares names trimmed and without case, by the validator's own `collision_key` (the new
     `validate::group_name_key`).
   - `Op::Rename` on a surface group, and `Op::RegroupFaces`, refuse a taken name with `name_taken`.
   - The validator warns about a project that already holds two alike (`group_name_duplicate`). It is a
     warning, documented in solver-contract Part A.
   - `renameProblem` in the scene list uses the same key and sends the trimmed name. The bridge's
     `Group n` picks a free name by the same key.
   - **Undo trade-off.** Undo, redo and a batch's rollback apply recorded ops through
     `Op::apply_recorded`, which skips the name check. Without that, undoing the rename that told an
     imported duplicate apart would fail: the undo fuzz caught exactly this on the first try.
   - **No results file or folder is named after a group.** `config_xml::write` uses a group's name only
     in an error message, and no solver reads it.
   - Tests: `group_ops` (refusals by case and by whitespace, the validator's warning, an exact undo
     back to the duplicate), the undo fuzz `schema_roundtrip`, and `groupsModel.test.ts`.
2. **The transmission the solver gets is shown on its cell** (`4ef4108`; spec fix `71a467f`).
   - A loss the writer clamps to tau = alpha reads, for example, `5.00 → 13.01`. A band whose
     absorption is 0, which the writer leaves out, reads `→ off`. The cell's tooltip says why, in the
     writer's terms.
   - `transmissionWritten` mirrors `transmission_loss_written` (write.rs:623) and is tested.
   - `m13.blank` asserts the cell, its text and its tooltip: 5 dB at alpha 0.05 shows "5.00 → 13.01".
   - The validator's warning waits until a group uses the material (`validate/project.rs:85-95`), so
     the cell is the only place that says it while the material is unused.
3. **Full names in the scene list** (`ae6da2c`).
   - A surface row now has two lines. The name has the row's whole width; the face count and the
     material sit under it and shorten first. The tooltip carries the full name.
   - Six names of 22 to 24 characters at the default panel width: none truncated (`scrollWidth` ≤
     `clientWidth`, 201 px wide). Screenshot: `.out\blank\names-after\scene-names.png`; the widths are
     in `rows.json` beside it.
4. **A source on a mesh edge** (`fa8e68c`). Bed: `bed_edge.py`, CPU SPPS, receipts in `.out\blank\bed-edge\`.

   | Source at | Verdict | Lost to loops, per band |
   |---|---|---|
   | the centroid (3, 5, 1.5) | **FAIL** `particle_loss_excess` | 3.11 to 3.31 % |
   | typed on the interior edge, (1.8, 3, 0.9) | **FAIL** `particle_loss_excess` | 4.31 to 4.41 % |
   | the control, off the edge, (2.1, 3.7, 1.33) | **OK** | 0.000 to 0.002 % |

   - Neither failing case passed as OK.
   - The reason's detail now ends "Source S1 sits on an edge of the room mesh; move it a few
     centimetres." The line is added when an enabled source lies within 1 mm of an edge (or so of a
     node) of the run's **tetrahedral** mesh: `run::locate::sources_on_edges`, called in
     `manager::judged`. The tetrahedral mesh, not the triangle surface mesh, because the centroid is
     1.5 m from every surface edge and lies on the box's one interior edge.
   - It is a hint in a failure. Nothing is refused before the run for it.
   - Test: `locate::tests::a_source_on_a_mesh_edge_is_named`.
   - The beds now refuse a DevTools port already in use, and check after connecting that the page
     belongs to the `app.exe` they started (`bedport.py`).
5. **No reference spectrum is ever dropped** (`b2df081`). The case the audit names cannot occur. The
   lowest octave a band set allows (63 Hz) spans 50, 63 and 80 Hz, and the highest (16 kHz) spans 12.5,
   16 and 20 kHz, all inside upstream's 27 bands. So `shape_on` is `None` only for a non-nominal
   frequency, which the schema refuses. The doc says so, and `group_ops` checks all eight spectra on
   the widest third-octave and octave sets. No menu text was needed.
6. **The dead pink and white fallback** in `spectrumOptions` is removed (`d6b1123`).

**Re-runs after the fixes:**

- `m13.blank`: 9 of 9 on the same partial harness at 14:54
  (`C:\tmp\nm-target-blank\gates\m11\`, newest stamp).
- Touched core targets: 393 passed, 0 failed, 1 ignored. These are `--lib`, `group_ops`,
  `regroup_faces`, `reason_codes_docs`, `run_manager`, `run_verdict`, `run_locate` and
  `settings_editor`, plus the release `schema_roundtrip` 17/17.
- app: 95 passed.
- `npm run typecheck` passes, and `npm test` is 347 of 347.

The full simpa-core and simpa suites were not re-run after the fixes; their last full run was at 14:32,
before them.
