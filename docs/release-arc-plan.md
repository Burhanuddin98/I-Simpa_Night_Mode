# THE RELEASE ARC — I-Simpa Night Mode, public-release ready

Opened 2026-09-08 02:06 (Grace, from the ATTuner window; clone stood up at `B:\repos\I-Simpa_Night_Mode`).

## The charter, verbatim

> i wanna make a public-release ready version of i-simpa, the original idea was that it sucked, and that the UI was so bad that it made it a nightmare to use

> the base idea is, lets continue with the work

*(this session, 2026-09-08 02:06 and 02:11)*

**My reading.** The product is the April build: I-Simpa's solvers behind a GUI that is not a nightmare. "Public-release ready" means a stranger can download it, run it, get a correct answer, build it from source, and see under what licence. None of those five holds today.

## Where it stood on 2026-09-08

Receipts in `session-logs/summary-2026-09-08-release-arc-open.md`.

- **The repo** — `Burhanuddin98/I-Simpa_Night_Mode`, public. Eleven commits, all 2026-04-04. Roughly 10k lines of own C++17: ImGui docking + OpenGL 4.6 GUI, ImPlot, ImGuizmo, GLFW, GLM, miniz, all via FetchContent. It shells out to upstream's `spps.exe`, `classicalTheory.exe`, `preprocess.exe` and `tetgen.exe` (`project/solver.cpp:767-802`, `CreateProcessA` at `:1029`). Tests (`tests/test_pipeline.cpp`, 515 lines) cover the data model and file writers only. The last commit reverts a surface receiver that crashed SPPS.
- **The release** — one tag `v0.3.0` carrying four zips v0.3.0 through v0.3.3, 1.8 MB each: the GUI exe, four solver exes, and upstream's Octave/Python experimental cores. One download in total.
- **What the repo does not contain** — the solver source; the four solver fixes (README § Changelog v0.3.1 describes them in prose, made in `src/spps/` and `src/ctr/` of an upstream checkout at `C:\RoomGUI\Michael\I-Simpa` on Zeph, never committed anywhere public, and no fork of upstream exists); a LICENSE file, though the README claims GPLv3; screenshots; CI; any record of which upstream version the GUI's `.cbin`, `.mbin` and config.xml writers were written against.
- **Upstream is alive, not abandoned** — `Universite-Gustave-Eiffel/I-Simpa`, GPL-3.0, 298 stars, 122 open issues, `v1.4.0_snapshot_14_01_2026` released 2026-01-09 with a Windows installer, a macOS dmg and a Flatpak. Its GUI is still wxWidgets, so the charter's premise about the interface holds. The four solver-bug claims are public assertions against a live project and were never verified or filed.
- **The April plan** (`~/.claude/projects/C--RoomGUI/memory/project_isimpa_rewrite_plan.md`, mirrored in BuSha) — Phase 1, correct simulations, complete. Phase 2, UX and reliability, in progress: the solver stdout read blocks, `.proj` ZIP extraction hangs, auto-load landed. Phase 3 never started. `AUDIT_COMPARISON.md` puts the custom GUI at about 60% of upstream's visualisation and 95%+ of its simulation and data model, but its critical-gaps list predates the v0.3.0 changelog, which claims several of them landed. Reconcile before trusting either.

## The shape

Assumed 2026-09-08 on my recommendation, not yet ratified in so many words.

**A standalone GUI repo; solvers built from a pinned upstream tag plus a `patches/` directory; CI on a Windows runner producing the zip.** The alternative, forking upstream and carrying the GUI in-tree the way the Zeph layout did, was presented and not chosen: it carries a 409 MB tree, wxWidgets, and 122 issues we did not write. Under the chosen shape the four solver fixes go upstream as pull requests where they belong, and stay in `patches/` until merged.

Pinned upstream is `v1.4.0_snapshot_14_01_2026`, commit `929a5c8e`, checked out at `B:\repos\I-Simpa-upstream` with `--depth 1`. It is never edited in place; patches are files in this repo.

## Licensing

A release blocker, not a footnote.

- **This repo is GPL-3.0**, forced by shipping upstream's solvers and their source. Add `LICENSE` carrying the GPL-3 text, as upstream's `LICENSE.md` does, and a `THIRD_PARTY_NOTICES.md`.
- **TetGen 1.6.0 is dual-licensed: AGPL-3, or a paid WIAS commercial licence** (`I-Simpa-upstream/src/tetgen/LICENSE`). A free GPL-3 release that ships TetGen's source alongside is compatible under GPLv3 section 13. A closed or commercial Night Mode cannot ship `tetgen.exe` without the WIAS licence. That is a standing constraint on Dockyard's commercial plans.
- Fetched dependencies, to be confirmed from each source tree's own LICENSE at build time: Dear ImGui (MIT), ImPlot (MIT), ImGuizmo (MIT), GLFW (zlib), GLM (MIT), miniz (MIT).

## Work list, in order

| # | item | status |
|---|---|---|
| 1 | Baseline: build the four solvers from the pinned tag and the GUI from a clean clone on Grace; run `test_pipeline`; one headless smoke solve | first run 2026-09-08 02:14 died on a session limit with all nine agents erroring, nothing measured; re-run 05:11 |
| 2 | Recover or re-derive the four solver patches against the pinned tag; derive the physics for each, do not take the README's word | same run |
| 3 | Format compatibility of the GUI's `.cbin`, `.mbin`, config.xml and result readers against upstream 1.4.0 | same run |
| 4 | Physics verification bed per patch: shoebox against Sabine and Eyring analytical RT60 for the Eyring clamp, a partition transmission case, atmospheric absorption over distance against ISO 9613-1, a frequency-dependent encumbrance. Two arms differing in exactly the patch | not started |
| 5 | File the four fixes upstream as issues and pull requests, carrying the bed's numbers | after 4 |
| 6 | `LICENSE`, `THIRD_PARTY_NOTICES.md` | `LICENSE` done 2026-09-08 (GPL-3, copied from upstream's text); notices pending, each dependency's licence to be read off the FetchContent tree once the build populates it |
| 7 | CI: GitHub Actions Windows build of solvers (pinned tag plus `patches/`) and GUI; zip artifact; one tag per version | after 1 |
| 8 | The SPPS crash behind the reverted scene-type surface receiver, commit `35778f0a` | not started |
| 9 | Reconcile `AUDIT_COMPARISON.md` against the v0.3.0 changelog; the surviving gap list becomes the UI backlog | **done 2026-09-08** → `docs/ui-parity-backlog.md`. 14 of 20 priority items have code in the tree, including all seven marked critical; 6 remain open, none of them a release blocker. The audit is bannered as superseded rather than rewritten. Green rows mean code exists, not that it works |
| 10 | April Phase 2 leftovers: the blocking stdout read in `RunSolver`, the `.proj` ZIP extraction hang, room volume for TCR, the receiver inside-mesh test | not started |
| 11 | Screenshots, quickstart, versioning. Windows-only for v1, since the solver launch is Win32 | last |

## Pending verdicts

- The shape above, standalone plus pinned upstream plus patches. Assumed, not ratified.
- Whether a commercial Night Mode is in scope at all, which decides the TetGen question.
