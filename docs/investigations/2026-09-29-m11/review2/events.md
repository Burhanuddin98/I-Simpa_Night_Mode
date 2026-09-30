# M11 review 2: the progress and Console event stream

Reviewer's area: the core's `RunEvent` emission, the app's `Batcher` and run thread, the Tauri
`Channel`, and the UI's stream handler with the Console and Runs panes. Tree: `m11` at `2642cff`
(code as `414ad5c`). Nothing in product code was edited. I did not launch the app or a real
solver. The only process I launched was the CLI `simpa.exe`, with a batch-file stand-in for the
solver, created `CREATE_NO_WINDOW`. Before that I checked the process list: no `app.exe`, solver
or driver was running.

## Verdict

The path from the solver to the stream loses nothing, repeats nothing and keeps the order. That
holds at 10,000 lines and beyond, and after Cancel too. The `Batcher` is verified: see the
harness below. The gate's `m11-a` check would catch a batch lost in transit.

**One major defect is in the UI.** The stream handler files a run's events under the global
`runStore`, not under the channel they came on. A second Run that the backend refuses clears
`runStore` while the first run is live. From then on every event of that run is dropped before
the Console, and the Cancel control disappears. The Console's own gap check cannot see it.

| # | Severity | What | Where |
|---|---|---|---|
| E1 | **major** | A Run click or F5 that lands while an earlier Run is still saving starts a second `run_start`. The backend refuses it with `RUN_ACTIVE`, and its `catch` sets `runStore` to null while the first run is live. Every later event of that run is dropped: 0 solver lines counted, no FAIL or WARN line shown. Cancel disappears, and Run, New and Open are enabled again. Only the counts strip flags it, after the run has ended | `app/ui/src/actions.ts:396,400,404,406,422`; `actions.ts:350-351` |
| E2 | minor | A Cancel that reaches the backend before `run_start` has registered the run gets the answer `false`, and the UI ignores it. The run runs to the end while the UI reads "Cancelling…" and Cancel stays disabled | `actions.ts:430-433`; `SimulatePanel.tsx:193`; `MenuBar.tsx:105` |
| E3 | minor | The UI never reads `batch.last`. A stream that ends without `ended` or `failed` leaves the UI "running" for good: Run is blocked by `RUN_ACTIVE`, Cancel answers `false` and the status sticks at "cancelling". Nothing reconciles it with the backend | `actions.ts:343-387` |
| E4 | minor | `BatchStats.send_failures` is counted, then thrown away: `let _ = batcher.finish()`. PLAN 2.5 says a closed channel "is counted", but no log, `run.json` or UI ever reads the count | `app/src-tauri/src/runs.rs:1173` |
| E5 | minor | The gate never compares the stream of a run cancelled under load. `m11-c` (the hall, cancelled mid-stream) compares no count at all. `m11-dock-live` compares the counts after Cancel, but not `gaps`, `dupes` or `n`, and it does not assert that the UI's `stream-check` FAIL is absent | `app/e2e/specs/m11.gate.e2e.ts:220-260`; `m11.dock.e2e.ts:287-296` |
| E6 | minor | `RunLog.lines` is copied whole for every non-PROGRESS line, which is O(n²) per run, and nothing reads it | `app/ui/src/flow.ts:72`; `store.ts:140` |

## E1: a second Run clears the live run (major)

**The mechanism.** `runStart` checks `refuseDuringRun('Run')` at `actions.ts:396`. It then awaits
`save()` (400, whenever the project is dirty: PQ1, the usual edit-then-Run flow) and
`refreshSolvers()` (404). Only after that does it set `runStore` (406). The Run button, the F5
handler (`App.tsx:69-73`, which has no `e.repeat` check) and the menu entry all gate on
`runStore !== null`. So a second trigger during those awaits passes the check too.

