# M8b tamper: the two tamper-only holes of the M8a gate, closed

Worktree `m8b-tamper`, branch `m8b-tamper`, cut from `rebuild` at `5d46aa7` (M8a, M11 and M8b step 1).
Burhan, 04:29: "dont take any chances". So nothing here is closed on a guess, and every claim has a
test or a receipt. The M8a bed `C:\tmp\nm-m8a-bed\20260929T093134Z` was only read, never written, and
the 6-hour bed was not rerun. Nothing was deleted.

Two rounds:
- **Round 2** (code `8a26d99` to `fbd346d`) answers adversarial check 1 and re-judge 1 of round 1. It
  comes first below.
- **Round 1** (code `239c6f5`) closed the two holes. Its record follows round 2, with round 2's
  corrections marked in place.

## Round 2: adversarial check 1 and re-judge 1, answered

Round 1's code is `239c6f5`. It was checked twice, and neither check came back clean:
- `VERIFY-adversarial-1.md` (`a3280e9`): **`ok` false**. With no seal, a forged `run.json` brought back
  both hole-41 cases, and the M8a bed was exposed the same way through a renamed folder or `--from`
  without `--seal`.
- `REJUDGE-1.md` (`4d79c7d`): the real bed re-judged 23 of 23, `pass true`, but the check did not
  refute that bypass, and it found three smaller faults in this record and in `report.meta`.

Round 2's code is six commits, each one finding, each test failing with its fix reverted alone
(`failing-first-2.txt`):

| Commit | What it fixes |
|---|---|
| `8a26d99` | Adversarial finding 2: a run a seal does not name is refused |
| `0069cd2` | Adversarial finding 1: no run is judged on the word of its own `run.json` |
| `277d785` | Adversarial finding 1, its suggested defence in depth: two seeds that read the same T30 are refused |
| `208a916` | Adversarial finding 4: the seal is found under any spelling of the bed folder's name |
| `55c1a93` | Re-judge finding: `meta.git_commit` is the tree's when the bed starts |
| `fbd346d` | New, found by this round's own receipt: round 1's byte-level seed rule refused a valid bed |

This record and its receipts are the commits after `fbd346d`. Scope, as for adversarial check 1: an
attacker who can edit the bed folder is in scope. One who can also change the committed seal in git is
not, and the gate now refuses a seal that git does not track unchanged from `HEAD` (G4 below).

**Verdict: every finding in scope is fixed, each with a failing-first test, and the real M8a bed still
passes.**
- **No run is judged on its own `run.json`.** A run is bound only by a record from outside the folder
  being judged:
  - a run this process made, by what it made, held in memory;
  - a run of an earlier bed, by its seal.
- **U1 and U2 are refused on every path.**
  - With `--from`: `simpa bed` refuses to start without `--seal` (F4), and the seal refuses the tampered
    files (F5).
  - On a fresh run: the forged `run.json` is not what this process wrote (the test
    `a_run_this_process_made_is_bound_to_what_it_made_not_to_its_run_json`).
  - Should either get past the binding, the seeds read the same T30 and are refused.
- **The route to the M8a bed is closed.**
  - A renamed folder: `m8a.ps1` refuses to start (G1), and `simpa bed` refuses the seal for it.
  - `--from` without `--seal`: refused (F4).
- **The real bed.** Being re-judged by the gate as this is committed; its results are the next commit.

### The findings

