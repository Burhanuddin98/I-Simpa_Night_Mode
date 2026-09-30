# M8b tamper, round 1: the real M8a bed re-judged by the gate with the two tamper fixes

**M8a PASSED: 23 of 23 checks, gate exit 0.** `report.pass` is true, with 0 failures.
- **No run was refused.** All 433 runs were bound by the committed seal (`bound_by` `seal` × 433) and
  matched to their plan. None was refused `bed_run_unbound`, `bed_run_files_changed`,
  `bed_seed_outputs_identical` or `bed_run_not_planned`.
- **A, B and C are bit-identical** to the M8a bed's `report.json`, and so is every other section but
  `tcr`.
- **D is not bit-identical.** 164 of `tcr`'s 868 numbers differ, by at most 4.441 × 10⁻¹⁶ s. D's
  reference is now the plan's analytic time (hole 42's fix), and it differs from the run's own by at
  most 2 ulp. All 128 D band verdicts are the same, and all 20 TCR verdicts.
- **The largest difference over all 42,396 numeric values of the two reports is 4.441 × 10⁻¹⁶**, at
  `tcr[4].d.bands[1].analytic_eyring_s`: 2.597263765023588 s against 2.5972637650235884 s.
- **No SPPS run was started, and the M8a bed was not written.** Every file of it has the same path, size,
  mtime and sha256 after the gate as before, and the whole bed matches the seal before and after.

The gate passing on the real bed does not show that the fixes hold against a tamper. See "What this does
not show": adversarial check 1 (`VERIFY-adversarial-1.md`, committed during this run) has a bypass that
passes without a seal, and nothing in this run tests it.

## Settings

| | |
|---|---|
| Command | `powershell -File tools/gates/m8a.ps1 -From C:/tmp/nm-m8a-bed/20260929T093134Z -Jobs 12`, in the `m8b-tamper` worktree. It ran through `rejudge-1-wrapper.ps1`, which stamps each line with the clock and writes it through a single writer open for shared reading, so no line was lost (`rejudge-1-gate.txt`, 99 lines) |
| Environment | `CARGO_TARGET_DIR=C:/tmp/nm-target`, `CARGO_BUILD_JOBS=16`, `CARGO_INCREMENTAL=0`, `SIMPA_SOLVERS_DIR=C:/tmp/nm-m8a-solvers` |
| Tree at the start | `dd5c683`, which is `239c6f5`'s code plus its record. `FIXES.md` names `239c6f5` as the code commit. `origin/m8b-tamper` was `dd5c683`, and the tree was clean |
| The seal | Found by the gate from the folder's name: `seal: …\beds\m8a-20260929T093134Z\outputs-seal.json`. `report.meta.seal_sha256` is `762f3fba…c15e09d`, the sha256 of the committed file |
| Bed read | `C:\tmp\nm-m8a-bed\20260929T093134Z`, read only (`meta.from`) |
| Work folder | `C:\tmp\nm-target\gates\m8a\20260930-061500`. The re-judged report is `reread\20260930T041605Z\report.json` |
| Machine | GRACE. Adversarial check 1 ran from 06:11 to 06:45, by its record. It shared the CPU and the target folder during the read and transport phases |

**The tree moved during the run.**
- Adversarial check 1 committed `a3280e9` on `m8b-tamper` at 06:42:34, during the transport phase.
  That commit adds only its record and receipts: `git diff 239c6f5 a3280e9 -- . ':!docs'` is empty.
- So `report.meta.git_commit` is `a3280e9`. `simpa bed` reads git when it ends, and it ended at 06:50:54.
  `git_dirty` is false.
- The `simpa.exe` that read the bed was built between 06:15:42 and 06:16:05, from `dd5c683`.
- Every build in the gate was of a tree whose code is `239c6f5`'s.

## Times

