# M11 review 2: the run lifecycle

2026-09-30, 00:06-00:43, Grace. Branch `m11` at `2642cff`, diff `d7ecbda...m11`. Area: Cancel, window
close, `app_quit` and a killed `app.exe` against every stage (solvers, geometry, validate, meshing
with TetGen and `preprocess.exe`, export, pre-launch, solve, results). I did not launch the app or
run any e2e spec. The core path was driven by a scratch harness that calls `run_project` exactly as
the app's run thread does (`MeshChoice::Build` with tetgen and preprocess, `verify` = the embedded
manifest). The UI path was driven by the real `actions.ts` and `store.ts` behind a Tauri IPC
stand-in. Both harnesses are in `lifecycle-harness/` beside this file. Every process they started
was accounted for, and at the end tasklist lists no spps, tetgen, preprocess or harness process.

## Verdict

**Cancel is sound on every stage once a child is inside its job.** Measured through `run_project`
on the corrected hall, `run.json` is written 1.3 to 45 ms after a cancel, and no program is running
when `run_project` returns (table below).

**Kill is not sound.** A child spawned while `app.exe` dies stays behind: suspended, outside every
job, and invisible to the gate's asserted check. I produced 21 such orphans (B1, M1).

The UI can also lose a Cancel, or lose track of a live run, in two narrow races (M2, M3).

| # | Sev | Finding |
|---|---|---|
| B1 | blocker | A kill landing in the ms between `CreateProcessW` and `AssignProcessToJobObject` leaves the child running outside any job: suspended, never run, surviving its parent |
| M1 | major | The gate's process check cannot see that orphan: CIM reports an empty `ExecutablePath` for it |
| M2 | major | A Cancel sent before `run_start` has registered the run is dropped. The UI then locks itself at "Cancelling…" and the run completes |
| M3 | major | Two Run triggers before `runStore` is set: the loser's `RUN_ACTIVE` catch nulls `runStore` while the winner runs. The UI has no Cancel control and folds none of the run's lines |
| m1 | minor | The close handler's let-through path sleeps up to 3 s on Tauri's event-loop thread |
| m2 | minor | Cancel racing a natural exit records `exit_code: null` and CANCELLED for a solver that exited 0 with every file written |
| m3 | minor | A cancel during meshing (TetGen, or before it) lists `tetgen_output_missing` and `neigh_missing` on the Cancelled row, even when TetGen never ran |
| m4 | minor | `m11-d-close`'s "app.exe exited N ms" receipt cannot report less than about 2.2 s |

---

## B1 (blocker): a child spawned while app.exe dies survives, outside every job

**Where.** `crates/simpa-core/src/process/winproc.rs:70-83`. `Job::new` (71), then
`command.spawn()` with `CREATE_SUSPENDED` (72-74), then `job.assign` and `resume` (75). The module
doc (13-20) accepts this window as "microseconds". PLAN.md 2.8 ("Stop-Process needs no code") and
`app/src-tauri/src/main.rs:25-26` both say a killed app ends the solver through
`KILL_ON_JOB_CLOSE`. Every child goes through this one spawn:
- the solver: `run/manager.rs:1588`;
- TetGen and its `-d` follow-up: `mesh/tetgen.rs:96`, reached through `mesh.rs` `call()`;
- `preprocess.exe`: `mesh/preprocess.rs:84`, through `run_logged`.

**Evidence.** The harness child ran `run_project` on the hall, and was killed with
`TerminateProcess` (what `Stop-Process -Force` does) a set delay after `run_logged` or `launch`
created the program's log. That log is the last thing written before `JobTree::spawn`. The check
2 s later was whether the program's image was still mapped, by opening it for write.

| Program image | Kill delay after its log appears | Orphans |
|---|---|---|
| preprocess.exe, first run of a fresh copy | 0 ms (not yet created) | 0 of 3 |
| | 2, 5, 10 ms | 0 of 9 |
| | **20 ms** | **2 of 3** |
| | 40 ms | 0 of 3 |
| preprocess.exe, warm (already run once) | **1 ms** | **3 of 3** |
| | **2 ms** | **3 of 3** |
| | **3 ms** | **2 of 3** |
| | **4 ms** | **1 of 3** |
| | 5, 7, 10, 15, 20 ms | 0 of 15 |
| tetgen.exe, warm | **2 ms** | **4 of 4** |
| spps.exe, warm | **2 ms** | **3 of 4** |

