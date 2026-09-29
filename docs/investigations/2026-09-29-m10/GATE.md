# M10 gate: passed

2026-09-29, Grace, branch `m10` at **`1024de6`**, with a clean tree. Built to `PLAN.md` on the
foundation (`13d7d54`, `9fabfe5`), with the three packages committed as `b5c9133` (viewport),
`d1aad23` (materials) and `bbd0796` (scene), and the integration as `1024de6`.

| Run | When | Result |
|---|---|---|
| `powershell -File tools/gates/m10.ps1`, bare (Windows PowerShell 5.1: `pwsh` is not installed, although the gate text says `pwsh`) | 10:40:56-10:53:56 | **M10 PASSED**, exit 0 |
| `powershell -File tools/gates/m9.ps1 -TargetDir C:\tmp\nm-target` | 10:54:05-10:54:48 | **M9 PASSED**, exit 0 |

## The gate text against the tests

| Gate (`rebuild-plan-raw-2026-09-23.json`, M10) | Test id (spec) | Receipt from the passing run's `wdio.log` |
|---|---|---|
| (a) raw `elmia.ply`: a Console FAIL line with 'open' and '955' | `m10-a-console` (shell) | `FAIL Model check refused elmia.ply: open_boundary: 955 open edges in the census (953 on the outer shell after analysis), 1085 faces with the exterior on both sides. ...`. Control: the teaching room adds no FAIL line |
| (a) the viewport hook reports more than 0 highlighted faces | `m10-a-highlight` (viewport) | `raw hall, 1086 faces uploaded to the check-highlight overlay`; the chip reads `FAIL · 1086 faces highlighted`. Control: the teaching room gives 0 and no chip |
| (a) the Run control is disabled | `m10-a-run` (scene) | raw hall `data-blockers = GEOMETRY_REFUSED MATERIALS_UNASSIGNED SOURCE_NONE M11_PENDING`. Control: the teaching room reads exactly `M11_PENDING`, so "disabled" is not vacuous |
| (b) the INFO line 'Closed volume, 0 self-intersections' | `m10-b-console` (shell) | `INFO Closed volume, 0 self-intersections · 7860 faces, 10 surface groups` |
| (b) the step bar shows Materials '0 / 10' | `m10-b-materials` (scene) | corrected hall `'0 / 10'`. Control: `tutorial1_box.simpa` `'3 / 3'` |
| (c) pasted `materials_6x6.tsv` saves byte for byte as `materials_6x6.expected.json` | `m10-c` (materials) | the saved `c.simpa` equals the expected file; control first: the committed start file differs from it |
| (d) double-click the box ceiling: 2 faces, group Ceiling | `m10-d` (viewport) | `{"faces":[10,11],"groups":["Ceiling"]}`. Control: a wall double-click gives `{"faces":[2,3],"groups":["Walls"]}`, 2 faces and not the group's 8 |
| (e) a receiver outside shows RECEIVER_OUTSIDE, project unchanged | `m10-e-outside` (scene) | `[data-issue-code="RECEIVER_OUTSIDE"]` shown; `projectJson()` and `undoDepth()` equal before and after. Control: x = 4 accepted, one undo step |
| (e) the label 'a/b' is rejected with LABEL_UNSAFE | `m10-e-label` (scene) | `[data-issue-code="LABEL_UNSAFE"]` shown, project unchanged. Control: 'Front row' accepted |
| (f) 50 scripted edits then 50 undos: saved file byte-identical | `m10-f` (shell) | 50 applied edits (undo depth 1 to 50), one refused edit in the middle that does not move it, 50 real Ctrl+Z key events, the saved file equals `teaching_room.simpa`. Control: after 49 undos the file differs |
| (g) `document.querySelectorAll('canvas').length == 1` | `m10-g` (viewport) | one canvas at each of 13 points: at start, after the corrected hall, on all five steps, in both views, on all three dock tabs, after the box |
| (h) **not in the gate text**, added by PLAN.md from the milestone rules: no solver-computed acoustic number on screen | `m10-h` (shell) | every step and dock tab, on the teaching room and the hall: 0 matches outside `[data-input]` and `[data-geometry]`, and no digit in the Acoustics panel |

The 14 tests without an id are the packages' extra checks (PLAN.md 3). The verdict requires them
too: 27 tests, 0 failed, 0 skipped.

## Output of the passing run

