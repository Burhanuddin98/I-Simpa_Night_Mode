# M8b, step 1: adversarial check of the two gate fixes

Checked 2026-09-30, 03:03 to 03:35, on worktree `m8b` at `8c2e603`, against `FIXES.md` beside this
file. No product code was edited. The M8a bed `C:\tmp\nm-m8a-bed\20260929T093134Z` and the smoke bed
`C:\tmp\nm-m8a-smoke\20260929T072041Z` were only read. Everything written is under
`C:\tmp\nm-judge-m8b\`, and nothing was deleted.

**Verdict: both fixes hold. Two tamper-only holes remain outside them, so `ok` is false.**
- Every break case the judge found now fails, for the right reason, on real runs. This includes a
  real exploratory bed with the same cell ids read as `--from`, and real 10 ms runs made by this build.
- Every other attempt short of editing files inside a run folder also fails: a band list with the
  right count, a repeated band, one setting changed, a real run of other settings, and a seed filed
  under the wrong folder.
- No false positive. All 433 runs of the M8a bed are accepted and re-judge to its verdict. So are the
  say-NO reads with their faults set, and a fresh run.
- Two cases still pass. Both need a hand edit of files inside a run folder, and no bed or `--from`
  read produces them (findings 1 and 2).

Receipts beside this file:
- `verify-harness.rs`: a scratch crate built at `C:\tmp\nm-judge-m8b\harness`, target
  `C:/tmp/nm-target`. It calls `simpa_core::bed` as `simpa bed --from` does: `run::read_in` or
  `read_existing`, then `run::into_reads`, then `report::evaluate`. The transports come from the M8a
  `report.json`, and nothing is re-traced.
- `verify-harness-out.txt`: its output. It ran in 627 s. The runs were read 8 at once, beside a
  gate `-From` run another agent was making at the same time.
- `verify-faults.rs` and `verify-faults-out.txt`: the same crate built with `fault-injection`, for
  N2 and N8.

## 1. The judge's break cases, on the fixed code

| Case | Before (judge, `gate-harness-out.txt`) | Now |
|---|---|---|
| Gated `6x10x3-a0.2-tcr-air-on`, 8 kHz removed, after `into_reads` (only D sees it) | `pass true`, 0 failures | `pass false`, TCR Fail: `D Fail: the run's bands are not the bed's for 6x10x3-a0.2-energetic-air-on: missing 8000 Hz` |
| The same run with only 125 Hz left, after `into_reads` | `pass true`, 0 failures | `pass false`, TCR Fail: `… missing 250 Hz, 500 Hz, 1000 Hz, 2000 Hz, 4000 Hz, 8000 Hz` |
| Both cases before `into_reads`, as a run folder would give them | – | Refused `bed_run_not_planned: … bands [125, 250, 500, 1000, 2000, 4000], the bed's [… 8000]` (and `bands [125]`). TCR NotJudged, E2 false, `pass false` |
| `--from` over a real exploratory bed with the same cell ids: the smoke bed, cells `5x4x3-a0.4-*` at 100,000 or 300,000 particles. Its 14 matching keys were read in place of the M8a runs | – | 14 of 14 refused `bed_run_not_planned`. Each SPPS refusal names the project sha256 and `particles per source 100000, the bed's 1500000` (or 300000 against 31000000). Each TCR refusal names the sha256. Bed: `pass false`, E2 false, 20 failures naming `bed_run_not_planned` |
| Real 10 ms runs, made by this build (`run::run_one`) from the M8a bed file with `time_step_s` 0.01, filed under the M8a plan | – | Energetic `5x4x3-a0.4-air-off` seed 1 (170 s): refused, naming the sha256 and `time_step_s 0.009999999776482582, the bed's 0.0010000000474974513`. `5x4x3-a0.4-tcr-air-off`: refused on the sha256. Bed: `pass false`, cell and TCR NotJudged |

The TCR cases are where only the new check stands between a wrong run and a pass:
- Judged by D alone, without `check_planned`, the smoke bed's two TCR reads and the 10 ms TCR read
  each give **D Pass**. TCR does not use the particle count or the step.
- The 10 ms TCR run's four output files are byte-identical to the M8a run's (`cmp`).
- Only the project sha256 refuses them, and it does.

## 2. Attempts to get around each fix

