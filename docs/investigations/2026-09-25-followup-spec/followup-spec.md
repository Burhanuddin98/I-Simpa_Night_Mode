# Pre-M8 follow-up: the early check, the seed batch and the refusals

For Burhan, 2026-09-25, 19:30. This is the spec for the follow-up piece that runs after the pre-M8
merge and before M8. It was written read-only. No solver was run and no code of record was
changed. The new evaluations in it are Python on existing files, in
`target/agents/followup-design/spec/`.

**Labels.** *Measured* means computed from existing runs, or from the skeptics' noise-free
models, which were checked against real seed means. *Inferred* means derived from measured numbers
through a stated step. *Guess* means no data stands behind it.

**Code of record:** `pre-m8` at `812853c` (`params/decay.rs`, `params/noise.rs`). Piece C
(`bd58f36`, `8a558ee`) has since moved the C and D limits to 0.1 dB and 0.005. It changed nothing
in the early check.

**Limits:** your strict 1/10-limen rule, to be revisited after M8: EDT, T20 and T30 at 0.5 %; Ts at
min(1 ms, 0.5 % of Ts); C at 0.1 dB; D50 at 0.005. *L* below means a quantity's limit, and "0.8×L"
means an error of 0.8 times it.

---

## The short answer

1. **All three designs died.** Each had zero false accepts on the 16 real datasets. Each let wrong
   numbers through where those datasets never went: receivers close to the source, 8 to 20 kHz,
   rooms deader than EDT 0.2 s, receiver radii below 0.31 m, and a blocked direct path.
2. **Chosen: W1G.** It is the geometric bracket (w1d) with its EDT extremes made exact and its air
   factor corrected, plus five guards. Each guard closes one hole a skeptic found:
   - a step of at most 2 ms;
   - a receiver radius of at least 0.31 m;
   - the direct sound taking at most 3 dB of EDT's 10 dB;
   - EDT's range reaching past the bracketed bins;
   - an arrival that fits the onset bin.
3. **Evidence: zero false accepts on every set that exists.** That is 69,404 series-steps: the
   real data, both skeptics' noise-free rooms, the 200 tutorial positions against a converged
   truth, and the seven hill-climbed cases. W1G accepted 9,487 EDT and 12,002 Ts values. The worst
   errors are 0.87×L for EDT and 0.84×L for Ts. Section 1.3 gives the tables.
4. **This is not a proof.** The curve after the bracketed bins is assumed log-linear. The guards'
   thresholds were chosen after reading the attacks, so those attack sets are in-sample for the
   guards. Section 1.4 gives the exact residual risk.
5. **The cost at 2 ms:**
   - tutorial 1's room: 13 % of EDTs and 4 % of Ts refused;
   - rooms with EDT from 0.2 to 0.35 s: 99 % of EDTs and 86 % of Ts refused, so they need 1 ms;
   - 3 ms and coarser: nothing at all.

   At both tutorial receivers, EDT and Ts come through from 125 Hz to 4 kHz, and EDT is refused
   from 8 kHz up (measured, noise-free).
6. **The seed batch is pre-registered** in section 2. It is reconciled with round 4: the batch is
   the authority, and round 4 is a floor inside it. Round 4 also judges single runs, but only if
   R4 shows that it covers the observed scatter.
