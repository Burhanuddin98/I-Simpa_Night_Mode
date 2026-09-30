# M8b tamper: the two tamper-only holes of the M8a gate, closed

Worktree `m8b-tamper`, branch `m8b-tamper`, cut from `rebuild` at `5d46aa7` (M8a, M11 and M8b step 1).
The code is commit `239c6f5`; this record and its receipts are the commit after it.
The holes are `docs/v1.1-backlog.md` rows 41 and 42, and `../2026-09-30-m8b-gate/VERIFY-adversarial.md`
findings 1 and 2. Burhan, 04:29: "dont take any chances". So nothing here is closed on a guess, and every
claim has a test or a receipt. The M8a bed `C:\tmp\nm-m8a-bed\20260929T093134Z` was only read, never
written, and the 6-hour bed was not rerun. Nothing was deleted.

**Verdict: both holes are closed, on the unit tests and on the real bed.**
- Every tampered copy that gave `pass true` before is now refused, with a named code.
  - This holds with the real record.
  - It also holds with a forged record, where the next layer refuses it.
- Every fix failed its tests when reverted alone, and passed once restored byte for byte: 8 reverts,
  one of them on the real bed's runs.
- The real M8a bed still passes:
  - All 433 of its runs are bound by the committed seal and matched to their plan.
  - Re-judged, it gives `report.json`'s verdicts with the same numbers. The one exception is D's
    reference, which is now the plan's. It differs from the run's own by at most 2.6 × 10⁻¹⁶ relative
    (2 ulp), and no verdict changes. *(Round 2, finding R3: this said 2.2 × 10⁻¹⁶, which 13 of 128
    bands exceed.)*
- Without the seal, 433 of 433 runs are refused `bed_run_unbound`.

Receipts beside this file:

| File | What it is |
|---|---|
| `failing-first.txt` | Each revert as a diff, the command, the test output with the revert, the byte-for-byte restore, and the output after it |
| `failing_first.py` | The driver that made it |
| `harness.rs`, `harness-out.txt` | A scratch crate, built at `C:\tmp\nm-judge-tamper\harness` with target `C:/tmp/nm-target`. It reads the real bed and the tampered copies through this build |
| `seed-identity-probe.py`, `seed-identity-probe-out.txt` | The measurement behind the seed rule |
| `cli.txt` | The product CLI end to end (`simpa bed --from … --seal …`), and its refusals of a bad `--seal` |

Line numbers are those of the code commit, `239c6f5`.

## Hole 41: a run's outputs were bound to nothing, and seeds could be copies

**Before.** `run.json` hashed only the inputs. Two copies passed `check_planned` and the bed:
- seed 3's folder with seed 4's 20 output files (its own `run.json`, `config.xml` and mesh);
- seeds 2 to 10 each carrying seed 1's outputs. The cell passed A, B and C on ten identical seeds.

**Change.**

1. **`run.json` records what the run left.** This covers every solver (SPPS, TCR) and TetGen's outputs.
   - `run/manifest.rs:78` `OutputRef { path, size, sha256 }`.
   - `run/manifest.rs:234` `RunManifest::outputs: Option<Vec<OutputRef>>`. It has
     `serde(default, skip_serializing_if = "Option::is_none")`, so a `run.json` written before this
     reads as `None` and writes back byte for byte (`tests/run_manifest.rs:139`).
   - `run/manifest.rs:142` `hash_tree(dir, except)`: every file under the folder, recursively, with
     `/` paths, its size and sha256, sorted. A link, or anything else that is not a file or a folder,
     is an error, never skipped. So is a name that is not UTF-8.
   - `run/manager.rs:493`: `Record::manifest` hashes the run folder, all but `run.json`, as the last
     thing before it writes `run.json`, on every ending:
     - the solver's outputs;
     - the inputs in `solve/` as they then stand;
     - the logs;
     - `mesh/`, which holds TetGen's outputs and `mesh.json`.
   - If the folder cannot be read at the end, `outputs` is `None`, and the bed refuses the run.
   - `docs/solver-contract.md:480` documents the key.

