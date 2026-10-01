# M8b EDT held-out test: plan for the harness

Draft, 2026-10-01 02:00, branch `m8b-edt`, revised the same night for the crucible's five findings and, at 04:40,
for three found by step 3's test writer (section 8).
It plans the code that will run `PREREG.md`'s test. Building it produces no test data. One step, taken as soon as
the rooms and the solver driver exist, times one run in a stand-in room that is not a held-out room.

**In plain words.** The test needs code that does not exist yet: seven new rooms, a driver for the solver runs,
two noise-free generators, a kit for the attacker, and a scorer for H1-H6. This plan builds that code with its
tests written first. As soon as the rooms and the driver work, it times one 1M-particle, 0.1 ms run in a
stand-in for the slowest room, so that Burhan can decide on the long stretch with a measured number while the
rest is built. Then it checks the code against numbers the repo already has, and on fake data with planted
faults. No run of the test itself happens here.

Line references are to `PREREG.md` unless a file is named. Paths are relative to this worktree unless absolute.

## 1. What PREREG.md already fixes

- **Freeze.** Nothing may change once the held-out data is generated. A failed method is fixed and tested again
  on a different fresh set (:3-4).
- **Method.** `frozen/method.py`, sha256 `462c37cf…`, Z = 2, MIN_POINTS = 8, TAIL_SHARE = 0.02, HW_FLOOR = 0.005
  (:8-9). The hash is verified before every run, and a mismatch voids the run (:10). Z = 3 is reported by changing
  only `Z` and does not decide (:11). The hash also pins what the text leaves unnamed: JND 0.05, C = 343.2, the
  default ball R = 0.31 m, the 20 % tail blocks, `analyse(bins, dt, t_arrival, meta)` and the statuses ok, wide
  and refused (`frozen/method.py:8-9, 14-20, 27, 69-70, 90`).
- **Comparator.** Upstream's EDT, through the validated port in `target/agents/upstream-edt/`, on the same rows (:12).
- **Held-out rule and corpus list** (:14-18).
- **SPPS-fresh.** At least 6 rooms with the listed features (:20-26). Tested runs: 1, 2 and 5 ms steps, 150k
  particles plus 50k in one room, 3 seeds, and SPPS's default run length, recorded (:28-32). Truth: the sum of
  K ≥ 4 references, each ≥ 1M particles at 0.1 ms and run to −60 dB, read with the corpus's truth function,
  which the evaluator names (:34). Truth uncertainty is the spread over √K, and rows above 1 % are excluded and
  counted (:35).
- **ISM-fresh.** New seeds, rooms disjoint from the ISM sample, and an image set of at least the run length
  plus 20 % (:36).
- **Synth-fresh.** Rate ratios 1.5 and 5, DRR −20 to +10 dB, steps 1/2/5/10 ms, runs of 0.3-3 T60 (:37-41),
  noise-free (:55).
- **Attack.** The attacker has seen only `frozen/method.py`, its interface and the physics (:42).
- **Definitions, bar, refusals, who decides.** ok, usable, wrong-silent (> 5 %) and covered (:46-49); H1-H6
  (:55-60); refusals are never wrong and are reported by reason and set (:62-63); the criteria are a technical
  call, while Z and short runs stay Burhan's (:67-68).

## 2. Open parameters, fixed here

This section is the draft of `ADDENDUM-A1.md`. Following the noise-calibration precedent
(`2026-09-25-noise-calibration/PREREGISTER.txt:75-76`), it is committed before the first held-out row exists.
**T** marks a technical call (decision row 13). **B** marks a call that is Burhan's because it changes what users see.

### 2.1 Freeze and scope

| # | Parameter | Fixed as | Reason | Call |
|---|---|---|---|---|
| P1 | When the freeze starts | At the first held-out row of any set. Before that, gaps are filled only by dated addenda | The conservative reading of :3. The precedent filled its gaps the same way | T |
| P2 | Corpus to keep clear of | :16-18 widened to: all four ISM rooms (t1, corridor, dead, deader) and their radius variants; all 32 noise-cal cells, including W-E6 and the 10 ms cells; the critique's double-slope ratios of 3 **and 4**; z3's 105 distinct boxes (230 confirmed cases); M8a's rooms. The list goes in `harness/corpus_rooms.json`, with the hash of each source | The text understates the corpus: `ism_rows.pkl` holds 4 rooms, `tail_blocks.py` read W-E6, and `scan_i4.py:21` uses ratio 4. Widening the list can only remove candidates | T |
| P3 | "Not in the corpus", for a room | Two boxes are near-duplicates when each sorted dimension is within 10 %. Two non-box rooms are near-duplicates when their sorted bounding boxes, volumes and surface areas are each within 10 %. A box and a non-box room are not near-duplicates. A room is fresh when it is a near-duplicate of no corpus room. `corpus_rooms.json` records each corpus room's kind, box or not, from its source, and T10 applies the rule to every pair | Simple and checkable. The five fresh boxes clear it by 17.1-31.25 %: against its nearest corpus box, each one's largest per-axis difference is 30.4 % for F1 (z3's box028), 22.0 % for F2 (box096), 17.1 % for F5 (box055, tied with four other z3 boxes), 31.25 % for F6 (M8a's 20 × 8 × 4) and 24.0 % for F7 (the five 5 × 4 × 3 m corpus rooms, ISM's dead among them), as `rooms.p3_clearance` computes it. A partition or a corner changes the decay, which is why F3 and F4 exist. That every corpus room is a box is checked, not assumed. Today it holds: ISM (`ism.py:1`, `attack_ism.py:217-223`), z3 and z3grid (`z3-verdict.md:147`), the noise-cal cells and real rows (`noise_calibration.rs:61`, `PREREGISTER.txt`'s cell tables), M8a (`bed/file.rs:43`); the scans and t1 have no geometry | T |
| P4 | Data seen before A1 | The frozen method runs only on corpus inputs and planted fixtures. Draws from the fresh families and rooms go through a stub. M8a's 433 runs are never read by the method | This keeps A1's choices blind. M8a has no independent truth and different settings (`HANDOFF-2026-10-01.md:63`) | T |

### 2.2 SPPS-fresh rooms

