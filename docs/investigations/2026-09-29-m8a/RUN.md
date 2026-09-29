# M8a run: the T30 bed at 12 jobs

**M8a PASSED: 23 of 23 checks, gate exit 0.** `report.pass` is true, with 0 failures and no cell needing
an extension. Nobody independent has judged it yet: that is `VERDICT.md`.

## Settings

| | |
|---|---|
| Gate | `tools/gates/m8a.ps1 -BedRoot C:\tmp\nm-m8a-bed -Jobs 12`, through `C:\tmp\nm-m8a-bed\run-gate-j12.ps1` (detached) |
| Build | `da6f15c` on m8a, `git_dirty` false; `simpa.exe` in `C:\tmp\nm-target-m8a` |
| Bed file | `beds/m8a.json`, sha256 `a9af34ae…b7f181` |
| Solvers | `C:\tmp\nm-m8a-solvers`, solver commit `929a5c8`, manifest sha256 `d159c9dc…b05a45`. E1 held for all four exes, and again from the files for all 433 `run.json` |
| Upstream | `B:\repos\I-Simpa-upstream`, for the atmospheric validation |
| Matrix | 40 cells × seeds 1 to 10, dt 1 ms, 6 bands air off (125 Hz to 4 kHz), 7 air on. 32 cells gated (6×10×3 m, 5×4×3 m), 8 reported (20×8×4 m) |
| Machine | GRACE, 28 logical CPUs, 12 SPPS at once (Burhan, 11:29) |
| Output | `C:\tmp\nm-m8a-bed\20260929T093134Z`: 16,902 files, 1.21 GB on NTFS (projected 19,706) |

## Times

| | Clock (+02:00) |
|---|---|
| Gate start | 11:31:11 |
| Bed start | 11:31:34 |
| Last SPPS run written | 16:58:36 |
| Bed end, `report.json` | 17:24:38: 21,181 s, of which 19,621 s solvers, 1,559 s transports |
| Say-NO checks, unit tests, clippy, fmt | 17:24:38 to 18:05:31 |
| Gate end, `=== DONE ===` | 18:05:31 |

**Forecast against realised.** SPEC section 8 forecast 52.7 h of single-process SPPS time.
Measured per run at 12 jobs, the runs took 1.13 to 1.32 times their single-process forecast (median 1.20,
72 runs between 14:48 and 15:48). The first 12 runs took 1.38 to 1.59 times: the M10 and M9 gates ran
beside them from 11:42 to 11:56. Each `spps.exe` had a full logical CPU (CPU seconds equal to wall
seconds), so the slowdown is shared cores, not waiting.

## Gated cells: all 32 pass

Limits: A ±5 % (T30 against Kuttruff's corrected Eyring, 95 % interval); B 2 % (seed spread of the cell
mean); C ±0.5 % (against the independent transport, `|d| + t·SE`); D ±0.5 % (TCR against analytic
Eyring).

| Check | Worst gated cell | Value | Limit | Share of limit |
|---|---|---|---|---|
| A | 5x4x3 α 0.05 random air on, and 5x4x3 α 0.1 random air on | interval edge 1.18 % | 5 % | 24 % |
| B | 5x4x3 α 0.4 random air off | 0.77 % | 2 % | 39 % |
| C | 5x4x3 α 0.05 random air on | d −0.165 %, interval [−0.330, +0.000] % | 0.5 % | 66 % |
| D | 5x4x3 α 0.1 TCR air off | 4.4 × 10⁻⁷ | 0.5 % | 0.0001 % |

Per cell. A is the range of the band means of `T30/kuttruff − 1` and the worst interval edge over bands.
B is the seed spread of the cell mean. C is `d` and its 95 % interval.

