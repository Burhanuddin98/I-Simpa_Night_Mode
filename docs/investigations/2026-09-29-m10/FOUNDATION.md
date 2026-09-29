# M10 foundation: what the packages build on

2026-09-29, branch `m10`. Built to `PLAN.md` section 6.0. This page lists what exists now, where
it departs from the plan, and the receipts. Package owners read "What a package gets" before they
start.

## What a package gets

**Commands** (`app/src-tauri/src/commands.rs`). The 9 of PLAN.md 1.2 exist. Each is async and
runs through `guard::blocking`. Each is registered in the attributes, `generate_handler!`,
`build.rs` and `capabilities/default.json`: 28 commands in all four places.

**Session** (`bridge.rs`, `scene.rs`). It holds everything in PLAN.md 1.3 to 1.11:
- the checked apply, with issue identity by entity and field;
- the dirty serials and `geometry_rev`;
- the check and issue caches;
- the pending lines, the placeholder rule, the exhaustive `ui_code` table and the mesh buffer.

`run_blockers` holds `GEOMETRY_REFUSED`, then `MATERIALS_UNASSIGNED`, then each validator
error's UI code once, then `M11_PENDING`. The plan names only the three app codes; the validator
errors are added because they block a run too. On the teaching room the list is exactly
`M11_PENDING`.

**UI modules** (`app/ui/src/`):

| Module | What it holds |
|---|---|
| `store.ts` | `sceneStore`, `meshStore`, `stepStore`, `selectionStore`, `toolStore`, `refusalStore`, `busyStore`, `importRequestStore`, `viewportStore`, plus M9's `consoleStore`, `statusStore` and `log`. **`projectStore` is gone**: read `sceneStore.get()?.info` |
| `actions.ts` | The only caller of the M10 commands: `newProject`, `openProject`, `openPath`, `openDialog`, `importModel`, `save`, `saveAs`, `apply(op, fieldKey?)`, `undo`, `redo`, `placeAt`, `projectJson`, and `fire` for UI handlers |
| `ops.ts` | The builders of PLAN.md 2.2, plus `nextName`, `newReceiver`, `newSource`, and `opText` |
| `numbers.ts`, `mesh.ts`, `issues.ts` | Pure modules, each with a `node --test` suite beside it |
| `testhooks.ts` | `registerHook(name, fn)`, which returns the unregister function; `idle()`; and the foundation's hooks from PLAN.md 2.5 |

**Slots.** `App.tsx` mounts every region. None takes props; each reads the stores.

| Package | Slot |
|---|---|
| viewport | `features/viewport/Viewport.tsx` |
| materials | `features/materials/MaterialsPanel.tsx`, mounted by `chrome/PropertiesPanel.tsx` on the Materials step |
| scene | `chrome/*` |

Each stub is M9's working code, moved.

**CSS.**
- `theme.css` holds the tokens, the base rules, the shared `.segmented` rule, the layout, the
  field and `.issue` rules, and the themed scrollbars. It is frozen: `m10.ps1` compares its git
  blob.
- The region rules moved to `chrome/chrome.css` and `features/viewport/viewport.css`.
- Materials adds `features/materials/materials.css`.

**For the viewport package in particular:**
- Set `viewportStore` to `{ live: true, drawnRev }` after each frame. `idle()` waits for the
  drawn revision only once `live` is set.
- `features/viewport/libraries.ts` references the three.js and three-mesh-bvh classes, so the
  release bundle already holds them. Keep it or replace it.

## Where this departs from PLAN.md

- **`@types/node` 24.19.0 added**, exact, as a dev dependency. The `node --test` suites are
  type-checked by a second config, `ui/tsconfig.test.json`. The app's own config excludes
  `*.test.ts`, so browser code never sees Node's globals. `npm run typecheck` runs both configs.
- **`store.ts`'s `Store` no longer uses a parameter property.** `erasableSyntaxOnly` refuses one.
- **A smoke spec, `m10.smoke.e2e.ts` (id `m10-smoke`), is added.** It launches the release app,
  reads the step bar, and checks that the hooks are installed. `m10.ps1 -Spec` takes `smoke`
  besides the four specs of the plan.
- **`app/e2e/tsconfig.json`** type-checks the harness against the wdio install on C:, through
  `typeRoots`. The paths are Grace-local, like the harness itself.
- **`m9.ps1`**:
  - the three harness fixes of PLAN.md 4.5;
  - two strings three.js brings into `dist`, each read in context before it was listed:
    - `http://www.w3.org/1999/xhtml`, an XML namespace in `createElementNS`;
    - `https://jcgt.org/published/0007/04/01/`, a citation in a GLSL comment of
      `PMREMGenerator`.
- **The core crates' tests in `m10.ps1` run with `--no-fail-fast`, on the C: paths below, minus
  the targets that write under `<repo>\target`.** A worktree on B: would put that output on the
  exFAT drive. The gate prints the targets it leaves out on every run:
  - the five oracle golden targets;
  - `config_xml_write` and `parity_inputs`;
  - `parity_tutorials`;
  - one `run::manager` unit test.

  Fixing that means moving those writers to `SIMPA_TEST_SCRATCH_ROOT`, a core change outside
  M10.