**Defect 1 (D's bands).** Each case was tried twice: on the reads after `into_reads`, where D alone
judges, and on the read before, where `check_planned` judges.

| TCR band list | After `into_reads` | Before |
|---|---|---|
| Right count, 8 kHz relabelled 16 kHz | D Fail: `missing 8000 Hz`; `not the bed's: 16000 Hz` | refused, bands named |
| Right count, 4 kHz twice, no 8 kHz | D Fail: `missing 8000 Hz`; `repeated: 4000 Hz` | refused |
| Right count, 125 Hz twice, no 8 kHz | D Fail: `missing 8000 Hz`; `repeated: 125 Hz` | refused |
| Right count, the 4 kHz row relabelled 8 kHz (8 kHz twice) | D Fail: `missing 4000 Hz`; `repeated: 8000 Hz` | refused |
| All seven, plus 8 kHz again (8 rows) | D Fail: `repeated: 8000 Hz` | refused |
| Air-off run (6 bands), 4 kHz relabelled 8 kHz | D Fail: `missing 4000 Hz`; `not the bed's: 8000 Hz` | refused |

Every case gives `pass false`. `band_problems` returns empty only when the two lists are equal, and it
falls back to naming both lists, so no list other than the bed's can pass D.

**Defect 2 (the run filed under a key).**

| Attempt | Result |
|---|---|
| A real read (`5x4x3-a0.4-energetic-air-off` s3) with one setting changed at a time, step right: `trans_epsilon` 7, `trans_epsilon` 9.000000000001, particles 150,000, particles +1, method 0, seed none, seed 0, bands of the right count with a wrong band, 2 kHz twice, two bands swapped, seed 4's project sha256, step NaN | Each refused, naming the one difference |
| N5's real run (31,000 particles, seed 10) filed as its cell's seed 10 | Refused on the sha256 and `particles per source 31000, the bed's 31000000`. Cell NotJudged, `pass false` |
| The random cell's seed-1 run filed as the energetic cell's | Refused on the sha256, particles, method and `trans_epsilon` |
| `5x4x3-a0.1-energetic-air-off` s3 filed as `5x4x3-a0.05-energetic-air-off` s3: the same particles, method, step, epsilon, seed and bands, with α and duration differing | Refused on the **sha256 alone**. This is the case the six compared settings cannot see |
| The energetic cell's seed-1 SPPS run filed as its TCR run (the same project bytes) | Refused: `a Spps run where the bed runs Tcr` |
| The matrix's `5x4x3-a0.2-energetic-air-off` s1 filed as N6 | Refused on the sha256 and the particle count |
| Seed 4's real folder read as seed 3 | Refused on the sha256 and `random_seed 4, the bed's 3`. With s3 and s4 swapped, `pass false` |
| A 1 ms run folder carrying the 10 ms run's output files | Not read: `results_file_invalid: … its time step is Some(0.01), config.xml's pasdetemps is 0.001` |
| Seed 4's run with run.json's sha256 edited to seed 3's plan | Refused: `random_seed 4, the bed's 3` |
| A TCR folder carrying the 10 ms TCR run's outputs | Accepted, but those outputs are byte-identical to its own. Not a bypass |
| **Seed 3's folder with seed 4's 20 output files** (run.json, config.xml and the mesh left as seed 3's) | **Accepted, `pass true`.** Its T30 is seed 4's (finding 1) |
| **Seeds 2 to 10 each carrying seed 1's outputs** | **Accepted, `pass true`, cell Pass on ten identical seeds** (finding 1) |
| `5x4x3-a0.1-energetic-air-off`'s run with run.json's sha256 edited to `a0.05`'s plan | Accepted by `check_planned`. The bed still fails: one seed fails E3 (Kuttruff 0.994 s against seed 1's 2.022 s); all ten fail C (d = −50.78 %) (finding 2) |
| **TCR `5x4x3-a0.2-tcr-air-off`'s run with run.json's sha256 edited to `5x4x3-a0.4-tcr-air-off`'s plan** | **Accepted, `pass true`, TCR Pass** (finding 2) |

## 3. False positives

| Check | Result |
|---|---|
| All 433 planned runs of the M8a bed, read with this build | Accepted: cells 400 of 400, TCR 20 of 20, N5, N6, atmospheric SPPS 10 of 10 and atmospheric TCR 1 of 1. 0 refused, 0 unread |
| Project sha256, three ways | `project.simpa` on disk = run.json's `source.sha256` in 433 of 433. run.json's = the plan's in 433 of 433 |
| Bands per run | Cells: air off 6 bands (240 runs), air on 7 (160). TCR: air off 6 (12), air on 7 (8). Atmospheric: 27 (10 SPPS, 1 TCR). N5 6, N6 6 |
| `config.xml` read as text, independent of the reader | `random_seed` = folder seed, `nbparticules` = the plan's (N5 31000), `pasdetemps` 0.001. 0 problems in 412 SPPS runs |
| Plans of the say-NO runs | N5: 31,000 particles, seed 10. N6: scattering 0 on every band |
| The bed re-judged through `into_reads` | `pass true`, 0 failures, E2 true. 40 of 40 cell and 20 of 20 TCR verdicts as in `report.json`. 0 D band problems. N5 and N6 `as_required`, no error. 0 atmospheric errors |
| N2's reads with `KuttruffFullVariance` set, through `into_reads` | 80 of 80 accepted. A Fail in 8 of 8 α 0.4 gated cells |
| N8's reads with `TcrAnalyticPhysicalConstant` set | 20 of 20 accepted. D Fail in 104 of 104 gated bands, 0 band problems |
| Fresh runs: `run_one` at 10 ms against its own 10 ms plan | Both accepted. The bytes `run_one` saves hash to the plan's sha256 |
| Float comparison of the step and `trans_epsilon` | The writer prints `{v}`, the shortest text that round-trips (`config_xml/num.rs:21`). The reader applies `atof`, then `f32` (`locate::to_float`). `as_spps_reads` is `v as f32`. Identical, so no rounding false positive |
| The new `D bands` row in `summary.json` | Nothing in `m8a.ps1` or `tools/bed/m8a_plots.py` reads summary rows. The plots read `report.json`'s cells |
| `cargo test -q -p simpa-core --lib bed::` | 29 passed, 1 ignored |
| The product's own `simpa bed --from` on this commit. Another agent's gate run (`C:\tmp\nm-target\gates\m8a\20260930-025530\reread\20260930T005634Z\report.json`), read only | `bed PASS`: gated cells 32 Pass, 0 failures, N5 and N6 `as_required`, 0 D band problems. `git_commit` `8c2e603`, `git_dirty` false (this file was written into the worktree only after that report) |
eread60930T005634Z
eport.json`), read only | `bed PASS`: gated cells 32 Pass, 0 failures, N5 and N6 `as_required`, 0 D band problems. `git_commit` `8c2e603`, `git_dirty` false (this file was written into the worktree only after that report) |