2. **The bed binds every file of a run to its record before it reads a number from it.**
   `bed/bind.rs` (new):
   - `bind.rs:210` `bind(dir, folder, outputs, inputs, sealed)` hashes the run's folder under the bed,
     `runs/<cell>/s<seed>/`.
   - `bind.rs:182` `differences`: against `run.json`'s `outputs`, it holds every file of the run folder
     but `run.json`. Against the seal's entry, it holds every file of `runs/<cell>/s<seed>/`, including
     `project.simpa` and `run.json`. Both are checked when both exist.
   - A file changed, gone or added is refused **`bed_run_files_changed`** (`bind.rs:30`). The refusal
     names each file with both sizes and hashes.
   - A run with neither record is refused **`bed_run_unbound`** (`bind.rs:28`, `:223`), not judged.
   - `bed/run.rs:342` `read_folder` binds first (`:349`), then hashes from disk what the plan check
     needs (`:356`), and only then reads. `read_in` (`:333`) and `read_existing` (`:432`) take the
     seal. `run_one` binds a fresh run to its own `run.json`.
   - `read.rs:51-63`: the result goes into `RunInfo`:
     - `bound_by` (`run.json`, `seal`, or both);
     - `outputs_sha256`;
     - the solver's output files, which are held in memory and not written to the report.
     All have `serde(default)`, so the M8a report still reads.

3. **No two seeds of a cell share a solver output file.**
   - `bed/run.rs:735` `refuse_shared_outputs`, called by `into_reads` (`:725`, `:727`) over each cell's
     seeds read so far (an extension's with the first ten), and over the atmospheric validation's.
   - The solver's outputs are the files under `solve/` that are not among `run.json`'s inputs
     (`bind.rs:290`).
   - Every seed that shares any output file byte for byte, at the same path, with another seed is
     refused **`bed_seed_outputs_identical`** (`bind.rs:32`). So is a seed whose whole output set is
     another's. The error names the other seeds and the files.
   - This is stricter than "the same sha256 set". It also refuses a partial copy, and a solver that
     ignores its seed for some receivers only.
   - It is safe on real data. On the M8a bed, 0 of 8,490 solver output files is byte-identical to
     another seed's at the same path, over the 41 groups of seeds (40 cells and the atmospheric
     validation), and 0 groups have two equal whole sets (`seed-identity-probe-out.txt`).
   - A future output that did not depend on the seed would be refused loudly, never accepted.
   - *(Round 2, finding N1: it refused a valid bed. At 31,000 particles per source, two seeds wrote
     the same `SPPS particle statistics.gabe`. Since `fbd346d`, only a receiver's file shared refuses
     two seeds.)*

4. **The seal of the M8a bed.** `bind.rs:40` `Seal`. `bind.rs:99` `Seal::load` checks it
   (`bind.rs:110`) before use:
   - its version;
   - that every path is relative, with `/` and no `.` or `..` parts;
   - that every sha256 is 64 lower-case hex digits;
   - that each path appears once;
   - that each run folder has its `run.json`;
   - that the totals add up.

   Its hash is taken with line ends as git stores them. `bind.rs:149` `for_bed` refuses a seal of
   another bed folder. The seal is described in "The seal" below.

## Hole 42: run.json's word was the only link between a run and its plan

**Before.**
- `run.json`'s `source.sha256` was the only thing that bound α, the materials, the scattering, the
  geometry, the duration and the air to the plan.
- TCR `5x4x3-a0.2-tcr-air-off`'s run, with that sha256 edited to `5x4x3-a0.4-tcr-air-off`'s plan and
  filed as α 0.4's, was accepted, and the bed passed. D's analytic reference came from the run's own
  `config.xml`.

**Change.**