| Room | Geometry (m) | α (every band), scattering | Design T60, 125 Hz to 4 kHz (s) | Role |
|---|---|---|---|---|
| F1 | box 3.4 × 2.9 × 2.5 (24.7 m³) | 0.15, Lambert 1 | 0.48-0.44 | small room (V ≤ 30 m³) |
| F2 | box 17 × 12.5 × 8 (1,700 m³) | 0.08, Lambert 1 | 3.63-2.26 (3.46 at 500 Hz, 3.33 at 1 kHz) | T60 ≥ 2.5 s; longest T60 |
| F3 | main 9 × 7 × 4, a 0.2 m partition with a 2.0 × 2.5 m doorway at y 2.9-4.9, z 0-2.5 (as `rooms.py` builds it, so that the far receivers are through and blocked as listed below), chamber 6 × 7 × 4 | main room 0.40, its partition face included; chamber, doorway reveals and the partition's chamber face 0.04; Lambert 1 | late 2.11-1.56, early 0.30-0.29 (the main room by the same formula, its doorway also at α = 1) | non-uniform absorption, double slope, coupled |
| F4 | L: arm 16 × 5 × 3.5, plus arm x 11-16, y 5-15, h 3.5 | 0.12, Lambert 1 | 1.20-1.00 | L-shaped; blocked receiver |
| F5 | box 14 × 9.5 × 6 | 0.20, scattering **0.2** | 1.05-0.89 | mostly specular, like real finishes |
| F6 | box 24 × 5.5 × 3.2 | 0.30, Lambert 1 | 0.42-0.39 | long room at high absorption, where the noise model failed (noise-cal V4-E1/E4); the 50k runs |
| F7 | box 3.8 × 3.3 × 2.4 (30.1 m³) | 0.45, Lambert 1 | about 0.14 at 500 Hz (Eyring; `rooms.py` writes the bands) | small dead room. Added 2026-10-01 02:40, before any data: none of F1-F6 is the kind of room where 13 of the method's 19 real-seed wrong-silent rows sit (C-R3, 5 × 4 × 3, α 0.4). Sorted, it is 18-24 % off C-R3 on every axis. Its source and receivers are F1's scaled per axis (× 3.8/3.4, 3.3/2.9, 2.4/2.5), except the one receiver that scaling puts 0.576 m under the ceiling, which is lowered to 1.80 m. They are listed below, to the millimetre; `rooms.py` writes them and T10 checks them |

Sources and receivers, with distance in metres (near < 2, far > 10). Every point is at least 0.6 m from every
surface, and this has been checked. F1 and F7 have points at exactly 0.6 m, so a check of this allows 1e-9. A
receiver is blocked when its straight segment to the source leaves the room.

```
F1 src (0.8,0.8,1.3)   near (1.6,1.2,1.2) .90 (2.0,0.9,1.5) 1.22 (1.3,2.1,1.1) 1.41 (2.4,1.6,0.9) 1.83
                       mid  (2.7,2.2,1.6) 2.38 (2.8,1.9,1.2) 2.28 (2.6,2.3,0.7) 2.42 (2.8,1.2,1.9) 2.13
F2 src (3.0,4.0,1.6)   near (4.2,4.6,1.3) 1.37 (3.5,5.6,1.7) 1.68 (2.2,5.0,1.2) 1.34
                       mid  (7.5,6.0,1.4) 4.93 (10.0,3.0,2.2) 7.10 (6.0,10.5,1.5) 7.16
                       far  (14.5,9.0,1.4) 12.54 (16.0,11.5,3.0) 15.07
F3 src (2.5,3.5,1.5)   near (3.8,3.9,1.2) 1.39 (2.0,5.0,1.4) 1.58 (3.2,2.0,1.7) 1.67
                       mid  (6.5,2.0,1.2) 4.28 (7.8,5.5,1.5) 5.66 (5.5,6.0,2.2) 3.97
                       far  (14.0,3.6,1.2) 11.50 through the doorway; (13.0,5.8,1.3) 10.75 blocked, grazing the doorway edge
F4 src (2.0,2.5,1.5)   near (3.2,3.0,1.2) 1.33 (2.8,1.2,1.6) 1.53 (1.2,3.8,1.3) 1.54
                       mid  (6.0,3.5,1.4) 4.12 (9.0,1.5,1.7) 7.07 (11.5,4.0,1.2) 9.62
                       far  (14.5,2.5,1.3) 12.50; (13.5,12.0,1.2) 14.92 blocked (the way round the corner is 1.9 m longer)
F5 src (3.0,4.75,1.6)  near (4.3,5.2,1.3) 1.41 (2.4,3.2,1.4) 1.67 (3.5,6.4,1.8) 1.74
                       mid  (7.5,3.0,1.2) 4.84 (9.0,7.5,1.7) 6.60 (6.0,8.5,2.5) 4.89
                       far  (13.0,8.5,1.3) 10.68 (13.2,1.2,1.5) 10.80
F6 src (2.0,2.75,1.5)  near (3.3,3.2,1.2) 1.41 (2.6,1.3,1.6) 1.57 (1.0,3.9,1.3) 1.54
                       mid  (6.0,2.0,1.4) 4.07 (9.5,4.5,1.8) 7.71 (11.0,1.5,1.2) 9.09
                       far  (16.0,3.0,1.3) 14.00 (22.5,2.2,1.6) 20.51
F7 src (0.894,0.910,1.248)  near (1.788,1.366,1.152) 1.01 (2.235,1.024,1.440) 1.36 (1.453,2.390,1.056) 1.59
                            mid  (2.682,1.821,0.864) 2.04 (3.018,2.503,1.536) 2.67 (3.129,2.162,1.152) 2.56
                                 (2.906,2.617,0.672) 2.70 (3.129,1.366,1.800) 2.35
```

| # | Parameter | Fixed as | Reason | Call |
|---|---|---|---|---|
| P5 | Design T60 | Eyring with air: T = 24 ln10·V / (c(−S ln(1−ᾱ) + 4mV)), with m from ISO 9613-1 at 20 °C and 50 % RH (`ism.py` `m_energy`). F3 uses its chamber, with the doorway counted at α = 1, which gives the late slope | Fixed before any data, so it cannot be tuned. It is used by H4's filter (P30), by the reference run length (P17) and to pick the longest-T60 room | T |
| P6 | Confirming the features | After the truth runs, report F2's truth T30 at 1 kHz (≥ 2.5 s expected) and F3's truth T30/EDT at 1 kHz (≥ 1.25 at half its receivers or more). Reported, not gated | :21-22 ask for the features. The design is set to meet them with margin | T |
| P7 | How a room is built | A union of axis-aligned boxes, with its boundary on one global grid so the surface is watertight and conforming. Written as OBJ with one `usemtl` per material, then `simpa import --unit m --up z` (`geometry/import.rs:5-7, 154`), then the materials, source, receivers and settings are set in the JSON | `simpa bed` builds only uniform boxes (`bed/file.rs:43-70`). This uses the import path users use | T |

### 2.3 SPPS runs

