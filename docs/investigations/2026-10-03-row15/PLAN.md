# Scope row 15 (1) plan: face regrouping (G19) and grouped receivers on `.proj` import (M37), 2026-10-03

Decision-log rows 21 (v1) and 22 (own piece). Receipts: `KEEL.md`, `SONAR.md` beside this file. Branch `row15-groups`
off `rebuild`, worktree `.claude/worktrees/row15-groups`.

## What "done" means (the load-bearing question, KEEL)

No face changes material, and no receiver joins or leaves a run, without the user seeing it. Concretely: regrouping
faces never gives a face a material it did not have unless the user chose it, every surface receiver and fitting zone
keeps exactly the faces it had, and one undo restores the project bit for bit. A `.proj` with receivers in folders
imports with every receiver present and its folder kept.

## G19: send selected faces to a new group

- **Core op** (`schema/ops.rs`): one invertible op that creates a surface group at an index and moves the listed faces
  into it (name it e.g. `RegroupFaces { index, group, faces }`; the builder may fit the existing op family). Its inverse
  puts every face back in its former group and removes the new group: apply then inverse is `==` the original, order
  included. Refused (nothing changes) when: a face index is out of range, the list is empty or has duplicates, the
  group id or name is taken.
- **Material of the new group:** the faces' common material when they all have the same effective material under the
  active variant; otherwise upstream's placeholder, so `material_placeholder` blocks Run until the user picks one
  (decision row 22 (2)). Builder reads upstream's `projet.cpp:1638-1651` and states what upstream does; if it differs,
  report it, do not copy a silent choice.
- **Surface receivers and fitting zones:** read how they reference faces or groups in our model. If membership is by
  group, the op must keep each face's receiver/zone membership (refuse, or carry it), never drop or extend it. Test it.
- **The emptied group** stays (empty) so undo is exact; check that `config.xml` export and the validator accept an
  empty group, or that the validator names it.
- **UI:** with faces selected in the viewport (selection kind `faces`), a context-menu / command-palette entry "New
  group from selection" applies the op through `actions.apply` (undo, dirty). The new group appears in the Scene panel.

## M37: grouped receivers on `.proj` import

- `PointReceiver` gains `group: Option<String>` (a path, the shape `Source.group` already has; serde default so older
  project files load unchanged; the schema bindings regenerated).
- `geometry/import/proj.rs` (~921-928): nested `<recepteursp>` inside `<recepteursp>` is walked like
  `source_elements` (~1051); each receiver keeps its folder path. A malformed nesting is refused by name, as
  `SOURCE_GROUP_MALFORMED` is for sources.
- `config.xml` export is unchanged (the solvers read receivers flat); test that a grouped import writes the same
  receiver list as the flat equivalent.
- UI: the Scene panel shows the folder path read-only (GUI groups are v1.x per parity).
- Fixture: no upstream `.proj` has nested receivers (SONAR 3), so build one by editing `tutorial_1.proj`'s XML in a
  test helper (the `tutorial3_edited` pattern, `tests/geometry_import_proj.rs:1254`). Update
  `every_upstream_project_imports_or_is_refused_by_name` if its expectations change.

## Order of work (tests first, each red before green)

1. Core G19 op + tests (apply/inverse exact; refusals; material rule; receiver/zone membership; empty group export).
2. Core M37 + tests (nested import, path kept, config.xml receiver list unchanged, older files load).
3. UI: G19 command, Scene panel group path; unit tests.
4. e2e: select faces in the teaching room, new group, assign a material, undo restores; import the nested-receiver
   fixture and see every receiver with its folder.
5. Gates: `tools/gates/m11.ps1` (with m10, m9) passes; workspace suite passes (baseline 1070).

## Traps (paid for)

As PQ3 (`docs/investigations/2026-10-03-pq3/PLAN.md`): ops travel as text through `Op::from_json`; build into
`C:\tmp\nm-target-h`, `CARGO_INCREMENTAL=0`, no debuginfo; the test env vars; suite timeout 5400000 ms; tauri-driver
port 4444 one e2e at a time; outputs to `B:\data\`. The gate fails on any untracked file in the worktree: commit
reports before the final gate run. Do not reuse `tools/extract_upstream_scene.py`'s centroid heuristic as a regroup op.
