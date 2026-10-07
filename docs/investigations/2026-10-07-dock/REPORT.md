# C2: the bottom dock, draggable and full-window (report, 2026-10-07)

Branch `dock`, from `gpu` 20e8f09. Spec: `SPEC.md` beside this file. Decision-log row 74.

## What this means for Burhan

The Acoustics / Console / Runs dock is no longer stuck at 230 px. Grab its top edge and pull it up or down.
Pull it to the top, press the new square button beside the chevron, or double-click the tab strip, and it fills
the whole work area (side panels included) with the 3D view hidden behind it; Escape puts it back where it was.
At full window the Acoustics tab spreads its cards across the width and scrolls down, so every table is readable
without the sideways strip. The height is remembered across steps and launches (maximised for the session only,
so a new launch never opens with the 3D view hidden). A tab clicked on the folded strip now opens the dock.

**One choice for you:** maximised, the dock covers the scene list and the properties panel too. The spec said
"full height"; with the centre column alone the Acoustics tables got 800 of 1440 px and the receivers table was
cut. If you would rather keep the side panels visible beside a maximised dock, that is one CSS rule.

## Done-when

| Item | Holds | Receipt (`.out\dock\bed-dock.json`, run 14:35-14:37, pid 23276, 10 of 10 checks) |
|---|---|---|
| Top edge is a drag handle (mouse and keyboard), from the tab strip only to the full window; 3D view hidden at full; resizes live | yes | the 3D view's region went 468, 438, ... 258 px over eight 30 px pointer moves, each read *before* release; drag to the top maximises over `.work`, `.viewport` hidden; keys: Up +24, Shift+Down -96, End maximises, Home folds (36 px), Up reopens at the height |
| Maximise/restore button and double-click on the tab strip toggle last height / full window; Escape restores | yes | button and double-click: 756 px maximised, then back to 490; Escape after a drag to the top: back to 490 |
| Height and maximised persist across steps; Results opens at the last height; Simulate's live run visible when not maximised | yes | 520 px held Results -> Simulate -> Results, maximised held both ways; on Simulate the 3D view kept 228 px (never under 120 px unless maximised); `nm-dock-height` stored "520" |
| At full window: Acoustics tables in the width available, no narrow wrapped note; Console more lines; Runs full table | yes | cards in a grid 1412 px wide (RT, Decay, Sabine/Eyring side by side; advice, receivers, absorption a row each), every card at its content height (cut 0), no sideways scroll; the ISO wording 793 px wide, 30 px tall (was an 8-line column); Console 719 px of lines, Runs table at full width |
| m11.dock and m12 e2e still pass or re-pinned; m12 hook fields and `data-part` names unchanged; model unit tests | dock yes; m12 see below | m11.dock 5/5; m12 35/40, the 5 failures identical with this branch's dock code removed (control arm), so not this branch's; no hook or `data-part` renamed or removed (one hook added, `dockSize`); 5 new model tests pass |
| Screenshots at three heights on CR4-27's results | yes | below |

## Screenshots (1440 x 900, CR4-27 run 20261007-105106-077-spps, Results step, Acoustics tab)

- `B:\repos\I-Simpa_Night_Mode\.out\dock\dock-1-min.png`: folded, the tab strip only (36 px).
- `B:\repos\I-Simpa_Night_Mode\.out\dock\dock-2-half.png`: half the column (374 px), the 3D view above it.
- `B:\repos\I-Simpa_Night_Mode\.out\dock\dock-3-full.png`: maximised over the work area (756 px).
- Extras: `dock-3-full-console.png`, `dock-3-full-runs.png`; log `bed-dock.log`.