| Cell | N | A band means | A worst | B | C |
|---|---|---|---|---|---|
| 6x10x3 α 0.05 random air off | 8.4M | +0.03..+0.45 % | 0.80 % | 0.74 % | +0.009 % [−0.168, +0.186] |
| 6x10x3 α 0.05 energetic air off | 1.5M | +0.18..+0.19 % | 0.21 % | 0.02 % | −0.012 % [−0.048, +0.023] |
| 6x10x3 α 0.1 random air off | 18M | +0.15..+0.46 % | 0.77 % | 0.48 % | −0.086 % [−0.198, +0.026] |
| 6x10x3 α 0.1 energetic air off | 1.5M | +0.31..+0.34 % | 0.37 % | 0.03 % | −0.020 % [−0.062, +0.023] |
| 6x10x3 α 0.2 random air off | 33M | +0.26..+0.65 % | 0.91 % | 0.56 % | +0.043 % [−0.098, +0.183] |
| 6x10x3 α 0.2 energetic air off | 1.5M | +0.39..+0.43 % | 0.48 % | 0.08 % | −0.002 % [−0.044, +0.040] |
| 6x10x3 α 0.4 random air off | 110M | −0.39..−0.05 % | 0.65 % | 0.26 % | +0.047 % [−0.030, +0.125] |
| 6x10x3 α 0.4 energetic air off | 1.5M | −0.30..−0.21 % | 0.40 % | 0.12 % | +0.034 % [−0.027, +0.094] |
| 6x10x3 α 0.05 random air on | 8.4M | −0.01..+0.60 % | 1.05 % | 0.58 % | +0.059 % [−0.094, +0.211] |
| 6x10x3 α 0.05 energetic air on | 1.5M | +0.08..+0.19 % | 0.21 % | 0.02 % | +0.009 % [−0.037, +0.054] |
| 6x10x3 α 0.1 random air on | 18M | +0.10..+0.49 % | 0.75 % | 0.57 % | −0.011 % [−0.155, +0.133] |
| 6x10x3 α 0.1 energetic air on | 1.5M | +0.18..+0.33 % | 0.35 % | 0.05 % | −0.010 % [−0.047, +0.027] |
| 6x10x3 α 0.2 random air on | 33M | +0.20..+0.72 % | 1.09 % | 0.47 % | +0.102 % [−0.015, +0.220] |
| 6x10x3 α 0.2 energetic air on | 1.5M | +0.28..+0.45 % | 0.47 % | 0.05 % | +0.004 % [−0.039, +0.047] |
| 6x10x3 α 0.4 random air on | 110M | −0.41..−0.12 % | 0.74 % | 0.55 % | +0.019 % [−0.119, +0.158] |
| 6x10x3 α 0.4 energetic air on | 1.5M | −0.30..−0.21 % | 0.40 % | 0.12 % | −0.015 % [−0.075, +0.044] |
| 5x4x3 α 0.05 random air off | 3.5M | −0.14..+0.32 % | 0.71 % | 0.62 % | −0.011 % [−0.144, +0.121] |
| 5x4x3 α 0.05 energetic air off | 1.5M | +0.17..+0.18 % | 0.20 % | 0.02 % | +0.000 % [−0.023, +0.022] |
| 5x4x3 α 0.1 random air off | 7.9M | +0.20..+0.52 % | 0.85 % | 0.56 % | +0.039 % [−0.077, +0.155] |
| 5x4x3 α 0.1 energetic air off | 1.5M | +0.30..+0.32 % | 0.33 % | 0.04 % | +0.005 % [−0.024, +0.035] |
| 5x4x3 α 0.2 random air off | 14M | +0.14..+0.48 % | 0.89 % | 0.47 % | −0.029 % [−0.119, +0.062] |
| 5x4x3 α 0.2 energetic air off | 1.5M | +0.33..+0.36 % | 0.39 % | 0.06 % | −0.008 % [−0.040, +0.024] |
| 5x4x3 α 0.4 random air off | 31M | −0.76..−0.37 % | 1.09 % | 0.77 % | +0.003 % [−0.195, +0.201] |
| 5x4x3 α 0.4 energetic air off | 1.5M | −0.62..−0.55 % | 0.67 % | 0.08 % | +0.001 % [−0.033, +0.035] |
| 5x4x3 α 0.05 random air on | 3.5M | −0.23..+0.53 % | 1.18 % | 0.54 % | −0.165 % [−0.330, +0.000] |
| 5x4x3 α 0.05 energetic air on | 1.5M | +0.07..+0.18 % | 0.19 % | 0.03 % | −0.014 % [−0.036, +0.008] |
| 5x4x3 α 0.1 random air on | 7.9M | −0.02..+0.64 % | 1.18 % | 0.69 % | −0.095 % [−0.255, +0.065] |
| 5x4x3 α 0.1 energetic air on | 1.5M | +0.18..+0.30 % | 0.32 % | 0.02 % | −0.005 % [−0.027, +0.018] |
| 5x4x3 α 0.2 random air on | 14M | +0.14..+0.45 % | 0.83 % | 0.64 % | −0.044 % [−0.204, +0.116] |
| 5x4x3 α 0.2 energetic air on | 1.5M | +0.27..+0.36 % | 0.39 % | 0.04 % | +0.004 % [−0.021, +0.029] |
| 5x4x3 α 0.4 random air on | 31M | −0.56..−0.41 % | 0.86 % | 0.69 % | +0.086 % [−0.078, +0.251] |
| 5x4x3 α 0.4 energetic air on | 1.5M | −0.62..−0.52 % | 0.66 % | 0.08 % | +0.012 % [−0.024, +0.047] |

