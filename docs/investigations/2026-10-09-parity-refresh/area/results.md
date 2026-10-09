# Parity: 2.4 Results, status now (read-only audit, R = parity-ro at 95ec062, 2026-10-09)

Statuses were taken from code (ui `app/ui/src`, tauri `app/src-tauri/src`, core `crates`), not from docs' claims. Paths are relative to R. 'unverified' = read from code, not run.

## 1. Table

| ID | Feature | Imp. | 09-25 | Now | Receipt | Note |
|---|---|---|---|---|---|---|
| R1 | Run list (results browser) | core | Planned | READY | ui features/dock/RunsPane.tsx:1-14,:305; tauri commands.rs:542 runs_list | Runs tab lists runs from run.json, newest first |
| R2 | Label a run | common | Absent | ABSENT | no rename in RunsPane.tsx; no command in main.rs:233-284 |  |
| R3 | Delete a run | common | Absent | ABSENT | no delete command in main.rs:233-284 | Runs only accumulate |
| R4 | Open a run folder or file in the OS | common | Absent | ABSENT | capabilities/default.json:4 "No shell"; folder path is text only (RunsPane.tsx Detail) |  |
| R5 | Refresh a results folder | niche | Absent | READY | RunsPane.tsx:~296 refreshRuns on every tab show | Automatic, no button |
| R6 | Per-run record of inputs | common | Ready | READY | core run/manifest.rs; RunsPane.tsx Detail row (hashes, exit code, folder) | Better than upstream: solver build verdict |
| R7 | Receiver level table | common | Absent | PARTIAL | ResponseWindow.tsx:1-30 (time x band echogram map + readout); commands.rs:655 run_echogram | Graphic map and cursor readout; no numeric level table per step |
| R8 | Echogram and Schroeder curve chart | core | Planned | READY | AcousticsPane.tsx:287 DecayChart (Schroeder); ResponseWindow.tsx | Decay chart + echogram map window |
| R9 | Spectrum chart at a receiver | common | Absent | PARTIAL | AcousticsPane.tsx:571-621 receivers table with Band picker; RT chart vs band | Level per band is in a table one band at a time; no level-vs-band spectrum chart |
| R10 | Level per source (.recps) | niche | Absent | PARTIAL | Source picker AcousticsPane.tsx:467-479; core spps.rs:126 | Per-source parameters yes (needs echogram_per_source); the .recps totals are not tabled |
| R11 | Per-source echograms and parameters | common | Core only | READY | AcousticsPane.tsx:467-479; v1.1-backlog 77 (done 2026-10-04) | Source picker drives chart, tables, decay; Elmia e2e m12.sources |
| R12 | SPL per band at receivers | core | Planned (at risk) | READY | acoustics/model.ts:63 spl_db; shownParams model.ts:77 | Shown only with a PASS bed (decision 47 MQ6) |
| R13 | A-weighted level dB(A) | common | Absent | READY | model.ts:73 dba | Receiver-scope column |
| R14 | T30 (and T20) | core | Planned (at risk) | READY | model.ts:66-67 t20_s, t30_s | Refusals still frequent in Elmia (backlog 78, 82) |
| R15 | T15 and chosen decay ranges | common | Absent | ABSENT | model.ts:62-73 has no T15 | T20 and T30 only; chosen ranges not user-editable |
| R16 | EDT | core | Planned (at risk) | READY | model.ts:65 edt_s | Always with range (scope.md Decided 2026-09-27) |
| R17 | C50 and C80 | core | Core only | READY | model.ts:68-69 c50_db, c80_db |  |
| R18 | D50 | common | Core only | READY | model.ts:70 d50 |  |
| R19 | Centre time Ts | common | Core only | READY | model.ts:71 ts_s |  |
| R20 | Custom time limits for C, D, LF | common | Absent | ABSENT | limits fixed in core params/decay.rs; no UI field |  |
| R21 | Early stage support ST_early | niche | Absent | DEFERRED | scope.md Decided 2026-09-29 row 21 ("LF, LFC, LG, ST early ... stay later") |  |
| R22 | STI at receivers | common | Deferred | READY | model.ts:72 sti, STI_NOTE model.ts:89; v1.1-backlog 71 (no MC range) | Brought into v1 (scope.md row 19); fixes upstream female-weights bug |
| R23 | NC-curve background for STI | niche | Deferred | CORE-ONLY | ops.ts:179 background_noise field; no NC curve; no UI field found | Background noise per receiver band, not an NC curve; unverified whether any panel edits it |
| R24 | "Global" all-band row | common | Ready | READY | AcousticsPane.tsx:582 "bands summed"; Global map band ResultsOverlay.tsx:364 |  |
| R25 | "Average" row across bands | common | Not porting | NOT-PORTING | docs/params.md:65-70 | Unchanged |
| R26 | Decay parameters on a multi-source sum | common | Not porting | NOT-PORTING | docs/results.md:469-473 | Unchanged; replaced by the per-source picker (R11) |
| R27 | Schroeder curve table | common | Core only | PARTIAL | AcousticsPane.tsx:287 chart; core report.rs:341-344 | Chart only, no Schroeder table |
| R28 | Lateral fractions LF and LFC | common | Absent | DEFERRED | scope.md Decided 2026-09-29 row 21 | Data read (spps.rs:98-102) |
| R29 | Sound strength G | common | Absent | READY | model.ts:64 g_db | Added to v1 (scope.md row 21) |
| R30 | Late lateral level LG | niche | Absent | DEFERRED | scope.md Decided 2026-09-29 row 21 |  |
| R31 | .gap table view | niche | Absent | CORE-ONLY | core results/spps.rs:95-116 | Read for lateral/noise, no view |
| R32 | Receiver intensity table | niche | Absent | CORE-ONLY | core results/spps.rs:102-103 | Read, no view |
| R33 | Intensity vector animation | niche | Deferred | DEFERRED | plan.md:133-134 (as 09-25); no GUI code |  |
| R34 | Total room energy decay | niche | Core only | CORE-ONLY | core results/report.rs:554 (line from matrix, not re-read) | No view |
| R35 | TCR main results | common | Planned | READY | AcousticsPane.tsx:665-696; model.ts:438-447 | Sabine/Eyring A, T, L per band |
| R36 | TCR receiver levels | common | Core only | CORE-ONLY | core results/tcr.rs:70 PointReceiver | Tab shows room-level Sabine/Eyring L only; per-receiver direct/Sabine/Eyring not found in model.ts (unverified for a TCR run) |
| R37 | TCR maps | common | Planned | READY | results_data.rs:385-390 (TCR surfaces served); resultsView.ts:146-147 groupKey adds field | Maps per field "Direct/Sabine/Eyring" via same panel; not seen rendering a TCR run |
| R38 | SPPS level map (summed) | core | Planned | READY | ResultsOverlay.tsx:343-481 map panel; commands.rs:623 run_surface_map | WebGPU, bit-exact vs csbin (gate c) |
| R39 | Time-step map animation | common | Planned | READY | ResultsOverlay.tsx:518-558 transport; viewport/animator.ts | One timeline for map and particles |
| R40 | Cumulative map animation | niche | Absent | READY | ResultsOverlay.tsx:383-389 map-cumulative; viewport/cumulative.ts |  |
| R41 | Band or Global choice for maps | core | Design only | READY | ResultsOverlay.tsx:345-370 map group + Band chips incl. all bands |  |
| R42 | Parameter maps: T30, EDT, C80, D50 | common | Planned | DEFERRED | v1.1-backlog 76 | Maps show solver energy only |
| R43 | Other parameter maps | niche | Absent | ABSENT | not named in backlog 76 (T30, EDT, C80, D50, STI only) |  |
| R44 | STI map | common | Deferred | DEFERRED | v1.1-backlog 76 |  |
| R45 | Map opacity, front/back | common | Absent | ABSENT | no control; only fixed fade while particles play ResultsOverlay.tsx:636-645, resultsLayer.ts:168 | No front/back either |
| R46 | Smooth or flat map colouring | common | Absent | READY | ResultsOverlay.tsx:439 map-smooth |  |
| R47 | Fixed colour range | common | Absent | READY | ResultsOverlay.tsx:467-473 map-fixed + range parse |  |
| R48 | Iso-contour lines | common | Absent | READY | ResultsOverlay.tsx:450-460 contour steps (CONTOUR_STEPS_DB) | Fixed dB steps; no custom level list |
| R49 | Palette choice | common | Absent | ABSENT | one Dockyard ramp + diverging diff palette; viewport/warmRamp.ts | Single palette by design (docs/design/README.md:17) |
| R50 | Map legend | core | Design only | READY | ResultsOverlay.tsx:498-512 map-legend |  |
| R51 | Value probe on maps | common | Absent | READY | ResultsOverlay.tsx:244 map-probe; mapView.ts probeOf | Level, band, time (and window mean) |
| R52 | Difference map between runs | common | Planned | PARTIAL | ResultsOverlay.tsx:372-381; resultsView.ts defaultBaseline | Switch against an automatically chosen baseline; no picker for any run pair seen, not saved, level only (unverified: whether baseline is user-selectable) |
| R53 | Particle animation | common | Planned | READY | ResultsOverlay.tsx:578-668; viewport/particles.ts, rays.ts; commands.rs:640 | Needs particles_saved > 0 (default 0, model.rs:994; SettingsEditor.tsx:387; MQ4 notice) |
| R54 | Particle trails ("Rays") | niche | Absent | READY | ResultsOverlay.tsx:580-598 trails, :601-624 Dots/Glow/Rays |  |
| R55 | Animation transport controls | common | Planned | READY | ResultsOverlay.tsx:518-575 play, step, speed |  |
| R56 | Export the 3D view as an image | common | Absent | READY | MenuBar.tsx:100; export/exportActions.ts:77-109; export.rs KINDS | PNG only (upstream PNG/JPG/BMP) |
| R57 | Parameter tables | core | Planned | READY | AcousticsPane.tsx:597-621 receivers table; model.ts:63-73 | Gated by bed PASS |
| R58 | Copy table cells (TSV) | common | Absent | ABSENT | no copy handler in AcousticsPane.tsx |  |
| R59 | Save a table as CSV | core | Absent | READY | MenuBar.tsx:107,114; exportActions.ts:54; exportModel.ts:115 paramsCsv | Parameters table (all receivers, bands, sources) to CSV/JSON; no arbitrary table save |
| R60 | Chart from a table selection | common | Absent | ABSENT | no equivalent |  |
| R61 | Editable spreadsheet from a selection | niche | Absent | ABSENT | no equivalent |  |
| R62 | Chart zoom, show/hide, legend | common | Absent | PARTIAL | ResponseWindow.tsx:1-20 zoom, pan, span chips; AcousticsPane.tsx:270 cursor off on RT/decay charts | Echogram window fully zoomable; RT and decay charts fixed; no series hide |
| R63 | Export a chart as an image | common | Deferred | ABSENT | only 3D view exports PNG (exportActions.ts:109); no row in v1.1-backlog | Matrix cited the old ImGui backlog:62 |
| R64 | Chart display styles | niche | Absent | ABSENT | no chart styling |  |
| R65 | All receivers in one table | common | Design only | READY | AcousticsPane.tsx:597-621; exportModel.ts:53 paramRows | All receivers per band in one table, full set in CSV |
| R66 | Particle paths to CSV | niche | Core only | CORE-ONLY | simpa dump pbin (crates/simpa/src/main.rs:127; formats/pbin.rs) | Hex floats, not CSV |
| R67 | Open results of upstream runs | niche | Not porting | NOT-PORTING | proj.rs:5-6; M7 "verified runs only" |  |
| R68 | Report document ("Make report") | common | Design only | DEFERRED | decision-log row 47 MQ5 "no Export report in v1"; no v1.1-backlog row | Only parameter CSV/JSON and WAV exports exist |
| R69 | Area by level of a map | niche | Absent | ABSENT | no area-by-level |  |
| R70 | Normalise SPL to a reference receiver | niche | Absent | ABSENT | no normalisation |  |

