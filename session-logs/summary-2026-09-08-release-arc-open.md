# 2026-09-08 — THE RELEASE ARC opened; repo stood up on Grace

Worked from the ATTuner window (Grace, `B:\repos\ATTuner`) because the question arrived there. Everything below targets `B:\repos\I-Simpa_Night_Mode` by absolute path. Nothing pushed.

## How this started

Michael asked whether Burhan had a repository holding a fully custom version of I-SIMPA that he meant to release publicly. Local memory said only that a rewrite was *planned* and pointed at a folder on Zeph that no longer exists. The public GitHub API settled it: the repo exists and is public.

## What was found

- **`Burhanuddin98/I-Simpa_Night_Mode`** — public, created and last pushed 2026-04-04, 11 commits, MIT-free C++17, described as *"A custom build of I-Simpa built by Dockyard Acoustics Sound Technology."*
- **One release**, tag `v0.3.0`, carrying four Windows zips v0.3.0 through v0.3.3, 1.8 MB each. Contents: `ISimpa_NewGUI.exe` plus `solvers/` holding `spps.exe`, `classicalTheory.exe`, `preprocess.exe`, `tetgen.exe` and upstream's Octave and Python experimental cores. One download in total across all four.
- **The GUI is genuinely from scratch** — ImGui docking, OpenGL 4.6, ImPlot, ImGuizmo, GLFW, GLM, all via FetchContent; miniz and a glad loader vendored. Roughly 10k lines of own code. Largest units are `project/solver.cpp` at 2217 lines, `app/app.cpp` at 1925, `viewport/viewport.cpp` at 1640, `project/result_parser.cpp` at 942.
- **The solvers are external processes**, not linked code. `FindSolverExe` at `project/solver.cpp:767-802` maps `spps` to `spps.exe` and `tcr` to `classicalTheory.exe`, searching `exeDir\solvers` first; `RunSolver` launches via `CreateProcessA` at `:1029` and parses stdout lines beginning with `#` for progress. Mesh generation shells `preprocess.exe` then `tetgen.exe` with `-pq1.5 -A -n -Y -T1e-7`.
- **Tests exist but are narrow** — `tests/test_pipeline.cpp`, 515 lines, over 40 cases covering the data model, material assignment, config persistence, mesh I/O and XML round-trip, with GL and GPU stubs for headless runs. It does not test rendering, result visualisation, or an actual solve.
- **The last commit reverts a feature** — `35778f0a` backs out a scene-type surface receiver because it crashed SPPS.
- **No LICENSE file** despite a GPLv3 badge and claim in the README. **No CI. No screenshots** (the README says "coming soon"). **No record of which upstream version** the file writers were built against.

## The gap that defines the arc

The four solver fixes claimed in the README changelog were made in `src/spps/` and `src/ctr/` of an upstream checkout at `C:\RoomGUI\Michael\I-Simpa` on Zeph. That path does not exist on Grace, Burhan holds no fork of upstream, and the commit that announces the fixes (`5d42532`, *"v0.3.1: Fix 4 upstream solver bugs + add prebuilt release"*) touches **README.md only, 11 lines added**. The fixes exist as prose and inside a shipped binary. Nobody but Burhan can build this today.

## What upstream actually is

Not the abandoned project the premise assumed. `Ifsttar/I-Simpa` now redirects to **`Universite-Gustave-Eiffel/I-Simpa`**: GPL-3.0, 298 stars, 71 forks, 122 open issues, last pushed 2026-01-18. Release `v1.4.0_snapshot_14_01_2026`, published 2026-01-09, ships a Windows installer, a macOS dmg and x64 plus aarch64 Flatpaks. Its GUI is still wxWidgets, so the charter's complaint about the interface stands. But the four bug claims are public assertions against a live, maintained project, made against an unknown older version, never verified against an analytical reference and never filed upstream.

## Licensing, checked

