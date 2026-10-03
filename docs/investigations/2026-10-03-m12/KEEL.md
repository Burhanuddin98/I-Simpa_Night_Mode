# KEEL brief: M12 arc-open (2026-10-03, rebuild @ 3ef3591)

## 1. Spec (docs/rebuild-plan-raw-2026-09-23.json, milestones M12; table row docs/rebuild-plan.md:98; deps M11, M7, M8)
Deliverable: Acoustics tab (RT per band vs DIN 18041 target band, Sabine/Eyring table, absorption by surface group); surface-receiver maps as faces x steps data texture, black-red-yellow; particle playback from GPU-resident .pbin on one shared Animator timeline; uPlot decay charts + A/B variant switch; "A parameter whose bed status in beds/summary.json is not PASS is not rendered."
Gate: `pwsh tools/gates/m12.ps1` exits 0 (does not exist; tools/gates has m0-m11, m8a, parity). (a) DOM numbers == `cli results <run> --json` at displayed precision, 0 mismatches. (b) 0 DOM elements for any parameter not PASS in beds/summary.json. (c) 3 sampled (face,step) texels == .csbin float32 exactly. (d) at 5 steps rendered particle count == alive in .pbin. (e) DIN 18041 A3 target for 180 m3 box = 0.55 s. (f) variant switch replaces RT series with other run's JSON, 0 mismatches.
Rulings that add/alter (docs/decision-log.md):
- Row 18 (:30, 09-29): SPL/C50/C80/D50 engineered properly; "without this M12 would show only T30 and EDT".
- Row 19 (09-29): STI in v1, receivers and map. Row 21 (09-29 09:03): "Every number v1 shows is tested; nothing in v1 needs the 'not yet validated' label"; adds T20, Ts, dB(A), G, STI.
- Row 20 (:32, 09-29 09:00): core metrics tested, rest labelled "not yet validated"; SUPERSEDES M12's "only passing-bed numbers" for non-core. Row 21 (3 min later) says no label needed. Gate (b) text predates both.
- Row 37 (10-02 08:42): (1) EDT not validated for receiver radius > 1 m (`edt_validated` flag); (2) Energetic EDT stays not validated; (3) every M8b metric shows a range; (5) BOTH marks visible in the GUI, not only JSON/CLI; (6) ball-vs-point EDT difference measured before EDT "done".
- Row 39 (10-02 15:28): public wording "checked against reference solutions to within the just-noticeable difference", never "validated". Backlog 74 (v1.1-backlog.md:87): wording over-claims for set B (reference = SPPS at 1M particles = convergence, not physics); done when wording (README, app, M12 Results screen) names what was checked against what; decision 39 to be amended. Burhan 10-03 08:31 "isnt validated such an egoistic thing"; 08:32 "worry about the ego behind semantics later".
- Row 42: C/D/Ts accept arrival after onset bin when it holds leading edge. Row 43: new projects 125-8k Hz, 7 bands. Row 44 (10-03 08:38): (2) particles saved stays 0 for new projects, "Yes: at M12, when playback needs particles"; (3) echogram per source on for new projects, off for legacy imports. Row 45: grouped receivers import with folder path. Row 30 (:42): style frozen; M11, M8b, M12, M13 then release.
- Row 14 (:25): Gate 2 tail-bound wording "settled at M12": 'guaranteed range for time-step discretisation; late-energy term uses a checked diffuse-field bound with 1000x margin'.
- rebuild-plan.md:141: map selector and difference-from-baseline map IN M12; library/Odeon/CATT/grids/source groups after M12. rebuild-plan.md:107,127: only M12 waits for the bed.
- scope.md:9: "M12-M14 not started".
M11 deferrals: PLAN.md:42 (only M12 may show a number); :158 C8 particles-saved, C22 echogram-per-source "matter to M12 playback and multi-source" (PQ3 editor since built, row 44); R41 map selector; GATE.md:34 backlog 38 (verified vs verified build; CLOSED f745900, row 32).
Backlog rows naming M12: 38 (closed), 47 (results_version; stale: REPORT_VERSION is now 10, report.rs:65, row says 5), 74 (wording), V2-2 STI (row 19). Also params.md:17, 262, 709, 721, 740, 1474, 1521 and results.md:12, 218-221: M12 owns curved-decay display, decay charts from the same code as the numbers, saying which direct-sound reading is used, `lambert_walls` use, "M12's rooms need their own evidence".

