# C1: a blank geometry to results (spec, 2026-10-07 13:25)

Burhan, 13:17, verbatim: "can someone take a proper geometry put it in here and then actually analyse the
room acoustics, like does this let me name a completelyblank geometry and actually assign material props?"
13:18: "can i name the walls if they dont even have assignments, are those features even there". 13:23:
"these are crucial features bro, please make sure they are there".

What a recon found at 13:18 (code read, not run): import of PLY/OBJ/STL with a unit/up-axis dialog and a
closed-room check (proven e2e, m10.scene); "New group from selection" from faces picked in the 3D view
(`app/ui/src/features/.../actions.ts:299-316`, e2e m11.groups row15-regroup); a flood-fill selection
(`viewport/floodfill.ts`); 11 library materials + "+ Material" with a third-octave absorption grid (e2e
m10.materials); sources/receivers add, place, move, rename (F2), delete (e2e m10.scene). MISSING or
UNPROVEN: no group rename, merge or split op (`ops.ts` has none); a file with NO groups has no explicit
fallback in `crates/simpa-core/src/geometry/import.rs`; source power, spectrum and directivity are read-only
text (`SourcesPanel.tsx:304-313`), no set op; per-band diffusion and transmission editing not seen in the
materials grid; no e2e spec starts from a raw blank OBJ (all use the corrected hall or tutorial fixtures).

## Done when

A stranger with a blank OBJ box can, in the app, with no file editing: import it; carve it into named
groups (floor, ceiling, four walls) from the 3D view; rename a group; merge two groups; give each a material
from the library or typed per band (absorption, and diffusion/transmission where the solver takes them);
add a source and set its power and spectrum; add receivers and a plane; run SPPS (CPU and GPU) and read the
acoustics and a map. Every one of those is driven by an e2e spec that starts from a RAW groupless OBJ written
by the test (a 6 x 10 x 3 m box), and passes.

## Order of work

1. **The bed first, before any code.** Write the box as a groupless OBJ (one object, no `g`, no `usemtl`)
   and a second with six `g` groups; drive the release app headless (`tools/devtools/`, DevTools port,
   `--e2e`) through import → groups → materials → source → receivers → run (CPU SPPS) → results. Record
   exactly where it stops and why (screenshots, the refusal text) in `BED.md` in this folder. This is the
   receipt for what is missing; build from it, not from the recon.
2. **Group-less import:** a mesh with no groups imports as one group named for the file (or "Surfaces"),
   never refused for that reason, and the Geometry step says so.
3. **Group ops** in the core (`schema/ops.rs`) + backend + UI: rename (F2 in the outliner, like sources),
   merge (select two or more groups → one, keeps the first's material), split = "New group from selection"
   as today, plus "Move selection to group". Undo for each. The outliner shows the face count per group.
4. **Source power and spectrum:** set the sound power level (dB, overall or per band on the same
   third-octave grid as materials), pick a spectrum from upstream's list or type one, pick a directivity
   from the ones the solver has (omni, and whatever upstream ships; measured balloons stay as they are).
   Read `docs/formats/` and the config.xml writer (`validate/export.rs`) for what SPPS reads; what the
   writer refuses, the UI refuses with the same words.
5. **Per-band diffusion and transmission** on a material: if the grid lacks them, add the rows, written to
   config.xml as upstream expects (SPPS-LOOP.md and A4/A6 cover how transmission is read); if the writer
   refuses a combination, the UI says why.
6. **The e2e spec** `m13.blank.e2e.ts` (or the numbering the specs use): the whole done-when, from the raw
   OBJ, both groupless and grouped. Pixel pins only where the existing specs pin.
7. Keep decisions in `docs/decision-log.md` (next free number after 72) and the scope ledger row.

## Constraints

- Worktree `B:\repos\I-Simpa_Night_Mode\.claude\worktrees\blank` (branch `blank`, from `gpu`). Outputs only
  to `B:\repos\I-Simpa_Night_Mode\.out\blank\` and `C:\tmp\nm-target-blank` (cargo). Solvers:
  `SIMPA_SOLVERS_DIR=C:\tmp\nm-solvers-gpu` (the verified five). Do not touch `C:\tmp\nm-target`, other
  worktrees, or any app.exe you did not start (Burhan has one open on CR4-27: never its window).
- Build the app with `npx --no-install tauri build --no-bundle` in `app/`, `CARGO_TARGET_DIR=C:/tmp/nm-target-blank`.
  Before any `cargo test`, check C: free space; stop and report under 15 GB (28 GB at 11:32). Run the test
  targets you touch; the simpa-core, simpa and app suites once at the end (`SIMPA_TETGEN160=B:\repos\I-Simpa_Night_Mode\target\solvers\build\src\tetgen\Release\tetgen.exe`).
- Another builder works on the bottom dock (Acoustics/Console/Runs panel layout) on branch `dock` at the
  same time: do not edit the dock's layout, resize or tab components; the outliner, materials grid, sources
  panel, ops and core are yours.
- Do NOT run the m10/m11/m12 gates (they open windows on Burhan's screen); run only your own new spec with
  the gate's e2e runner if it can take one spec (`tools/gates/m11.ps1 -Only e2e -Spec <name>`), and say in
  the report whether you did. Drive your own app headless.
- Read the solver's stderr on every run. Commit on `blank` as you go, messages on the why, no Claude
  attribution, no Co-Authored-By. Do not push. `npm run typecheck` and `npm test` pass at the end.

## Report

`docs/investigations/2026-10-07-blank-geometry/REPORT.md` on `blank`, committed. Lead with the done-when
(holds / what is missing), the BED.md findings before and after, what was built (files, ops), the e2e spec's
result, test counts, what remains unproven.