| # | Parameter | Fixed as | Reason | Call |
|---|---|---|---|---|
| P8 | Settings the PREREG does not name | The default of a freshly imported project: Random, air on, R 0.31 m, trans_epsilon 5, octave bands 125 Hz-4 kHz, 150,000 particles per source per band (`schema/model.rs:962-973`, `schema/bands.rs:124-131`) | The test then measures what users get | T |
| P9 | Exceptions to the defaults | `random_seed` non-zero, so runs are reproducible with disjoint streams (it forces one thread, `model.rs:926`). Surface and receiver intersection files off. No fittings | Intersection files cost disk at 1M particles and do not touch the histograms | T |
| P10 | Run length of the tested runs | Raised before the test, on Burhan's word (2026-10-01 02:23, "raise the default"), to a fixed 10 s for now (08:13, decision row 36). The dead-room probe showed a longer run costs a dead room no extra time (`PROBE.md`: a 10 s run took P0b no longer than a 2 s run), and 10 s covers the product's T60 range. The rule that sets the length from each room's predicted reverberation time, which he chose at 08:05 (row 35), is deferred to backlog item 57. The product default changes from 2.0 s in its own step, with its own tests and gates, before A1 is committed, and the test takes it | The default is what users see | **B**, decided |
| P11 | Energetic mode | Added, on Burhan's word (2026-10-01 02:23, "include the energetic mode, as long as its verified"): the same 72 tested runs in Energetic mode. The truth runs serve both modes, since the expectations match above energetic's −50 dB kill. Energetic is scored as its own set: EDT is shown for energetic runs only if energetic passes H1-H6 on its own, and Random's verdict does not depend on it. M8a measured energetic at 18× random's time on one room (`HANDOFF-2026-10-01.md:59-60`) | Which modes the verdict covers is a claim users read | **B**, decided |
| P12 | Seeds | 150k: 1101-1103 (1 ms), 1201-1203 (2 ms), 1501-1503 (5 ms). 50k: 2101-2103, 2201-2203, 2501-2503. Truth: 9001-9004. Probe: 9999, reserved | No two runs share a random stream. The truth is independent of every tested row | T |
| P13 | The matrix | 7 rooms × 3 steps × 3 seeds at 150k, plus F6 × 3 × 3 at 50k: 72 tested runs in each mode (P11), so 144, and 6,912 rows (8 receivers × 6 bands × 144). Truth: 7 rooms × K = 4, so 28 runs, shared by both modes | :28-32 and :34 | T |
| P14 | Solver build | `solvers/manifest.json`: upstream `929a5c8e`, spps.exe code sha256 `550485c6…`. Checked before every run as M8a's E1 does (`tools/gates/m8a.ps1:8-9`, `solvers/pe-fingerprint.ps1`), and each report's `solver_build` must read verified (backlog 38, `results/report.rs:899-901`). No CLI or bed report can read verified until backlog 54's CLI half is built (decision row 33; `mesh_run.rs:520` passes no manifest), so the truth and tested runs wait for it. The probes run no `simpa results` (section 7), so only the check before the run applies to them | "Upstream SPPS" names no build (:20). This is the verified one | T |

### 2.4 Truth