## 2. Counts

Rows: 70

| Status now | n |
|---|---|
| READY | 33 |
| PARTIAL | 6 |
| CORE-ONLY | 6 |
| DEFERRED | 7 |
| ABSENT | 15 |
| NOT-PORTING | 3 |
| UNVERIFIED | 0 |

Importance x status:

| Importance | READY | PARTIAL | CORE-ONLY | DEFERRED | ABSENT | NOT-PORTING | UNVERIFIED | total |
|---|---|---|---|---|---|---|---|---|
| core | 11 | 0 | 0 | 0 | 0 | 0 | 0 | 11 |
| common | 19 | 5 | 1 | 4 | 10 | 2 | 0 | 41 |
| niche | 3 | 1 | 5 | 3 | 5 | 1 | 0 | 18 |

Core-importance rows not READY: 

Movement since 09-25: every M10-M12 'Planned' row resolved. Planned -> READY: R1, R8, R12, R14, R16, R35, R37, R38, R39, R53, R55, R57. Planned -> DEFERRED: R42 (backlog 76). Planned -> PARTIAL: R52. Design only -> READY: R41, R50, R65. Absent -> READY: R13, R29, R46, R47, R48, R51, R54, R56, R59 (R13 and R29 entered v1 scope on 2026-09-29). Deferred -> READY: R22 (STI moved to v1). Core only -> READY: R11, R17, R18, R19 (R11 via the backlog-77 source picker).
## 3. Features the matrix missed (upstream receipts, `B:/repos/I-Simpa-upstream/src/isimpa/`)