1. **The files are hashed from disk and held to the plan.**
   - `read.rs:71` `OnDisk` and `read.rs:102` `on_disk` hash, from disk:
     - the `project.simpa` beside the run folder;
     - `solve/config.xml`, with the value of its `workingdirectory` left out. It is the one thing in
       the file that depends on where the run was made (`bind.rs:309` `without_workdir`);
     - `solve/mesh.cbin` and `solve/tetramesh.mbin`.
   - It also records the mesh stamp and the `.mbin` sha256 as `run.json` and `mesh/mesh.json` give
     them.
   - It runs the run manager's own reused-mesh check (`run::manager::reused_mesh_check`,
     `read.rs:90`). That check holds the `.mbin` the solver read to the scene it read, and takes no
     `mesh.json` as proof. It is needed because the mesh stamp is FNV-1a, which finds accidental change
     only.
   - `bed/run.rs:464` `planned_inputs(p)` computes the same values from the plan. It runs the project
     through `run::manager::project_as_run` (`run/manager.rs:631`), the step `run_project` itself now
     uses (`:667`). It then takes:
     - the sha256 of the bytes `run_one` saves;
     - `config_xml::write` for the planned solver, its `workingdirectory` left out;
     - `cbin::write(config_xml::scene_mesh(..))`;
     - `validate::mesh_input_hash`.
   - `bed/run.rs:494` `disk_problems` names each difference. `check_planned` refuses the run with it,
     `bed_run_not_planned` (`:662`), for any of these:
     - `project.simpa` on disk;
     - `config.xml`;
     - the scene;
     - either mesh stamp;
     - the `.mbin` not being one file in all three places;
     - a failed mesh check.
   - A read that went through no binding, or whose files were not hashed from disk, is refused too.
     No code path reaches `into_reads` without them.
   - `run.json`'s own sha256 is still compared, as before.

2. **D's reference is the plan's.**
   - `results/tcr.rs:177` `analytic_of_project(project)` computes TCR's analytic Sabine and Eyring times
     on the inputs this build exports for the project:
     - its `config.xml` (materials, bands, air), read exactly as a run's is;
     - its scene mesh;
     - for the volume, the volume that scene encloses (`tcr.rs:205` `enclosed_volume`, the divergence
       theorem). This stands in for the `.mbin`'s, which only TetGen makes.
   - A scene that does not close is refused: every edge must be crossed once each way.
   - `RoomInputs::read` is split so that both paths share one reader (`tcr.rs:337` `RoomInputs::of`,
     `:424` `no_fittings`).
   - `bed/check.rs:1206` `planned_analytic(bed, t)` takes the planned project of the TCR run
     (`bed/file.rs:787` `tcr_project`, which `run::plan` now uses too).
   - `evaluate_tcr` holds TCR's Eyring time to it (`check.rs:1129`, `:1141`). R8's Sabine ratio also
     uses it.
   - The run's own analytic time is still reported, never judged, as `BandD::run_analytic_eyring_s`
     (`check.rs:121`, `serde(default)`).
   - Say-NO N8 (`crates/simpa/tests/bed_m8a.rs:313`) now holds its fault over the judging as well as
     the reading, because that is where D's reference is now computed.

## The seal (`beds/m8a-20260929T093134Z/outputs-seal.json`)

**What it holds.**
- One file, 2,531,856 bytes, sha256 `762f3fba…c15e09d` (as written, with LF).
- The bed's 433 runs, **16,861 files and 1,118,668,944 bytes**, each file as `[path, size, sha256]`
  under its run's folder `runs/<cell>/s<seed>/`.
- `report.json` sha256 `5c0041fc…0fad` and `summary.json` `6af1656e…3ced`. These are the bed's own
  files, and also the blobs committed at `beds/m8a-20260929T093134Z/`.
- The newest file under `runs/` against `report.json`'s time.
- Its `why`.

**How it was made.**
- `tools/bed/seal_bed.py`, run at 05:03 on 2026-09-30 over the bed, which it only read.
- The script is deliberately not the Rust that checks the seal. The two walk and hash the files
  independently, so a re-read that accepts every run checks both. It did: 433 of 433 runs were bound
  (`harness-out.txt`, A1).

**Why sealing the files as they were is justified.** Four checks:

| Check | Source | Result |
|---|---|---|
| T30 recomputed | The M8a judge (`../2026-09-29-m8a/judge/t30.md`) | T30 recomputed from the raw `.recp` of all 400 cell runs reproduced `report.json`'s A, B and C to 6.4 × 10⁻⁵. No verdict changed |
| Bed re-read | M8b step 1 (`../2026-09-30-m8b-gate/REJUDGE.md`) | All 433 runs re-read and re-judged bit-identical to `report.json`. The bed's fingerprint was the same at 02:54:59 and 04:12:32 |
| Bed at sealing | Measured here | The same fingerprint: 16,985 files, 1,238,241,863 bytes, newest mtime 2026-09-29T15:25:01.891Z (`plots\summary.png`), and the same two report hashes |
| mtimes | Measured here | No file under `runs/` is newer than `report.json`: newest 14:58:36Z against 15:24:38Z |

