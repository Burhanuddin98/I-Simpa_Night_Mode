# M10 review: gate honesty and design fidelity

2026-09-29, Grace, branch `m10` at `8030e88` (the integrator's GATE.md receipt; the code is `1024de6`).
The tree was clean before the review. After it, no tracked file differs (`git diff --stat` is
empty), and the branch is still up to date with `origin/m10`. No code was changed. The only new,
untracked files are this one and `review-screens/`, which holds 8 PNG.

## Verdict

- **A, gate honesty.** Each gate check (a) to (g) fails when the behaviour it claims to test is
  broken. There were 20 mutation runs, each reverted by `git checkout -- <file>` on the mutated file
  only.
  - **One gate check still passes when it should not.** `m10-a-highlight` passes with the highlight
    drawn invisible (A2b). It counts the faces uploaded to the overlay, not what is drawn.
  - **So does the plan's extra check (h).** `m10-h` passes with unitless acoustic numbers such as
    `STI 0.62 · D50 0.45` in the chrome (H2). No such number exists in M10.
  - **After the reverts,** all five gate specs plus `screens` pass: 30 tests, 0 failed, 0 skipped.
    `app.exe` was rebuilt from the clean tree.
- **B, design fidelity.** The chrome follows Concept B closely:
  - all 18 colour tokens equal the design's hex values;
  - every region measures the same at 1440 × 900;
  - there is no logo and no wordmark;
  - there is no serif or italic type anywhere in the DOM;
  - every scroll container has the themed scrollbar colours.

  I found 25 deviations. Most are M10's scope: Run is unwired, there are no computed numbers, and
  the materials grid replaces the α strip. Two need a decision from Burhan:
  - **the scrollbar hover.** The design's red hover thumb does nothing in WebView2 (item 25);
  - **the Materials panel layout** (item 20).

## A. Gate honesty

**Method.** For each check:
1. One temporary mutation that should break it.
2. `powershell -File tools/gates/m10.ps1 -Only e2e -Spec <spec>`. This rebuilds `app.exe`
   (`tauri build --no-bundle`) under the e2e lock and reads the verdict from the junit files.
3. `git checkout -- <file>` on the mutated file, then `git status --short` empty before the next
   mutation.

Each run took 33 to 83 s. A core-crate mutation takes the longest, because `simpa-core` recompiles.

| # | Check | Mutation (uncommitted, reverted) | Spec | Result | Failing assertion, as the verdict printed it |
|---|---|---|---|---|---|
| A1 | (a) Console FAIL line | `app/src-tauri/src/scene.rs:499` the refused check's lines `LineClass::Fail` → `Warn` | shell | **FAILED** `m10-a-console` | `no FAIL line with 'open' and '955' among: WARN Model check refused elmia.ply: …`; b-console, f, h passed |
| A2a | (a) highlighted > 0 | `features/viewport/engine.ts:275` hook `highlightedFaceCount` returns 0 | viewport | **FAILED** `m10-a-highlight` | `highlightedFaceCount() = 0` |
| A2b | (a) highlighted > 0 | `engine.ts:208` highlight overlay material `opacity: 0.55` → `0` (faces uploaded, drawn invisible) | viewport | **PASSED: should have failed** | none: 6 of 6 passed, "its checks passed" |
| A3 | (a) Run disabled | `chrome/RunButton.tsx:15` `disabled` → `disabled={false}` | scene | **FAILED** `m10-a-run` | `Run is disabled on the raw hall: true !== false` |
| A3b | (a) Run disabled for the right reason | `scene.rs:565` `GEOMETRY_REFUSED` dropped from `run_blockers` (Run still disabled by the others) | scene | **FAILED** `m10-a-run` | `data-blockers: MATERIALS_UNASSIGNED SOURCE_NONE M11_PENDING` |
| B1 | (b) Console INFO line | `scene.rs:447` the passing check's line `Info` → `Ok` | shell | **FAILED** `m10-b-console` | the line arrived as `OK Closed volume, 0 self-intersections · 7860 faces…` |
| B2 | (b) Materials '0 / 10' | `scene.rs` `group_assigned` counts the placeholder `Default` as assigned | scene | **FAILED** `m10-b-materials` | `'10 / 10'` for `'0 / 10'`; the import-dialog extra check failed on the same sub |
| C1 | (c) byte-identical save | `tests/fixtures/ui/materials_6x6.expected.json` one byte: `…726515` → `…726516` | materials | **FAILED** `m10-c` | `c.simpa (6797 B) differs from …expected.json (6797 B) at byte 3482` |
| C2 | (c) byte-identical save | `features/materials/paste.ts:221` pasted values stored as `Math.fround` (f32) | materials | **FAILED** `m10-c` | `differs … at byte 2365: [0.019999999552965164…`; 3 extra materials checks failed too |
| D1 | (d) ceiling → 2 faces, Ceiling | `engine.ts:860` double-click keeps only the first coplanar face | viewport | **FAILED** `m10-d` | `faces: [10]` for `[10, 11]` |
| D2 | (d) | `engine.ts:860` double-click takes the whole surface group | viewport | **FAILED** `m10-d`, **at the wall control only** | `{"faces":[2,3,4,5,6,7,8,9],"groups":["Walls"]} 8 !== 2`. The ceiling half passed |
| D3 | (d), is the aim hook circular? | `features/viewport/pick.ts` `firstFace` returns `faceIndex ^ 1` | viewport | **FAILED** `m10-d` | `aimAtFace found a point where the first hit is the ceiling: null`; 3 extra viewport checks failed too |
| E1 | (e) RECEIVER_OUTSIDE | `crates/simpa-core/src/validate/project.rs:341` receiver-outside issues dropped | scene | **FAILED** `m10-e-outside` | `[data-issue-code="RECEIVER_OUTSIDE"] still not displayed after 30000ms` |
| E1b | (e) project unchanged | `app/src-tauri/src/bridge.rs:469` a refused edit is committed anyway; the refusal is still shown | scene | **FAILED** `m10-e-outside` and `m10-e-label` | `the project is unchanged` (projectJson differs) |
| E2 | (e) LABEL_UNSAFE | `validate/project.rs:716` the file-name check skipped | scene | **FAILED** `m10-e-label` | `[data-issue-code="LABEL_UNSAFE"] still not displayed after 30000ms` |
| F1 | (f) 50 undos byte-identical | `crates/simpa-core/src/schema/ops.rs:745` the inverse of `Rename` keeps the new name | shell | **FAILED** `m10-f` | `f.simpa (6764 B) differs from teaching_room.simpa (6755 B) at byte 3711 … "Velour curtain"` |
| G1 | (g) one canvas | `engine.ts:506` a hidden extra `<canvas>` appended on every model load | viewport | **FAILED** `m10-g` | `7 canvases at start`; the Plan-tab extra check `6 !== 1` |
| H1 | (h) no acoustic number | `chrome/StatusBar.tsx` adds `T30 1.8 s` | shell | **FAILED** `m10-h` | `"8 s" in the text outside [data-input] and [data-geometry]` |
| H2 | (h) | `StatusBar.tsx` adds `STI 0.62 · D50 0.45` | shell | **PASSED: should have failed** | none: 4 of 4 passed |
| V1 | the verdict | `app/e2e/specs/m10.scene.e2e.ts` `it.skip` on `m10-e-label` | scene | **FAILED** verdict | `not passed: m10-e-label; failures 0; skipped 1` |

**Every failure named its own assertion.** The other tests in the same spec passed, except where
the table says otherwise. So each failure is attributable to its mutation and not to the
environment.

**The post-revert control run** was
`m10.ps1 -Only e2e -Spec smoke,shell,viewport,materials,scene,screens`, from 11:20:45 to 11:21:51.
It gave 30 tests, 0 failed and 0 skipped, including the 13 required ids. It also leaves the shared
`C:\tmp\nm-target\release\app.exe` built from the clean tree, not from a mutant.

### Findings

1. **`m10-a-highlight` does not see the highlight (A2b).** Severity: major.
   - **What it reads.** `highlightedFaceCount()` is the overlay's index count
     (`engine.ts:586`). The chip's `FAIL · n faces highlighted` is rendered from the same number.
   - **What passes it.** Anything that hides the overlay: its opacity, `visible`, its colour, its
     depth or render order, or its removal from the scene.
   - **Against the gate text.** The gate text asks for the hook count, so the gate is met as
     written. The claim "the refused faces are highlighted" is what goes untested.
   - **Fix.** Read the canvas colour at a highlighted face's `faceClientPoint`, with the overlay
     and without it (a WebDriver element screenshot is enough). At least, assert that the overlay
     is in the scene, visible, with opacity > 0.
2. **`m10-h` sees only numbers with a unit (H2).** Severity: minor in M10, major from M12.
   - **What it reads.** `ACOUSTIC_NUMBER` (`app/e2e/lib/dom.ts`) needs `dB`, `s`, `ms` or `%`
     after the digit. Only the Acoustics panel is checked for any digit.
   - **What passes it.** STI, D50 as a fraction, and `1.8 sec` all pass anywhere else in the chrome.
   - **Why it matters later.** M10 has no such number, so M10 is not affected. STI is in v1 scope
     (row 19). Before M12, also flag a parameter name (`T20|T30|EDT|C50|C80|D50|STI|G|SPL|RT`)
     followed by a number.
3. **The wall control in `m10-d` does real work (D2).** Severity: note, keep it.
   - The ceiling half of the check passes a whole-group fill, because Ceiling has exactly faces 10
     and 11. Only the wall double-click (2 faces, not Walls' 8) catches it.
   - Separately, D3 shows that `aimAtFace` is not circular. It requires the BVH pick at the face's
     own centroid to name that face, so a wrong face mapping fails it.
4. **The gate prints `PASS  e2e: wdio run …` when wdio exits 1.** Severity: minor, cosmetic.
   - The check returns `$true` and leaves the verdict to the junit files.
   - The verdict line and the exit code were right in every failing run above. A1, for example,
     prints `exit 1 in 11.1 s` next to `PASS  e2e: wdio run`.
   - A reader scanning the PASS lines sees a PASS for the e2e. Rename the check to "e2e: wdio ran
     (verdict below)", or make it pass only on exit 0.
5. **`app/e2e/specs/m10.screens.e2e.ts:19-21` has invisible key constants.** Severity: minor.
   - `CTRL`, `ENTER` and `BACKSPACE` are literal private-use characters (bytes `EE 80 89`,
     `EE 80 87`, `EE 80 83`), which show as `''` in an editor.
   - GATE.md item 4 says this was fixed in the shell and scene specs. The screens spec, added in
     the integration, reintroduced it. It is not a gate spec.

## B. Design fidelity

**Setup.** The app is the post-revert build of the clean tree, a release build with the custom
protocol. tauri-driver ran it at **1440 × 900, DPR 1**, the design's artboard size.

**Screenshots.** They are in `review-screens/`: 8 PNG, 1.77 MB.
- **1 to 5** come from `m10.ps1 -Spec screens`:
  - Geometry with the corrected hall, then with the raw hall;
  - Materials with Rear wall selected;
  - Sources with S1 selected;
  - R1 refused at x = 20.
- **6 to 8** come from a read-only WebDriver probe run from the scratchpad, not committed:
  - the Acoustics tab on Materials;
  - the Runs tab with three variants;
  - the Console scrollbar under the pointer.

  The probe also read the computed styles and the region rectangles.

The reference is `docs/design/concept-b-approved.dc.html`: its markup at lines 26-482 and its script
at lines 484-800.

### What matches, measured

**Colours.**
- The 18 colour tokens in `theme.css` equal the design's hex values one for one:

  | Token group | Values |
  |---|---|
  | Neutrals | `#09090B` `#0F0F11` `#1D1D21` `#16161A` `#26262B` `#131316` `#1A1A1E` `#1F1F24` |
  | Text | `#EDEDEF` `#A1A1AA` `#85858E` |
  | Red | `#E0202E` `#FF4A55` `#3A0B10` `#1A0B0E` |
  | States | `#74D39F` `#F2A93B` `#FFB3B8` |

- Pixel samples from the screenshots:
  - the current step's underline and the selected dock tab's underline are `#E0202E`;
  - the variant, the selected scene row and the active tool are filled `#3A0B10`;
  - the panels are `#0F0F11`;
  - the view is `#09090B`, with the 7 % red radial glow;
  - the borders are `#1D1D21`;
  - the Ready dot is `#74D39F`;
  - the scrollbar thumb is `#34343C`.

**Layout.** `getBoundingClientRect` at 1440 × 900 matches the design in every region:

| Region | Measured |
|---|---|
| Menu bar | 36 |
| Step bar | 44 |
| Scene list | 248 + 1 border |
| Properties | 344 + 1 |
| Dock | 250 + 1 (tabs 34 + 1) |
| Status bar | 24 |
| Run | 118 × 28 |
| Plan box | 180 × 116 |

**No logo, no wordmark.**
- The DOM has 0 `img`, `picture`, `object` or `embed` elements.
- The only SVGs are the icons in Commands, Run, the filter and the tools, and the axis gizmo.
- The only brand-like text is the status bar's `Solvers: I-Simpa 1.4.0 · SPPS, TCR`, which the
  design has too.
- The window title "I-Simpa Night Mode" is the OS title bar, outside the chrome.

**No serif, no italic.**
- Every element under `body` computes `font-style: normal`, with a first family of Barlow, Barlow
  Condensed or JetBrains Mono. There are 0 exceptions.
- The fonts are bundled locally, because of the CSP. Every face in use is `loaded`.

**Step bar and variant switch.**
- Five steps, each with a badge, a name and a sub.
- The current step has a red badge, a 2 px red underline and a `#EDEDEF` name.
- The subs read `closed · 6 / 6 · 1 · 3`, as in the design.
- The condensed `VARIANT` label sits beside a segmented control. The selected variant is
  `#3A0B10` on white, the others transparent on `#A1A1AA`.
- Three variants are shown in `7-*.png`, as in the design.

**Dock tabs.**
- Acoustics, Console and Runs. The selected tab has a 2 px red underline and `#EDEDEF` text.
- The badges are mono 10.5 px.
- Console lines are mono 12 px: time, tag as text, then the message. FAIL rows are tinted
  `rgba(224,32,46,.08)`.

**Status bar.** The same content in the same order.

**Themed scrollbar colours.** All five scroll containers compute `scrollbar-width: thin` and
`scrollbar-color: #34343C transparent`: segmented, scene-list, dock-body, props and mg-scroll.

### Deviations (every one found)

The tags:
- **[scope]:** M10's plan or rules call for it.
- **[choice]:** a package chose it (GATE.md "Choices the packages made").
- **[fix]:** worth changing.

**Menu bar**
1. **Run reads "Run" and is disabled.** The design reads "Run SPPS". The button sits at opacity
   0.45, so the design's one solid-red call to action renders as `#6D161E`. [scope: Run is wired in
   M11]
2. **Commands is disabled but has no disabled style.** It looks live, and only its tooltip says
   otherwise. [fix, small]

**Step bar**

3. **A `+` button after the variant switch** adds a variant. The design has none. [choice, PLAN 7.5]
4. **The Simulate sub is empty,** where the design reads "run 3 valid". [scope]

**Scene list**

5. **Materials show their full names, truncated,** such as "Linoleum on concr…". The design shows
   short names: "Linoleum", "Ceiling tile", "Plaster", "Wood panel". PLAN.md asks for the "short
   name", and no short-name logic exists. [fix, or rename the fixture]
6. **A PROJECT section is added** for project-level issues. Its message is cut: `FAIL SOURCE_NONE No
   source is enabl…` (`1-*.png`, `2-*.png`). [choice; the cut message is a small fix]
7. **An unassigned group** shows a dashed swatch and "unassigned". The design has no such state.
   [scope]
8. **No audience-grid row,** and Receivers counts "3", not "3 + grid". [scope]

**3D view**

9. **No floor grid.** The design draws a 24 px grid on the floor. [minor]
10. **The tools differ.**
    - There are six tools: Place source is added.
    - Section and Measure are disabled, at 0.35.
    - The Section view tab is disabled too.

    [choice and scope]
11. **The plan inset lacks two details.** It has no S1 and R1-R3 text labels, and no faint 22 px
    ring round the source. The rear wall does turn red on Materials, as designed. [minor]
12. **The axis gizmo follows the camera,** so its axes are foreshortened to about 10-15 px. The
    design draws a fixed triad in the same 60 px box. The colours match. [choice]
13. **A check chip and an amber overlay are added:** the chip reads "FAIL · n faces highlighted"
    with an amber swatch. They are not in the design. They follow the README's two rules: amber for
    out-of-range, and a FAIL label on every failure. [scope]

**Dock**

14. **Acoustics is an empty state.** There is no "Live · Sabine and Eyring, 1/1 octave" note, and no
    Acoustics or Runs badge. [scope: no computed number before M12]
15. **The Console badge counts the session's FAIL lines.** It reads "4 fail" while a clean project
    is open (`7-*.png`). The design's badge is historical too. [note]

**Properties**

16. **The model check changes its markers and wording.**
    - Rows carry OK and FAIL text labels in place of the design's green tick icons.
    - The header reads "Passed" or "FAIL · refused", not "Ready to mesh".
    - An "Open edges" row is added.

    [choice, following the FAIL-label rule]
17. **On the refused hall, a raw code is shown as a label:** `degenerate_faces   1 · 1 faces`, after
    the Units row (`2-*.png`). [fix, small]
18. **Two number displays in the Geometry panel mislead.**
    - The dimensions mix precisions: `41.45 m`, `29.71 m`, `16.1 m`. The design shows `10.00 m`.
    - The refused raw hall shows `Volume 0 m³`. That is not a measured zero: the hall has no
      enclosed volume.

    [fix, small]
19. **"Import model…" with `PLY · OBJ · STL`** replaces "Replace model…" and six formats. [scope,
    PLAN 7.2]
20. **The Materials panel is rebuilt around the grid.**
    - All six library materials are options; the design shows three.
    - The design's per-material "Absorption α editable" strip of six cells and its "Scattering
      0.10" line are gone.
    - In their place is the LIBRARY grid: every material × band, an Absorption and Scattering
      toggle, and + Material, Delete and Fill row.
    - At 900 px the panel scrolls, and the grid's buttons sit at its bottom edge (`3-*.png`).

    [scope: the grid is M10's materials feature. The layout is for Burhan to decide]
21. **The Materials hint drops "Odeon and CATT material files import into the library."** [scope,
    and honest]
22. **The Sources panel is editable and does more.**
    - Position cells are editable inputs with a unit, not read-only mono values.
    - Added: a NAME field, a Remove button, a State row, "Read-only in this build.", a Sources list
      with + Source, and + Receiver.
    - The hint says a receiver outside is refused, not that it "turns red and blocks the run".

    [scope and choice]
23. **A RECEIVER_OUTSIDE refusal outlines X, Y and Z in red,** though only X changed
    (`5-sources-refused.png`). The refusal points at `/position`. [minor]

**Type**

24. **The FAIL and OK labels are synthesised bold.** They use JetBrains Mono at weight 600:
    `.issue .code`, `.check-row .state` and `.issue-tag .state`. Only weights 400 and 500 are
    bundled, so the browser synthesises the bold. [minor]

**Scrollbars**

25. **The design's `::-webkit-scrollbar` rules do nothing in WebView2 154.** Those rules ask for an
    8 px bar, a 4 px radius, a 2 px border in the panel colour and a red thumb on hover.
    - **Why.** `* { scrollbar-width: thin; scrollbar-color: … }` makes Chromium 121 and later draw
      the standard scrollbar instead.
    - **Measured:**
      - the gutter is 10 px (`.dock-body` offsetWidth − clientWidth), not 8;
      - arrow buttons are drawn;
      - the thumb under the pointer turns `#5A5A61`, not `#E0202E` (`8-console-scrollbar-hover.png`,
        pixels at x = 1087 to 1092).
    - **What survives.** The design page itself would render the same way in a current Chromium.
      So the colours Burhan approved are there. The red hover and the 8 px bar with no arrows are
      not.
    - **Fix.** `theme.css` is frozen, so a fix goes through the frozen-hash update in `m10.ps1`.

    [fix, small, Burhan's call]

## Files

**On B:, in the tree:**
- this file;
- 8 PNG in `review-screens/`, 1.77 MB. Files 1 to 5 are regenerated by the gate's `screens` spec;
  files 6 to 8 come from the probe.

**On C:**
- 21 gate work folders under `C:\tmp\nm-target\gates\m10\`, from `20260929-110030` to
  `20260929-112045`, holding 112 files;
- `app.exe` rebuilt from the clean tree.

**In the scratchpad, outside the repo:**
- 21 run logs;
- 9 enlarged crops;
- 3 scripts: `run-mut.ps1`, `px.ps1` and `probe.mjs`.

Nothing was deleted, committed or pushed.