| # | Parameter | Fixed as | Reason | Call |
|---|---|---|---|---|
| P15 | Truth function | Definition A: the ISM truth `truth_ideal` (`target/agents/followup-design/skeptic-gf3/attack_ism.py:55-77`, with `rerun/mirror.py:20, 109-146`). Direct sound is a step at t_arr, and reflections sit where the ball records them. For SPPS, the K references are summed. Bins overlapping [t_arr − R/c, t_arr + R/c) count as direct and the rest as reflected. A blocked receiver gets direct = 0, with t at the start of its first bin with energy (the critique's occluded truth, `critique/scan_i1_occluded.py:1-4, 20`). ISM-fresh uses the image-source split, as the corpus did. Synth-fresh uses `critique/synth.py` `truth_edt`, which is the same definition in closed form | t1, z3, ISM and the critique all use Definition A (EVAL.md:16-19). The real set's truth, the edt3 midpoint, is the shipped calculator's own reading and is not independent (EVAL.md:18) | T |
| P16 | References | K = 4. 1,000,000 particles per band at 0.1 ms. Method, air, bands and R as the tested runs | The minimum the PREREG allows. The 1 % screen guards precision | T |
| P17 | Reference run length | min(6.5 s, max(3.0 s, t_arr,max + 2 × the design T60 maximum)), rounded up to 0.1 s: F1 3.0, F2 6.5, F3 4.3, F4 3.0, F5 3.0, F6 3.0. F2's 6.5 s is 65,000 steps, under the 65,536 limit (`validate.rs:287`) | Reaches −60 dB with margin. In Random mode particles die when absorbed, so a longer run should add little cost; the probe measures it | T |
| P18 | "Reached −60 dB" | Per receiver-band, the summed reference's energy in its last 10 % must be ≤ 10⁻⁶ of its total. Otherwise the row is excluded as `truth_truncated` and counted | A check that runs after the data, fixed before it | T |
| P19 | Truth uncertainty | u = SD(ddof 1) of the K individual EDTs, ÷ √K, ÷ the summed truth's EDT. If u > 0.01 the row is excluded as `truth_uncertain`, and a NaN truth as `truth_nan`; both are counted | :35 says "spread". The SD is the standard choice | T |

### 2.5 ISM-fresh and Synth-fresh

| # | Parameter | Fixed as | Reason | Call |
|---|---|---|---|---|
| P20 | ISM generator | `ism.py` (specular box, per-wall α, ball receiver), with the series built as `eval_ism.py:59-88` builds it: air applied per step, from a 0.02 ms fine grid. The truth is `truth_ideal` at 0.02 ms with continuous air | The corpus's validated generator (EVAL.md:19) | T |
| P21 | ISM rooms | 12 rooms. **Four relatives**, one of each corpus ISM room (t1, corridor, dead, deader): each dimension times its own factor from U[1.12, 1.33], each wall's α times its own factor from U[0.9, 1.1], the source scaled with the room. **Eight drawn**: L1 U[4, 24], L2 U[3.5, 14], L3 U[2.6, 7] m; V 60 to 2,500 m³; α per wall U[0.02, 0.80]; design T60 at 125 Hz from 0.1 to 3.0 s. Every room: at most 10⁸ images, and fresh per P3. Drawn by rejection with seed `SeedSequence(2026100102)`, each rejection counted by reason | P33. α reaches the z3 weak spots' 0.05-0.58, and T60 reaches dead's 0.20 s (P5's formula). 3.0 s is H4's edge. Bounds alone are marginal: with α drawn per wall, a room as evenly dead as deader (every wall at 0.5 or more) comes up in about 0.3 % of draws, so every corpus ISM room gets a relative, chosen by room and not by result. The cap is cost: `ism.py` builds every image at once, and the draft's own worst case, 60 m³ at 2.4 s, is 7.9 × 10⁷ | T |
| P22 | ISM receivers and runs | 12 receivers per room: 4 near (d from R + 0.2 m to under 2 m), 4 mid (2-10 m), and 4 far (over 10 m) where the room allows, otherwise 4 more mid. A room allows far receivers when a point 1.54 m (the largest R + 0.04 m) from every wall lies more than 10.5 m from the source. R per receiver log-U[0.1, 1.5] m, drawn first. The receiver then lies uniformly over the points of its class that are at least R + 0.04 m from every wall, and an R that leaves its class no such point, or only a sliver, is drawn again, counted. The source at least 0.7029691338539124 m from every wall: uniformly over those points in a drawn room, its parent's scaled with the room in a relative (P21). Bands 125 Hz to 16 kHz in octaves, plus 20 kHz. Steps 1, 2 and 5 ms, as SPPS-fresh (:29), whose noise-free twin this set is. Run 2.0 s. Image set to max(1.2 × run, t_arr + 1.1 × the design T60 maximum), t_arr the room's latest arrival, 2.4 s to about 3.4 s. Truncation check as P18, on the image set. 3,888 rows | P33: the weak spots sit at 20 kHz (dead) and at R 1.13-1.49 m with d − R down to 0.27 m (z3); the corpus grid put receivers R + 0.04 m from the floor (`attack_ism.py:230-234`). P33 widened the draft's source clearance of 1.0 m to exactly the weak-spot value under it: z3grid's dis-010 at 1 ms, z3's dis-010 geometry (box032) with its source 0.703 m from a wall, which z3 itself ran at 0.5 ms. z3's dis-035, dis-016, dis-012 and dis-029 sit at 0.90-0.93 m. Every other binding value lies inside these ranges. 4 receivers per class × 9 bands gives subgroups of 36 rows, so H3 can act. The run length matches SPPS-fresh | T |
| P23 | Synth families | Double slopes only, k2 = k1/1.5 and k2 = k1/5. The late part's share of the reverberant energy is U[−20, −3] dB (the critique's parametrisation, `scan_i4.py:20-23`, with new values). No single slopes. For H3, "family" means the ratio | Single slopes and ratios 3 and 4 are in the critique's scans | T |
| P24 | Synth draws | T60 of k1 log-U[0.1, 10] s. DRR U[−20, +10] dB, with Ed = S_rev·10^(DRR/10) as in the critique. R log-U[0.1, 1.5] m, passed as `half_width` = R/c. d U[0.1716, 30.8022] m, redrawn while d < R − 0.1384 m. Reflection gap U[0, 40] ms. Source delay: none on half the rows, U[0, 60.00000284984708] ms on the rest, emitted at the next whole step as SPPS does (`spps.rs:49-52`), with `t_arrival` = emission + d/c as Night Mode's `arrival_s` (`spps.rs:308`). Run = emission + t_arr + f·T60, f U[0.3, 3]. 500 rows per family × step, so 4,000 rows. Seed `SeedSequence(2026100101)` | :37-41 and P33. The z3 weak spots inside these steps have balls of 1.13-1.49 m, d − R of 0.27-0.39 m and delays of 15-60 ms. The corpus scans reached T60 8 s and gaps of 40 ms (`run_scans.py:61, 84`). 0.1-10 s is `PHYSICS.md`'s T60 limit. 500 per subgroup makes the 0.5 % bar of H1 mean something. P33 widened three of the draft's bounds (d 0.4-30 m, d ≥ R + 0.2 m, delay ≤ 60 ms), each exactly to the weak-spot value outside it: d and d − R for scan 1 as the critique first ran it (`scan_i12.json`, no d cutoff), whose arrivals sit 0.05 or 0.95 of a step into it, so d 0.1716 m with R 0.31 m, and 30.8022 m; the delay for z3's dis-012, dis-016 and dis-029, delayed 60 steps of a float32 1 ms step. The PREREG fixes none of these ranges, and the product accepts every value (P25's limits name only T60 and DRR, and validation has no rule on a receiver's distance from a source) | T |
| P33 | How P21-P24's open bounds are set | `corpus.py` writes `weak_spots.json`: every corpus row on which the frozen method is wrong-silent, or ok and more than 2.5 % off (half the JND), from its logged results (`final/res_*.pkl`) or, for the scans, recomputed on their corpus inputs (P4). For each quantity a fresh set draws, its range contains that quantity's value in every weak-spot row that carries it, unless the PREREG fixes that range, the row's step lies outside the range of the set's steps, or the value is outside `PHYSICS.md`'s limits. Synth-fresh is bound by every row. ISM-fresh is bound by the rows of sets that have a room (ISM, z3, z3grid, the real rows), since a scan's V and d are parameters of a formula, not of a room. A bound may go wider. A cost cap is a stated number, and its rejections are counted in the report. T22 checks every bound; a bound that fails is widened, and the list is never trimmed | The draft's bounds avoided the known weak spots (section 8, finding 1). A list written by a script from the method's own results leaves the authors nothing to steer. Today it holds 10 z3 rows (balls of 0.88-1.49 m, near receivers, delayed sources, `res_z3.pkl`), 2 ISM rows (dead, 20 kHz, 1 ms, `res_ism.pkl`), scan 1's one wrong row, the noise scan's 16 and the real seeds' 19 (`scans_all.log:10, 59, 62`), whose noise and particle counts no noise-free set draws; the scans' near misses are counted when it is written. SPPS-fresh's rooms are designed for :20-26, not drawn, and P33 does not reach them | T |

### 2.6 Attack