The backend accepts the first `run_start` and refuses the second with `RUN_ACTIVE`
(`runs.rs:886`, `966`). The refused call's `catch` then runs `runStore.set(null)` unconditionally
(`actions.ts:422`), wiping the state of the run that is live. `onRunEvents` files every event
except `started` under `current?.run`, which is the global `runStore` (`actions.ts:350`). With
`runStore` null, `runName` is `undefined` and the event is never folded (`351`). The channel still
delivers everything; the handler discards it.

**What the user sees.**
- No solver FAIL or WARN line reaches the Console.
- The live PROGRESS line and the Simulate step's running block (with Cancel,
  `SimulatePanel.tsx:332-333`) vanish.
- Run, New and Open are enabled mid-run (`MenuBar.tsx:61-78`). The Console's only new FAIL line is
  "Could not start the run: a run is active: cancel it first (RUN_ACTIVE)", with no Cancel to press.
- New or Open is now allowed mid-run, which PQ4 forbids. The run is then orphaned from the Runs tab.
- The run's `stream-check` reads 0 gaps: the dropped events never entered the log, so the gap
  check cannot see them.
- Only after `ended` does the counts strip show `FAIL run.json counts …`.
- No child process outlives a close: `app_quit` cancels through the backend slot
  (`runs.rs:816-833`). That is why this is major, not blocker.

**Evidence.** The harness `review2/ts-harness/stream.test.mjs` runs the real `actions.ts`, stores,
`flow.ts` and `features/dock/model.ts` under node, with only `@tauri-apps/api/core` and
`plugin-dialog` mocked. Command, from `review2/ts-harness`:
`node --import ./register.mjs --test stream.test.mjs`, 7 of 7 pass. The mock backend refuses a
second `run_start` exactly as `runs.rs` does. The save is modelled at 80 ms, with the second click
40 ms after the first. Output:

```
double click: run_start called 2 times; first answered {"solver":"spps",...}; second {"code":"RUN_ACTIVE",...};
  runStore now null while the backend's run is live; Run blockers []
double click: received counts {"PROGRESS":0,"INFO":0,"OK":0,"WARN":0,"FAIL":0}, gaps 0, dupes 0;
  verbatim solver lines shown []; strip check {"match":false,"manifest":{"PROGRESS":200,"INFO":1,"OK":1,"WARN":1,"FAIL":1}};
  FAIL lines ["Could not start the run: a run is active: cancel it first (RUN_ACTIVE)"]
```

There are two controls:
- **One click.** The same stream gives
  `counts {"PROGRESS":200,"INFO":1,"OK":1,"WARN":1,"FAIL":1}`, 4 verbatim lines and
  `match: true`.
- **Saved project, same double click.** Only one `run_start` happens, so the window is exactly
  the awaits before line 406.

**Not measured:** how long `project_save` takes in the real app. That duration is the width of
the window. On a clean project the window is one `solvers_status` round trip (the gate's pings
have a p50 of 0.70 ms), and no human double click fits in it. On a dirty project it is a save of
the `.simpa` to B: plus that round trip.

**Fix direction (not applied).**
- File each event under its own channel's run: give each run's handler a closure that remembers
  the name from its own `started` event, instead of reading `runStore`.
- Set a "starting" guard synchronously at line 396, before the first `await`.
- In the `catch`, clear `runStore` only if it still holds the object this call set.

A regression test: the harness's `defect:` case, with its assertions inverted.

## E2 to E6 (minor)

- **E2.** In the harness's `early cancel` case, `run_cancel` answers `false` because the run is not
  registered yet. The output is `runStore status after run_start cancelling, mid-run cancelling;
  the run ended OK`. The window is one `run_start` round trip, whose `plan()` is cheap once the
  solver cache is warm. A human will rarely hit it.
