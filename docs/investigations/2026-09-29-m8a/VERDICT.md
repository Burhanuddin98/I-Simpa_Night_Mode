PASS

# M8a verdict: the T30 bed

Bed `C:\tmp\nm-m8a-bed\20260929T093134Z`, built from `da6f15c` (`meta.git_dirty` false, `meta.from`
null, 12 jobs). RUN.md at `3dd0c9c`. `report.json` sha256 `5c0041fc…0fad`, byte-identical to the copy
in `beds/m8a-20260929T093134Z/`. Judged 2026-09-29, 18:10 to 18:46, by three independent lenses. Their
reports are in `judge/`.

**The M8a PASS stands.** The raw solver output supports it, and so do independently computed
references and an independent transport. No finding is blocking: all three lenses return an empty
`verified_blocking`. The gate lens returned CONCERN, not FAIL. It found two major defects in the pass
logic. Neither affected this bed, but the gate must not be reused until both are fixed (findings 1 and 2).

## What the verdict rests on

| Required | Lens | Shown by |
|---|---|---|
| T30 recomputed independently | t30 | Own GABE parser and ISO 3382-1 fit, no project code imported. Covers all 1,200 raw `Sound level.recp` files, 400 runs and 7,648 seed-receiver-band values (6,240 gated). 0 of 96 gated A/B/C verdicts change. |
| References recomputed independently | refs | Exact γ² by integral geometry, checked on the cube (0.344950), plus a Monte Carlo of Lambert reflections. Kuttruff, Eyring and Sabine computed for all 40 cells (208 gated cell-bands). m taken from two sources. |
| Transport independent of SPPS | refs | The code path shows the transport reads no run output (`bed/transport.rs`, `run.rs::trace_transports`). An independent numpy transport over 64 distinct transports agrees with the bed's to z rms 1.02. Gate C passes 32/32 against it. |
| The gate can fail | gate | A, B, C and D each flip `pass` to false just outside their limit (5.010 %, 2.010 %, 0.510 %, 0.51 %). Just inside, A, C and D keep it true (4.990 %, 0.490 %, 0.49 %). B turns back to Pass at 1.990 %, but the same shift makes C inconclusive, so `pass` stays false. 10 of 12 missing or refused inputs fail closed; the 2 that do not are finding 1. `m8a.ps1` exits 1 with no PASSED line when `pass` is false or `exploratory` is true. |
| Every gated cell, seed, receiver and band | gate, t30, and my spot-check of report.json | 32 of 40 cells gated, all Pass. Seeds exactly 1 to 10 in all 32. 6,240 values: 6,239 `value` and 1 `monte_carlo_noise` (6x10x3-a0.05-random-air-on, seed 8, R002, 8 kHz: t 1.0327 s, mc_sd 0.0283), with 0 refusals. A has 208 bands, B 32 cells, C 32 cells with no extension. TCR: 16 of 20 gated, D on 104 bands. |
| Verified solvers | gate | Code sha256 of all 4 executables equals `solvers/manifest.json`. The executable sha256 in all 433 `run.json` files equals the checked file. All 433 `mesh.json` carry TetGen `ac68f014…`. |
| dt 1 ms | t30, gate | `pasdetemps="0.001"` in all 412 SPPS `config.xml` files (gate), 400 of them also read by the t30 lens. The `.gap` time step is 1.00000005e-3 (f32) and the rows equal duration/dt. |

Worst margins in report.json, which I re-derived myself:
- A: 1.178 % against 5 % (5x4x3-a0.1-random-air-on, 250 Hz, [+0.110, +1.178] %).
- B: 0.765 % against 2 % (5x4x3-a0.4-random-air-off).
- C: 0.330 % against 0.5 % (5x4x3-a0.05-random-air-on, d −0.165 %, [−0.330, +0.000] %).
- D: 4.4e-7 relative against 0.5 % (5x4x3-a0.1-tcr-air-off, 4 kHz).
- E5: 0.584 % against 0.6 % (5x4x3 α 0.4).

## Mismatches: ours against report.json against the limit

None changes a verdict. The biggest is E5 in the refs lens (row 11).