**Limit of the provenance.**
- The judge recorded no per-file hashes. So the seal proves that the files are what they were at
  05:03. It does not prove they are byte-identical to what the judge read.
- The link is the unchanged fingerprint, and a re-judge that reproduces `report.json` bit for bit
  (`harness-out.txt`, A1).

## Tests, each failing-first

`failing-first.txt` holds each revert as a diff, and the output with it and after the restore. Every
restore was checked byte for byte, and the files' sha256 were compared before and after the whole run.

| Case | Test | Revert (only the fix) | With the revert | Restored |
|---|---|---|---|---|
| The seed-copy tamper | `bed.rs:651` `a_seed_carrying_another_seeds_outputs_does_not_pass_the_bed` (seed 3 carries seed 2's outputs) | `into_reads` no longer calls `refuse_shared_outputs` | FAILED `bed.rs:655` `!r.pass` | ok |
| Ten identical seeds | `bed.rs:680` `ten_seeds_carrying_one_seeds_outputs_do_not_pass_the_bed` | the same | FAILED `bed.rs:686` `!r.pass` | ok |
| A modified output file under `run.json`'s hashes | `bind.rs:388` `a_run_bound_to_run_json_is_refused_when_any_file_is_not_its_records` (an output overwritten, an input of the same size changed, a file added, a record's file gone) | `bind` skips the `run.json` comparison | FAILED at the first tamper, `unwrap_err` on `Ok` | ok |
| A modified file under the seal | `bind.rs:445` `a_run_bound_to_a_seal_is_refused_when_any_file_is_not_its_records` (`run.json` edited, `project.simpa`, an output, another run folder, a file added) | `bind` skips the seal comparison | FAILED at `run.json` edited | ok |
| No hashes and no seal entry | `bind.rs:494` `a_run_with_no_output_hashes_and_no_seal_entry_is_refused` | the `bed_run_unbound` return disabled | FAILED `unwrap_err` on `Ok` | ok |
| `run.json` records what the run left (a real SPPS run) | `tests/run_manager.rs:176` `run_json_records_every_file_the_run_left`: every file compared with an independent walk, and a run refused before launch too | `outputs` set to `None` | FAILED `expect("outputs recorded")` | ok |
| The edited-sha256 TCR tamper | `run.rs:1194` `a_tcr_run_whose_run_json_names_another_plan_is_refused_on_its_files` | `check_planned` no longer adds `disk_problems` | FAILED `unwrap_err` on `Ok` | ok |
| A mismatched `project.simpa` on disk (and `config.xml`, the scene, each stamp, the `.mbin`, the mesh check) | `run.rs:1233` `a_run_whose_files_on_disk_are_not_the_plans_is_refused` | the same | FAILED | ok |
| An unbound or unhashed read | `run.rs:1295` `a_read_not_bound_or_not_hashed_from_disk_is_refused` | the same | FAILED | ok |
| The TCR tamper at report level: refused through `into_reads`; judged without that refusal, D fails against the plan | `bed.rs:707` `a_tcr_run_of_another_plan_does_not_pass_the_bed` | (a) the disk check reverted | FAILED `bed.rs:741`: the run was judged, and D failed it (Fail, not NotJudged) | ok |
| | the same test | (b) D uses the run's own analytic time | FAILED `bed.rs:750` `!r.pass` | ok |
| D holds TCR to the plan | `check.rs:1455` `d_fails_a_run_whose_bands_are_not_the_beds` (air-on times under the air-off plan) | D uses the run's own analytic time | FAILED `check.rs:1565`, `Pass` against `Fail` | ok |
| D on the real M8a runs | `bed_m8a.rs` N8, with the seal | N8's fault over the reading only, as before M8b | FAILED: D **Pass** in every band (the run's own analytic time is no longer D's reference) | ok, D Fail 104 of 104 |

**Also new, not reverts.**
- `bind.rs:528` `a_seal_is_checked_before_it_is_used`.
- `bind.rs:587` `the_working_directory_is_left_out_of_config_xml_and_nothing_else`.
- `tcr.rs:677` `the_analytic_times_of_a_project_are_its_boxs`. For all 20 M8a TCR runs it checks:
  - the volume is the box's, exactly, and so is the area;
  - the Eyring time equals `params::room`'s to 1e-12;
  - a turned face is refused.
