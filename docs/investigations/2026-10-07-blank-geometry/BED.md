# C1 bed: a raw blank box through the release app

The instrument is `bed.py` in this folder. It writes a 6 x 10 x 3 m box as an OBJ, either groupless (one
object, no `g`, no `usemtl`) or with six `g` groups. It then drives the release `app.exe --e2e` over the
DevTools protocol only, with no window focus and no OS mouse. The import goes through the dialog's Import
button. Faces are picked in the 3D view with real pointer events (`aimAtFace`, then a click or a
double-click), menus and rows are clicked, F2 and Enter are sent as keys, and the grid takes a paste. A
control that is not on the page is recorded as MISSING with what was looked for. Receipts:
`bed.json`, `bed.log` and screenshots under `.out\blank\bed-<before|after>-<blank|grouped>\`. The
solvers are the verified five, from `C:\tmp\nm-solvers-gpu`.

The bed runs its own WebView2 data folder (`WEBVIEW2_USER_DATA_FOLDER`). With Burhan's app open, a
shared folder joins his browser process, which ignores the debugging port, and the bed then never
connects. That was the first attempt, at 13:31.

## Before the build (blank `20e8f09`, app built 13:27, run 13:33 to 13:39)

| Step | Groupless box | Grouped box | What the app did |
|---|---|---|---|
| Import through the dialog | OK | OK | Groupless: one group, `default` (the OBJ reader's name before any `g`), 12 faces. The Geometry step says "Surface groups 1" and nothing about the file having none. The check passed: closed, 0 open edges. Grouped: six groups named from `g` (`wall_south` and so on). |
| Carve floor, ceiling and south wall (double-click, right click, New group from selection) | OK | n/a | `Group 1` to `Group 3`, 2 faces each, material `Default` (the placeholder) |
| Split one face (north triangle 1) | OK | n/a | `Group 4`, 1 face |
| Move selection to group (north triangle 2) | **MISSING** | n/a | The view's menu holds only "New group from selection" |
| Merge two groups (west triangles, Ctrl+click both rows) | **MISSING** | n/a | No multi-select in the scene list; the Edit menu has Undo, Redo and New group from selection |
| Rename a group (F2 on its row) | **MISSING** | **MISSING** | A row click selects the group and moves to the Materials step. F2 focuses nothing, and no panel has a group name field. |
| Library material per group | OK | OK | `+ From library`, then the material option |
| Absorption typed per band (+ Material, one row pasted) | OK | OK | 7 octave bands (the import default), values exact |
| Scattering typed per band | OK | OK | The grid's Scattering tab |
| Transmission typed per band | **MISSING** | **MISSING** | The grid has two tabs. The panel reads "Transmission is read-only in this version". |
| Add a source | OK | OK | `S1` at (3, 5, 1.5), the room's centre in plan at 1.5 m |
| Source power, spectrum, directivity | **MISSING** | **MISSING** | The fields are name and position only. Emission is read-only text: "Read-only in this build". |
| Two receivers and a plane | OK | OK | |
| Run SPPS on the CPU | **REFUSED, then FAIL** | **FAIL** | Groupless: first refused with `MATERIALS_UNASSIGNED`, because the second west triangle could not be merged and kept the placeholder. Once the bed gave it a material, the run ended **FAIL `PARTICLE_LOSS_EXCESS`**: 3.15 % of particles were lost to infinite loops in every band (4731 of 150000 at 125 Hz). The solver's stderr is empty, and so is TetGen's. |
| Run SPPS on the GPU | FAIL | not run | The same, 3.12 % |
| Results, Acoustics, map | **FAIL** | **FAIL** | Results "refused", and the Acoustics tab: "This run's results were refused: no value is shown." |

### The run failure, isolated (13:35 to 13:37, `.out\blank\loss-exp*`)

The saved groupless project was rerun with one change at a time, on SPPS on the CPU (receipts:
`run.json` `particles.bands`):

| Change | Lost to loops, per band | Verdict |
|---|---|---|
| none | 3.13 to 3.22 % | FAIL |
| max tetrahedron volume 1 m³, then 0.1 m³ | 3.06 to 3.21 % | FAIL |
| `-Y` off, run 2 s instead of 10 s, transmission off | about 3.15 % | FAIL |
| plane removed, every receiver removed | about 3.15 % | FAIL |
| one material on every wall | about 3.15 % | FAIL |
| **source moved to (2.1, 3.7, 1.33)** | **0.00 %** | **OK** |
| source at (3, 5, 1.2) or (3, 5.3, 1.5) | 0.00 % | OK |
| source at (1.8, 3, 1.2) | n/a | FAIL `source_unlocatable` (on an internal facet; the verdict already names that one) |

So the loss follows one point. It is not the mesh, the materials or the receivers. `+ Source` puts a
source at the room's centre in plan, 1.5 m up. In a 3 m box that is the room's exact centroid, which
lies on the box's main diagonal: an edge every tetrahedron of the 6-tet mesh shares, and one that
refinement keeps. From there 3 % of the particles loop. The verdict calls this `PARTICLE_LOSS_EXCESS`,
and nothing on screen points at the source. Any stranger who imports a 3 m high box and presses
+ Source gets a refused run.

### What this bed says to build

1. A groupless import names its group for what it is, and the Geometry step says the file had no groups.
2. Group rename (F2), merge, and Move selection to group, each one undo step. Face counts in the scene list.
3. Source power (overall and per band), spectrum and directivity.
4. Transmission per band in the grid.
5. The run failure from a centred source. This is a solver behaviour with a symptom the app misnames. It
   is reported for Burhan's call, not fixed in the solver. See the report for what was done about it.