| # | Quantity | Ours | report.json | Limit | Receipt |
|---|---|---|---|---|---|
| 1 | One T30 value, ISO fit at the step edges | 0.219734 s | 0.219644 s (0.041 %) | 0.1 % "major" line | t30.md §3; 5x4x3-a0.4-random-air-off, seed 2, R000, 2 kHz |
| 2 | Largest per-cell mean T30 difference, gated | 0.0015 % | – | 0.1 % | t30.md §3; 6x10x3-a0.4-energetic-air-off, sign varies |
| 3 | T30, continuous fit (the project's method) | −4.750e-8 on all 7,648 values | – | 0.1 % | t30.md §3. The difference is f32(0.001)/0.001 − 1: the bed takes dt from the `.gap` column. |
| 4 | Largest ISO-fit difference in any cell, including reported ones | 0.65 % | – | not gated | t30.md §3; 20x8x4-a0.4-random-air-off, which has a sparse tail |
| 5 | A band mean or interval edge | largest diff 6.4e-5 absolute | – | 0.05 | t30.md §4; 6x10x3-a0.4-random-air-on |
| 6 | B spread | 0.760 % | 0.765 % | 2 % | t30.md §4; 5x4x3-a0.4-random-air-off |
| 7 | C: per-seed y_s / \|d\|+t·SE | largest diff 5.2e-5 / 1.6e-5 | – | 0.005 | t30.md §4 |
| 8 | Kuttruff T_K (exact γ², own m) | 0.221102 s | 0.221096 s (2.94e-5) | 2e-3 blocking | refs.md §2; 5x4x3 α 0.4. The gap is entirely γ²'s draw. With the report's γ² and m: 5.3e-8. |
| 9 | γ² (6x10x3 / 5x4x3 / 20x8x4) | exact 0.388874 / 0.352401 / 0.410059 | 0.388929 / 0.352297 / 0.409961 | – | refs.md §1: +0.38, −0.84 and −0.56 of the report's SE. Moves T_K by at most 2.9e-5. |
| 10 | Air term m, ISO 9613-1 against upstream's form | 0.006 to 0.033 % lower | equals upstream's form exactly | SPEC 0.035 % | refs.md §2. Moves T_K by at most 2.05e-4 (8 kHz). The report's m is the form the solver computes. |
| 11 | **E5: Kuttruff against the transport's receivers, 5x4x3 α 0.4** | **+0.616 % ± 0.024 %** (own transport) | +0.584 % (bed's transport, SE 0.012 %) | **0.6 %** | refs.md finding 1. The own transport lands 0.016 points (0.7 SE) past the bound. Air-on bands of the same cell (R3, reported only) reach +0.599 % at 1 kHz. |
| 12 | Transport T30, 5x4x3 α 0.4, receivers | 0.219742 ± 0.000053 s | 0.219812 ± 0.000026 s (−0.032 %, z −1.18) | – | refs.md §3. Over 64 transports: z mean −0.40, rms 1.02, max 2.22. |
| 13 | C with the own transport in place of the bed's | worst reach 0.302 %; 32/32 Pass | worst reach 0.330 % | 0.5 % | refs.md §3. 6x10x3 α 0.2 random air-on: d +0.142 % [+0.004, +0.280], which excludes 0 and still passes. |
| 14 | Gate re-judged from the 422 run folders | pass true, every verdict identical | pass true | – | gate.md §1. Numeric diffs ≤ 3.2e-11 relative. The harness's JSON round trip alone changes 4,239 of 87,206 lines. |
| 15 | SPEC's `refs.py` γ², 20x8x4 | exact 0.410059 | spec 0.4104 ± 0.0002 (+1.7 SE) | – | refs.md finding 5. The bed does not use the spec's value; it uses its own 0.409961. |

## Findings

### Major (latent: not triggered in this bed, and blocking any reuse of the gate)

1. **D does not check a TCR run's bands against the bed's bands.**
   - `check.rs:1051-1057` judges whatever bands `r.bands` holds. `check_d` passes any non-empty list.
   - Test: gated 6x10x3-a0.2-tcr-air-on with its 8 kHz band removed gives `pass true`, 0 failures. With
     only its 125 Hz band left, also `pass true`.
   - Not triggered: 104 of 104 gated D bands are present.
   - Fix: compare the run's band list with `bed.bands_hz(cell.air)` and fail on a mismatch, with a
     unit test.
   - Receipt: gate.md finding 1, `judge/gate-harness-out.txt`.
