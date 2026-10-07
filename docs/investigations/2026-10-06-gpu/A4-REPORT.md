# A4: the two SPPS rules spps-gpu shipped untested, bedded (2026-10-07 10:04-10:30, Grace)

## Verdict

| rule | verdict | the numbers (REALISED) |
|---|---|---|
| **10, one-sided pass-through** (`side_material` 0: a particle crosses the face from the side its normal points to, and is reflected from the other) | **PROVEN** | Beam through a one-sided panel: SPPS and spps-gpu **bit-identical at every step of every receiver and source echogram** (48 of 48 series, 3,000 steps each, two seeds), and both equal the exact model to 1.1e-4. Omni case: the reflecting-side source puts **exactly 0 energy, in 0 steps,** on the four far-side receivers, in all 8 runs (2 solvers x 2 seeds x energetic and random). Parameters: no bias against SPPS (mean signed d/range -0.08 to +0.20 against controls -0.31 to +0.13). CPU and GPU walks identical. |
| **11, transmission** | **DIVERGES** in the energetic child branch (0 < alpha < 1); the alpha = 1 branch and random mode are PROVEN | The per-child arithmetic is exact: the first transmitted arrival over the incident one is **0.1000021 against tau = 0.1 (TL 10 dB) and 0.01000021 against 0.01 (TL 20 dB)**, in both solvers. But spps-gpu holds at most **16 waiting children per particle** (`QCAP`, `walk.h:371`, dropped in `pushChild`, `:527-529`); SPPS's queue is an unbounded `std::list` (`CalculationCore.h:33`, drained at `sppsNantes.cpp:148-155`). In the beam, SPPS needs a queue **1,192 deep** at TL 10 dB; spps-gpu drops **383 of 2,541 children per particle** and its receiver totals fall **2.45 to 3.26 % short** of SPPS (0.011 to 0.032 % at TL 20 dB), while SPPS matches the exact model to 1.7e-5 and spps-gpu matches the same model with a 16-child queue to 1.7e-5. In a realistic omni case at 500 Hz it drops **3.97 M children**, and with them their descendants: it runs **8.49 M particles where SPPS runs 14.46 M**, and **T30 reads 0.8 % short in the source room** (0.5049 s against 0.5088 s, all 8 spps-gpu values below all 8 SPPS values). The run's verdict is still OK. |

Nothing was refused: spps-gpu ran every case with exit 0. Neither solver was patched.

