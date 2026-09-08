# I-Simpa Night Mode — Claude Code instructions

**What this is.** A from-scratch ImGui and OpenGL 4.6 GUI over I-Simpa's room-acoustics solvers, SPPS particle tracing and TCR classical theory. Windows-only, GPL-3.0. Upstream is `Universite-Gustave-Eiffel/I-Simpa`, alive, with a wxWidgets GUI. We ship their solvers, built from a pinned tag plus our `patches/`, behind our own GUI.

**Live arc:** THE RELEASE ARC, `docs/release-arc-plan.md`. Read it first. Its work-list table is the single source of what is live and what is done. Do not restate findings here.

## Layout

`app/` main loop and UI · `viewport/` 3D · `panels/` results, materials, outliner, properties, console · `project/` data model, `solver.cpp` for mesh export, TetGen, config.xml and solver launch, `result_parser.cpp` · `mesh/` loaders and miniz · `commands/` palette · `tests/test_pipeline.cpp` · `docs/` · `session-logs/`.

## Rules

- **The upstream checkout is `B:\repos\I-Simpa-upstream`**, tag `v1.4.0_snapshot_14_01_2026`, cloned `--depth 1`. Never edit it in place. A fix to solver code is a file in `patches/` here and a pull request upstream.
- **Build:** `cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release` inside the Visual Studio 2022 x64 environment, located the same way Attuner's `plugin/build.ps1` does it. Solver executables are expected in `solvers/` beside the GUI executable, per `FindSolverExe` in `project/solver.cpp`.
- **A physics claim about a solver carries a bed:** two arms differing in exactly the patch, measured against an analytical reference. The README's prose is not a receipt.
- **TetGen is dual-licensed, AGPL-3 or a paid WIAS licence.** A free GPL release is fine. A closed one is not, without that licence.
- **Do not claim** the four solver fixes are correct, or that upstream is wrong, in the README, the UI or any marketing copy until item 4 of the arc plan has run.
- Commit conventions follow `~/.claude/CLAUDE.md`: no Claude attribution, messages focused on the why, and a session summary in `session-logs/` before the final push.

## Picking this up

Live handoff: `session-logs/HANDOFF-2026-09-08.md`. Read it before the arc plan — it names
what is proven, what is retracted, and the traps already paid for.

**Nothing is pushed.** `main` sits ahead of `origin/main`; the remote is still the April
release. Pushing is Michael's call and has not been asked.

**Develop against `testdata/elmia_corrected.ply`**, not against upstream's raw `elmia.ply`.
The raw hall self-intersects and produces a solve in which ~99.997% of particles die while
every gate reports success (arc item 12). Regenerate the corrected mesh with
`tools/extract_upstream_scene.py`; pass the original layered `.ply` as the third argument or
the whole hall gets one material.

**Drive it headless** with `--auto --load <ply> --load-results <dir> --play-particles [band]`,
or `--run-spps --wait <s> --quit` to solve. Always read the solver's **stderr**: the GUI does
not, which is how the broken solve went unnoticed for five months.
