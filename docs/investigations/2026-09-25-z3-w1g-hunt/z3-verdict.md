# Z3 verdict: W1G fails

For Burhan, 2026-09-25, 23:05. This is the judge's report on the three Z3 hunters. It is read-only on the repo. No solver was run. Everything below is Python on noise-free echograms, and everything is in `target/agents/z3-hunt/judge/`.

## The answer

**W1G does not survive Z3.** The pass rule, fixed in the spec before any hunting, was: no accepted value beyond L, and one failure sends W1G back. I reproduced every claim independently. **217 claims hold up as false accepts, and 186 of them need no source delay.**

All ratios below are conservative: each is the least ratio over two generators, three fine steps and an EDT fit bottom of −10 ± 0.01 dB.

| Where | Worst confirmed error |
|---|---|
| Anywhere | **EDT 25.2×L** (0.1057 s accepted, true 0.1214 s, −12.9 %) |
| Ts, anywhere | 20.3×L (1.195 ms accepted, true 1.331 ms) |
| At the 2 ms preset | EDT 23.1×L; Ts 20.3×L |
| At 1 ms and finer | EDT 14.5×L (1.0 ms) |
| Ordinary rooms (every α ≤ 0.7), EDT | 4.0×L at 1.98 ms (1 kHz); 3.4×L at 2 ms; 3.4×L at 0.96 ms (uniform α 0.7) |
| Ordinary rooms, Ts | 2.0×L at 2 ms (tutorial-1 materials, 20 kHz, 1.2 m from the source) |

- No step from 0.5 to 2 ms is safe. EDT still fails at 0.5 ms: up to 3.75×L at 20 kHz, and 1.1×L at 250 Hz.
- The advice "use 1 ms in dead rooms" does not rescue it.
- **The guards are not what fails.** The confirmed cases pass G3b with at least 6 dB to spare. The failure is in what the bracket assumes.

## What I did

- **The run W1G reads (generator A).** I wrote a new image-source generator for a specular box with SPPS's ball receiver:
  - Each coarse step is integrated exactly.
  - SPPS's air is applied once per step, as p^(n+1). I checked this in upstream `CalculationCore.cpp:57` and `base_core_configuration.cpp:115`.
  - Values are stored as f32.
  - A source delay is emitted at a whole step, as in `sppsNantes.cpp:91`.
  - Images go to 80 dB below the slowest decay, up to 15 s (24 s in the convergence rebuilds). The fitted tail is never more than 3·10⁻⁹ of the energy.
  - It agrees with GF3's `ism.py` to 1e-6 of the peak per bin, and W1G's values agree to 1e-8.
- **Truth A.** The ideal reading at 0.02, 0.01 and 0.005 ms, with continuous air. EDT is also read with the fit bottom at −9.99 and −10.01 dB. Ts is exact per image.
- **Truth B (a different generator).** `skeptic-twostep/synth.py`, read at 0.01 and 0.005 ms, or at 0.02 and 0.01 ms when R > 0.6 m.
- **The rule.** `spec/composite.evaluate`, called exactly as `harness.py` calls W1G.
- **Confirmed** means all of these hold:
  - W1G accepts the value on my run;
  - the value is beyond L of both truths at every fine step;
  - the receiver ball is clear of the walls.
- **Convergence.** I rebuilt 44 claims (the headline, borderline and rejected ones) with images 1.6× longer and a pruning floor 100× lower. Values and truths moved by about 1e-8.
- **Why a long image set matters.** The hunters fitted tails to short image sets. In a live corridor at 4 kHz, a tail fitted from 0.6 s moved the EDT truth by 2.6×L.

## Per hunter

| Hunter | Claims | Confirmed | Robust | Not confirmed, and why |
|---|---|---|---|---|
| Geometry | 148 | 126 | 121 | See the list below. |
| Acoustics | 59 | 59 | 52 | 7 ill-conditioned, including its headline 29.1×L (see below). |
| Discretisation | 45 distinct | 45 | 44 | 1 ill-conditioned. 31 of the 44 robust cases are delayed-source cases (M3). |