7. **Nothing ships until the checks in section 4 pass.** The Python-only checks (a fresh skeptic
   and the port's parity) can run any time. The solver runs wait until the pre-M8 gates are done.
8. **Decisions for you:** 11 of them, in section 6. The ones that matter most are D1 (adopt W1G),
   D5 (a step ceiling for C and D too) and D7 (single-run energetic T20 and T30 before R4).

---

## 1. The early check

### 1.1 What goes wrong today

The shipped check reads each onset-relative quantity three ways:
- with the reverberation continued back to the arrival;
- flat from the start of the first bin wholly after the direct sound;
- flat to that bin's end.

It refuses when the three readings lie more than the limit apart. So it bounds *when* the
reverberation starts. It does not bound *where in a bin* energy arrives.

Six mechanisms, with the evidence for each:

| | Mechanism | Evidence |
|---|---|---|
| E1 | **A reflection lumped into the direct sound.** The bins the curve reads as direct sound reach past the time the first reflection enters the receiver ball. All three readings put that reflection at the arrival, so the bracket cannot contain the truth. | 1,257 of the shipped check's 1,378 EDT false accepts (`resolution/mechanism.txt`). POS200 R011: the ceiling reflection enters at 6.51 ms, the 2 ms lumped bin runs to 8 ms, and EDT is +1.0 % off. |
| E2 | **The continued-decay reading is physically impossible** once the gap to the first reflection is resolved. | 117 EDT false accepts. |
| E3 | **Shape inside a bin.** | 4. |
| E4 | **The direct sound takes most of EDT's 10 dB.** This happens near the source, in dead rooms and at 8 to 20 kHz. The fit then sees 1 to 4 dB of reverberant decay and amplifies any error inside a bin. | Tutorial room, 16 kHz, 0.62 m, 1 ms: truth 1.026 s, shipped check 0.935 s, w1d as designed 1.818 s (`bracket-skeptic/case_t1_16k.txt`). Not seen in the real data: there the level after the direct sound is never below −3.8 dB. |
| E5 | **Small receiver radius.** A specular pulse narrower than a bin sits somewhere inside it, and the histogram cannot show where. | 1.0 to 1.3×L at R = 0.1 and 0.05 m, found by hill-climbing (`skeptic-twostep/verify_fa*.txt`). |
| E6 | **Blocked direct path.** The arrival misfits the onset bin, the curve falls back to the two ends of the onset bin, and the arrival used is wrong. | Ts 2.3 to 7.5×L under the relaxed arrival rule (`skeptic-gf3/occluded.txt`). |

### 1.2 The rule: W1G

**Inputs.** `core::results` computes these per receiver and source, and passes them on
`EnergySeries`.
- **Arrival** `t = emission + d/c` and **half-width** `h = R/c`: as shipped.
- **First-reflection time** `t1 = emission + d1/c`, new. `d1 = max(d, min over the plane of every
  .cbin face of |mirror_plane(S) − Rpos|)`. A path whose first bounce lies in plane P is at least
  `|mirror_P(S) − Rpos|` long, by the triangle inequality, whatever the reflection law. So no
  reflected energy enters the ball before `t1 − h`.
- **Ball clear**, new: the receiver centre lies more than R from every face plane.
- **The band's air attenuation** `m`, in Np/m of energy, as SPPS computes it, new.

**Readings.** They replace `Early::{Arrival, FirstBin, AfterFirstBin}`.
- **(a) The lump.** These are bins `top_bin .. first−1`, which the shipped curve reads as direct
  sound.
  - Reflected energy can sit only in the lump bins that end after `t1 − h`:
    `X_max = S[max(⌊(t1−h)/dt⌋, top_bin)] − S[first]`. It is 0 when `t1 − h ≥ first·dt`.
  - When the ball is clear and `t1 − h ≥ t`, subtract a lower bound on the direct sound those bins
    must hold: `A·(S[0] − S[k_x])`.
    - `k_x = ⌊(2√(t²−h²) − τ)/dt⌋` and `τ = max(bin start, t)`.
    - `A = exp(−m·(2R + c·dt))`. SPPS applies air absorption once per whole step
      (`CalculationCore.cpp:50-58`), so across a chord the energy can drop by `exp(−m·c·dt)` as
      well as by the path.
    - The design's fixed `A = 0.95` was wrong at 16 to 20 kHz: in 57 of 13,563 cases its "lower
      bound" exceeded the true direct energy (`bracket-skeptic/dsym_air_search.txt`).
  - Over `(0, u1]` the curve is `S_first + X`, with `X` in `[0, X_max]` arriving in
    `[max(0, t1−h−t), u1]`.
- **(b) The window.** These are bins `first .. w_last`, with `w_last = max(first, bin holding
  t1 − h) + 1`. In bin k the curve stays at `S_k` until it drops to `S_{k+1}` somewhere in
  `[max(bin start, t1 − h), bin end]`.
- **(c) After the window:** the shipped log-linear model, unchanged.
- **LOW and HIGH.** LOW has every drop at its earliest, with `X = 0`. HIGH has every drop at its
  latest, with `X = X_max`. The Schroeder curve is monotone, so these two are Ts's exact extremes
  over (a) and (b).
- **EDT's two extreme readings.** Let ū be the time centre of the fit. The least-squares slope is
  linear in the level, and G3b (below) fixes the time the fit covers. So the slope's extremes over
  (a) and (b) are the curves that:
  - put each drop at the end of its interval whose midpoint lies on the far side of ū, for the
    largest slope;
  - put each drop at ū, or at the admissible point nearest it, for the smallest.

  w1d used the midpoint rule for both. The bracket skeptic showed its readings were not exact
  extremes (`split_counter.txt`). With G3b in force they now are.
- **The value** is midway between the lowest and highest reading. It is refused `early_unresolved`
  when the half-spread exceeds L.

**Guards.** A quantity is refused unless every guard holds. Each guard has its own reason.

| Guard | Condition | Refusal | Applies to | Hole it closes |
|---|---|---|---|---|
| G1 | `f32(dt) ≤ f32(0.002)` | `step_too_coarse` | EDT, Ts | Coarse steps. The morning skeptic's f32 trap: `f64(f32(0.002)) = 0.0020000000949949 > 0.002`. |
| G2 | `f32(R) ≥ f32(0.31)` | `receiver_too_small` | EDT, Ts | E5. |
| G3 | `10·lg(S_first / S_top) ≥ −3 dB`: the direct sound at most equal to all the energy after it | `direct_dominates` | EDT | E4. |
| G3b | `10·lg(S_{w_last+1} / S_top) > −10 dB`: EDT's range reaches past the bracketed bins, so every admissible curve spends the same time in it | `window_past_range` | EDT | Makes the EDT extremes exact, and catches E4 where G3 alone would not. |
| G4 | The decay start falls back to `Detected` | `arrival_misfit` | EDT, Ts (Ts also under the arrival relaxation H) | E6. |
| G5 | No geometric bound: fitting zones, or no face planes | `t1 = t` (no refusal of its own) | EDT, Ts | A celerity gradient gives no arrival, so G4 refuses it. Several sources are already refused (`several_sources`). |
| G6 | A reading that is not finite, a reading whose fit has no sloped piece, or no energy after the direct sound | `early_unresolved` | EDT, Ts | The GF3 skeptic's 7.5·10¹⁴ s EDT. w1d's flat-fit readings (the 155×L case). Ts = 0 in an anechoic room stays refused, as shipped. |

**Other quantities.**
- **T20, T30, C50, C80, D50.** They are read with the same LOW and HIGH readings in place of the
  three, and with no new guards.
  - Their windows lie after the bracketed bins, so their readings coincide. Nothing changes for
    them in practice.
  - T20 and T30 step error is at most 0.24 % at 10 ms (measured, `skeptic/t20_t30_step.txt`).
  - A step ceiling for C and D is decision D5.
- **The band aggregate and the noise resamples** carry the same inputs. For the aggregate, the
  air factor uses the largest `m` among the summed bands.

**Receipts.**
- `spec/REGISTERED.txt` fixed the rule and its variants at 18:58, before any of it was run.
- `spec/composite.py` is the Python mirror.
- `spec/check_mirror.py` checks it. With the guards off, the midpoint split and the fixed factor,
  it reproduces the w1d rule on 1,728 real series-steps: 0 verdict mismatches, values identical.

### 1.3 Evidence

Each row gives false accepts / accepted / series with a truth, and the worst accepted error. A
false accept is an accepted value more than its limit from the truth.

**Every set together** (`spec/totals.txt`):

| Check | EDT | Ts |
|---|---|---|
| Shipped (three readings) | 5,245 / 38,359 / 66,468, worst 11,588×L (EDT 9.47 s accepted against a true 0.161 s: tutorial room, 20 kHz, 0.84 m from the source, 3 ms) | 4,345 / 42,501 / 67,919, worst 21×L |
| w1d as designed, plus G6 | 18 / 12,639, worst 6.2×L | 10 / 16,654, worst 3.1×L |
| **W1G** | **0 / 9,487, worst 0.87×L** | **0 / 12,002, worst 0.84×L** |
| W2G (2 extra window bins) | 0 / 7,920, worst 0.87×L | 0 / 10,396, worst 0.84×L |
| W1G without G3 | 4 / 9,803, worst 3.6×L | 0 |
| W1G without G1 and G2 | 2 / 12,282, worst 1.3×L | 10 / 16,657, worst 3.1×L |
| W1G without G3b | 0, identical to W1G | 0 |

- G1, G2 and G3 are each load-bearing.
- G3b catches nothing that G3 misses on these data. It stays because it is what makes the EDT
  extremes exact.
- Margins: 87 of 9,487 accepted EDTs lie above 0.5×L and 6 above 0.75×L. The median is 0.05×L.
  For Ts it is 103 above 0.5×L and 4 above 0.75×L.

**By set, at the steps W1G can accept:**

| Set | What it is | W1G EDT at 1 / 2 ms | W1G Ts at 1 / 2 ms | Shipped EDT false accepts at 2 ms |
|---|---|---|---|---|
| Real (`spec/eval_real.py`) | The 16 fine-step datasets, 26,664 series-steps. Truth is the fine step's settled midpoint. | 0/144/144; 0/2,828/5,304 (46.7 % refused), worst 0.84×L | 0/144; 0/3,287 (38.0 % refused), worst 0.62×L | 24 |
| Synthetic (`spec/eval_synth.py`) | The bracket skeptic's specular boxes: near the source, near walls, corners, heights, R 0.31/0.5/0.9, up to 20 kHz. Truth is converged, at 0.02 ms. | 0/1,053; 0/492, worst 0.87×L | 0/1,693; 0/856 | 306 |
| POS200 noise-free (`spec/eval_pos200.py`) | Tutorial 1's room at POS200's 200 positions, 125 Hz to 16 kHz. Truth converged. | 0/1,232; 0/818, worst 0.60×L | 0/1,321; 0/1,015, worst 0.67×L | 61 |
| Image source (`spec/eval_ism.py`) | The GF3 skeptic's exact echograms: tutorial 1's room, the dead-floor corridor, 5×4×3 m at α 0.4 and 0.6, R 0.1/0.2/0.5, 125 Hz to 20 kHz. Runs with SPPS's per-step air. Truth ideal, and fine. | 0/1,251; 0/695 (0/974 at 1.5 ms), worst 0.80×L | 0/1,564; 0/943 | 345 |
| Hill-climbed (`spec/hunt.txt`) | The two-step skeptic's 7 false accepts | all 7 refused, by G2 (6) and G1 (1) | | |

The hill-climbed cases also test what the guards buy. Without G1 and G2, W1G accepts 2 of the 7
wrong (Ts 1.12×L, EDT 1.00×L), both at R ≤ 0.1 m.

**What it gives, per room, at 2 ms** (real data, `spec/report.txt`):

| Rooms | EDT refused | Ts refused | Shipped check |
|---|---|---|---|
| Tutorial 1's room (C-E6, C-R6, POS200, 1,920 series) | 13.2 % | 4.4 % | 0.2 % and 0 %, with 24 wrong EDTs and 8 wrong Ts |
| Dead rooms, EDT 0.2 to 0.35 s (A02 ×3, C-E3/4, C-R3/4, V-E2, V-R2; 2,250) | 98.8 % | 85.9 % | 0 % |
| Live rooms (B02, V-E5, V-R6, W-E6; 1,134) | 0 % | 0 % | 0 % |

- **At the 1 ms step itself**, on the twelve real 1 ms datasets, W1G accepts 5,154 of 5,160 EDTs;
  6 are refused `direct_dominates`. It accepts all 5,160 Ts (`spec/at_1ms.txt`).
- **Tutorial 1's two receivers**, noise-free, per-step air (`spec/tut_receivers.txt`):
  - At 2 ms, EDT and Ts are accepted at 125 Hz to 4 kHz with errors of at most 0.14×L. EDT is
    refused from 8 kHz up. Ts is refused from 8 kHz at Receiver 1 and from 16 kHz at Receiver 2.
  - At 1 ms, EDT comes through up to 16 kHz at Receiver 1 and up to 8 kHz at Receiver 2.

### 1.4 Residual risk, exactly

**What is proven,** given straight-line propagation and SPPS depositing energy times chord length
per step:
- the lump bound (the triangle inequality);
- the in-bin staircases (the Schroeder curve is monotone);
- Ts's extremes;
- EDT's extremes over (a) and (b) once G3b holds;
- the direct-symmetry subtraction for a point source whose chords are whole, with the per-step air
  factor.

**What is assumed** is the log-linear curve after the window. The only exact bound there is
staircases over the whole fit range, about ±1.5·dt/U relative (U the time to reach −10 dB). That
bound refuses nearly everything above about 0.2 ms.

The assumption is what fails when:
- R is small (E5, closed by G2);
- the fit is short (E4, closed by G3 and G3b);
- the step is coarse (closed by G1).

Nobody has yet shown it failing inside all three guards.

**In-sample:**
- The thresholds were chosen after the skeptics' attacks were read:
  - G2's 0.31 m is the only radius tested at 2 ms apart from 0.5 and 0.9 m;
  - G3's −3 dB was chosen from the physics (direct equal to reverberant); the attack sets' false
    accepts start at −6 dB, and the real data never go below −3.8 dB;
  - G1's 2 ms is the proposed default.
- The one-bin window was chosen by the w1d design after w0 failed.
- So "zero" here is zero on data that shaped the rule. It is not an out-of-sample result.

**Truths:**
- The real data's truth is the fine step: 0.2 ms only in 4 Lambert cells, 1 ms elsewhere.
- The 1 ms reading is itself up to 1.46×L off the converged truth at the 200 tutorial positions
  (noise-free, `bracket-skeptic`), and 1.33×L at 16 kHz (`skeptic-twostep`).
- The noise-free sets use converged truths, but they are specular boxes only.

**Not covered by any data:**
- non-box and non-convex rooms, fitting zones, coupled volumes;
- partly diffuse walls in the noise-free sets;
- real runs above 4 kHz;
- rooms deader than EDT 0.09 s;
- temperatures other than 20 °C;
- directional sources;
- sources nudged to a tetrahedron vertex (about 1.5 cm; the bracket skeptic found no effect);
- the bin that holds ū when the fit is very short.

**Not hill-climbed:** nobody has hill-climbed W1G yet. The two-step skeptic's hunts on its own rule
pushed accepted errors at R 0.31 m to 0.86×L (Ts, 4 ms) and 0.73×L (EDT, 3 ms), and crossed 1×L
only at 5 ms. A fresh hunt on W1G is the first confirmation step (Z3).

### 1.5 The rejected designs, and why each died

| Design | What it was | How it died | Receipt |
|---|---|---|---|
| The shipped three readings | Continued, first bin, after first bin | 24 EDT false accepts at 2 ms on POS200 (worst 2.0×L), 105/380/870 at 3/4/5 ms, and 4 EDT and 120 Ts at 10 ms. On the noise-free sets: 3 EDT false accepts even at 1 ms, up to 1.46×L. | `bracket/report.txt`; `spec/report.txt` |
| A 2 ms ceiling alone (resolution D) | Refuse above 2 ms, keep the three readings | The morning skeptic: 24 wrong EDTs at 2 ms at 7 of 200 positions. Every failing receiver has the direct sound's trailing edge in the first 40 % of a bin. | `t30-edt-diagnosis/skeptic/positions_step.txt` |
| dt ≤ EDT/200; agreement of dt and 2·dt; Richardson at order 1, order 2, observed order and phase means | Consistency and extrapolation | Every form lets wrong values through. Every coarser grid lumps the same reflection (E1), so no test internal to the histogram can see it. | `resolution/step_consistency.txt`; `twostep/twostep.txt` |
| w0 (the brief's literal form) | The lump plus the first-reflection bin | 3 EDT false accepts at 3 ms (1.09×L), and geometric: the same sign in every band. The pulse window gives 2. | `bracket/summary_*.txt` |
| **w1d (geometric bracket)** | The lump plus the window, with 0.95 air | The bracket skeptic found 10 EDT false accepts on noise-free specular rooms, up to 155×L, every one with the direct sound taking 6 to 9 dB of EDT's range. Its split readings are not exact extremes. Its air factor is wrong at 16 to 20 kHz. The Detected path is undefined. It accepts Ts = 0 in anechoic cells. **W1G keeps its core and closes each hole.** | `bracket-skeptic/` |
| GF + 3-bin | A geometric gate, then 3 bins bracketed | 154 of 1,110 accepted EDTs false on exact echograms. 19 of them were refused by the shipped check. Ts at R 0.1 up to 3.1×L. An occluded source under H gives Ts 2.3 to 7.5×L. A NaN bug accepts EDT 7.5·10¹⁴ s. Its chord bound is false per realisation. It refuses any receiver with `d1 − d ≤ 4R/3` at every step. | `skeptic-gf3/` |
| TWOSTEP-G | A change bound over every grid phase, plus an onset guard | Hill-climbed false accepts at R 0.1 and 0.05 m (1.0 to 1.3×L), and Ts at 5 ms (1.015×L). The change bound cannot see where a pulse sits in its bin: in a toy case the truth moves 3.7×L while every reading stays identical. It refuses 93 % of EDTs at 2 ms, and tutorial 1 loses EDT at both receivers at every step. TWOSTEP-L lets through 24 wrong EDTs, up to 5.5×L. | `skeptic-twostep/` |

### 1.6 The port

All in `crates/simpa-core`.

- **`params.rs`**
  - `EnergySeries::with_early_reverberation_unresolved()` becomes
    `with_early_reverberation(EarlyBound { first_reflection_s: Option<f64>, ball_clear: bool,
    air_np_per_m: Option<f64>, receiver_radius_m: f64 })`. `None` means `t1 = t`.
  - New `NotEvaluable` variants: `StepTooCoarse`, `ReceiverTooSmall`, `DirectDominates`,
    `WindowPastRange`, `ArrivalMisfit`. Each carries its numbers.
  - `aggregate` passes the bound on.
- **`params/decay.rs`**
  - Replace `enum Early` with the slots of 1.2.
  - `Curve::new` builds flat pieces with in-bin breaks. `Shape::LogLinear` with `s0 == s1`
    already integrates correctly.
  - `settle` is unchanged, apart from the G6 flat-fit check.
  - Compare dt and R in f32.
- **`params/noise.rs`**: resamples carry the same `EarlyBound`.
- **`results/spps.rs`**
  - Compute `t1` per source and receiver from `RoomInputs`' `.cbin` triangles (already read for
    the reference), and include `emission_s`.
  - Compute ball-clear against face planes, as tested. The polygon distance is a later refinement.
  - `m` per band from the run's atmosphere, as `Coef_Att_Atmos.cpp` computes it (mirrored in
    `skeptic-gf3/ism.py`).
  - Fitting zones in `config.xml` set `first_reflection_s: None`.