- `manifest.rs:345` `a_tree_is_hashed_file_by_file`.
- `tests/run_manifest.rs:139` `the_outputs_are_written_only_when_recorded`.
- `crates/simpa/src/bed_cmd.rs:484` `a_seal_is_taken_only_with_from`.

## On real runs (`harness-out.txt`)

**A. The M8a bed, read only, with the committed seal (8 at once).**
- 433 of 433 runs were bound by the seal and accepted by `check_planned`. The read took 283 s on a
  quiet machine, and 411 s in the final run, while the CLI's transport phase shared every core. Step
  1's build took 285 to 290 s, so the hashing adds little.
- Every plan check held on every run: `project.simpa`, `config.xml`, the scene, the stamps, the
  `.mbin` and the mesh check. This covers the 10 atmospheric SPPS runs and the atmospheric TCR run,
  whose meshes went through `preprocess.exe` (each `mesh.json` records the call, exit 0). So there
  is no false refusal.
- Re-judged with `report.json`'s transports: `pass true`, 0 failures, E1 over 433 runs, E2 true, gated
  cells 32 Pass.

Against `report.json`, both read with the crate's exact JSON reader:

| Section | Numbers | Result |
|---|---|---|
| Cells | 39,159 | Bit-identical |
| Preconditions | 115 | Bit-identical |
| `say_no` | 9 | Bit-identical |
| Atmospheric | 1,134 | Bit-identical |
| R7 | 102 | Bit-identical |
| Transports | 1,008 | Bit-identical |
| Every seed's T30 array | – | Equal |
| TCR | 868 | 164 differ, by at most 4.4 × 10⁻¹⁶ s. Those are D's reference (the plan's now) and D's deviation |

- D's reference differs from the run's own analytic time by at most 2.6 × 10⁻¹⁶ relative (2 ulp) over
  128 bands. *(Round 2, finding R3: this said 2.2 × 10⁻¹⁶.)* The volume enclosed by the scene equals the `.mbin`'s to rounding. Every D verdict is the
  same, and every `run_analytic_eyring_s` is `report.json`'s `analytic_eyring_s`.
- The other differences are keys the M8a report does not have: step 1's `project_sha256` and
  `band_problems`, and this fix's new keys.

**B. Without the seal.**
- 433 of 433 runs are refused `bed_run_unbound`. The bed gives `pass false`, E2 false.
- A seal that lacks one run's entry: that run is refused `bed_run_unbound`, and its cell is NotJudged.

**C. The judge's tampered copies, and more made here** (under `C:\tmp\nm-judge-tamper\cases\`). Every
tamper is refused. With a forged record, where the tampered folder's own hashes are put in its seal
entry, the next layer refuses it.

| Tamper | Before (VERIFY-adversarial) | Real seal | Forged seal |
|---|---|---|---|
| swap-seed: s3 with s4's 20 outputs | `pass true` | `bed_run_files_changed`, 20 files named. Cell NotJudged, `pass false` | Bound and accepted by `check_planned`, then seeds 3 and 4 each refused `bed_seed_outputs_identical` ("all of them"). `pass false` |
| swap-all-to-s1: s2 to s10 with s1's outputs | `pass true` | 9 of 9 `bed_run_files_changed`. `pass false` | 10 of 10 seeds refused `bed_seed_outputs_identical`. `pass false` |
| tcr-edited-sha: α 0.2's TCR run filed as α 0.4's, sha256 edited | `pass true` | `bed_run_files_changed`, 46 differences: the seal names another run folder, whose files are gone, and the folder found is not in the seal | `bed_run_not_planned`: `project.simpa` on disk is `5bf9fc96…`, the plan's `f6eb65d7…`; `config.xml` `58281202…`, the plan's `53dcd1bf…`. TCR NotJudged, `pass false` |
| The same α 0.2 outputs judged as α 0.4 by D alone | – | – | D Fail in all 6 bands: +128.9 % against the α 0.4 plan's 0.203675 s. Against the run's own analytic time, it would have been −7.75 × 10⁻⁸ |
| s3 with s4's `project.simpa` (made here) | – | `bed_run_files_changed`: `project.simpa` | `bed_run_not_planned`: `project.simpa` on disk. `pass false` |
| s3 with one byte of R000's `Sound level.recp` flipped (made here) | – | `bed_run_files_changed`, the file named. `pass false` | – |