**Geometry, not confirmed (22):**
- 14 have a receiver ball that is not clear of a wall. The image model does not clip the ball there, so it proves nothing.
- 4 are at 125 Hz, in C8 and variants of C7. A converged echogram gives 0.47–0.93×L. C7's representative still holds at 2.16×L.
- 2 are C30 Ts at 1 kHz: 0.99×L once the long flutter tail is included.
- 1 is a C3 variant at 1.64 ms: truth B gives 0.40×L, and the case is ill-conditioned.
- 1 is a C31 variant: truth B gives 0.999×L.

**Geometry, confirmed but ill-conditioned (5):** C9, C37, C38, and two C0/C1 variants at 1.8 ms.

**The acoustics headline.** Its 29.1×L case (C0) is ill-conditioned: with the fit bottom at −10.01 dB, the truth comes within 0.44×L. Its robust worst is the dead_big case at 25.2×L, which is the overall worst above.

**Hunter values differ from mine in 25 confirmed claims,** by 0.1 % to 19 %. Their echograms were shorter. The verdict changes only where noted above.

## What each case defeats

| Mechanism | Robust cases | Worst | What it defeats |
|---|---|---|---|
| **M1: the curve after the window is not log-linear** | 142 (125 EDT, 17 Ts), plus 20 where the part after the window alone is under L and the bracket's own width adds the rest | EDT 25.2×L, Ts 20.3×L | The assumption spec 1.4 names ("the log-linear curve after the window"). |
| **M2: SPPS applies air once per step** | 24 (20 Ts, 4 EDT) | EDT 5.2×L, Ts 2.4×L | Nothing in W1G. The bracket bounds where energy sits inside a bin, not how much energy the bin holds. |
| **M3: delayed source** | 31 (18 Ts, 13 EDT) | Ts 10.7×L, EDT 5.0×L; 8.2×L at R 0.31 m | The spec's text only, not the Python mirror. |

**M1.** Strong discrete reflections arrive after the window: grazing floor and ceiling images, flutter, one hard surface in a dead room. They put steps in the Schroeder curve that a log-linear piece between bin edges does not follow.
- The test: take the curve that is true up to the window's end, then log-linear between the true bin-edge levels. That reading lies inside W1G's bracket in 162 of 162 cases, while the truth lies outside.
- So the lump and window bounds hold, and the error comes after the window.
- It happens in extreme rooms (near-anechoic surfaces with one hard surface: 25×L) and in ordinary ones:
  - a 27×34×8.6 m hall with tutorial-1 materials, 10 kHz, 2 ms: 2.65×L;
  - a room at uniform α 0.7, 0.96 ms: 3.4×L;
  - a 2.8×12.7×3.7 m corridor, 4 kHz, 2 ms: 1.28×L.

**M2.**
- **How it works.** A bin carries p^(n+1), where the converged echogram carries exp(−m r). At 20 kHz and 2 ms, p = 0.92. So the direct-to-reverberant ratio in the run is off by several per cent.
- **The test.** In all 24 cases, the same run with continuous air is within L or refused.
- **Tutorial-1 materials, 1.2 m from the source, 20 kHz, 2 ms:** Ts 2.04×L, against 0.36×L with continuous air.
- **A 20×32×3 m room at α 0.301, 6.3 kHz, 2 ms:** EDT 3.37×L, against 0.10×L with continuous air.
- **Ts has no G3.** The per-step-air Ts cases span 1 to 20 kHz and 0.9 to 24 m from the source, with the direct sound at −12 to −1 dB.

**M3.**
- **The formula.** Spec 1.2 computes k_x from √(t² − h²) with t = emission + d/c. The chord symmetry holds about emission + √((d/c)² − h²).
- **What goes wrong.** With a delay, the formula subtracts more direct energy than the lump holds. X_max then falls below the reflected energy that is really there.
- **The mirror is safe; the spec is not.** The Python mirror, as the harness calls it, takes no emission input. It refuses all 31 cases, and so does the same geometry without a delay.
- **So the mirror and the spec disagree.** A Rust port that follows 1.2 and 1.6 would ship the bug, and Z4 parity on rows with emission 0 cannot see it.

