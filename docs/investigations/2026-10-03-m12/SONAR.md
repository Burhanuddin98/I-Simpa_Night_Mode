# M12 site-map (Sonar, 2026-10-03, rebuild @ 3ef3591)

## 1. Parity matrix (docs/investigations/2026-09-25-parity-audit/parity-matrix.md)
Section 2.4 Results starts :259. Before-v1 list = section 3 (:543-613). Not all rows have a verdict line; sizes from rows.
- R8 echogram+Schroeder chart, Planned, M, :270 (M12 'Decay charts in uPlot'); before-v1 not stated in 3b/3c
- R12 SPL/band, Planned at risk, S, :274; R14 T30/T20 :276 S; R17 C50/C80 S+bed :279, :553 Before v1 (decide); R18 D50 S+bed :562 Before v1 (decide); R19 Ts v1.x :574
- R35 TCR main table (Sabine/Eyring) Planned S :297; R36 TCR receiver levels v1.x :576; R37 TCR maps Planned S :299
- R38 SPPS level map L :300; R39 time-step map animation M :301; R42 parameter maps T30/EDT/C80/D50 L :304
- R41 band choice for maps S, Design only :303, :606 Before v1; R50 map legend S :312, :607 Before v1; R49 palette Absent :311 (M12 fixes black-red-yellow)
- R52 difference map M, Planned :314 (design:446-447, gate (f) variant switch); R53 particle animation M :315 (needs C8>0); R55 transport controls S :317
- R57 parameter tables M, Planned :319 (M12 Acoustics tab, gate (a)); R65 all receivers in one table S :327, :613 Before v1; R68 report export M v1.x :614
- R27 Schroeder table (comes with CSV) v1.x :575; R66 particle paths CSV Later :591; R58 TSV copy Absent :320; R9 spectrum chart Absent :271; R51 value probe Absent :313; R3 delete run Absent :265
- Before-v1 prerequisites touching M12: C8 particles saved :558 (default 0, model.rs:963), C21 maps per band :560, C22 echogram per source :561, M40 surface receiver :554, M41 cutting plane :555
- Count before v1 with M12 landing: R17, R18, R41, R50, R65, C8, C21, C22, M40, M41 (10) among 3b/3c; R8,R12,R14,R35,R37,R38,R39,R42,R52,R53,R55,R57 Planned for M12 (size/before-v1 not stated)
- Risks :29 (M12(b) hides params without PASS bed: C80,D50,C50,Ts), :30 (.pbin empty by default)
- M12 plan text: docs/rebuild-plan-raw-2026-09-23.json:217-225 (raw:219-220 gates)

## 2. Design (docs/design/concept-b-approved.dc.html)
- :34 top-bar step button 'Results'; :587 step==='results'; :618 step list {results,num 5}; :731 isResults
- :109-113 viewport floor: coloured cell grid (cells list, 24px) shown on Results; :115 rearBg/rearBorder wall
- :164-170 map legend overlay bottom-left: legendTitle, gradient bar legendBg, lo/mid/hi labels
- :429-470 right panel: header 'Run N - variant' + 'SPPS - all parameters computed for every receiver' (:432-433)
- :436-445 Map selector: SPL T30 EDT C80 D50 STI (6 chips, SPL active), Band row '1 kHz'
- :446-450 'Difference from Baseline' switch + diffNote
- :453-463 'Receivers - SPL 1 kHz' list: receiver n, dist from S1, SPL dB, delta colour
- :466-467 buttons 'Export report', 'Listen at R1'
- No echogram chart, no CSV, no per-band table drawn in the design. Palette rule: docs/design/README.md:17