- **E3.** In the harness's `no ended` case, the output is `after the last batch runStore.status
  running; Run blockers ["RUN_ACTIVE"]; Cancel answered false, status now cancelling`. The trigger
  would be a Tauri channel message that is never delivered. See "Not verified" below: I did not
  reproduce it.
- **E4.** `runs.rs:1099-1107` turns `channel.send(..).is_ok()` into `send_failures`, and
  `runs.rs:1173` discards the stats. The existing unit test `a_refused_batch_is_counted`
  (`events.rs:231`) proves only that the Batcher counts; nothing reads the count.
- **E5.** `m11-a` is strong: it compares the DOM counts, `runLog.counts`, and `n`, `min`, `max`,
  `dupes` and `gaps` with `run.json` on a run of 10,002 events. The one cancelled run with a count
  check (`m11-dock-live`, `box_long`) checks the five class counts only. A lost batch that carries
  no solver line (for example a lone `stage`) would give `gaps` 1, show the UI's `stream-check`
  FAIL, and still pass `m11-dock-live`.
- **E6.** The harness's `cost` case put 10,000 lines through `onRunEvents` in 315-event batches,
  315 being the box run's average batch. PROGRESS took 5.4 ms in total. WARN took 101.1 ms, and
  its per-batch cost grew to 4 to 7 ms by the end. The React render is not included.
  `grep "\.lines\b"` over `app/ui/src` finds `RunLog.lines` written at `flow.ts:72` and read
  nowhere.

## The questions asked

**Can a line be lost, duplicated or reordered between the solver and the Console?** Not on the
normal path. Stage by stage:

1. **Process layer.** `process.rs:142` uses a bounded queue of 4,096 lines, so a full queue
   stalls the child, never drops a line. Lines are dropped only after the tree is dead and
   2 s have passed with nothing on the pipes (`DRAIN_IDLE`, `:94`, `:195`). Any lines dropped
   that way are missing from the stream, from `run.json`'s counts and from the log files alike.
2. **Core.** `Sink::take` sends each classified line to `on_event` and then records it
   (`manager.rs:1462-1469`). `run.json`'s `lines` are `ClassCounts::of` that same `Vec`
   (`:1598`, `:1641`), so the streamed solver lines equal `run.json` by construction.
3. **Run thread.** Every event gets the next `seq`, and `ended` or `failed` takes the one after
   (`runs.rs:1123-1124`, `1146-1159`).
4. **Batcher.** Verified by the harness below.
5. **Tauri 2.11.6.**
   - A batch of 8,192 bytes or more goes by an async `fetch`, a smaller one by `eval`
     (`tauri-2.11.6/src/ipc/channel.rs:200-219`). Each carries a per-channel `index`.
   - `@tauri-apps/api` 2.11.1 `Channel` delivers messages in index order, holding early ones
     back, and drops a repeated index (`node_modules/@tauri-apps/api/core.js:82-115`).
   - One PROGRESS event is 195 bytes, so a batch of more than 42 events takes the fetch path.
     The box run averaged 315 events (about 61 KB) per batch.
   - `m11-a`'s receipt, from the final gate `wdio.log`: 10,002 events in 1,587 ms, `n 10002,
     min 0, max 10001, dupes 0, gaps 0`, equal to `run.json`. The fetch path and the reordering
     were exercised at load, and they held.
6. **UI.** The only way to lose events is E1.

**Under what load?** The Batcher harness is `review2/batcher-harness`. It compiles the real
`app/src-tauri/src/events.rs` through `#[path]`, and its own three tests run too. Command:
`CARGO_TARGET_DIR=C:/tmp/nm-target cargo test --offline --release -- --nocapture --test-threads=1`.
All 8 tests pass:
- **Bursts:** 300,000 events in bursts of 1 to 3,000, into a sink that stalls up to 40 ms one call
  in five. 8 batches, the biggest 53,064 events; every event exactly once and in order.
- **Pauses around the period:** 20 seeds, pauses of 0 to 120 ms, a sink stalling up to 80 ms.
  142,354 events in 478 batches: every event exactly once and in order, batch numbers
  contiguous, exactly one `last` and it is the final batch, and no empty batch before it.