```
      PASS  (g) 0 non-async #[tauri::command]
      PASS  every command body runs through guard::blocking (except panic_probe_unguarded)
      PASS  (f) capabilities: 0 'shell:' and 0 'fs:'; dialog is the only plugin
      PASS  strict CSP: 'self' only, no unsafe-inline or unsafe-eval, no remote origin
      PASS  WebView2-specific code only in src/webview2.rs
PASS  M9 static checks (m9.ps1 -Only static -TargetDir C:\tmp\nm-target)
      attributes    28 command(s)
      handler       28 command(s)
      build.rs      28 command(s)
      capabilities  28 command(s)
PASS  command inventory: 28 commands, the same set in the attributes, generate_handler!, build.rs and capabilities
      45 UI files read; 0 call(s) outside the three
PASS  lint: backend is called only from actions.ts, selftest.ts and App.tsx (boot)
PASS  lint: projectApply (M9's unchecked apply) only in selftest.ts
PASS  lint: a canvas is created only under features/viewport/ and in gpu.ts
      theme.css blob dd3e89e270604ee648944c854c68ba3a2b51d331; frozen blob dd3e89e270604ee648944c854c68ba3a2b51d331
PASS  lint: theme.css unchanged since the M10 foundation (git blob)
PASS  UI: tsc typecheck (app and its node --test suites)
      checksum known answers: 6 of 6 passed
PASS  UI: npm test (checksum known answers; node --test ui/src/**/*.test.ts)
      test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s
PASS  app crate: unit tests
PASS  app crate: clippy -D warnings
PASS  app crate: rustfmt --check
      test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
PASS  fixtures: tests/fixtures/ui equal their recipe (cargo test -p simpa-core --test ui_fixtures)
      CLI fallback staged at C:\tmp\nm-target\target\solvers\bin: 4 exe(s), 0 copied now
      NOT RUN here: simpa-core dump_helpers, gabe_golden, pbin_golden, poly_golden, tetgen_golden :: build the oracle into <repo>\target\oracle (tests/common/paths.rs oracle())
      NOT RUN here: simpa-core config_xml_write, parity_inputs :: scratch folders under <repo>\target\test-runs (config_xml_support.rs fresh_run_dir)
      NOT RUN here: simpa parity_tutorials :: solver runs under <repo>\target\parity-bed
      NOT RUN here: simpa-core lib: run::manager logs_that_cannot_be_created_are_launch_failed :: folders under <repo>\target\tmp
      NOT RUN here: simpa-core mesh_project: every_failure_code_fires_on_its_input :: HANGS with scratch on NTFS (C:): it expects a read-only stale file to survive deletion, which holds on exFAT but not on NTFS with this toolchain, so the fake mesher that must not run panics and the binary never ends (measured 2026-09-29, M10 foundation)
      solvers C:\tmp\nm-m10-solvers\bin; upstream B:\repos\I-Simpa-upstream; scratch C:\tmp\nm-target\test-scratch
      71 test binaries: 726 passed, 0 failed, 23 ignored, in 708 s; logs C:\tmp\nm-target\gates\m10\20260929-104056\cargo-test-core.log, C:\tmp\nm-target\gates\m10\20260929-104056\cargo-test-cli.log
PASS  core crates: cargo test -p simpa-core -p simpa (--no-fail-fast)
      held after 0 s
PASS  e2e lock (C:\tmp\nm-e2e\e2e.lock)
      - Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.
         Compiling app v0.1.0 (B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m10\app\src-tauri)
          Finished `release` profile [optimized] target(s) in 26.06s
             Built application at: C:\tmp\nm-target\release\app.exe
      exit 0 in 28.9 s; app.exe rebuilt: True
PASS  build: npx tauri build --no-bundle (custom protocol, ui/dist embedded)
      C:\Users\Burhan\.cargo\bin\tauri-driver.exe (tauri-driver v2.1.0:)
PASS  harness: tauri-driver present
      WebView2 runtime pv 154.0.4258.37; Microsoft Edge WebDriver 154.0.4258.37 (f6ddc7bb8d2ad05a6979ebc09b83893c2d128a62)
PASS  harness: msedgedriver matches the WebView2 runtime exactly
      current: lock sha256 76cb093cb9b13282
PASS  harness: WebdriverIO installed from app/e2e/package-lock.json (by its sha256)
PASS  harness: the e2e config and specs typecheck against it
      B:\repos\I-Simpa-upstream\src\isimpa\resources\doc\tutorial\tutorial 2\elmia.ply
PASS  harness: the raw hall is present (gate a)
      exit 0 in 34.8 s; log C:\tmp\nm-target\gates\m10\20260929-104056\wdio.log
PASS  e2e: wdio run app/e2e/m10.conf.ts (-Spec smoke,shell,viewport,materials,scene)
      e2e lock released
      passed      0.3 s  m10-smoke        m10 smoke the built app launches and shows the five steps
      passed      0.2 s  m10-a-console    m10 a console the raw hall shows a FAIL line naming the open boundary and its 955 census edges
      passed      0.1 s  m10-b-console    m10 b console the corrected hall shows the INFO line Closed volume 0 self intersections
      passed      6.2 s  m10-f            m10 f 50 edits then 50 Ctrl Z leave the saved file byte identical
      passed      2.0 s  m10-h            m10 h no solver computed acoustic number is shown on any step or dock tab
      passed      0.2 s  m10-a-highlight  m10 a highlight the refused faces of the raw hall are highlighted
      passed      1.0 s  m10-d            m10 d a double click on the box ceiling selects its 2 faces group Ceiling
      passed      5.5 s  (no id)          viewport a pick after the camera orbits maps to the right project face
      passed      1.0 s  (no id)          viewport the Plan tab is a top orthographic view and picks through it
      passed      3.1 s  (no id)          viewport a placement click puts a receiver 1 2 m and a source 1 5 m above the floor
      passed      0.8 s  m10-g            m10 g document querySelectorAll canvas length 1 throughout
      passed      0.3 s  m10-c            m10 c the pasted 6x6 block saves byte identical to materials 6x6 expected json
      passed      0.4 s  (no id)          materials Ctrl C copies the exact shortest round trip values and pasting them back is exact
      passed      0.4 s  (no id)          materials row fill copies the focused cell across its row as one undo step Ctrl R and the button
      passed      0.5 s  (no id)          materials 1 5 on a used material is refused as MATERIAL VALUE OUT OF RANGE and 0 5 as NOT A NUMBER the project unchanged
      passed      0.1 s  (no id)          materials tutorial1 box s third octave band headers read 50 20k in ascending order
      passed      0.4 s  (no id)          materials a paste with a header row maps its columns by frequency
      passed      0.2 s  m10-a-run        m10 a run Run is disabled and why is named
      passed      0.2 s  m10-b-materials  m10 b materials the step bar reads Materials 0 10 on the corrected hall
      passed      0.5 s  m10-e-outside    m10 e outside a receiver placed outside shows RECEIVER OUTSIDE and changes nothing
      passed      0.6 s  m10-e-label      m10 e label the label a b is refused with LABEL UNSAFE
      passed      0.3 s  (no id)          scene the step subs read the project closed refused no model assigned sources receivers
      passed      0.3 s  (no id)          scene the dirty dot follows edits saves and undo
      passed      0.8 s  (no id)          scene a variant is added made active switched and renamed
      passed      0.5 s  (no id)          scene the filter narrows the list by name and by material
      passed      0.4 s  (no id)          scene every scroll container has the themed scrollbar colours
      passed      0.2 s  (no id)          scene the import dialog opens on m and z shows every choice and imports
      27 test(s); required 13; not passed: ; failures 0; skipped 0
PASS  verdict: every required id passed, 0 failures, 0 skipped

work folder: C:\tmp\nm-target\gates\m10\20260929-104056 (21 files, 0.4 MB)
M10 PASSED
exit 0
```

