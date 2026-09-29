# M8a judge: gate integrity (adversarial)

Judged 2026-09-29, 18:10 to 18:40, on worktree `m8a` at `3dd0c9c` (bed built from `da6f15c`,
`report.meta.git_dirty` false, `meta.from` null: a fresh bed, 12 jobs). No Python was run. Evidence
read only; scratch work under `C:\tmp\nm-judge-gate\` (a Rust crate outside the worktree that
calls `simpa_core::bed`, built into `C:\tmp\nm-target-m8a`; the scratch reports and gate
extracts). Receipts beside this file: `gate-harness.rs` (the harness; its output
`gate-harness-out.txt` is from the run before the diff and round-trip modes were added,
`gate-harness-diff.txt` from the diff mode), `gate-runs-check.ps1` and `gate-runs-check.txt`.

**Verdict: CONCERN.** The 23 PASS lines are earned: each check tests what its line says, and
`report.pass` is derived from every gated cell's A, B, C and every gated TCR run's D, failing
closed on every kind of missing SPPS data I could inject. Coverage is complete (numbers below).
Two latent defects in the pass logic would let a future bed pass on data this bed does not have;
neither is triggered here, so the M8a PASSED result stands, but the gate should not be reused
before they are fixed.

## 1. The gate can fail

- `cargo test --release -p simpa-core --lib bed::`: **25 passed, 1 ignored** (the eight-cell
  transport trace, run on purpose), 177 filtered out. The one env-gated test that the gate runs
  without its variable, `a_cells_project_is_the_measured_cells_byte_for_byte`, I ran with
  `SIMPA_M8A_MEASURED_PROJECT=C:\tmp\nm-m8a-cell\m8-cells-1790662923\000\project.simpa`: passes
  (the bed builds the measured cell's project byte for byte).
- **Re-judged from the run folders with this build** (scratch crate calling
  `bed::run::read_existing` on the bed's 422 cell, TCR, N5 and N6 runs, `report::evaluate`, the
  transports taken from `report.json` as `bed_m8a.rs` N2/N3/N8 take them; nothing re-traced):
  `pass true`, 0 failures, every cell and TCR verdict the report's, `say_no` identical. The
  numbers differ from the parsed `report.json` in 3,900 leaves, all in the last digit (largest
  relative 3.2e-11, on a C per-seed value of 1e-5, i.e. about 3e-16 absolute). That is my
  harness's JSON parse, not the bed: parsing `report.json` and writing it again alone changes
  4,239 of its 87,206 lines, and the sampled leaves (`gamma2` 0.38892897132029125, as written in all 16
  6x10x3 cells; an A bound −0.0023392447912728206) are the file's digits exactly. The re-read
  took 344 s for 422 runs.
- **Each limit, just outside and just inside**, one perturbation at a time on a copy of what was
  read (the form the code sees):

| Check | Perturbation | Just outside | Just inside |
|---|---|---|---|
| A | 5x4x3-a0.1-random-air-on, 250 Hz (the worst A band), Kuttruff ×1.05390 / ×1.05368 in all ten seeds (bitwise equal, so E3 holds) | reach 5.010 %: A Fail, B/C Pass, **pass false**, 1 failure | reach 4.990 %: **pass true** |
| B | 5x4x3-a0.4-random-air-off (worst B), seed 3 ×1.01243 / ×1.01223 | spread 2.010 %: B Fail, **pass false** | spread 1.990 %: B Pass (C goes Inconclusive from the same shift, so pass false by C) |
| C | 5x4x3-a0.05-random-air-on (worst C), all T30 ×0.998194 / ×0.998394 | reach 0.510 %: C Inconclusive, `needs_extension` names the cell, **pass false** | reach 0.490 %: **pass true** |
| D | 5x4x3-a0.1-tcr-air-off, 125 Hz, TCR Eyring at analytic ×1.0051 / ×1.0049 | +0.5100 %: D Fail, **pass false** | **pass true** |

- **Missing or refused data in a gated cell** (6x10x3-a0.2-random-air-on or its TCR run): seed
  7's run removed (E2), one T30 refused `range_not_reached` (E6), one T30 relabelled `truncated`
  with its value kept (E6), one T30 NaN (A Fail), seed 5's run `FAIL` (E2), seed 5 with 2
  receivers (E3), Kuttruff refused in one band (E3), one band's transport removed (C not
  judged), the gated TCR run removed (E2), one TCR band's analytic time `None` (D Fail): **every
  one pass false**. A reported cell (20x8x4-a0.4-random) made 30 % long: pass stays true, as
  designed. **Two cases pass: see finding 1.**
- **`m8a.ps1` on a report that does not pass:** I ran the gate's own lines (the `Check` function,
  lines 58-73; the two report checks, 192-201; the final verdict, 294-297, extracted by line
  number, not retyped) on scratch copies of `report.json`: `"pass": false` → `FAIL report.pass is
  the JSON true`, `M8a FAILED: 1 of 2`, **exit 1**, no `M8a PASSED`; `"exploratory": true` → FAIL,
  exit 1; the real report → `M8a PASSED`, exit 0. The final line also re-tests `report.pass`
  itself (line 295), so a pass=false report cannot print PASSED even if a check line were
  dropped; and `simpa bed` exits 8 (not 0) on a report that does not pass (`bed_cmd.rs:401-407`),
  which fails check 6 too.

## 2. The 23 checks against their code

| # | Line | Code | Tests what it claims? |
|---|---|---|---|
| 1-4 | E1 ×4 | `m8a.ps1:144-154`, `Get-CodeSha256` against `manifest.code_sha256` | Yes. My recomputation: all four code sha256 equal the manifest |
| 5 | E5 | `cargo test --release -p simpa-core --test params_kuttruff`, exit code | Yes: 4 passed, 1 ignored (the evidence test); the gating `kuttruff_reproduces_the_transports_t30_in_m8s_cells` runs. Exit-code only (finding 6) |
| 6 | bed exited 0 | `bed_cmd.rs:401-407`: 0 only when E2 holds and `rep.pass` | Yes |
| 7 | schema | `bed_m8a.rs:176` boon-validates the file and deserialises it back | Yes (a self-consistency check: the schema comes from the same types; it would catch a NaN written as null) |
| 8 | exploratory false (E7) | `file.rs:601-630`, canonical text against `BedFile::m8a()` in code | Yes. I checked `BedFile::m8a()` against SPEC 2.1-2.4 by reading, and every run's `config.xml` against the SPEC tables typed independently (section 4) |
| 9 | report.pass | `report.rs:530-540` | Yes: E1-E7, `!gated_cells.is_empty()`, every gated cell `Pass`, every gated TCR `Pass`; cell `Pass` only for (A,B,C) all Pass with E2/E3/E6 (`check.rs:877-937`). NaN fails every comparison. Latent holes: findings 1, 2 |
| 10 | E1 from the files | `m8a.ps1:202-219` | Yes for this bed; skips a run with no `mesh.json` or no TetGen sha (finding 7) |
| 11 | files < 20,000 | `m8a.ps1:220-226` | Yes (16,904) |
| 12 | decays and plots | `m8a.ps1:227-235` | Existence only (finding 8); 40 npz (e.g. 553 kB) and 41 png present |
| 13 | N1 | `m8a.ps1:238-256` | Yes: flipped `.text` byte, exit 2, `refused before any run (E1)`, 0 things written |
| 14 | N2 | `bed_m8a.rs:205`, fault in `results::reference`, `require_fail` asserts `Fail` (not merely not-Pass) | Yes: A −9.2 to −11.4 % in every band of all 8 α 0.4 gated cells |
| 15 | N3 | `bed_m8a.rs:221`, `check.rs:763` swaps A's reference | Yes: +8.2 to +11.0 % in every band of all 8 |
| 16 | N4 | `bed_m8a.rs:240`, transport retraced under `LambertUniformReflection` | Yes by assertion (C `Fail` in all 32); no C numbers in the log (finding 3) |
| 17 | N7 | `bed_m8a.rs:268`, transport keyed and traced with air off | Yes by assertion (C `Fail` in all 16 air-on); no C numbers in the log (finding 3) |
| 18 | N8 | `bed_m8a.rs:295`, fault in `results::tcr` | Yes: D `Fail` required in every band of every gated TCR run (+1.230 %) |
| 19 | N5 | reads `report.say_no.n5.as_required` (`report.rs:700-713`: `!passes && no error`) | Yes, and I re-derived it from the runs (identical). But it was stopped by E6, not B (finding 4) |
| 20 | N6 | reads `report.say_no.n6.as_required` (`report.rs:718-746`) | Checks the six refusal codes directly, not through `e3_of` (finding 5) |
| 21 | unit tests | `cargo test -q -p simpa-core --lib bed::`, exit code | Yes (25 run); exit-code only, and one test passes by skipping in the gate (finding 6) |
| 22-23 | clippy, fmt | cargo | Yes |

No check reads a proxy in place of the thing it names (the M10 class). N5 and N6 read the report's
own booleans, but those booleans are the claimed quantity (`!passes`, codes in every band) and
re-derive identically from the run folders.

## 3. Coverage against SPEC.md (from `report.json`, re-derived from the runs)

- Gated cells **32 of 40**, all `Pass`; seeds exactly 1-10 in every one (320 seed-runs, each
  `run` present, status OK, folder `runs/<cell>/s<seed>/`, distinct cell means in every cell).
- Receiver-band-seed values **6,240** = 16 × 10 × 3 × 6 + 16 × 10 × 3 × 7: `value` 6,239,
  `monte_carlo_noise` 1, **no other refusal and no E6 among gated cells**.
- A: 208 bands, each over n = 10, reference `kuttruff_s`; B: 32 cells on 10 seeds; C: 32 cells on
  10 seeds, no extension, `needs_extension` empty; every gated band has `kuttruff_s`,
  `lambert_walls`, and a transport T30.
- TCR: **16 gated of 20**, D on **104 bands** = 8 × 6 + 8 × 7, all Pass.
- E1 433 runs checked; E4 **8 cells**, largest difference 4.6e-10 s (limit 1e-9); E5 **8 cells**
  (tightest 5x4x3 α 0.4: +0.584 % against 0.6 %); E2, E3, E6, E7 hold with 0 problems.
- Air on: the run's `m` equals the bed's `m` exactly in every band (largest relative difference 0).
- Worst margins: A reach 1.178 % (5x4x3-a0.1-random-air-on 250 Hz, [+0.110, +1.178] %) against
  5 %; B 0.765 % (5x4x3-a0.4-random-air-off) against 2 %; C 0.330 % (5x4x3-a0.05-random-air-on,
  d −0.165 %, [−0.330, +0.000] %) against 0.5 %; D 0.00004 % against 0.5 %.
- Reported only, correctly kept out of `failures`: 20x8x4 random α 0.05, 0.1, 0.4 not judged
  (E6: `missing_moves`, `range_not_reached` at 1.5 M), α 0.2 A/B/C not passing.
- `report.json` in the bed and in `beds/m8a-20260929T093134Z/` are byte-identical; `summary.json`'s
  `report_sha256` equals the file's sha256 (`5c0041fc…0fad`).

## 4. Verified solvers and the step (every run, not a sample)

`C:\tmp\nm-judge-gate\runs-check.ps1` read all 433 run folders against the SPEC tables typed
independently of the code:
- Code sha256 of the four executables in `C:\tmp\nm-m8a-solvers` = `solvers/manifest.json`
  (`550485c6…`, `fad4ab5d…`, `d0704003…`, `7d4ba6af…`). `report.preconditions.e1.solvers` names
  those paths, all `matches`.
- **433 of 433** `run.json` executables are the raw sha256 of the checked file (spps `1d9900db…`,
  classicalTheory `c4a7bca3…`), and the file each names hashes to it now; **433 of 433**
  `mesh.json` carry TetGen `ac68f014…`. `preprocess` was not run (`mesh.json` `"preprocess":
  null`), so its E1 is moot.
- **dt = 0.001 s in all 412 SPPS `config.xml`** (every room × method × air: 40 or 41 each, plus the
  10 atmospheric seeds). In the 402 matrix and say-NO runs, particles, duration, `random_seed` =
  folder seed, `computation_method` (0 random, 1 energetic: upstream `core_configuration.h:50`),
  `trans_epsilon`, `abs_atmo_calc`, the band list, α, scattering 1 (0 in N6), `loi` 2, source and
  the three receivers match SPEC 2.1-2.3; the 20 TCR configs match band list, air and walls.
  0 problems.

## 5. Findings

1. **major (latent): D does not check a TCR run's bands against the bed's.** `evaluate_tcr`
   (`check.rs:1051-1057`) judges whatever bands the run reports, and `check_d` passes any
   non-empty list. With the gated 6x10x3-a0.2-tcr-air-on run's 8 kHz band removed, and again with
   only its 125 Hz band left, `report.pass` stayed **true** with 0 failures. Not triggered here:
   104 of 104 gated D bands are present. Fix: compare `r.bands` freq list with
   `bed.bands_hz(cell.air)` and fail (not judged) on a mismatch, with a unit test.
2. **major (latent): runs read with `--from` are never compared with the cell they are filed
   under.** `SppsRead` carries particles, method, `time_step_s` and `trans_epsilon`, and `run.json`
   the project sha256, but nothing in `check.rs`/`report.rs` compares them with the matrix or the
   seed; E7 certifies only the bed file. `m8a.ps1 -From` over a bed made from another matrix with
   the same cell ids (for example an exploratory bed at 10 ms) would print `M8a PASSED`. Not
   triggered: this bed ran fresh and every config matches the SPEC (section 4).
3. **minor: N4 and N7 leave no numbers.** The note regex's first 12 matches are the transport
   progress lines containing `None` (`gate-j12.txt:47-71`); the per-cell C lines the tests print
   come later and were cut, and `Cargo` keeps output in memory only. The PASS rests on
   `require_fail`'s assertion, which is sound.
4. **minor: N5 did not exercise B.** Seed 10 at 31,000 particles was refused `range_not_reached`
   in many receiver-bands (SPEC 7 expected `noise_uncalibrated`), so the cell stopped at E6 and B
   was never computed. The SPEC allows E6, so the line is true, but B's only say-NO on real data
   did not test B. B's limit is shown by the unit test and by the perturbation in section 1.
5. **minor: N6 bypasses `e3_of`.** `say_no` checks the refusal codes itself; that `e3_of` refuses
   a gated cell with a refused Kuttruff is shown only by my perturbation (E3 false, pass false),
   not by any bed unit test.
6. **minor: cargo checks are exit-code only.** Check 21 would pass if `bed::` matched nothing, and
   `a_cells_project_is_the_measured_cells_byte_for_byte` returns ok without running in the gate
   (it never sets `SIMPA_M8A_MEASURED_PROJECT`). Both hold today (25 run; the skipped one passes
   when run).
7. **minor: E1's TetGen leg skips missing records.** `m8a.ps1:211-214` (`if ($t -and …)`) and
   `report.rs:352` (`if let Some(t)`) pass a run with no `mesh.json` or no TetGen sha. 433 of 433
   have one, all equal.
8. **note: the plots check is existence only.** It claims only that the files are written.
9. **note: counts not pinned in the pass logic.** E4/E5 need a non-empty cell list, not eight;
   gated TCR need not be non-empty. E7 pins the matrix, and this bed has 8, 8 and 16.
10. **note:** a receiver one band short panics in `check_a` (`check.rs:130`), fail-closed and
    unreachable (`read_spps` refuses such a run).

## What this bed does not show, from this lens

That the two latent holes are closed; that N4/N7's C margins are large (only that C failed);
that B catches a genuinely noisy seed on real runs (N5 stopped at E6); anything about the
atmospheric validation beyond its solver hashes and step; and whether the physics is right: the
gate only proves the bed judges its numbers against the pre-registered limits as the SPEC says.