| | Clock (+02:00) |
|---|---|
| Bed fingerprint, before | 06:13:21 |
| Process watch | 06:14:48 to 07:32:03 |
| Gate start, E1 | 06:15:00 |
| E5 | 06:15:42 |
| `simpa bed --from` | 06:16:05 to 06:50:54. That is 2,088 s by the gate, and `meta.wall_s` is 2,087.4 s |
| &nbsp;&nbsp;Reading and binding the 433 runs | 297.4 s (`meta.solver_phase_s`). Step 1 took 290 s, and FIXES.md's CLI run 309 s |
| &nbsp;&nbsp;The transport phase | 1,789.5 s (`meta.transport_phase_s`). Step 1 took 1,787 s |
| Report checks, plots, N1 | 06:50:54 to 06:51:40 |
| N2, N3 | 06:52:17, 06:52:53 |
| N4, which traces the transports again | 06:52:53 to 07:25:02 |
| N7, which traces the air-off transports again | 07:25:02 to 07:31:22 |
| N8, N5, N6 | 07:31:22 |
| Bed unit tests, clippy, fmt | 07:31:39, 07:31:40, 07:31:41 |
| Gate end, `exit 0` | 07:31:41. That is 1 h 16 min 41 s after the start |
| Bed fingerprint, after | 07:32:09 |

N4's, N7's and N8's note lines are printed when each test ends, so they share its clock time.

## No SPPS run was started

**The process watch** (`rejudge-1-watch.ps1`):
- It polled every 0.3 s for `spps.exe`, `classicalTheory.exe`, `tetgen.exe`, `preprocess.exe` and
  `simpa.exe`, and logged each one's whole parent chain.
- It was armed to stop the gate if a solver's chain reached `m8a.ps1`.
- It saw **0 solver processes** from 06:14:48 to 07:32:03. There were 76 heartbeats, and the largest
  interval between two was 62 s.
- It saw one `simpa.exe`: the gate's `simpa bed … --from C:/tmp/nm-m8a-bed/…`, pid 10152, at 06:16:05.
- **Its limit.** A process shorter than one poll can be missed. N1's `simpa.exe`, refused within a
  second, was not seen. So the watch rules out an SPPS run, which takes minutes, but not a 20 ms TCR run.

**The other evidence**, which does rule that out:
- `simpa bed` printed `reading 433 runs from C:/tmp/nm-m8a-bed/20260929T093134Z`. That is the `--from`
  path (`run::read_existing`), which never calls `run::execute`.
- It printed no extension line, and `needs_extension` is `[]`.
- The re-judged stamp has no `runs/`. It holds 124 files: `simpa bed`'s 41 decay CSVs, its
  `report.json` and `summary.json`, and the plot script's 81 files.
- The work folder holds 134 files and 117 MB. None of them is a run folder (`*-spps`, `*-tcr`).
- N1 refused before any run (E1, exit 2). `n1-out` does not exist.
- The bed itself is unchanged (below).

## The bed was not written

| | Before (06:13:21) | After (07:32:09) |
|---|---|---|
| Files | 16,985 | 16,985 |
| Bytes | 1,238,241,863 | 1,238,241,863 |
| Newest mtime (UTC) | 2026-09-29T15:25:01.891 (`plots/summary.png`) | the same |
| `report.json` sha256 | `5c0041fc…0fad` | the same |
| `summary.json` sha256 | `6af1656e…3ced` | the same |
| Every file's path, size, mtime (ns) and sha256 | manifest digest `c4e427cf…041e6f` | the same. The two manifests are byte-identical (`cmp`) |
| Held to the seal, by Python, not the Rust | 16,861 of 16,861 files under `runs/` as sealed, none extra, none missing. The report and summary hashes are the seal's | the same |

The manifests are in the scratchpad, not the repo: 3.1 MB each (see "Scratch").

## What the new checks did on the real bed (`rejudge-1-out.txt` section 7)

These were read from the re-judged `report.json`. Each run was then held to its own seal entry by
`rejudge-1-new-checks.py`, independently of the Rust.
- **433 run records, 433 distinct run folders.** 400 are cell runs, 20 TCR, 11 atmospheric (10 SPPS, 1
  TCR) and 2 say-NO (N5, N6). All have status `OK`, and every seal entry is among them.
- **`bound_by`:** `seal` on all 433.
- **`on_disk`:** set on all 433, with every field set. The mesh check gave a reason on none. On every
  run:
  - `project.simpa`, `solve/mesh.cbin` and `solve/tetramesh.mbin` as hashed from disk equal the seal's
    sha256 of the same file;
  - the `.mbin` is one file in all three places;
  - the two mesh stamps agree;
  - `run.json`'s project sha256 is `project.simpa`'s on disk.