The first orphan was found by the plain kill test, before any sweep: `preprocess-lk-8316.exe`,
pid 2740. Every orphan looked the same:
- one thread, `Wait/Suspended`;
- kernel and user time 0;
- `IsProcessInJob` false;
- parent dead;
- `Win32_Process.ExecutablePath` empty.

Outside the window, the job does its job:
- TetGen killed mid-run was gone 0.2 ms after the kill.
- A two-process stand-in for TetGen (cmd.exe running a private `ping` copy) had its grandchild
  gone 5.3 ms after.
- preprocess.exe killed at 5 ms or later was gone in 15 of 15 trials.

**What an orphan costs.**
- It never executes, so it produces no wrong number.
- It holds the solver image. Copying over it failed: `Device or resource busy`. That blocks
  replacing or uninstalling the solvers, and the M13 updater must replace them.
- It holds every inheritable handle it was born with. My `| tee` pipeline never got EOF and hung
  600 s, until I stopped orphan pid 19272.
- It lasts until something kills it or the user logs off.
- It does not pin its run folder: that folder deleted fine, because a never-run process opens no
  current-directory handle.

**Window size.** On this machine it is about 3 to 4 ms per spawn with a warm image, and reaches
about 20 ms for an image's first run: the gate's private solver copies, and the first run after an
install or update. A run of the hall spawns TetGen and the solver, plus preprocess.exe and
`tetgen -d` when those apply.

**Close paths that end the process themselves are safe.** Both cancel and wait first, and nothing
spawns after a cancel:
- `app_quit`: `commands.rs:527-533`;
- the let-through in `main.rs:142-145`.

**Fix direction (not applied).** Either change closes the window:
- create each child already inside its job, with `PROC_THREAD_ATTRIBUTE_JOB_LIST` (a raw
  `CreateProcessW`, which std cannot do);
- or put `app.exe` (and `simpa.exe`) into a `KILL_ON_JOB_CLOSE` job of its own at startup, so every
  child is born inside it (nested jobs).

## M1 (major): the gate's "no spps.exe" check cannot see this orphan

**Where.** `app/e2e/lib/procs.ts:82-91` (`processesFrom`) keeps processes whose
`ExecutablePath -ieq <private spps>`. Every asserted "no solver left" goes through it:
- `m11.gate.e2e.ts:240` (m11-c);
- `m11.close.e2e.ts:94` (m11-d-close);
- `m11.kill.e2e.ts:36` (m11-d-kill);
- `m11.dock.e2e.ts:277`.

**Evidence.** With orphan pid 19272 alive (`spps-warm.exe`, suspended), the three lookups
disagreed:
- `processesFrom`'s exact query printed nothing;
- `@(Get-Process -Name 'spps-warm').Count` (the shape of `machineWideCount`, printed and never
  asserted) printed `1`;
- `tasklist /FI "IMAGENAME eq spps-warm.exe"` listed it.

CIM gives no `ExecutablePath` for a process that has never run. The core's own CLI tests match by
image name through tasklist (`crates/simpa/tests/support/mod.rs:81-89`), so they would see it.

**Impact.** No gate id can fail on B1:
- m11-d-kill kills after `waitRun(run, 'progress')`, long after the spawn, so it never lands in the
  window;
- and if a gate run did land in it, the asserted check would read "none".

**Fix direction.** Match processes by image name as well, and add a kill-during-spawn say-NO.

## M2 (major): a Cancel sent before the run is registered is dropped, and the UI locks

**Where.**
- `actions.ts:406` sets `runStore` to `status: 'starting'` before `backend.runStart` (418).
- `SimulatePanel.tsx:332-333` then shows `RunningBlock`, whose Cancel button (190-198) is enabled
  in `starting`.
