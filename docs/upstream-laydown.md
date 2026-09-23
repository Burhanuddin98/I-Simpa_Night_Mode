# Original I-Simpa: why it is bad to use

**2026-09-23.** A read-only laydown of upstream I-Simpa at `v1.4.0_snapshot_14_01_2026`
(`929a5c8`). That tag is upstream `main` and the latest release online, checked with
`git ls-remote` at 01:42. It was produced by seven independent readers covering the data
model, the UI shell, the 3D engine, the interface libraries, the official tutorials, all 228
GitHub issues and the forks. A synthesis agent combined their reports, and an adversarial
critic checked the file references: 30 of the 33 it opened held.

The raw result, with every receipt, is in `upstream-laydown-raw-2026-09-23.json`. The
corrections below are the critic's, already applied.

⚠️ **Nobody ran I-Simpa.** Every user-visible effect here was read from code, docs or issues,
or inferred from them. None was observed. The cheapest confirmation of reasons 1 and 2:
import a self-intersecting box into the official build and watch what the run reports.

## The verdict

The solvers are worth keeping. They are separate processes behind a plain `config.xml` and
working-folder contract. The GUI around them is the problem: it reports failed meshes and
solves as finished runs, freezes for the whole solve, routes every action through tree nodes
and right-click menus, draws with fixed-function OpenGL 1.x, and has undo and save paths that
lose work. Of the 71 forks the GitHub API returns, only one carries substantial work, and it
has been dormant since 2023. Nobody maintains an alternative GUI.

## The repo

| Component | Size | What it is |
|---|---|---|
| `src/isimpa` | 68,081 lines, 361 files | The wxWidgets GUI: project model, trees, property grid, 3D view, solver launch, results, embedded Python |
| ↳ `data_manager` | 31,819 | One generic tree of `Element` nodes. Properties are child nodes looked up **by name string**. Each class draws its own tree item, menu, grid row, GL geometry, XML and solver config |
| ↳ `IHM` + `main` + `manager` | 14,382 | AUI frame with 6 menus and 5 toolbars, 3 tree tabs, a `wxGrid` property sheet, a console. `processManager` runs every executable synchronously |
| ↳ `3dengine` + `GL` | 14,310 (about 1,060 dead) | Fixed-function OpenGL 1.x: immediate mode and display lists, no shaders, no VBOs |
| `src/lib_interface` | 47,926, of which about 14.3k is upstream's own | Every GUI↔solver file format, the solver-side `config.xml` loader, geometry tools. The rest is vendored pugixml, rply and utfcpp |
| `src/tetgen` | 44,890 (vendored 1.6.0) | AGPL-3 or a paid commercial licence |
| `src/VolumetricMeshRepair` | 6,249 | Marching-cubes "average model remesh". Internals not read |
| `src/spps` | 3,017 | The particle solver. One argument: the `config.xml` path |
| `src/preprocess` | 2,109 | Scene repair. Runs by default at every import. Internals not read |
| `src/ctr` | 1,402 | TCR, classical theory |
| `src/python_bindings` | 1,037 | `libsimpa` SWIG module: read and write every format headless |
| Tests | — | Format round-trip tests only. No GUI or end-to-end tests |

The physics is about 4.4k lines. Nearly everything else is interface.

## Why it is bad to use, ranked

Corroboration means the reason was found independently in the code, in the vendor's own
tutorials and in user issues.

**1. Failed meshes and failed solves look like finished runs.** *(code, issues and docs agree)*
`uiRunExe` treats only a negative return as failure (`manager/processManager.cpp:155,172-178`).
When TetGen meshing aborts it logs "Meshing aborted" and returns `true`
(`data_manager/projet_maillage.cpp:140-142, 280-282`). `RunCoreCalculation` stores the solver's
result and never reads it, then refreshes Results anyway (`data_manager/projet.cpp:811-846`).
The solvers' streams are inverted: fatal lines go to stdout as ordinary messages, while the
lost-particle warning goes to stderr. There is no status bar or notification, only a console
pane. The effect: a self-intersecting model produces an instant "finished" run with an empty
results folder that looks like a real one. #421 (2026): *"The simulation finishes almost
instantly"*; the maintainer's answer was *"You have to fix your 3d model."*
*Critic's correction:* SPPS's `main()` returns 0, but `lib_interface` can call `exit(1)` or
`exit(-1)` (`coreTypes.cpp:215`, `coreinitialisation.cpp:432`). An `exit(-1)` is logged as
failed and then ignored anyway.

**2. Getting geometry in is the main barrier.** *(code, tutorials, issues and a 2025 paper agree)*
The loaders read only `.3ds .ply .poly .bin .stl`: no OBJ, DXF, IFC, SKP or glTF. Binary STL
is broken, with face indices of `(idtri, 3*idtri+1, 3*idtri+2)` and the file opened in text
mode (`3dengine/Core/stl.cpp:316-319`). The official Elmia repair takes about 25 actions and
two imports. *Critic's correction:* validity is not entirely unchecked. The "Repair model"
option, on by default, runs `preprocess.exe` at every import (`projet.cpp:3217-3229`), but its
outcome is invisible and it exits 0 whatever happens.