## 2. What exists
- ResultsPanel.tsx (152 lines): verdict only (`[data-results-state]` none|running|checking|verified|unverified|refused|errored); header comment: no `[data-result]`, no digit outside `[data-run-label]`. IPC `run_results` commands.rs:504. Report REPORT_VERSION 10 (report.rs:65, not 8): value + status ok|wide + lo/hi, refusal codes, `edt_validated` (report.rs:298), `sti`, `g_db`, `dba`, `straddle`, `refused_resamples`; `validated_by_bed: false` hard-coded (report.rs:1947; asserted false at cli_results.rs:250).
- params/din18041.rs exists (gate e feasible).
- No m12.ps1, no m12 e2e; no uPlot/Animator/.pbin playback seen in scope.md.
- Gates to flip/retire: m10 (h) (m10.ps1:10,70) and m11-h (m11.ps1:7,82) assert no solver number on screen: narrow to non-Results surfaces or replace with "number only if verified AND bed PASS"; ResultsPanel header comment; `validated_by_bed` semantics + cli_results.rs:250.
- beds/summary.json: only beds/m8a-20260929T093134Z/summary.json (T30, pass:true, git da6f15c) plus beds/m8a.json. No M8b summary, no per-parameter status map for 11 parameters. Gate (b)'s source file has no format yet.

## 3. Bed status (docs/investigations/2026-10-02-bed/RESULT.md; ef05c66 10-02, 5b2f316 10-03)
- T30: M8a 23/23 (06588a2, 09-29); after build F 1 borderline miss in 833 (G6 R007 1 kHz seed 4201, wide 0.385 vs truth 0.360, range excludes by 0.0015 s); range uncalibrated (backlog 65).
- T20, Ts, C50, C80, D50, SPL: no wrong-silent on 1,008 rows/metric/seed set at defaults, answered >= 92%. G, dB(A), STI: ADDENDUM-2-STI, ADDENDUM-5 (STI passes after build H).
- Marks: EDT not validated R > 1 m; Energetic EDT not validated (row 37 1-2); C50/D50 miss the 99%-within-1/10-limen exactness bar on exact inputs, shown `wide` at the 50 ms edge (build F). Backlog 69: C50/C80/D50 `ok` beyond 1/10 limen on short double-slope closed forms at 1 ms (12/12/3 rows). 62 EDT no-LOS anchor; 64 Ts no straddle bracket; 66-68, 71-73 STI edges, STI has no MC range (row 37 (3) asks one); 70 G/dB(A) absolute not tested end to end; 74 set B convergence only. No comparison with a measured room (V2-9).

## 4. Load-bearing question
Does Results render only numbers whose bed result is PASS, AND show every range (`wide`, lo/hi) and both EDT marks, in wording that says what was checked? Wrong if: (i) gate (b) is satisfied by a hand-written PASS list; (ii) status/lo/hi or edt_validated dropped at display; (iii) STI shown as a bare value; (iv) "validated" language.

## 5. Ruled, not built
- Row 37 (5) both marks visible in GUI (GUI shows no values yet).
- Row 37 (6) ball-vs-point EDT measurement: not verified done here.
- Row 14 wording settled at M12; backlog 74 / row 39 amendment: open.
- Backlog 47 version rule + required-fields-vs-version test: open.
- Row 44 (2) particles saved 0 vs playback needing .pbin: unresolved.
- rebuild-plan.md:141 map selector + difference-from-baseline map; M11 R41.
- Row 20 vs 21 label contradiction never reconciled.
