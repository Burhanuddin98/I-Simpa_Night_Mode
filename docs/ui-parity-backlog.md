# UI parity backlog — what is actually still missing against upstream

Arc-plan item 9, done 2026-09-08. Supersedes the priority list in `AUDIT_COMPARISON.md`, which is kept as the historical record of the April audit.

## Why this exists

`AUDIT_COMPARISON.md` was written mid-session on 2026-04-04 and its twenty-item priority list was largely worked off *within the same session*. Read today it says all seven of its CRITICAL gaps are open. The v0.3.0 through v0.3.3 changelog says every one of them shipped. Two prose documents disagreeing about the same day is not evidence, so each item below was checked against the source tree.

**What a green row here means:** the code exists. It is not by itself a claim that the feature works.

**Updated 05:52 the same day — some rows are now stronger than that.** The GUI was built from a
clean checkout, driven end to end against solvers built from the pinned upstream tag, and
photographed running (`session-logs/summary-2026-09-08-release-arc-open.md`). Rows marked **seen**
below were observed rendering Elmia Hall with real SPPS results, 955 verts / 1086 faces /
10 groups, on an RTX 5070 under OpenGL 4.6. That is a much stronger claim than code presence, and
it is confined to what was actually visible in one frame.

## Landed — code present, behaviour unverified

| audit item | receipt |
|---|---|
| 1. Colour legend bar | `panels/results.cpp`, `viewport/viewport.cpp` |
| 2. Intensity vector arrows | `panels/results.cpp`, `viewport/viewport.cpp` |
| 3. Iso-contour lines | **seen** — white contour lines over the floor map |
| 4. Surface colormap, smooth + palette selector | **seen** — smooth green-to-yellow field on the floor map, translucent room shell over it |
| 5. Time-step surface maps | `panels/results.cpp` |
| 6. Acoustic parameter maps, RT60/EDT/C80/D50/Ts | **parameters computed and read back**: R1 near RT60 0.90 s, EDT 0.18 s, C80 15.7 dB, D50 97%, Ts 15 ms. Whether the *map* renders was not visible in the frame |
| 7. Animation step buttons and speed slider | `panels/results.cpp` |
| 8. Solid-cube heatmap removed | absent from the tree |
| 9. Mesh quality parameters UI | `panels/properties.cpp`, `project/solver.cpp` |
| 10. Frequency band presets | `panels/properties.cpp:137,143,167,173` — Octave and 1/3 Oct, for both SPPS and TCR |
| 11. Ground type presets | `panels/properties.cpp:111-119` — eight, Water 0.0001 m through Urban 2.0 m |
| 14. Face display modes | `viewport/viewport.cpp`, `app/app.cpp` |
| 18. RGB axis arrows | v0.3.2, palette and axis code in `viewport/` |
| Drag-drop file open (§1 of the audit) | `app/app.cpp` GLFW drop callback |

Fourteen of the twenty. The audit's own feature tables are stale in the same way and should not be quoted.

Also **seen** working in the same frame, none of them on the priority list: the cutting-plane
cross-section that replaced the reverted scene-type surface receiver, the material library cards
with per-material absorption curves (Acoustic Foam 0.753, Heavy Curtain 0.466, Brick 0.043,
Plaster 0.031), the left workflow rail counting room / materials / sources / receivers, and the
pre-flight check panel reporting geometry, materials, sources and receivers all green before a
run. The pre-flight panel is `ValidateProject()` from the April plan's Step 2.2, working.

## Still open — no code found

| # | item | evidence it is absent |
|---|---|---|
| 12 | Per-source echogram | one echogram plot exists at `panels/results.cpp:585-587`; nothing keys it by source |
| 13 | Result comparison / subtraction | no match for compare or subtract outside `solver.cpp` and miniz |
| 15 | Material colour mode, original vs acoustic | no match for any colour-mode symbol |
| 16 | Contour-only wireframe mode | wireframe is all-or-none |
| 17 | Full XY / XZ / YZ grid toggles | ground plane only, no grid-plane symbols |
| 20 | Directivity pattern file loading | only an enum in `project/project.h`; no `.dir` reader |

Plus, from the audit tables and not in its priority list: preferences dialog, language selection, save-a-copy, surface group from selection, vertex editing, material categories, custom directivity visualisation, the TLM solver, mesh preview in the viewport, and graph export.

## Deliberately not parity

- **Surface receivers, scene type.** Present as a struct, wired and then reverted in `35778f0a` because it crashed SPPS. The shipped workaround is cutting planes with a cross-section. This is arc-plan item 8 and it is a solver crash, not a UI gap.
- **Embedded Python.** Upstream runs Python in-process with an event system. Night Mode shells out and exchanges JSON. That is an architecture choice, not a missing feature, and the in-process route would pull a Python dependency into the GUI.
- **Multi-column scene tree with Calculation and Results tabs.** Night Mode puts configuration in Properties and results in a Results panel. Deliberate, and the reason the UI is not a nightmare.

## What this means for release

None of the six open items blocks a release. The two worth doing before one are per-source echogram and result comparison, both of which a user hits the first time they run two variants of a room. The rest are polish.

The real blockers live in the arc plan: reproducible build, the solver patch provenance, licence files, and one verified end-to-end solve.