| # | Feature | Importance | Upstream receipt | Status now |
|---|---|---|---|---|
| X1 | The report script also writes a CSV of RT30 per band per receiver (semicolon, decimal comma), beside the HTML | common | resources/doc/tutorial/script_tutorial/SppsReportSample/__init__.py:~100-117 (`SaveDataToCSV`), called ~:158 | Absent as such; the parameter CSV (R59) covers the data in another layout |
| X2 | Custom iso-level list for contours ("Iso level list", text field) | niche | data_manager/tree_rapport/e_report_recepteurssvisualisation.h:~137-142 (`isolvllist`) | Absent: fixed dB steps only (ResultsOverlay.tsx:450-460) |
| X3 | Map record types: SPL, power gain, and time-valued maps with their own min/max labels in seconds (RT-style maps) | common | e_report_recepteurssvisualisation.h:~85-105 (`rstype`, "Maximum value (s)") | Deferred with R42 (v1.1-backlog 76); the min/max fields exist only as the fixed range (R47) |
| X4 | "Sum only" surface receivers (one time step) handled as a static map, no time animation | niche | e_report_recepteurssvisualisation.h:~79-82 (`onlySum`) | Not checked in the app (unverified) |
| X5 | Respect per-curve chart styling: line, marker, labelled points, font, fill, background image | niche | IHM/simpleGraphDialogs.cpp:281-282, :533-554, :632-663 | Absent (same as R64; the matrix lists only "display styles") |
| X6 | Axis scale min/max and tick spacing set by hand on charts | common | IHM/simpleGraphDialogs.cpp:548-553 | Absent on RT/decay charts (R62 covers zoom only) |
| X7 | Data sheet saved as .gabe (native) as well as CSV; insert row/column, rename lines and columns, edit cells | niche | IHM/GabeDataGrid.cpp:342, :396-445, :520-532 | Absent (R61 covers only the editable spreadsheet export) |
| X8 | Per-source "Calculate acoustic parameters" on the `.recps` file, with the parameter dialog (clarity ms, definition ms, decay dB list, NC curve) | common | data_manager/tree_rapport/e_report_gabe_recps.cpp:49; data_manager/projet_calculation.cpp:796-830 | Per-source parameters exist (R11); dialog not ported (R15, R20, R23) |
| X9 | Result-tree entries for particle animation as "Particles" or "Rays" are separate loads, each per frequency | niche | data_manager/tree_rapport/e_report_partvisualisation.h:~85-92 | Ready: Dots/Glow/Rays looks and a band picker (ResultsOverlay.tsx:601-624, :214) |

