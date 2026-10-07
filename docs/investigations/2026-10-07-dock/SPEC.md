# C2: the bottom dock, draggable and full-window (spec, 2026-10-07 13:25)

Burhan, 13:23, verbatim: "also the bottom bar, which contains acoustics consols and runs, needs to be able
to be dragged and expanded to full screen, it is very difficult to access the details there, because it is
restricted to that much only".

Today (screenshot `.out\b3\app-bed-cr4-27\b3-live-2-middle.png`, 1440x900): the dock with the Acoustics,
Console and Runs tabs sits at the bottom of the 3D view at a fixed height of about 230 px, with a small
chevron at its right; the Acoustics tab's tables and the "Why values are missing" strip are cut off and
scroll inside it. The dock has a model (`app/ui/src/features/dock/`, `model.test.ts`) and an e2e spec
(m11.dock).

## Done when

- The dock's top edge is a drag handle: drag it up or down with the mouse (and keyboard: a focusable handle
  with arrow keys), from a minimum that shows the tab strip only, to the full height of the window, where the
  3D view is hidden and the tab's content has the whole area. The 3D view resizes live, not after release
  (the viewport's resize path already exists; the live view and playback keep running).
- A maximise/restore button on the tab strip, and double-click on the tab strip, toggle between the last
  dragged height and full window. Escape restores from full window.
- The height (and maximised state) persists per project session and across step changes; the Results step
  opens at the last height; the Simulate step's live run is visible when the dock is not maximised.
- At full window, the Acoustics tab lays its tables out in the width available (no narrow column of wrapped
  text like the "Computed to ISO 3382-1" note in `.out\a5\a5-results-gpu.png`), the Console shows more
  lines, the Runs tab its full table.
- The m11.dock and m12 e2e specs still pass (or are re-pinned with the reason in the commit); the m12
  hook fields and `data-part` names are unchanged; UI unit tests for the model (clamping, persistence,
  toggle) pass.
- Screenshots at three heights (minimum, half, full window) on CR4-27's results, in `.out\dock\`.

## Constraints

- Worktree `B:\repos\I-Simpa_Night_Mode\.claude\worktrees\dock` (branch `dock`, from `gpu`). Outputs only to
  `B:\repos\I-Simpa_Night_Mode\.out\dock\` and `C:\tmp\nm-target-dock` (cargo). Solvers
  `SIMPA_SOLVERS_DIR=C:\tmp\nm-solvers-gpu`. Do not touch `C:\tmp\nm-target`, other worktrees, or any
  app.exe you did not start (Burhan has one open on CR4-27: never its window).
- Build the app with `npx --no-install tauri build --no-bundle` in `app/`, `CARGO_TARGET_DIR=C:/tmp/nm-target-dock`
  (a release build only; this step needs no cargo test). Drive your own app headless (`tools/devtools/`).
- Another builder works on the outliner, materials grid, sources panel, ops and core on branch `blank` at the
  same time: the dock's layout, resize, tabs and the Acoustics/Console/Runs tab layouts are yours; do not
  edit those other panels.
- Do NOT run the full m10/m11/m12 gates; run only the dock and m12 specs with the e2e runner if it can take
  named specs (`tools/gates/m11.ps1 -Only e2e -Spec dock,m12.acoustics,...`), and say what you ran.
- Match the instrument look (decision 50 and the 10-06 decisions 60-64: glass panels, Segoe UI + Cascadia
  Mono, the red/black palette); no new design language.
- Commit on `dock` as you go, messages on the why, no Claude attribution, no Co-Authored-By. Do not push.
  `npm run typecheck` and `npm test` pass at the end. Log a decision (next free number after 72, coordinate
  by taking 74 if `blank` takes 73) in `docs/decision-log.md`.

## Report

`docs/investigations/2026-10-07-dock/REPORT.md` on `dock`, committed: the done-when (holds / not), the
screenshots, what was built, which e2e specs ran and their result, re-pins with reasons.