- `run_cancel` (`commands.rs:433-436`) calls `RunSlot::cancel` (`runs.rs:799-807`), which answers
  `false` when the slot is empty. The slot stays empty until `start()` has run `plan()` and
  registered a fresh `CancelToken::new()` (`runs.rs:953`, 961, 964-978). Nothing records a cancel
  that arrives earlier.
- `actions.ts:430-434` ignores the `false`, sets `cancelling`, and never clears it. `started`
  keeps `cancelling` (359).
- The button is then disabled (`SimulatePanel.tsx:193`), and so is Simulate › Cancel run
  (`MenuBar.tsx:105`).

**Evidence.** `lifecycle-harness/ui/lifecycle.mjs`, F1: the real `actions.ts` with
`run_start`'s registration held until the Cancel had been answered.

```
F1 t0: runStore.status=starting run=- progress=null; backend slot EMPTY
F1 runCancel() answered false; now runStore.status=cancelling
F1 +150 ms: runStore.status=cancelling run=20260930-000001-000-spps progress=9; backend run active=true, its token cancelled=false
F1 end: the run ended OK (cancelled=false) after the user pressed Cancel
```

**Reachability: not reproduced in the app.**
- The window is one IPC plus `plan()`. The gate measured the IPC at p50 0.70 ms and p99 1.50 ms
  (gate `20260929-233848`, `wdio.log:18`).
- The Running block paints about 16 ms after `runStore` is set, so a mouse cannot normally land
  in the window.
- The `runCancel` hook can, and so can a session lock held across `plan()`.

If it does land, the solver runs to its end with the UI reading "Cancelling…", and no second
Cancel can be sent.

## M3 (major): a second Run before `runStore` is set orphans the live run from the UI

**Where.**
- `actions.ts:396` refuses on `runStore` alone, but `runStore` is set only at 406, after
  `await save()` (400, when dirty or never saved) and `await refreshSolvers()` (404). A second
  trigger in that gap passes: the Run button (`RunButton.tsx:48`), the panel button
  (`SimulatePanel.tsx:349`), F5 (`App.tsx:69-74`), or Simulate › Run.
- The backend refuses the loser with `RUN_ACTIVE` (`runs.rs:964-971`).
- Its catch runs `runStore.set(null)` (`actions.ts:421-422`) while the winner's run is live.
- `onRunEvents` then drops every non-`started` event, because `current?.run` is undefined (350).

**Evidence.** `lifecycle.mjs`, F2: the second call issued 1 ms after the first.

```
F2 first runStart resolved spps; second runStart rejected RUN_ACTIVE
F2 +60 ms: runStore=null; backend run active=true (20260930-000002-000-spps)
F2 run log folded in the UI for 20260930-000002-000-spps: 0 solver lines
F2 a third Run click: rejected RUN_ACTIVE; runStore=null
```

**Impact.** Until the run ends by itself, the UI shows it as idle:
- no Cancel control;
- no Console lines for the run;
- every Run click is a FAIL line.

Closing the window still cancels the run, through `app_quit`.

**Window.** About two IPC round trips on a saved, clean project. With a dirty project it also
includes a `project_save`, whose duration in the app I did not measure. A double click on Run
with unsaved edits is the plausible trigger.

## m1 (minor): the let-through close path blocks Tauri's main thread for up to 3 s

`main.rs:142-145` calls `runs::cancel_and_wait(.., QUIT_WAIT)`. That function sleeps in 20 ms
steps for up to 3 s (`runs.rs:816-833`). The call comes from three places:
- the UI channel not registered yet (148);
- a hung UI (159);
- a failed send (173).

`on_window_event` runs synchronously inside tao's event loop: `tauri-runtime-wry-2.11.4`
`src/lib.rs:4307-4308` calls `on_close_requested`, which calls the handlers at 4437-4462. This
contradicts the file's own rule at `main.rs:161` ("Never block the window's thread"). The wait
cannot deadlock: the run thread and the batcher never need the main thread (`events.rs:133-137`
never blocks; `channel.send` posts). Inferred from the code; I did not measure a frozen window.

## m2 (minor): cancel racing a natural exit records `exit_code: null` for a solver that exited 0