**D. Fresh runs by this build** (`run::run_one`, under `C:\tmp\nm-judge-tamper\fresh\`).

| Run | `run.json` outputs | Read | Tampered copy | Result |
|---|---|---|---|---|
| TCR 5x4x3-a0.2-tcr-air-off | 21 files | Bound by `run.json`, accepted | One byte of `Main results.gabe` flipped | `bed_run_files_changed` |
| TCR 5x4x3-a0.2-tcr-air-off | – | – | `run.json`'s `outputs` removed | `bed_run_unbound` |
| N5 | 37 files | Bound by `run.json`, accepted | `project.simpa` replaced by its cell's seed-10 project | `bed_run_not_planned`, on `project.simpa` on disk. `run.json` does not bind the project file; the plan does |

## The product CLI (`cli.txt`)

- **The run.** It ran from 05:22:45 to 05:57:29:

  ```
  simpa bed beds/m8a.json --out C:/tmp/nm-judge-tamper/cli --from C:/tmp/nm-m8a-bed/20260929T093134Z
    --seal beds/m8a-20260929T093134Z/outputs-seal.json --upstream B:/repos/I-Simpa-upstream --jobs 8 --json
  ```
  - It read 433 runs in 309 s and re-traced the 68 transports in 1,774 s.
  - Result: **`bed PASS`**, gated cells 32 Pass, 0 failures, exit 0.
  - `meta.seal` names the seal, and `meta.seal_sha256` is `762f3fba…c15e09d`.
- **Its `report.json` against the M8a bed's** (step 1's `rejudge-compare.py`, whose Python reader
  rounds correctly):
  - The cell and TCR verdicts are equal, and N5 and N6 are `as_required`.
  - Every numeric leaf is bit-identical, the re-traced transports included, except these:
    - `tcr.d`: at most 4.4 × 10⁻¹⁶ s, D's reference being the plan's now;
    - `tcr.sabine_against_analytic`: at most 2.2 × 10⁻¹⁶.
  - There are 0 non-numeric differences, and no key is missing.
  - The new keys are this fix's (`bound_by` `seal` × 433, `on_disk`, `outputs_sha256`,
    `run_analytic_eyring_s`) and step 1's (`project_sha256`, `band_problems`).
- **The new report** validates against the schema (`bed_m8a report_validates_against_its_schema`).
- **The binary.** It was built from this tree at 05:22. The later edits before the code commit are
  rustfmt layout, two clippy fixes (a type alias, `contains`), the wording of the
  `bed_run_files_changed` message, and doc comments. `harness-out.txt` and `failing-first.txt` were
  made on the committed code.
- **A bad `--seal` is refused with exit 2, before anything is written**, in each of these cases:
  - `--seal` without `--from`;
  - a seal of another bed folder (`the seal is of the bed 20260929T093134Z, not of …`);
  - a seal whose totals do not add up;
  - a seal file that is not there.

## Checks run

On this tree: `CARGO_TARGET_DIR=C:/tmp/nm-target`, `CARGO_BUILD_JOBS=16`, `CARGO_INCREMENTAL=0`,
`SIMPA_SOLVERS_DIR=C:/tmp/nm-m8a-solvers`, `SIMPA_UPSTREAM=B:/repos/I-Simpa-upstream`.

| Check | Result |
|---|---|
| `cargo test -p simpa-core --lib` | 221 passed, 1 ignored, on a quiet machine, on the committed code. With `bed::` alone: 40 passed, 1 ignored. Without `SIMPA_UPSTREAM`, `validate::directivity`'s upstream test cannot run. `process::winproc`'s `dropping_the_tree_kills_the_child`, which allows a killed child 2 s to exit, failed twice while every core was busy (parallel builds; the CLI's transport phase), and passed each time the machine was quiet. `src/process` is not touched (`git diff` is empty) |
| `cargo test -p simpa-core --test params_reference --test reason_codes_docs --test results_load --test run_contract_docs --test run_manager --test run_manifest --test validate_contract_docs` | 3, 2, 14, 4, 14, 10 and 3 passed |
| `cargo test -p simpa` (every test that runs without a bed), with `SIMPA_TETGEN160=C:/tmp/nm-m10-solvers/build/src/tetgen/Release/tetgen.exe` | All passed. The binary's own tests passed 7 (`a_seal_is_taken_only_with_from` among them). By target: `cli_mesh` 10, `cli_results` 23, `cli_run` 13, `parity_tutorials` 7, `run_folder_fixtures` 3. `bed_m8a`, `m8_evidence` and `noise_calibration` are ignored by design. `parity_tutorials`' `tutorial_3` needs upstream's TetGen 1.6.0 build, and without `SIMPA_TETGEN160` it failed on the missing file. The test holds the file given to the manifest's reference, and that one passed |
| `bed_m8a report_validates_against_its_schema`, `SIMPA_BED_REPORT=beds/m8a-20260929T093134Z/report.json` | ok: the M8a report validates against the new schema and reads back, `pass true`, 40 cells, 0 failures. The CLI's new report validates too (above) |
| `bed_m8a` N8 through the seal | D Fail in 104 of 104 gated bands at +1.230 %, as the original gate |
| `bed_m8a` N2 and N3 through the seal (release) | A Fail in 8 of 8 α 0.4 gated cells each: N2 −11.07 % to −11.37 %, N3 +10.60 % to +10.97 % |
| `cargo clippy -p simpa-core -p simpa --all-targets -- -D warnings` | clean |
| `cargo fmt -p simpa-core -p simpa --check` | clean |

## What this does not show

- **The full gate.**
  - `tools/gates/m8a.ps1 -From` was not run. It now finds the committed seal by the bed's folder name
    and passes it to `simpa bed` and the say-NO tests.
  - N4 and N7 were not run. They re-trace the transports, and their faults act on the transport, not
    on what is read.
  - `simpa bed --from --seal` was run on its own (above).
- **A forged `run.json` for a new bed.**
  - A new bed's record is its runs' `run.json`, which is not signed. A hand-forged `run.json` whose
    `outputs` match forged files passes the binding.
  - What stands after it: the plan binding and D against the plan. *(Round 2, finding A3: this also
    named the seed rule and, for SPPS, E3 and C. They do not stand. A copied seed is real data of its
    cell, so E3, A, B and C pass it, and one unread byte per file defeats the byte-level seed rule
    (`VERIFY-adversarial-1.md`, U1 and U2).)*
  - A committed seal closes this for a bed used as evidence. Sealing each bed a gate judges is backlog
    row 44. *(Round 2 closes it for every bed: no run is bound by its `run.json` alone.)*
- **`simpa results` and the app.**
  - `results::load` does not check `run.json`'s `outputs`. It still verifies the inputs and judges the
    outputs again by the verdict's signals.
  - M11 shows no value, and the bed, the one reader of numbers today, checks them. This is backlog
    row 43.
- **TetGen was not run again.** The `.mbin` is bound to its record, to `mesh.json`'s stamp of the
  plan, and to the plan's scene by the reused-mesh check. It was not remade.
- **Between the binding and the reading.** The files are hashed before they are read. A file swapped in
  the moments between the two is not seen. This is a check against files changed at rest.

## Scratch left in place (nothing deleted)

- `C:\tmp\nm-judge-tamper\`, 107 MB and 721 files:
  - `cases\` (the tampered copies);
  - `fresh\` (the two fresh runs);
  - `cli\` (the CLI's report folder, 93 MB);
  - `cli-checks\`;
  - `harness\` (its binary is `C:\tmp\nm-target\release\nm-judge-tamper.exe`);
  - the scripts and logs.
- `B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b-tamper\target\parity-bed\`: 64 files, 18 MB,
  gitignored. The first `parity_tutorials` run, without `SIMPA_TETGEN160`, kept its `tutorial_3`
  scratch there, as that test does on a failure.
- Under `%TEMP%`: 9 `simpa-bed-bind-*` folders, 8 small files each. They were left by the `bind`
  tests' failing-first reverts; a test removes its folder only when it passes.
- Free space on C: at the end is 19,886 MB. No new target folder was made: every build used
  `C:/tmp/nm-target`.
