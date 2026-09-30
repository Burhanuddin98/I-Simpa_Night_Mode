# Backlog 38 and 39: the gates

Pass bar 4 and 7 of `PLAN.md`, on branch `b38-39` at `8f6f77e` with the tree clean, on Grace,
2026-09-30 23:14-23:40. The gate runner did not write the code. Each gate ran once, in the plan's
order, each after the previous one ended, with the parameters that
`docs/investigations/2026-09-29-m11/GATE.md` records (lines 42-44). Nothing was fixed or re-run.
Every number below is what a gate printed. Each gate's console was captured to
`C:\tmp\b38-39\gate-<m11|m10|m9>.console.txt`. Where a gate writes a count only to its log, the
log in its work folder is named.

## Verdicts

| Gate | Command | Start-end | Verdict, as printed | Counts, as printed |
|---|---|---|---|---|
| M11 | `powershell -File tools/gates/m11.ps1`, bare | 23:14:58-23:28:47 (13m49s; 14m45s on `c9edf1e`) | **M11 PASSED**, exit 0 | e2e: **32 of 32 required ids passed** (32 tests, 0 failures, 0 skipped), `m11-b38` among them. Core crates: **73 test binaries, 781 passed, 0 failed, 31 ignored**, in 531 s. Focus: **0 foreground changes** in 197,297 ms (748 samples, 15 `app.exe` sessions), `m11-focus: PASS`. 32 top-level checks PASS, 0 FAIL |
| M10 | `powershell -File tools/gates/m10.ps1 -SolversDir C:\tmp\nm-m8a-solvers`, with `SIMPA_TETGEN160=C:\tmp\nm-m10-solvers\build\src\tetgen\Release\tetgen.exe` | 23:29:15-23:38:48 (9m32s; 9m31s) | **M10 PASSED**, exit 0 | e2e: **13 of 13 required ids passed** (27 tests, 0 failures, 0 skipped). Core crates: **73 test binaries, 781 passed, 0 failed, 31 ignored**, in 501 s, TetGen 1.6.0 reference present. m10.ps1 has no focus watcher. 22 top-level checks PASS, 0 FAIL |
| M9 | `powershell -File tools/gates/m9.ps1 -TargetDir C:\tmp\nm-target` | 23:39:07-23:39:53 (45 s; 45 s) | **M9 PASSED**, exit 0 | **23 PASS, 0 FAIL**. No e2e ids, core crates step or focus watcher in m9.ps1 |

**Failures: none.** No check line read FAIL at any indentation, the runs nested inside a gate
included. The PASS check lines were 88 in M11's console, 27 in M10's and 23 in M9's.

**Against pass bar 4:** M11 needs every required e2e id (32 with `m11-b38`), 0 core failures and 0
focus steals, and it printed 32 of 32, 0 failed and 0 foreground changes. M10 and M9 printed
PASSED.

**Work folders**, all on C::
- M11: `C:\tmp\nm-target\gates\m11\20260930-231458` (381 files, 17.8 MB)
- M10: `C:\tmp\nm-target\gates\m10\20260930-232915` (21 files, 0.4 MB)
- M9: `C:\tmp\nm-target\gates\m9\20260930-233908` (8 files, 0.2 MB)

The inner runs made their own folders. M11's static step and its prior gates made
`m10\20260930-231458` and `m10\20260930-232651`, and `m9\20260930-231459`, `…-232651` and `…-232802`.
M10's static step made `m9\20260930-232916`.

## M11

- **Static and Rust, all PASS.** M10's static checks, which run M9's. The command inventory: 37 in
  each of the four places. The four lints, with theme.css at the frozen blob `dd3e89e2`. Typecheck.
  `npm test`: 146 of 146, checksum known answers 6 of 6 (`npm-test.log`). Harness `node --test`:
  42 of 42 (`e2e-lib-test.log`). The watcher's `-CheckOnly`. `cargo test -p app`: 55 passed. Clippy
  `-D warnings` on the app and on the core crates. `cargo fmt --all --check`. The fixtures, 1 passed.
- **Core crates.** 73 test binaries: 781 passed, 0 failed, 31 ignored, in 531 s, with
  `RUST_TEST_THREADS` 4. The run on `0f5cb12` had 72 binaries and 766 passed. The printed
  `NOT RUN here` targets are the three groups GATE.md lists.
- **Build and harness.** `app.exe` was rebuilt (exit 0 in 80 s) and `simpa.exe` built (exit 0). The
  msedgedriver matches WebView2 154.0.4258.37. All four private solvers verified by code sha256.
  The mesh-failure run: exit 4, stage mesh, FAIL, with `tetgen_skipped_facets`. The planted-loss run
  `20260930-232528-416-spps`: `simpa run` exit 0, OK, lost per band
  `125:150 250:0 500:1234 1000:7 2000:1000 4000:0`. `t1_cli.simpa` 6,491 B.
