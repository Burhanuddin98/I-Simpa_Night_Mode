# M8b, step 1: the two latent M8a gate defects, fixed

Branch `m8b`, cut from `rebuild` at `57ef997`. The defects are decision row 23 and
`../2026-09-29-m8a/VERDICT.md` Major 1 and 2. The judge's receipts are `../2026-09-29-m8a/judge/gate.md`
findings 1 and 2, `gate-harness.rs` and `gate-harness-out.txt`. **The full gate (`tools/gates/m8a.ps1`)
was not run.** The M8a bed `C:\tmp\nm-m8a-bed\20260929T093134Z` was only read, never written.

Receipts beside this file:
- `failing-first.txt`: each revert as a diff, and the test output with the revert and after the restore.
- `fixcheck.rs`: a scratch crate, built outside the worktree. It reads the M8a bed's 433 runs through
  this build.
- `fixcheck-out.txt`: that crate's output.

Line numbers are those of this commit.

## Defect 1: D judged whatever bands a TCR run held

**Before.** At `57ef997`, `evaluate_tcr` passed `r.bands` straight to `check_d` (`check.rs:1051-1057`),
and `check_d` passed any non-empty list. The judge's harness took the gated 6x10x3-a0.2-tcr-air-on run
and tried two cases: its 8 kHz band removed, and only its 125 Hz band left. Each gave `pass true` with
0 failures (`gate-harness-out.txt:77-80`).

**Change.**
- `check.rs:252`: `band_problems(want, got)` names the difference between two band lists. It names
  each band missing, each band that is not the bed's, each band repeated, and the order of the shared
  bands when that order differs. It returns empty exactly when the lists are equal and in the same
  order.
- `check.rs:294-325`: `check_d(bands_hz, bands, limit)` now takes the bed's bands. D passes only when
  there are no band problems and every band is within 0.5 %.
- `check.rs:100`: the band problems are stored in `CheckD.band_problems`. The field has
  `serde(default)`, so the M8a `report.json` still reads.
- `check.rs:1068`: `evaluate_tcr(bed, …)` takes the bed.
  - The bands come from `bed.bands_hz(cell.air)` of the cell `project_of` (`check.rs:1108-1115`).
  - A TCR run whose `project_of` is not a cell is not judged.
  - Each band problem is a failure, for example `6x10x3-a0.2-tcr-air-on: D Fail: the run's bands are
    not the bed's for 6x10x3-a0.2-energetic-air-on: missing 8000 Hz` (`check.rs:1126`).
- `report.rs:293` passes the bed to `evaluate_tcr`.
- `report.rs:857` adds a `D bands` row per TCR run to `summary.json`: the number of band problems, with
  a limit of 0. Without it, a D Fail on a missing band would have no row in the summary that shows it.
- No other check judges a TCR run's bands. R8 and the atmospheric TCR comparison are reported only,
  and they match bands by frequency.

**Tests.**
- `check.rs:1399` `d_fails_a_run_whose_bands_are_not_the_beds` uses the gated 6x10x3-a0.2-tcr-air-on
  run of the real matrix:

  | Run | Verdict | Failure names |
  |---|---|---|
  | Its seven bands | Pass | – |
  | 8 kHz removed | Fail | `missing 8000 Hz` |
  | Only 125 Hz | Fail | `missing 250 Hz, 500 Hz, 1000 Hz, 2000 Hz, 4000 Hz, 8000 Hz` |
  | A 16 kHz band added | Fail | the extra band |
  | 8 kHz twice | Fail | the repeat |
  | 125 and 250 Hz swapped | Fail | the order |

  The air-off run passes on its six bands and fails with 8 kHz added.
- `bed.rs:431` `a_gated_tcr_run_short_of_a_band_does_not_pass_the_bed` works at report level, on a
  small bed with a gated air-on TCR run:
  - untouched: `pass true`;
  - 8 kHz removed: `pass false`, the failure names 8000 Hz, and the summary's `D bands` row is `fail`;
  - only 125 Hz: `pass false`, and the six missing bands are named.
- `check.rs:1373` `d_holds_tcr_to_its_analytic_value` is updated to the new signature.

**Failing-first** (`failing-first.txt`, "DEFECT 1").
- Revert (`check.rs:296`): `let band_problems = band_problems(bands_hz, &got);` becomes
  `let band_problems: Vec<String> = Vec::new();`.
- With the revert, both tests FAILED:
  - `check.rs:1443`: `left: Pass right: Fail`. The run with its 8 kHz band removed is judged Pass.
  - `bed.rs:469`: `assertion failed: !r.pass`. The bed passes.