| # | Parameter | Fixed as | Reason | Call |
|---|---|---|---|---|
| P25 | Attacker | A fresh subagent working in `C:\tmp\m8b-edt\attack\`, which holds only `frozen/method.py`, `INTERFACE.md`, `PHYSICS.md` (SPPS histogram physics, plus hard limits: what the product accepts, T60 0.1-10 s, DRR −40 to +30 dB) and the generator API (synth, ISM and compound-Poisson noise, `synth.py:103-106`, with the source delay the product has, `model.rs:551`). Its transcript is checked afterwards. Any read outside the folder, or of the tuning logs, voids the attack, and a new attacker is run. At most 30 classes | :42 | T |
| P26 | Classes, instances, judges | A class is JSON: generator, a box of parameters, noise, step, run length and R. A class out of bounds is rejected with its reason. The harness draws 20 instances, seeded by the sha256 of the class, and computes their truth itself. A class is reproducible if 5 or more of its 20 draws are wrong-silent. Plausibility is judged before any class is run, by three fresh subagents, each alone, each given only the class files and `PHYSICS.md`. Each answers plausible or implausible for every class, with reasons. A class is implausible only if all three say so; a split, a missing answer or one that does not parse counts as plausible. Each judge runs once, with no re-run, and every vote and reason goes in the report with the panel's agreement. H6 fails if a reproducible class is plausible | :60 leaves "class", "reproducible" and the judge open. Judging before any run keeps the judges blind by construction. Needing all three means a judge's noise can only fail H6, never pass it, and a split vote records that a re-run might have gone the other way | T |

### 2.7 Scorer

| # | Parameter | Fixed as | Reason | Call |
|---|---|---|---|---|
| P27 | Rows and denominators | Rows excluded for their truth (P18, P19, NaN) leave every truth-based count (wrong-silent, coverage, H1-H3, H5) and are counted by reason. They stay in the usable and ok shares and in H4, which use no truth. A ratio whose denominator is 0 fails its criterion | Conservative. A set that answers nothing cannot pass | T |
| P28 | Covered, wrong-silent | Covered: edt_lo·(1 − 1e-9) ≤ truth ≤ edt_hi·(1 + 1e-9), the evaluator's tolerance. Wrong-silent is strict: \|edt/truth − 1\| > 0.05 | :48-49 | T |
| P29 | H2, H3 | H2 pools SPPS-fresh. H3 measures distance on the straight line between source and receiver: near < 2, mid 2-10, far > 10 m. It pools over band, particle count and seed (SPPS) or over band (ISM). "≥ 20 ok rows" counts ok rows with a truth | :56-57 | T |
| P30 | H4 | Each set separately, SPPS-fresh and ISM-fresh, must reach ≥ 90 %. Rows at 1 ms whose band's design T60 (P5) is ≤ 3.0 s. The ok share is reported | The stricter reading of :58 | T |
| P31 | H5 | On each of SPPS-fresh, ISM-fresh, Synth-fresh and Attack, over the same eligible rows. 0 against 0 is not strictly lower, so it fails | :59, read literally | T |
| P32 | Z = 3, upstream | Z = 3 comes from a second instance of the verified file, with `Z = 3.0` set after import, so the file is unchanged (`final/dev/z3_sensitivity.py:5-6`). Every table is repeated for it, and none decides. Upstream is the port `uphunt_upstream_edt.py` (sha256 `57f391e6…`), not the docs copy (`1d12623f…`). It is ok when its value is finite and positive, with a point range (EVAL.md:9) | :11-12 | T |

## 3. Harness components

The code goes in `harness/` beside this file, as a package `m8b` with numpy and pytest. Everything copied from
gitignored `target/` comes in by text, and `m8b/provenance.json` records the source hash of each copy. Scratch
and all output stay on C:, under `C:\tmp\m8b-edt\`. B: is exFAT with 128 KB clusters, and a test run once
leaked 368k temp files there.

| Component | Does | Reuses |
|---|---|---|
| `corpus.py` | P2, P3, P33. Writes `corpus_rooms.json` (each corpus room's geometry and kind, from its source) and `weak_spots.json`, each with its sources' hashes | `attack_ism.py` `ROOMS`, the noise-cal cell list, `bed/file.rs`'s M8a rooms, z3's `judge_results.json`, `final/res_*.pkl`, the scans' constructions (`final/run_scans.py`) |
| `method.py` | sha256 gate, loading, the Z = 3 instance | `critique/common.py:6-7` loader |
| `upstream.py` | P32 | `uphunt_upstream_edt.py` copied. Upstream's 2019 tutorial-1 `.gabe` files and `oracle.json` become fixtures (`uphunt_oracle.py`). The wrapper is the old evaluator's |
| `truth.py` | P15-P19 | `truth_ideal` (attack_ism.py `3fb8a84b…`), plus `line`, `integral` and `MIN_REGRESSION_BINS` (mirror.py `c5603d5b…`), copied |
| `rooms.py` | P3, P5, P7, F1-F7, and P0 and P0b for the probes (section 7). Writes the nine projects and the design T60s | The material, source and receiver prototypes from `tests/fixtures/rooms/tutorial1_box.simpa`, handled as `bed/file.rs:700-782` does. Canonical JSON edits and `simpa validate` exit 0, as `tools/fixture-gen/mkrooms.py:13-18` does. `simpa import` |
| `driver.py` | The matrix and a plan-only mode. A pool of jobs. `simpa run … --json`, then `simpa results … --json` into `report.json`, with stderr kept and classed. The solver check (P14). A reader for bins (`energy_pa2`), `arrival_s`, R/c and dt. A probe mode that records section 7's numbers and opens no output file. Guards: held-out seeds refused before A1 is committed, seed 9999 refused, folders outside the data root refused | The `simpa bed --jobs` model. M8a's E1 and N1 (`m8a.ps1:8-9, 16`). The old real loader's field access. `run.json` `outcome.elapsed_ms` |
| `ism_fresh.py` | P20-P22, P33 | `ism.py` copied (`a7c9d41e…`) and `eval_ism.py:59-88` |
| `synth_fresh.py` | P23, P24, P33 | `critique/synth.py`, committed, with its hash pinned |
| `attack.py` | P25-P26: the sandbox, schema, draws, the judges' prompt and panel rule, and H6 | The synth and ISM generators, `noisy` |
| `score.py` | Definitions, P27-P32, H1-H6. Writes `REPORT.md` (plain words first), `summary.json` and `rows.jsonl` | The table layouts of EVAL.md and FINAL.md. `critique/common.py:18-26` for score semantics |

## 4. Tests, written first

Each module starts as a stub that returns None or an empty result, so each test fails on its assertion and not
on an import. An independent subagent of the default type runs the suite against the stubs and records every red
in `RED.md`, as backlog 38-39 did. A control passes both before and after.

| Id | Asserts | Before the code |
|---|---|---|
| T1 | `frozen/method.py` hashes to `462c37cf…` | control |
| T2 | The loader refuses a copy with one byte changed (VoidRun) and records the hash it checked | fails |
| T3 | The Z = 3 instance has Z = 3.0, the primary instance keeps 2.0, and the file hash is unchanged | fails |
| T4 | The upstream copy hashes to `57f391e6…` and reproduces `oracle.json`'s EDT for every band, bit for bit | fails |
| T5 | Upstream wrapper: a finite positive value is ok with a point range; NaN is refused | fails |
| T6 | 12 rows of `ism_rows.pkl`, 3 from each corpus room, reproduced: bins and truth to 1e-9 relative | fails |
| T7 | Split rule: echograms from the corpus rooms t1 and corridor, summed and rebinned to 0.1 ms, read by P15 agree with `truth_ideal` on the true split within 0.3 % | fails |
| T8 | Blocked rule: the occluded synth construction, read by P15, agrees within 0.3 % with `truth_edt(t_arr + gap, 0, …)` | fails |
| T9 | Four planted references give u as P19 defines it. Rows above 1 % are excluded, and a last-10 % share above 1e-6 gives `truth_truncated` | fails |
| T10 | The seven rooms meet P3, P5 and 2.2: their features, V, design T60s, distance classes, clearance and blocked flags. F2 is the longest by design. P3 holds against every corpus room, and each corpus room's recorded kind matches its source. P0 has F2's V, S and α within 1 %, differs from F2 by more than 10 % on every axis, and its source and receivers are F2's scaled per axis. P0b stands to F7 in the same way | fails |
| T11 | Each project passes `simpa validate` (exit 0) and `simpa check`. Its settings match P8-P12. Its duration equals a freshly imported project's | fails |
| T12 | Plan-only lists exactly 144 tested runs (72 in each mode) and 28 truth runs with P12's seeds. No seed repeats, and 9999 and 9998 are absent | fails |
| T13 | Guards: a flipped spps.exe is refused before any run; a report whose `solver_build` is unverified is refused; seed 9999, or a folder outside the data root, is refused; a held-out seed (SPPS, ISM or Synth) before A1 is committed is refused | fails |
| T14 | The reader returns, for 3 corpus noise-cal `report.json` files, exactly the old loader's `energy_pa2`, `arrival_s`, R/c and dt | fails |
| T15 | Synth-fresh: 500 rows per family × step, ratios of 1.5 and 5 only, the ranges of P24, a delayed row equal to its undelayed twin shifted by whole steps. Reproducible from a dev seed | fails |
| T16 | ISM-fresh with a dev seed: four relatives, one per corpus ISM room, every axis 12-33 % up; the eight drawn rooms fresh and in their bounds; no room over 10⁸ images, rejections counted; receivers fill their classes; the image set meets P22 | fails |
| T17 | Attack: a class out of bounds is rejected with its reason; the draws reproduce from the class hash; the sandbox holds exactly the allowed files; the judges' prompt holds the class files and `PHYSICS.md` and no result field | fails |
| T18 | Definitions on hand-made rows, including \|err\| exactly 0.05 (not wrong) and the 1e-9 edge | fails |
| T19 | Exclusions and empty denominators behave as P27 says | fails |
| T20 | H3 subgroups and the 20-row rule. H4's filter, per set. H5 is strict, with 0 against 0 failing. H6: a reproducible class fails it unless 3 of 3 judges call it implausible; a split, a missing or an unreadable vote counts as plausible | fails |
| T21 | The scorer voids the evaluation when the method file's hash differs | fails |
| T22 | P33: `weak_spots.json` reproduces from its sources, and every weak-spot value that P33 says binds a set lies inside that set's bounds | fails |

## 5. Pass bar for the harness

This bar is about the harness, not the EDT result. Items 4 and 5, with T10, T11 and T13, hold before the probe
(section 6, step 5). All of it holds before `ADDENDUM-A1.md` is committed.

1. **Tests.** T1-T22 pass. Every red was seen on its assertion in `RED.md`, and the controls pass before and after.
2. **Reproductions,** all in-sample and needing no new data. T4, T6 and T14 hold at their tolerances. The critique's scan 1
   (`final/run_scans.py:25-58`), regenerated through the harness, gives 3,648 rows. On them, the frozen method's
   wrong-silent counts are 0/0/1/0 and its refusals 0/2/40/160 at 1/2/5/10 ms, as FINAL.md:55.
3. **Dry run on synthetic input.** The expected table, `expected_dry.json`, is committed first.
   - D1: 2,000 rows of the critique's ratio-3 family, dev seeds, the full pipeline.
     *Amended 12:41 (Jarvis; 7c's expectation writer, `73a0ca9`).* No ratio-3 construction in the repo gives
     2,000 rows; the critique's ratio-3 scan (scan 2b) gives 1,980, with counts already published before the
     harness existed (`FINAL.md:55`, `harness/weak_spots.json` "scan2b"). D1 is those 1,980 rows: an
     expectation fixed by a document older than the code under test is the stronger check.
   - D2: fake SPPS run folders in the `simpa results` schema: one mock room, 8 receivers, 6 bands, 3 steps,
     3 seeds and K = 4 references at 0.1 ms, built from single-slope and ratio-3 synth histograms with
     compound-Poisson noise. It plants
     disagreeing references (u > 1 %), one truncated reference, one NaN truth, one blocked receiver, and a method
     that multiplies EDT by 1.08 in one subgroup.
   - D3: ISM on the corpus rooms t1 and corridor.
   - D4: two hand-written attack classes run against a stub. The judge panel then judges two control classes
     whose answer is known: F2's settings at 1 ms (plausible), and DRR +30 dB at 30 m with T60 10 s
     (implausible: the diffuse-field DRR then needs a room of about 3 × 10⁹ m³). It must call both, 3 of 3,
     before any real class is judged.

   Every count and every H verdict must equal `expected_dry.json`. The hash check must be recorded, and each
   planted fault must land in its own counter.
4. **Rooms.** All nine projects, F1-F7, P0 and P0b, mesh: `simpa mesh` exits 0 and `simpa mesh-verify` exits 0. This is TetGen only.
5. **Nothing leaked.** The driver's launch log is empty before the probe and holds only the probe's line after it,
   and nothing exists under `C:\tmp\m8b-edt\heldout\`. The count of files on B: outside `harness/` is unchanged.
   C: keeps at least 8 GB free.
6. **Review.** One reviewer, a default subagent (not `assay`), finds no blocker, or its blockers are fixed and checked again.

## 6. Order of work

Each step appends a line `HH:mm <step>: <what>` to `C:\tmp\m8b-edt\progress.log`, which the 15-minute
check-in reads (standing order). The probe comes as early as its guards allow, so that Burhan has its number
before most of the code is written.

1. Build `simpa` from this worktree's HEAD into `C:\tmp\nm-target`. Point `SIMPA_SOLVERS_DIR` at the verified
   build (`C:\tmp\nm-target\target\solvers\bin`, checked against the manifest).
2. `corpus.py` writes `corpus_rooms.json` and `weak_spots.json` from their sources, with their hashes.
3. Write the stubs and T1-T22. The independent red check produces `RED.md`. Commit.
4. Rooms with meshing, then the driver's run path and its guards, until T10, T11 and T13 are green and the nine
   projects mesh. Commit.
5. **The timing probe** (section 7). Its numbers go to Burhan in the next 15-minute update, in plain words,
   with P10, P11 and the question of the long stretch (`HANDOFF-2026-10-01.md:61`).
6. The rest, each until its tests are green, with a commit after each: method, upstream, truth; scorer;
   Synth-fresh; ISM-fresh; the driver's matrix, with P10 and P11 as Burhan sets them (the defaults until he
   does); attack kit. None of it waits for his answer, because the PREREG needs it whatever the probe says
   (H1, H5, H6). If he stops M8b, the build stops there.
7. Run the reproductions and D1-D4. `GREEN.md` holds the receipts of section 5. Commit.
8. Review, fix, commit.

`ADDENDUM-A1.md` and every held-out row wait for Burhan's word.

## 7. The timing probe (step 5)

- **When.** After step 4, alone on the machine.
- **Room.** P0, a stand-in for F2 that is not a held-out room: a box 18.9 × 9.5 × 9.46 m (V 1,698.5 m³,
  S 896.4 m², each 0.1 % under F2's), α 0.08 and Lambert 1 as F2. Its mean free path and Eyring T60 (3.46 s at
  500 Hz) are F2's, and it differs from F2 by 11-24 % on every axis. Its source and 8 receivers are F2's, scaled
  per axis. It is never scored, so nothing it writes is held-out content. T10 asserts all of this.
- **SPPS.** As F2's truth runs (P16, P17): Random; 1,000,000 particles per source per band; step 0.1 ms; 6.5 s
  (65,000 steps); octave bands 125 Hz-4 kHz; air on with the default atmosphere; R 0.31 m; trans_epsilon 5;
  `random_seed` 9999, which is reserved and runs one thread as the truth runs will; all 8 receivers; 0 particles
  saved; intersection files off; no per-source echogram; the verified build.
- **Command.** `simpa run <P0-probe.simpa> --solver spps --runs C:\tmp\m8b-edt\probe --json`, keeping stderr.
- **Recorded** by the driver's probe mode, in `PROBE.md` beside this file: the wall time, `run.json`
  `outcome.elapsed_ms` and verdict, spps.exe's peak working set (polled), the output size (a stat of each file;
  no file is opened), stderr lines by class, and the exe hashes.
- **Kept apart.** `simpa results` is not run on it, so P14's report check has nothing to read, while the check
  before the run applies. The reader refuses seed 9999 and folders outside the data root
  (T13). It is not one of the K references and never becomes one. The folder goes on
  `session-logs/TO-DELETE-<date>.md` for its disk space, for deletion after 07:00.
- **What it gives Burhan.** The time of one run with F2's cost drivers, and the peak memory, which decides how many
  runs fit at once. F2 has the longest design T60, and in Random mode the cost follows how long particles live, so
  F2 should be the slowest room, and P0 should time as F2 does (same volume, area, absorption and settings).
  Neither is measured. If both hold, 28 × that time bounds the single-thread truth time. The report gives no
  projection beyond that arithmetic.

**Not in this plan:** any held-out row of the four sets, the attacker session, committing A1, and row 15's ×1000
calibration (the verdict settles it, `HANDOFF-2026-10-01.md:63-65`).

### 7.1 The dead-room probe (P0b), for the run-length rule

Burhan asked at 02:37 why the run cannot simply go on until the sound has died. SPPS runs for the duration it is
given, but a particle stops being traced once it is absorbed (Random) or falls below −50 dB (Energetic). If a
longer duration then costs a dead room almost nothing, a generous fixed default behaves like a run that ends when
the sound dies. If it does cost, the default should come from the room's predicted reverberation time instead.

P0b answers this before P10's rule is chosen. It is a stand-in for F7: the same V, S and α within 1 %, more than
10 % off F7 on every axis, never scored, with seed 9998. It runs twice in Random mode at 150k particles and 1 ms,
once with a 2 s duration and once with 10 s, and once more each way in Energetic mode. The driver records wall
time, elapsed_ms and output size for each, as for P0, in `PROBE.md`. The numbers go to Burhan with P0's, together
with the two choices for P10.

## 8. Crucible findings and their disposition

- **MAJOR, P21/P23/P24, bounds authored with the failure maps in hand: changed.** The draft did avoid the weak spots.
  P21's 125 Hz T60 ≥ 0.3 s left out dead (0.20 s), and P22's 4 kHz top band left out 20 kHz, where the method's
  only ISM near misses sit (dead, 20 kHz, 1 ms, −2.8 % and −2.6 %, `res_ism.pkl`). P22 and P24 fixed R at 0.31 m,
  while all 10 of its z3 weak-spot rows have balls of 0.88-1.49 m (`res_z3.pkl`, `judge_results.json`). Now P33
  ties every open bound to `weak_spots.json`, which a script writes and T22 checks. P21, P22 and P24 are widened
  to it, and each corpus ISM room gets a fresh relative. The synth ratios, DRR and steps the finding also names
  are the PREREG's own (:37-41), fixed before this plan, and stay.
- **MAJOR, P26/H6, one unreplicated judge: changed.** Three judges, each alone, judge every class before any run.
  A class is implausible only 3 of 3, so a judge's noise can only fail H6. Each judge runs once, and every vote is
  reported. D4 checks the panel on two control classes, T17 its blindness and T20 its rule.
- **MAJOR, P3, non-box rooms fresh by construction: changed; the premise was right but unchecked.** Every corpus room
  is a box today, with receipts in P3. P3 now compares non-box rooms with non-box corpus rooms, `corpus_rooms.json`
  records each room's kind from its source, and T10 applies the rule to every pair.
- **MINOR, §7, the probe writes a real F2 histogram before the freeze: taken.** The probe runs in P0, a box with
  F2's volume, area and absorption that is 11-24 % off F2 on every axis and is never scored. The driver records the
  output size by stat and opens no file.
- **HEAVY, §3-§6, the probe comes last: taken in part.** The probe moves to step 5, right after the rooms and the
  driver's run path with their guards, and ahead of the generators, scorer and attack kit. These do not wait for
  Burhan's answer: the PREREG needs them whatever the number is (H1, H5, H6), and the number decides when and where
  the SPPS truth runs go, not whether the noise-free sets are needed. If he stops M8b, the build stops at once.

### 8.1 Step 3's findings and their disposition

The test writer found these at 04:16, before any data. The main session checked each again before changing the plan.

- **F3's design T60 did not follow P5: corrected.** The chamber, with its doorway at α = 1, gives 2.11-1.56 s,
  not 2.16-1.59. This was recomputed with `ism.py`'s `m_energy` and c = 343.2, and F1, F2 and F4-F7 match
  their rows. F3's reference run (P17) is therefore 4.3 s, not 4.4. The early figure is now the main room by the
  same formula.
- **F7's scaled receivers broke 2.2's 0.6 m clearance: corrected.** One point landed 0.576 m under the ceiling
  and is lowered to 1.80 m. F7's points are now listed with the others: 3 near and 5 mid, because scaling moves
  one of F1's near receivers out to 2.04 m.
- **P14's report check cannot pass today: recorded, guard unchanged.** No CLI or bed run records its solver build
  (backlog 54), so every real run's report reads `solver_build_unrecorded`. The guard stays as written, and the
  truth and tested runs wait for backlog 54's CLI half, which Burhan decided on 2026-10-01 (decision row 33). The
  probes are not affected.

The red check (`RED.md`, 05:24) fixed four tests and raised the following. Each stays open until it is settled,
and all of them are settled before A1:

- **The method's pin held on one machine's git settings only: fixed** (`00e793e`). PREREG's sha256 `462c37cf…`
  is the committed text checked out with CRLF line ends. The `.gitattributes` beside this file now keeps `frozen/`
  in that form on every checkout. The frozen file and PREREG.md are unchanged.
- **Synth-fresh near DRR 9.54 dB: open, for decision before A1.** The synth truth (P15) is NaN for every DRR
  above 10·log10(9) = 9.54 dB, because the direct sound alone takes the level past −10 dB. That was 61 of 4,000
  dev rows, excluded as `truth_nan` (P27). Below the edge the truth is finite but can be very large, and the
  reflection gap sets how large: for a 1 s decay, 28 s at 9.50 dB with a 5 ms gap, 13.3 s already at 9.0 dB
  with a 40 ms gap, and 1.0 s with no gap (the second red check, `RED.md`). PREREG fixes the range at up to
  +10 dB (:39). How those rows bear on H1 has not been examined.
- **P3's figures: corrected from `rooms.py` in step 4.** P3 now gives the five fresh boxes, F7 included, and
  their clearances, 17.1-31.25 %, in place of four at 18-44 %. The lowest is F5's 17.1 % against z3's box055, as
  the writer measured, so F5 is still fresh.

Step 6's truth check (08:03) raised one more:

- **P15's split close to the direct sound: open, for decision before A1.** On all 944 finite corpus
  receiver-bands, the split rule is off the image-source split by more than 0.3 % on 181, and by at most 2.08 %
  (at 20 kHz). In 125 Hz-4 kHz it is off by more than 0.3 % on 49 of 477, and by at most 1.20 %. Every one of
  these rows has its first reflection within 2R/c = 1.81 ms of the direct sound. With a longer gap, the split
  is within 0.017 %. The wrong-silent line is 5 %, so a row near it can be misjudged. `truth.py`'s docstring
  gives the figures.

### 8.2 Calls on the open points (Jarvis, 2026-10-01 10:31; technical calls under decision row 13, Burhan may overrule)

These settle section 8.1's open points before A1. Each is built and tested before A1 is committed.

- **The synth edge at DRR 9.54 dB: no change.** NaN truths stay excluded and counted as `truth_nan` (P27). The
  finite rows just below the edge stay in. They test whether the method refuses or widens an ill-conditioned
  case, and a refusal is never wrong (PREREG :62).
- **Sources inside the receiver ball (d < R) in Synth-fresh: excluded, with the reason recorded.** The generator
  drops the direct sound there (`synth.ball_atoms` floors rho), so the histogram does not hold the row's truth.
  Scan 1's weak spot at d 0.17 m was the generator's: the method read its histogram within 0.1 %. P33 gains this
  exception: a weak spot whose own histogram does not hold its truth does not bind a bound. Draws redraw while
  d < R + 0.2 m, as P24 first said.
- **P15's split close to the direct sound: borderline rows do not decide.** When a row's first reflection falls
  within 2R/c of the direct sound, and its error is within the split's measured bound of the 5 % line (1.2 % in
  125 Hz-4 kHz, 2.1 % above), it is reported apart as `truth_split_borderline` and leaves the H1-H3 and H5
  counts, counted by reason (as P27's exclusions are).
- **ISM-fresh truths flagged truncated: kept when converged.** A truth that P18's 10 % test calls truncated
  is kept if lengthening its image set 1.5 times moves it by less than 1e-3 relative. Otherwise it is excluded
  as `truth_truncated`, as before.
  *Corrected 11:30 (the 10:31 text misquoted step 6; audit of `7481a67`).* Step 6 measured both small moves at
  **2 times**, not 1.5: 1.3e-5 and 1.6e-4 relative, in the two dev rooms with the longest design T60. Extending
  an image set only adds tail energy, so the 1.5-times move is bounded by the 2-times move there; that is
  inferred, not measured. The one row measured at 1.5 times, the corridor relative's far receiver, moved
  +0.8 % at 125 Hz (+0.3 % at 1 kHz) and stays excluded, which is right: its truth is about 1 % low
  (`harness/m8b/ism_fresh.py` docstring). 2 times is not used because the corridor reaches over 8e7 images
  there (step 6: one 7.8e7-image echogram took 1,113 s at 7.6 GB). Consequence, accepted and counted: the
  corridor relative's rows at 125 Hz-4 kHz leave ISM-fresh as `truth_truncated` in most draws.
- **Wiring owed before step 7's dry runs pass (audit of `7481a67`).** Calls 3 and 4 exist as functions with
  tests but no real row reaches them yet. `make_row(..., retry_truncated=True)` must be what builds every
  held-out ISM-fresh row (its default stays False so T6's bit-exact reproduction holds), and
  `truth.split_borderline` must be applied by the SPPS-fresh row builder when it is written. Each gets a test
  that fails if the held-out path skips it.
- **The first-reflection gap in every room, not only boxes (Jarvis 12:01, audit of `03470f9`, which blocked on
  F3 and F4).** The gap that call 3 tests is a lower bound, taken the same way in every room: mirror the source
  in the plane of every face of the room's own mesh, take the shortest image-to-receiver distance, subtract the
  direct distance, divide by C. No visibility test. Any reflection, specular or diffuse, off a face is at least
  as long as the path through that face's plane, so the true first reflection can only come later; an extra
  plane (a doorway's wall) only shortens the bound. Erring short sends more rows to the borderline check,
  which only removes rows already within the split's bound of the 5 % line; erring long could let a split
  error score as wrong-silent. In a box the bound equals the box formula, which a test pins. A room whose
  geometry cannot be read raises; nothing falls back to the first box.
  *Amended 12:14 (`ff85de8` gave negative gaps: F3 rec1 -0.55 ms, F4 rec7 -9.92 ms).* A face counts only when
  the source and the receiver both lie in front of it (on the side its normal points to, inside the room). Any
  reflection off a face, specular or diffuse, arrives from and leaves to its front side, so the bound stays a
  lower bound, and for two points on the same side the mirror path is never shorter than the direct one, so the
  gap is never negative. A receiver with no direct line to the source has no direct sound to split from; it
  takes whatever status the harness already gives a blocked receiver (D2 plants one), and the gap is not
  computed for it.
- **PHYSICS.md gains the diffuse-field consistency check (Jarvis 13:25; D4's judge panel, first run).** With
  only the product's input limits in PHYSICS.md, all three judges called control B (DRR +30 dB at 30 m, T60
  10 s) plausible, because both values sit on the inclusive edge of the limits; control A was 3 of 3
  plausible. The panel could not tell an impossible room from a real one. PHYSICS.md gains: the critical
  distance r_c = 0.057 sqrt(V / T60) (m, V in m^3, omnidirectional source), DRR(d) = 20 log10(r_c / d), so
  a class's (T60, DRR, d) implies V = T60 (d / 0.057)^2 10^(DRR / 10); allowing +/-10 dB for fields that are
  not diffuse (coupled, long or specular rooms), a class is physically implausible only if every instance's
  implied volume exceeds 1e8 m^3 even at the -10 dB end (about eight times the largest enclosed building)
  or is smaller than a room that holds its own source-receiver distance even at the +10 dB end. The bound is
  set wide on purpose: a judge that calls a real attack implausible lets H6 pass falsely, which is the worse
  error. Control B implies 2.8e9 m^3 (2.8e8 at -10 dB): implausible. Control A implies at most 1.4e5 m^3:
  plausible. D4's panel is re-run on the regenerated prompts and must give 3 of 3 on both.