The project was opened in place from `.out\b3\app-bed-cr4-27\app-CR4-27\` (7.3 GB, not copied; nothing run or
saved). A cold read of its results took over 15 min twice on exFAT; warm, 100-104 s.

**The 14:19-14:22 intrusion:** the C1 bed drove pid 12808 (my bed of 14:17). That run timed out with no check and
no screenshot; it left the fold state in my bed's WebView2 profile, so the next run (pid 3820, 14:32-14:34) opened
folded and failed its first check. Both are discarded. Every receipt and screenshot here is from pid 23276
(14:35-14:37), on CR4, checked by eye.

## What was built

- `app/ui/src/features/dock/model.ts`: the height rules, pure and tested: `clampHeight`, `dockHeight`, `dragTo`
  (folds below 60 px; maximises when the 3D view would keep under 120 px; both keep the start height to return
  to), `toggleMax`, `escapeMax`, `keyTo`, `dockTall` (440 px), `parseDockHeight` / `dockHeightText`.
- `Dock.tsx`: the handle (`.dock-grip`, `role="separator"`, focusable; pointer capture, one write per frame, stored
  on release), the maximise button (`.dock-max`, the fold chevron's look), double-click on the tab strip, Escape
  (yields to open menus, dialogs, the response window and the presentation view), the height inline as a CSS
  `clamp()` so a resized window needs no measuring, the panes memoised so a drag re-renders only the dock's box,
  a tab click on the folded strip unfolds, the `dockSize` e2e hook.
- `dock.css`: grip and button, `box-sizing: border-box`, the maximised dock absolute over `.work` on `--panel`,
  `.viewport` `visibility: hidden` behind it (the canvas keeps drawing: a live run and playback go on), no
  height transition while dragging.
- `acoustics.css`: the head wraps (the wording takes the rest of its line or one of its own below 360 px, at any
  height); a tall dock lays the cards in a grid (`auto-fill, minmax(440px, 1fr)`, max-content rows, dense flow).
- `tools/devtools/bed-dock.py` (+ README line): the bed; CDP input events only, its own WebView2 profile
  (`WEBVIEW2_USER_DATA_FOLDER`), so nothing it stores reaches another app.exe.

## e2e (`tools/gates/m11.ps1 -Only e2e -TargetDir C:\tmp\nm-target-dock -SolversDir C:\tmp\nm-solvers-gpu`)

`-Spec dock,m12.viewport,m12.acoustics,m12.bedplant,m12.sources,m12.response,m12.plane,m12.mapview,m12.fill,m12.trails,m12.export,m12.mapwindow`.
Run 1 at 2823cf8 (`C:\tmp\nm-target-dock\gates\m11\20261007-140301`), run 2 at 4e3dc20 (`...\20261007-141116`).

| Spec | Run 2 | Note |
|---|---|---|
| m11.dock | 5/5 | run 1: m11-dock-interrupted failed ('' !== 'Interrupted': the dock opens folded in a fresh profile since decision 50, its rows hidden). Fixed in the app (a tab click opens the dock), not re-pinned |
| m12.viewport, acoustics, bedplant, sources, plane, fill, trails, mapwindow | 5, 4, 1, 2, 4, 3, 3, 5: all pass | |
| m12.response | 2/4 | resp-crop ("a Full run toggle with nothing cut"), resp-numbers (a digit in "ReceiverR1R2R3") |
| m12.mapview | 3/4 | w5-probe (legend text '' against `/^Cutting planes · 1 kHz · /`) |
| m12.export | 3/5 | w9-png (170 !== 0), w9-layout (legend and tools overlap) |

**Control arm** (`...\20261007-140806`): the same harness, solvers and build with `app/ui/src/features/dock` and
`acoustics.css` checked out at 20e8f09, `-Spec dock,m12.response,m12.mapview,m12.export`: the same six failures
with the same messages. They predate this branch and lie outside the dock (response window, map legend, export,
viewport overlays); left alone per the spec's scope. No spec was re-pinned. Run 2's focus watcher: PASS.

## Counts

`npm run typecheck` passes; `npm test` 341 of 341 (336 before, plus 5 C2 cases). Release builds only, no cargo
test. Not pushed.