- Restored (byte for byte, `cmp`): `2 passed; 0 failed`.

**On the M8a bed's real runs** (`fixcheck-out.txt`). These are the same two cases that gave `pass true`
in the judge's harness:

| 6x10x3-a0.2-tcr-air-on | `pass` | TCR run | Failure |
|---|---|---|---|
| 8 kHz removed | false | Fail | `… missing 8000 Hz` |
| Only 125 Hz | false | Fail | `… missing 250 Hz, 500 Hz, 1000 Hz, 2000 Hz, 4000 Hz, 8000 Hz` |
| Untouched | true | Pass | – |

## Defect 2: no run was matched with the cell and seed it is filed under

**Before.** `SppsRead` carried the particle count, method, `time_step_s` and `trans_epsilon`, and
`run.json` carried the project's sha256. Nothing compared them with the matrix or the seed.
Consequence: `m8a.ps1 -From` over another bed with the same cell ids, for example an exploratory bed at
10 ms, would have printed `M8a PASSED`.

**Change.**
- `read.rs:41`: `RunInfo.project_sha256` is read from `run.json`'s `source.sha256` (`read.rs:97`).
  - It is `None` for a fixture run.
  - It has `serde(default)`, so the M8a report still reads and validates (checked below).
  - `report.json` now shows it for every run.
- `read.rs:285`: `SppsRead.random_seed` is `config.xml`'s `random_seed` as SPPS reads it, through
  `report_of` (`read.rs:315-320`). The results report does not carry the seed.
- `run.rs:411`: the refusal code, `RUN_NOT_PLANNED = "bed_run_not_planned"`.
- `run.rs:415`: `planned_project_sha256(p)` is the sha256 of `schema::to_json(&p.project)`.
  - These are the bytes `run_one` saves as `project.simpa`, and the run manager hashes them into
    `run.json` when the run starts.
  - For a cell seed, `p.project` is `file::cell_project(spec, seed, bed.time_step_s, bands)`. The
    same holds for a TCR run (seed 1), N5, N6 and an extension seed.
  - For an atmospheric seed, it is upstream's imported project with section 2.5's three changes.
- `run.rs:443`: `check_planned(p, read)` refuses a run unless all of these hold:
  1. The solver is the one planned.
  2. `run.json`'s project sha256 is the planned project's.
  3. For SPPS, what the solver read equals the planned project's settings:
     - particles per source;
     - `computation_method`;
     - `time_step_s` and `trans_epsilon` as SPPS reads them, `f64::from(v as f32)`. This is compared
       exactly: the writer prints the shortest text that round-trips, and SPPS reads it with `atof`
       into a `float`;
     - `random_seed`;
     - the computed bands, in order.
  4. For TCR, the band list.

  Every difference is named. For example: `bed_run_not_planned: <folder> is not the run the bed files
  under 5x4x3-a0.2-energetic-air-off seed 2: its project has sha256 …, the project the bed writes for
  it …; time_step_s 0.009999999776482582, the bed's 0.0010000000474974513`.
- `run.rs:538`: `into_reads` calls `check_planned` on every run before filing it.
  - Every read passes through `into_reads`: a fresh `simpa bed` (`run::execute`), one with `--from`
    (`run::read_existing`), gate C's extension in both cases (`bed_cmd.rs:246-248, 297-298`), and
    `bed_m8a.rs`'s `read`.
  - A refused run becomes an `Err`, which is handled as a run that could not be read:

    | Run refused | Effect |
    |---|---|
    | Cell seed | E2 fails and the cell is NotJudged, so it does not pass |
    | TCR run | Not judged, and E2 fails |
    | N5 or N6 | `error` is set, so `as_required` is false and the gate's N5 or N6 line fails |
    | Atmospheric seed | Its `SeedRun.error` is set (reported only, as before) |