- **Tests**
  - Each guard gets a fault seam that makes it say no.
  - A parity test: the Rust verdicts equal the Python mirror's on a committed sample of
    `spec/real_rows.pkl` and `spec/ism_rows.pkl`.
  - `docs/params.md`, "The early reverberation", and the refusal-code table are rewritten.
    `docs/results.md` gets the costs.

---

## 2. The seed batch, pre-registered

It is registered here, before R1 or R4 is run or read. At the port it is copied to
`docs/investigations/<date>-seed-batch/PREREGISTER.txt`.

**SB-1 Scope.** Energetic-mode T20 and T30, as you decided on 2026-09-25 at 10:43. Energetic EDT
is decision D8.

**SB-2 A batch.**
- A batch is k = 10 SPPS runs of one project whose `config.xml` files differ only in
  `random_seed`.
- The seeds are 10 distinct non-zero values (1 to 10 by default).
- The whole batch is refused (`batch_invalid`) for any of these:
  - **A seed of 0.** An unseeded SPPS run starts its band threads, which share one global unlocked
    generator (`sppsTypes.cpp`; `sppsNantes.cpp:304-306, 376-377`). Its draws are then neither
    reproducible nor shown to be independent.
  - **A duplicate seed.**
  - **Any other difference in the configuration,** compared as SPPS reads it: reals as f32 bit
    patterns, for example `pasdetemps`.
  - **A different mesh,** by file hash.
  - **A run that is missing, failed or unverified.**