Scripts scanned: SystemScript has recp_res_tool (R65), recp_tool and source_tools (geometry, not results), job_tool (C40), sample/parttocsv.py (R66), graphy (chart library behind R60/R64, nothing separate). `ReadSurfReceiver.py`, `user_core`, `preceiv_sourceTracker` are scripting samples or placement tools, not results features.

## 4. What the current app has that upstream lacks

- **Every value carries its range and verdict** (ok/wide/refused with a reason code), and a parameter without a PASS bed is not shown at all (acoustics/model.ts:77; AcousticsPane.tsx:92-132). Upstream prints bare numbers.
- **"Why values are missing"**, the run-quality advisor with "Apply and re-run" (AcousticsPane.tsx:143-191; backlog 80), plus particle loss as a pass/fail verdict.
- **Response window**: receiver echogram as a time x band map, zoom/pan, level span, decay strip (ResponseWindow.tsx).
- **Auralization**: Listen window, impulse response and convolved WAV export (AuralWindow.tsx; commands.rs:672 `run_auralize`).
- **DIN 18041 target** on the RT chart with a shaded band; **absorption by surface group** table (AcousticsPane.tsx:230-285, :698-754).
- **Sound strength G, dB(A) and STI** with the standard's male/female weights fixed (upstream uses female for all, calc.cpp:748); T20 beside T30.
- **GPU SPPS solver** and WebGPU viewport: smooth/contour/fixed-range maps with a **time-window mean**, cumulative maps, a value probe, glowing/ray particle looks, trails (ResultsOverlay.tsx:343-668).
- **Verified results**: runs show "UNVERIFIED solver build"; run.json provenance per run (RunsPane.tsx).
- **Source picker** with per-source decay, and a variant switch that follows the newest run of each variant (AcousticsPane.tsx:330-344).
- **Exports with checked content** (path extension and byte signature, export.rs:49-114); headless `simpa results --json`.
