# A5: "SPPS on the GPU" in the app (spec, 2026-10-07 09:05)

PLAN.md's A5, verbatim: "In the app. The Simulate step's solver choice gains "SPPS on the GPU" when a CUDA
device is found (the app asks `spps-gpu --probe`), with the device named; refused, with the reason, when not."
Done when: "The app runs CR4 on the GPU and shows the results; the run says which solver made it."

Burhan's order for the day, 2026-10-07 08:57: "okay proceed with da work" (the 21:50 NEXT list in
`session-logs/HANDOFF-2026-10-06.md` on `glass`: merge, e2e, A5, the Zeph build, A4's missing cases, B3).

## The design (decided 09:03; log it as decision 70 in `docs/decision-log.md`)

- **`SolverKind` stays `{Spps, Tcr}`.** spps-gpu is a drop-in for SPPS: same `config.xml`, same output files
  (A2-REPORT.md). Every spps-keyed path (verdict.rs, expect.rs, results.rs `spps::read`, validate/export.rs,
  runSize.ts, the acoustics fallbacks) must keep treating a GPU run as SPPS, unchanged.
- **The device is a run option, not project data.** A new `SppsDevice { Cpu, Gpu }` (name it to the code's
  idiom) passed with the run request; CPU is the default and everything that exists today is CPU. The project
  file and its schema do not change (a project made on Grace must open on a machine without CUDA).
- **Exe:** device GPU runs `spps-gpu.exe`, found by the same `ExeSearch` as the others, and checked against
  `solvers/manifest.json` like every solver (`SOLVER_UNVERIFIED` when it does not match).
- **Manifest row:** `spps-gpu.exe` in `solvers/manifest.json` (`sha256` and `code_sha256`, the latter from
  `solvers/pe-fingerprint.ps1`, the way `solvers/build.ps1` makes the others) for the **fat** build at
  `C:\tmp\nm-spps-gpu\fat\spps-gpu.exe` (sm_75 + sm_120; built 09:03 by `solvers/spps-gpu/build.cmd
  C:\tmp\nm-spps-gpu\fat fat`; check it exists and `--probe` exits 0 before you pin it). Record the spps-gpu
  source commit the way the manifest records upstream's. Whatever pins the manifest's own hash (tests,
  `SolverManifestRecord`, the embedded copy at `bed.rs:32`) is updated with it, with the reason.
- **run.json:** `RunManifest` (`deny_unknown_fields`) gains one optional field recording the device: absent
  for CPU runs, so every existing manifest and fixture stays byte for byte what it was; for GPU runs it holds
  the device line `--probe` printed (e.g. "NVIDIA GeForce RTX 5070, sm_120, 48 SMs, 11.9 GiB, driver CUDA
  13.2, runtime 13.2"). The `exe` field already names spps-gpu.exe and its hash. If `docs/formats/` or
  `docs/m5-m6-design.md` describes run.json, update the description in the same commit.
- **Probe:** a backend command that runs `spps-gpu.exe --probe` (found by `ExeSearch`), with a timeout of a
  few seconds, once per app session (cached), returning available + the device line, or unavailable + the
  reason (exe not found; exit 1 with its output; a timeout; not the verified build). No other code runs it.
- **UI:** the Simulate step's solver choice (`SimulatePanel.tsx` `SOLVERS`) offers SPPS, "SPPS on the GPU",
  TCR. The GPU entry names the device when available; when not, it is shown disabled with the reason, never
  hidden. The choice is carried through ipc/store to the run request. The Results step and the acoustics
  report say which solver made the run ("SPPS on the GPU, <device>"), read from run.json (`results.rs:483
  solver_build` is the existing reader). Match the panel's existing look; no new design language.
- **Refusals:** spps-gpu exits 2 for fittings, stratified air and measured directivity balloons, with a
  reason on stderr. The run's verdict and the app must show that reason to the user (check the existing path
  reads solver stderr; if it does not, make it). A pre-flight that greys the GPU entry for such a project is
  welcome if it is small and exact; the exit-2 path must work regardless.
- **CLI:** `simpa run ... --solver spps --device gpu` (or the CLI's idiom), so beds can run headless.

## Bed (the done-when, with receipts)

1. `simpa run` on CR4 with `--device gpu`: verdict OK, results load (`simpa results`), run.json names
   spps-gpu.exe, its hash and the device. Compare its point-receiver T20/EDT at MP1-MP5 against a CPU run of
   the same project and particles; within the noise is the expectation (A2-REPORT.md's table is the
   precedent). Use a copy of a CR4 project under `B:\repos\I-Simpa_Night_Mode\.out\a5\` sized to finish in
   minutes on SPPS (A2's 1 kHz 2 x 300 k 10 s case is a good size; `solvers/spps-gpu/bed/stage.py` built it).
2. The app (release build) on that project: the GPU entry shows the RTX 5070; a run started from the app on
   the GPU completes; the Results step shows the results and names the solver and device. Drive it headless
   with `tools/devtools/` (README there: `--e2e`, the DevTools port, `window.__m10.setStep`); screenshots to
   `.out\a5\`.
3. The refusal: a project with a fitting (or balloon) on the GPU shows spps-gpu's reason in the app.
4. The unavailable path: point the probe at a missing exe (or force exit 1) and show the disabled entry with
   its reason (unit test is enough).

## Constraints

- Work only in the worktree `B:\repos\I-Simpa_Night_Mode\.claude\worktrees\a5` (branch `a5`, from `gpu`
  11347e6). Outputs only to `B:\repos\I-Simpa_Night_Mode\.out\a5\`, `C:\tmp\nm-target-a5` (the cargo target
  for everything you build or test) and `C:\tmp\nm-solvers-a5\` (a solvers folder: copy
  `C:\tmp\nm-solvers-timebin\bin\*` and the fat `spps-gpu.exe` into it; launch everything with
  `SIMPA_SOLVERS_DIR=C:\tmp\nm-solvers-a5`). Do not touch `C:\tmp\nm-target`, `C:\tmp\nm-solvers-timebin`,
  other worktrees, or any app.exe you did not start.
- **Disk:** debug test binaries filled C: twice on 10-06. Before every `cargo test`, check C: free space; stop
  and report if it is under 20 GB. Run the test targets you touched, not the whole workspace blindly; at the
  end run the simpa-core, simpa and app crates' suites once.
- Build the app with `npx --no-install tauri build --no-bundle` in `app/` and `CARGO_TARGET_DIR=C:/tmp/nm-target-a5`
  (`cargo build --release -p app` makes a dev-mode binary that shows "localhost refused to connect").
  A running app.exe locks its file: stop yours before a rebuild.
- Do NOT run the m10/m11/m12 e2e gates (they open windows on Burhan's screen; the main thread runs them).
- A script that copies a solve folder rewrites the ROOT element's `workingdirectory` in config.xml and refuses
  to start unless it names the copy (the 10-06 19:30 incident overwrote a real run this way).
- Never delete anything outside your own outputs. Read the solver's stderr on every run.
- Commit on `a5` as you go: messages say why, no Claude attribution, no Co-Authored-By line. Do not push.
- UI: `npm run typecheck` and `npm test` in `app/` pass at the end; Rust suites as above.

## Report

Write `docs/investigations/2026-10-06-gpu/A5-REPORT.md` on `a5` and commit it: what was built (files),
the decisions you had to make beyond this spec, each bed item with its receipt (paths, numbers labelled
REALISED with their configuration), the test counts, what is untested, and anything you found broken on the
way. Lead with whether the done-when holds.