- **The seed rule, from the report.** There are 41 groups (40 cells and the atmospheric validation), with
  410 seeds. No group has two equal `outputs_sha256`, and all 433 `outputs_sha256` are distinct.
  - This checks whole output sets only. The per-file rule is the product's: it refused nothing here.
    `seed-identity-probe-out.txt` measured it on this bed, with 0 of 8,490 files shared.
- **Refusal codes anywhere in the report:** `bed_run_unbound` 0, `bed_run_files_changed` 0,
  `bed_seed_outputs_identical` 0, `bed_run_not_planned` 0.
- **The bed unit tests the gate ran** (`cargo test -p simpa-core --lib bed::`, listed again after the
  gate with no rebuild) passed 40, with 1 ignored. They include each tamper test `FIXES.md` names under
  `bed::`:
  - the seed-copy and ten-seed cases;
  - the three `bind` refusals and the seal's own check;
  - the three `check_planned` disk tests;
  - the TCR tamper at report level;
  - `d_fails_a_run_whose_bands_are_not_the_beds`.

## `report.json` against the M8a bed's (`rejudge-1-out.txt` sections 3 and 4)

This is step 1's `../2026-09-30-m8b-gate/rejudge-compare.py`, unchanged. It leaves out `meta` and
`files`, and compares `folder`, `root` and `from` as paths.