## 4. Findings

1. **minor: a run's outputs are not bound to its run.json, and nothing requires the seeds to
   differ.**
   - Failure: seed 3's run folder was left with its own run.json, config.xml and mesh, and its 20
     output files were overwritten with seed 4's. `check_planned` accepted it, and the bed gave
     `pass true`.
   - With seeds 2 to 10 all carrying seed 1's outputs, the cell passed A, B and C on ten identical
     seeds.
   - Why: run.json hashes only the inputs (`results.rs` `check_inputs`), and the outputs are judged
     again only by the verdict's output signals.
   - The fix did not claim this. It is M7's manifest design, and only hand surgery in `solve/`
     reaches it.
   - Suggested closure: record the outputs' sha256 in run.json and check them in `results::load`, or
     add a bed check that no two seed runs of a cell carry equal T30. The second also catches a solver
     that ignores its seed.
2. **minor: run.json's `source.sha256` is the only thing binding α, materials, scattering, geometry,
   duration and air to the plan.**
   - Failure: TCR `5x4x3-a0.2-tcr-air-off`'s run, with run.json's sha256 edited to the α 0.4 plan's,
     was accepted and the bed gave `pass true`.
   - D's analytic reference is computed from the same run's config.xml, so nothing independent checks
     a TCR run.
   - For SPPS the same edit is accepted by `check_planned`, but the bed fails at E3 or C, against the
     bed's own transport.
   - The same trust means that a bed from a build whose config.xml writer differs, but which writes
     the same project bytes, would be accepted. This was not demonstrated.
     `crates/simpa-core/src/config_xml` did not change between `da6f15c` and `8c2e603`.
   - Suggested closure: compare run.json's recorded config.xml sha256, which `results::load` already
     verifies, with the config.xml this build writes for the planned project, with
     `workingdirectory` normalised. For TCR alone, comparing the analytic inputs (V, S, ᾱ) with the
     plan's would also do.

Neither finding is a configuration mismatch that `--from` over a real bed can produce. Each needs a
file inside a run folder rewritten by hand. They are reported because the check asked for exactly
these cases, and `ok` is false by its rule.

## Scratch left in place (nothing deleted)

- `C:\tmp\nm-judge-m8b\harness\` and `harness-faults\`: the crates. Their binaries are in
  `C:\tmp\nm-target\release\`.
- `C:\tmp\nm-judge-m8b\harness-out.txt`, `harness-err.txt` (empty) and `faults-out.txt`.
- `C:\tmp\nm-judge-m8b\bed10\`: the two 10 ms runs.
- `C:\tmp\nm-judge-m8b\tamper\`: the tampered copies of M8a run folders.
- `C:\tmp\nm-judge-m8b\VERIFY-adversarial.md`: the draft of this file.

## What this does not show

- **The full gate.** Another agent was running `m8a.ps1 -From` on this commit at the same time.
  Only its `simpa bed --from` step had finished when this was written (section 3). The say-NO
  lines and the verdict are that run's own receipt.
- **N4 and N7.** They were not run here. Their faults act on the transports, not on the reads that
  `check_planned` sees, and their reads are the ones checked above without a fault.
- **An extension.** No gate C extension exists in the M8a bed. `plan_extension`'s runs go through
  the same `into_reads`. A refused extension seed fails the extension's E2, and C becomes NotJudged.