**Guard margins in the robust cases without a delay:**
- **EDT:**
  - direct level −3.00 to −0.37 dB, so G3 is met, with some cases exactly at its −3 dB edge;
  - window-end level −3.5 to −0.8 dB, so G3b is met with at least 6.5 dB to spare;
  - R from 0.31 to 0.86 m, with 80 of 125 M1 cases at exactly 0.31;
  - steps from 0.5 to 2.0 ms.
- **Half-spread:** 0.09 to 0.998×L, so a margin rule does not help.

## The smallest change that refuses them: all post hoc

**All of these are post hoc.** Each was chosen after reading the hunt. It must be registered again and hunted again. None of it is a result.

- **M3: fix the formula.** Use e + √((t − e)² − h²). A delayed run then reads exactly like the undelayed one, which refuses all 31 cases. The mirror also needs an emission input, so that Z4 can test it.
- **M2 on Ts: extend G3 to Ts.**
  - It refuses 13 of the 20 per-step-air Ts cases, and 44 of the 62 Ts cases overall.
  - It costs 6 of the 3,287 real Ts values accepted at 2 ms.
  - It leaves 7 per-step-air Ts cases and 11 others. No partial fix I tried covers them all.
- **Everything without a delay: widen the window.** The only single knob in W1G's own family that refuses all 186 cases is a window of 9 extra bins for EDT and 11 for Ts.
  - The refusals come from the spread growing, not from the truth entering the bracket.
  - The cost on the spec's real data at 2 ms (`judge_cost.py`): accepted EDTs fall from 2,828 to 361 (−87 %) and accepted Ts from 3,287 to 646 (−80 %).
  - In tutorial 1's room, accepted EDTs fall from 2,387 to 361 of 2,640.
  - This is what spec 1.4 predicted for an exact bound.
- **The registered W2G does not fix it.** It still accepts 62 wrong EDTs (worst 14.5×L) and 12 wrong Ts.

**My read.** M1 and M2 cannot be fixed by a guard threshold. They are what is left once you decide that a coarse-step histogram can place EDT and Ts to 0.5 %.

**Three options, and the choice is yours:**
1. Take the wide-window cost.
2. Add a per-step air term, and accept the large refusal rate at 2 ms.
3. Relax what W1G claims for EDT and Ts, for example to a looser limit, and hunt again against that.

## Receipts: representative confirmed cases

All at 20 °C, 50 % RH and 101.325 kPa unless stated. Boxes are specular. α is given as (x0, xL, y0, yL, floor, ceiling). Ratios are conservative.

