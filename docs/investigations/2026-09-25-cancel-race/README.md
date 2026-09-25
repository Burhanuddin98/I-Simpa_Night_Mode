# The cancelled-run test's race (pre-M8 piece C, after its review)

`cli_results::a_cancelled_run_is_refused_with_exit_5` needs a run that was cancelled while SPPS
ran. The pre-M8 piece B verifier found it flaky: with other SPPS runs of the same test binary
alongside, the run came back OK (exit 0), not cancelled. Piece C moved the cancel from a timer to
SPPS's own first progress line at or above 1 % (`--cancel-after-progress 1`) and asserted that
the run stopped at the `solve` stage. Its receipt, the test and the old design each run 20 times
beside 6 and then 32 concurrent SPPS runs, was not discriminating: the old design was cancelled
20 times out of 20 as well, so the original failure was never reproduced (the review of piece C).

This measurement reproduces the failure on purpose. A cancel held up by load is a late cancel, so
the cancel is made late by a known amount, and the margin each design leaves is measured: how
late its cancel may land and still find SPPS running.

- Script: `cancel_race.ps1` (one SPPS run at a time). Log: `cancel_race_log.txt` (the script
  writes it as `cancel_race.log`, a name `.gitignore` drops).
- Build: debug `simpa.exe` of the pre-m8 tree at `8a558ee` with piece C's fixes in progress, the
  solvers of `solvers/manifest.json`, Grace, 2026-09-25 20:08, nothing else running.
- Project: `tests/fixtures/rooms/seats_box.simpa`, 2,000 particles per source as committed, and a
  copy at 1,000,000.

| Run | Cancel | Exit | Verdict | SPPS ran |
|---|---|---|---|---|
| 2,000 particles | none | 0 | OK | 218 ms |
| 2,000 particles | timer 1 ms (the first design), 3 times | 130 | CANCELLED | 2-3 ms |
| 2,000 particles | timer 50, 100, 150, 200 ms | 130 | CANCELLED | 3, 59, 169, 211 ms |
| 2,000 particles | timer 300, 400, 600, 1000 ms | **0** | **OK** | 192-218 ms |
| 1,000,000 particles | SPPS's 1 % line (the design now), 3 times | 130 | CANCELLED | 128-143 ms |
| 1,000,000 particles | timer 150 ms (the second design, `812853c`) | 130 | CANCELLED | 87 ms |
| 1,000,000 particles | timer 5,000 ms | 130 | CANCELLED | 4,937 ms |
| 1,000,000 particles | timer 10,000 ms | 0 | OK | 6,033 ms (its whole run) |

What it shows:

- **The failure, reproduced.** At 2,000 particles SPPS's whole run takes about 0.2 s. A cancel that
  lands 0.3 s or more after the launch finds it finished, and the run comes back OK, exit 0: the
  verifier's failure. A 1 ms timer thread held up by about 0.2 s by other SPPS runs gives exactly
  that. The first design's margin is about 0.2 s.
- **The margin now.** At 1,000,000 particles SPPS runs about 6.0 s. Its 1 % line sets the cancel
  off, and the solver was stopped 128 to 143 ms into the run, so a cancel would have to be held
  up by about 5.9 s after that line to miss: about 30 times the first design's margin. The
  second design (a 150 ms timer on the same run) had about the same margin, 5.9 s; what the
  progress trigger adds is that the cancel cannot fire before SPPS runs, since it waits for a
  line SPPS printed, which the test now asserts (`stage` is `solve`), and that it keeps its place
  in SPPS's run however slowly the loaded machine runs it.
- **What it does not show.** A hold-up of 5.9 s was not tried and is not ruled out. The
  concurrent loads the builder ran (6 and 32 SPPS runs) never held the first design's cancel up
  by 0.2 s.
