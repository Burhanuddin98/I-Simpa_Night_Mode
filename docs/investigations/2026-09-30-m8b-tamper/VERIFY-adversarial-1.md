# M8b tamper: adversarial check 1 of the two tamper fixes

Checked 2026-09-30, 06:11 to 06:45, on worktree `m8b-tamper` at `dd5c683` (code `239c6f5`), against
`FIXES.md` beside this file. Burhan, 04:29: "dont take any chances".
- No product code was edited.
- The M8a bed `C:\tmp\nm-m8a-bed\20260929T093134Z` was only read. Its fingerprint and a full re-hash
  against the seal were identical before (04:22:41Z) and after (04:39:25Z) the harness.
- Everything written is under `C:\tmp\nm-judge-tamper\adv1\`, behind a guard that refuses any other
  path. Nothing was deleted.

Scope, as set for this check: an attacker who can edit only the bed folder is in scope. One who can
also edit the committed seal is not.

**Verdict: `ok` is false.**
- **With its committed seal, the M8a bed holds.**
  - All 7 earlier tampered copies are refused.
  - All 28 new in-scope tampers of the sealed bed are refused, among them TCR, the atmospheric
    validation, N5 and N6.
  - Nothing valid is refused: 433 of 433 runs are accepted and re-judge to `report.json`'s verdicts.
- **Without a seal, both hole-41 cases pass again** (U1 and U2 below). `run.json` is then the only
  record, and the bed folder that holds it is the one being edited. Setting one unchecked padding byte
  in each copied output file defeats the byte-identity seed rule. E3, A, B and C pass because the
  copied data are real runs of the same cell.
  - Seed 3 carrying seed 4's outputs: `pass true`.
  - Seeds 2 to 10 carrying seed 1's outputs: `pass true`, cell Pass, B spread 0.0.
- **This reaches the M8a bed itself.** Rename its folder, or call `simpa bed --from` without
  `--seal`, and no seal is used: `m8a.ps1` looks for one by the folder's name and carries on without it.
- Backlog row 44 already records a forged `run.json` as open. It says the seed rule, E3 and C "stand
  after it". U1 and U2 show that they do not.

Receipts beside this file:

| File | What it is |
|---|---|
| `verify1-harness.rs` | A scratch crate, built at `C:\tmp\nm-judge-tamper\adv1\harness`, target `C:/tmp/nm-target`. It reads as `simpa bed --from` reads: `run::read_existing` or `read_in`, then `run::into_reads`, then `report::evaluate`. The transports come from the M8a `report.json`; nothing is re-traced |
| `verify1-harness-out.txt` | Its output. It ran in 497 s, the bed read 8 at once in 390 s |
| `verify1-bed-fingerprint.py`, `verify1-fingerprint.txt` | The bed's fingerprint, and every sealed file re-hashed by Python, independently of the Rust and of `tools/bed/seal_bed.py`, before and after the harness |

## 1. The earlier tamper cases, replayed

Each of the 7 tampered copies in `C:\tmp\nm-judge-m8b\tamper\` was copied fresh, and each copy was
checked byte for byte against the old one. The tamper was checked against the bed. For example,
`swap-seed`'s seed 3 outputs are seed 4's in the bed, and its `run.json` is seed 3's.

Each copy was read three ways:
- with the committed seal;
- without a seal;
- with a forged record, that is, `run.json`'s `outputs` written by this build's own serializer for the
  tampered files, and no seal.

| Case | Before (step 1) | With the seal | No seal | Forged record, no seal |
|---|---|---|---|---|
| `swap-seed`: s3 with s4's 20 outputs | `pass true` | `bed_run_files_changed`, 20 files. `pass false` | `bed_run_unbound` | Bound, then s3 and s4 each `bed_seed_outputs_identical` ("all of them"). `pass false` |
| `swap-all-to-s1`: s2 to s10 with s1's | `pass true`, cell Pass | 9 of 9 `bed_run_files_changed`. `pass false` | 9 `bed_run_unbound` | 10 of 10 `bed_seed_outputs_identical`. `pass false` |
| `edited-sha-other-cell`: α 0.1's runs as α 0.05's, sha256 edited | accepted, then failed at E3 and C | 10 of 10 `bed_run_files_changed` (another run folder) | 10 `bed_run_unbound` | 10 of 10 `bed_run_not_planned`: `project.simpa` and `config.xml` on disk |
| `edited-sha-other-seed`: s4's run as s3, sha256 edited | refused, `random_seed` | `bed_run_files_changed` | `bed_run_unbound` | `bed_run_not_planned`: `random_seed 4`, `project.simpa`, `config.xml` |
| `outputs-10ms`: s1 with the 10 ms run's outputs | not read (time step) | `bed_run_files_changed` | `bed_run_unbound` | not read: `results_file_invalid`, `.gap` time step 0.01 |
| `tcr-outputs-10ms` | accepted: its outputs are its own, byte for byte | **accepted, `pass true`**: every byte is the sealed run's (checked), so nothing was tampered | `bed_run_unbound` | accepted, `pass true`, for the same reason |
| `tcr-edited-sha`: α 0.2's TCR run as α 0.4's | `pass true`, TCR Pass | `bed_run_files_changed`, 46 differences | `bed_run_unbound` | `bed_run_not_planned`: `project.simpa` `5bf9fc96…` against `f6eb65d7…`, `config.xml` |

The other cases of step 1's check were run again on this build. All are refused:

| Case | Result |
|---|---|
| TCR band lists of `6x10x3-a0.2-tcr-air-on`: 8 kHz removed, only 125 Hz, relabelled to 16 kHz, 4 kHz twice, 125 Hz twice, the 4 kHz row relabelled 8 kHz, 8 kHz twice. Also the air-off run with 4 kHz relabelled 8 kHz | Each refused `bed_run_not_planned` as a read, and each D Fail after `into_reads`. `pass false` in all 8 |
| 12 SPPS settings changed one at a time on s3 (`trans_epsilon` 7 and 9.000000000001, particles 150,000 and +1, method 0, seed none and 0, three band edits, seed 4's sha256, step NaN) | 12 of 12 `bed_run_not_planned` |
| 8 real runs filed under another key: N5 as its cell's s10; the random cell's s1 as the energetic cell's; α 0.1 s3 as α 0.05 s3; SPPS s1 as TCR; a scattering cell's s1 as N6; s3 and s4 each way; α 0.2's TCR run as α 0.4's | Each folder `bed_run_files_changed`. Each read in memory under the other plan: `bed_run_not_planned`. s3 and s4 swapped: `pass false` |
| The smoke bed's 14 matching keys | With the seal: 14 `bed_run_files_changed`. Without: 14 `bed_run_unbound`. Forged records: 14 `bed_run_not_planned` (sha256 and particles, e.g. 300,000 against 31,000,000) |
| The two 10 ms runs by this build, under the 1 ms plan | With the seal `bed_run_files_changed`, without `bed_run_unbound`. Forged records: `bed_run_not_planned` (sha256 and `time_step_s`) |

## 2. Attempts to get around the new checks

### 2a. The sealed bed, edited in the bed folder only

Each case is a fresh copy of the runs it needs, read with the committed seal under the key named, then
judged in place of the real run. Seed 3 below is `5x4x3-a0.4-energetic-air-off` s3.

| Attempt | Result |
|---|---|
| s3 with s4's outputs, and `run.json`'s `outputs` forged to match | `bed_run_files_changed`, 21 differences (`run.json` 9,902 bytes against the sealed 3,600, and the 20 outputs). `pass false` |
| A file the bed never reads changed (`solver.stdout.txt`, one byte added) | `bed_run_files_changed`. It fails closed |
| Whole slots of s3 and s4 swapped (`project.simpa` and the run folder) | Both `bed_run_files_changed`. `pass false` |
| s4's run folder beside s3's `project.simpa` | `bed_run_files_changed` (another run folder, 77 differences) |
| α 0.1's s3 slot as α 0.05's s3 | `bed_run_files_changed` |
| TCR α 0.2's slot as α 0.4's | `bed_run_files_changed`, TCR NotJudged |
| One output's padding byte set. The file reads the same (see 2b) | `bed_run_files_changed`: same size, another sha256 |
| The same, with its record forged in `run.json` | `bed_run_files_changed`, 2 differences (`run.json` and the file) |
| Band files moved: `Intensity animation/125 Hz` and `250 Hz` swapped | `bed_run_files_changed`, both named |
| Receivers moved: R000 and R001's four files swapped | `bed_run_files_changed`, 8 differences |
| `Sound level.recp` truncated by one byte, or to nothing | `bed_run_files_changed` (31,604 or 0 bytes against 31,605) |
| s3's slot a junction to a copy of s4's slot | `bed_run_files_changed`: the files are hashed through the junction, and they are s4's |
| `solve/Punctual receivers/R000` a junction, to s4's or to an identical copy of s3's | `bed_run_files_changed: … R000: neither a file nor a folder`, both. Any link is refused, even one to identical files |
| `Sound level.recp` a hard link to a copy of s4's | `bed_run_files_changed` (content s4's) |
| An extra file in `solve/`, beside `project.simpa`, or in `mesh/` | `bed_run_files_changed`: "is not in the seal" |
| An empty receiver folder `R099`; an empty folder `S000` in R000 | Refused by `results::load`: the receiver folders are not `config.xml`'s labels; source folders with `output_recp_bysource 0` |
| A second run folder that sorts after the real one (a copy of s4's) | `find_run_folder` takes it, then `bed_run_files_changed` |
| TCR `5x4x3-a0.4-tcr-air-off`: a padding byte in `Main results.gabe`; α 0.2's four outputs | Both `bed_run_files_changed`, TCR NotJudged, `pass false` |
| Atmospheric validation: seed 2 carrying seed 1's outputs; the TCR run with one padding byte | Both `bed_run_files_changed`. The report's seed 2 carries the error. The validation is reported, not gated, so `pass` stays true, and no tampered number enters it |
| N5 carrying its cell's seed-10 outputs; N6 carrying `5x4x3-a0.2-energetic-air-off` s1's | Both `bed_run_files_changed`. `as_required` false, so `m8a.ps1`'s N5 or N6 check fails the gate |
| A seal entry for a file the folder does not have (the seal edited in memory; out of scope) | `bed_run_files_changed: … not-there.recp is gone` |
| The committed seal given for a renamed bed folder | Refused: `the seal is of the bed 20260929T093134Z, not of …` |

Accepted, with nothing tampered. Each was checked: T30 and `outputs_sha256` are the real s3's.
- s3's slot a junction to an identical copy of s3.
- `Sound level.recp` a hard link to an identical copy.
- Empty folders, which `hash_tree` does not list, in three places: `solve/zz-empty`, the slot, and
  `Intensity animation/16000 Hz`.

Not made:
- A file symbolic link, for lack of the privilege (`mklink`: "You do not have sufficient privilege").
- The code refuses it by the same branch as the junction above: on Windows a symbolic link's
  `file_type()` is neither a file nor a folder, and `hash_tree` returns an error for that
  (`run/manifest.rs`, `hash_tree`).
- No unit test covers links.

### 2b. No seal: `run.json` is the only record (in scope)

A bed read with `--from` and no `--seal` binds each run to its own `run.json`'s `outputs`
(`bed/bind.rs:221`, `bed/run.rs:354`). That is every bed but M8a's, and M8a's too when no seal is
used.

Two things make the seed rule no defence there:
- **The gabe reader skips bytes unchecked.** `formats/gabe.rs:211-212` reads the two header lengths
  and never uses them. `:181-185` skips the three padding bytes after the read-only flag unchecked.
  `:298` drops every byte after the last column.
- **So a copied output can be made byte-different and read the same.** Setting byte 17 does it. Every
  output of these runs is a gabe: the harness checked the header of each file it changed, all 20 of a
  cell run's and all 33 of an atmospheric run's, and TCR's outputs are four `.gabe` tables. The seed
  rule compares bytes (`bed/run.rs:735`).

The cases, cell `5x4x3-a0.4-energetic-air-off`, its ten seeds copied, each `run.json` given `outputs`
by this build's own `RunManifest::to_json`:

| Case | Reads | Judged |
|---|---|---|
| U0: each seed's true record (a control) | 10 of 10 accepted, bound by `run.json`. T30 and `outputs_sha256` equal to the sealed reads | `pass true`, cell Pass, C d +0.0008 % ± 0.0339 % |
| **U1: s3 carrying s4's 20 outputs, one padding byte set in each, record forged** (hole 41, first case) | s3 **accepted**. Its T30 is s4's | **`pass true`**, cell Pass (A, B, C Pass, d +0.0032 % ± 0.0349 %), no seed error |
| **U2: s2 to s10 carrying s1's outputs, padding byte = seed, records forged** (hole 41, second case) | 10 of 10 **accepted**. All ten T30 arrays are seed 1's | **`pass true`**, cell Pass on ten identical seeds: B spread **0.0**, C per-seed all −0.00037, d −0.0368 % ± 0.0272 % |
| U3: TCR α 0.4's run carrying α 0.2's outputs, record forged (hole 42) | Accepted by the binding and `check_planned`: its inputs are α 0.4's | **D Fail** in all 6 bands, +128.92 % against the plan's 0.203675 s. `pass false`. Hole 42 holds without the seal |
| U4: s3 carrying the smoke bed's s3 outputs (100,000 particles), record forged | Refused by `results::load`: `particle_total_short` | `pass false` |
| U5: atmospheric seed 2 carrying seed 1's outputs, padding byte set, records forged | 10 of 10 accepted | `pass true`. The reported 50 Hz mean moves from 0.996896 s to 0.997102 s. The validation is reported, not gated |
| U6: s4's genuine run filed as s3, its true record | `bed_run_not_planned` (`random_seed 4`, sha256, `project.simpa`, `config.xml`) | – |

**How this reaches the M8a bed.** Nothing is edited but the bed folder:
1. **The attacker edits a copy of the bed.** Tamper as in U1 or U2, and write the tampered files'
   hashes into every `run.json`'s `outputs`. U0 shows that a record written this way reads bit for bit
   as the sealed read does.
2. **The attacker gets the read made without the seal**, in either of two ways:
   - **Through the gate.** Give the folder another name. `m8a.ps1` looks only for
     `beds\m8a-<folder name>\outputs-seal.json` (`tools/gates/m8a.ps1:60-62`). When there is none, it
     prints `seal: (none: runs without output hashes in run.json are refused)` (`:146`) and goes on:
     - `simpa bed` runs without `--seal` (`crates/simpa/src/bed_cmd.rs:216-229`, `:454`);
     - the say-NO tests read with `SIMPA_BED_SEAL` empty, which is no seal
       (`crates/simpa/tests/bed_m8a.rs:48-54`).
   - **Through the CLI.** Keep the name and call `simpa bed --from` without `--seal`.
3. **What the gate sees.** Nothing else it checks sees the edit:
   - E1 again from the files: `run.json`'s `exe.sha256` is untouched;
   - the file count;
   - the schema;
   - N2, N3 and N8, which read the same forged records;
   - N5 and N6.

   The gate would print `M8a PASSED`. The only trace is `meta.seal` null in the report, which nothing
   checks.

This last step was not run end to end: it needs a full copy of the bed, which this check was told not
to make. The harness calls the same three functions the CLI does. The CLI's own re-traced transports
matched `report.json`'s bit for bit in `FIXES.md`'s run.

### 2c. Gate C's extension seeds (in scope, latent for M8a)

**The gap.** With a seal given, a run the seal does not name is still bound by its `run.json` alone:
`seal.and_then(|s| s.run(&p.key))` is `None`, and `bind` takes the `run.json` record. Nothing requires
every run of a sealed bed to be in the seal. An INCONCLUSIVE gated cell's seeds 11 to 20 are read from
the `--from` folder with the same seal (`bed_cmd.rs:322`), so they would be bound by their `run.json`
alone.

**The demonstration.**
- Seed 11 was built from s10's run: seed 11's `project.simpa`, s10's `config.xml` with `random_seed`
  11, which is exactly the plan's for seed 11 (workingdirectory aside), one padding byte in each
  output, and the record forged.
- Read under the planned extension key **with the committed seal**, it is accepted, `bound by run.json`.
- Through `into_reads` with seeds 1 to 10, it is kept.

**Why M8a cannot reach it.** The real bed's re-judge gives `needs_extension []`. Every first-ten seed
is sealed, so no in-scope edit can make a gated cell INCONCLUSIVE: an edit makes it NotJudged instead.

## 3. False refusals

| Check | Result |
|---|---|
| The whole M8a bed, read only, with the committed seal (8 at once, 390 s) | 433 of 433 bound by the seal and accepted by `check_planned`: 400 cells, 20 TCR, 10 atmospheric SPPS, 1 atmospheric TCR, N5 and N6. 0 `bed_seed_outputs_identical` |
| Re-judged with `report.json`'s transports | `pass true`, 0 failures, E1 true over 433 runs, E2 true, `needs_extension []` |
| Against `report.json` | Cell verdicts 40 of 40 equal. Every seed's T30 array and error equal in 40 of 40 cells. TCR verdicts 20 of 20 and D band verdicts equal. Atmospheric seeds' T30 10 of 10 equal. N5 and N6 `as_required` true, as before |
| The seal against the bed, re-hashed by Python | 16,861 of 16,861 files and 1,118,668,944 bytes as sealed, and the 16,861 files under `runs/` are exactly the sealed ones. Bed: 16,985 files, 1,238,241,863 bytes, newest mtime 2026-09-29T15:25:01.891Z, `report.json` `5c0041fc…`, `summary.json` `6af1656e…`, the same before and after |
| Fresh runs by this build (`C:\tmp\nm-judge-tamper\fresh`), no seal | TCR `5x4x3-a0.2-tcr-air-off` and N5 accepted, bound by `run.json` |
| A cell's ten seeds under their true records, no seal (U0) | Accepted, reads bit-identical to the sealed reads, `pass true` |

One refusal of a valid bed by the way it is named:
- **What happens.** `Seal::for_bed` compares the folder name case-sensitively (`bed/bind.rs:149`).
  `--from C:\tmp\nm-m8a-bed\20260929t093134z` is the same folder on Windows, and it is refused with
  exit 2 before any run is read.
- **What the gate does with it.** `m8a.ps1` finds the seal case-insensitively (`Test-Path`), then
  `simpa bed` refuses it, and the gate fails.
- **Why it does not count against `ok`.** It fails closed with a clear message, and no run is judged
  wrongly. It is finding 4.

## 4. Findings

1. **major: without a seal, the bed binds a run only to its own `run.json`. So a forged record and one
   unchecked padding byte per file reproduce both hole-41 cases, and this reaches the M8a bed through a
   renamed folder or `--from` without `--seal`.**
   - Failure:
     - U1: seed 3 carrying seed 4's outputs gives `pass true`.
     - U2: seeds 2 to 10 carrying seed 1's outputs give `pass true`, cell Pass on ten identical seeds
       (B spread 0.0).
   - Why:
     - `bind` holds a run to `run.json`'s `outputs` when no seal names it, and `run.json` sits in the
       folder being edited.
     - `refuse_shared_outputs` compares bytes, and the gabe reader ignores header and padding bytes and
       everything after the last column.
     - `m8a.ps1 -From` and `simpa bed --from` both go on without a seal.
   - Backlog row 44 is open for a forged `run.json` on a new bed. It does not say that:
     - the M8a bed is exposed the same way under another folder name;
     - the gate proceeds without a seal.
   - Suggested closure (row 44, extended):
     - `simpa bed --from` refuses a run bound by `run.json` alone unless the bed was just made by this
       process. In practice `--from` requires `--seal`.
     - `m8a.ps1 -From` throws when no committed seal is found, rather than going on.
     - Each bed a gate judges is sealed and the seal committed, as row 44 says.
     - A test: a renamed copy of a sealed run folder with a forged `run.json` is refused.
   - Defence in depth, not closure:
     - compare seeds on what is read (the decoded tables or the T30 arrays), not on bytes;
     - have B refuse a spread of exactly 0.
2. **minor: with a seal given, a run the seal does not name is bound by its `run.json` alone, so gate C's
   extension seeds would escape the seal.**
   - X: a fabricated seed 11 is accepted with the committed seal, `bound by run.json`.
   - M8a cannot reach it (`needs_extension []`, and every first-ten seed is sealed).
   - Any sealed bed with an INCONCLUSIVE gated cell can.
   - Suggested closure: when a seal is given, every run read from `--from`, extension seeds included,
     must have an entry, or be refused `bed_run_unbound`.
3. **minor: the record overstates what stands after a forged `run.json`.**
   - `FIXES.md` ("What this does not show") and backlog row 44 name the seed rule, and for SPPS E3 and
     C, as what stands after it. U1 and U2 pass all three.
   - The seed rule is sound for what `FIXES.md` also gives it: a solver that ignores its seed would
     write byte-identical files. It is not a defence against a deliberate copy.
   - The two texts should say so.
4. **minor: `Seal::for_bed` compares the folder name case-sensitively.**
   - The M8a bed given as `…\20260929t093134z` is refused with its own seal (library, A2a). Through
     `m8a.ps1` that fails the gate.
   - It fails closed. Comparing names case-insensitively on Windows would fix it.

Findings 2 to 4 alone would leave `ok` true. Finding 1 is an in-scope tamper that passes.

## What this does not show

- **The gate end to end on a forged, renamed copy.** It needs a full copy of the bed. The path is
  shown by the code lines cited in 2b, and by the harness, which calls the functions the CLI calls.
- **A file symbolic link.** It could not be made without the privilege (2a).
- **N2, N3, N4, N7 and N8 on a forged bed.** They were not run. They read what `read_existing` reads,
  with no seal when `SIMPA_BED_SEAL` is empty.
- **The seal edited consistently.** This is out of scope by definition: its totals recomputed, or an
  entry replaced with a tampered folder's hashes. `FIXES.md`'s forged-seal column shows the layers
  after it.
- **Files changed between the binding and the reading.** Out of scope, as `FIXES.md` says.

## Scratch left in place (nothing deleted)

- `C:\tmp\nm-judge-tamper\adv1\`, about 160 MB and 5,985 files:
  - `cases\`: every case above, 60 folders;
  - `harness\`: its binary is `C:\tmp\nm-target\release\nm-adv1.exe`;
  - `bed_fingerprint.py`, `fingerprint-before.txt` and `fingerprint-after.txt`;
  - `harness-out.txt`, and `harness-err.txt`, which is empty.
- `cases\A7a` to `A7d` hold four junctions, and `A7g` and `A7h` two hard links. Each points into
  `cases\A7-sources\`, never into the bed. `A7e` and `A7f` are the copies whose symbolic link could not
  be made: each lacks its `Sound level.recp`.
- No new target folder: the build used `C:/tmp/nm-target`. Free space on C: at the end was 19,706 MB.