- **The misread cell of `materials_6x6.tsv` sits at 0.40, not near 0.70.** Above 0.5 no double
  needs a 17-digit spelling, and at 0.35 serde_json reads every 17-digit decimal right.
  `ui_fixtures.rs` and `tests/fixtures/ui/PROVENANCE.md` give the reasoning.

## Receipts (2026-09-29, Grace)

**Files created:**

| Location | Files |
|---|---|
| `app/node_modules` (worktree, B:) | 3,473 at `npm ci`; 6,031 after three, three-mesh-bvh, @types/three and @types/node (841 MB) |
| `ui/dist` | 10 |
| `tests/fixtures/ui` | 4 |
| `C:\tmp\nm-e2e\node\node_modules` (wdio 9.32.0) | 13,215 |
| `C:\tmp\nm-e2e\msedgedriver\154.0.4258.37` | 3 |
| `C:\tmp\nm-m10-solvers` | 5: the solver build, copied from the `t3-proj` worktree for the core tests |

Nothing from wdio or msedgedriver went to B:.

**Tools:**
- tauri-driver v2.1.0, in `%USERPROFILE%\.cargo\bin`;
- msedgedriver 154.0.4258.37, which matches the WebView2 `pv`.

**Checks:**

| Check | Result |
|---|---|
| `cargo test -p app` | 41 passed |
| clippy `-D warnings` and `fmt --check` (app) | clean |
| `ui_fixtures` | passes |
| `npm run typecheck` | clean |
| `npm test` | checksum known answers, 6 of 6; `node --test`, 12 of 12 |
| e2e, the smoke and shell specs, run by hand against the release app | `m10-smoke`, `m10-a-console`, `m10-b-console`, `m10-f` and `m10-h` pass. The raw hall's FAIL line reads `open_boundary: 955 open edges in the census (953 on the outer shell after analysis), 1085 faces with the exterior on both sides.` |

The package ids fail as pending, by design.

**`m10.ps1 -SolversDir C:\tmp\nm-m10-solvers\bin`, in full, 09:34-09:43.** Two checks failed.

- **Passed:** every static check, the Rust checks, the fixtures, the build, and the harness
  prerequisites.
- **Failed, the core crates.** 725 passed, 2 failed, 23 ignored. These are the two environment
  failures below.
- **Failed, the verdict.** On the rerun, which reads the ids correctly, the five foundation ids
  pass and the eight package ids fail as pending.
- **`-Only e2e -Spec smoke,shell`:** a partial run, passed.
- **The UI lints say no:**
  - a probe file that called `backend.projectApply` and created a canvas failed all three lints;
  - then it was removed.

**`m9.ps1 -TargetDir C:\tmp\nm-target`, in full.** Every check passed except (e), and (e)
differed only because the regenerated `ipc.json` and `ipc.ts` were not yet committed. The
regenerated blobs equalled the working tree.
- (c) passed: the self-test, the NVIDIA renderer, and the IPC numbers.
- (f) passed with the two new three.js strings.
- (h) passed.

After the foundation commit (`13d7d54`), `m9.ps1` in full printed **M9 PASSED** (09:47, 40 s).

## The core crates' tests: what fails in this environment

The core crates have no change on this branch except the new `tests/ui_fixtures.rs`.

**The run.**
- `SIMPA_SOLVERS_DIR` and `SIMPA_UPSTREAM` set, scratch on C:, `--no-fail-fast`, minus the
  targets that write under `<repo>\target`.
- 725 passed, 2 failed, 23 ignored, 1 hung. The hung test was stopped by hand.

**What fails, and why:**

| Test | What happens | Cause |
|---|---|---|
| `mesh_project::every_failure_code_fires_on_its_input` | hangs | It expects a read-only stale file to survive deletion (`STALE_DELETE_FAILED`). On NTFS the file was deleted: its folder shows it replaced. So the fake mesher that must not run panicked, and the binary never ended. |
| `cli_run::a_run_whose_preprocess_gives_up_records_the_warning` | fails | The CLI finds solvers through `<exe ancestor>\target\solvers\bin` (`run::manager`, `candidates`). |
| `cli_run::a_passing_test_leaves_no_scratch_behind_and_a_failing_one_keeps_its_folder` | fails | The same solver lookup. |

**Why the two `cli_run` tests fail.** Cargo builds into `C:\tmp\nm-target`, as the corrupt-B:
rule requires, so no ancestor of the test exe holds `target\solvers\bin`, and the CLI fails with
"classicalTheory.exe not found".

**What `m10.ps1` does about it.**
- It skips the hanging test by name and prints why, so a gate run cannot hang.
- The two `cli_run` failures stay in, and they fail the core check. The fix is a core change,
  outside M10:
  - the scratch writers and the oracle move to `SIMPA_TEST_SCRATCH_ROOT`;
  - the stale-file test gets a way to hold a file on NTFS;
  - the CLI tests' solver fallback no longer depends on where cargo builds.