2. **Runs read with `--from` are never compared with the cell they are filed under.**
   - `SppsRead` carries the particle count, method, `time_step_s` and `trans_epsilon`, and `run.json`
     carries the project's sha256. No line in `check.rs`, `report.rs` or `bed_cmd.rs` compares them
     with the matrix or the seed. E7 certifies only the bed file.
   - Consequence: `m8a.ps1 -From` over another bed with the same cell ids, for example an exploratory
     bed at 10 ms, would print `M8a PASSED`.
   - Not triggered: `meta.from` is null. `gate-runs-check.txt` finds 0 problems in 422 configs checked
     against SPEC tables typed independently of the code.
   - Receipt: gate.md finding 2.

### Minor

3. **E5's 0.6 % has almost no margin.**
   - Bed: +0.584 %, 97 % of the limit.
   - Independent transport: +0.616 % ± 0.024 % on the receivers and +0.596 % on the room energy.
   - "Kuttruff within 0.6 %" (row 17 (7)) therefore holds or fails depending on the draw. `docs/params.md`
     already calls it "not a held-out bound".
   - Gate A (5 %) is unaffected: the gap there is 0.58 %.
   - Receipt: refs.md finding 1, `refs_recompute.json` `own_vs_bed_transport`.
4. **N4 and N7 leave no C numbers in the gate log.**
   - `m8a.ps1:267` shows the first 12 regex matches. These are transport progress lines
     (`gate-j12.txt:47-71`). Cargo keeps its output in memory only.
   - The PASS rests on `require_fail` asserting C == Fail in all 32 (N4) and all 16 (N7) cells
     (`bed_m8a.rs:122-172`). That assertion is sound.
   - Receipt: gate.md finding 3.
5. **N5 stopped at E6 and never tested B.**
   - Seed 10 at 31,000 particles was refused `range_not_reached`, where SPEC §7 expected
     `noise_uncalibrated`. B was never computed.
   - B's limit is shown only by the unit test and by the harness perturbation: 2.010 % gives pass
     false, 1.990 % gives B Pass.
   - Receipt: gate.md finding 4.
6. **N6 checks the refusal codes directly, not through `e3_of`.**
   - `report.rs:718-746` recomputes the E3 condition itself.
   - That `e3_of` refuses a gated cell with a refused Kuttruff is shown only by the harness (E3 false,
     pass false). No bed unit test covers it.
   - Receipt: gate.md finding 5.
7. **The cargo checks read the exit code only.**
   - Check 21 would pass if the `bed::` filter matched nothing.
   - `a_cells_project_is_the_measured_cells_byte_for_byte` returns ok without running, because the
     gate never sets `SIMPA_M8A_MEASURED_PROJECT` (`file.rs:912-916`).
   - Today 25 tests run, and the skipped test passes when the variable is set.
   - Receipt: gate.md finding 6.
8. **E1's TetGen leg skips a run with no `mesh.json` or no TetGen sha.**
   - `m8a.ps1:211-214`, `report.rs:352`.
   - 433 of 433 runs have one, and all are equal.
   - Receipt: gate.md finding 7.

### Notes

- The T30 estimator `params::decay::decay_time` is shared by SPPS's receivers and the transport, so C
  cannot see a bias in it. The refs lens's own estimator on its own transport reads about 0.02 % lower
  (z mean −0.40 over 64). Receipt: refs.md finding 2.
- `params::lambert` produces both A's γ² and C's transport, so A's and C's references are not
  independent. The exact γ² closes this for γ² (T_K within 2.9e-5). Receipt: refs.md finding 3.
