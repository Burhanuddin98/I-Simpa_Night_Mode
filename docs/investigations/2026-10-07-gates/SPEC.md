# C4: the m10 and m11 gates on today's `gpu`, before the push (spec, 2026-10-07 15:43)

Burhan, 15:42, verbatim: "sure yes lets go with what you recommended" (the recommendation, 15:37: run the m10 and
m11 gates now, fix anything real, then push `gpu` onto the public `rebuild` branch).

Why: the m10 and m11 gates have not passed since 10-05 22:40 (decision 57, on `b82`). Since then `gpu` took the
10-06 work (time-binned and sparse solver maps, the reader fixes, the response window) and today's: the viewport on
WebGPU (`gfx`), "SPPS on the GPU" in the Simulate step (A5), the live view (B3), the dock (C2), the blank-geometry
tools (C1: scene list rows on two lines, group rename/merge/move, Transmission tab, source emission editor), and the
m12 fixes (C3). Proven today: Rust 103/103 suites, UI 352/352, m12 40/40 + m11.dock 5/5 (run
`C:\tmp\nm-target-dock\gates\m11\20261007-150805`), m13.blank 9/9, a package smoke. NOT proven: the m10 and m11
specs that drive the shell, Geometry, Materials, the viewport, scene, Simulate, Console, Runs, project save/reload,
settings and groups.

## Done when

`tools/gates/m10.ps1` and `tools/gates/m11.ps1` both end with verdict PASS on this branch (the scripts' own verdict
lines, quoted with their run folders), with no spec skipped or weakened. Every failure on the way is classified
and handled:

- **Intended change** (the look or wording changed on purpose today or on 10-06: a pixel pin, a theme.css blob
  hash, a layout count, a string): re-pin, with the reason in the spec's comment and the commit message naming the
  commit that changed it. Never loosen a tolerance to make a pin pass.
- **Real defect** (behaviour that worked on 10-05 and does not now: a refused import that no longer refuses, a
  paste that no longer pastes, a reload that loses data, a run that does not start): fix it in the app, add or
  keep the spec that catches it, and say in the report what was broken and since which commit.
- **Harness or timing** (a flake): show it with two runs, fix the wait in the spec, never by adding sleeps longer
  than the specs already use.

Then run the m12 specs and m13.blank once more on the final tree (the dock builder's list:
`-Spec dock,m12.viewport,acoustics,bedplant,sources,response,plane,mapview,fill,trails,export,mapwindow,m13.blank`)
so the push carries all four green on one tree.

## Constraints

- Worktree `B:\repos\I-Simpa_Night_Mode\.claude\worktrees\gates` (branch `gates`, from `gpu` c86b5c7). Gate target
  dir: `-TargetDir C:\tmp\nm-target` (it holds a release build of c86b5c7 from 15:26). Solvers:
  `-SolversDir C:\tmp\nm-solvers-gpu` (the verified five, spps-gpu 0.2.1 pinned). `SIMPA_TETGEN160` =
  `B:\repos\I-Simpa_Night_Mode\target\solvers\build\src\tetgen\Release\tetgen.exe`; `SIMPA_UPSTREAM` =
  `B:\repos\I-Simpa-upstream`.
- Read `tools/gates/m10.ps1` and `m11.ps1` before the first run: their static phase may run `cargo test`, which
  puts GBs of debug binaries into the target dir. The Rust suites already passed today (103/103 on fa92f42; the
  later merge touched no Rust), so use `-SkipCore` if the script offers it, and say so in the report. Check C: free
  space before each run (61.8 GB at 15:43); stop and report if under 20 GB.
- **Disk rule for today (Burhan 15:27/15:34): regenerable output is deleted, not archived.** When you finish,
  delete your gate scratch and bed staging; keep the gate logs (`gates\m10\<run>\`, `gates\m11\<run>\`: the
  wdio logs and verdict files, screenshots of failures) as receipts.
- The gates open unfocused windows on Burhan's screen; that is agreed. Never click in or drive an app you did not
  start; your DevTools port for any bed of your own: 9291, check the pid.
- Commit on `gates` as you go: messages on the why, no Claude attribution, no Co-Authored-By. Do not push.
  `npm run typecheck` and `npm test` pass at the end.

## Report

`docs/investigations/2026-10-07-gates/REPORT.md` on `gates`, committed (if the harness refuses your write of the
.md, put the full report in your final message and the main thread writes it). Lead with the four verdicts and
their run folders; then one row per failure: spec and check id, message, class, what was done, commit.
