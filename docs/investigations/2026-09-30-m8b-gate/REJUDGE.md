# M8b, step 1: the real M8a bed re-judged by the fixed gate

**M8a PASSED: 23 of 23 checks, gate exit 0.** `report.pass` is true with 0 failures. Every A, B, C and D
statistic is bit-identical to the original `C:\tmp\nm-m8a-bed\20260929T093134Z\report.json`: the largest
difference over all 42,396 numeric values of the two reports is **0**. 433 of 433 runs matched their plan,
and no run was refused `bed_run_not_planned`. The fixes do not reject a valid bed.

The run did not start any SPPS run, and the M8a bed was not written: its file count, byte count, newest
mtime, and the sha256 of its `report.json` and `summary.json` are the same after the run as before.

## Settings

| | |
|---|---|
| Command | `powershell -File tools/gates/m8a.ps1 -From C:/tmp/nm-m8a-bed/20260929T093134Z -Jobs 12`, in the `m8b` worktree, through a wrapper that stamps each line with the clock |
| Environment | `CARGO_TARGET_DIR=C:/tmp/nm-target`, `CARGO_BUILD_JOBS=16`, `CARGO_INCREMENTAL=0`, `SIMPA_SOLVERS_DIR=C:/tmp/nm-m8a-solvers` |
| Build | `8c2e603` (the fix commit). `report.meta.git_commit` is `8c2e603…`, and `git_dirty` is false |
| Tree during the say-NO checks | `d56d6c0` landed on `m8b` at 03:32:50, during N4. It adds only `VERIFY-adversarial.md` and its receipts: `git diff 8c2e603 d56d6c0 -- . ':!docs'` is empty. The code gated is `8c2e603`'s throughout |
| Bed read | `C:\tmp\nm-m8a-bed\20260929T093134Z`, read only (`meta.from`) |
| Work folder | `C:\tmp\nm-target\gates\m8a\20260930-025530`. The re-judged report is `reread\20260930T005634Z\report.json` |
| Machine | GRACE. A second agent ran its own check of the fixes from 03:03 to 03:35 (`VERIFY-adversarial.md`), sharing the target folder and the CPU |

## Times

| | Clock (+02:00) |
|---|---|
| Gate start | 02:55:29 |
| E1, then E5 | 02:55:30, then 02:56:11 |
| `simpa bed --from` | 02:56:34 to 03:31:12, 2,078 s in all |
| &nbsp;&nbsp;Reading the 433 runs | 290 s (`meta.solver_phase_s`). The original bed ran them in 19,621 s |
| &nbsp;&nbsp;The transport phase | 1,787 s (`meta.transport_phase_s`). The original took 1,559 s. The second agent was building, reading the bed and running SPPS at the same time |
| Report checks, plots, N1 to N3 | 03:31:12 to 03:32:51 |
| N4, which traces the transports again | 03:32:51 to 04:05:15 |
| N7, which traces the air-off transports again | 04:05:15 to 04:11:39 |
| N8, N5, N6, unit tests, clippy, fmt | to 04:11:46 |
| Gate end, `exit 0` | 04:11:46, 1 h 16 min 17 s after the start |

## No SPPS run was started

- `simpa bed` printed `reading 433 runs from C:/tmp/nm-m8a-bed/20260929T093134Z`, the `--from` path
  (`run::read_existing`). No cell needed gate C's extension (`needs_extension` is `[]`).
- The re-read stamp folder has no `runs/`. It holds 124 files: the 41 decay CSVs `simpa bed` counted,
  then its `report.json` and `summary.json`, and the plot script's 81 (`.npz`, `.png` and
  `summary.png`).
- N1 refused before any run (E1, exit 2) and wrote nothing. `n1-out` does not exist.
- One `spps.exe` was seen during the run. It was the second agent's real 10 ms run
  (`C:\tmp\nm-judge-m8b\bed10\runs\5x4x3-a0.4-energetic-air-off\s1\20260930-031349-616-spps`, 03:13:49
  to 03:16:38; `VERIFY-adversarial.md` gives it 170 s). At that time the gate's `simpa bed` was in its
  in-process transport phase. It finished transports 24 to 32 between 03:13:55 and 03:16:37, and 33
  at 03:17:10.