- **e2e.** wdio exit 0 in 80.8 s; `32 test(s); required 32; not passed: ; failures 0; skipped 0`.
- **`m11-b38` (T38-9)** passed in 0.2 s. Its receipts, as printed:
  - Results step: `{"codes":["SOLVER_BUILD_UNRECORDED"],"results":0,"run":"20260930-232528-416-spps","text":"Run 1 · Baseline\nSPPS · results checked before any value is shown\nUNVERIFIED\nResults unverified\nSOLVER_BUILD_UNRECORDED\nsolver_build_unrecorded\n…`
  - `run_results {"refusal":null,"run":"20260930-232528-416-spps","unverified":{"code":"solver_build_unrecorded",…,"ui_code":"SOLVER_BUILD_UNRECORDED"},"verified":false}`
  - `Runs row: data-verified unrecorded, data-build-code SOLVER_BUILD_UNRECORDED`
- **The verified path still renders.** The control in `m11-e-results` reads `OK` and
  `Results verified` for the box run. In `m11-sim-numbers` and `m11-sim-link`, the box's SPPS and TCR
  runs from the app read state `verified`.
- **Other gate receipts.** `m11-b-ipc` p99 1.60 ms. `m11-b-frames` p95 16.80 ms. `m11-c`: no
  `spps.exe` from the gate's copy at t0 + 2,003 ms. `m11-h`: 9 say-NO plants, each flagged under
  its own rule, and 60 views clean (162 diagnostic spans, 23 of them a non-zero loss, and 1,700
  verbatim lines). `m11-reload`: the run was taken back and Open refused `RUN_ACTIVE`.
- **Prior gates inside M11.** `m10.ps1 -SkipCore` exited 0 as a partial run whose checks all
  passed. `m9.ps1` printed `M9 PASSED`.
- **`m11-focus`.** `watcher stopped (stop_file) after 197297 ms: 0 foreground change(s), 0
  button-down(s), 0 key-down(s), 748 sample(s)`, 15 `app.exe` sessions, `m11-focus: PASS`.
- `files this run left in the repository (B:): 0`

## M10

Every static and Rust check PASS: M9's static checks, the inventory (M10's 28 commands named, 37
in all), the lints, theme.css, typecheck, `npm test` 146 of 146, the app crate's 55 tests, clippy,
fmt and the fixtures. The core crates: 73 test binaries, 781 passed, 0 failed, 31 ignored, in
501 s. The CLI fallback was staged with 0 copied, and the TetGen 1.6.0 reference was present. The
build: `app.exe` rebuilt, exit 0 in 29.5 s. The e2e: wdio exit 0 in 34.2 s; `27 test(s); required
13; not passed: ; failures 0; skipped 0`. Every `m10-*` id passed.

## M9

23 PASS, 0 FAIL, `M9 PASSED`. Among them:
- (c) the selftest: 22 of 22 UI checks; renderer ANGLE on the NVIDIA GeForce RTX 5070;
  `single_16MB_ms` 94.1.
- (d) the panic probe.
- (h) the five steps.
- (e) the bindings. `schema.json`, `ipc.json`, `schema.ts` and `ipc.ts` regenerated from the
  release `app.exe` to the same blobs (`6ebd736b7a55`, `ab5eb5950ffe`, `e01ff5ac0f4b`,
  `66ce4c130938`), and `git diff` exited 0. GREEN.md had checked the bindings against the debug
  app's dump only. This settles that on the release build.

## Before and after

| | Before (23:11-23:14) | After (23:40) |
|---|---|---|
| C: free | 18.48 GB | 18.14 GB (lowest seen during the run: 18.14 GB) |
| Main checkout, untracked | 3: `session-logs/LEAVINGS-2026-09-30-1030.md`, `-1936.md`, `-2152.md` | the same 3 |
| Worktree, untracked | 0 | 0 |

**No new untracked file in either tree.**

**Finding: one tracked file reads modified in the worktree after the run.** `git status
--porcelain` lists ` M app/src-tauri/Cargo.toml`, and before the run it listed nothing. The file's
bytes are identical to its HEAD blob (648 B, LF endings; `cmp` against `git show
HEAD:app/src-tauri/Cargo.toml`). `git diff` shows no change, only the warning "LF will be
replaced by CRLF the next time Git touches it". Its mtime is 23:24:08, 2 s after M11's `npx tauri
build --no-bundle` began (`tauri-build.log` created 23:24:06). The builds in M10 and M9 did not
change the mtime again. The gate counts only untracked files as "left on B:", so it does not see
this. The file was left as it is.

After the run, no `app.exe`, `spps.exe`, `tetgen.exe`, `preprocess.exe`, `classicalTheory.exe`,
`tauri-driver.exe`, `msedgedriver.exe`, `simpa.exe`, `cargo.exe` or `node.exe` was running (23:40).

## Check-ins

As PLAN.md's "Timing and check-ins" asks, the run appended to `C:\tmp\b38-39\progress.log` at each
gate's start and end, with check-ins in between (23:14, 23:15, 23:24, 23:29, 23:31, 23:39, 23:40).
Each line said whether the run was on plan and what the results so far were. The consoles were
read every 9 minutes or sooner, and a watcher reported every top-level line and the free space on C:.

After the run, a PreToolUse hook refused one read-only count: its drive-root guard read `awk -F:`
as the drive `F:`. The count was not retried in another form. The counts above are the gates' own
printed lines.