**3. Every task goes through trees and right-click menus.** *(code, tutorials and 61 GUI-usability issues agree)*
Properties sit in a `wxGrid` sorted alphabetically by translated label
(`IHM/MainPropGrid.cpp:45-48`). Almost every create and run verb lives in context menus. The
simplest official tutorial, a 6×10×3 m box with 3 materials, 1 source and 2 receivers, is 36
numbered steps and about 110 actions end to end, runs and results included. Setup alone is
roughly 65 to 75 actions, and assigning 3 materials is 31 of them. One custom material is
about 36 actions, 30 of them cell edits. *Critic's correction:* the menu bar does carry a few
verbs (Meshing, Compare surface receivers, New scene). The "62 menu sites across 33
overrides" count was not reproduced, so it is not quoted here.

**4. A solve freezes the whole app.** `wxExecute(..., wxEXEC_SYNC)` sits under the source's
own comment `// TODO run the program in asynchronous mode`, inside an app-modal progress
dialog (`processManager.cpp:99-147`). You cannot orbit, edit or look at earlier results for
the length of a solve. Issues #69 and #181 report that Cancel did not stop SPPS. Whether
Cancel is clickable at all under a synchronous call was not observed.

**5. The 3D view is legacy OpenGL and renders wrongly.** CMake forces LEGACY GL
(`CMakeLists.txt:401-406`), and there is no MSAA. **The mouse wheel is thrown away**
(`3dengine/Camera.cpp:182-185`); zoom is a left-drag. #185, open since 2017: surface
receivers and legends render black on Linux and some Windows GPUs. The maintainer called it
"wrong OpenGL calls" and promised a new engine, which never shipped. The 2020 forum advice
was Windows 7 compatibility mode.

**6. Undo and save lose work.** Undo keeps at most 5 whole-XML snapshots, taken *after* the
edit, so the first Ctrl+Z reloads an identical copy (`projet_undo_redo.cpp:34-106`). Save
compares modification times truncated to the minute, zips every past run into the `.proj`,
and logs "Save finished" whatever happened (`projet.cpp:2507-2535`). The tutorial projects are
95% report and TetGen temp files. #73: a corrupted 2.5 GB `.proj`. The vendor's own docs
recommend turning Undo off.