- The process watch has one gap, from 03:47 to 03:52, while the monitor was re-armed. The gate was in
  N4 then, and N4 re-traces the transports in-process.

The bed's fingerprint was taken at 02:54:59, before the gate, and at 04:12:32, after it:

| | Before | After |
|---|---|---|
| Files | 16,985 | 16,985 |
| Bytes | 1,238,241,863 | 1,238,241,863 |
| Newest mtime (UTC) | 2026-09-29T15:25:01.891 (`plots\summary.png`) | the same |
| `report.json` sha256 | `5C0041FC…0FAD` | the same |
| `summary.json` sha256 | `6AF1656E…3CED` | the same |

## Against the original gate run (`../2026-09-29-m8a/gate-j12.txt`)

- **The 23 PASS lines are the same text, in the same order.** No line is FAIL.
- **Every note line matches.** That includes N2's and N3's per-band percentages, N8's `+1.230 %`, N5's
  `not_judged, failed by E6` and N6's `params_reference_not_applicable`. The comparison leaves out the
  clock, each transport's seconds and the paths.
- One note line differs, and the difference is expected: `the bed's files`.

  | | Original | Rerun |
  |---|---|---|
  | Files counted | 16,904 under the bed | 16,985 under the `-From` folder |
  | MB counted | 1,158.6 | 1,180.9 |
  | Report's count | 16,902 files | 41 files, the re-read stamp only |

  With `-From`, the gate counts the bed folder, and that folder now holds the 81 plot and `.npz` files
  the original gate's plot step wrote after its own count (16,904 + 81 = 16,985). The check still
  passes: under 20,000.

## `report.json` against the original

`rejudge-compare.py` reads both reports and walks every value. `rejudge-compare-out.txt` is its output.
It leaves out `meta` and `files`, which differ by construction. It compares `folder`, `root` and `from`
as paths, ignoring the slash direction.