**Tests.**
- `run.rs:765` `a_run_at_10_ms_filed_under_a_1_ms_cell_is_refused` holds runs against the real
  matrix's plan for 5x4x3-a0.2-energetic-air-on, seed 3. The runs are written out literally, not
  taken from the plan.
  - The run of its own project is accepted.
  - The same cell and seed from a bed at 10 ms is refused, naming the step and the project sha256.
  - Each of these is refused and named:
    - the step alone, with the right project bytes;
    - the project alone;
    - a fixture run with no project;
    - seed 4's run filed under seed 3 (`random_seed 4, the bed's 3`);
    - the particle count, the method, `trans_epsilon` and the band list, each changed alone.
  - N5: its own run (31,000 particles, seed 10) is accepted. The cell's full-count seed-10 run filed
    as N5 is refused.
  - N6: a run with the matrix's walls (scattering 1) filed as N6 is refused on its project.
  - TCR: its own run is accepted. A run one band short, a run of another project, and an SPPS run
    where TCR is planned are each refused.
- `bed.rs:521` `a_run_at_10_ms_filed_under_a_1_ms_cell_does_not_pass_the_bed` works at report level,
  through `run::plan`, then `run::into_reads`, then `report::evaluate`.
  - With every run the plan's own: `pass true`.
  - With seed 2 replaced by the same seed from a 10 ms bed (its project and its step):
    - `pass false`, with E2 false and the cell NotJudged;
    - seed 2's error starts with `bed_run_not_planned` and names the step and the project;
    - a failure reads `seed 2: bed_run_not_planned`.

**Failing-first** (`failing-first.txt`, "DEFECT 2").
- Both halves reverted: `check_planned` returns Ok for every run (`if true || problems.is_empty()`),
  and the call in `into_reads` is removed. Both tests FAILED:
  - `bed.rs:570`: `assertion failed: !r.pass`. The 10 ms seed passes the bed, which is the judge's
    finding.
  - `run.rs:781`: `unwrap_err()` on an `Ok` value. The 10 ms run is accepted.
- The call alone reverted (`check_planned` intact): the report-level test FAILED at `bed.rs:570`, and
  the direct test passed. So the report-level test is what guards the call site.
- Restored (byte for byte, `cmp`): `2 passed; 0 failed`.

**On the M8a bed's real runs** (`fixcheck-out.txt`, read-only, 285 s with 8 at once, the transports
taken from the bed's `report.json`, nothing re-traced):
- **433 of 433 planned runs match their plan: 0 refused, 0 unread.** These are 400 cell runs, 20 TCR,
  N5, N6, 10 atmospheric SPPS and 1 atmospheric TCR (upstream's tree read from
  `B:\repos\I-Simpa-upstream`).
- Re-judged: `pass true`, 0 failures, E1 over 433 runs.
  - 40 of 40 cell verdicts and 20 of 20 TCR verdicts are as in `report.json`.
  - N5 and N6 are `as_required`.
  - There are 0 D band problems.
- The bed's own 1 ms runs, filed under a plan at 10 ms, are each refused `bed_run_not_planned`. For
  5x4x3-a0.4-energetic-air-off seeds 1 to 3, the refusal names the project sha256 and the step. For
  5x4x3-a0.4-tcr-air-off, it names the project.

## Checks run

On this commit's tree, `CARGO_TARGET_DIR=C:/tmp/nm-target`:

| Check | Result |
|---|---|
| `cargo test -q -p simpa-core --lib bed::` | 29 passed, 1 ignored (the eight-cell transport trace, run on purpose), 179 filtered out. Before: 25 passed. The 4 new tests are the ones above. |
| `cargo test -p simpa --test bed_m8a` | 0 passed, 6 ignored. Each needs a bed that has run. |
| `SIMPA_BED_REPORT=beds/m8a-20260929T093134Z/report.json cargo test -p simpa --test bed_m8a -- --ignored --exact report_validates_against_its_schema` | ok. The M8a report validates against the new schema and reads back (`pass true`, 40 cells, 0 failures). It reads the report only. |
| `cargo clippy -q -p simpa-core -p simpa --all-targets -- -D warnings` | clean |
| `cargo fmt -p simpa-core -p simpa --check` | clean |

## What this does not show

- **The gate itself.** `m8a.ps1` was not run. Nor were `bed_m8a`'s ignored say-NO tests (N2, N3, N4,
  N7, N8), which read the bed through `into_reads`. The fixcheck shows that every read they make
  matches its plan.
- **Beds from another build.** `check_planned` holds a run to the project this build writes. A bed made
  by a build whose `cell_project`, template or `schema::to_json` writes other bytes is refused in full,
  even when its settings are the same. That is deliberate. Between `da6f15c` (the build of the M8a bed)
  and `57ef997`, `schema/`, `bed/file.rs` and the template did not change, and 433 of 433 runs match.
- **The project file as it is now.** The project is matched by `run.json`'s `source.sha256`, the bytes
  the run started from. The settings are matched by what `results::load` reads from the `config.xml`
  whose hash the manifest verifies. `project.simpa` itself is not hashed again.
- **E1's count for a refused run.** A refused run has no `RunInfo` in the report, so E1 does not count
  its executable. E2 fails for it anyway.
- **The judge's minor findings.** VERDICT minors 3 to 8 are not addressed here.