## What the integration found and changed

1. **The first e2e run of all five specs (10:18): 26 of 27 passed.** The one failure was the viewport
   package's extra check "an orbit keeps the target", which compared the target for exact
   equality. It moved by one or two ulps (3.0000000000000004 to 3.000000000000001):
   `OrbitControls.update()` re-clamps the target about its cursor on every call
   (`target.sub(cursor).clampLength(min, max).add(cursor)`, `OrbitControls.js:771-773`). The
   check now bounds the drift at 1 nm; a pan moves it by metres. No gate id was involved.
2. **The core crates' check fails without a code change, on a harness cause.** Only
   `cli_run::a_passing_test_leaves_no_scratch_behind_and_a_failing_one_keeps_its_folder` fails. Its
   failing arm points `$SIMPA_SOLVERS_DIR` at an empty folder. It expects the run to get as far as
   the test's own `preprocess.exe` lookup, which needs the CLI's last fallback: the nearest
   `<ancestor>\target\solvers\bin` above `simpa.exe` (`run::manager` `candidates`). With cargo
   building into `C:\tmp\nm-target`, no such folder exists.
   - FOUNDATION.md also lists `a_run_whose_preprocess_gives_up_records_the_warning` as failing.
     That entry is this test's child run, whose `test ... FAILED` line the gate's grep picked up
     from the parent's output. The test passes when run by itself.
   - The fix: the gate copies the solver build to `<target>\target\solvers\bin`, the same place
     relative to the exe as the repo's own build. No test changed and none is skipped for it:
     `cli_run` 13 of 13, and the core crates 726 passed, 0 failed.
   - With no `-SolversDir` and no repo build, the gate falls back to the foundation's copy,
     `C:\tmp\nm-m10-solvers\bin`, so the gate runs bare.