**7. Results are a raw folder mirror.** The Results tab mirrors
`report/<code>/<timestamp>/` five levels deep. RT, EDT, C, D, STI, G and LF are not produced by
the run: each needs a right-click and a modal dialog on one receiver file, so 20 receivers
mean 20 dialogs. Every double-click opens another floating 380×200 pane
(`projet.cpp:3184-3188`). **Every STI value is computed with female weighting** because
`if(gen='F')` is an assignment, not a comparison (`projet_calculation.cpp:748`, #427).

**8. It crashes on routine property edits.** *(symptom only; cause untraced)* Switching a
source spectrum from White to Pink noise closes the program: #424 (2026), also reported in
#242 (2020) and #323 (2022). The 2022 report was answered "There is no issue in I-Simpa
1.3.4". Suspects exist in code, but nobody traced this crash to them.

**9. The documentation is stale.** readthedocs was built from a 2022 commit and titled 1.3.4.
`/en/latest/` returns 404, and all 8 internal links checked return 404. A 2025 fix swapping
back "Energetic" and "Random" was never rebuilt onto the live site.

**10. The language picker leaves out English.** The picker lists only fr, pl, pt_BR and
zh_CN, and French is preselected (`IHM/languageSelection.cpp:87-113`). *Critic's correction:*
the "menus half in French with mojibake" complaints come from 2017-2020 issues and were not
found in the v1.4.0 source.

**11. There is no headless or batch mode.** The only command-line input is a project path.
Scripting runs only inside a live GUI, is looked up by label string, and silently returns a
default when a name is misspelt. `job_tool` uses `collections.Sequence`, which Python 3.10
removed, while the zip bundles Python 3.14.

**12. There are modal dialogs everywhere, and typed parameters are not validated.** There
are 40 `ShowModal()` sites, and up to 4 modals chain at first launch. Typos silently become
numbers. Custom C, D and RT windows must be retyped every time.

**13. Fixes do not reach users.** v1.3.4 shipped in 2020-12. v1.4.0 (2026-01) is still
flagged "Preview", with 179 downloads against 1.3.4's 22,773. PR #426, which fixes
surface-receiver corruption on Linux and macOS, has had no reply since 2026-07-07. The
median open-issue age is 9.4 years. Of 12 external issues filed in 2024-2026, 5 got any staff
reply.

## What users say, from 228 issues and 542 comments

61% of the tracker is the maintainers' own to-do list. Of the external complaints, the
largest group is GUI usability (61 issues). The rest cover results and export (24), accuracy
(18), scripting (16), docs (15), install (14), crashes (10), meshing (9) and CAD import (6).
One pattern repeats: **the tool fails and does not say why.**

## Worth copying, not losing

- **The solver contract.** One `config.xml` argument, a working folder with `mesh.cbin` and `tetramesh.mbin`, results written beside them. A dated folder per run makes any run re-executable. `jeninor/sound` (2020) already drove `spps.exe` headless this way.
- **Settings declared once**, with name, default, range and precision. Keep the idea as a real typed schema.
- **Material consistency rules**: α=1 forces diffusion to 0, and τ ≤ α is corrected with a warning. Put them in a validation layer that tells the user.
- **Re-import keeps material assignments** by matching face centres within a tolerance, with unit conversion at import.
- **TetGen debug mode** maps rejected faces back onto the model. Trigger it automatically when meshing fails.
- **Double-click flood-fills a coplanar surface.** It is the right gesture for materials. Make it iterative, not recursive.
- **The inside view:** front faces culled, internal partitions exempt. Outline wireframe instead of every triangle edge.
- **One Animator interface** on a shared timeline for particles, maps and vectors. Copy the abstraction, not the display lists.
- **Versioned result formats with round-trip fixtures**, plus `libsimpa` for headless read and write.
- **Spreadsheet-grade editing:** TSV copy and paste with Excel, fill a row, natural sort of bands. CATT and Odeon material import. Receiver grids, source groups, cutting-plane receivers, A/B difference maps.

## Traps a rebuild must not repeat

- **Do not trust exit codes or streams.** Classify every output line by its content. Fatal lines arrive on stdout.
- **`config.xml` fails silently.** A missing attribute becomes 0. Every source, material and fitting must carry every band, because arrays are sized by XML child count and indexed by band.
- **A missing or unparseable directivity file silently makes the source omnidirectional.** Validate before launch.
- **`.mbin` has no magic number or version, and RSBIN never checks `is_open()`.** Tie each `.mbin` to the `.cbin` it was built from. A failed re-mesh can otherwise leave the old tetrahedra in use.
- **Element ids are per-session counters reused as solver ids and file names.** Use stable ids.
- **Receiver labels become folder names unvalidated**, and a `%` in a label reaches `printf`.
- **Geometry is stored in float32 in a normalised, axis-swapped frame.** Keep world units.
- **On Linux and macOS**, a `memcpy` of 24 bytes into 12 corrupts every surface-receiver face. PR #426 fixes it, unmerged. The `.finfo` header is `unsigned long`, which is not portable.
- **Do not port the GUI's acoustic post-processing verbatim.** The STI bug lives there, and the RT, EDT, C and D routines were not audited.
- **`lib_interface` calls `exit()` on bad input.** Never link it into the GUI process.

## Other versions out there

| Who | What | Alive? |
|---|---|---|
| Upstream `v1.4.0` | First release since 2020, still "Preview"; Flatpak not on Flathub | Slow: last push 2026-01-18 |
| **wbinek/I-Simpa** (AGH Kraków) | The only substantial fork: about 270-300 own commits. A third solver, SPPS-AGH (path tracing, experimental MLT, BRDF reflection), STI and CSV export, a Polish UI, wx 3.2 builds. STI and several fixes went upstream; SPPS-AGH never did. **Same wx GUI.** | Dormant since 2023-12 |
| diegotonetti99/I-Simpa | PR #426, the Linux and macOS surface-receiver fix, plus an ambisonics B-format export branch | Pushed 2026-07-09 |
| Upstream branch `mdf_python` | Python modal and diffusion-equation solvers. Its commit messages record defects left unfixed | Idle since 2024-09 |
| ahmad-abosrea/ISIMPA-STI-tools | Plugins: STI with male weighting, %ALcons, an STI map | New, 2026-09 |
| Kyle-F-Brooks, qmichalski, ArmandDuverger | Script patches, a VTK exporter, a GLL-to-directivity converter | Dormant or new; add-ons only |
| jeninor/sound | Headless SPPS optimisation loop, no GUI. Proof that the "new front end, same solvers" approach works | Dormant since 2020 |
| CHORAS (TU Eindhoven) | **Not a derivative.** A web front end over diffusion and DG solvers. Does not run SPPS or TCR. Its paper cites I-Simpa's local install as a limitation. Cloned at `B:\repos\choras` | Alive |

The other 64 forks carry no work of their own. **No one has built or is maintaining a new
GUI for I-Simpa.**

## On the 2025 accuracy claim

A 2025 Forum Acusticum paper (Dijkman, Hoekstra and Hornikx) found I-Simpa closest to the
measured RT and EDT on BRAS CR2, our S09. Treat that as weak support. The same paper says
none of the tools predict RT consistently within the just-noticeable difference. The
comparators were Pyroomacoustics, dg-acoustics and Image2Reverb, with no commercial
geometrical-acoustics tool. It used 5,000 rays, and the values came through the GUI's
unaudited post-processing. Solver correctness is still arc item 4.

## Not examined

- The solvers' physics.
- The internals of VolumetricMeshRepair, preprocess and TetGen.
- The GUI's RT, EDT, C and D routines.
- Linux, macOS and Flatpak CI.
- Accessibility and keyboard-only use.
- The contents of the maintainer's pending PR #418 (`fix_v1.4`), which partly addresses two of the traps.
- Behaviour on large models.
- Which commit the macOS dmg was built from.
- The 72nd fork (probably a deleted one).
