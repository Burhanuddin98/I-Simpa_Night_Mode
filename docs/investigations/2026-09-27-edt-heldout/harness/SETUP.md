# M8b harness: build, solver folder, Python and the corpus files

Steps 1 and 2 of `../HARNESS-PLAN.md` section 6, 2026-10-01, on Grace. No solver ran and no held-out data was made.

**In plain words.** `simpa` is built from this branch. The verified solvers are in `C:\tmp\nm-m8a-solvers`, not in
the folder the plan names: both hold the same bytes today, but the plan's folder is a slot that the M10 gate overwrites.
The Python environment is pinned in `requirements.txt`. `corpus.py` writes the two corpus files, and a second run
reproduces them exactly.

## simpa

- Built from HEAD `610e691` into `C:\tmp\nm-target`: `cargo build --release -p simpa`, with `CARGO_TARGET_DIR=C:\tmp\nm-target`,
  `CARGO_INCREMENTAL=0` and `CARGO_BUILD_JOBS=16`. It exited 0 after 22.8 s, at 02:42:55.
- `C:\tmp\nm-target\release\simpa.exe`, 8,135,168 bytes, sha256 `cfe8caff70527eceb1acae3db2951f44714baddfe8213b9208c8657408c9d50d`.
  `simpa --version` prints `simpa 0.1.0 (solvers from upstream 929a5c8)`.
- Checked again at 03:21. The same build from the same HEAD left `simpa.exe` untouched, with the same time and hash.
  `C:\tmp\nm-target` is the cargo target that every worktree shares, so a step that uses the exe checks this hash first.

## Solver folder: `SIMPA_SOLVERS_DIR = C:\tmp\nm-m8a-solvers`

The plan names `C:\tmp\nm-target\target\solvers\bin`. Both folders hold the verified build today, byte for byte, but
the harness points `SIMPA_SOLVERS_DIR` at `C:\tmp\nm-m8a-solvers`, for these reasons:

- **It is the gates' own folder.** M8a's bed ran every run from it: `beds/m8a-20260929T093134Z/report.json` names it
  437 times. M10 and M11 passed with `-SolversDir C:\tmp\nm-m8a-solvers` (`2026-09-29-m11/GATE.md:43, 135, 177`).
- **The plan's folder is M10's staging slot** (`tools/gates/m10.ps1:252-270`). On each run, M10 copies its solver
  folder there, file by file, wherever the hashes differ.
  - Run with neither `-SolversDir` nor `SIMPA_SOLVERS_DIR`, M10 takes the repo's own `target\solvers\bin` first. On
    Grace that folder holds TetGen 1.6.0 (see below). One such run would leave an unverified `tetgen.exe` in the plan's
    folder, and nothing would say so.
  - M11 removed the need for the slot (`2026-09-29-m11/PLAN.md:852-853`).
- **`C:\tmp\nm-target` is shared.** It is the cargo target of every worktree.

The check was done the E1 way (`tools/gates/m8a.ps1:137-153`): `solvers/pe-fingerprint.ps1` against
`solvers/manifest.json`'s `code_sha256`. It was repeated with `tools/fixture-gen/pe_fingerprint.py`. Both folders
match on all four executables:

| exe | manifest code sha256 | `nm-m8a-solvers` | `nm-target\target\solvers\bin` | raw sha256, both |
|---|---|---|---|---|
| spps.exe | `550485c6…` | match | match | `1d9900db…` |
| classicalTheory.exe | `fad4ab5d…` | match | match | `c4a7bca3…` |
| tetgen.exe | `d0704003…` | match | match | `ac68f014…` |
| preprocess.exe | `7d4ba6af…` | match | match | `34d9b874…` |

- **Why the raw sha256s differ from the manifest.** The manifest's `sha256` values carry the link times of the
  2026-09-24 build. The code sha256 is the verified build's fingerprint (`2026-09-29-m11/PLAN.md:62`).
- **The other folders.** `C:\tmp\nm-m10-solvers\bin` holds the same bytes. `B:\repos\I-Simpa_Night_Mode\target\solvers\bin`
  is not the verified build: its `tetgen.exe` has code sha256 `b0c89b7f…`, which is the manifest's TetGen 1.6.0
  reference (`upstream_160_reference_code_sha256`).

This record does not replace P14. Every run still checks its own folder.

## Python

