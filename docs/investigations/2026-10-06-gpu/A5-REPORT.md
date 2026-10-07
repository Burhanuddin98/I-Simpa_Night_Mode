# A5: SPPS on the GPU, in the app (2026-10-07 09:03-09:40, Grace)

**Done-when holds: yes.** "The app runs CR4 on the GPU and shows the results; the run says which solver
made it." The release `app.exe` ran CR4 (1 kHz, 2 x 300,000 particles, 10 s) from its own Run button on
the RTX 5070: verdict OK, Results step "Results verified", the Results panel and the Acoustics tab read
"SPPS on the GPU, NVIDIA GeForce RTX 5070", and the run's `run.json` names `spps-gpu.exe`, its sha256 and
the device line. The CLI run of the same project agrees with a CPU SPPS run within the Monte-Carlo noise on
every EDT and T20 value (20 of 20), 49 of 50 values in all. All four bed items have receipts below.

**What it means for Burhan.** On a machine with a CUDA device, the Simulate step now has a third choice,
"SPPS on the GPU", naming the card. On CR4 at 1 kHz the solver took 1.22 s against SPPS's 44.0 s. A
machine without one shows the choice greyed with the reason; a project the GPU build does not take (a
fitting, a balloon, stratified air) fails with spps-gpu's own reason shown under the run. Nothing changes
for projects: the device is chosen per run and never saved.

Branch `a5` (from `gpu` 11347e6), commits listed at the end. Not pushed.

## What was built