**SB-3 Every seed is checked.**
- Each seed is read by the full per-run pipeline, the same build.
- If any seed refuses the quantity, the batch refuses it (`batch_seed_refused`, naming the seed and
  its reason). This covers every refusal: truncated, missing moves, floor, range, early check,
  arrival, step and radius.
- The one exception is the seed's own single-run noise judgement (`monte_carlo_noise`,
  `noise_uncalibrated`). The batch replaces that judgement for the quantity it covers.
- A mean over a subset of seeds is never formed. That rules out the selection bias the calibration
  measured (energetic T30 −1.23 % ± 0.13 %, T20 −0.75 %; `selection.txt`).
- Whether per-seed noise refusals also count is decision D6.

**SB-4 Value.** The mean x̄ of the k seed values.

**SB-5 Noise.**
- s is the seeds' sample standard deviation, with k − 1 degrees of freedom.
- U is the one-sided 95 % upper bound on the mean's standard error:
  `U = s·√((k−1)/χ²₀.₀₅,k−1)/√k`. For k = 10, `χ²₀.₀₅,₉ = 3.3251`, a factor of 1.645.
- The batch's noise is `σ_b = max(U, m_cal/√k)`. `m_cal` is the shipped calibrated single-run
  standard deviation (round 4's roughness, or round 3's uniform-Lambert entry) when it exists
  inside its domain, and is left out otherwise.
