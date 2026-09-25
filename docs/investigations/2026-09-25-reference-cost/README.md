# What Kuttruff's reference costs `simpa results` (2026-09-25)

Owed since piece A of the pre-M8 work fixed the transport's precision (`FreePathSettings::STANDARD`,
16 × 4096 rays of 1 + 32 + 512 paths, 32 times the first settings): the wall time of `simpa results
--json`, release build, on the corrected Elmia hall, and whether the reference should be computed
only where `lambert_walls` is true. It adds about 5 s there, for bands whose walls it does not
describe, so it is now computed only for bands with Lambert walls (`results::reference`), and the
transport only when at least one computed band has them.

Grace (28 threads), nothing else running, three calls each, alternating (`timing.txt`):

| Run | Before (every band) | After (Lambert bands only) |
|---|---|---|
| `rooms/elmia_corrected.simpa` as committed, SPPS energetic, 1,000,000 particles, 6 bands, 1.5 s in 5 ms steps (a run made for this, 1,255 s of SPPS, in `target/agents/pm8-safety-scratch/elmia-runs/`) | 6.33, 6.34, 6.17 s | 1.10, 1.03, 0.78 s |
| `rooms/elmia_loss_gate.simpa` (M6's gate run of 2026-09-25 00:44, 100,000 particles, seed 1) | 5.92, 6.03 s | 0.73, 0.78 s |
| the Seat box with Lambert walls at α 0.4, energetic, 150,000 particles (where the reference applies; `cli_results.rs`' values test builds the same room) | 1.03, 1.02 s | 1.20, 1.12 s |

- Before, the hall's report carried `γ²` 0.55499 ± 0.00024 and a Kuttruff time in every band,
  although no band has Lambert walls (its reflectors are specular and the rest scatter 0.15 to
  0.3). After, every band's `kuttruff_s` is refused `params_reference_not_applicable`,
  `free_paths` is `null`, and `eyring_s` is unchanged.
- Builds: before `simpa.exe` sha256 `0d913886…` (the `pre-m8` tree with the C and D limits
  aligned, whose reference is `812853c`'s), after `282d8b55…` (the same tree with this change).