| file | what |
|---|---|
| `crates/simpa-core/src/run/gpu.rs` (new) | `SppsDevice {Cpu, Gpu}` (default Cpu), `SPPS_GPU_EXE_NAME`, `probe(exe, timeout)` (runs `spps-gpu --probe` through the process layer with a 5 s cancel timer: the device line on exit 0 with one line, otherwise the exit and the executable's own words, a timeout, or a launch failure), `probe_verified(search, manifest, timeout)` (finds it with `ExeSearch`, checks it against the manifest first, never starts an unverified build) |
| `run/manager.rs` | `RunOptions::gpu_device: Option<String>` (the probe's line; `None` = CPU; ignored for TCR) and `on_gpu()`; `run_exe_name(solver, gpu)` (`spps-gpu.exe` for SPPS on the GPU); the `solvers` stage checks the exe under that name; `run.json` records the line |
| `run/manifest.rs` | `RunManifest::gpu_device`, `skip_serializing_if = None`: absent for CPU runs, so every existing manifest and fixture is byte for byte what it was |
| `results.rs` | `solver_build` looks for the check named `run_exe_name(solver, gpu_device.is_some())` |
| `run/classify.rs`, `docs/solver-contract.md` | row 23 of Part B: `spps_gpu_refused`, stderr, `^spps-gpu: refused: [a-z_]+: `, FAIL, so the verdict carries spps-gpu's exit-2 reason as its own reason with the line as detail (before this, the line would have been `unclassified_line`, a WARN, and the verdict only `exit_nonzero`). The contract also names `spps-gpu.exe` in "Solver build" |
| `solvers/manifest.json` | `spps-gpu.exe` in `sha256` (45879035...ec68a1) and `code_sha256` (23107f29...2dc388, by `solvers/pe-fingerprint.ps1`), and a `spps_gpu` record: source commit `8f9f891` (the last commit that changed `solvers/spps-gpu/src`, `build.cmd`, `third_party`), source tree `baaae08`, built from the `gpu` checkout at `11347e6`, build command, nvcc 13.2, architectures sm_75 + compute_75 PTX + sm_120, build log, built 2026-10-07T09:02 |
| `solvers/build.ps1` | `-UpdateCommittedManifest` carries the `spps-gpu.exe` rows and the `spps_gpu` record over (build.ps1 does not build it; without this a rebuild of upstream's four would drop the pin) |
| `tools/gates/m1.ps1` | `Test-Manifest` required exactly 4 `code_sha256` entries; it now takes 4, or 5 when the fifth is a 64-hex `spps-gpu.exe` |
| `crates/simpa/src/mesh_run.rs`, `main.rs` | `simpa run`/`run-folder ... --device cpu\|gpu` (default cpu); gpu with TCR is a usage error; gpu runs `probe_verified` first and is a usage error (exit 2, no run folder) with the reason when there is no device or the build is not the verified one; the device line goes to stderr and into `run.json` |
| `app/src-tauri/src/runs.rs`, `commands.rs`, `main.rs`, `bindings.rs`, `build.rs`, `capabilities/default.json` | `GpuStatus {available, device, reason}`; `GpuCache` in `AppState`: the probe runs once per app session (`spps_gpu_status` command); `run_start(solver, device?)` (absent = cpu, so older callers are unchanged): gpu takes the session's probed exe and line, refused `SPPS_GPU_UNAVAILABLE` with the probe's reason when there is none, and checks the exe against the manifest like the others; `RunStarted.gpu_device`, `RunRow.gpu_device` (from `run.json`); the command is in the ACL |
| `app/ui/src/...` | `deviceStore`, `gpuStatusStore`; `refreshGpu()` at boot; `simulate/model.ts` `solverChoices`, `choiceKey`, `deviceName`, `runSolverText`, `runLabel(run, solver, device)`; `SimulatePanel.tsx` three entries (`data-solver` `spps`, `tcr`, `spps-gpu`); the GPU entry names the device (whole line in its tooltip), or is shown disabled with the reason, never hidden; Run reads "Run SPPS on the GPU"; the last run's `spps_gpu_refused` reason shows spps-gpu's words inline; `ResultsPanel.tsx` and `AcousticsPane.tsx` say "SPPS on the GPU, <device>" from the run's `gpu_device`; `reasonWords` + `reasonCodes.txt` gain `spps_gpu_refused`; test hooks `setDevice`, `gpuStatus`, `runStart(solver, device)` |
| `tools/devtools/bed-a5.py` | the app bed driver (DevTools protocol only: no mouse, no focus); stops only the app it started |
| `docs/decision-log.md` | decision 70 |

## Decisions beyond the spec

1. **The CLI probes too.** The spec says no code but the backend command runs the probe; `simpa run
   --device gpu` needs the device line for `run.json` and has no app session, so it calls the same core
   function (`gpu::probe_verified`) once per invocation. The app's command is still the only caller in the
   app.
2. **`run.json`'s field is `gpu_device`**, and its presence is what makes a run a GPU run for
   `solver_build` (no second field that could disagree with it). `RunOptions` carries the line, not an enum
   plus a line.
3. **The refusal is a classifier row** (`spps_gpu_refused`, FAIL), not a special case in the verdict, so
   the existing path (FAIL lines become reasons with their text) carries it to `run.json`, the CLI, the
   Runs tab and the Simulate step. The contract page's table and its row-count test moved from 22 to 23.
4. **Layout.** SPPS and TCR keep their row; "SPPS on the GPU" takes a full-width second row under them (the
   panel is two columns wide; three in a row truncated the device name). The DOM order is SPPS, TCR, GPU,
   so arrow keys follow what is seen; arrows skip a disabled entry.
5. **Digits on the Results step.** The m11 gate allows digits only inside `[data-run-label]`; a device name
   has them ("RTX 5070"). The solver text is in its own `data-run-label` span (`data-part="run-solver"`),
   after the title, so the gate's first-`[data-run-label]` read is still the title. On the Simulate step the
   device line passes `ACOUSTIC_NUMBER` and `PARAMETER_NUMBER` (checked by hand: "48 SMs", "11.9 GiB",
   "GeForce", "RTX" match neither).
6. **A GPU chosen before the probe answers unavailable falls back to CPU** (`refreshGpu`), so a stale choice
   can never send a run to a missing device.
7. **No pre-flight greying for fittings/balloons/stratified air.** The exit-2 path is exact and works
   (bed 3); a pre-flight would duplicate spps-gpu's refusal rules in the UI.

## The bed

Configuration of every run below (REALISED, Grace, i7-14700KF + RTX 5070, 2026-10-07): the project is
`app/src-tauri/examples/bras_cr4.simpa` with only the 1 kHz band computed and `random_seed` 101
(`.out\a5\cr4-1k\CR4-1k.simpa`, made by a scratch script): 2 sources x 300,000 particles, 10 s at 1 ms,
energetic, receiver radius 0.31 m, the 0.5 m audience cutting plane, its active variant "Absorbing panels".
Solvers: `C:\tmp\nm-solvers-a5` (the timebin build's four + the fat `spps-gpu.exe`), every run with
`SIMPA_SOLVERS_DIR` set to it, every run checked against the embedded manifest (sha256 `2b7b4308...`).
`simpa.exe` and `app.exe` are release builds in `C:\tmp\nm-target-a5`.

### 1. `simpa run --device gpu` on CR4, against a CPU run

| arm | run folder (`.out\a5\cr4-1k\runs\`) | verdict | exe | solver wall | fates (atmosphere / materials / loops / lost of 600,000) |
|---|---|---|---|---|---|
| GPU | `20261007-091702-499-spps` | OK, exit 0, 0 FAIL, 0 WARN, stderr empty | `spps-gpu.exe` 45879035... | **1.219 s** (spps-gpu.json: kernel 0.61, trace 0.72) | 12,860 / 587,033 / 8 / 99 |
| CPU | `20261007-091713-013-spps` | OK, exit 0, stderr empty | `spps.exe` 973059a5... | **43.992 s** | 12,928 / 586,957 / 9 / 106 |

REALISED speed on this configuration: **36x** on the solver process (43.99 / 1.22 s); the whole `simpa run`
command took 1.95 s against 44.3 s (meshing included). `run.json` of the GPU run: `exe` `C:\tmp\nm-solvers-a5\spps-gpu.exe`
with its sha256, `gpu_device` "NVIDIA GeForce RTX 5070, sm_120, 48 SMs, 11.9 GiB, driver CUDA 13.2, runtime
13.2", `solvers` spps-gpu.exe / tetgen.exe / preprocess.exe all matching, `solver_manifest.source`
embedded. `simpa results` on both: exit 0, "solver build verified".

**Within the noise** (A2's test, |a - b| <= sqrt(ha^2 + hb^2), h the half range `simpa results` gives;
`.out\a5\cr4-1k\compare.md` and `.json`), per receiver and source at 1 kHz (the summed receivers' decay
values are `NE(several_sources)` on both arms, as in A2):

| parameter | within / compared | worst \|d\| / range |
|---|---|---|
| EDT | **10 / 10** | 0.47 (MP1/LS2: 1.663 vs 1.582 s) |
| T20 | **10 / 10** | 0.30 (MP4/LS1: 1.663 vs 1.495 s) |
| T30 | 10 / 10 | 0.13 |
| SPL | 10 / 10 | 0.53 |
| C80 | 9 / 10 | **1.05** (MP4/LS1: 4.37 vs 3.08 dB) |

49 of 50 in all. **Control** (CPU seed 101 against CPU seed 202, `.out\a5\cr4-1k-s202\`, run
`20261007-091853-482-spps`, OK): 37 of 37 within, 13 not evaluable on one side (`ctl\compare.json`; its
column headed "GPU" is the seed-101 CPU run). One value out at 1.05 in 50 is what 2.5-sigma ranges give
(about 1 % each). **Caveat:** at 0.31 m receivers T20's ranges are wide (half widths 0.15 to 1.5 s), so
T20 agreeing is a weak test here; EDT's are 0.10 to 0.19 s. A2's 1 m-radius CR4 comparison remains the
strong one.

### 2. The app on that project, run on the GPU from the Run button

`python tools/devtools/bed-a5.py C:\tmp\nm-target-a5\release\app.exe C:\tmp\nm-solvers-a5 <CR4-1k.simpa>
<box_fitting.simpa> .out\a5` (receipt `.out\a5\app-bed.json`, log `app-bed.log`; the app was built at
09:27 from `bba993a`'s tree; DevTools port 9231):
- **GPU entry** (`a5-simulate-gpu-entry.png`): `gpuStatus` available, device "NVIDIA GeForce RTX 5070, sm_120,
  48 SMs, 11.9 GiB, driver CUDA 13.2, runtime 13.2"; the entry reads "SPPS on the GPU / NVIDIA GeForce RTX
  5070", enabled, with the whole line as its tooltip.
- **Chosen** (`a5-simulate-gpu-chosen.png`): both Run buttons read "Run SPPS on the GPU", no blockers.
- **Run** (the panel's Run button, a DOM click): run `.out\a5\app-cr4\runs\20261007-092830-067-spps` (Run 2),
  ended 1.65 s after the click; the Runs row: OK, `gpu_device` the line above, exe `spps-gpu.exe`
  45879035..., solver wall 1.3 s, solver build verified; `run.json` checks spps-gpu.exe, tetgen.exe,
  preprocess.exe, all matching; `solver.stderr.txt` empty.
- **Results** (`a5-results-gpu.png`): state `verified`; the panel's sub "SPPS on the GPU, NVIDIA GeForce RTX
  5070 · results checked before any value is shown"; the Acoustics tab `ready`, its head "Run 2 ·
  Absorbing panels  SPPS on the GPU, NVIDIA GeForce RTX 5070".
- The first pass of the same bed (09:28:01, Run 1 `20261007-092804-521-spps`) gave the same; its receipts
  are in `.out\a5\first-pass-0928\`.

### 3. The refusal, shown in the app

Project: `tests/fixtures/rooms/tutorial1_box_fitting.simpa` (copied to `.out\a5\refusal\` and
`.out\a5\app-refusal\`).
- CLI: `simpa run box_fitting.simpa --solver spps --device gpu`: FAIL, exit 5, run
  `.out\a5\refusal\runs\20261007-092330-556-spps`, reasons `exit_nonzero` ("exit 2 (0x00000002)") and
  `spps_gpu_refused` ("spps-gpu: refused: fittings_unsupported: enc_calc is on and the mesh holds fitting
  volumes (encombrement); the fitting walk (CalculationCore.cpp:32-39, 132-185) comes in A3"),
  `gpu_device` recorded, 11.9 ms.
- App (`a5-refusal.png`): run `.out\a5\app-refusal\runs\20261007-092838-715-spps`; the last run reads FAIL
  with "The solver finished but reported an error. EXIT_NONZERO", "SPPS on the GPU does not run this
  project; SPPS on the CPU does. SPPS_GPU_REFUSED" and spps-gpu's reason in its own words beneath.

### 4. The unavailable path

- Unit: `simpa-core` `run::gpu::tests` (a folder with no `spps-gpu.exe`: unavailable, the reason names
  where it looked; a file there that is not the verified build: refused before it is started), `app`
  `runs::tests::a5_the_gpu_is_unavailable_with_its_reason_when_the_probe_finds_nothing` (the same through
  `probe_status`, and the device names), UI `model.test.ts` "A5 bed 4" (missing exe, exit 1: the entry is
  shown, disabled, with the reason as its text and tooltip; not answered yet; no reason given).
- Exit 1 on the real executable: `crates/simpa-core/tests/gpu_probe.rs` (ignored by default; needs
  `spps-gpu.exe` and a device), run with `--ignored`: with the device, the line in 0.219 s; with
  `CUDA_VISIBLE_DEVICES=-1`, "no CUDA device: no CUDA-capable device is detected (--probe exited 1)".
- In the real app, by accident: the first bed attempt (09:25) ran before the command was in Tauri's ACL;
  the probe answered "Command spps_gpu_status not allowed by ACL", and the app showed the entry disabled
  with that reason (`.out\a5\superseded-acl-0925\a5-simulate-gpu-entry.png`). Those runs fell back to the
  CPU and are kept there, superseded.

## Tests

Run with `CARGO_TARGET_DIR=C:	mp
m-target-a5` and `SIMPA_SOLVERS_DIR=C:	mp
m-solvers-a5`.
- **`cargo test -p simpa-core -p simpa -p app --no-fail-fast`, once** (09:29-09:41, log
  `C:	mp
m-target-a55-suites.log`): **1,170 passed, 7 failed, 41 ignored**. The 7:
  - mine, fixed in `f30a4d9`: `run_verdict::each_fail_line_is_its_own_reason_code` (counted 11 FAIL rows;
    now 12, with a sample of the new row) and `run_folder_fixtures::every_run_folder_fixture_gives_its_expected_verdict`
    (M6(e) "every row hit through run-folder": no fixture prints `spps_gpu_refused`). The run-folder
    fixtures are generated against upstream's source and solvers (`mkstubs.py`, `mkexpected.py`'s stub
    evidence, which looks for each stub line in upstream's source), so a stub of spps-gpu's line does not
    fit them; the gate now names that one row as judged elsewhere and prints "22/22 classifier rows hit
    (spps_gpu_refused is judged in run_verdict.rs)". A decision beyond the spec, for Burhan to overrule.
  - environment, pass when it is given: `parity_tutorials::tutorial_3` and
    `solver_fingerprint::{the_tetgen_160_reference_is_found_and_our_tetgen_is_refused,
    a_relinked_reference_is_still_the_reference_and_one_changed_code_byte_is_not}` need upstream's
    TetGen 1.6.0 beside the solvers folder; with `SIMPA_TETGEN160` set to the main checkout's
    `target\solversuild\src	etgen\Release	etgen.exe` (read only): parity 7 passed 4 ignored,
    fingerprint 3 passed.
  - known load-flaky, passes alone: `process::winproc::tests::dropping_the_tree_kills_the_child`
    (HANDOFF-2026-10-05 line 35).
  - **not A5's, still failing:** `app results_data::tests::a_large_map_is_served_at_a_coarser_bin_within_the_bound`,
    `attempt to subtract with overflow` at `results_data.rs:1019` (`hi - lo`), deterministic alone. The test
    was written on 10-06 and never run (`docs/investigations/2026-10-06-third-octave-bug/READER.md:50`,
    "NOT RUN"); A5 does not touch `results_data.rs` or `csbin`.
- Targeted reruns after the fixes: `run_verdict` 30/30, `run_folder_fixtures` 3/3 (29/29 fixtures),
  `cli_run` 22/22 (with the new `a5_device_gpu_is_refused_with_its_reason_before_any_run`: `--device
  cuda`, gpu with TCR, gpu with `spps.exe` as `--solver-exe`, each exit 2 with its reason and no run folder),
  `gpu_probe -- --ignored` 1/1.
- New tests: core `run::gpu::tests` (4), `run_manifest::the_gpu_device_is_written_only_for_a_gpu_run`,
  `results_solver_build::a5_a_gpu_run_is_verified_by_its_spps_gpu_check_only`, `gpu_probe` (ignored by
  default), app `a5_the_gpu_is_unavailable_with_its_reason_when_the_probe_finds_nothing`, CLI as above, UI 3.
- UI: `npm run typecheck` clean; `npm test` **330 / 330**.

## What is untested

- **Zeph.** The fat build holds sm_75; nothing here ran on the RTX 2060. `tools/devtools/package-zeph.ps1`
  does not copy `spps-gpu.exe` yet (the Zeph build is its own step).
- **A machine with no CUDA device, in the app.** Covered by unit tests, by `CUDA_VISIBLE_DEVICES=-1` on the
  executable, and by the ACL accident, not by an app on such a machine.
- **`solvers/build.ps1 -UpdateCommittedManifest`'s carry-over and the `m1.ps1` change** were not executed
  (the carry-over's two PowerShell idioms, `[ordered]@{} + $hashes` and `OrderedDictionary.Insert`, were
  checked in PowerShell 5.1 on their own).
- **The m10/m11/m12 e2e gates** were not run (the spec's constraint). The Results step's new
  `[data-run-label]` span comes after the title, so the gates' first-label read should be unchanged; CPU
  runs show "SPPS" exactly as before.
- **The probe's timeout** (5 s) has no test with a hanging executable.
- Transmission children (A2's open rule 11) are untouched by A5.

## Found on the way

1. **Tauri's ACL refuses a new command** until it is in `app/src-tauri/build.rs` and
   `capabilities/default.json`; `cargo test` cannot see this, only a release app can (fixed, `bba993a`).
2. **`solvers/build.ps1` would have dropped the `spps-gpu.exe` pin** on the next
   `-UpdateCommittedManifest`, and **`tools/gates/m1.ps1` would have failed** on a manifest with five
   executables (both fixed, untested as above).
3. **spps-gpu's refusal text names a plan step** ("the fitting walk ... comes in A3"), which the app now
   shows to the user. Changing it needs a rebuild of spps-gpu and a new pin.
4. **`SPPS_GPU_BACKEND=cpu` in the app's environment** would make a "GPU" run trace on the CPU while
   `run.json` records the GPU device; `solve/spps-gpu.json`'s `backend` says which, but nothing
   cross-checks it.
5. The probe is cached for the session: a device that appears later (a driver installed) needs a restart.
6. The Runs tab's solver column still reads "SPPS" for a GPU run, and the status bar still says "Solvers:
   I-Simpa 1.4.0 · SPPS, TCR" (not in the spec).

## Commits on `a5`

`f41cdda` the core, the CLI, the manifest pin and the app backend · `855e8cf` the UI · `bba993a` the ACL
and the bed driver · `30547cf` decision 70 and the contract's words · `f30a4d9` the test fixes and the CLI `--device` test ·
the report commit (this file).
