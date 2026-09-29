# Scope ledger

Every dimension of the rebuild's scope, where each one is tracked, and its state. Any new item goes
into the file named here in the same session, never only into a chat or a handoff. Last updated
2026-09-26 04:20.

| Dimension | Where it lives | State |
|---|---|---|
| **Milestones M0-M14** | `docs/rebuild-plan.md` (table) | M0-M7 and M9 gated. Pre-M8 merged (`df7d8e5`; M4 12/12, M5 43/43, M6 39/39, M7 32/32, M9 23/23, parity 26/26). **M10 gated and merged 2026-09-29** (`a67b8db`). **M8a gated and merged 2026-09-29** (`06588a2`): the T30 bed passed 23 of 23 at 12 jobs, and an independent three-lens judge returned PASS (`docs/investigations/2026-09-29-m8a/VERDICT.md`). Two latent gate defects (VERDICT findings 1 and 2) block any reuse of `simpa bed` until fixed: that is M8b's first step (decision row 23). M8b and M11-M14 not started. **M11 planned 2026-09-29** (`docs/investigations/2026-09-29-m11/PLAN.md`): one foundation, then three packages (simulate, dock, project). |
| **Engine work before M8** | This file, below | Open. |
| **M8 decisions** | This file, below | The 7 technical calls were adopted as recommended on 2026-09-29 (decision row 17). The reference is decided, and Burhan's word is final. |
| **Upstream feature parity** (what the screens must hold) | `docs/investigations/2026-09-25-parity-audit/parity-matrix.md` | Upstream has 250 user-facing features. **43 are before v1** (15 absent or deferred, 14 core-only, 14 design-only; 38 small, 5 medium), 85 are v1.x and 55 later. They fold into M10-M13. |
| **Product decisions for Burhan** | The parity matrix (open decisions, raw:348-360); this file, below; M11 PLAN section 10 | 8 open. M11 adds PQ1-PQ6 (PLAN section 10) and PQ7 (the foundation, `docs/investigations/2026-09-29-m11/FOUNDATION.md` F-1: a project never saved and never edited leaves without the save prompt), each with a recommended default that the build follows until Burhan says otherwise. |
| **Deferred work (v1.1)** | `docs/v1.1-backlog.md` | 18 items, closed by M14. Rows 6-18 were added by the M11 plan: M10's minor findings, the core test targets under `<repo>\target`, and the remaining-time estimate. |
| **v1 pieces outside the milestone specs** | This file | Two, both v1 (row 21), neither started. (1) Face regrouping (G19) and grouped receivers on `.proj` import (M37): row 22 gives them their own piece. (2) The Simulate settings editor (C7, C8, C10, C11, C12, C21, C22, C25, C26, C27): M11 PLAN PQ3, due before M12 because C8 and C22 feed M12's playback and multi-source runs. |
| **v2 contenders** | `docs/v1.1-backlog.md`, "v2 contenders" | 14 items (the GPU tracer and 13 community requests, row 16), after M14. |
| **Open physics questions** | This file, below | 4 open. |
| **Why W1G was dropped** | `docs/investigations/2026-09-25-z3-w1g-hunt/z3-verdict.md` | 217 robust false accepts; three mechanisms. |
| **Decisions (who, why, revisit?)** | `docs/decision-log.md` | 22 recorded. |
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

- **EDT display (Burhan): "Show the range always".** EDT is always shown with its guaranteed range; wide ranges are flagged.
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
6. ρ = 10 for energetic lost particles: accept, and measure more in v1.1.
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