- The floor keeps your rule that the noise is never below the observed noise and never below the
  calibrated model's.

**SB-6 Refusal.**
- The batch refuses `monte_carlo_noise` when `σ_b/x̄ > 2.5 %`, the shipped single-run limit.
- The refusal names:
  - the seed count k′ at which U would be within the limit at the observed s (inferred: it assumes
    s holds);
  - and, as an alternative, the particles per run `N·(σ_b/(0.025·x̄))²`. Scaling as 1/√N was
    confirmed one-sided on all 23 pairs.

**SB-7 Bias.** Nothing new is added. Every seed passed every bias bound, so the mean is within each
limit of each seed's unknowns. Averaging adds a bias of about sd², about 0.01 % here (inferred).

**SB-8 Output.** x̄, U, σ_b, k, the seeds, `m_cal/√k` and every seed's value.

**SB-9 Its bed is R4** (section 4). It passes only if both hold:
- **(a) Coverage.** The 30 seeds are split into three disjoint batches of 10. For each batch and
  receiver-band where all 10 seeds give the value, U ≥ s₃₀/√10 in at least 90 % of pairs, in every
  cell. The 90 % allows for s₃₀ being a noisy proxy. The expected rate under normal noise is to be
  computed by simulation and committed before R4 is read.
- **(b) The mean.** The batch mean lies within 2·σ_b of the mean of the other 20 seeds in at least
  90 % of pairs.

  If either fails, the batch does not ship. The pass rules are not changed after reading.

