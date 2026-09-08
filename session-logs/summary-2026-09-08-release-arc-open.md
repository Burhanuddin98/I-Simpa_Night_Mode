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
