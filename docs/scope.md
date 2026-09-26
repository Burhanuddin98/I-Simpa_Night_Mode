# Scope ledger

Every dimension of the rebuild's scope, where each one is tracked, and its state. Any new item goes
into the file named here in the same session, never only into a chat or a handoff. Last updated
2026-09-26 04:20.

| Dimension | Where it lives | State |
|---|---|---|
| **Milestones M0-M14** | `docs/rebuild-plan.md` (table) | M0-M7 and M9 gated. Pre-M8 merged (`df7d8e5`; M4 12/12, M5 43/43, M6 39/39, M7 32/32, M9 23/23, parity 26/26). M8, M10-M13 and M14 not started. |
| **Engine work before M8** | This file, below | Open. |
| **M8 decisions** | This file, below | 7 technical calls open, each with a recommendation. The reference is decided, and Burhan's word is final. |
| **Upstream feature parity** (what the screens must hold) | `docs/investigations/2026-09-25-parity-audit/parity-matrix.md` | Upstream has 250 user-facing features. **43 are before v1** (15 absent or deferred, 14 core-only, 14 design-only; 38 small, 5 medium), 85 are v1.x and 55 later. They fold into M10-M13. |
| **Product decisions for Burhan** | The parity matrix (open decisions, raw:348-360); this file, below | 8 open. |
| **Deferred work (v1.1)** | `docs/v1.1-backlog.md` | 5 items, closed by M14. |
| **v2 contenders** | `docs/v1.1-backlog.md`, "v2 contenders" | 1 item (the GPU particle tracer), after M14. |
| **Open physics questions** | This file, below | 4 open. |
| **Why W1G was dropped** | `docs/investigations/2026-09-25-z3-w1g-hunt/z3-verdict.md` | 217 robust false accepts; three mechanisms. |
| **The running log** | `session-logs/HANDOFF-2026-09-23.md` | Burhan's words verbatim, decisions, restart order. |

## Engine work before M8

1. **The early-parameter band** (EDT, Ts, C50, C80, D50): model-free, with a guaranteed range, replacing the
   early check and W1G.
   - Designed and attacked in Python (`target/agents/edt-band/`, workflow `wf_83f875af-399`). The adversary
     found 0 escapes, and 217 of 217 Z3 cases are inside the band.
   - Then comes the fix round, the evaluation (which also re-checks the upstream EDT port) and the critic.
   - The design and its evidence are committed to `docs/investigations/` when the workflow ends.
   - Then the Rust build, with Z4 parity against the Python version.
2. **The seed-batch noise rule** (SB-1..SB-9, k = 10), from the accepted follow-up spec.
3. **The 2 ms ceilings for C50, C80 and D50** (D5). At 10 ms they can be off by up to 0.13 dB today.
4. **Refuse single-run energetic T20/T30 until R4** (D7).
5. **Check the solver executables against `solvers/manifest.json` on every run.** The main checkout's
   tetgen.exe is stale; the one in `t3-proj` is correct.

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