**What it means.** A project with one-sided faces gets the same answer on the GPU as from SPPS:
bit for bit where no random number is drawn, and within Monte-Carlo noise where one is. A project with a
transmitting wall gets a GPU answer that is slightly wrong whenever the wall's absorption is below 1, in
energetic mode. Every transmitted particle is right, but most of them are never run once a particle has
more than 16 waiting. On these cases the error is below what a listener notices: T30 0.8 % against a
5 % JND, with no SPL shift within noise. It grows with lower absorption, a lower transmission loss and a
higher extinction exponent (the app's default is 7), and the run manager reports the run as OK. The
fix is its own step: a deeper or spilling child queue in `walk.h`, then this bed again. Random mode and
fully absorbing transmitting walls (alpha 1) never queue a child, so they are unaffected.

## The cases (re-runnable)

One geometry for all cases, written by `solvers/spps-gpu/bed/a4_cases.py`. A 6 x 10 x 3 m box is split
at y = 5 by an 18 m2 internal panel, two triangles whose edges lie on the outer walls. Room 1 is
y 0-5 and room 2 is y 5-10. The panel is wound so that its normal is +y (towards room 2). SPPS computes
a scene face's normal as (b - a) x (c - b) (`lib_interface/Core/mathlib.h` `FaceNormal`). In the
`mesh.cbin` the panel's faces are (4,6,5) and (4,7,6), which gives (0, 18, 0). So a particle in room 2
moving -y has dir . n < 0, the "doInvertNormal" side (`CalculationCore.cpp`, `dir . n <= -BARELY_EPSILON`).

The walls, floor and ceiling share one material: alpha 0.2, double-sided. TetGen gives 72 tetrahedra.
The settings common to every case are octave bands 500 and 1000 Hz, energetic (`computation_method` 1)
unless stated otherwise, `trans_calc` 1, `trans_epsilon` 7 (the app's default), dt 1 ms, duration 3 s,
receiver radius 0.31 m, 20 °C, 50 % RH and 101,325 Pa. The point receivers each have their own
echogram per source.

| case | panel | surfaces | sources | receivers | air abs. | particles per source per band |
|---|---|---|---|---|---|---|
| `trans` | alpha 0.3, TL 10 dB at 500 Hz and 20 dB at 1 kHz, double-sided | all diffuse (scattering 1, Lambert) | S1 omni (2.1, 1.4, 1.45) in room 1 | R1a-d in room 1, R2a-d in room 2, each at least 2.4 m from S1 | on | 200,000 |
| `trans-random` | as `trans` | as `trans` | as `trans` | as `trans` | on | 200,000, `computation_method` 0 |
| `trans-a1` | alpha 1, TL 10 / 20 dB, specular | walls diffuse | as `trans` | as `trans` | on | 200,000 |
| `beam` | as `trans`, specular | all specular | S1 unidirectional +y at (3.1, 1.6, 1.55) | B1a y 0.6, B1b y 3.3, B2a y 6.7, B2b y 8.6, all at x 3.1, z 1.55 (on the beam) | off | 2,000 |
| `beam-a1` | as `trans-a1` | all specular | as `beam` | as `beam` | off | 2,000 |
| `onesided` | alpha 0.2, no transmission, **single-sided** | all diffuse | Srefl omni (2.1, 1.4, 1.45) in room 1; Spass omni (3.0, 7.4, 1.6) in room 2 | R1a-d, R2a-d | on | 200,000 |
| `onesided-random` | as `onesided` | as `onesided` | as `onesided` | as `onesided` | on | 200,000, `computation_method` 0 |
| `beam-onesided` | as `onesided`, specular | all specular | Srefl unidirectional +y at (3.1, 1.6, 1.55); Spass unidirectional -y at (3.1, 8.0, 1.55) | as `beam` | off | 2,000 |

Receiver positions: R1a (4.6, 2.9, 1.1), R1b (1.0, 3.9, 2.05), R1c (4.9, 0.8, 2.2), R1d (1.3, 4.3, 0.9),
R2a (1.3, 6.1, 1.2), R2b (4.8, 6.4, 1.9), R2c (1.5, 8.9, 1.8), R2d (4.5, 8.7, 1.1).

To re-run (all output under `.out\a4\` and `C:\tmp\nm-a4\`; solvers from `C:\tmp\nm-solvers-a5`:
`spps.exe` sha256 973059a5…, `spps-gpu.exe` 2401ae86…, both "solver build verified" in every run.json;
`simpa.exe` built from this tree into `C:\tmp\nm-target-a4`):

```
python solvers/spps-gpu/bed/a4_cases.py .out/a4/projects
SIMPA_SOLVERS_DIR=C:\tmp\nm-solvers-a5  simpa run .out/a4/projects/<case>.simpa --solver spps --runs .out/a4/mesh-runs/<case>
                                        (meshes it and runs SPPS once at 2,000 particles; its solve folder is the bed's source)
python solvers/spps-gpu/bed/a4_bed.py a C:\tmp\nm-a4\runs\a [case ...]                 arm (a)
python solvers/spps-gpu/bed/a4_bed.py b <tag> spps,gpu 101,202 [case ...]             arm (b) runs
python solvers/spps-gpu/bed/analyse.py  .out/a4/bed/analysis.json .out/a4/bed/analysis.md .out/a4/bed/bed-*.jsonl
python solvers/spps-gpu/bed/a4_bias.py  .out/a4/bed/bias.json .out/a4/bed/bed-*.jsonl
python solvers/spps-gpu/bed/a4_physics.py .out/a4/bed/physics.json .out/a4/bed/physics.md .out/a4/bed/bed-*.jsonl   arm (c)
```

Each staged folder is a copy whose ROOT `workingdirectory` is rewritten (`stage.py`; its output roots now
include `.out\a4` and `C:\tmp\nm-a4`), or a `__RUNDIR__` template that `simpa run-folder` fills with
the run's own folder. A sweep of every `config.xml` under both roots found only `a4` paths and
`__RUNDIR__`.

## (a) spps-gpu --cpu against spps-gpu on the GPU, seed 11

Source: `C:\tmp\nm-a4\runs\a\summary.jsonl` and `<case>-compare.json` (`cpu_gpu.py` with
`SPPS_GPU_EXE=C:\tmp\nm-solvers-a5\spps-gpu.exe`). "walk" counts primary particles whose step count, fate,
final energy bits, child count or child steps differ between the two builds.

| case | band | particles | steps traced | walk | fates equal | double sums, max rel. of peak | trace s CPU / GPU |
|---|---|---|---|---|---|---|---|
| trans | 500 / 1000 | 200,000 | 1,270,560,239 / 507,202,765 | **0 / 0** | yes | 1.5e-14 / 1.4e-14 | 6.57 / 6.43 |
| trans-random | 500 / 1000 | 200,000 | 8,048,690 / 7,547,794 | **0 / 0** | yes | 3.7e-16 / 4.9e-16 | 0.11 / 0.13 |
| trans-a1 | 500 / 1000 | 200,000 | 54,230,131 / 35,411,180 | **0 / 0** | yes | 1.4e-14 / 1.4e-14 | 0.38 / 0.37 |
| beam | 500 / 1000 | 2,000 | 43,304,000 / 18,630,000 | **0 / 0** | yes | 2.8e-15 / 1.8e-15 | 0.36 / 0.16 |
| beam-a1 | 500 / 1000 | 2,000 | 368,000 / 194,000 | **0 / 0** | yes | 1.8e-15 / 1.6e-15 | 0.03 / 0.05 |
| onesided | 500 / 1000 | 400,000 | 241,095,258 / 241,104,048 | **0 / 0** | yes | 2.6e-14 / 4.3e-14 | 1.73 / 1.79 |
| onesided-random | 500 / 1000 | 400,000 | 16,911,969 / 16,816,942 | **0 / 0** | yes | 4.2e-16 / 4.3e-16 | 0.19 / 0.20 |
| beam-onesided | 500 / 1000 | 4,000 | 4,260,000 / 4,260,000 | **0 / 0** | yes | 4.0e-15 / 2.3e-15 | 0.08 / 0.06 |

**Arm (a) holds in all 8 cases:** 0 walk mismatches in 2.82 M primary particles and 2.47 billion steps,
and every double sum within 1e-12 (worst 4.3e-14). Both builds drop the same children: the queue is in
the shared `walk.h`. In `trans` the GPU is no faster than 28 CPU threads (6.43 against 6.57 s), because
a family's children run one after another in one slot.

## (b) spps-gpu against SPPS, seeds 101 and 202, through `simpa run-folder --device cpu|gpu`

**The noise band** is A2's method (`analyse.py`). For each receiver, band and parameter (SPL, EDT, T20,
T30, C80), two values agree when |a - b| <= sqrt(ha² + hb²). Each h is half the range `simpa results`
reports with the value: 2.5 Monte-Carlo standard deviations, or EDT's own method range. Two controls set
the scale: SPPS s101 against SPPS s202, and spps-gpu s101 against s202. `a4_bias.py` adds the signed
mean and rms of d/range over receivers and both seeds, which shows a shift that stays inside every
range; pure noise gives a mean near 0 and an rms near 0.4. Counts include each receiver's per-source
echogram, which repeats the sum when a case has one source.

| case | gpu s101 vs spps s101 | gpu s202 vs spps s202 | control spps vs spps | control gpu vs gpu |
|---|---|---|---|---|
| onesided | 165 of 168 (worst 1.14) | 160 of 168 (1.89) | 158 of 168 (1.58) | 162 of 168 (1.61) |
| onesided-random | 165 of 166 (1.05) | 166 of 166 (0.72) | 166 of 166 (0.95) | 166 of 166 (0.82) |
| beam-onesided | **every .recp step bit-identical** (24 of 24 series) | the same | - | - |
| trans-a1 | 152 of 152 (0.90) | 152 of 152 (0.96) | 150 of 152 (1.24) | 150 of 152 (1.08) |
| trans-random | 124 of 126 (1.04) | 124 of 124 (0.67) | 124 of 124 (0.60) | 124 of 124 (0.81) |
| beam-a1 | **every .recp step bit-identical** (16 of 16 series) | the same | - | - |
| **trans** | **144 of 152 (1.62)** | **150 of 152 (1.03)** | **152 of 152 (0.97)** | **152 of 152 (0.92)** |
| **beam** | **0 of 16 series identical; first differing step 72-106; totals -2.45 to -3.26 % (500 Hz), -0.011 to -0.032 % (1 kHz)** | the same (no random number is drawn) | identical | identical |

`onesided`: the values outside are T20 and T30 in room 2, in every pair, controls included. Room 2's decay
there is short and noisy: it loses its energy into room 1 and gets none back. Signed bias, gpu - spps
against the controls (mean, rms), for T20 at 500 Hz is +0.20, 0.83 against -0.08, 0.75 and -0.09, 0.56.
For T30 at 1 kHz it is +0.07, 0.47 against -0.25, 0.80 and +0.10, 0.36. Every other parameter's mean is
within ±0.13. **No bias.**

**`trans` shows the divergence.** The 8 values outside at s101 and the 2 at s202 are all decay values in
room 1: T30 at R1b, R1c and R1d, T20 at R1b at 500 Hz, and EDT at R1b at 1 kHz. Signed bias at 500 Hz is
**T30 -0.66 (rms 0.82)** against controls -0.16 and +0.08, and **T20 -0.44** against -0.16 and +0.17.
SPL is unbiased (+0.02 and -0.01). The raw T30 at 500 Hz:

| receiver | SPPS s101, s202 | spps-gpu s101, s202 |
|---|---|---|
| R1a | 0.5085, 0.5083 | 0.5055, 0.5054 |
| R1b | 0.5101, 0.5087 | 0.5047, 0.5045 |
| R1c | 0.5073, 0.5073 | 0.5059, 0.5027 |
| R1d | 0.5107, 0.5092 | 0.5040, 0.5061 |
| room 1 mean | **0.5088** | **0.5049 (-0.77 %)** |
| room 2 mean (R2a-d) | 0.5788 | 0.5778 (-0.16 %) |

Each range is about ±0.003 s. Every one of the 8 spps-gpu values in room 1 lies below every one of the
8 SPPS values. The particle statistics show the cause (run.json, 500 Hz, s101): SPPS ran 14,459,245
particles, 72 per source particle with the children counted, and spps-gpu ran 8,487,515.
spps-gpu's stderr reported `child_queue_overflow: 3968312 transmitted particles dropped at 500 Hz ...
energy 6.87536e-09 J` and 104,146 at 1 kHz. That is 2.1e-4 and 3.7e-7 of the source's energy
(3.3386e-5 and 6.6614e-5 J) at the moment each child was dropped. The dropped children are the deep,
late generations, which is why the late decay in room 1 (fed back from room 2) moves and the level does
not.

Elapsed time, from run.json, SPPS against spps-gpu: `trans` 111.0 against 8.3 s, `onesided` 23.1 against
2.0 s, `beam` 23.2 against 0.3 s (at 500 Hz the beam's spps-gpu runs 81 particles per family against
SPPS's 2,542).

## (c) The physics

### Transmission, exact: the beam (`a4_physics.py`, `beam_model`)

The beam is specular and unidirectional with no air absorption, so every particle follows one path and
draws no random number. Each pass through an on-axis receiver adds E x chord, the same chord for every
receiver. Write a = 1 - alpha_panel = 0.7, b = 1 - alpha_wall = 0.8 and tau = 10^(-TL/10).

**First arrivals, which test tau directly.** The first transmitted child carries E0·tau
(`CalculationCore.cpp`, energetic branch: `configurationPTransmise.energie *= tau`). Every ratio is
relative to B1b's first arrival, and the step windows are measured / model:

| quantity | expected | SPPS = spps-gpu, 500 Hz | 1 kHz | steps |
|---|---|---|---|---|
| B2a first / B1b first | tau = 0.1 / 0.01 | **0.1000021** | **0.0100002** | 13-15 / 13-15 |
| B1b second (panel reflection) / first | a = 0.7 | 0.7000018 | 0.7000018 | 13-15 / 13-15 |
| B2a second (end wall) / B2a first | b = 0.8 | 0.8000070 | 0.8000070 | 33-34 / 33-34 |
| B2b first / B1b first | tau | 0.0999994 | 0.0099999 | 19-21 / 19-21 |

The first three arrivals at all four receivers land in the model's step windows, with energies within
3.0e-5 of the model's. The residual is common to both solvers and to every case. It is the float
geometry of the chords (the solvers' positions are float).

**Totals against the exact truncated model.** `beam_model` follows every particle of one family with
SPPS's energetic rules. A wall hit multiplies E by b. A panel hit makes a child of E·tau when
E·tau > eps and then multiplies the parent by a. A particle dies when E <= E0·10^-7. The model
reproduces **SPPS's own particle count per family: 2,542 at 500 Hz and 119 at 1 kHz**
(run.json 5,084,000 and 238,000 for 2,000 source particles). The closed-form infinite series is also
given (no truncation; J2 = tau / ((1 - ab)(1 - g²)) enters room 2, g = tau·b / (1 - ab)).

| receiver, 500 Hz | series (no truncation) | model | SPPS / model - 1 | spps-gpu / model - 1 | spps-gpu / model with a 16-child queue - 1 |
|---|---|---|---|---|---|
| B1a | 3.03846 | 3.03743 | +8.3e-6 | **-3.26 %** | +8.3e-6 |
| B1b | 4.03846 | 4.03743 | +3.0e-6 | **-2.45 %** | +3.0e-6 |
| B2a | 0.961538 | 0.960561 | +1.65e-5 | **-2.59 %** | +1.65e-5 |
| B2b | 0.961538 | 0.960561 | -3.6e-6 | **-2.59 %** | -3.7e-6 |
| **1 kHz** B1a / B1b / B2a / B2b | | | +8.3e-6 / +2.9e-6 / +1.65e-5 / -3.7e-6 | -0.031 / -0.023 / -0.009 / -0.011 % | within 1.7e-5 |

Steady-state ratio across the panel, B2a over B1b: SPPS 0.237917 at 500 Hz against 0.238095 for the
series, and 0.0240545 at 1 kHz against 0.0240616. Under SPPS's rules this ratio is not tau: it is
J2(1 + b) / [(1 + a) + J1(1 + b)], with the panel's reflection a and the walls' b. The series gives the
same ratio within 0.08 % (500 Hz) and 0.03 % (1 kHz), and the eps truncation accounts for the
difference: each of the 2,541 children dies with up to E0·1e-7 left.

**The divergence, exactly.** The model with a 16-child queue drops **383 children per family at
500 Hz and 72 at 1 kHz**. spps-gpu reports 766,000 and 144,000 for 2,000 families: the same count. The
model's dropped energy is 0.0251569 E0 per family, against spps-gpu's reported 8.39888e-7 J /
3.3386e-5 J = 0.0251568. The queue depth SPPS reaches is **1,192 at TL 10 dB and 86 at TL 20 dB**. The
first SPPS/spps-gpu step that differs is step 72 at B1b, 80 at B1a, 101 at B2a and 106 at B2b.

**alpha = 1 (`beam-a1`).** The parent itself passes with E·tau, and no child is made. B2a first / B1b
first is 0.1000021 and 0.0100002, and every total matches the model within 2.5e-5. SPPS and spps-gpu are
bit-identical at every step.

### Transmission, diffuse: the omni cases against the two-room balance

The reference is the two-room steady-state energy balance, the relation behind ISO 10140-2 and ISO
16283-1 (R = L1 - L2 + 10 lg(S/A2)): **E2/E1 = tau·S / A2**. Here A2 = sum(alpha·S) over room 2's walls
(108 m2 x 0.2), plus alpha_panel·S (the panel's alpha includes the transmitted share), plus 4mV2
(ISO 9613-1: m = 6.3e-4 and 1.07e-3 1/m at 500 and 1,000 Hz). L1 - L2 is 10 lg of the mean time-integrated
energy over R1a-d divided by that over R2a-d. The relation assumes diffuse incidence everywhere and
ignores the direct sound. A refined form takes the two direct terms out of the measurement instead of
modelling them:
- each R1 receiver's direct pulse is removed from E1 (its echogram from 1 step before the direct arrival
  to 2 steps after, before any reflection can arrive);
- the same pulse gives the source's w/c through D(r) = (w/c)/(4πr²);
- the panel's direct share, Ω/4π = 0.0766, is added to its incident power.

| case, band | plain prediction | refined | SPPS s101, s202 | spps-gpu s101, s202 | deviation from refined |
|---|---|---|---|---|---|
| trans 500 Hz (TL 10) | 11.80 dB | 11.50 dB | 11.416, 11.408 | 11.397, 11.411 | -0.08 to -0.09 dB |
| trans 1 kHz (TL 20) | 21.82 dB | 21.52 dB | 21.461, 21.430 | 21.436, 21.488 | -0.04 to -0.09 dB |
| trans-random 500 Hz | 11.80 | 11.50 | 11.263, 11.427 | 11.488, 11.328 | -0.23 to 0.00 dB |
| trans-random 1 kHz | 21.82 | 21.52 | 21.740, 21.181 | 21.348, 21.401 | -0.34 to +0.23 dB |
| trans-a1 500 Hz | 13.45 | 12.97 | 12.271, 12.335 | 12.360, 12.324 | -0.59 to -0.70 dB |
| trans-a1 1 kHz | 23.47 | 22.98 | 22.289, 22.333 | 22.290, 22.462 | -0.53 to -0.71 dB |

The energetic case agrees with the refined balance within 0.1 dB. Random mode scatters ±0.3 dB, because
it carries fewer surviving particles. `trans-a1` reads 0.5-0.7 dB short in both solvers alike: its panel
absorbs everything from room 2's side as well, so room 2 holds a strong sink next to the transmitting
face (mean alpha 0.32), and the uniform-E2 assumption fails. That is a limit of the formula, not of either
solver. The exact test of the alpha = 1 rule is `beam-a1`. **The level difference cannot see the queue's
divergence:** the dropped children shift the late decay, not the integrated energy.

### One-sided faces

- **`onesided` and `onesided-random`, omni.** Srefl's energy at R2a, R2b, R2c and R2d is **0.0, in 0 of
  3,000 steps, in both bands, in all 8 runs** (SPPS and spps-gpu, seeds 101 and 202, energetic and
  random). Spass reaches room 1. Its mean energy over R1a-d divided by that over R2a-d is 0.870-0.882
  (SPPS) and 0.859-0.880 (spps-gpu) in energetic mode, and 0.873-0.879 and 0.860-0.894 in random mode.
- **`beam-onesided`, exact.** Srefl, aimed +y at the panel's reflecting side: B2a and B2b are 0 in every
  step. B1b's total over its first arrival is 5.0000148 against (1 + a)/(1 - ab) = 5, and B1a over B1b's
  first is 4.0000327 against 4. Spass, aimed -y at the passing side: B2a sees **exactly one pass**, B2b 0.
  It crosses without loss: B1b's first arrival over B2a's is 0.9999727. Room 1 then traps it: B1b and B1a
  total 4.99948 and 4.99987 times B2a's pass, against (1 + b)/(1 - ab) = 5. Every one of the 24 echogram
  series (4 receivers, sum and 2 sources, 2 bands) is bit-identical between SPPS and spps-gpu, at both
  seeds.

## Seen on the way

- **The run manager calls an overflowing run OK.** spps-gpu's `child_queue_overflow` line on stderr is
  classified as `unclassified_line`, a warning. The verdict stays `{"status": "OK"}`, and `simpa results`
  reads the run as verified. A user running `trans` on the GPU is not told that, at 500 Hz, 41 % of
  SPPS's particles were never run.
- **The GPU gives no speed on a transmission-heavy family** (`trans`: a 6.43 s GPU trace against 6.57 s on
  28 CPU threads), because one slot runs a whole family in sequence. End to end it is still 13x faster
  than SPPS (8.3 against 111.0 s), partly because it runs 41 % fewer particles.
- The plain two-room formula is off by 0.1-1.2 dB on these rooms, and the bias comes from the direct
  sound (see above). Use the refined form, or the beam, to judge a solver.
- `analyse.py` counts a single-source receiver's per-source echogram as a second value, so the "within"
  counts above double-count those rows. `a4_bias.py` leaves them out.
- The exported `mesh.cbin` writes the y = 0 coordinates as -0.0 (`80000000`). It is harmless here: both
  solvers read the same file, and the normals come out exact.

## Untested after A4

- **The fix**: a deeper or spilling child queue, and this bed's `trans` and `beam` cases rerun against it.
  Not done in this step, by the spec.
- How often real projects overflow: CR4 has no transmitting surface, and no Elmia or tutorial-3 run with
  `--device gpu` has been counted.
- A **single-sided transmitting** face, where the reflecting side transmits and the other side passes
  freely. The two rules were bedded apart, not together.
- A surface receiver on a transmitting or one-sided face. This is hazard 4, the in-place normal flip,
  which A2 left open. These cases carry no surface receivers.
- Grazing incidence on a one-sided face, with dir . n in (-1e-4, 0]: such a particle is not passed but
  collides as if it came from the reflecting side.
- A transmitting face with fittings (`faceEncombrement`), direct-field-only runs with internal faces, and
  more than two bands.
- Random mode with alpha = 1 on a transmitting face: not run on its own (the omni random case uses
  alpha 0.3).

## Files and commits (branch `a4`)

- `solvers/spps-gpu/bed/a4_cases.py`: the eight cases' projects. `a4_bed.py`: arms (a) and (b).
  `a4_physics.py`: arm (c), the beam event model, the QCAP-16 model and the two-room balance.
  `a4_bias.py`: signed bias. `stage.py`: the A4 output roots. `cpu_gpu.py`: `SPPS_GPU_EXE`.
- Receipts (gitignored):
  - `.out\a4\projects\`, `.out\a4\mesh-runs\`;
  - `.out\a4\bed\bed-main.jsonl`, `bed-bos.jsonl`, `bed-ba1.jsonl` (one line per run, with its run folder);
  - `.out\a4\bed\analysis.{json,md}`, `bias.json`, `physics.{json,md}`;
  - `C:\tmp\nm-a4\runs\a\summary.jsonl` and `*-compare.json`.
- Commits: `ae48197` the cases and the driver; `98955a6` the beam references and the physics arm;
  `588db1b` the QCAP-16 model and the bias table; this report.