| # | Finding | Verdict | What was done |
|---|---|---|---|
| A1 | major: with no seal, `run.json` alone binds a run; a forged one plus one padding byte per file brings back U1 and U2, and reaches the M8a bed by a rename or `--from` without `--seal` | Right, in scope | `0069cd2`, below; defence in depth `277d785` |
| A2 | minor: a seal given, a run it does not name is bound by `run.json` alone (gate C's extension seeds) | Right, in scope | `8a26d99`: refused `bed_run_unbound` |
| A3 | minor: `FIXES.md` and backlog row 44 say the seed rule, E3 and C stand after a forged `run.json` | Right | Corrected in round 1's text below, marked "Round 2, finding A3", and in row 44. A copied seed is real data: E3, A, B and C pass it. The seed rule is a check against a solver that ignores its seed, and since `277d785` it also compares what is read |
| A4 | minor: `Seal::for_bed` compares the folder name case-sensitively | Right, in scope | `208a916`: the name the file system stores. The gate had the same fault one step on (git's pathspecs), fixed in the same commit |
| R1 | major: the re-judge does not refute A1's bypass | Right | Closed by A1's fix. This round's gate also checks `report.meta.seal_sha256` against the committed seal |
| R2 | minor: D is not bit-identical to `report.json` (at most 4.441 × 10⁻¹⁶ s) | Right, by design: D's reference is the plan's (hole 42). Round 1's record says so | No change |
| R3 | minor: the bound "2.2 × 10⁻¹⁶ relative" is exceeded | Right. Measured again here from the round-1 re-judge's report: at most 2.584 × 10⁻¹⁶ (`20x8x4-a0.2-tcr-air-off`, 4 kHz), 2.416 × 10⁻¹⁶ over the gated runs, 13 of 128 bands above 2.2 × 10⁻¹⁶, never more than 2 ulp | The text is corrected to "at most 2.6 × 10⁻¹⁶ relative, 2 ulp". No verdict depends on it |
| R4 | minor: `meta.git_commit` names the tree when `simpa bed` ends, not the one it was built from | Right, in scope: the report is the gate's evidence | `55c1a93` |
| N1 | new: round 1's seed rule refused a valid bed (`fresh-seal-receipt.txt`, first attempt) | Found here | `fbd346d`, below |
| N2 | new: `process::winproc`'s `dropping_the_tree_kills_the_child` failed once | Found here, not in the bed's code | Backlog row 45 |

Out of scope, and why:
- **An attacker who edits the committed seal in git** (as set for adversarial check 1).
  - A seal edited in the work tree and not committed is refused by the gate (G4).
  - A seal edited and committed is a change to the repository, seen in its history. The gate reads what
    `HEAD` holds.
- **A change to the files between the binding and the reading**, as in round 1. This is a check of files
  at rest.
- **A change during a fresh run, before the run manager hashes the run folder.** That is the run's own
  time, not a bed at rest. From the moment the run manager hashes the folder, what it made is held in
  memory, and the bed reads nothing that is not that.

Suggested, and not done:
- **B refusing a spread of exactly 0.** Ten seeds that read the same T30 are refused before B is
  judged (`277d785`), so a copy never reaches B. A spread of exactly 0 from seeds that read differently
  would be a coincidence of cell means, not a sign of a copy.
- **A stricter gabe reader** (the unused header lengths, the padding, what follows the last column).
  - It would not close a forgery: the numbers can be changed too.
  - What upstream writes in those bytes has not been characterized, so a stricter reader could refuse
    valid files.
  - A copied or edited file is refused by its bytes, against a record from outside the bed folder.

### What changed

**1. A run is bound only by a record from outside the folder being judged** (`0069cd2`, finding A1).
- `bed/bind.rs:264` `Records { made, sealed, run_json }`. `bind` (`:282`) refuses a run with neither
  `made` nor `sealed`, `bed_run_unbound` (`:297`), whatever its `run.json` records. `run.json`'s
  `outputs` are still held when a run records them, and bind nothing alone.
- **A run this process made.**
  - `bed/run.rs:458` `made_record(p, report)` builds, from memory, the run's folder in a seal's form:
    - `project.simpa`, as the bed saved it (`schema::to_json`);
    - the run folder's files, as the run manager hashed them when the run ended
      (`RunReport::manifest.outputs`);
    - `run.json`, as the run manager wrote it (`RunManifest::to_json`).
  - `run_one` (`:395`) is `launch` (`:402`) then `read_fresh` (`:443`). `read_fresh` binds to that
    record. The folder's own `run.json` is held to it, never taken as what binds the run.
  - `RunInfo::made` (`bed/read.rs:70`, `serde(skip)`) keeps the record for the bed's seal.
- **A run of an earlier bed.**
  - `read_in` (`bed/run.rs:335`) and `read_existing` (`:516`) take a `&Seal`, not an `Option`. There is
    no read of an earlier bed without a seal.
  - `simpa bed --from` needs `--seal`, exit 2 without it (`crates/simpa/src/bed_cmd.rs:172`).
  - The seal is refused when it lies inside the bed folder it would hold (`bed/bind.rs:210`
    `seal_outside`, both paths resolved by the file system).
  - The say-NO harness requires `SIMPA_BED_SEAL` and checks it the same way
    (`crates/simpa/tests/bed_m8a.rs:50`).
- **Each bed `simpa bed` runs is sealed by it** (`bed_cmd.rs:459`).
  - The seal is written as `outputs-seal.json`, from the runs' `made` records, and its sha256 is
    printed. It names the report and summary it was written with.
  - A later `--from` takes it only from a copy outside the bed folder, which is committed beside the
    bed's `report.json`.
- **The gate** (`tools/gates/m8a.ps1`).
  - **`-From`** refuses to start without a seal (`:99`).
  - It takes only a seal that git tracks (`:106`) and that is unchanged from `HEAD` (`:108`).
  - It checks that `simpa bed` read the runs against that seal: `report.meta.seal_sha256` (`:248`).
  - **`-BedRoot`** copies the seal `simpa bed` wrote out of the bed folder, and checks the copy against
    the printed sha256 (`:256`). The say-NO tests and N1 re-read the bed against that copy.
  - A new check runs `tests/bed_binding.rs` (`:374`).

**2. A seal holds its whole bed** (`8a26d99`, finding A2). `read_in` refuses a run its seal does not
name, `bed_run_unbound`, before anything is read (`bed/run.rs:383` `unsealed`). Since `0069cd2`, `bind`
would refuse it too.

**3. Two seeds that read the same T30 are refused** (`277d785`, defence in depth for A1).
- In `refuse_shared_outputs` (`bed/run.rs:855`), every seed is refused `bed_seed_outputs_identical`, the
  receiver named, when it reads the same T30 as another seed at a receiver in every band. Values,
  `mc_sd` and sources are compared by their bits, and at least one band must have a value.
- Over the M8a report's 1,845 pairs of seeds (40 cells and the atmospheric validation, 18 to 27
  receiver-bands each), no two seeds read the same value in any single receiver-band
  (`t30-identity-probe.py`, `t30-identity-probe-out.txt`). So this cannot refuse the M8a bed.
- It is no defence against a copy whose numbers were changed too. The binding is.

**4. The seal is found under any spelling of the bed folder's name** (`208a916`, finding A4).
- `Seal::for_bed` (`bed/bind.rs:159`) compares the name the file system stores for the folder the path
  resolves to (`std::fs::canonicalize`).
- Refused still:
  - another folder;
  - a junction named like the bed that resolves elsewhere;
  - a path that is not there.
- `m8a.ps1` resolves the seal's path to the stored names before it asks git (`:76` `StoredPath`). Git's
  pathspecs match case exactly, and `0069cd2`'s gate called the committed seal "not tracked" under a
  lower-case `-From` (G7).

**5. `meta.git_commit` is the tree's when the bed starts** (`55c1a93`, finding R4).
- `bed_cmd.rs:217` reads git before any run is made or read.
- The state at the end goes to `meta.git_commit_at_end` and `git_dirty_at_end` (`bed/report.rs:42`,
  `serde(default)`, so the M8a report still reads). A commit that lands during the run shows there.

**6. The byte-level seed rule holds only the receivers' files** (`fbd346d`, finding N1).
- **What happened.** This round's fresh-bed receipt ran a small bed of 31,000 particles per source.
  Round 1's rule refused seeds 1 and 2 of its random α 0.4 cell, which shared `SPPS particle
  statistics.gabe` byte for byte:
  - a count file, which coincides when few particles run;
  - seed 3's differs from seed 1's in 2 bytes of 2,505;
  - every receiver file of the three seeds is its own.
- **The change** (`bed/run.rs:815` `is_receiver_file`). Only a point receiver's file (under
  `Punctual receivers/`, what T30 is read from) shared by two seeds refuses them.
- **What still refuses.** Another file shared refuses them only as part of the whole set, which still
  counts. So these are all still refused:
  - a copied receiver;
  - a solver that ignores its seed;
  - a wholesale copy.
- No judged number depends on the files no longer held alone. The M8a bed shared none of its 8,490
  output files, so no refusal of it changes.
- **The tests.** A real-run test (`tests/bed_binding.rs:246`) reproduces the coincidence with this
  build's solvers and fails with the old rule.

### Tests, each failing-first

`failing-first-2.txt` holds each revert as a diff, the command, the output with the revert, the
byte-for-byte restore, and the output after it. `failing_first_2.py` is the driver. Every case was run
again on the final code at `fbd346d`. The first case, `f2-commitA`, is kept from commit
`8a26d99`'s own tree, where it was the only guard.

| Case (`failing-first-2.txt`) | Test | Revert (only the fix) | With the revert | Restored |
|---|---|---|---|---|
| `f1-bind` | `bind.rs:482` `a_run_bound_only_by_its_own_run_json_is_refused`, with U1's shape: every file is what its forged `run.json` says | `bind` takes `run.json`'s outputs alone | FAILED: `unwrap_err` on `Ok`, `Bound { by: ["run.json"], … }` | ok |
| `f1-fresh` | `tests/bed_binding.rs:170` `a_run_this_process_made_is_bound_to_what_it_made_not_to_its_run_json`. A real TCR run, then one byte of `Main results.gabe` changed after it ended, and `run.json` rewritten by the crate's own writer to match | `read_fresh` builds its record from the `run.json` in the folder | FAILED: the forged run accepted | ok |
| `f1-cli` | `bed_cmd.rs:643` `from_without_a_seal_is_refused` | `--from` without `--seal` taken | FAILED: `parse` accepted it | ok |
| `f1-outside` | `bind.rs:713` `a_seal_inside_its_bed_folder_is_refused`; `tests/bed_binding.rs:204` `the_seal_of_a_bed_this_process_ran_holds_its_runs_for_a_re_read` | `seal_outside` takes any path | FAILED, both: a seal inside the bed folder taken | ok |
| `f2-commitA` | `tests/bed_binding.rs` `a_run_its_seal_does_not_name_is_refused`, on `8a26d99`'s code (then uncommitted on `4d79c7d`) | the seal's missing entry passed on as none | FAILED: accepted, `bound_by: "run.json"` | ok |
| `f2-final-a` | the same test, final code | `read_in`'s check alone | FAILED on the message. The run is still refused, `bed_run_unbound`, by `bind`'s rule: two layers now | ok |
| `f2-final-b` | the same | `read_in`'s check and `bind`'s rule | FAILED: accepted on `run.json` | ok |
| `d1-reads` | `bed.rs:849` `a_seed_reading_another_seeds_t30_does_not_pass_the_bed`; `bed.rs:876` `ten_seeds_reading_one_seeds_t30_do_not_pass_the_bed` | the T30 comparison skipped | FAILED, both: the bed passes on copied reads | ok |
| `f4-case` | `bind.rs:802` `a_seal_is_for_its_bed_folder_by_the_name_the_file_system_stores` | `for_bed` compares the name as given | FAILED: the lower-case path refused, `the seal is of the bed 20260929T093134Z, not of …20260929t093134z` | ok |
| `f8-git` | `bed_cmd.rs:620` `the_work_tree_is_recorded_as_it_was_when_the_bed_started` | `git_commit` taken from the end | FAILED: `a3280e9` where `dd5c683` is wanted | ok |
| `r2-receivers` | `bed.rs:748` `only_a_receivers_file_shared_by_two_seeds_refuses_them`; `tests/bed_binding.rs:246` `seeds_whose_particle_statistics_coincide_are_not_refused` (three real runs) | every shared output file refuses | FAILED, both: seeds 1 and 2 refused over `SPPS particle statistics.gabe` | ok |

**Also new, not reverts.**
- `bind.rs` `a_run_bound_to_a_seal_is_refused_when_any_file_is_not_its_records`, now through `Records`.
- `bind.rs` `a_run_with_no_output_hashes_and_no_seal_entry_is_refused`, the same.
- `tests/bed_binding.rs:204` `the_seal_of_a_bed_this_process_ran_holds_its_runs_for_a_re_read`. On a
  real TCR run, it checks five things:
  - the seal made from `made` equals the folder hashed from disk;
  - it re-reads the run;
  - a copy of it inside the bed folder is refused;
  - a renamed copy of the bed is refused by `for_bed`;
  - U1's tamper in that copy, and in place, is refused `bed_run_files_changed`.
- `bind.rs:802`: a junction named like the bed that resolves to another folder is refused.

### The gate, and a bed run fresh, end to end

**The gate** (`gate-seal-receipt.txt`, `gate_seal_receipt.py`). Each case ran `m8a.ps1`. The cases
marked "stopped" were killed once the line named had printed, before any build or read. The bed was
never read.

| Case | Gate | Result |
|---|---|---|
| G1 | this round's | `-From` a renamed bed folder: refused, "no committed seal", 0.3 s |
| G2 | this round's | `-Seal` a copy outside the repository: refused |
| G3 | this round's | `-Seal` an untracked copy inside the repository: refused, "not tracked by git" |
| G4 | this round's | the committed seal changed by one byte in the work tree: refused, "differs from HEAD". Restored byte for byte |
| G5 | this round's | `-From` the M8a bed: the committed seal taken, sha256 `762f3fba…c15e09d`, stopped |
| G6 | this round's | `-From` the M8a bed spelled `…\20260929t093134z`: the same seal taken, under the name git tracks, stopped |
| G0 | `4d79c7d`'s, before round 2 | `-From` the renamed folder: it printed `seal: (none: …)` and went on to E1, stopped |
| G7 | `0069cd2`'s | the lower-case spelling: refused, "not tracked by git". Fixed by `208a916` |

**A bed run fresh, end to end** (`fresh-seal-receipt.txt`, `fresh_seal_receipt.py`).
- **The bed.** A small exploratory bed cut from `beds/m8a.json` (`fresh-seal-bed.json`):
  - the 5x4x3 room;
  - α 0.4 random and α 0.2 energetic, at 31,000 particles per source;
  - their TCR run;
  - seeds 1 to 3;
  - N5, and N6 at 31,000 particles.
  - It never passes (E7), which does not matter here.
- **Its first attempt found N1.**

| Case | Result |
|---|---|
| F1 run fresh | 9 runs, each bound by `run.json and this process`. The seal is written: 9 runs, 335 files. The printed sha256 is the file's, and it names the `report.json` written. Re-hashed by Python, the seal has 0 differences from the folder |
| F2 re-read with a copy of the seal outside the bed | 9 of 9 bound by `run.json and seal`, 0 errors. `meta.seal_sha256` is the copy's. Every verdict equal, every seed's T30 equal, the cells equal as a whole, apart from each run's record |
| F3 `--seal` the seal inside the bed | exit 2, "is inside the bed folder", nothing written |
| F4 `--from` with no `--seal` | exit 2, "--from needs --seal" |
| F5 a copy under the same name: U1 in seed 2, U2 in seed 3 (seed 1's 20 outputs), records forged | Seeds 2 and 3 refused `bed_run_files_changed`: 2 differences and 21 differences. The cell is not judged, `pass false` |

**The gate's `-BedRoot` path on that bed** (`gate-fresh-bed.txt`).
- **The seal handoff held.**
  - "simpa bed sealed the bed it ran": PASS. The copy is in the gate's work folder, with the printed
    sha256.
  - N1 ran with `--from` and `--seal` set to the copy: PASS.
  - N8 re-read the TCR run through the copy: D Fail in every band, +1.230 %.
  - N4 re-read the energetic cell through the copy and judged it: C Fail, d +14.55 %.
- **8 of 25 checks failed, each for a reason of that bed, as expected:**
  - E7, `pass`, and `simpa bed`'s exit;
  - N2, N3 and N4: the random cell is not judged at 31,000 particles (E6, `range_not_reached`);
  - N7: the bed has no air-on cell;
  - the bed's unit tests: `the_atmospheric_validation_imports_as_the_spec_saw_it` reads upstream, and
    the receipt pointed `-Upstream` at an empty folder so that the atmospheric validation would not
    run.

### On the real bed

*The real bed is being re-judged by the gate as this is committed (`rejudge-2-wrapper.ps1`, started 08:47:04 on `5907044`). Its results are the next commit.*

### Checks run

On this tree: `CARGO_TARGET_DIR=C:/tmp/nm-target`, `CARGO_BUILD_JOBS=16`, `CARGO_INCREMENTAL=0`,
`SIMPA_SOLVERS_DIR=C:/tmp/nm-m8a-solvers`, `SIMPA_UPSTREAM=B:/repos/I-Simpa-upstream`.

| Check | Result |
|---|---|
| `cargo test -p simpa-core --lib` | 226 passed, 1 ignored, in 3 of 4 runs. The first run, just after a build, failed `process::winproc`'s `dropping_the_tree_kills_the_child` (the child not gone 2 s after its job was dropped). That test passed 5 of 5 alone and in the 3 full runs after. `src/process` is untouched by M8b. Round 1 saw it fail twice under load. It is a finding, backlog row 45, not a tolerance |
| `cargo test -p simpa-core --lib bed::` | 46 passed, 1 ignored |
| `cargo test -p simpa-core --test bed_binding` | 4 passed, real runs of this build |
| `cargo test -p simpa-core --test params_reference --test reason_codes_docs --test results_load --test run_contract_docs --test run_manager --test run_manifest --test validate_contract_docs` | 3, 2, 14, 4, 14, 10 and 3 passed |
| `cargo test -p simpa`, with `SIMPA_TETGEN160` as in round 1 | All passed. The binary's own tests: 9. By target: `cli_mesh` 10, `cli_results` 23, `cli_run` 13, `parity_tutorials` 7, `run_folder_fixtures` 3. `bed_m8a`, `m8_evidence` and `noise_calibration` are ignored by design |
| `cargo clippy -p simpa-core -p simpa --all-targets -- -D warnings` | clean |
| `cargo fmt -p simpa-core -p simpa --check` | clean |
| The gate on the real bed | *running, see "On the real bed"* |

### What this round does not show

- **The gate's `-BedRoot` on M8a's matrix.** That is the 14-hour bed. Its new steps ran on the small bed
  above:
  - the seal written from memory;
  - the copy out of the bed folder;
  - the say-NO re-reads through it.
- **A file symbolic link** (as in round 1). It still cannot be made without the privilege.
  - `hash_tree` refuses it by the branch that refused the junctions: a link is neither a file nor a
    folder.
  - `for_bed` resolves a junction to its target, and a junction is refused by the test at
    `bind.rs:802`.
- **A forgery with its numbers changed.** A copied run whose T30 values were changed too would pass the
  T30 rule. It does not pass the binding, which is what closes it.
- **`simpa results` and the app** hold a run to its inputs only, as in round 1: backlog row 43, open.
- **A commit of a new bed's seal.** It is a hand step: the gate prints where the seal it made is, and
  `-From` refuses to re-read a bed whose seal git does not track unchanged.

### Scratch left in place (round 2; nothing deleted)

*listed when the re-judge ends*

---

## Round 1 (code `239c6f5`)

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

### Hole 41: a run's outputs were bound to nothing, and seeds could be copies

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

### Hole 42: run.json's word was the only link between a run and its plan

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

### The seal (`beds/m8a-20260929T093134Z/outputs-seal.json`)

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

### Tests, each failing-first

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

### On real runs (`harness-out.txt`)

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

### The product CLI (`cli.txt`)

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

### Checks run

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

### What this does not show

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

### Scratch left in place (nothing deleted)

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
