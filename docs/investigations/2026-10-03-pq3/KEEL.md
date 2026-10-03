# KEEL brief: Simulate settings editor (PQ3), 2026-10-03

VERDICT: findings(6)

## PRIOR
- No editor built before. PLAN.md:157-158 and :1058 (2026-09-29, M11 PQ3): "No: shown read-only, as drawn"; editor = own v1 piece before M12. scope.md:15. Parity matrix:545 sized it "one M-sized panel, ~25 fields, inline validator messages, over SetSolverSettings/SetEnvironment/SetBands/SetBandComputed (ops.rs:213-223)".
- Read-only rows live in app/ui/src/features/simulate/model.ts:265-357 (M11, merged bbe33b9, gate 31/31 at c9edf1e). Editing must stay out of `[data-input]` exemption: m11-h exempts only listed regions (GATE.md F2, 72e6f25); an editable field there is flagged by rule 1x.
- M10 pattern (m10/PLAN.md section 9 traps, 2026-09-29): ops go as TEXT through Op::from_json (serde_json reader not correctly rounded; -0 and NaN lose in JSON.stringify); native dialogs not WebDriver-drivable, e2e goes via hook calling the same action; B: exFAT leak, builds to C:/tmp/nm-target; tauri-driver port 4444 lock; React StrictMode double-mount.
- HANDOFF-2026-10-03.md:34-36: "Plan first (keel/compass at arc open), tests first; the new defaults are what the editor shows."

## PROBE (core API already exists; editor binds to it)
- Op::SetSolverSettings (ops.rs:216, applied :1075 with integrity::check_solver_settings), SetEnvironment (:213), SetBandComputed (:219, :1081), SetBands (:120). SppsSettings/TcrSettings/for_bands at model.rs:915-1010. app bridge commit() bridge.rs:301.
- Validators exist (validate/project.rs): time() :530 (positive, step count < MAX_TIME_STEPS 65,536 validate.rs:287, source delay vs step), spps_settings() :608 (eps >0 finite; particles 1..=i32::MAX; saved <= particles), radius :321, echogram_per_source name-collision :772. Integrity only caps C ints at i32::MAX (integrity.rs:326).
- Probe: one test that SetSolverSettings round-trips each field through save/load, and that validate() returns the code for each bad value. Not run by me. Defaults: 10 s / 1 ms = 10,000 steps (limit 65,536); at 10 s the floor is 0.000153 s.
- Gap found: grep of validate/ finds no refusal when every band is off (parity-matrix:241 same); no check that bands_computed.len() == bands after SetBands other than band_count. A run with zero bands computed must be refused or the editor must block it.
- Not read: upstream solver hard limits beyond 16-bit step counter (cited from validate comments: base_core_configuration.cpp:94, sppsNantes.cpp:91-93).

## ADRIFT
- Load-bearing: does every field the editor lets through reach config.xml as a value the solver honours AND the validator refuses the ones it kills silently (eps<=0, step count >= 65,536, particles_saved > particles, zero bands, echogram_per_source name collision)? Arc "done" = those refusals shown inline before Run, not "fields are editable".

## RULED (made, not built or not fully built)
- Row 11 (decision-log:22, 2026-09-27): 1 ms default; imports keep own, legacy .proj none -> 10 ms. Built (8f2a662, for_bands model.rs:970). Editor must not silently overwrite an import's value with the default.
- Row 36 (:48, 2026-10-01): 10 s fixed; room-based rule deferred backlog 57. Row 38 (:50, 2026-10-02 14:56): energetic default, "random stays available"; open follow-up "whether random should warn on T20/T30" (not built). Energetic EDT marked not validated (row 37(2), :49): mark must stay visible when method is toggled. Row 41 (:53): eps 7 new projects, imports keep own. Row 43 (:55): 7 octaves 125-8k new projects, imports keep own.
- Rulings silent (needs Burhan or a call, none found): is trans_epsilon user-visible (C13 "niche, v1.x with C12", parity:580 -> deferred, so NOT in v1 editor unless decided; but eps 3 gave T30 11.8% short, params.md:371); bands outside 125-8k (BandSet::range, C26 six presets, parity:410, "goes with picker"); echogram_per_source default (parity:561 "default on with 2+ sources"; for_bands sets false, model.rs:~975: NOT BUILT); particles_saved default 0 (parity:558; M12 playback empty; for_bands 0: NOT BUILT; M12 gate (d) needs C8>0); "auto" particles (row 38 open, design:668 "auto" vs true 150,000 label, REVIEW-gate-design:270 deviation 17 says PQ3).
- backlog 57 (run-length rule), 69-73 (bed items) adjacent, not this arc.

## TRAPS
- sound_maps_per_band (C21) default true already (model.rs for_bands) though parity audited "Core only"; check the editor reflects, not flips, it.
- Dirty-flag/PQ1: Run saves first; edits must mark dirty and go through ops/undo, not write the file.
- Imports keep own values (rows 11/41/43): editor displays true values; a bands change via SetBands rebuilds per-band arrays (ops.rs:633 each new band takes ...) and resizes bands_computed.
