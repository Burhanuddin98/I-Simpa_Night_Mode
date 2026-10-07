# Scope ledger

Every dimension of the rebuild's scope, where each one is tracked, and its state. Any new item goes
into the file named here in the same session, never only into a chat or a handoff. Last updated
2026-09-26 04:20.

| Dimension | Where it lives | State |
|---|---|---|
| **Milestones M0-M14** | `docs/rebuild-plan.md` (table) | M0-M7 and M9 gated. Pre-M8 merged (`df7d8e5`; M4 12/12, M5 43/43, M6 39/39, M7 32/32, M9 23/23, parity 26/26). **M10 gated and merged 2026-09-29** (`a67b8db`). **M8a gated and merged 2026-09-29** (`06588a2`): the T30 bed passed 23 of 23 at 12 jobs, and an independent three-lens judge returned PASS (`docs/investigations/2026-09-29-m8a/VERDICT.md`). Two latent gate defects (VERDICT findings 1 and 2) block any reuse of `simpa bed` until fixed: that is M8b's first step (decision row 23). **M8b step 1 merged 2026-09-30** (`43284a9`): the two gate holes are closed, and the real M8a bed re-judged by the fixed gate is bit-identical (`docs/investigations/2026-09-30-m8b-gate/REJUDGE.md`, decision row 31). **M8b done at product grade and merged 2026-10-03** (`c7bf396`): every metric (EDT, T20, T30, Ts, C50, C80, D50, SPL, G, dB(A), STI) checked at the new-project defaults against independent references within the JND (`docs/investigations/2026-10-02-bed/RESULT.md`; decisions 38-43); research grade is v1.1 (backlog 61). **M12 gated 2026-10-03** (`docs/investigations/2026-10-03-m12/`: m12.ps1 22/22 with M11, M10, M9; workspace 1110; assay SHIP-WITH-FIXES, fixed; decisions 46, 47); M13-M14 not started. **M12 closed 2026-10-05** (decision 57): the full gate green on `b82` at 22:40 (M12, M11, M9; m12-a 2,540 numbers, 0 mismatches), pushed as `rebuild` 4ee9695; decision 56 (lost particles reported, warning from 0.3 %, refused from 1 %) and backlog 82/84 in it. The advisor (backlog 80) is M12b, before M13; backlog 87 (the canvas count) stays in M14. **M12b built 2026-10-05 on `b80`, its e2e not yet run** (the window was in use): the core advisor, report version 17's `advice`, `simpa advise`, the Simulate step's "Run quality" and the Results step's "Why values are missing" with Apply; unit tests green; `b80.advisor` (4 ids) wired into `tools/gates/m12.ps1` for the next gate. **M11 planned 2026-09-29** (`docs/investigations/2026-09-29-m11/PLAN.md`): one foundation, then three packages (simulate, dock, project). **M11 gated and merged 2026-09-30** (`bbe33b9`): M11 31/31, M10 and M9 pass at `c9edf1e` (`docs/investigations/2026-09-29-m11/GATE.md`), after two adversarial reviews; the second found 2 blockers and 8 majors, all fixed with failing-first tests (decision row 29). |
| **Engine work before M8** | This file, below | Open. |
| **M8 decisions** | This file, below | The 7 technical calls were adopted as recommended on 2026-09-29 (decision row 17). The reference is decided, and Burhan's word is final. |
| **Upstream feature parity** (what the screens must hold) | `docs/investigations/2026-09-25-parity-audit/parity-matrix.md` | Upstream has 250 user-facing features. **43 are before v1** (15 absent or deferred, 14 core-only, 14 design-only; 38 small, 5 medium), 85 are v1.x and 55 later. They fold into M10-M13. |
| **Product decisions for Burhan** | The parity matrix (open decisions, raw:348-360); this file, below; M11 PLAN section 10 | 8 open. M11 adds PQ1-PQ6 (PLAN section 10) and PQ7 (the foundation, `docs/investigations/2026-09-29-m11/FOUNDATION.md` F-1: a project never saved and never edited leaves without the save prompt), each with a recommended default that the build follows until Burhan says otherwise (decision row 25); the packages' own user-visible calls are decision row 26, with their defaults built. |
| **Deferred work (v1.1)** | `docs/v1.1-backlog.md` | 22 items, closed by M14. Rows 6-18 were added by the M11 plan: M10's minor findings, the core test targets under `<repo>\target`, and the remaining-time estimate. Rows 19-22 by M11's integration: the Law column's keyboard path, per-band law editing, the import dialog's key blocking, and a semi-diffuse material blocking Run before meshing. |
| **v1 pieces outside the milestone specs** | This file | Two, both v1 (row 21); both built 2026-10-03. (1) Face regrouping (G19) and grouped receivers on `.proj` import (M37): **built 2026-10-03** on `row15-groups` (`docs/investigations/2026-10-03-row15/`: PLAN, BUILD, ASSAY SHIP; m11.ps1 32/32, e2e 37/37, 1086 tests; decision 45). (2) The Simulate settings editor (C7, C8, C10, C11, C12, C21, C22, C25, C26, C27): **built 2026-10-03** on `pq3-settings` (`docs/investigations/2026-10-03-pq3/`: PLAN, BUILD, ASSAY SHIP-WITH-FIXES, fixed `896f745`; m11.ps1 32/32, 1070 tests; decision 44). |
| **A blank geometry to results (C1, v1, 2026-10-07)** | `docs/investigations/2026-10-07-blank-geometry/` (SPEC, BED, REPORT); decision 73 | **Built on `blank` 2026-10-07**, not merged. A raw groupless OBJ imports as one group named for the file; groups are carved, renamed (F2), merged and moved into from the scene list and the view; a source's power, spectrum and directivity and a material's transmission per band are edited; + Source sits off the room's centre. `m13.blank` 9/9 on the m11 harness (`-Only e2e -Spec m13.blank`), CPU and GPU runs OK. |
| **Auralization (v1, added 2026-10-06)** | `docs/investigations/2026-10-06-auralization/PLAN.md`; decision 67 | Burhan's order of 16:51 made it v1. Planned, not built: core synthesis from the per-band energy echogram, a CLI command, Play and Save on the Results step, then convolution with an anechoic WAV. Awaits his word on the plan's three open choices. |
| **The solver's memory (v1, 2026-10-06)** | `docs/investigations/2026-10-06-third-octave-bug/` (MEMORY, COMPACT, SPARSE, BED-0001, BED-0002); decisions 65, 66; `patches/` | **Fixed and committed on `ui`.** Patch 0001 (the sound-map time bin, a user option) and patch 0002 (sparse per-cell series: memory follows the data, output bit for bit the old solver's). The 0.1 m CR4 plane runs at 3.07 GB where it aborted at 30 GB. Open: backlog 88 (band batching, now a fallback), 91 (scene-receiver arm), 93 (the Global file), 94 (the forecast's one-run calibration). |
| **v2 contenders** | `docs/v1.1-backlog.md`, "v2 contenders" | 14 items (the GPU tracer and 13 community requests, row 16), after M14. |
| **Open physics questions** | This file, below | 4 open. |
| **Why W1G was dropped** | `docs/investigations/2026-09-25-z3-w1g-hunt/z3-verdict.md` | 217 robust false accepts; three mechanisms. |
| **Decisions (who, why, revisit?)** | `docs/decision-log.md` | 27 recorded (rows 24-27: M11's plan calls, its product questions with the defaults built, and its package and integration calls). |
| **The running log** | `session-logs/HANDOFF-2026-09-23.md` | Burhan's words verbatim, decisions, restart order. |

## Engine work before M8

1. **The early-parameter band** (EDT, Ts, C50, C80, D50): model-free, with a guaranteed range, replacing the
   early check and W1G.
   - Designed and attacked in Python (`target/agents/edt-band/`, workflow `wf_83f875af-399`). The adversary
     found 0 escapes, and 217 of 217 Z3 cases are inside the band.
   - Then comes the fix round, the evaluation (which also re-checks the upstream EDT port) and the critic.
   - The design and its evidence are committed to `docs/investigations/` when the workflow ends.
   - **Workflow done (07:22)**, committed at `docs/investigations/2026-09-26-edt-band/`: 0 wrong among 24,866 accepted EDTs.
     The critic's gates before Rust: (1) the f32 air-rate fix (**closed 2026-09-27**, GATE1.md), (2) production tail and eps inputs (**open**: x1000 tail factor uncalibrated, refuses 32.7 % of real EDTs; decision rows 14-15), (3) noise and band evaluated together,
     (4) a Z4 golden corpus (**open**: 167 cases frozen, Z3 and grid rows not yet), (5) a measured Rust benchmark.
   - Then the Rust build, with Z4 parity against the Python version.
2. **The seed-batch noise rule** (SB-1..SB-9, k = 10), from the accepted follow-up spec.
3. **The 2 ms ceilings for C50, C80 and D50** (D5). At 10 ms they can be off by up to 0.13 dB today.
4. **Refuse single-run energetic T20/T30 until R4** (D7).
5. **Check the solver executables against `solvers/manifest.json` on every run.** The main checkout's
   tetgen.exe is stale; the one in `t3-proj` is correct. **Built for the app in the M11 foundation**
   (C7, `RunOptions::verify`: stage `solvers`, `solver_unverified`, recorded in `run.json`); the CLI
   and the bed run without it, as before.

## Decided 2026-09-27

- **EDT display (Burhan): "Show the range always".** EDT is always shown with its guaranteed range; wide ranges are flagged, and one that reaches below zero is refused (`range_below_zero`, decision-log row 48).
- **M8 split (Jarvis's call, Burhan may overrule):** M8a T30 first, M8b EDT once the band is in Rust.
- **Default step:** pending the step-cost measurement (Burhan: "needs more research").

## Decided 2026-09-29

- **M8b's scope (Burhan, row 18):** EDT, C50, C80, D50 and SPL, each with a chosen reference and a gate. None is 'reported only'.
- **STI is in v1 (row 19),** tested like the core parameters. **Every other parameter ships labelled 'not yet validated' (row 20).**
- **v1's scope is settled (row 21):** the about 102 planned features, plus T20, Ts, dB(A), G and STI, all tested. The 85 v1.x features become point releases in the order users ask. LF, LFC, LG, ST early and platform work stay later.
- **M8b's scope now reads:** EDT, T20, Ts, C50, C80, D50, SPL and STI, each against a chosen reference. dB(A) and G get unit tests on top of the tested SPL.

## M8 decisions (the critic's, 2026-09-25 23:12; recommendations of 2026-09-26 00:34)

1. EDT in M8: build the new band first. *Recommended.*
2. Seeds refused for noise: judge every seed's value, refused or not, with the seed spread as uncertainty.
3. The noise model in M8's cells: let the bed measure it (10 seeds).
4. Particle counts shown to users are too high: accept for now, and move the fix to v1.1.
5. The solver build and the manifest check: use the verified build, and check on every run (engine item 5).
6. Lost particles: reported, not bounded (decision 56, 2026-10-05, replacing "ρ = 10: accept, and measure more in v1.1"): every quantity of a band's series carries a warning from 0.3 % lost and is refused `lost_particles` from 1 %; the ρ = 10 bound and `ENERGETIC_LOST_ENERGY_RATIO` are gone (`docs/investigations/2026-10-05-b82-b84/FINDINGS.md`).
7. Reference precision: keep 32×, and accept 0.6 % through the room-energy argument (v1.1 item 5).

## Product decisions for Burhan (from the parity audit)

3DS import · the average-model remesh (VolumetricMeshRepair) · the preprocess default · which
parameters ship · embedded Python · the updater · macOS and Linux · the project archive.

## Open physics questions

1. Random and energetic T30 differ by +1.35 % on tutorial 1 (pooled z 2.66). This must be settled before either
   mode's T30 is shown (`target/agents/t30-edt-diagnosis/resolution.md`).
2. The noise model has no margin in M8's α 0.4 cells, and the α 0.05-0.2 cells were never run at 1 ms.
3. EDT bands are wide at coarse steps (a median of about ±8 % on the Z3 cases at 0.5-2 ms). The candidate levers
   are the direct sound's known energy (untested) and a finer default step (Burhan's decision after the
   evaluation).
4. Burhan's EDT tolerance for users (2.5 % suggested) and for M8 (strict).