`process.rs:156-158` looks at the cancel flag before `wait_exit`. A cancel that arrives after the
child exited, but before `drive` looks again (up to 20 ms, `POLL`), is taken as "killed by
cancel". The `Outcome` doc (80-82) says `None` means "killed by cancel before it reported one".

- **Process layer.** `cmd /d /c "echo done& exit 0"` gave `exit_code: Some(0)` with no cancel.
  With the cancel set 300 ms after its last line, it gave `exit_code: None, cancelled: true`.
- **Run layer.** `box_run.simpa`, with the cancel 1.5 s after SPPS printed "End of calculation."
  (the sleep is in the callback to widen the window). The result, 2 of 2:
  `status Cancelled, outcome { exit_code: None, cancelled: true }, files present 20 of 20
  expected`.

Status CANCELLED on a cancel is the deliberate rule (`run_verdict::cancelled_is_never_ok_even_after_exit_0`).
The null exit code is the one untrue field. The race the other way is fine: when the exit is seen
first, the run is judged OK, and `run.json`, the `ended` row and the Console agree.

## m3 (minor): a Cancelled mesh-stage row carries TetGen failure reasons

- `mesh.rs:722-729` (cancel before TetGen) calls `classify(&paths, None, ..)`, which adds
  `tetgen_output_missing` and `neigh_missing` (1442-1458) for a TetGen that never ran.
- A cancel mid-TetGen does the same after `cancelled` (1355-1362).
- `run_project` copies every mesh code into the verdict (`manager.rs:756-763`, `mesh_reasons`).
- So the Runs row reads Cancelled with `MESH_TETGEN_OUTPUT_MISSING` and `MESH_NEIGH_MISSING`
  (`runs.rs:333-348`).

Measured on the hall:

| Cancel lands | Reasons in `run.json` |
|---|---|
| at the Mesh stage, before TetGen | `["cancelled","tetgen_output_missing","neigh_missing"]` |
| mid-TetGen (3 of 3) | the same |
| hall with preprocess on, same point | `["cancelled"]` |
| mid-preprocess | `["cancelled"]` |
| mid-SPPS | `["cancelled"]` |

The code is pre-existing M5 behaviour. M11's Runs tab is the first place a user sees it.

## m4 (minor): `m11-d-close`'s exit-time receipt is the poll time, not the exit time

`m11.close.e2e.ts:92-105` starts looking for the exit only after `sleepUntil(t0 + 2000)` and a CIM
query of about 200 ms. The gate's "app.exe exited 2214 ms after WM_CLOSE" (`wdio.log:44`) is
therefore when the check first looked. The assertion (exit within 5 s) is valid; the number is not
the exit latency.

---

## The CancelToken, stage by stage

**Path from the click.** The Cancel button (`SimulatePanel.tsx:195`) or Simulate › Cancel run
(`MenuBar.tsx:104`) calls `actions.runCancel` (`actions.ts:430-434`). That goes through
`backend.runCancel` (`backend.ts:113`) and `invoke('run_cancel')` to `commands.rs:433-436`, which
runs through `guard::blocking` (`guard.rs:55-73`, `spawn_blocking`). There `RunSlot::cancel`
(`runs.rs:799-807`) calls `CancelToken::cancel` (`process.rs:68-70`, a SeqCst store). App-side cost:
one IPC (p99 1.5 ms) and one uncontended slot lock.

Measured times below are the release harness on the corrected hall (`hall_run.simpa`, and
`hall_pre.simpa` with preprocess on), or `box_long.simpa` for the solve. Each is from the cancel to
`run_project` returning with `run.json` written and every private image unmapped.