| Section | Numeric values | Largest difference |
|---|---|---|
| `cells[].a` (A) | 2,380 | 0 |
| `cells[].b` (B) | 814 | 0 |
| `cells[].c` (C) | 1,073 | 0 |
| `cells[].seeds` (every seed's T30s, MC sd and run record) | 17,296 | 0 |
| `cells[].reference` | 2,944 | 0 |
| `cells[].reported`, `random_against_energetic`, `atmospheric_validation` | 15,472 | 0 |
| `transports` (all 68, traced again in this run) | 1,008 | 0 |
| `tcr[].d` (D) | 532 | **4.441 × 10⁻¹⁶ s** |
| `tcr[].sabine_against_analytic` | 128 | **2.220 × 10⁻¹⁶** |
| `tcr[].eyring_against_kuttruff`, `tcr[].run` | 208 | 0 |
| All the other sections | 541 | 0 |
| **All** | **42,396** | **4.441 × 10⁻¹⁶** |

**Which numbers differ.** 164 of `tcr`'s 868:
- `analytic_eyring_s`: 54 of 128 bands, at most 2 ulp;
- `deviation`: 46;
- `sabine_against_analytic`: 64.

The last two are small differences of nearly equal times, so they are best read in absolute terms: at
most 2.22 × 10⁻¹⁶ each.

**D's reference against the run's own.**
- The largest relative difference is **2.58 × 10⁻¹⁶**, at `20x8x4-a0.2-tcr-air-off` (reported, not
  gated). Over the gated TCR runs it is 2.42 × 10⁻¹⁶ (`5x4x3-a0.05-tcr-air-on`, 8 kHz).
- `run_analytic_eyring_s` equals the original `analytic_eyring_s` in 128 of 128 bands.
- The largest |deviation| is 4.41 × 10⁻⁷, against the 0.5 % limit.

**Everything else.**
- Non-numeric differences: **0**. Every verdict, refusal code, failure string and run folder is the
  same.
- `pass`, failures, `exploratory` and `needs_extension` are true, 0, false and `[]`, against the same.
  Cell verdicts are 40 of 40 the same (32 gated `pass`, 8 `reported`), and TCR verdicts 20 of 20.
- N5 and N6 are `as_required` true, as before.
- Keys only the new report has: 1,880, each from step 1 or this fix:
  - `band_problems` × 20, all empty;
  - `project_sha256` × 433;
  - `bound_by` × 433;
  - `on_disk` × 433;
  - `outputs_sha256` × 433;
  - `run_analytic_eyring_s` × 128.

**Against the other two re-judges** (sections 5 and 6):
- **Step 1's re-judge** (`8c2e603`, before this fix): the same 164 D differences and nothing else, and
  1,427 new keys (this fix's).
- **FIXES.md's CLI run** (built at 05:22 from the uncommitted tree): **0 differences of any kind**, and
  no key only in either. So the committed code judges the bed exactly as the build `FIXES.md` measured.

## `summary.json`, the decays, and the gate's lines (sections 8 and 9)

- **`summary.json`.**
  - Its 607 original rows are all present, in the same order.
  - 46 differ, each only in `value`, and all 46 are D rows, by at most 2.22 × 10⁻¹⁶. Every row's
    verdict is the same.
  - It adds step 1's 20 `D bands` rows: value 0, limit 0, `pass`.
  - Its `report_sha256` is the new `report.json`'s sha256.
- **The decays.** The 41 `decays/*.csv` from `simpa bed` and the 40 `.npz` from the plot script are
  byte-identical to the M8a bed's. All 240 arrays are equal.
- **The gate's lines** against step 1's (`../2026-09-30-m8b-gate/rejudge-gate.txt`), leaving out the
  clock, the work folder, the worktree's name and the seconds:
  - The 23 PASS lines are the same text in the same order, and there is no FAIL.
  - Every note line is the same: N2's and N3's per-band percentages, N8's `+1.230 %` in every listed
    band, N5's `not_judged, failed by E6`, N6's `params_reference_not_applicable`, and `the bed's files`
    (16,985 files, 1,180.9 MB; the report counted 41 files, 89.0 MB).
  - The one added line is `seal: …\beds\m8a-20260929T093134Z\outputs-seal.json`.

## Findings

1. **As expected: no valid run is refused, and no verdict or A, B or C number changes.** Everything
   this round was expected to show held but one. D is equal to within 2 ulp, not bit for bit
   (finding 2).
2. **minor, the expectation's wording: "every A/B/C/D statistic equal" holds for A, B and C only.**
   - D's 164 numbers differ by at most 4.441 × 10⁻¹⁶ s. This is the change hole 42's fix makes on
     purpose, and `FIXES.md` states it.
   - No D verdict changes: 128 of 128 bands and 20 of 20 runs are the same.
3. **minor, the record: `FIXES.md` understates D's difference.**
   - It says D's reference "differs from the run's own by at most 2.2 × 10⁻¹⁶ relative over 128
     bands".
   - Measured, it is 2.58 × 10⁻¹⁶, and 2.42 × 10⁻¹⁶ over the gated runs. 13 of the 128 bands are
     above 2.2 × 10⁻¹⁶.
   - This is 2 ulp, and no verdict depends on it. But the number in the record is not the measured one.
     This file does not edit `FIXES.md`.
4. **process: `report.meta.git_commit` names `a3280e9`, not the `dd5c683` the gate started on.**
   - A docs-only commit by another agent landed in this worktree mid-run, as `d56d6c0` did in step 1.
   - The code is the same throughout.
   - `meta.git_commit` records the tree when `simpa bed` ends, not the tree its binary was built from.

## What this does not show

- **That the fixes hold against a tamper.** This run judged only the valid bed, through its seal.
  - Adversarial check 1 (`VERIFY-adversarial-1.md`, `a3280e9`) finds a major in-scope bypass (its
    finding 1). Without a seal, a forged `run.json` plus one unchecked gabe padding byte per output file
    gives `pass true` on both hole-41 cases.
  - It reaches the M8a bed through a renamed folder, because `m8a.ps1` looks for the seal by the
    folder's name and carries on without one (`tools/gates/m8a.ps1:60-62`, `:146`).
  - This run confirms only that precondition: the gate has no check of `report.meta.seal`. Here the
    seal was found, and `meta.seal` is set. Nothing here tests the bypass.
- **The per-file seed rule on real data.** Section 7 checks whole output sets. The per-file rule is shown
  by the product refusing nothing, and by `seed-identity-probe-out.txt`.
- **A fresh bed.** Everything went through `--from`.
- **Byte identity with what the M8a judge read.** This is the seal's own limit, as `FIXES.md` says. This
  run shows that the bed is as sealed at 06:13 and at 07:32.
- **The scripts as run.** The committed scripts are byte-identical to the ones that ran, with one
  exception. The two "before" runs (fingerprint and seal check, 06:13) ran without each script's first
  line, `import sys as _s; _s.stdout.reconfigure(encoding="utf-8")`. That line was added at 06:18 so
  that `α` prints. There is no other change, and the "after" runs used the committed text.

## Receipts beside this file

| File | What it is |
|---|---|
| `rejudge-1-gate.txt` | The gate transcript, every line with its clock |
| `rejudge-1-out.txt` | The output of every check above, in 12 sections, each with its command. It includes two one-off scripts inline, with their code |
| `rejudge-1-wrapper.ps1`, `rejudge-1-watch.ps1` | The wrapper and the process watch |
| `rejudge-1-fingerprint.py`, `rejudge-1-seal-check.py` | The bed's fingerprint and per-file manifest; the manifest held to the seal |
| `rejudge-1-new-checks.py` | What the new checks put in the report, each run held to its seal entry |
| `rejudge-1-d-ulps.py` | D's differences in ulp, and D's reference against the run's own |
| `rejudge-1-outputs.py` | `summary.json`, the decay CSVs and the `.npz` against the bed's |
| `rejudge-1-gate-lines.py` | The gate's lines against step 1's |

## Scratch left in place (nothing deleted)

- `C:\tmp\nm-target\gates\m8a\20260930-061500\`: the work folder, 134 files and 117 MB. It holds the
  re-judged `report.json`, `summary.json`, `decays/` and `plots/`, `n1-solvers/`, and the bed, plot and
  N1 stdout and stderr.
- In the session scratchpad
  `C:\Users\Burhan\AppData\Local\Temp\claude\b--repos-I-Simpa-Night-Mode\2944fcd7-7038-44bd-9fc4-e0055382b531\scratchpad\`:
  28 files named `r1-*` and `r1_*`, 6.2 MB. They are:
  - the scripts as run;
  - `r1-gate.txt`;
  - `r1-watch.log`, and the empty `r1-watch.stop` that ended the watch;
  - the two 3.1 MB manifests `r1-bed-before.tsv` and `r1-bed-after.tsv`;
  - each check's output.
- Free space on C: was 19,884 MB at 06:12:58 and 19,586 MB at 07:32:52. No new target folder was made.

## Gate output

```
START 2026-09-30 06:15:00 +02:00 wrapper pid 23288 head dd5c6833e411a8721c581516e1ed5cb735c7f41a
06:15:00 work: C:\tmp\nm-target\gates\m8a\20260930-061500
06:15:00 solvers: C:/tmp/nm-m8a-solvers
06:15:00 bed file: B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b-tamper\beds\m8a.json
06:15:00 upstream: B:\repos\I-Simpa-upstream
06:15:00 seal: B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b-tamper\beds\m8a-20260929T093134Z\outputs-seal.json
06:15:00 PASS  E1 spps.exe in SIMPA_SOLVERS_DIR has the code sha256 solvers/manifest.json lists
06:15:00 PASS  E1 classicalTheory.exe in SIMPA_SOLVERS_DIR has the code sha256 solvers/manifest.json lists
06:15:00 PASS  E1 tetgen.exe in SIMPA_SOLVERS_DIR has the code sha256 solvers/manifest.json lists
06:15:00 PASS  E1 preprocess.exe in SIMPA_SOLVERS_DIR has the code sha256 solvers/manifest.json lists
06:15:42 PASS  E5 cargo test --release -p simpa-core --test params_kuttruff: Kuttruff within the bare 0.6 % of the committed transport T30s in M8's 8 cells
06:50:54       exit 0 after 2,088 s; C:\tmp\nm-target\gates\m8a\20260930-061500\reread\20260930T041605Z
06:50:54 PASS  simpa bed ran and exited 0 (0 only when report.pass is true; 8 not passed, 5 a run not OK)
06:51:20 PASS  report.json validates against its schema (simpa bed --schema, by a JSON Schema validator)
06:51:20 PASS  report.exploratory is false: the bed file is M8a's matrix (E7)
06:51:20       0 failures
06:51:20 PASS  report.pass is the JSON true
06:51:21       433 run.json read; 0 not the checked files
06:51:21 PASS  E1 again, from the files: every run's run.json executable, and every mesh.json's TetGen, is the raw sha256 of the file checked
06:51:22       16985 files, 1,180.9 MB under C:/tmp/nm-m8a-bed/20260929T093134Z; the report counted 41 files, 89.0 MB
06:51:22 PASS  the bed's files: fewer than the 20,000 a bed may leave (B: is exFAT with 128 KB clusters)
06:51:40       40 cells; missing: 
06:51:40 PASS  the decays and plots: tools/bed/m8a_plots.py writes decays/<cell>.npz, plots/<cell>.png and plots/summary.png
06:51:40       exit 2; simpa: bed refused before any run (E1): C:\tmp\nm-target\gates\m8a\20260930-061500\n1-solvers\spps.exe is not the verified build: code sha256 46c7df48… is not the verified build's 550485c6… (solvers/manifest.json); 0 things written
06:51:40 PASS  N1 says NO: a copy of spps.exe with one .text byte flipped: the bed refuses before any run (E1), exit 2, nothing written
06:52:17 PASS  N2 says NO: Kuttruff with its 1/2 dropped (through results::reference): A fails in every alpha 0.4 gated cell
06:52:53 PASS  N3 says NO: plain Eyring as A's reference: A fails in every alpha 0.4 gated cell
07:25:02 PASS  N4 says NO: the transport reflecting evenly over the hemisphere, traced again: C fails in every gated cell
07:31:22 PASS  N7 says NO: the transport traced with the air off: C fails in every air-on gated cell
07:31:22       6x10x3-a0.05-tcr-air-off 125 Hz: D Fail, +1.230 %
07:31:22 PASS  N8 says NO: TCR's analytic time with the physical K (through results::tcr): D fails in every band of every gated TCR run
07:31:22       run OK; cell not_judged, failed by E6
07:31:22 PASS  N5 says NO: seed 10 of 5x4x3 alpha 0.4 random at 31,000 particles in its place: the cell does not pass
07:31:22       run OK; kuttruff_s refused: params_reference_not_applicable
07:31:22 PASS  N6 says NO: walls of scattering 0: the bed refuses the cell (params_reference_not_applicable in every band, E3)
07:31:39 PASS  the bed's unit tests: cargo test -p simpa-core --lib bed::
07:31:40 PASS  clippy -D warnings (core and CLI)
07:31:41 PASS  cargo fmt --check (core and CLI)
07:31:41 M8a PASSED: 23 of 23 checks
END 2026-09-30 07:31:41 +02:00 exit 0
```

The extract above shortens the hashes, and leaves out the E1 code lines, N2's and N3's eight per-cell lines
each, N4's and N7's twelve transport lines each, and N8's other eleven band lines. `rejudge-1-gate.txt`
has every line.

`simpa bed`'s own stderr, without its 68 transport lines:

```
E1 ok   spps.exe: code sha256 550485c6952925013b421207a726e0f0f0faf45d17652955445c562b6601963d (C:\tmp\nm-m8a-solvers\spps.exe)
E1 ok   classicalTheory.exe: code sha256 fad4ab5d3f024829ed7398de3de326f89148c4ad5d70a0743ab037954bd82234 (C:\tmp\nm-m8a-solvers\classicalTheory.exe)
E1 ok   tetgen.exe: code sha256 d0704003805eb9fdf22c8a2ebf18f2ad6270cb1d59086aa40f86fb12cac5c60f (C:\tmp\nm-m8a-solvers\tetgen.exe)
E1 ok   preprocess.exe: code sha256 7d4ba6afab56b2a13053c76213876f081cf5786abf8cec01f21a3c62f3d79e26 (C:\tmp\nm-m8a-solvers\preprocess.exe)
[06:16:05] bed B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b-tamper\beds\m8a.json: 433 runs, 12 at once, into C:\tmp\nm-target\gates\m8a\20260930-061500\reread\20260930T041605Z (NTFS; about 0 files)
[06:16:05] reading 433 runs from C:/tmp/nm-m8a-bed/20260929T093134Z
[06:50:54] bed PASS: gated cells {"Pass": 32}, reported cells {"Reported": 8}, 0 failures, 41 files, 93.3 MB; C:\tmp\nm-target\gates\m8a\20260930-061500\reread\20260930T041605Z\report.json
```