3. **The materials package asked for a shared change.** `actions.ts` now passes `backend`'s
   `asCmdError` on, and `inline.tsx` re-exports it as `errorOf` instead of keeping a copy
   (PLAN.md 2.4, rule 7).
4. **Invisible characters in the specs.** The WebDriver key constants in the shell and scene specs
   were literal private-use characters (U+E009 and others), which show as `''` in an editor. They
   are now `'\uE009'` escapes, as the materials spec already wrote them.
5. **`-Spec screens`** is a spec with no gate id. It saves the screenshots below into
   `-ScreensDir`. It is not in the default spec list, so it never affects "M10 PASSED".
6. **PLAN.md 2.5's hook table** gains the five hooks the packages added: `faceClientPoint`,
   `cameraState`, `materialsGrid`, `materialsCopy` and `openImportDialog`.

No other shared file changed. The scene package's `MenuBar` imports `setView` and `frameModel`
from `features/viewport/engine.ts`, and both exist under those names.

## M9

`m9.ps1` passed in full on `1024de6` (above), and at 10:40 on the tree before the integration
commit.

The first full run at 10:37 failed one check, "app crate: unit tests". It printed no
`test result` line, and `m9.ps1` keeps only lines matching `test result|FAILED|panicked`, so the
cause was not captured. `cargo test -p app` run directly straight after passed 41 of 41, and the
next two full runs passed.

**Unexplained, and not reproduced.** If it comes back, the first step is to keep cargo's whole
output in a log, as `m10.ps1` does.

## Screenshots (`screens/`, 5 PNG, 1.3 MB)

Taken by `m10.ps1 -Only e2e -Spec screens` against the release app, at the window size
tauri-driver gives it:
- `1-geometry-hall.png`: the corrected hall, check passed, Materials `0 / 10`;
- `2-geometry-raw-hall.png`: the raw hall refused, 1086 faces highlighted, FAIL rows;
- `3-materials.png`: the teaching room with Rear wall selected;
- `4-sources.png`: S1 selected;
- `5-sources-refused.png`: R1 at x = 20, with `FAIL RECEIVER_OUTSIDE` inline and the project
  unchanged.

## Files

**On B:, in this integration:**
- 5 PNG;
- 1 spec, `m10.screens.e2e.ts`;
- this file.

The packages had added 25 files in `app/ui/src` and changed 17 there, plus their 3 specs. An
18th changed file there, `actions.ts`, is the integration's.

**On C:** the gate work folders under `C:\tmp\nm-target\gates\m10\` (21 files for the passing run),
and 4 exe staged in `C:\tmp\nm-target\target\solvers\bin`.

## Open, for Burhan

**Checks still owed:**
- **One manual paste from Excel at hand-over (PLAN.md 9).** The e2e sends synthetic `paste` and
  `copy` events, so it exercises the grid's handlers and not the OS clipboard.
- **The core targets the gate does not run on a B: worktree.** It prints them on every run:
  - the oracle goldens, `config_xml_write`, `parity_inputs` and `parity_tutorials`;
  - one `run::manager` unit test;
  - the `mesh_project` test that hangs on NTFS.

  They write under `<repo>\target` or depend on exFAT. The fix is a core change, outside M10
  (FOUNDATION.md).

**Not built:**
- **A9, the save prompt.** File › New project, Open and closing the window discard unsaved work
  without asking. It was listed in PLAN.md 7.3 as worth adding.

**Choices the packages made, each open to change:**
- **Viewport:**
  - a placement rounds to 1 mm;
  - there is a Place source tool;
  - a marker click selects the marker;
  - a click on empty space clears any selection;
  - the Plan view has a fixed fit.
- **Materials:** a new material gets 0.1 in every band, specular, double-sided.
- **Scene:**
  - a row click also switches step;
  - + Source and + Receiver add the point at the bounding-box centre;
  - removing the only source is refused `SOURCE_NONE`;
  - the Units row reads "chosen at import" when the project has no saved path.

**Questions still open from PLAN.md section 8:**
- F1, whether the core should refuse `Default`;
- which parity items of 7.2 and 7.3 fold into M10;
- the UI codes;
- `m10-h` as part of the gate.

**PLAN.md 6.4's integration items not done here:**
- a critic;
- the updates to `docs/rebuild-plan.md`, `docs/scope.md` and the decision log for section 8's
  calls;
- the session summary in `session-logs/` before the final push.