The venv at `C:\tmp\m8b-edt\venv` runs Python 3.13.13 with numpy 2.5.2 and pytest 9.1.1, the same three versions as
Grace's system Python. `requirements.txt` lists every package with its version. Run from this folder with
`PYTHONDONTWRITEBYTECODE=1`:

```
C:\tmp\m8b-edt\venv\Scripts\python.exe -m m8b.corpus
```

A full run takes 131 s with 8 workers, 109 s of it for z3 and z3grid. It writes nothing on B: except the two files.

## corpus.py: `corpus_rooms.json` and `weak_spots.json`

Each file lists the sha256 of every file it read. The `generated` block is last and holds the versions, timings and
time. The text before it reproduced byte for byte in a second run (T22).

### corpus_rooms.json (P2, P3)

There are 119 corpus rooms. Each one is a box by its source's own words:

| Set | Rooms |
|---|---|
| ISM | 4, with the radius variants of `t1` and `dead` |
| z3 | 105 boxes from 230 confirmed cases |
| noise-cal | 6 rooms; 94 cells, of which 32 are the EDT corpus (11 at 1 ms and 21 at 10 ms, W-E6 included) |
| M8a | 3 rooms, plus the atmospheric validation room |

It also lists the critique's double-slope ratios of 3 and 4, and the corpus items with no geometry.

### weak_spots.json (P33)

The file holds 575 rows: 73 wrong-silent and 502 near misses.

**Which rows were scored**

- **Logged sets.** t1, z3, real and ISM are read from `final/res_*.pkl`. Each one was recomputed with the frozen method,
  and every row matches its log: 160, 217, 3,600 and 1,920 rows.
- **run_scans.py's five scans.** These were recomputed, and their tallies reproduce `final/scans_all.json`.
- **The critique's other constructions.** Each is built the way its own file builds it:
  - scan 1 in three runs:
    - `run_scans`;
    - the committed `scan_i12.py`, whose 672 ill-conditioned rows `run_scans.py` left out. Its 4,320 rows match the
      grid and truth in its output;
    - the critique's first run (`scan_i12.json` in `target/`, 6,240 rows, with no 0.6 m cutoff). Its grid matches,
      and so do its truths at d ≥ 0.6 m. Below 0.6 m its truth function, which was not kept, differs from the
      committed one on 187 of 480 rows; on 172 of those the committed one gives no truth. Those rows are scored
      against the committed `synth.truth_edt`;
  - scan 2;
  - scan 2b with the method also called as `scan_i4b.py` calls it, with no meta;
  - scan 5 (a) and (b);
  - the rows of `noise_probe.py` and `ablate.py`;
  - the real seeds as `real_seeds.py` calls the method, which changes no result.
- **z3grid**, through the text copy of the judge's generator (`m8b/_z3echo.py`).

**What was not scored.** `not_scored` lists each item with its reason. These include the W1G attack's rows, which are
not EDT corpus rows: the ISM receivers outside the 160-receiver sample, and z3's 13 non-robust and 22 unconfirmed
cases.

| Set | Wrong-silent | Near miss | What |
|---|---|---|---|
| z3 | 4 | 6 | balls of 0.88-1.49 m, d − R 0.27-0.40 m, delays of 12-60 ms (P33's 10) |
| z3grid | 16 | 50 | steps of 0.1-2 ms, errors −57 % to +25 % |
| ISM | 0 | 2 | dead, 20 kHz, 1 ms (P33's 2) |
| real | 0 | 6 | its truth is the shipped calculator's own reading |
| seeds | 19 | 203 | P33's 19 |
| scan 1 | 3 | 51 | 1 run_scans (P33's 1); 1 ill-conditioned (−8.3 %, DRR +7.8 dB, 1 ms); 1 first-run row at d 0.17 m, inside the 0.31 m ball (−9.8 %) |
| scan 3 | 16 | 108 | P33's 16 |
| scan 5 (a) | 10 | 34 | energetic-mode noise, 150k particles |
| noise_probe | 5 | 33 | scan 3's 200 m³, 5 m configuration at another noise seed |
| ablate | 0 | 8 | |
| scan 4 | 0 | 1 | |

t1, scan 2, scan 2b and scan 5 (b) have no weak row.

**What the plan's text undercounts.** P33's text counts scan 1's 1 wrong row, and does not count z3grid's 16, scan 5
(a)'s 10 or noise_probe's 5. Which rows bind which bounds is T22's to check: rows outside a set's steps, or outside
`PHYSICS.md`'s limits, do not bind.
