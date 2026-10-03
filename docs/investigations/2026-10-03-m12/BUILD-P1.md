# M12 P1 build: core and contracts (2026-10-03)

Branch `m12` off `rebuild` @ `e66beb3`, worktree `.claude/worktrees/m12`. Spec: `PLAN.md` beside this file, P1 items
1-4, with MQ1-MQ6 at their defaults. Receipts (red and green logs, the progress log, the gate logs): `B:\data\m12\`.

## What it means

**The Results screen can now be told, from the evidence, which numbers it may show, and it has a way to get them.**
The bed status of every parameter is computed by a script from the bed files themselves, and a test fails if anyone
edits the result by hand. Applied mechanically, that gives:

- **Shown (PASS): SPL, T20, Ts, C50, C80, D50, STI.**
- **Hidden (FAIL): T30, EDT, G, dB(A).** None of these is a new finding; the script only refuses to call them passed
  where the files do not say so:
  - **T30**: set B's fresh draw has one wrong-silent value (G6 R007 1 kHz, seed 4201), the one `RESULT.md` already
    reports. Decision 39's product grade is "no wrong-silent value", so T30 does not pass it. M8a itself passed.
  - **EDT**: the scored round-2 file (`B:\data\m8b-edt\round2\results\score\summary.json`) says `verdict.pass: false`
    (H5, H6). `VERDICT-2.md` reads Random mode as PASS using a separate attacker run that has no summary file.
  - **G and dB(A)**: no bed file scores them. Decision 21 says they are covered by unit tests on the tested SPL; the
    script has no artifact to read that from.

**What Burhan must choose (none of it blocks P2 or P3):** whether T30 and EDT are hidden in v1 as the evidence stands,
or what new evidence or ruling would let them pass; and whether G and dB(A) may take SPL's status (a rule that would
be written into the script by his word, not inferred). Until then gate (b) hides all four.

## Commits

| Commit | What |
|---|---|
| `8d84d3d` | Item 1: `tools/bed/summary.py`, `beds/summary.json` (generated), `crates/simpa-core/tests/bed_summary.rs`; `.gitattributes` keeps the summary's bytes on every checkout |
| `794e5b5` | Item 2: `core::results::bed`; the report carries `bed` and reads `validated_by_bed` from it; results version 11; schema and `results-json.md`; backlog 47 half closed (the pin test) |
| `bbb32ff` | Item 3: `app/src-tauri/src/results_data.rs`, five commands, bindings, `backend.ts`, `ui/src/resultsData.ts` decoders; m11.ps1's inventory 39 -> 44 |
| `0dc54b8` | Item 4: m10-h and m11-h narrowed (`dom.ts`, `acoustic.ts`, both specs), the checker's tests, `tools/gates/m12.ps1` skeleton |

## Tests, red then green

| Test | Red (receipt in `B:\data\m12\`) | Green |
|---|---|---|
| `crates/simpa-core/tests/bed_summary.rs` (4): the committed file is the script's output byte for byte; every parameter has a status, a PASS has artifacts, a FAIL has reasons, every artifact a sha256; a wrong-silent row planted in a copy of the evidence turns T20 FAIL; a missing artifact refuses SPL, C50 and T20 by name | `red-item1-bed_summary.txt`: 4 failed (no script, no file) | 4 passed |
| `cli_results.rs` `gate_e_seat...` and `an_spps_report_carries...`: `bed` equals `beds/summary.json` read from disk, status, reasons and notes; `validated_by_bed` = all PASS; version 11 | `red-item2-cli_results.txt`: "no bed" | 25 passed (the file) |
| `core::results::bed` (4): the compiled-in summary reads and names each parameter once; its sha256 is the file's; a summary that does not read, or names an unknown parameter, fails every parameter; `validated_by_bed` is all-PASS | compile, then the order assertion (serde_json sorts keys) | 4 passed |
| `report.rs` `the_required_fields_are_pinned_to_the_results_version` (backlog 47) | `red-item2-pin.txt`: digest `178cbc01...` vs the zero placeholder | passed, pinned at (11, `178cbc01...`) |
| app `results_data::tests` (6): the report is `checked_report`'s, the CLI's, with the summary's statuses; a refused run has its refusal and no report or data, and a data read says `RESULTS_REFUSED`; the index lists `outputs_spps`'s 6 maps, 2 particle files and 2 echograms; every map's nodes, faces, receivers, record steps and float32 values bit for bit against `csbin::read_file`; every particle's first step, offsets, positions and energies bit for bit against `pbin::read_file`, and `PARTICLES_NOT_SAVED` for a run with none; the echogram is the `.recp` columns widened, per source too, `NO_TIME_SERIES` for TCR | `red-item3-app.txt`: 6 failed (stubs) | 6 passed (app 68) |
| UI `resultsData.test.ts` (3): both layouts decode bit for bit (0.1 reads as `Math.fround(0.1)`, not the double), a step with no record is `null`, a wrong magic, version or size is refused | `red-item3-ui.txt`: module not found | 3 passed (UI 159) |
| harness `acoustic.test.ts`, 2 new: the same numbers on another step fail rules 1, 2 and the Acoustics rule; inside the Results regions on the Results step they pass; a number outside the regions on the Results step still fails; a tooltip likewise | `red-item4-acoustic.txt`: 2 failed, and tsc refuses `results` | 45 passed |

## The derived status table (`beds/summary.json`)

| Parameter | Status | Read from | Why |
|---|---|---|---|
| `spl_db` | PASS | set A (3 files), B (2), C | 0 beyond the JND; 0 wrong-silent, scorer pass in B and C |
| `edt_s` | **FAIL** | EDT round 2 score summary | `verdict.pass` false, `verdict.failing` H5, H6 |
| `t20_s` | PASS | set A, B, C | as SPL |
| `t30_s` | **FAIL** | M8a summary (pass), set A, B, C | `B-score-F-fresh`: `score.t30.wrong_silent` 1, `pass_` false |
| `c50_db` | PASS | set A, B, C | as SPL; note: 97.8 % within 1/10 JND on exact inputs, research bar 99 % not met (MQ1) |
| `c80_db` | PASS | set A, B, C | as SPL |
| `d50` | PASS | set A, B, C | as SPL; note: 98.1 % within 1/10 JND, research bar not met (MQ1) |
| `ts_s` | PASS | set A, B, C | as SPL |
| `sti` | PASS | STI A (S1 build H, S2), B7, C7 fresh (ADDENDUM-3 and -5 scoring) | 0 beyond 0.03, 0 wrong-silent, scorer pass; note: no noise range (MQ3) |
| `g_db` | **FAIL** | none | no final bed artifact scores it |
| `dba` | **FAIL** | none | no final bed artifact scores it |

Each artifact entry carries its path, sha256, the RESULT section that cites it, the commit it ran at and whether the
tree was dirty (read from the artifact or its scorer's `run.log`; `null` with a note where not recorded), and each
check with the value read. Notes carry the marks (row 37 (1)-(2) on EDT), the `wide` rules, backlog 64, 65, 69, 70,
71 and 74 (set B is a convergence check), and "no comparison with a measured room".

## The IPC surface for P2 and P3

All five go through `results::load`, so nothing unverified leaves; each is a `guard::blocking` command, declared in
`backend.ts`, with types from the regenerated `ipc.ts`. `app/src-tauri/src/results_data.rs` holds both byte layouts in
its header; `app/ui/src/resultsData.ts` decodes them.

| Command | Rust | TS (`backend`) | Returns |
|---|---|---|---|
| `run_report` | `run_report(run: String) -> CmdResult<ReportView>` | `runReport(run): Promise<ReportView>` | `{ state: ResultsState, report: Report \| null }`: the CLI's report (`checked_report`), `report.bed.parameters[name].status` for gate (b) |
| `run_data` | `run_data(run: String) -> CmdResult<RunDataIndex>` | `runData(run): Promise<RunDataIndex>` | `{ state, data: { solver, bands_hz, time_step_s, steps, surfaces: SurfaceMapInfo[], particle_files: ParticleFileSummary[], echograms: EchogramInfo[] } \| null }` |
| `run_surface_map` | `run_surface_map(run: String, path: String) -> CmdResult<Response>` | `runSurfaceMap(run, path): Promise<ArrayBuffer>` | SMAP v1 bytes; `decodeSurfaceMap(buf)`, `surfaceValue(map, face, step)` (float32 or `null`) |
| `run_particles` | `run_particles(run: String, band_hz: i32) -> CmdResult<Response>` | `runParticles(run, bandHz): Promise<ArrayBuffer>` | PART v1 bytes; `decodeParticles(buf)` |
| `run_echogram` | `run_echogram(run: String, receiver: String) -> CmdResult<EchogramView>` | `runEchogram(run, receiver): Promise<EchogramView>` | `{ receiver, folder, time_step_s, bands: {freq_hz, energy}[], sources: {source, file, bands}[] }`, Pa² per step |

Error codes: `NO_PROJECT`, `RUN_NOT_FOUND`, `RESULTS_REFUSED`, `MAP_NOT_FOUND`, `PARTICLES_NOT_SAVED` (MQ4's case),
`PARTICLES_NOT_FOUND`, `RECEIVER_NOT_FOUND`, `NO_TIME_SERIES` (TCR), `RESULTS_TOO_LARGE`. `run_results` is unchanged;
`runs::results_state` now shares `results_data::open`, so the Runs row and the Results step read one loader.

## m10-h and m11-h, narrowed

`app/e2e/lib/dom.ts`: `RESULTS_REGIONS` (`[data-props-step="results"]`, `[data-dock-panel="acoustics"]`,
`[data-results-region]`) and `RESULTS_STEP_CURRENT` (`[data-step="results"][aria-current="step"]`). They are hidden
from the reads only while the page says the Results step is current. m10-h asserts the page agrees with its loop
before it trusts that, and keeps the Acoustics no-digit rule on every other step. m11-h's `collectSnapshot` adds a
`results` view (the page outside the regions) on the Results step only, and `judge` reads rules 1, 2, 1t and the
Acoustics rule from it there and from the whole page elsewhere. P2 and P3 mark any other Results-step element that
shows a number (the map legend, for one) `[data-results-region]`.

## Gates

- `m12.ps1 -Only static -SkipPrior` (`m12-static-1.log`): 6 of 6 P1 checks PASS, "M12 PARTIAL RUN ... this is not a
  gate pass". In a full run the six e2e ids (a)-(f) fail as NOT BUILT, by design.
- Workspace suite (`cargo test --workspace --no-fail-fast`, 4 test threads, `suite-1.log`, 12:35-12:46): **1101
  passed, 0 failed, 40 ignored** in 158 binaries. Baseline 1086; the difference is the 15 tests added here (core 9,
  app 6).
- `m11.ps1` in full, with the narrowed checks: appended after the run (this file is committed before it, so the gate
  sees no untracked file).

## Left open

- **T30, EDT, G, dB(A) hidden** until Burhan rules (above). P2 must render by `report.bed`, not by a list.
- **Two more M11 checks will break when P2 renders values**: `m11-sim-numbers` (its Results branch: no digit, no
  `[data-result]`) and `m11-sim-link` (no digit) on a verified run. Not in item 4's text (m10-h, m11-h), so not
  touched; P2 narrows them the same way. `m11-e-results` (a refused run shows no number) stays valid as it is.
- **Evidence age**: M8a ran at `da6f15c` (before build F); STI B7 at `5d0d281` (build G; RESULT.md says build H's
  re-read moved nothing, with no artifact); set A ran on dirty trees (`dcc5e11` + 460 lines, `8ec5cec` + build F's
  work). The summary records each; no rule here re-runs a bed.
- **The CLI's text banner** still reads "UNVALIDATED: M8's physics bed has not passed" (`results_cmd.rs:186`). It is
  still true (`validated_by_bed` is false) but no longer the whole story; wording is backlog 74 / MQ2.
- **Backlog 47** half closed: the pin test exists; the history line for `solver_build` needs Burhan's word.
- **No cache**: every read re-loads and re-verifies the run. Fine for the fixtures; P3 may want one for band
  switching on large runs, and a particle file crosses IPC at about its `.pbin` size.
