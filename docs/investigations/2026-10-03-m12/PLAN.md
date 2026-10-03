# M12 plan: Concept B, Results (2026-10-03)

Spec verbatim: `docs/rebuild-plan-raw-2026-09-23.json:217-225` (deliverable and gate (a)-(f)). Rulings that add to it,
with receipts: `KEEL.md` beside this file. Code coordinates: `SONAR.md`. Branch `m12` off `rebuild`.

## What "done" means (the load-bearing question, KEEL 4)

The Results screen shows a number only when its parameter's bed passed, and then always with what the report says
about it: its range (`lo`/`hi`) and `status` (`ok`/`wide`), its refusal when refused, and the two EDT marks of row
37 (1)-(2) wherever EDT appears. Every number on screen equals `simpa results <run> --json` at the displayed
precision. No screen text says "validated".

## Packages (in order; P2 and P3 in parallel after P1, separate worktrees)

**P1, core and contracts** (worktree `m12`):
1. **`beds/summary.json`, generated, never hand-written.** A script under `tools/` reads the bed artifacts the
   result documents name as final (M8a: `beds/m8a-20260929T093134Z/summary.json`; M8b: the summaries under
   `B:\data\m8b-bed\` that `docs/investigations/2026-10-02-bed/RESULT.md` cites, including STI) and writes, per
   parameter: `status` PASS/FAIL, the bed, the RESULT document and section, each artifact's path and sha256, the git
   commit the bed ran at, and notes (marks, `wide` rules). The status rule is decision 39's product grade, read from
   the artifacts: no wrong-silent value at the defaults. A test re-runs the script and checks its output is
   byte-identical to the committed file, so the file cannot drift from its evidence. If the artifacts do not let a
   parameter's status be derived mechanically, the script refuses that parameter by name (FAIL, with the reason).
2. Report: `validated_by_bed` (hard-coded false, `report.rs:1947`) reads the summary; `cli_results.rs:250` updated.
   Backlog 47 (version rule) closed here if cheap, else left.
3. IPC: the report's values to the UI (today `run_results` returns no value, `runs.rs:284`); reads for surface maps
   (`formats/csbin.rs`), particles (`formats/pbin.rs`) and echograms (`formats/gabe.rs`) as typed responses.
4. `tools/gates/m12.ps1` skeleton on the m11.ps1 pattern; m10-h and m11-h narrowed to "no solver number outside the
   Results step", with a test that still fails if a number leaks onto another step.

**P2, the Acoustics tab** (worktree `m12-acoustics`, from P1): parameter tables, all receivers in one table (R57,
R65, R12, R14, R17, R18), each value with its range, status and refusal; the EDT marks; RT per band against the
DIN 18041 target band (gate (e): 0.55 s for the 180 m3 teaching room, A3); Sabine/Eyring table (R35); absorption by
surface group; decay charts in uPlot drawn from the same code as the numbers (R8); the A/B variant switch (gate (f)).
The words on screen follow MQ2.

**P3, the viewport** (worktree `m12-viewport`, from P1): surface-receiver maps as a faces x steps data texture, the
black-red-yellow map (R38, R49), band choice (R41), legend (R50), step animation (R39), difference from baseline
(R52); particle playback from GPU-resident `.pbin` on one shared Animator timeline (R53, R55). Test hooks for gate (c)
(3 texels == `.csbin` float32 exactly) and (d) (rendered count == alive in `.pbin` at 5 steps).

**P4, integration and gate:** merge P2 and P3 into `m12`; `m12.ps1` runs (a)-(f) plus m11, m10 and m9; workspace
suite passes (baseline 1086); assay; merge into `rebuild`.

## Product questions (each has a default; the build follows it until Burhan says otherwise)

| # | Question | Default (built) | Why |
|---|---|---|---|
| MQ1 | C50 and D50 failed the bed's stricter exactness bar (99 % within 1/10 of the JND on exact inputs) but no value was ever beyond the JND, and the product now says `wide` at the 50 ms edge. Shown? | **Shown, PASS**, with their ranges | Decision 39's product grade is "no wrong-silent value"; they meet it; the `wide` status is the honest part |
| MQ2 | The words on the Results screen (backlog 74, decision 39) | **"Computed to ISO 3382-1 / IEC 60268-16 and checked against exact solutions to within the just-noticeable difference. Simulated with I-Simpa's solvers; not compared with measured rooms."** Never "validated" | Burhan 10-03 08:31; it says what was checked against what |
| MQ3 | STI has no noise range yet (backlog 71), but row 37 (3) asks every metric for one | **Shown with its value and a visible "noise range not computed" note**; backlog 71 stays v1.1 | It passed its bed (within 0.01 on every receiver); hiding it loses a passed number, a bare value would hide the gap |
| MQ4 | Particle playback when particles saved is 0 (decision 44 (2)) | **Playback shows "No particles saved for this run" and how to turn it on, with the file size**; the gate run sets it above 0 | The 1.1 GB default cost stands |
| MQ5 | The design draws "Export report" and "Listen at R1" | **Not built in v1** (report export is R68, v1.x; listening is auralisation, not in scope) | Not in M12's deliverable |
| MQ6 | Decision rows 20 and 21 disagree on a "not yet validated" label | **No such label**: a parameter without a PASS is not rendered (gate (b), row 21); the only marks are row 37's two EDT marks | Row 21 is the later ruling, and gate (b) |

## Traps (paid for)

As PQ3 and row 15 (`docs/investigations/2026-10-03-pq3/PLAN.md`, `-row15/PLAN.md`): ops as text through
`Op::from_json`; build into `C:\tmp\nm-target-h` (one builder at a time per target dir: P2 and P3 in parallel use
`-h` and `-g`); test env vars; suite timeout 5400000 ms; tauri-driver port 4444 is one e2e at a time (P2 and P3 take
turns); outputs to `B:\data\m12\`; the gate fails on untracked files. `REPORT_VERSION` is 10 (`report.rs:65`), not 8.
Bed set B is a convergence check (backlog 74): the summary says so in its notes.