| Case | Quantity, band, step | Room (m), α | Source → receiver, R | Accepted / truth A (0.005 ms) / truth B | ×L |
|---|---|---|---|---|---|
| aco-002 | EDT, 10 kHz, 1.38 ms; 37.0 °C, 34.6 %, 81.8 kPa | 21.69×24.05×6.66; α 0.139, 0.861, 0.047, 0.959, 0.998, 0.650 | (15.087, 4.725, 2.213) → (1.644, 2.585, 1.180), 0.31 | 0.105692 / 0.121366 / 0.121413 s | 25.2 |
| aco-006 | EDT, 10 kHz, 2.00 ms; 1.8 °C, 21.9 %, 98.7 kPa | 28.91×9.91×4.82; α 0.882, 0.893, 0.717, 0.083, 0.875, 0.009 | (8.004, 7.163, 2.180) → (5.228, 2.528, 2.057), 0.31 | 0.169750 / 0.192333 / 0.192357 s | 23.1 |
| aco-008 | Ts, 200 Hz, 2.00 ms; 46.2 °C, 39.3 %, 99.0 kPa | 8.82×11.80×5.31; α 0.998–0.999 except ceiling 0.127 | (1.858, 0.892, 1.889) → (2.439, 2.737, 3.653), 0.31 | 1.19537 / 1.33062 / 1.33062 ms | 20.3 |
| aco-012 | EDT, 10 kHz, 1.00 ms; as aco-006 | 29.46×9.77×4.74; as aco-006 | (8.210, 7.026, 2.130) → (5.185, 2.536, 2.012), 0.31 | 0.173522 / 0.187051 / 0.187055 s | 14.5 |
| dis-017 (M3, delay 60 steps) | Ts, 20 kHz, 0.70 ms | 12.84×6.42×3.47; α 0.499, 0.227, 0.111, 0.334, 0.349, 0.625 | (7.197, 5.038, 0.982) → (7.836, 3.813, 1.849), 1.281 | 2.93099 / 3.09732 / 3.18449 ms | 10.7 |
| geo-000 (C0) | EDT, 20 kHz, 2.00 ms | 31.22×37.45×15.06; α 0.8, 0.8, 0.8, 0.8, 0.05, 0.05 | (30.438, 33.220, 7.213) → (4.593, 29.597, 5.507), 0.31 | 0.205384 / 0.213892 / 0.213879 s | 7.76 |
| aco-026 | EDT, 1 kHz, 1.98 ms; 19.9 °C, 69.7 % | 10.08×11.19×4.84; α 0.050, 0.650, 0.585, 0.505, 0.698, 0.600 | (8.180, 2.722, 1.766) → (2.501, 8.800, 2.894), 0.31 | 0.273447 / 0.279926 / 0.279926 s | 4.02 |
| geo-024 (C2, M2) | EDT, 6.3 kHz, 2.00 ms | 19.94×31.75×3.05; uniform α 0.301 | (16.906, 2.289, 0.811) → (5.760, 8.271, 2.240), 0.3105 | 0.679094 / 0.690742 / 0.691200 s | 3.37 |
| aco-027 | EDT, 400 Hz, 0.96 ms; 22.4 °C, 32.1 % | 16.28×12.30×4.86; α 0.7 (ceiling 0.699) | (9.277, 11.121, 1.730) → (4.114, 7.737, 2.779), 0.392 | 0.311248 / 0.316569 / 0.316570 s | 3.36 |
| geo-037 (C6) | EDT, 10 kHz, 2.00 ms | 27.15×34.16×8.61; tutorial-1 materials (0.2, 0.2, 0.2, 0.2, 0.1, 0.3) | (2.947, 25.522, 0.927) → (22.776, 29.557, 0.330), 0.31 | 0.504196 / 0.528955 / 0.528975 s | 2.65 (9.36 at −10 dB) |
| geo-069 (C13, M2) | Ts, 20 kHz, 2.00 ms | 8.82×13.25×7.24; tutorial-1 materials | (3.787, 8.972, 4.338) → (4.753, 8.832, 3.626), 0.31 | 2.03283 / 2.05375 / 2.05758 ms | 2.04 |
| dis-006 | EDT, 4 kHz, 2.00 ms | 2.78×12.65×3.73; α 0.158, 0.266, 0.547, 0.946, 0.131, 0.069 | (1.123, 2.748, 1.542) → (1.177, 4.398, 1.808), 0.31 | 0.311741 / 0.313886 / 0.313890 s | 1.28 |

Every case also passes G1 to G6, with R ≥ f32(0.31) and dt ≤ f32(0.002).

## Limits of this judgement

- **Specular boxes only, noise-free.** There are no diffuse walls, no non-box rooms and no directional sources.
- **I re-checked claims; I did not hunt.** Absence from this list proves nothing.
- **Non-clear balls are excluded,** because the image model does not clip the ball at a wall.
- **Per-step air follows upstream at the pinned tag.** This branch carries no solver patches.
- **M3 assumes SPPS's whole-step emission.**

## Files

**In `target/agents/z3-hunt/judge/`:**
- `judge.py`: the generator, the truths and the self-test.
- `judge_run.py`: the claims and the per-claim checks.
- `judge_report.py`: the classification.
- `judge_converge.py`: the convergence rebuilds.
- `judge_cost.py`: the post-hoc cost measurement.
- `judge_results.json`, which holds:
  - `groups`: every claim, reproduced;
  - `summary.confirmed`: the confirmed cases, each with its mechanism, the continuous-air run, the hybrid reading, the smallest refusing window and its conservative ratio;
  - `summary.not_confirmed`: the rejected claims, with reasons;
  - `convergence`;
  - `cost_real`;
  - `verdict`.

**To reproduce** (about 30 min on 8 processes): `python -B judge.py selftest`, then `judge_run.py run 8`, `judge_report.py`, `judge_converge.py <ids>` and `judge_cost.py`.
