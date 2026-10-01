# ADDENDUM-A1: the M8b EDT held-out test's open parameters, fixed before any held-out row exists

Committed 2026-10-01 18:05 on branch `m8b-edt`, from tree `7c0525e379e4`. Following the noise-calibration
precedent (`2026-09-25-noise-calibration/PREREGISTER.txt:75-76`), as `PREREG.md` :3 and `HARNESS-PLAN.md` P1 require.
From this commit on, nothing below changes. A failed method is fixed and tested again on a different fresh set
(PREREG :3-4).

## The go-ahead

Burhan, 2026-10-01, verbatim: 10:59 "exactly i did not want to be asked, keep building and building and finish the work";
11:43 "i was hoping you could go all the way to M12". The decisions that are his (B) were taken earlier in his words:
P10 at 08:13 ("keep it fixed, 10s for now then", decision row 36), P11 at 02:23 ("include the energetic mode, as long
as its verified"), and the attacker round at 08:05 ("Keep it", row 35). Z = 3 is reported, not deciding (PREREG :11).

## State at the freeze (checked when this file was written)

- No held-out row exists: `C:	mp\m8b-edt\heldout\` does not exist.
- The harness: 53 of 53 tests pass; the step 8 review found no blocker (`GREEN.md`, `3ddaa64`).
- The product the test runs: the default SPPS run length is 10 s (`ccf28f4`, gated), and `simpa run` checks its solver
  build by default, with an override manifest never reading verified (`0694037`, `a116c98`, gated, reviewed SHIP).
- Hashes, sha256 of the bytes as committed in this tree:

| File | sha256 |
|---|---|
| `PREREG.md` | `d70f5fabf6a3ce28efdbfb8b0e60ef4403f16e39399cf4d173bd2133ba2ea2d3` |
| `frozen/method.py` | `462c37cf159d4d9bd8abadd8261f975fa47ace66f0d4cb4b1e6f4234c77fdf6e` |
| `HARNESS-PLAN.md` | `b819cc9f91bcb3770a51d2146f0738575bf1ea89200d3e903803432a8149b9b9` |
| `expected_dry.json` | `00552fafcf34a828c385c4e08bb3fe0eff627034ddd9ccd9b4ddf0483bdede79` |
| `harness/corpus_rooms.json` | `dc3d0b2c0b8be0f69c7927f0c749a88dad76cc791edae2474f09cb1fd44da882` |
| `harness/weak_spots.json` | `c75d53212f4114e785369668e50d81143bc09882c06d6cab2a9c792b730da9b8` |
| `harness/m8b\__init__.py` | `202f941d5b47e33892782e341c57d31845493d770dd7536f2e0ce3cab0d3d6db` |
| `harness/m8b\_ism.py` | `a7c9d41ec61dd119a14583faaa98d4a88c98e897a0cf4a8d69c6d151d2cef747` |
| `harness/m8b\_mirror.py` | `9824f4bb1d0e4f24554f31efc7749a20ba243bf8ef971e4f88d82ad5b4ebad79` |
| `harness/m8b\_truth_ideal.py` | `2a37e532855df11e61bfc402ef2637c3e3acc2c372f29aa22e5e8991c72c6f3e` |
| `harness/m8b\_upstream_edt.py` | `57f391e6cb09ded8a408462fbabc7aacd93743a3fa46cc28deeef3f7a319f282` |
| `harness/m8b\_z3echo.py` | `5fff8405ed0b485d87a8c9fc5097d343b7c56c29232da0f44ebd698a51e16c4e` |
| `harness/m8b\attack.py` | `728d0926549fefd1c56db333b978c196d8a15c4695f247c6897c3b708d3608de` |
| `harness/m8b\corpus.py` | `c6754dc73f4a94bf3766548f77ce85627a10c2c75fea077995b0c6d39badb3c3` |
| `harness/m8b\driver.py` | `16fbc37fd919bbb7d83237e36ec33a2fbc50306fcf4cf581c72939ce8cffe70e` |
| `harness/m8b\ism_fresh.py` | `ca78892d9746db3b0ad1982e1111c7b90049114f9bfbbc79200ee194d7ceb9a3` |
| `harness/m8b\method.py` | `19e61dd02408413e0f4db0f147ba4a303f885575a4b0d48170ad4d220256cd86` |
| `harness/m8b\rooms.py` | `ab9ea71374b2ce9e9fc8118a5f3233ee0ee07da45a6c8e7f50eed67acecbe603` |
| `harness/m8b\score.py` | `29520a017d414a34e5ecf1a68b168adfb5d2bdfa769ca3eac55bbe24426249e4` |
| `harness/m8b\spps_rows.py` | `63357373443552a6989a437d229ad26e8f3d28385557cbfe211347d72ec18546` |
| `harness/m8b\synth_fresh.py` | `1d9470be0510c3c54b156a3d88360e4cb924048d85a2d230eb44ca1962e9a243` |
| `harness/m8b\truth.py` | `412cb9ece0cf36b3d5f2ca143235898d978c5d155d2b6c5f72244efacc47af90` |
| `harness/m8b\upstream.py` | `76ea8c3209d608a83ab36c33618debdca79dff3a015f516ca2dd0539ff155781` |

## The fixed parameters: HARNESS-PLAN.md section 2, verbatim

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

## The calls on the open points: HARNESS-PLAN.md section 8.2, verbatim, with today's amendments

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