| Stage (length on the hall) | Where the token is polled | Child alive? | Worst case, cancel to child gone | Measured, cancel to `run.json` |
|---|---|---|---|---|
| solvers (1.4-1.7 ms) | `manager.rs:677` | none | n/a | 1.8 ms |
| geometry (20.7-21.9 ms) | `manager.rs:697` | none | n/a | 21.9 ms |
| validate (0.8-1.5 ms) | `manager.rs:719` | none | n/a | 1.3 ms |
| mesh, before a program | `mesh.rs:1036` (preprocess), `mesh.rs:722` (TetGen) | none yet | n/a | 28.8 ms (hall); 11.4 ms (preprocess on) |
| mesh, preprocess.exe running | `call()` watchdog `mesh.rs:1278-1281` (`WATCH` 20 ms) sets the local token; then `drive` `process.rs:156` (`POLL` 20 ms, or the next line); then `kill_all` `winproc.rs:96-125` | yes, in its job | 20 + 20 ms + kill, bounded by `REAP_LIMIT` 10 s | 42.8, 39.4 ms (killed at 23.8, 29.2 ms elapsed) |
| mesh, TetGen and `tetgen -d` running | same, through `call()` (`mesh.rs:732`, 1597) | yes | the same | 33.2, 42.5, 44.9 ms (TetGen `cancelled: true, exit_code: null` at 21.9-33.5 ms, against 78-80 ms uncancelled) |
| mesh, after TetGen (classify, verify, `.mbin`) | `mesh.rs:762` (skips `-d`), 1709 | none | n/a | 39-46 ms, a cancel on TetGen's first line (see note) |
| export (12-13 ms) | `manager.rs:824` | none | n/a | 10.6 ms |
| pre_launch (1.9 ms) | `manager.rs:848`, 857 | none | n/a | 1.9 ms |
| solve | `process.rs:156`, then `kill_all` | yes | 20 ms + kill, bounded by 10 s | 5.7, 7.8, 5.6 ms (SPPS stopped at about 374 ms, 0 of 20 files) |
| results | `judged` (`manager.rs:1616`) runs after `process::run` returns; `run_results` (`commands.rs:469-481`) takes no token | none | n/a | n/a |

**Note on TetGen's output.** TetGen's stdout is block-buffered into the pipe, so a callback keyed
to its first line fires after TetGen has exited. In the test, 40 lines arrived at once; TetGen's
record reads `exit_code: 0, cancelled: false`; the cancel was caught at 1709. The Console shows
"Meshing…" with no TetGen line until TetGen ends. That is not a lifecycle defect.

**Process layer alone.** `process_job` measured cancel to `run()` returning at 24.6 ms for a
PowerShell plus ping tree, 14 passed. `cli_mesh`'s two cancel tests passed: TetGen ran 35.5 ms and
preprocess.exe 25.0 ms before the kill.

## Job Object coverage and lifetime

**Coverage.** Every child the run path starts goes through `process::run`, and so through
`JobTree::spawn`:
- the solver (`manager.rs:1588`);
- TetGen, and its `-d` follow-up (`tetgen.rs:96` via `call()`, `mesh.rs:1597`);
- `preprocess.exe` (`preprocess.rs:84`).

Nothing else in `app/src-tauri` or `crates/*/src` spawns a process on the run path. The only
`Command::new` outside `process.rs` are test code (`run/clock.rs:185`) and `bed_cmd.rs`'s `git`.

**Lifetime.** The app does not hold a job for its own lifetime. Each child gets its own unnamed,
non-inheritable job (`winproc.rs:133-153`), owned by the `JobTree` that `drive` holds
(`process.rs:125-128`, 134). The job's handle closes when `drive` returns or unwinds. For a running
child that is enough: killing `app.exe` closes the handle and `KILL_ON_JOB_CLOSE` fires, as
measured above. What it leaves uncovered is B1: from process creation to the assign, the child
has no job.

## Close, app_quit and kill

**Close with the UI alive.**
1. `CloseRequested` calls `prevent_close` and sends the request to the UI (`main.rs:161-178`).
2. The UI shows the save prompt if needed (`actions.ts:467-471`).
3. `app_quit` (`commands.rs:525-535`) runs `cancel_and_wait` for up to 3 s (`runs.rs:816-833`), then
   `app.exit(0)`.

The cancelled run writes `run.json` 1.3 to 45 ms after the cancel, far inside the 3 s. The
`false` a timeout would return is ignored (`commands.rs:528`). After a timeout the process exits
anyway, so any child still alive dies with the job.

**Hung or unregistered UI.** The let-through path (m1): the same cancel and wait, on the main
thread.