- Upstream is GPL-3.0 (`LICENSE.md`, 674 lines, the standard text). Night Mode ships its solvers, so Night Mode is GPL-3.0. **`LICENSE` added this session**, copied from upstream's text.
- **TetGen 1.6.0, bundled at `src/tetgen/`, is dual-licensed AGPL-3 or a paid WIAS commercial licence.** Its header states *"It may be copied, modified, and redistributed for non-commercial use."* A free GPL-3 release shipping the source alongside is compatible. **A commercial Dockyard product cannot ship `tetgen.exe` without buying the WIAS licence.** This is the one licensing fact that could bite later and it is now recorded in `CLAUDE.md`.

## Done this session

- Clone at `B:\repos\I-Simpa_Night_Mode`; pinned upstream at `B:\repos\I-Simpa-upstream`, tag `v1.4.0_snapshot_14_01_2026`, commit `929a5c8`, `--depth 1`, 409 MB.
- `docs/release-arc-plan.md` — the charter verbatim, the state with receipts, the chosen shape, the licensing constraints, an eleven-item work list, two pending verdicts.
- `CLAUDE.md` — lean, pointing at the arc plan rather than restating it.
- `LICENSE` — GPL-3 text.

## Running at close of this entry

Workflow `wf_534c7507-ca4`: recover the April patches from the transcript archive, locate the four bug sites in the pinned tag and derive each fix's physics independently, check the GUI's `.cbin` / `.mbin` / config.xml / result formats against upstream 1.4.0, inventory the toolchain, then build the four solvers and the GUI on Grace, run `test_pipeline`, attempt a headless smoke solve, and adversarially verify every claim against artifacts on disk.

An earlier launch of the same workflow at 02:14 died with all nine agents erroring on a session limit. Nothing was measured in that run; no result from it is quoted anywhere.

## Next

Items 4 through 11 of the arc plan, gated on what the running workflow reports. The first real decision waiting on Burhan is whether a commercial Night Mode is in scope, because that alone decides the TetGen question.

---

## 06:04 — RETRACTION: the solve is broken and the GUI cannot see it

Chasing a better particle animation, I ran `spps.exe` directly instead of through the GUI, and
PowerShell surfaced its **stderr**:

```
Warning 599982 particles has been in error on 600000 particles.
The computation result may be wrong, please check the particles statitics file for more details.
```

99.997% of particles fail, and the solver still exits 0.

**Reproduced at the GUI's own settings.** I had raised duration 2 s to 5 s and particles 100k to
200k first, so the obvious suspicion was my change. Reverting all three values to exactly what the
GUI wrote and re-running gave the same result at the smaller scale. The defect is pre-existing and
was present in the run reported earlier this session as clean.

**Why nobody noticed.** `RunSolver` reads only stdout through its pipe. The warning is on stderr.
The GUI printed `[Solver] Completed successfully` and cheerfully loaded the results.

### Retracted

Every acoustic figure measured tonight, including the ones quoted to Michael: R1 46.9 dB / RT60
1.03 s, R2 34.7 dB, Receiver 3 28.5 dB, and R1's EDT 0.18 s, C80 15.7 dB, D50 97%, Ts 15 ms. They
were computed from a run in which essentially every particle failed. They are not evidence of
anything and must not be quoted.

What still stands, because it does not depend on the solve being physically right: the GUI builds
from a clean checkout (61/61, exit 0), the four solvers build from the pinned tag and run, the
test suite links and runs again (17/19), and the whole chain executes end to end producing files
the GUI parses and renders. The **plumbing** is proven. The **physics** is not.

### Also surfaced, same output

```
Xml Property disable_absatmo_computation doesn't exist !
Xml Property absatmo doesn't exist !
```

The GUI's `WriteConfigXML` omits two properties this solver version reads. That is a
format-compatibility gap against upstream 1.4.0, and atmospheric absorption is precisely the
subject of the first of the four unverified solver-fix claims.

### Prime suspect, unproven at time of writing