- **Latency:** a steady 20,000 events per second. The oldest event of a batch waited at most
  51.7 ms, against the 50 ms period.
- **Slow sink:** a sink taking 120 ms per call loses nothing, but events wait up to 291 ms. So
  "at most one period late" (`events.rs:5-6`) holds only while the sink is fast. Tauri's `eval`
  posts to the event loop and does not block, so this is not a defect.

**Does the per-class count strip derive from what was received, and would the gate catch a loss?**

- **It derives from what was received.** The strip shows `log.counts` (`model.ts:67`), which
  `foldEvent` increments for each solver `line` event received (`flow.ts:74-75`). It is compared
  with `run.json` once the run has ended (`model.ts:61-68`). The `stream-check` FAIL shows gaps
  and repeats (`ConsolePane.tsx:86,110-115`).
- **The gate catches a loss in transit.** `m11-a` (`m11.gate.e2e.ts:140-152`) asserts the DOM
  strip, `runLog.counts` and `[n, min, max, dupes, gaps]` against `run.json`, so a lost or
  repeated batch fails it.
- **It cannot catch E1.** No spec starts a run twice, and the gate's box run is started on a
  saved project. It also has the E5 gaps.

**Lines emitted after Cancel.** They are kept, streamed and counted the same way as the rest.
- **Core, measured.** `simpa run-folder` ran with a batch-file stand-in that writes 20,002 lines in
  one `type` burst, with `--cancel-after-progress 40`. The binary was rebuilt from this tree;
  output is in the scratchpad `postcancel/`. Result: 20,002 lines streamed through `on_event`
  (INFO 1, PROGRESS 20,000, OK 1), and `run.json` `lines` gave the same numbers. 12,000 of those
  PROGRESS lines, and the `End of calculation.` line, arrived after the cancel fired. Verdict
  CANCELLED, `cancelled: true`.
- **UI, harness.** Lines after `runCancel` are folded, and the strip matches `run.json` with 0
  gaps.
- **One consequence to note:** after a late Cancel, a Cancelled run's Console can show SPPS's own
  `OK End of calculation.` line verbatim.

**A run started while the previous stream is still flushing.** The backend frees the slot
(`runs.rs:1127-1132`) before it builds the `ended` row and pushes it (`1142`, `1172`). In that
window it would accept a new `run_start`. The UI does not send one, because `runStore` stays set
until `ended` is folded (`actions.ts:369-370`, `383`). One run's events cannot reach another run's
log: each run has its own channel, and `ended` is the last event on it. The one way to get two
overlapping `run_start` calls is E1, and E1 does not mix two streams. It loses the one live
stream.

## Not verified

- **Tauri's fetch path failing.** If the fetch fails (`channel.rs:218`, `.catch(console.error)`),
  that index is never delivered. The JS `Channel` then holds every later message, `ended`
  included, forever, which is E3's stuck state. A case that might trigger it: a custom-protocol
  failure that makes the IPC layer re-send the fetch over postMessage, where the data is already
  taken ("data not found"). I did not reproduce this; the app was not launched.
- **Render cost in the WebView.** I did not measure the React Console's render or frame cost when
  thousands of non-PROGRESS lines arrive, only the handler (E6).
- **The real E1 window.** The `project_save` duration in the app, which is the width of E1's
  window, is modelled at 80 ms, not measured.

## Scratch left in place, not committed

- `review2/batcher-harness/`: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`. Build output went to
  `C:/tmp/nm-target`.
- `review2/ts-harness/`: `register.mjs`, `hooks.mjs`, `mock-core.mjs`, `mock-dialog.mjs`,
  `stream.test.mjs`. Its `defect:` case exposes E1 and could become the regression test. Whether
  to commit it is the orchestrator's call; the task said not to commit.
- The CLI run folders are in this session's scratchpad on C:, under `postcancel/`.