- RUN.md's attribution holds: A's offset is the gap between the formula and the transport.
  - Correlation over 32 cells: 0.984.
  - A − gap: mean −0.002 %, sd 0.050 % (bed's transport).
  - At α 0.4, truncation (renewal/K −1.1 % and −1.4 %) and path correlation (+0.84 % and +0.87 %)
    partly cancel.
  - Receipt: refs.md §4.
- ISO truncation compensation changes no A, B or C statistic to 4 decimals of a percent. The tail
  energy is at most 2.5e-10 of the total. Receipt: t30.md §5.
- The 20x8x4 random refusals are right. All 26 `range_not_reached` values are at R002, where the last
  step holding energy is 32.3 to 35.0 dB down. Receipt: t30.md §6.
- The pass logic does not pin its counts. E4 and E5 need a non-empty cell list, not 8, and the gated
  TCR list may be empty. E7 pins the matrix, and this bed has 8, 8 and 16. The plots check tests
  existence only. `check_a` panics on a receiver one band short (`check.rs:130`); that fails closed
  and cannot be reached. Receipt: gate.md findings 8 to 10.

## What the bed does NOT show

- **Other rooms.** Only convex boxes with uniform α and Lambert walls at scattering 1. Nothing about
  specular or partly diffuse walls, uneven absorption, or non-convex rooms, where the chord identity
  behind the exact γ² fails.
- **Kuttruff's 0.6 % in other rooms.** It depends on the room. In the reported 20x8x4 room at α 0.4,
  Kuttruff is 1.06 % above the transport's receivers (0.41421 against 0.40987 s) and 0.19 % below its
  room energy. SPPS follows the receivers (A −0.96 to −1.10 %).
- **Other parameters.** T30 only, at dt 1 ms. No EDT or T20 (M8b), and no other time step.
- **User particle counts.** Gated random cells ran 3.5 M to 110 M particles, 23 to 730 times the
  150,000 the app ships with. The pass says nothing about T30 at user counts.
- **The reported 20x8x4 random cells at 1.5 M.**
  - α 0.2 fails A at 500 Hz (edge 5.28 % against 5 %) and B (5.40 % against 2 %). C is inconclusive:
    +0.745 % [−0.581, +2.071].
  - α 0.05, 0.1 and 0.4 are not judged: E6 refusals, 26 `range_not_reached` and 6 `missing_moves`.
  - With plain Schroeder values in place of the refusals:
    - α 0.4 would fail A (9.13 %) and C (+2.24 % [+1.22, +3.27]).
    - α 0.1 would fail B (2.63 %).
    - α 0.05 would leave C inconclusive (reach 0.554 %).
  - Random mode at 1.5 M particles in a 640 m³ room is not usable.
- **A bias in reading T30 from a Monte Carlo decay.** The t30 lens's ISO fit and the bed's agree, so
  they would share such a bias. The refs lens checked the estimator only on transport-like decays, not
  on SPPS-specific data: noise refusals, the floor, lost particles.
- **The physical model itself.** Both transports use an ideal Lambert wall, a ball receiver weighted
  by path length and air applied per bin. Agreement cannot detect an error in that model, for example
  against a physical microphone.
- **Independent seeds.** The ten seeds' outputs are distinct (sha256 in every cell). Whether they are
  statistically independent draws, as B and C assume, is not shown.
- **Gate strength.**
  - The two latent holes (findings 1 and 2) are open.
  - How far N4 and N7 push C past its limit is unknown.
  - B has not caught a noisy seed on real runs (N5 stopped at E6).
  - The atmospheric validation was checked only for solver hashes, dt, seeds and particle counts. The
    harness did not re-read it.
- **SPPS physics beyond these checks.** The gate proves that the bed judges its own numbers against
  the pre-registered limits. Beyond that, it shows only that SPPS agrees with Kuttruff within 5 % and
  with the transport within 0.5 %, in two boxes.

## Receipts committed

`judge/t30.md`, `t30_recompute.py`, `t30_recompute_summary.json` · `judge/refs.md`,
`refs_recompute.py`, `refs_recompute.json` · `judge/gate.md`, `gate-harness.rs`,
`gate-harness-out.txt`, `gate-harness-diff.txt`, `gate-runs-check.ps1`, `gate-runs-check.txt`.

`t30_recompute_summary.json` is a trimmed copy of `t30_recompute.json`: it keeps `bed`, `problems`
(0) and `summary`, and drops the per-value cells. The full file (3.1 MB) is not committed;
`t30_recompute.py` regenerates it in about 16 s.
