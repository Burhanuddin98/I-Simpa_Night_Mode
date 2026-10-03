# Row 15 (1) build: face regrouping (G19) and grouped receivers on `.proj` import (M37), 2026-10-03

Branch `row15-groups` off `rebuild` @ `f617b1f`, worktree `.claude/worktrees/row15-groups`. Spec: `PLAN.md` beside
this file. Progress log and every receipt below: `B:\data\row15\`.

## What it means

Faces picked in the 3D view can now be sent to a new surface group (right click on the model, or Edit > New group
from selection). No face changes material without the user seeing it. If all the faces share a material, the new
group keeps it. If not, it gets upstream's placeholder, which blocks Run until a material is chosen. A surface
receiver or fitting zone keeps exactly the faces it had. A selection that would split one is refused, and says
which one. One undo gives the project back byte for byte. A `.proj` whose receivers sit in folders now imports
(before, it was refused), with each receiver's folder path kept and shown in the Scene panel. The solvers get the
same `config.xml` as for the flat project.

## Upstream's material rule, as read

`ProjectManager::OnMenuCreateFGroupFromSelection` (`projet.cpp:1638-1651`) appends
`new E_Scene_Groupesurfaces_Groupe(this)` (`e_scene_groupesurfaces.h:85-88`). That constructor
(`e_scene_groupesurfaces_groupe.cpp:70-97`) names it `Group` and sets `idmat` to **0**: reference material 0,
`Default`, the placeholder. The faces are then dropped in (`EndDrag`, `:376-428`), which does not touch `idmat`.
So **upstream always gives the new group the placeholder, whatever material the faces had.** It keeps the
emptied group (`e_scene_groupesurfaces.h:96-112` refuses only to delete a non-empty one). A surface receiver there
holds its own face list (a "pointer group", `PushFace`, `e_scene_groupesurfaces_groupe.cpp:250-263`), so moving
faces between material groups never affects it.

This differs from the plan's rule in one case: faces that all share a material. Upstream gives them the
placeholder; the plan, and this build, keep their material. The plan's rule is built as written (decision row 22
(2)), and the difference is reported here, not copied. The choice lives in one place, `Project::regrouped`
(`schema/ops.rs`), and the op accepts either: the faces' own material or the placeholder.

## Commits

| Commit | What |
|---|---|
| `02bdc6d` | Core: `Op::RegroupFaces { index, group, faces }`, `Project::regrouped`. Four new `OpError` codes: `faces`, `name_taken`, `split`, `material_change`. `tests/regroup_faces.rs`. The random-op fuzz now draws the op (38 ops) |
| `8739ae4` | Core M37: `PointReceiver::group` (optional, not written when `None`). `receiver_elements` walks nested `recepteursp`. New refusal `proj_receiver_group_malformed`, with its contract row. Four tests in `geometry_import_proj.rs` |
| `656c78d` | App: `Session::edit_regroup` and the `edit_regroup` command (the 39th; the m11.ps1 inventory is updated). `touches_geometry` covers the op. The checked apply lets a group the edit itself creates start with the placeholder. Bindings regenerated |
| `a985193` | UI: `regroupSelection` action, the view's context menu and the Edit menu entry, the receiver folder in the Scene panel. `sceneModel.regroupFaces` and `receiverFolder` |
| `d8fd468` | e2e: `m11.groups.e2e.ts` (`row15-regroup`, `row15-receiver-folders`), registered in m11.ps1 |

## How the op keeps its promises

- **Material:** `group.material` must be every listed face's base material, or the placeholder. Anything else is
  `material_change`.
- **Variants** (the plan did not cover them): under each variant, if every listed face has the same effective
  material, that material is carried to the new group as the variant's override (none when it equals the
  group's own). If the faces differ under a variant, the op is refused (`split`) unless the new group is the
  placeholder. `regrouped` keeps the common material only when the base project and every variant agree on it;
  otherwise it uses the placeholder. Under the active variant this reproduces the plan's rule. One case goes past
  the plan: faces with one base material that a non-active variant splits get the placeholder, because keeping
  the base material would silently change them under that variant.
- **Receivers and zones:** our model holds membership by group (`SurfaceReceiverShape::Scene { groups }`,
  `FittingShape::Surfaces { groups }`). A receiver or zone that holds every listed face gets the new group added
  to its list. One that holds none is untouched. One that holds some is refused (`split`, named).
- **The inverse** is a `Batch` of existing ops: the variant overrides removed, the receivers and zones replaced,
  `SetGeometry` back to the old face groups, and the new group removed. It is exact by construction, and the fuzz
  checks it.
- **The placeholder material:** if the project has none, `regrouped` adds upstream's reference material 0 in the
  same batch, so one undo removes both.

## Tests added, red then green

| Test | Red (`B:\data\row15\`) | Green |
|---|---|---|
| `crates/simpa-core/tests/regroup_faces.rs` (9): apply/undo exact through JSON text; common material; placeholder added and blocking (`material_placeholder` at `/surface_groups/3/material`) and reused; a foreign material refused; empty, out-of-range, duplicate, name taken, id taken, index, no such material refused with nothing changed; the scene receiver keeps faces [0, 1] (Floor moved, alone or both) and a straddling selection is refused; the same for a surfaces fitting zone; variant override carried, under base and active variant, and the 30 % common under the variant with mixed base; a variant that splits makes it a placeholder; the emptied Floor stays, validator output identical, `scene_mesh` and both `config.xml` identical | `red-step1-regroup.txt`: 15 compile errors | 9 passed (`green-step1-regroup.txt`) |
| `schema_roundtrip.rs` random ops: RegroupFaces drawn (half via `regrouped`), receivers drawn with a group | (extension of an existing test) | 10,000 x 24 ops: 162,838 applied, 77,162 refused unchanged; new codes seen: faces 1,878, material_change 97, name_taken 60, split 104; all undone/redone exactly; 38 op tags (`step1-roundtrip.txt`) |
| `geometry_import_proj.rs` (4): nested groups import with their paths (only the paths differ, ids normalised); the grouped import writes the same SPPS and TCR `config.xml` and `.cbin` as the flat one; a malformed child (unknown eid, no eid, in a group or in the list) refused as `proj_receiver_group_malformed` by name; files without the field load and resave byte for byte, and a grouped receiver round-trips | `red-step2-m37.txt`: 4 compile errors | 21 passed in the target (`green-step2-m37.txt`) |
| app `bridge::row15_tests` (3): one edit, `Group 1` with Walls' material, mesh revision moved, the mesh buffer's group index of faces 2 and 3 is 3, undo/redo byte for byte, next is `Group 2`; mixed faces accepted with the placeholder and blocked (`MATERIALS_UNASSIGNED`), a material set clears it, two undos byte for byte, and setting an existing group to the placeholder is still refused; a straddling selection is `OP_SPLIT` (not `OP_BATCH`), with nothing changed and no history; bad lists are `OP_FACES` | `red-step3-app.txt`: 7 compile errors | 3 passed; app 62 |
| UI `sceneModel.test.ts` (2): `regroupFaces` (faces once, ascending, null for any other selection), `receiverFolder` and the filter | `red-step3-ui.txt`: SyntaxError, no export | UI 156 of 156 |
| e2e `m11.groups.e2e.ts`: `row15-regroup`, `row15-receiver-folders` | written after the UI (no red) | 2 of 2 (below) |

e2e receipts (`m11-e2e-1.log`): teaching room faces [4, 5] (Left wall, by double-click) became `Group 1` with
Painted plaster. Left wall was emptied and kept, and the new row was selected. Slotted wood panel, picked in the
Materials step, landed on it alone. Two undos gave back the project text exactly. Control: with nothing picked,
the entry was disabled and a right click opened no menu. The edited tutorial 1 `.proj` (built in the spec: its
zip re-written, reports left out) listed Receiver 1 in `Stalls / Front` and Receiver 2 in `Balcony`, and the
filter `balcony` left only Receiver 2. The unedited `.proj` showed no folders.

## Gates

- Full core suite (`cargo test -p simpa-core -p simpa -- --test-threads 4`, nothing skipped, 594 s with the app):
  **1024 passed, 0 failed, 40 ignored** (`full-run1.log`). App crate: **62 passed** (`app-run1.log`). Total
  **1086 vs baseline 1070** (1011 + 59). The 16 added: core 13 (regroup 9, M37 4), app 3.
- `tools/gates/m11.ps1 -Only e2e -Spec groups` (`m11-e2e-1.log`): 2 of 2 required ids, 0 failures, 0 files left
  in the repository (a partial run, not a gate pass).
- Full `m11.ps1` (with m10 and m9): run after this file is committed; the result is in `m11-full-*.log` and the
  return contract.

## Left open

- **Plan deviations:** there is no command palette (it is not built), so the entry is in the view's context
  menu and in Edit. Variants are handled as described above. The checked apply has a narrow exemption: a new
  `material_placeholder` on a group the edit itself created is not a refusal. Without it the plan's
  "placeholder blocks Run" could never be reached, because the edit would be refused instead.
- Selections that straddle a receiver or zone are refused, not split into two groups. A user who wants that
  must make two selections.
- `PointReceiver::group` is new in the file. A build older than this one refuses a file that holds it
  (`deny_unknown_fields`); files without it are unchanged.
- The new group is named `Group <n>`; upstream names it `Group`. There is no rename in this piece (G20 / M39 are
  v1.x).
- `docs/decision-log.md` and `docs/scope.md` are not updated here: that happens at the merge, as for PQ3.
- `app/src-tauri/Cargo.toml` shows a line-ending-only diff in the worktree. It is not committed.