**Reconciliation with round 4** (shipped in `812853c`, single-run roughness):
- **What round 4 shows.** Its validation passes, pooled per cell, in 11 held-out rooms plus 5 after
  F6, at 1 and 10 ms. It overstates 2.4 to 5.6 times in specular rooms, which is safe. It has *no
  margin* in the Lambert boxes at α 0.4: 0.90 to 1.02 of the seeds' spread, and V4-E1 T30 at 1.05
  (lower bound 0.97).
- **What it does not show.**
  - Coverage per receiver-band.
  - Any step other than 1 and 10 ms. The step is stated in `monte_carlo.measured_on` but not
    checked.
  - The proposed preset (2 ms, ε 7).

  So the evidence does not show that the single-run estimate covers the observed scatter where
  users will run.
- **Therefore** the seed batch is the authority for energetic T20 and T30. Round 4 is its floor
  (SB-5).
- **Single runs** are decision D7. I recommend that round 4 judges single runs only if R4 shows
  that each seed's round-4 standard deviation is at least s₃₀ in at least 90 % of
  receiver-band-seeds per cell. Until then, a single run's energetic T20 and T30 are refused,
  naming the batch. Nothing is shown to users before M12 in any case.

---

## 3. Refusals that name the setting to change

Each message states the setting, the value to use, and whether that value is measured or inferred.

| Code | When | Text (template) |
|---|---|---|
| `step_too_coarse` | EDT or Ts with a step above 2 ms (C and D too, if D5) | "EDT needs a time step of 2 ms or finer; this run used {dt} ms. Rooms with an EDT below about 0.35 s need 1 ms (measured on box rooms)." |
| `early_unresolved` (W1G) | Half-spread above L | "The first reflections cannot be placed finely enough at {dt} ms: EDT lies between {lo} and {hi} s. Use a time step of 1 ms." At 1 ms and finer: "Use a time step of 0.5 ms (not tested), or move the receiver away from walls and the source." |
| `direct_dominates` | EDT, level after the direct sound below −3 dB | "The direct sound carries more energy than all the sound after it ({level} dB): EDT is not defined reliably this close to the source. Move the receiver further from the source." |
| `window_past_range` | EDT | "EDT's 10 dB decay ends within the first reflections at this step. Use a time step of 1 ms." |
| `receiver_too_small` | EDT or Ts with R below 0.31 m | "EDT and Ts are tested only with a receiver radius of at least 0.31 m; this run used {R} m." |
| `arrival_misfit` | EDT or Ts on the Detected fallback | "The direct sound is not where the geometry puts it: the path is blocked, or the source faces away. EDT and Ts are not computed for this receiver." |
| `missing_moves` (floor dominates) | The floor's share is above half the move | "Energetic mode drops particles at 10^−ε of their energy; at ε {eps} that moves {q} by {x} %. Raise trans_epsilon to 7." |
| `monte_carlo_noise`, energetic T20/T30, single run | | "…or run a seed batch of 10 seeds (section 2)." Keep the shipped count. |
| `monte_carlo_noise`, random T30 | | "Random mode needs about 20 to 35 million particles per source for T30 at 125 Hz to 4 kHz (inferred from 0.6 and 1.5 million runs, not measured at that count). Run at least {N_min} to be told a count for this room, or use energetic mode." `N_min = N·31,600/crossings`, about 1.44 million on tutorial 1 (inferred). |
| `batch_invalid`, `batch_seed_refused`, batch `monte_carlo_noise` | Section 2 | These name the seed, its reason, and k′ or N. |

The morning skeptic's point is kept: "2 ms or finer" is never the advice to someone already at
2 ms. At 2 ms the advice is 1 ms.

---

## 4. Confirmation runs

**Order:**
- Z1, Z3 and Z4 need no solver and can run any time CPU allows.
- The R runs start only after the pre-M8 gates and merge. Nothing runs during M6.
- Every run is seeded.
- Unseeded runs are for timing only, and are timed alone.