**Kill.** Covered by the job for every child except B1's window.

## A second Run while one is active

The backend refuses twice:
- `plan()`, under the session lock (`runs.rs:886-891`);
- `start()`, under the slot lock (`runs.rs:964-971`). This check is authoritative: two concurrent
  `run_start`s cannot both register.

The UI refuses through `refuseDuringRun` (`actions.ts:260-264`, 396), but only once `runStore`
is set (M3).

The slot is freed before the last event is pushed (`runs.rs:1127-1132`). A new run can therefore
start while the old thread sends its `ended`. Each run has its own channel, so the two streams do
not mix.

## Cancel racing a natural finish

- **The exit is seen first.** Judged OK. `run.json`, the `ended` row (read back from disk by
  `runs::list`, `runs.rs:1141-1145`) and the Console agree.
- **The cancel is seen first.** CANCELLED, with a false null exit code (m2).
- **Meshing.** A cancel after TetGen exited 0 is recorded CANCELLED at `mesh.rs:1709`. TetGen's own
  record keeps `exit_code: 0, cancelled: false`: consistent, and measured.

In every case, one status reaches `run.json`, the row and the Console.

## Receipts: what I ran

**Cargo tests.** Target `C:/tmp/nm-target`, scratch in the session scratchpad, no app.exe running:
- `cargo test -p simpa-core --test process_job`: 14 passed, 1 ignored, 4.42 s;
- `cargo test -p simpa --test cli_mesh a_cancel_50_ms_into`: 2 passed.

**Rust harness.** `lifecycle-harness/src/main.rs`, a crate outside the workspace depending on
`crates/simpa-core` by path, release build. `Cargo.lock` is the workspace's own, copied so the
versions match. Build with `RUSTUP_TOOLCHAIN=1.98.1 CARGO_TARGET_DIR=C:/tmp/nm-target cargo build
--release --offline`, and run with `SIMPA_SOLVERS_DIR` set.

| Mode | Runs |
|---|---|
| `stages` | 3 uncancelled plus 6 stage cancels |
| `cancel-meshline` | 5 |
| `cancel-timed` | TetGen 3, preprocess 2, solve 3 |
| `kill-meshline` | 3 |
| `kill-sweep` | 58 trials |
| `race-exit`, `race-solve` | 1 and 2 |

The harness created 21 orphans. Each one was stopped by the sweep or by hand: pids 2740, 19272 and
13928 by hand. A final `tasklist` shows none.

**Orphan probe.** `lifecycle-harness/probe.ps1`: CIM, the thread states and `IsProcessInJob`.

**UI harness.** `lifecycle-harness/ui/`, run with `node --import ./register.mjs lifecycle.mjs`
(Node 24.15, `NODE_PATH` set to `app/node_modules`).

## Not verified

- **Nothing was reproduced in the running app.** No app launch and no e2e spec: the other reviewers
  were working in the same worktree, and no existing spec lands in B1's window or M2/M3's race.
  - B1 is shown with the core in a helper process. `app.exe` runs the same core in-process, so the
    mechanism is identical, but no gate id shows it.
  - M2 and M3 are shown with the real `actions.ts` under a stand-in backend that follows
    `runs.rs`'s slot rules. The window sizes in the app are inferred from the gate's IPC
    measurement, not measured.
- **Unmeasured app timings:**
  - `project_save`'s duration in the app (M3's window when the project is dirty);
  - whether the hung-UI path actually freezes the window for 3 s (m1).
- **B1 on other machines.** The window's size elsewhere (AV product, disk) is not measured. Here it
  is about 3-4 ms warm and about 20 ms for a first run.
- **WebView2 reload.** I did not check whether a reload (browser accelerator keys, devtools) is
  possible in the release build. If it is, the UI loses `runStore` mid-run and ends in M3's state.
- **`REAP_LIMIT` failure.** A `kill_all` failure after 10 s is recorded as `tetgen_launch_failed` /
  `launch_failed` (`mesh.rs:744-746`, `manager.rs:1602-1608`). That is from the code only; I could
  not make a tree survive `TerminateJobObject`.