**A pattern worth the judge's eye.** A's band means are positive at α 0.05 to 0.2 (up to +0.72 %) and
negative at α 0.4 (down to −0.76 %), in both rooms and both methods. C is centred on zero in the same
cells. A compares SPPS with Kuttruff's formula and C compares SPPS with the transport, so the offset lies
between the formula and the transport, not in SPPS. E5 measured that gap at up to 0.6 % before the bed
ran.

**Random against energetic** (reported): the 16 gated pairs differ by −0.151 to +0.099 %, and every 95 %
interval contains 0.

**TCR:** all 20 runs OK. The largest `|deviation|` over the 16 gated runs and every band is 4.4 × 10⁻⁷.

## Reported cells: 20×8×4 m, not gated

- **Energetic, all four α:** within every gated limit. A band means −1.10 to +0.26 %, worst edge 1.24 %;
  B up to 0.24 %; C up to +0.172 % at the interval edge.
- **Random at 1.5M particles:**
  - **α 0.05 and 0.1:** not judged. E6: T30 refused `missing_moves` in 3 seed-receiver-bands of each.
  - **α 0.4:** not judged. E6: T30 refused `range_not_reached` in 26 seed-bands, all on receiver R002.
  - **α 0.2:** would fail A at 500 Hz (+2.115 %, interval [−1.049, +5.279] %, over the ±5 % limit) and B
    (spread 5.40 % against 2 %). C is inconclusive: +0.745 % [−0.581, +2.071] %.

The SPEC reports this room rather than gating it, so the pass does not depend on it. The finding stands:
random mode at 1.5M particles in a 640 m³ room is refused outright in three of four absorptions, and in the
fourth it has a 5.4 % seed spread. **Open question for M8b and M11:** what method and particle count does
the app default to, and does a user in a large room see a refusal or a noisy number?

## Upstream's atmospheric validation (reported, not like for like)

10 seeds, all OK, 27 bands from 50 Hz to 20 kHz. Upstream made its values with seed 0, dt 10 ms and
ε 6; ours use seeds 1 to 10, dt 1 ms and ε 7.

- Against upstream's SPPS 2018 values: −0.073 to +0.259 %.
- Against upstream's SPPS 1.3.4 values: −0.143 to +0.128 %.
- Against the MD octave values: −2.666 to −0.489 %.
- TCR against upstream's workbook: 9.5 × 10⁻⁸ at most.

## The gate can say no

All passed:

- **N1** (a byte-flipped spps.exe is refused before any run, exit 2, nothing written).
- **N2** (Kuttruff with its ½ dropped fails A in every α 0.4 gated cell: −9.2 to −11.3 %).
- **N3** (plain Eyring as A's reference fails A in every α 0.4 gated cell: +8.2 to +11.0 %).
- **N4** (a transport reflecting evenly over the hemisphere fails C in every gated cell).
- **N5** (seed 10 at 31,000 particles: the cell is not judged, E6).
- **N6** (walls of scattering 0 are refused, E3).
- **N7** (the transport without air fails C in every air-on gated cell).
- **N8** (TCR with the physical K fails D in every band, +1.230 %).

Preconditions E1 to E7 hold. 68 transports, 0 errors.

## Not in this run

- **The abandoned start.** `C:\tmp\nm-m8a-bed\20260929T085222Z` was a 4-job start at 10:52, stopped on
  purpose at 11:29 to restart at 12 jobs. It holds 4 `run.json` files. It is not a result, and nothing
  here reads it.
- **The hanging test.** `mesh_project::every_failure_code_fires_on_its_input` is not exercised: this
  gate's unit-test step is `cargo test -p simpa-core --lib bed::`. It remains open.
- **What this commits.** `beds/m8a-20260929T093134Z/`: `report.json`, `summary.json`, `summary.png`, and
  the gate log as `gate-j12.txt` here. The decays (40 `.npz`, 105 MB) and per-cell plots (6.2 MB) stay in
  `C:\tmp\nm-m8a-bed\20260929T093134Z\{decays,plots}` for the judge.