The tetrahedral neighbour fallback in `ConvertTetGenToMbin`. The GUI logged 6294 tetrahedra with
**288 boundary faces**, against a surface mesh of 1086 faces, having "matched 1475 tet faces".
Those three numbers do not reconcile. The April plan's bug #2 is exactly this failure mode:
neighbours missing means particles cannot traverse the mesh. The fix for it may be incomplete, or
may not survive against upstream 1.4.0. A `sentinel` seat was dispatched to root-cause it.

### The lesson worth keeping

A green "Completed successfully" meant only that the process exited. The gate never read the
channel the bad news arrives on. `feedback_green_gate_is_not_truth`, in a new repo.

### 06:22 — ROOT CAUSE PROVEN. Two hypotheses refuted by test, one chain confirmed end to end

**Refuted, mine:** the tetrahedral neighbour fallback. **Refuted, the reviewer's:** TetGen's `-Y`
flag. Dropping `-Y` only lowers the skipped-facet count from 535 to 324; under upstream's own
flags, `-pq1.5 -A -n`, it is still 325.

**The chain, each link with a receipt:**

| step | what happens | receipt |
|---|---|---|
| 1 | The raw Elmia PLY is self-intersecting as upstream ships it | converted independently from the PLY, TetGen skips 322 facets as polygons, 312 fan-triangulated; 7 duplicate vertex positions among 955 |
| 2 | TetGen exits 3, *"Program stopped"*, leaves partial output | `tetgen.log`: *"!!! 535 input triangles are skipped due to self-intersections"* |
| 3 | The Run button's pipeline downgrades that to a warning and continues | `solver.cpp` ~1229 site: exit≠0 with files present → *"TetGen returned warnings … continuing"*; caller `app.cpp:195` |
| 4 | Mesh converter builds a mesh of the wrong domain | 6294 tets, 288 boundary faces vs 1086 surface faces |
| 5 | SPPS destroys particles at faces with no material and no neighbour | `CalculationCore.cpp:365-372`; 599,982 of 600,000 lost; reflection order tops out at 1–4 |
| 6 | The warning is on stderr; the GUI reads stdout only | `RunSolver` pipe; classifier wants a leading `!`, SPPS writes *"Warning …"* |

**How upstream meshes the same hall.** Its tutorial project carries the hall *after* scene
correction: 3926 nodes, 7860 facets, eleven times Night Mode's poly, meshing clean with our TetGen
at 188,410 tetrahedra and zero skipped. Correction is ON in that project. Night Mode's
`RunMeshGeneration` (`app.cpp:483`) does call `preprocess.exe`, but on the `.cbin`, which the tool
cannot read (its importer is `ImportPOLY`), *after* the poly was already written from memory, and
without reading the result back. It rewrote the cbin as 156 bytes. Handed a poly in the syntax it
expects, `1 0 marker` per facet rather than a lone `1`, it merged the 7 duplicates, split 594
triangles, then oscillated between two points 2 cm apart on the ceiling plane, gave up at its
100-round ceiling, printed *"Mesh reparation has been aborted"*, exited 0, and wrote nothing.

**Positive control, passed.** The default box room: 8 vertices, 12 faces, 9 nodes, 12 tetrahedra,
all 12 tet faces matched. SPPS runs with **no particle-error warning**. So `WriteMeshBinary`,
`ConvertTetGenToMbin` and the SPPS integration are sound on a valid mesh. The defect is exactly:
no working repair path, and four silent failures in a row hiding it.

**What the fix looks like.** Repair before export, as upstream does. A real repair, since upstream's
standalone tool cannot do this hall. And no more swallowing: TetGen exit 3 is a failure, SPPS
stderr is read, the particle-loss ratio is surfaced. Arc plan item 12.

Two smaller things seen on the way: the solver rejects `disable_absatmo_computation` and
`absatmo`, which the GUI never writes; and SPPS creates duplicated receiver directories with a
trailing `0` (`R1 (near)0`), not chased.