| # | What | Settings | Pass (fixed before reading) | Amended from `resolution.md` |
|---|---|---|---|---|
| Z1 | Pre-register | Commit sections 1.2 and 2 as `PREREGISTER.txt` | Committed before any R run | As before |
| **Z3** | **A fresh skeptic on W1G** | Hill-climbing (as in the two-step hunts) on the image-source and synthetic generators; rooms, heights and bands no design used; R ≥ 0.31 m and dt ≤ 2 ms | No accepted value beyond L. One failure sends W1G back. | New |
| Z4 | Port parity | Rust against the Python mirror on committed rows | 0 verdict mismatches, values within 1e-12 | New |
| R1 | Tutorial 1 at the preset | Energetic, ε 7, 2 ms, 150k, seeds 1 to 10 at once, 27 bands, both receivers; plus one unseeded run alone | EDT and Ts pass the early check at both receivers from 125 Hz to 4 kHz in ≥ 95 % of receiver-band-seeds; EDT refused at 16 to 20 kHz; batch T20 and T30 within 2.5 %; `missing_moves` 0; record the batch wall time | Predictions updated to W1G |
| R2 | The reference, **over positions** | Tutorial 1's room: POS200's 200 positions plus both receivers, 27 bands, energetic ε 7, 1.5M, seeds 1 to 3, at 0.2 ms and as *real* 1 and 2 ms runs (per-step air, not rebins); plus one random seed | Every EDT and Ts that W1G accepts at 1 and 2 ms is within L of the same seed's 0.2 ms reading. The 0.2 ms reading agrees with the image-source truth for the same geometry within noise. | The morning skeptic: two receivers would have confirmed a broken ceiling |
| R3 | Random against energetic T30 | ≥ 3 seeded random runs at 20M, or 10 at 6M, against energetic ε 9, 1.5M, 10 seeds | A pooled test on per-seed band averages (pooled SE about 0.3 %). Pass: the 95 % interval of the gap lies within ±0.5 %. Fail: it excludes 0 and centres beyond 0.5 %. Otherwise add seeds. Neither mode's T30 ships before a pass. | The morning skeptic: the per-band rule passed a 1.35 % gap (t 2.53) |
| R4 | Coverage of the seed bound and of round 4 | Tutorial 1, energetic ε 7, 2 ms, 150k, 30 seeds; the W-E4 corridor (scattering 0.3), 30 seeds; a 5×4×3 m Lambert box at α 0.4 (round 4's no-margin case), 30 seeds | SB-9 (a) and (b); the round-4 single-run rule of D7 | Adds the α 0.4 box and the round-4 test |
| R5 | Deader than EDT 0.2 s | 5×4×3 m Lambert α 0.6, energetic ε 9, 3 seeds, 30 positions including 0.5 to 1 m from the source and near walls, at 0.2 ms and real 1 and 2 ms | No accepted EDT or Ts beyond L. Predicted: almost nothing accepted at 2 ms. | Position sweep added |
| R6 | Non-box and occluded | An L-shaped or partitioned room with receivers near walls (the ball not clear) and some with the direct path blocked, 3 seeds, at 0.2 and 2 ms | No accepted value beyond L; blocked receivers refused `arrival_misfit` | Replaces the 3 ms run, which is moot under G1 |
| R7 | Only if D2 takes the ratio form | Tutorial 1 at R 0.1 and 0.2 m, 0.2 and 1 ms, 200 positions | As R2 | New, optional |

---

## 5. What each change costs users in run time

| Change | Cost | Basis |
|---|---|---|
| The early check (W1G) | 4 readings per quantity for EDT, 2 for Ts, against 3 shipped. One min over the face planes per receiver. The Python mirror takes about 1 ms per series and variant; Rust will be less. Negligible. | Measured, Python: 26,664 series-steps × 7 variants in 52 s on 4 processes |
| A step of 2 ms instead of 10 ms | Run time 1.0 to 2.0 times; about 5 times the output | Inferred |
| A step of 1 ms (dead rooms) | 1.1 to 2.0 times the time at 10 ms; 1.2 to 1.6 times on M8's cells | Measured |
| trans_epsilon 7 instead of 5 | 1.3 to 1.4 times | Inferred (ε 9 measured at 1.65 times) |
| Seed batch, 10 × 150k at 2 ms, ε 7 | About 70 to 140 s wall on Grace. Seeded runs are single-threaded, run in parallel, and need 10 free cores. On an 8-core machine they run in 2 to 3 waves (not measured). | Inferred from 50 s measured for 20 concurrent seeded runs at 10 ms, ε 5 |
| … the same against upstream | 35 to 70 times upstream's default. Upstream's default is random mode, about 2 s on Grace (inferred). The 13.6 s "baseline" was an energetic run (the morning skeptic). | Inferred |
| Random mode for T30 | 20 to 35M particles per source: 14 to 63 min seeded on one thread; 0.5 to 2.5 min unseeded with band threads on Grace | Inferred, never measured |
| Storage per batch at 2 ms | About 35 MB of solver files and 60 to 75 MB of reports | Inferred |

**What users lose, whatever the setting:**
- EDT and Ts above 2 ms;
- EDT close to the source;
- EDT and Ts with receiver radii below 0.31 m;
- most EDTs in rooms with EDT below about 0.35 s unless they run at 1 ms.

M7 accepted EDT at 10 ms in slow Lambert rooms, within 0.30 % of 0.2 ms. Those values are
refused now.

---

## 6. Decisions for you

| # | Decision | Recommendation | Why |
|---|---|---|---|
| **D1** | Adopt W1G as the early check for SPPS EDT and Ts, to ship only after Z3, Z4, R2, R5 and R6 pass | **Yes** | Zero false accepts on 69,404 series-steps, including every case any skeptic found. The residual risk is stated in 1.4. The alternatives are worse: the shipped check is wrong 5,245 times, and TWOSTEP refuses 93 %. |
| D2 | The receiver-radius guard: a flat 0.31 m, or a ratio (R ≥ 155 m/s × dt, the tested 2 ms at 0.31 m scaled) | **Flat 0.31 m** | Only 0.31, 0.5 and 0.9 m are tested inside the guards. The ratio form needs R7. |
| D3 | G3's threshold at −3 dB | **−3 dB** | Physics sets it: the direct sound equal to the rest. In the real data it refuses 6 of 5,304 EDTs at 2 ms and 6 of 5,160 at 1 ms. The false accepts begin at −6 dB. |
| D4 | Keep the 2 ms ceiling (G1), although W1G without it had 0 false accepts at 3 to 5 ms on this data | **Keep** | W1G accepts only 7 to 18 % there anyway. Coarse steps are less tested for per-step air and phase. It gives users one message. |
| **D5** | A step ceiling for C50, C80 and D50 (resolution finding 2, still open after piece C) | **2 ms, the same message** | Under piece C's limits they are clean at 2 and 3 ms. At 5 ms, 4 C50 values are wrong on POS200. At 10 ms, 3 to 60 of 240 to 300 are wrong. The cost: tutorial 1 at 10 ms loses the 472 to 540 C and D values it has today. (3 ms is the largest clean step, if you prefer it.) |
| D6 | In a batch, does a seed's own noise refusal refuse the batch? | **No; every other refusal does** | The batch exists to replace the single-run noise judgement. Counting it would refuse most energetic T20 batches on tutorial 1: noise refuses 334 of its 540 single-run T20s under round 4. No seed is ever dropped, so there is no selection bias. This matches `docs/results.md`, which says to judge every seed's value whatever its noise refusal. |
| **D7** | Single-run energetic T20 and T30 before R4 | **Refused, naming the batch, unless R4 shows round 4 covers the per-receiver-band scatter** | Your rule puts the burden on round 4, and round 4 has no margin at α 0.4 and no data at 2 ms. Nothing is shown before M12, so this costs nothing now. |
| D8 | The batch's scope | **T20 and T30 now; energetic EDT after R4** | This is what you decided. EDT's 54 of 54 on tutorial 1 is inferred. |
| D9 | The batch's size | **k = 10, fixed** | 5 seeds also clears tutorial 1 (factor 2.37, inferred). Allow 5 only if R4's 5-seed split covers. |
| D10 | The M11 preset | **Energetic, ε 7, 2 ms, 150k × 10 seeds. Offer 1 ms when the pre-run Eyring EDT is below 0.35 s** | This is the resolution's package, corrected to W1G's cost in dead rooms. The 0.35 s threshold is inferred. |
| D11 | The arrival relaxation H for Ts | **Only together with G4** | Under H, a blocked path otherwise gives Ts 2.3 to 7.5×L. |

Also carried forward: random-mode and energetic-mode T30 differ by 1.35 % on tutorial 1's materials
(t 2.53). Neither mode's T30 is shown as the room's until R3 passes.

---

## Receipts

Everything is under `target/agents/followup-design/`.

**`spec/`** (this piece):
- `REGISTERED.txt` fixed the rule at 18:58, before any run.
- `composite.py` is the rule. `harness.py` holds the variants. `check_mirror.py` checks the mirror
  against w1d.
- The evaluations and their rows:
  - `eval_real.py` → `real_rows.pkl`
  - `eval_synth.py` → `synth_rows.pkl`
  - `eval_pos200.py` → `pos200_rows.pkl`
  - `eval_ism.py` → `ism_rows.pkl`
  - `eval_hunt.py` → `hunt.txt`
- `tut_receivers.py` → `tut_receivers.txt`; `at_1ms.py` → `at_1ms.txt`.
- `report.py` → `report.txt`, the tables; `totals.py` → `totals.txt`.
- To reproduce, about 20 min on Grace: `python eval_real.py 4`, `eval_synth.py 4`,
  `eval_pos200.py 6`, `eval_ism.py 6`, `eval_hunt.py`, then `report.py` and `totals.py`.
- Note: the row labelled "w1d" in the tables includes G6's non-finite and flat-fit checks. Without
  them, w1d also accepts the 155×L case.

**The designs:** `bracket/` (w1d), `resolution/` (GF + 3-bin), `twostep/`.

**The skeptics:** `bracket-skeptic/`, `skeptic-gf3/`, `skeptic-twostep/`. The morning skeptic is in
`t30-edt-diagnosis/skeptic/`.

**Source documents:** `t30-edt-diagnosis/resolution.md`, and
`docs/investigations/2026-09-25-noise-calibration/` at `812853c`.