| Section | Numeric values | Largest difference |
|---|---|---|
| `cells[].a` (A: the band intervals against Kuttruff) | 2,380 | 0 |
| `cells[].b` (B: seed spread) | 814 | 0 |
| `cells[].c` (C: against the transport, and its interval) | 1,073 | 0 |
| `tcr[].d` (D: TCR against its analytic value) | 532 | 0 |
| `cells[].seeds` (every seed's T30s, MC sd and run record) | 17,296 | 0 |
| `cells[].reference` (Kuttruff, Eyring, Sabine, γ², the transports) | 2,944 | 0 |
| `cells[].reported`, `random_against_energetic`, `atmospheric_validation` | 15,472 | 0 |
| `transports` (all 68, traced again in this run) | 1,008 | 0 |
| All the other sections | 877 | 0 |
| **All** | **42,396** | **0** |

- Non-numeric differences: **0**. Every verdict, refusal code and failure string is the same, and every
  run folder is the same path.
- `pass` true against true. Failures 0 against 0. `exploratory` false against false. `needs_extension`
  `[]` against `[]`. E1 to E7 all hold.
- Cell verdicts: 40 of 40 the same (32 gated `pass`, 8 `reported`). TCR verdicts: 20 of 20 the same.
- N5 `as_required` true against true, with `error` null. N6 the same.
- Keys that only the new report has: those are the fix's, and each has `serde(default)`:
  - `band_problems`: on each of the 20 TCR runs, all empty. The gated TCR runs have 104 of 104 D bands,
    and the largest |deviation| is 4.4 × 10⁻⁷ against the 0.5 % limit, as in the original.
  - `project_sha256`: on all 433 runs, with none null and 412 distinct. The 21 repeats are by design. A
    TCR run solves its `project_of` cell's seed-1 project, so each of the 20 TCR runs shares its sha256
    with that energetic cell's seed 1. The atmospheric TCR run shares its sha256 with atmospheric SPPS
    seed 1.
- `summary.json`: its 607 original rows are equal, row for row. It adds 20 `D bands` rows, one per TCR
  run (16 gated, 4 reported). Each has value 0, limit 0 and verdict `pass`.
- The 41 `decays/*.csv` that `simpa bed` wrote are byte-identical to the original's (`cmp`).

## Findings

- **No run was refused.** The fixes accept all 433 runs of the real bed. They change none of its numbers
  or verdicts.
- The rerun's `report.json` validates against the new schema, which has the two new keys.
- **Process, not product.** The wrapper also appended each line to a log file. A `tail -f` run to watch
  that file held it open, and 83 appends failed. The transcript below is rebuilt from the task's captured
  stdout, which has every line. `rejudge-gate.txt` beside this file is that transcript.

## What this does not show

- **A refusal on real data.** No run here should be refused, and none was. The refusals are shown by the
  tests in `FIXES.md`, and on real runs by `fixcheck-out.txt` and `VERIFY-adversarial.md`.
- **A fresh bed.** This run went through `--from`. `check_planned` is on the same `into_reads` path for a
  fresh run, but `simpa bed` was not run from scratch here.

## Scratch left in place (nothing deleted)

- `C:\tmp\nm-target\gates\m8a\20260930-025530\`: the work folder. It holds 134 files, 115.6 MB: the
  re-read report and summary, `decays/`, `plots/`, `n1-solvers/`, and the bed, plot and N1
  stdout/stderr.
- In the session scratchpad
  `C:\Users\Burhan\AppData\Local\Temp\claude\b--repos-I-Simpa-Night-Mode\2944fcd7-7038-44bd-9fc4-e0055382b531\scratchpad\`:
  - `rejudge.ps1`, the wrapper;
  - `rejudge-gate.txt`, the partial log: its appends failed, see above;
  - `rejudge-gate-full.txt`, the transcript;
  - `compare_reports.py` and `compare-out.txt`;
  - `watch-solvers.ps1`, the process watch.
- `__pycache__/` beside this file, from a syntax check of `rejudge-compare.py`. It is gitignored.

## Gate output

```
START 2026-09-30 02:55:29 +02:00
02:55:30 work: C:\tmp\nm-target\gates\m8a\20260930-025530
02:55:30 solvers: C:/tmp/nm-m8a-solvers
02:55:30 bed file: B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b\beds\m8a.json
02:55:30 upstream: B:\repos\I-Simpa-upstream
02:55:30       code 550485c6952925013b421207a726e0f0f0faf45d17652955445c562b6601963d, raw 1d9900db6cc260039c53963754d4df8de4370c7c6a65cfe998a66cb5720822e8
02:55:30 PASS  E1 spps.exe in SIMPA_SOLVERS_DIR has the code sha256 solvers/manifest.json lists
02:55:30       code fad4ab5d3f024829ed7398de3de326f89148c4ad5d70a0743ab037954bd82234, raw c4a7bca35374af01405a879e649dc0e746f9267c6449e47969bb41e2f4add863
02:55:30 PASS  E1 classicalTheory.exe in SIMPA_SOLVERS_DIR has the code sha256 solvers/manifest.json lists
02:55:30       code d0704003805eb9fdf22c8a2ebf18f2ad6270cb1d59086aa40f86fb12cac5c60f, raw ac68f014a3b2003dc6f88ca9b3dc53e5c193888868ab9029b636e89d5be8a8f9
02:55:30 PASS  E1 tetgen.exe in SIMPA_SOLVERS_DIR has the code sha256 solvers/manifest.json lists
02:55:30       code 7d4ba6afab56b2a13053c76213876f081cf5786abf8cec01f21a3c62f3d79e26, raw 34d9b87424f90af9999434d1c69747355d66eabf217f2cdb09448986649718d0
02:55:30 PASS  E1 preprocess.exe in SIMPA_SOLVERS_DIR has the code sha256 solvers/manifest.json lists
02:56:11 PASS  E5 cargo test --release -p simpa-core --test params_kuttruff: Kuttruff within the bare 0.6 % of the committed transport T30s in M8's 8 cells
03:31:12       exit 0 after 2,078 s; C:\tmp\nm-target\gates\m8a\20260930-025530\reread\20260930T005634Z
03:31:12 PASS  simpa bed ran and exited 0 (0 only when report.pass is true; 8 not passed, 5 a run not OK)
03:31:16 PASS  report.json validates against its schema (simpa bed --schema, by a JSON Schema validator)
03:31:16 PASS  report.exploratory is false: the bed file is M8a's matrix (E7)
03:31:16       0 failures
03:31:16 PASS  report.pass is the JSON true
03:31:17       433 run.json read; 0 not the checked files
03:31:17 PASS  E1 again, from the files: every run's run.json executable, and every mesh.json's TetGen, is the raw sha256 of the file checked
03:31:18       16985 files, 1,180.9 MB under C:/tmp/nm-m8a-bed/20260929T093134Z; the report counted 41 files, 89.0 MB
03:31:18 PASS  the bed's files: fewer than the 20,000 a bed may leave (B: is exFAT with 128 KB clusters)
03:31:37       40 cells; missing: 
03:31:37 PASS  the decays and plots: tools/bed/m8a_plots.py writes decays/<cell>.npz, plots/<cell>.png and plots/summary.png
03:31:38       exit 2; simpa: bed refused before any run (E1): C:\tmp\nm-target\gates\m8a\20260930-025530\n1-solvers\spps.exe is not the verified build: code sha256 46c7df488d46f57c480ebdde6aa68c4bc350916e05c31609bb79f841a1a1e186 is not the verified build's 550485c6952925013b421207a726e0f0f0faf45d17652955445c562b6601963d (solvers/manifest.json); 0 things written
03:31:38 PASS  N1 says NO: a copy of spps.exe with one .text byte flipped: the bed refuses before any run (E1), exit 2, nothing written
03:32:14       6x10x3-a0.4-random-air-off: A Some(Fail): 125 Hz -11.25 %, 250 Hz -11.07 %, 500 Hz -11.37 %, 1000 Hz -11.22 %, 2000 Hz -11.24 %, 4000 Hz -11.25 %
03:32:15       6x10x3-a0.4-energetic-air-off: A Some(Fail): 125 Hz -11.22 %, 250 Hz -11.30 %, 500 Hz -11.24 %, 1000 Hz -11.30 %, 2000 Hz -11.21 %, 4000 Hz -11.22 %
03:32:15       6x10x3-a0.4-random-air-on: A Some(Fail): 125 Hz -11.13 %, 250 Hz -11.14 %, 500 Hz -11.15 %, 1000 Hz -11.14 %, 2000 Hz -11.04 %, 4000 Hz -10.68 %, 8000 Hz -9.75 %
03:32:15       6x10x3-a0.4-energetic-air-on: A Some(Fail): 125 Hz -11.21 %, 250 Hz -11.27 %, 500 Hz -11.19 %, 1000 Hz -11.21 %, 2000 Hz -11.04 %, 4000 Hz -10.74 %, 8000 Hz -9.59 %
03:32:15       5x4x3-a0.4-random-air-off: A Some(Fail): 125 Hz -10.22 %, 250 Hz -10.57 %, 500 Hz -10.33 %, 1000 Hz -10.51 %, 2000 Hz -10.49 %, 4000 Hz -10.33 %
03:32:15       5x4x3-a0.4-energetic-air-off: A Some(Fail): 125 Hz -10.42 %, 250 Hz -10.38 %, 500 Hz -10.44 %, 1000 Hz -10.39 %, 2000 Hz -10.42 %, 4000 Hz -10.41 %
03:32:15       5x4x3-a0.4-random-air-on: A Some(Fail): 125 Hz -10.39 %, 250 Hz -10.31 %, 500 Hz -10.27 %, 1000 Hz -10.24 %, 2000 Hz -10.27 %, 4000 Hz -9.90 %, 8000 Hz -9.17 %
03:32:15       5x4x3-a0.4-energetic-air-on: A Some(Fail): 125 Hz -10.41 %, 250 Hz -10.37 %, 500 Hz -10.41 %, 1000 Hz -10.33 %, 2000 Hz -10.28 %, 4000 Hz -10.02 %, 8000 Hz -9.20 %
03:32:15 PASS  N2 says NO: Kuttruff with its 1/2 dropped (through results::reference): A fails in every alpha 0.4 gated cell
03:32:51       6x10x3-a0.4-random-air-off: A Some(Fail): 125 Hz +10.75 %, 250 Hz +10.97 %, 500 Hz +10.60 %, 1000 Hz +10.79 %, 2000 Hz +10.76 %, 4000 Hz +10.75 %
03:32:51       6x10x3-a0.4-energetic-air-off: A Some(Fail): 125 Hz +10.79 %, 250 Hz +10.70 %, 500 Hz +10.77 %, 1000 Hz +10.69 %, 2000 Hz +10.80 %, 4000 Hz +10.79 %
03:32:51       6x10x3-a0.4-random-air-on: A Some(Fail): 125 Hz +10.89 %, 250 Hz +10.84 %, 500 Hz +10.76 %, 1000 Hz +10.70 %, 2000 Hz +10.61 %, 4000 Hz +10.30 %, 8000 Hz +8.94 %
03:32:51       6x10x3-a0.4-energetic-air-on: A Some(Fail): 125 Hz +10.79 %, 250 Hz +10.67 %, 500 Hz +10.72 %, 1000 Hz +10.61 %, 2000 Hz +10.62 %, 4000 Hz +10.23 %, 8000 Hz +9.13 %
03:32:51       5x4x3-a0.4-random-air-off: A Some(Fail): 125 Hz +9.48 %, 250 Hz +9.06 %, 500 Hz +9.35 %, 1000 Hz +9.13 %, 2000 Hz +9.16 %, 4000 Hz +9.35 %
03:32:51       5x4x3-a0.4-energetic-air-off: A Some(Fail): 125 Hz +9.24 %, 250 Hz +9.28 %, 500 Hz +9.21 %, 1000 Hz +9.27 %, 2000 Hz +9.24 %, 4000 Hz +9.25 %
03:32:51       5x4x3-a0.4-random-air-on: A Some(Fail): 125 Hz +9.27 %, 250 Hz +9.34 %, 500 Hz +9.35 %, 1000 Hz +9.34 %, 2000 Hz +9.15 %, 4000 Hz +9.08 %, 8000 Hz +8.19 %
03:32:51       5x4x3-a0.4-energetic-air-on: A Some(Fail): 125 Hz +9.24 %, 250 Hz +9.27 %, 500 Hz +9.18 %, 1000 Hz +9.22 %, 2000 Hz +9.14 %, 4000 Hz +8.94 %, 8000 Hz +8.16 %
03:32:51 PASS  N3 says NO: plain Eyring as A's reference: A fails in every alpha 0.4 gated cell
04:05:15       uniform-reflection transport 1/68 6x10x3 α 0.05 air None: 29 s
04:05:15       uniform-reflection transport 2/68 6x10x3 α 0.1 air None: 28 s
04:05:15       uniform-reflection transport 3/68 6x10x3 α 0.2 air None: 27 s
04:05:15       uniform-reflection transport 4/68 6x10x3 α 0.4 air None: 24 s
04:05:15       uniform-reflection transport 33/68 5x4x3 α 0.05 air None: 31 s
04:05:15       uniform-reflection transport 34/68 5x4x3 α 0.1 air None: 30 s
04:05:15       uniform-reflection transport 35/68 5x4x3 α 0.2 air None: 29 s
04:05:15       uniform-reflection transport 36/68 5x4x3 α 0.4 air None: 25 s
04:05:15       uniform-reflection transport 65/68 20x8x4 α 0.05 air None: 29 s
04:05:15       uniform-reflection transport 66/68 20x8x4 α 0.1 air None: 28 s
04:05:15       uniform-reflection transport 67/68 20x8x4 α 0.2 air None: 27 s
04:05:15       uniform-reflection transport 68/68 20x8x4 α 0.4 air None: 24 s
04:05:15 PASS  N4 says NO: the transport reflecting evenly over the hemisphere, traced again: C fails in every gated cell
04:11:39       air-off transport 1/68 6x10x3 α 0.05 air None: 26 s
04:11:39       air-off transport 2/68 6x10x3 α 0.1 air None: 25 s
04:11:39       air-off transport 3/68 6x10x3 α 0.2 air None: 25 s
04:11:39       air-off transport 4/68 6x10x3 α 0.4 air None: 22 s
04:11:39       air-off transport 33/68 5x4x3 α 0.05 air None: 27 s
04:11:39       air-off transport 34/68 5x4x3 α 0.1 air None: 26 s
04:11:39       air-off transport 35/68 5x4x3 α 0.2 air None: 25 s
04:11:39       air-off transport 36/68 5x4x3 α 0.4 air None: 22 s
04:11:39       air-off transport 65/68 20x8x4 α 0.05 air None: 26 s
04:11:39       air-off transport 66/68 20x8x4 α 0.1 air None: 26 s
04:11:39       air-off transport 67/68 20x8x4 α 0.2 air None: 24 s
04:11:39       air-off transport 68/68 20x8x4 α 0.4 air None: 21 s
04:11:39 PASS  N7 says NO: the transport traced with the air off: C fails in every air-on gated cell
04:11:39       6x10x3-a0.05-tcr-air-off 125 Hz: D Fail, +1.230 %
04:11:39       6x10x3-a0.05-tcr-air-off 250 Hz: D Fail, +1.230 %
04:11:39       6x10x3-a0.05-tcr-air-off 500 Hz: D Fail, +1.230 %
04:11:39       6x10x3-a0.05-tcr-air-off 1000 Hz: D Fail, +1.230 %
04:11:39       6x10x3-a0.05-tcr-air-off 2000 Hz: D Fail, +1.230 %
04:11:39       6x10x3-a0.05-tcr-air-off 4000 Hz: D Fail, +1.230 %
04:11:39       6x10x3-a0.1-tcr-air-off 125 Hz: D Fail, +1.230 %
04:11:39       6x10x3-a0.1-tcr-air-off 250 Hz: D Fail, +1.230 %
04:11:39       6x10x3-a0.1-tcr-air-off 500 Hz: D Fail, +1.230 %
04:11:39       6x10x3-a0.1-tcr-air-off 1000 Hz: D Fail, +1.230 %
04:11:39       6x10x3-a0.1-tcr-air-off 2000 Hz: D Fail, +1.230 %
04:11:39       6x10x3-a0.1-tcr-air-off 4000 Hz: D Fail, +1.230 %
04:11:39 PASS  N8 says NO: TCR's analytic time with the physical K (through results::tcr): D fails in every band of every gated TCR run
04:11:39       run OK; cell not_judged, failed by E6
04:11:39 PASS  N5 says NO: seed 10 of 5x4x3 alpha 0.4 random at 31,000 particles in its place: the cell does not pass
04:11:39       run OK; kuttruff_s refused: params_reference_not_applicable
04:11:39 PASS  N6 says NO: walls of scattering 0: the bed refuses the cell (params_reference_not_applicable in every band, E3)
04:11:44 PASS  the bed's unit tests: cargo test -p simpa-core --lib bed::
04:11:45 PASS  clippy -D warnings (core and CLI)
04:11:46 PASS  cargo fmt --check (core and CLI)
04:11:46 
04:11:46 work: C:\tmp\nm-target\gates\m8a\20260930-025530
04:11:46 bed: C:\tmp\nm-target\gates\m8a\20260930-025530\reread\20260930T005634Z
04:11:46 M8a PASSED: 23 of 23 checks
END 2026-09-30 04:11:46 +02:00 exit 0
=== DONE ===
```

The N4 and N7 transport lines, and N8's band lines, are printed when each test ends, so they share its
clock time. The gate prints the first 12 of each, as in the original.

`simpa bed`'s own stderr, without its 68 transport lines:

```
E1 ok   spps.exe: code sha256 550485c6952925013b421207a726e0f0f0faf45d17652955445c562b6601963d (C:\tmp\nm-m8a-solvers\spps.exe)
E1 ok   classicalTheory.exe: code sha256 fad4ab5d3f024829ed7398de3de326f89148c4ad5d70a0743ab037954bd82234 (C:\tmp\nm-m8a-solvers\classicalTheory.exe)
E1 ok   tetgen.exe: code sha256 d0704003805eb9fdf22c8a2ebf18f2ad6270cb1d59086aa40f86fb12cac5c60f (C:\tmp\nm-m8a-solvers\tetgen.exe)
E1 ok   preprocess.exe: code sha256 7d4ba6afab56b2a13053c76213876f081cf5786abf8cec01f21a3c62f3d79e26 (C:\tmp\nm-m8a-solvers\preprocess.exe)
[02:56:34] bed B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b\beds\m8a.json: 433 runs, 12 at once, into C:\tmp\nm-target\gates\m8a\20260930-025530\reread\20260930T005634Z (NTFS; about 0 files)
[02:56:34] reading 433 runs from C:/tmp/nm-m8a-bed/20260929T093134Z
[03:31:12] bed PASS: gated cells {"Pass": 32}, reported cells {"Reported": 8}, 0 failures, 41 files, 93.3 MB; C:\tmp\nm-target\gates\m8a\20260930-025530\reread\20260930T005634Z\report.json
```
