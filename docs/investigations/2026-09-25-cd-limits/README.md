# The C and D truncation limits aligned to Burhan's rule (2026-09-25)

**What changed.** `params::decay::limits::CLARITY_DB` 0.01 → 0.1 dB and `DEFINITION` 0.001 →
0.005 (0.1 → 0.5 points): the most the tail after a series' end, the energy missing from it, or the
arrival inside the onset bin may move C50, C80 or D50 before the value is refused. Burhan confirmed
the rule, 1/10 of a difference limen, on 2026-09-24 14:11; the code had held C and D to gate (a)'s
tighter bounds since the M7 review. The M8 design decision 3 of 2026-09-25 00:20 aligned them
(`session-logs/HANDOFF-2026-09-23.md`; `docs/params.md`, "Truncation").

**What was measured.** Every run already made, read twice with `simpa results --json` in a
release build, before and after the change and with nothing else in the results path changed:

| Build | sha256 of `simpa.exe` |
|---|---|
| before: `812853c` | `85b59a5760e48bc9cbe43dd06baff62596b88349adbccf5dec9568dea9390443` |
| after: the limits changed on `pre-m8` | `0d913886942173ac8dda524274a06b381f6eff9cc68bfdc3aa75d20cca8519d9` |

- `cd_limits.py` reads every run of the noise calibration (rounds 1 to 4) and of tutorial 1 at
  upstream's default, 960 runs in `target/agents/pm8-noise-scratch/runs/` and
  `target/agents/pm8-fix-noise-scratch/runs/`, and writes per receiver-band each of C50, C80, D50,
  Ts, EDT, T20, T30 and SPL: its value, or its refusal's code and why. Its two outputs,
  `cd_before.json` and `cd_after.json` (16 MB each), stay in `target/agents/pm8-safety-scratch/`.
- `cd_compare.py` compares them; `cd_compare.txt` is its output.
- `m8cell_cd.txt`: the one M8 cell whose C80 the old limit refused (6×10×3 m, α 0.05, energetic,
  1.5 M particles, 4 s, three seeds; runs in
  `target/agents/m7fu-fix-critical-scratch/m8-cells-1790266434/024` to `026`).

**What it shows** (`docs/results.md`, "The C and D limits aligned"):
- The M8 cell: C80 through in every seed 5 → 18 of 18 receiver-bands, C50 0 → 18.
- The 960 runs: energetic C50 10,536 → 12,174 of 21,060 receiver-bands, C80 10,880 → 11,704,
  D50 12,867 → 13,008; random C50 7,136 → 7,213 of 13,860, C80 6,937 → 7,018, D50 7,644 → 7,680.
  Tutorial 1 at upstream's default: no change.
- No value given before is refused after; no other quantity changes; the new values carry no
  resolved bias against the other seeds' values of the same receiver-band (−0.002 ± 0.002 dB C50,
  +0.002 ± 0.003 dB C80, −0.03 ± 0.04 points D50).
- On gate (a)'s exact decays with the arrival detected (`params_synthetic.rs`,
  `an_arrival_not_given_leaves_c_d_and_ts_unresolved`): 40 C, D and Ts values at 1 ms where 10 came
  through before, 16 of them outside gate (a)'s bound and inside their limit. Gate (a) itself, with
  the arrival given, holds its 0.01 dB and 0.1 points unchanged.
