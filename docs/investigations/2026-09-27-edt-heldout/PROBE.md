# M8b EDT harness: the timing probes (plan section 6, step 5)

2026-10-01, branch `m8b-edt` at `f9f85c1`, on Grace (i7-14700KF, 32 GB). Five runs went through the driver's
probe mode (`harness/m8b/driver.py`), one at a time, with no other heavy process on the machine: P0 (section 7)
from 06:39:22 to 06:44:08, then P0b's four runs (section 7.1) from 06:45:13 to 06:45:45. P0 and P0b are
stand-ins, never scored and not held out. `simpa results` was not run, no file inside a run folder was opened,
and everything the runs wrote is on C:, under `C:\tmp\m8b-edt\probe\`. MB and GB here are 10⁶ and 10⁹ bytes.

**In plain words.** One run of the size the truth needs took 4 min 46 s on one core: 1M particles per band at
0.1 ms for 6.5 s, in a stand-in for F2, the room with the longest reverberation. It peaked at 116 MB of memory
and wrote 182 MB. If F2 is the slowest room and the stand-in times as F2 does, the 28 truth runs need at most
28 × 286 s = 2 h 13 min of single-core time. Neither condition has been measured. In the dead stand-in for F7,
a 10 s run took about the same time as a 2 s run (within 3 %) in both modes, but it wrote 4.8 times the output
and peaked at 2.4 times the memory. Energetic mode took 9 times as long as Random there. Two things are
Burhan's to decide:

- **The default run length (P10).** A fixed length (5.0 s was proposed), or a length set from each room's
  predicted reverberation time. The plan takes a fixed length if a longer run costs a dead room little. On
  time, this probe meets that. Disk and memory grow with the length.
- **The long stretch:** the 28 truth runs and the 144 tested runs, with the time above. Whatever he decides,
  they also wait for backlog 54's CLI half (P14, section 8.1).

Energetic mode (P11) is already decided. Its cost against Random is given here for the planning.

## What ran

| | P0 (section 7) | P0b (section 7.1), four runs |
|---|---|---|
| Room | box 18.9 × 9.5 × 9.46 m: V 1,698.5 m³, S 896.4 m², α 0.08, Lambert 1. F2 is 17 × 12.5 × 8 m, 1,700.0 m³, 897.0 m² | box 2.7 × 4.12 × 2.7 m: V 30.03 m³, S 59.08 m², α 0.45, Lambert 1. F7 is 3.8 × 3.3 × 2.4 m, 30.10 m³, 59.16 m² |
| Design T60, 125 Hz to 4 kHz (P5) | 3.63-2.26 s (3.46 s at 500 Hz), F2's to 0.001 s | 0.137-0.134 s, F7's to 0.001 s |
| Source and 8 receivers | F2's, scaled per axis (3 near, 3 mid, 2 far) | F7's, scaled per axis |
| Method | Random | Random, then Energetic |
| Particles per source per band | 1,000,000 | 150,000 |
| Step and length | 0.1 ms, 6.5 s (65,000 steps) | 1 ms; 2 s and 10 s each way |
| Seed | 9999, reserved (one thread) | 9998, reserved (one thread) |

The settings every run shares: octave bands 125 Hz to 4 kHz, air absorption on (ISO 9613-1, 20 °C, 50 % RH,
101,325 Pa), R 0.31 m, trans_epsilon 5, transmission on, 0 particles saved, intersection files off, no
per-source echogram, no fittings, no surface receivers, one source. They were read back from each project the
driver wrote. The room projects `P0.simpa` and `P0b.simpa` are, byte for byte, the ones that step 4 meshed and
verified in `C:\tmp\m8b-edt\rooms-736ce25\` (`simpa mesh` 0, `simpa mesh-verify` 0).

The command, for each run, was the driver's:
`simpa run <name>-probe.simpa --solver spps --runs C:\tmp\m8b-edt\probe --json`, with `--solver-exe`, `--tetgen`
and `--preprocess` pointing at the checked folder. stderr was kept beside the run folder.

**The check before each run (P14).** Before writing anything, the driver checked `C:\tmp\nm-m8a-solvers`
against `solvers/manifest.json` (sha256 `8e45168f…`) by code sha256, as M8a's E1 does. All four executables
matched, in each of the five runs:

| exe | code sha256, as the manifest | file sha256 |
|---|---|---|
| spps.exe | `550485c6…` | `1d9900db…` |
| classicalTheory.exe | `fad4ab5d…` | `c4a7bca3…` |
| tetgen.exe | `d0704003…` | `ac68f014…` |
| preprocess.exe | `7d4ba6af…` | `34d9b874…` |

simpa.exe is `C:\tmp\nm-target\release\simpa.exe`, sha256 `cfe8caff…`, the build that `harness/SETUP.md`
records. Nothing under `crates/` or `solvers/`, and neither `Cargo.toml` nor `Cargo.lock`, has changed between
its source, `610e691`, and `f9f85c1`. Each run's run.json names `C:\tmp\nm-m8a-solvers\spps.exe` with sha256 `1d9900db…`, which is the
file the check read. P14's report check had nothing to read, since no report was made (section 7).

## P0 (section 7)

| Recorded | Value |
|---|---|
| Wall time (the driver's, the whole `simpa run`) | 285.9 s (285.907), 06:39:22.131 to 06:44:08.039 |
| run.json `outcome.elapsed_ms` (the solver process alone) | 285,812 ms. Exit code 0, not cancelled. The 0.095 s left of the wall time is everything around the solver: the process starts, simpa's export, TetGen and the verdict |
| Verdict | OK, with no reasons and no warnings. Exit class 0 |
| spps.exe peak working set (polled) | 116,232,192 bytes (116.2 MB); peak private bytes 111,677,440. 144 polls, then read once more after exit |
| Output size (a stat of each file) | 181,876,014 bytes in 57 files |
| stderr by class | PROGRESS 9,803, INFO 2, OK 1, WARN 0, FAIL 0, simpa 0, other 0. The run manifest's counts agree, with 0 unclassified |
| Files the verdict expects | 40 of 40 present, 43 in all |
| C: free | 21.54 GB before, 21.36 GB after |

The output by part, from the directory listing (names and sizes only):

| Part | Files | Bytes |
|---|---|---|
| `solve/Punctual receivers/R000` to `R007` | 32 | 139,490,216. Each receiver: `Punctual receiver intensity.gabe` 7,935,535, `Sound level.recp` 4,812,005, `Advanced sound level.gap` 4,686,658, `Sound level per source.recps` 2,079 |
| `solve/Intensity animation/<band>/Intensity.rpi` | 6 | 37,485,678 (6,247,613 each) |
| `solve/Total energy.recp` | 1 | 4,812,005 |
| `solve/`, the rest (`config.xml`, the mesh copies, `SPPS particle statistics.gabe`) | 4 | 10,138 |
| `mesh/` | 11 | 8,010 |
| `run.json`, `solver.stdout.txt`, `solver.stderr.txt` (empty) | 3 | 69,967 |

## P0b (section 7.1)

| Run | Mode | Length | Wall time | `elapsed_ms` | Output | spps.exe peak working set |
|---|---|---|---|---|---|---|
| P0b-random-2s | Random | 2 s | 1.011 s | 921.9 | 5,855,587 bytes | 9,654,272 bytes |
| P0b-random-10s | Random | 10 s | 1.033 s | 949.9 | 28,207,580 bytes | 22,962,176 bytes |
| P0b-energetic-2s | Energetic | 2 s | 8.492 s | 8,411.1 | 5,855,582 bytes | 9,658,368 bytes |
| P0b-energetic-10s | Energetic | 10 s | 8.485 s | 8,405.8 | 28,207,573 bytes | 22,958,080 bytes |

They ran from 06:45:13.582 to 06:45:14.594, 06:45:19.766 to 06:45:20.800, 06:45:25.142 to 06:45:33.635 and
06:45:36.955 to 06:45:45.441. Each one exited 0, with exit class 0 and verdict OK, no reasons and no warnings.
Each wrote 57 files, with 40 of the 40 the verdict expects. On stderr each had PROGRESS 9,999, INFO 2, OK 1,
and no WARN, FAIL, simpa or other lines.

- **10 s against 2 s.** Random took 28.0 ms more solver time (+3.0 %). Energetic took 5.3 ms less (−0.06 %).
  In both modes the output grew 4.82 times and the peak working set 2.38 times.
- **Energetic against Random.** Energetic took 9.12 times Random's time at 2 s, and 8.85 times at 10 s.
- **The output follows the number of steps.** All five runs wrote 2.8-2.9 kB per time step: P0 wrote 181.9 MB
  over 65,000 steps, and P0b 5.86 MB over 2,000 steps and 28.2 MB over 10,000.
- **These are single runs.** The run-to-run spread was not measured, so the 3 % difference in a 1 s run is not
  shown to be real.

## What it gives Burhan (sections 7 and 7.1)

- **The long stretch.** One run with F2's cost drivers took 285.9 s on one core and peaked at 116 MB. At 06:37,
  just before P0, 19.7 GiB of the machine's 31.8 GiB of RAM was free. If F2 is the slowest room and P0 times as F2
  does, 28 × 285.9 s = 8,005 s (2 h 13 min) bounds the single-thread truth time. Neither condition is measured,
  and this record makes no projection beyond that arithmetic (section 7). The truth and tested runs also wait
  for backlog 54's CLI half (P14).
- **P10, the run-length rule.** P10 offers two rules. A fixed default (5.0 s was proposed) applies if a longer
  run costs a dead room little. A default set from the room's predicted reverberation time applies if it does
  not. In P0b, a run five times as long cost no measurable time in either mode, but it wrote 4.8 times the
  output and peaked at 2.4 times the memory.
- **P11, Energetic mode.** This is already decided. On P0b, Energetic took 9.1 times Random's time. M8a measured
  18 times on one of its rooms (`HANDOFF-2026-10-01.md:59-60`).

## Kept apart, and nothing leaked (section 5, items 4 and 5)

**Before P0**
- T10, T11 and T13 passed at 06:36: 3 passed in 3.8 s (`C:\tmp\m8b-edt\step5\step5-precheck-pytest.txt`). They
  ran on `f9f85c1`, which differs from step 4's `736ce25` only in HARNESS-PLAN.md's text.
- Step 4 meshed and verified the nine projects at 06:30, from `736ce25`.
- The launch log did not exist, and nothing existed under `C:\tmp\m8b-edt\heldout\`.
- No other heavy process was running: no cargo, rustc, simpa, spps, TetGen, node build or Python.

**After P0b**
- `C:\tmp\m8b-edt\launch.log` holds 5 lines, one per probe (sha256 `6a978e12…`). Each line is `"what": "probe"`,
  with its seed (9999 or 9998) and the runs root `C:\tmp\m8b-edt\probe`.
- `C:\tmp\m8b-edt\heldout\` does not exist.
- On B:, the worktree holds 1,213 files outside `harness/` and 34 inside, as it did before the probes, and
  `git status --ignored` lists nothing. The driver ran with its working folder on C: (`C:\tmp\m8b-edt`) and
  `PYTHONDONTWRITEBYTECODE=1`.
- C: has 21.29 GB free, against the 8 GB floor. The probe folder holds 250.9 MB, and C:'s free space fell by
  252.8 MB over the five runs.
- PREREG.md and `frozen/` are unchanged, and `frozen/method.py` still hashes `462c37cf…`.

## Records

Everything is on C:. Only this file is committed.
- `C:\tmp\m8b-edt\probe\probes.jsonl` holds the driver's five records (sha256 `a48a376b…`). Every number above
  is taken from it, except the output by part, which comes from the directory listing.
- The run folders under `C:\tmp\m8b-edt\probe\` hold 250.0 MB in all:
  - P0: `20261001-063922-138-spps`;
  - P0b: `20261001-064513-592-spps`, `-064519-775-spps`, `-064525-152-spps` and `-064536-962-spps`, in the
    table's order.

  They are listed in `session-logs/TO-DELETE-2026-10-01.md` for deletion after 07:00.
- Beside the run folders are each probe's project, simpa's stdout (the run manifest) and its stderr, with the
  two room projects under `rooms\`.
- `C:\tmp\m8b-edt\step5\` holds the launchers (`run-probe.cmd`, `launch-and-wait.ps1`), each probe's driver
  output, the solver check and the T10, T11 and T13 run.

## Notes

- The probes were started detached, through `run-probe.cmd`, so that the tool's shell could not cut a long
  run off. Each ran alone, one after the other. Each start and end went to the progress log with its wall time.
- During P0, SPPS's progress percentage was read twice from the PROGRESS lines of the kept stderr, to follow the
  run. Those lines have the form `#44.2`. No file inside a run folder was opened at any time.
- The memory poll did not catch `tetgen.exe` in any run. It polls every 0.1 s until spps.exe appears, so
  TetGen's peak is not recorded.
- Step 4's hand-over ran the driver from `harness/`. Here it ran from `C:\tmp\m8b-edt`, with the harness on
  `PYTHONPATH`. The driver finds the repo from its own path, so the run is the same.