## 3. Core
- crates/simpa-core/src/results/report.rs:65 REPORT_VERSION = 10 (not 8)
- report.rs:70 enum Evaluated {Value{value,mc_sd,status,lo,hi,refused_resamples,straddle} | NotEvaluable{not_evaluable: Refused}}; :116 Refused{code,message,..}; :194 status(), :202 value(), :210 refusal()
- report.rs:273 struct Parameters; :303 named()->[(&str,&Evaluated);8]; :364 parameters(); :222 EdtReport (lo_s/hi_s)
- report.rs:458 ReceiverBandReport; :551 AggregateReport; :737 SourceBandReport; :776 SppsReceiverReport; :800 SurfaceSummary; :821 SurfaceReceiverSummary; :967 SppsReport; :1161 TcrReceiverReport; :1232 TcrReport; :1242 Report{results_version}
- report.rs:1929 pub fn report(&RunResults)->Report; :1962 checked_report()->Result<Report,Refusal>; :1980 report_schema(); :1991 RefusalReport
- crates/simpa-core/src/results.rs:143 RunResults; :363 load(folder)->Result<RunResults,Refusal>; :476 solver_build
- params: crates/simpa-core/src/params/{decay,edt,level,noise,sti,room,air,din18041,lambert}.rs (decay.rs:335-400 T30, :403 C50/C80/D50, :419 SPL; noise.rs jnd/RangeStatus)
- results/spps.rs, results/tcr.rs, results/reference.rs
- CLI: crates/simpa/src/main.rs:86 results_cmd (results --json, --schema :48)
- IPC: app/src-tauri/src/commands.rs:504 run_results(run)->ResultsState; runs.rs:284 ResultsState{run,verified,refusal:Option<ReasonUi>,unverified:Option<ReasonUi>}; runs.rs:685 results_state. NO value returned; registered main.rs:261, build.rs:38; ui backend.ts:121 runResults
- Readers: formats/csbin.rs:354 read, :365 read_file (surface maps); formats/pbin.rs:137 read, :210 read_file (particles); formats/gabe.rs:201 read_prefix, :297 read, :301 read_file (.recp echogram/Gabe tables). None exposed over IPC.

## 4. UI today
- app/ui/src/features/simulate/ResultsPanel.tsx: header comment :1-14; shows only [data-results-state] none|running|checking|verified|unverified|refused|error, refusal codes (Codes :26), Run label; no digits. Uses resultsStore, selectedRunStore, actions.resultsFor.
- simulate/model.ts:413 resultsStateName/resultsCodes
- Viewport: app/ui/src/features/viewport/{engine.ts,geometry.ts,libraries.ts,Viewport.tsx} (three 0.186.1, three-mesh-bvh); no map/colour/legend hooks found (grep colorMap/vertexColors/cells: 0 hits)
- No uPlot in app/package.json (deps: three, three-mesh-bvh); no CSV/Blob export code in app/

## 5. Gates
- tools/gates/m10.ps1:70 shell = @('m10-a-console','m10-b-console','m10-f','m10-h'); m10-h test: app/e2e/specs/m10.shell.e2e.ts:200 'no solver-computed acoustic number is shown, on any step or dock tab' (tightening at :222). M12 must replace/relax it (it forbids numbers).
- m11.ps1: header :1-33 (gates a-e + m11-h), param :34-51, $repo/$target/$work :53-70, Untracked() :73, $specIds ordered table :80-97 (smoke,gate,close,kill,after,simulate,dock,project,reload,settings,groups; screens=@() not a gate), $fullRun :100, Check() :110, Note() :118; prior gates m10.ps1/m9.ps1 run under focus watcher
- m11.ps1 -Spec list/-SkipCore/-SkipPrior/-FocusSayNo/-FetchDriver; partial runs never print PASSED
- e2e: app/e2e/m11.conf.ts (wdio, tauri-driver port 4444, need() env M11_*), m10.conf.ts; specs app/e2e/specs/m11.<name>.e2e.ts; libs app/e2e/lib/{m11,runs,acoustic,focus-judge}.ts; gate spec m11.gate.e2e.ts (m11-e-results :382 run_results receipt); m11.screens.e2e.ts for screenshots
- m10-h/m11-h number assertions exist in m11.gate.e2e.ts (m11-h) and m11.dock.e2e.ts (m11-dock-h), m11.simulate.e2e.ts (m11-sim-numbers)
