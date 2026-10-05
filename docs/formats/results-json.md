# `simpa results --json`: the results of a run, for M12

The JSON the Results screen (M12) reads: a verified run's results and, per point receiver and
band, `core::params`' eight parameters, each a value or the reason it has none (for every TCR
receiver, all eight refused `no_time_series`), with sound strength G per band (`g_db`), the
A-weighted level per receiver (`aggregate.dba`) and STI per SPPS receiver (`sti`). Written by `core::results::report`
(`crates/simpa-core/src/results/report.rs`); what is read and refused is `docs/results.md`.

**The JSON Schema is `docs/formats/results-json.schema.json`**, generated from the same Rust types
that serialise the output (`simpa results --schema`; object `report` for a run read, `refusal`
for a run refused; draft 2020-12). `crates/simpa/tests/cli_results.rs` holds the output to it:
- `the_committed_schema_is_the_one_results_schema_prints`: the committed file is what the binary
  prints;
- `every_report_and_refusal_validates_against_the_committed_schema` (M7 follow-ups; the M7 critic:
  only the text and one report's top-level keys had been checked): a JSON Schema validator (the
  `boon` crate) validates the report of every committed run, SPPS and TCR, and a refusal of
  each exit, 5 and 6, against the committed schema, every field at every depth. Says no: a report
  with a string for a number, a fraction for a band, a required field removed, a value that is
  neither a value nor a refusal, a `null` energy or a number for the curvature flag fails it, and
  so does a refusal with a string for its exit code.

The schema does not forbid extra fields (`additionalProperties` is not set): a reader may meet a
field this page does not list, and must ignore it.

## Command and exit codes

```
simpa results <run-folder> [--json]
simpa results --schema
```

| Exit | When | stdout | stderr |
|---|---|---|---|
| 0 | the run's results were read (some parameters may still be not evaluable, and the solver build may be unverified: `solver_build`) | the report (JSON with `--json`, a table without) | nothing |
| 2 | a usage error: no folder, an unknown option, a second folder, a path that is not a folder, `--schema` with anything else | nothing | `simpa: <what>` |
| 5 | the run is FAIL, CRASH or CANCELLED (`results_run_failed`, `results_run_cancelled`) | with `--json`, the refusal | `simpa: results refused: <code>: <detail>` |
| 6 | the results do not verify (every other code of `docs/solver-contract.md`, "Result refusals") | with `--json`, the refusal | the same |

5 and 6 are the plan's stable codes, "5 solver run, 6 result verification" (raw plan JSON,
`plan.architecture`, its `cli` line; the `cli` component lists none), which `docs/m5-m6-design.md`,
"Exit codes", words as "5: solver run FAIL or CRASH", "6: result verification (M7)" and "130:
cancelled". A run whose verdict is FAIL or CRASH is a failed solver run: 5. **A CANCELLED run is
refused with 5 as well, not 130**: 130 says the command itself was cancelled, as `simpa run` exits
130 when its run is; `simpa results` was not cancelled, it read to the end the folder of a solver
run that did not succeed, and the refusal's code, `results_run_cancelled`, says how. (The first
text justified 5 for a cancelled run by the plan's "5 solver run" alone, which the design words as
FAIL or CRASH; the M7 critic.) A folder whose verdict says OK but whose manifest, inputs or outputs
no longer verify is not a failed run but results that fail verification: 6.

Without `--json` the table starts with `UNVALIDATED: M8's physics bed has not passed; these numbers
are not for publication.` After the run's own line comes the solver build's verdict, with its code
on the same line: `solver build verified: ...` or `solver build UNVERIFIED <code>: <detail>`.

## Numbers

- Every number is a JSON number: a finite `f64`, printed as the shortest decimal that reads back to
  the same `f64`. No value in a report is NaN or infinite: a run holding one is refused, and the
  report itself is walked for non-finite numbers before it is printed (`report::checked_report`,
  exit 6, `results_value_invalid`), because JSON would print one as `null`, indistinguishable from
  an absent value.
- **`null` appears only at these keys:** `spps`, `tcr` (the other solver's), `mc_sd` (a value not
  from a Monte-Carlo histogram), `band_hz` and `aggregate` (a surface file's: the first for the
  `Global` file, the second for a band's), `curved`, `decay_curve`, `field`, `air_m_per_metre`,
  `onset`, `position_m`, `arrival_s`, `decay_arrival`, `floor_db`, `lost_share`,
  `unfinished_share`, `crossings`,
  `crossings_per_particle`, `lambert_walls` and `uniform_lambert_walls` (a quantity's calibration that has no entry of its own for those walls),
  `free_paths` (the reference's, when the transport refused), and inside a refusal's typed
  `error`, `sd`, `with_tail`, `with_missing`, `low`, `high`, `particles_at_least` and
  `receiver_radius_scale_at_most`
  (`cli_results.rs`, `every_null_in_a_report_is_at_a_nullable_key`).
- Values read from the solvers' files are their `f32`, widened exactly. A reader that is not
  correctly rounded (serde_json's default is not) can come back one `f64` unit off; compare such
  values as `f32`. JavaScript's `JSON.parse` is correctly rounded.
- Units are in the field names: `_s` seconds, `_db` decibels, `_m` metres, `_m2`, `_m3`, `_hz`,
  `_pa2` pascals squared. `d50` is a fraction from 0 to 1, shown as a percentage.
- **Aggregates are labelled.** A value over all bands is never in a band's place, and every one
  sits in an object whose `aggregate` field says what it is (M7 follow-ups; the M7 critic found
  TCR's receiver `Global` values unlabelled):
  - an SPPS receiver's (and a source's) `aggregate`: `"all computed bands summed bin by bin"`;
  - a TCR receiver's `aggregate`, which sums nothing: `"none: TCR writes no series to sum"`, with
    `bands_hz` empty and every value refused `no_time_series`;
  - TCR's `global` and a TCR receiver's `global` (its `Global` row): `"energetic sum of the band
    levels"`;
  - a surface file's `Global` file: `"aggregate": "all computed bands: the solver's Global file"`,
    with `"band_hz": null`; a band's file has `"aggregate": null`.

## A report

```
{
  "results_version": 16,              // 16: lost particles reported, not bounded
                                      //    (decision 56): a band's lost_share is lost
                                      //    over emitted, beside lost_status; every
                                      //    quantity of a band's series (dB(A), STI:
                                      //    of its bands) carries lost_share_warning
                                      //    from 0.3 % and is refused lost_particles
                                      //    from 1 %;
                                      //    unfinished_share (the particles left alive
                                      //    at the end of a complete band) replaces the
                                      //    old lost_share's other part;
                                      //    lost_follows_decay is gone;
                                      // 15: edt_s refused missing_moves where the
                                      //    floor's and lost particles' energy, added
                                      //    back, moves EDT outside its own range;
                                      // 14: the room's volume is its air's, without the
                                      //    inside of closed obstacles, in room,
                                      //    reference and analytic and every time from
                                      //    it; room carries obstacle_volume_m3;
                                      // 13: range_below_zero, a refusal for an EDT,
                                      //    T20, T30, D50 or Ts whose range reaches below
                                      //    zero (shown wide with it before);
                                      // 12: room, the room from the run's own inputs
                                      //    (volume, area, DIN 18041 targets, absorption
                                      //    by surface group), for either solver; an SPPS
                                      //    reference band carries sabine_s;
                                      // 11: bed, each parameter's bed status from
                                      //    beds/summary.json, and validated_by_bed read
                                      //    from it (true only when every one is PASS);
                                      // 10: SPPS point receivers carry sti, the speech
                                      //    transmission index (IEC 60268-16:2011): male
                                      //    (shown) and female, MTF and MTI per band; a
                                      //    new project computes 125 Hz to 8 kHz;
                                      // 9: a value refused for its resamples alone is
                                      //    shown wide from its stand-ins, with
                                      //    refused_resamples; a C50, C80 or D50 whose bin
                                      //    straddling te can move it past its limit is
                                      //    wide, with straddle [lo, hi];
                                      // 8: bands carry g_db (sound strength G) and
                                      //    aggregates dba (the A-weighted level); nothing
                                      //    else changes;
                                      // 7: the eight parameters' values carry status (ok |
                                      //    wide), lo and hi, their range; a value refused
                                      //    monte_carlo_noise for its standard deviation alone
                                      //    is shown, wide, instead (decision-log row 37 (3));
                                      // 6: edt_s is EDT v2.1 (params::edt) and bands carry `edt`;
                                      // 2: mc_sd, noise, floor, lost-share and per-source fields;
                                      // 3: TCR receivers carry parameters and an aggregate;
                                      // 4: lost_follows_decay; an arrival outside the onset
                                      //    bin refuses C50, C80, D50 and Ts only; bands carry
                                      //    arrival, decay_arrival and
                                      //    early_reverberation_unresolved; bands and
                                      //    aggregates carry curvature and decay_curve; TCR
                                      //    receivers' global; surfaces' aggregate and
                                      //    receivers' id; 5 (pre-M8): spps.reference;
                                      //    noise calibrated per computation method
                                      //    (monte_carlo.method and .calibration,
                                      //    noise_model.method and .particles), and a
                                      //    refusal for noise carries particle_count;
                                      //    round 3 of the calibration: a correction
                                      //    for repeated crossings and a domain
                                      //    (monte_carlo.crossings_variable,
                                      //    .measured_on, the calibration's kappa,
                                      //    min_particles, max_crossings_per_particle
                                      //    and lambert_walls; noise_model.run; bands'
                                      //    and aggregates' crossings_per_particle;
                                      //    refusals noise_uncalibrated)
  "validated_by_bed": false,          // true only when every parameter in bed is PASS
  "bed": {                            // beds/summary.json as this build carries it (M12)
    "summary_sha256": "<64 hex>",     //   the compiled-in file's sha256
    "summary": "beds/summary.json",   //   where the evidence is: artifacts, sha256, rule
    "parameters": {                   //   one per parameter: spl_db, edt_s, t20_s, t30_s,
      "t30_s": {                      //   c50_db, c80_db, d50, ts_s, sti, g_db, dba
        "status": "PASS" | "FAIL",    //   FAIL: not rendered (M12 gate (b))
        "reasons": ["..."],           //   why it failed; empty for a PASS
        "notes": ["..."]              //   the marks, the wide rules, what was not tested
      }, ...
    }
  },
  "run_folder": "<as given>",
  "solver": "spps" | "tcr",
  "status": "OK",                     // always: any other run is refused
  "solver_build": {"status": "verified"}
    | {"status": "unverified", "reason": {"code": "solver_build_unrecorded", "detail": "..."}},
                                      // whether the solver build that made the run was
                                      // verified (core::results::solver_build, backlog 38):
                                      // it marks the run and refuses nothing; the codes are
                                      // docs/solver-contract.md, "Solver build"
  "started": "2026-09-24T11:50:30.959+02:00",
  "bands_hz": [500, 1000],            // the computed bands, ascending
  "spps": { ... } | null,
  "tcr": { ... } | null,
  "room": {                           // the room from the run's own inputs (M12 P2), read
    "status": "computed",             //   as TCR's analytic references read it
    "volume_m3": 180.0,               //   the air's: the .mbin's tetrahedra outside every
                                      //   closed obstacle (version 14)
    "obstacle_volume_m3": 0.0,        //   the tetrahedra left out: the obstacles' inside
    "area_m2": 216.0,                 //   the .cbin's faces
    "din18041": [                     //   A1 to A5, T_soll = a lg(V) + b at volume_m3
      {"group": "A3", "use": "teaching, communication",
       "target_s": {"value": 0.5517, "mc_sd": null}}, ...
                                      //   refused params_din_out_of_range outside a
                                      //   group's volume range
    ],
    "din18041_note": "...",           //   what a target applies to (80 % occupied, mid
                                      //   frequencies) and where the formulas come from
    "surfaces": [                     //   by material id: config.xml declares one
      {"material_id": 21, "faces": 2, "area_m2": 60.0,   // material per surface group
       "bands": [{"freq_hz": 500, "absorption": 0.3, "absorption_area_m2": 18.0}, ...]}
    ],
    "bands": [{"freq_hz": 500, "absorption_area_m2": 43.2}, ...]
                                      //   sum of S alpha: Sabine's area without air
  } | {"status": "not_computed", "why": "..."}
}
```

**The room, `room` (results version 12).** What the Results screen's Acoustics tab shows beside
the solver's values is computed here, so every number it shows is in this JSON (M12 gate (a)):
the volume and area, DIN 18041's five group-A targets at that volume (`params::din18041`; the
standard itself was not read, `din18041_note` says from where), and the absorption by surface
group: the `.cbin` faces grouped by material id, which config.xml declares one per surface group,
each with its faces, area, and per computed band its `absorption` (as the solvers read it, f32)
and `absorption_area_m2` (`S·α`); `bands` is each band's sum. Not computed, with why, for a scene
with fitting faces or one whose inputs do not read. Nothing of the solver's output is read.

**The volume is the air's (results version 14, backlog 85).** `volume_m3`, in `room`, in an SPPS
`reference` (where the transport's rays also start in it) and in TCR's `analytic`, sums the `.mbin`'s
tetrahedra of every region (`idVolume`) that reaches the mesh's outer surface
(`results::room::air_tetrahedra`). A closed shell nested in the room, a radiator or a stage panel, is
meshed as a region of its own that reaches only the air around it, so its inside is left out, and
`room.obstacle_volume_m3` says how much was: BRAS CR2's three radiator boxes 0.325 m³ (air 146.094 of
146.418), CR4's panels and canopy 39.05 m³ (air 8656.6 of 8695.7). Every Sabine, Eyring, Kuttruff and
DIN 18041 value takes this volume; versions up to 13 summed every tetrahedron, as TCR's own times in
`Main results.gabe` still do (`TC_CalculationCore.cpp:199-209`), so in a room with a closed obstacle
TCR's `bands` and `analytic` differ by the obstacle's share. A room without one reads the same
volume as before. Limits: a solid touching the outer surface, a box against a wall, reaches it and is
counted as air; a mesh of one region (upstream's without region attributes) is all air.

**The bed status, `bed` (results version 11).** Each parameter's status is read from
`beds/summary.json`, compiled in (`core::results::bed`). That file is written by
`tools/bed/summary.py` from the bed artifacts the result documents name as final, never by hand:
`crates/simpa-core/tests/bed_summary.rs` re-runs the script and requires the committed file byte for
byte. A parameter is PASS only when every check on every artifact holds (decision 39's product
grade); the Results screen renders a parameter only when its status is PASS. `validated_by_bed` is
true only when all eleven are PASS, so a reader must use the per-parameter status. The required
fields at every depth are pinned to `results_version` by a test (`report.rs`,
`the_required_fields_are_pinned_to_the_results_version`, backlog 47).

**`$SIMPA_BED_DEMOTE`, a test-only lever (M12 P4).** When set, it names a file of the summary's
shape, and a parameter is PASS only where both `beds/summary.json` and that file say PASS: it can hide
a number and never show one. A demoted parameter's `reasons` gain `demoted by $SIMPA_BED_DEMOTE
(<path>): ...`, `summary` reads `beds/summary.json, demoted by $SIMPA_BED_DEMOTE (<path>)`, and
`summary_sha256` stays the compiled-in file's. A file that cannot be read, or does not read, fails
every parameter. M12 gate (b)'s plant (`app/e2e/specs/m12.bedplant.e2e.ts`) runs the app and the
CLI with `tests/fixtures/beds/summary-c80-fail.json`, so the gate proves a FAIL is not rendered while
every real status is PASS.

### `spps`

| Field | What |
|---|---|
| `time_step_s`, `duration_s`, `steps` | `pasdetemps` and `duree_simulation` as SPPS stores them (`f32`), and the rows of every per-step table |
| `speed_of_sound_m_s`, `receiver_radius_m` | SPPS's `c` and `rayon_recepteurp` |
| `receiver_crossing_s` | `2R/c`: the direct sound is spread over this long at a receiver |
| `celerity_gradient` | `alog` or `blin` is not 0: no straight-line arrival is computed |
| `computation_method`, `particles_per_source`, `trans_epsilon`, `echogram_per_source` | `computation_method` (0 random, 1 energetic), `nbparticules`, `trans_epsilon` and `output_recp_bysource` as SPPS reads them |
| `monte_carlo` | how every value's noise was judged: `resamples`, `refused_resamples_allowed`, `seed` (a string, `0x` and 16 hex digits), the largest standard deviation allowed, `limit_decay_relative` (EDT, T20, T30), `limit_clarity_db`, `limit_definition`, `limit_centre_time_s`, `limit_spl_db`; `method` (`"random"` or `"energetic"`, the run's computation method, which picks the calibration); `crossings_variable` (`"crossings_per_particle"` or `"crossings_times_lifetime_spread"`: what each band's `crossings_per_particle`, `n`, is for this method); `measured_on` (what the calibration was measured on beyond what the code checks: room shapes, wall kinds, no transmission, no fittings); and `calibration`, per quantity by its name (`spl_db` … `ts_s`): `factor` and `kappa` (the bootstrap's standard deviation is multiplied by `factor·√(1 + kappa·n)`, calibrated against SPPS's own seed-to-seed spread), `min_particles` and `max_crossings_per_particle` (the domain: outside it the value is refused, `noise_uncalibrated`), `margin` (a refusal names the particle count at which the calibrated standard deviation would be the limit over it), `root_n_confirmed` (false: the spread was seen to fall slower than `1/√N` for the quantity in this method, and a refusal names no count), `structure` (how the resamples the factor applies to are drawn: `"constant"`, M7's deposit `d̄` in every bin, or `"roughness"`, each bin's deposit read from the series' own roughness, energetic T20 and T30 outside uniform Lambert rooms since round 4), `lambert_walls` (energetic T20 and T30: the same fields for bands whose every face reflects by Lambert's law with scattering 1 and not every face has the same absorption, equal to the quantity's own since round 4; `null` for the others) and `uniform_lambert_walls` (energetic T20 and T30: for bands whose every face reflects by Lambert's law with scattering 1 and has the same absorption, at most `uniform_lambert_max_mean_absorption`; `null` for the others) (`docs/params.md`, "Monte-Carlo noise"; `docs/investigations/2026-09-25-noise-calibration/`) |
| `sources[]` | `name`, `position_m` (`null` when not read), `emission_s` (`ceil(delay/dt)·dt`), `band_power_w` (per computed band, W, as SPPS computes it), `balloon` (a directivity balloon: its values are refused, `noise_unknown`) |
| `particles` | the statistics per band: absorbed by the atmosphere, the materials, the fittings; lost by loops and meshing; remaining; total |
| `total_energy[]` | per band, the room's energy per step (`<cumul_filename>`) |
| `point_receivers[]` | below, in the order of their folder names |
| `surfaces[]` | each surface-receiver and cutting-plane file, summarised: `path`, `field` (`null`), `band_hz` (`null` for `Global`), `aggregate` (the `Global` file's label, `null` for a band's), `cutting_plane` (the cutting planes' file, `rs_cut.csbin`, not the scene receivers' `Sound level.csbin`: kept apart by name, each holding only receivers of its kind, `docs/results.md`), `record_type`, `time_steps`, `time_step_s`, and per receiver its `id` (its `config.xml` id), `name`, `faces`, `records` and `value_sum`. The values stay in the `.csbin` |
| `particle_files[]` | when particles were saved: `path`, `freq_hz`, `particles` (those written: of the `nbparticules_rendu` per source, the ones that recorded a step) and `recorded_steps` (their step records together). Each file is checked against its run (`docs/results.md`, "Saved particles") |
| `reference` | the analytic reference on the run's own inputs, below. **Not validated** |

#### `reference`

M8's and M12's reference for the run's reverberation times (pre-M8; Burhan, 2026-09-24 23:14;
`docs/params.md`, "Kuttruff's reference"), computed from the run's inputs alone (the `.cbin`
faces and materials, the `.mbin`'s tetrahedra, `config.xml`'s air and SPPS's `c`), never from its
output. **Nothing in it is validated**, and `label` says so beside the numbers:

```
"reference": {"status": "computed",
              "label": "analytic reference for a diffuse field, not validated (M8's bed has not run): ...",
              "volume_m3": 180.0, "area_m2": 216.0,           // the air's volume, the .cbin's area
              "speed_of_sound_m_s": 343.20001220703125,       // SPPS's c
              "constant_s_per_m": 0.16101993084580937,        // K = 24·ln 10/c
              "free_paths": {"mean_free_path_m": 3.3340, "mean_free_path_se_m": 0.0004,
                             "gamma2": 0.38883, "gamma2_se": 0.00014,
                             "four_v_over_s_m": 3.3333, "volume_m3": 180.0, "area_m2": 216.0,
                             "paths": 33554432,
                             "settings": {"replicas": 16, "rays_per_replica": 4096,
                                          "burn_in_paths": 32, "paths_per_ray": 512,
                                          "seed": "0x6c616d6265727431"}} | null,
                                                              // always the fixed STANDARD; null
                                                              //   when no band has Lambert walls
              "bands": [{"freq_hz": 500, "air_m_per_metre": 0.000628 | null,
                         "mean_absorption": 0.2,              // ᾱ = Σ Sᵢαᵢ / S
                         "lambert_walls": true,               // every face Lambert, scattering 1?
                         "sabine_s": {"value": 0.6667, "mc_sd": null}, // results version 12
                         "eyring_s": {"value": 0.5957, "mc_sd": null},
                         "kuttruff_s": {"value": 0.6225, "mc_sd": 0.0000103}}, ...]}
                                                              // mc_sd: gamma^2's share only; a
                                                              //   refusal where lambert_walls is
                                                              //   false
| {"status": "not_computed", "why": "..."}
```

- **`kuttruff_s` is M8's reference**: Kuttruff's corrected Eyring,
  `K·V/(4·m·V − S·ln(1 − ᾱ)·[1 + (γ²/2)·ln(1 − ᾱ)])`, with `γ²` from `free_paths`, which a diffuse
  ray transport computed from the room's geometry alone, at fixed settings. Its `mc_sd` is **only**
  the standard deviation it inherits from `γ²`'s standard error, not its total uncertainty: it
  leaves out the formula's own error against a diffuse room, which no ray count reduces (−0.41 %
  to +0.59 % in M8's cells, `docs/params.md`, "Kuttruff's reference"; not measured in other
  rooms). A comparison that divides by `mc_sd` alone would fail a correct solver for the
  formula's error. **`eyring_s` is plain Eyring, and `sabine_s` Sabine,
  `K·V/(4·m·V + Σ Sᵢαᵢ)` (results version 12), both reported only.** All three use SPPS's `c` in `K` and
  the solver's own air term `m` (`null` with air absorption off).
- **Both describe a diffuse field.** `lambert_walls` says whether every face reflects by Lambert's
  law with scattering 1 in the band, the only walls the transport's `γ²` describes; with specular
  or partly specular walls neither time describes the run's field, and `kuttruff_s` is refused
  `params_reference_not_applicable` there (pre-M8: the transport is not run for such bands, and
  not at all when no computed band has Lambert walls; `eyring_s` is given whatever the walls).
- `free_paths` is `null` when the transport refused (a ray left the room, its mean free path is
  not `4V/S` within its error, or `γ²`'s standard error is above 0.002), every band with Lambert
  walls then carrying that refusal in `kuttruff_s`, `params_transport_refused`; and when no
  computed band has Lambert walls, so that it was not run. `eyring_s` is still given. `free_paths.settings` is always the transport's fixed `STANDARD`: nothing a caller
  chooses reaches it. Its `seed`, like `monte_carlo.seed`, is a string, `0x` and 16 hex digits:
  as a JSON number above 2⁵³ it would be read as another seed by any reader that parses numbers
  as doubles.
- `not_computed` when the scene has fitting faces (as TCR's `analytic`), the speed of sound varies
  with height (`celerity_gradient`), or the inputs do not read.

A point receiver:

| Field | What |
|---|---|
| `label`, `folder` | the folder's name, which is exactly one `config.xml` label, and its path under `solve/` |
| `position_m` | as SPPS stores it; `null` when not read |
| `arrival_s` | the direct sound's arrival at the centre, which every onset-relative parameter is measured from, the direct sound spread over `±receiver_crossing_s/2` about it; `null` when not computed, and the parameters then detect it. When it lies before a band's onset bin, or after it with the leading edge of its spread at or after the bin's end, C50, C80, D50 and Ts are refused `params_bad_arrival`; SPL, EDT, T20 and T30 are not |
| `bands[]` | per computed band: `freq_hz`; `complete` (random mode, `trans_epsilon` above 0, and SPPS's statistics count at most one particle in a million remaining when the steps ran out, so no tail after the series is bounded; lost particles, and those few remaining, do not make a band incomplete: the remaining ones' unfinished paths are bounded by `unfinished_share`, the lost ones are reported by `lost_share`); `floor_db` (energetic mode's `-10·trans_epsilon`, or `null`); `unfinished_share` (the share of the energy from the arrival on that the particles left alive at the end of a complete band can have taken, or `null` when there are none; version 16); `lost_share` (version 16, decision 56: the share of the band's particles SPPS lost to loops and meshing, over those emitted, or `null` when none was lost; reported, not bounded; until version 15 the share of the energy the lost and remaining particles were bounded to have taken); `lost_status` (`ok` below 0.3 % lost, `warning` from it, `refused` from 1 %: "Lost particles", below); `early_reverberation_unresolved` (always `true` for SPPS: each value is midway between the reverberation beginning at the arrival, at the first bin wholly after the direct sound and at that bin's end, or refused `early_unresolved`; `docs/params.md`, "The early reverberation"); `arrival` (what C50, C80, D50 and Ts are measured from: `{"arrival": "known", "time_s": …, "half_width_s": …}`, the direct sound at `arrival_s` spread over `±receiver_crossing_s/2`, or `{"arrival": "detected"}`); `decay_arrival` (what EDT, T20 and T30 are measured from, the same shape, or `null` when the series is refused); `contributing_sources` (the sources whose `.recps` total is above 0: with more than one, the seven onset-relative parameters are refused, `several_sources`); `noise_model` (`{"model": "crossings", "mean_deposit": …, "method": "random" | "energetic", "particles": …, "run": {"particles", "least_deposit", "lifetime_cv2", "lambert_walls", "uniform_absorption", "mean_absorption", "bands", "receiver_crossing_s"}}`, the mean deposit in Pa², the particles per source, and what the calibration's correction, domain and structure take from the run: the least of the contributing sources' deposits, the spread of the particles' lifetimes from the band's room table, whether every face is Lambert with scattering 1 in the band, whether every face has the same absorption, the faces' mean absorption, the bands summed, and `2R/c`, the roughness structure's shortest block; or `{"model": "unknown", "detail": …}`); `crossings` (the receiver crossings the model implies, or `null`); `crossings_per_particle` (`n`, the crossings of the receiver per particle as the calibration measures them, which its correction and domain take, or `null`); `energy_pa2` (the `.recp` series, one per step) and `total_pa2`; `source_power_rho_c` (Pa²·m², the free field at `r` is this over `4πr²`); `background_noise_db`; `onset` (`index`, `bin_start_s`, `bin_end_s`, or `null`); `parameters`; `g_db` (sound strength G, below); `curvature` and `decay_curve` (below) |
| `aggregate` | `aggregate` (the label), `bands_hz` (the bands summed), `crossings_per_particle` (the aggregate's own `n`: its bands' particles together, at the least deposit of any band and the largest lifetime spread; `null` for TCR), `parameters`, `dba` (the A-weighted level of the bands' SPL, below: a sum of band levels, not of the summed series), `curvature`, `decay_curve`. **Not ISO 3382-1's single-number value** (a mean of band values): one decay of all bands' energy, weighted by the source spectrum. Never show it as the room's value |
| `sti` | the speech transmission index at the receiver (results version 10, below): `method`, `weighting`, `shown` (`"male"`), `male`, `female`, `speech_level_dba_at_1m`, `noise`, `monte_carlo`, `modulation_hz`, `bands[]` |
| `by_source[]` | `source` and its `energy` per band, Pa² |
| `per_source[]` | with `echogram_per_source`, one per source in `config.xml`'s order: `source`, `file`, `arrival_s` (from that source alone), `bands[]` (`freq_hz`, `arrival`, `decay_arrival`, `noise_model`, `crossings`, `crossings_per_particle`, `energy_pa2`, `total_pa2`, `onset`, `parameters`, `g_db` (against that source's own free field), `curvature`, `decay_curve`) and `aggregate` (with that source's `dba`): the parameters of that source–receiver pair. Empty otherwise |

`parameters` holds `spl_db`, `edt_s`, `t20_s`, `t30_s`, `c50_db`, `c80_db`, `d50` and `ts_s`, each
exactly one of:

```
{"value": 64.3876, "mc_sd": 0.041, "status": "ok", "lo": 64.2851, "hi": 64.4901}
{"value": 0.83, "mc_sd": 0.099, "status": "wide", "lo": 0.5825, "hi": 1.0775}
{"not_evaluable": {"code": "params_not_evaluable",
                   "message": "params_not_evaluable: T20: monte_carlo_noise: ...",
                   "error": {"kind": "not_evaluable", "quantity": {"quantity": "t20"},
                             "why": {"why": "monte_carlo_noise", "value": 0.83, "sd": 0.009,
                                     "limit": 0.025, "resamples": 200, "refused_resamples": 31,
                                     "particle_count": {"count": "resampled", "multiple": 4,
                                                        "margin": 1.3,
                                                        "particles": 600000}}}}}
```

`mc_sd` is the value's estimated Monte-Carlo standard deviation in its unit, calibrated against
SPPS's own seed-to-seed spread (`monte_carlo.calibration`); every SPPS value carries one.

**Sound strength G, `g_db` (results version 8).** Beside `parameters` on every band (SPPS, a
source's, TCR's), the same shape as a parameter: `spl_db` less the level of the free field at 10 m
of the same sources, `G = 10·lg(Σ B_k / (source_power_rho_c/(4π·100 m²)))` (ISO 3382-1:2009 A.2.1,
Eqs. A.1-A.3; `params::level`). `source_power_rho_c` is SPPS's own sources' power times `ρc`, the
`ρc` of the deposits SPL sums, so `ρc` cancels: G is **not** the standard's `Lp − Lw + 31 dB`
(Eq. A.9), which assumes `ρc ≈ 400` and reads 0.15 dB high at SPPS's 413.25 (exact constant
30.85 dB). With several sources a band's G is every source's energy at the receiver against every
source's free field at 10 m, both summed; a source's own (`per_source[].bands[].g_db`) takes the
band's `source_power_rho_c` times that source's share of `band_power_w`. G is SPL less a constant:
it carries SPL's `mc_sd`, `status` and range shifted by that constant, and where SPL is refused G
is refused with the same refusal (TCR's `no_time_series` included). A band no source emits in is
refused `params_no_energy`.

**The A-weighted level, `aggregate.dba` (results version 8).** On every aggregate (a receiver's,
a source's, TCR's):

```
{"method": "energy sum over the computed octave bands of SPL plus the IEC 61672-1 A-weighting at the octave centre",
 "level_db": {"value": 71.2, "mc_sd": 0.03, "status": "ok", "lo": 71.125, "hi": 71.275},
 "bands_hz": [500, 1000], "weights_db": [-3.2, 0.0], "unweighted_hz": []}
```

`level_db = 10·lg Σ 10^((spl_db + A)/10)` over `bands_hz`, every computed band, with the
IEC 61672-1 A-weighting at the octave centres, `weights_db` (125 Hz −16.1, 250 −8.6, 500 −3.2,
1 k 0, 2 k +1.2, 4 k +1.0, 8 k −1.1 dB). Only the bands computed are summed: a run of 500 Hz and
1 kHz gives the A-weighted level of those two bands, not a broadband one. Its `mc_sd` is the
bands' propagated to first order, the bands independent (SPPS runs each band's particles on its
own): `√Σ (wᵢ·sdᵢ)²`, `wᵢ` band `i`'s share of the weighted energy; `status` and range as SPL's
(±2.5 `mc_sd`, `ok` within 1 dB). Refused when any band's `spl_db` is, with that band's refusal
(its `message` names the band); and `no_a_weight` when a computed band is not one of the seven
octave centres (it is listed in `unweighted_hz`, a third-octave run for instance): no other
weighting is pinned.

**STI, `sti` (results version 10).** On every SPPS point receiver (not TCR's, which has no
series; not per source), IEC 60268-16:2011 (edition 4) from the predicted energy response
(`params::sti`; `docs/params.md`, "STI"):

```
{"method": "IEC 60268-16:2011 (edition 4), indirect method: ...",
 "weighting": "male (IEC 60268-16:2011 Table A.3; ...), female computed beside it; calculated from an MTF derived from a predicted impulse response (cl. 8.3)",
 "shown": "male",
 "male": {"value": 0.612, "mc_sd": null},
 "female": {"value": 0.634, "mc_sd": null},
 "speech_level_dba_at_1m": 60.0,
 "noise": "none: the receiver has no background noise (...), so no noise term is applied",
 "monte_carlo": "not modelled: STI carries no Monte-Carlo standard deviation or range; ...",
 "modulation_hz": [0.63, 0.8, 1.0, 1.25, 1.6, 2.0, 2.5, 3.15, 4.0, 5.0, 6.3, 8.0, 10.0, 12.5],
 "bands": [{"freq_hz": 125, "mtf": [0.97, ...], "transfer_db": -6.1,
            "speech_male_db": 56.8, "speech_female_db": null, "noise_db": null,
            "mti_male": 0.71, "mti_female": null}, ...]}
```

- **`male` is the value shown** (A.3.4: male speech assesses a channel), `female` beside it; both
  are 0 to 1, truncated at 1.0 (Table A.3's note), and a value is `{"value", "mc_sd": null}` with no
  `status` or range: **STI's Monte-Carlo noise is not modelled** (`monte_carlo` says so). A consumer
  that shows STI shows `weighting` with it: cl. 8.3 asks a predicted STI to say it is predicted and
  which weighting it carries.
- `bands[]`: one per octave of the run from 125 Hz to 8 kHz, ascending: `mtf`, the room's
  modulation transfer at `modulation_hz`, before the level corrections; `transfer_db`, the band's SPL
  less the same sources' free-field level at 1 m; the speech level at the receiver for each speech
  (Table A.4 at `speech_level_dba_at_1m` plus `transfer_db`; female has no 125 Hz); `noise_db`, the
  background noise applied; and each speech's `MTI_k` (`null` when that speech's STI is refused).
  `mtf` and `transfer_db` are `null` for a band that cannot be read.
- `noise` names the noise applied: the receiver's background noise per band when it has one, or
  none (a receiver without one is written 0 dB in every band, and is read as having none).
- **Refused** (each speech on its own, `params_not_evaluable` with `quantity` `sti`, unless said):
  `band_missing` (an octave the speech needs is not in the run: 125 Hz to 8 kHz, female 250 Hz to
  8 kHz; with `freq_hz` and `needed_hz`: a run of the old 6-band default refuses for 8 kHz),
  `band_refused` (an octave it needs cannot be read: its series is refused or not complete, its SPL
  is refused, no source emits in it, or it has no T30, T20 or EDT to check the run's length
  against; with `freq_hz` and `detail`), `not_octave_bands` (a third-octave run), `several_sources`
  (more than one source reaches the receiver: STI is one talker's), and `params_series_too_short`
  when the response from the direct sound is shorter than 1.6 s or half the longest reverberation
  time of its bands (cl. 6.2 b, 8.3 a).

**Two more `wide` values (results version 9; the bed's findings, `docs/investigations/2026-10-02-bed/`).**
- **`refused_resamples`: shown from its stand-ins.** A value `params` refused `monte_carlo_noise`
  because more than 10 of its 200 resamples refused it, which at most 10 refuse when the same
  resamples are judged with their decay range on the series itself (`params::noise`, "The
  stand-ins"; on the bed's G2 every T30 was refused so, its resamples ending in a few whole deposits
  above -35 dB), is shown: `"status": "wide"` always, `refused_resamples` how many refused it as
  judged, `mc_sd` the judged standard deviation (the stand-ins' when the judged resamples gave none),
  and `lo`/`hi` = `value ∓ 2.5·sd`, `sd` the larger of the stand-ins' calibrated one and `mc_sd`. The
  curvature stays refused with it. A value whose stand-ins also refuse it more than 10 times stays
  refused.
- **`straddle`: the bin straddling te.** A C50, C80 or D50 whose bin straddling its window edge te,
  taken wholly late and wholly early, moves it by more than its limit (0.1 dB, 0.005;
  `params::decay::Straddle`) is `"wide"` with `straddle: [lo, hi]`, the value with that bin each way,
  and `lo`/`hi` covering it: `min(straddle[0], value) − 2.5·mc_sd` to `max(straddle[1], value) +
  2.5·mc_sd`. At a step of 10 ms that is most C and D; at 1 ms, the bands whose straddling bin holds a
  strong reflection.

Both fields are absent from every other value. With either, `lo`/`hi` are no longer `value ∓
2.5·mc_sd`, and `status` is `wide` whatever their width.

**Lost particles (results version 16, decision 56).** Lost particles are reported, not bounded, as
Odeon reports lost rays. A band's `lost_share` is the share of its particles SPPS lost (loops and
meshing) over those emitted; `lost_status` says what it does to every quantity of the band's series (SPL, G, EDT, T20, T30, C50, C80, D50, Ts and the curvature; dB(A) and STI by the largest share of the bands they use), of the band, of its per-source
bands and (the largest share of any band) of the aggregate:
- below 0.3 % (`ok`): shown as they are;
- from 0.3 % (`warning`): shown, each value carrying `lost_share_warning`, the share (a fraction):
  the late decay may hold slightly too little energy. The field is absent from every other value
  (TCR's, the references');
- from 1 % (`refused`): refused `lost_particles` (`params_not_evaluable`, `why`: `share`, `limit`
  0.01), whatever else they read: the model is broken (holes, a bad mesh). The run's verdict refuses
  a band over 1 % whole (`particle_loss_excess`), so this is seen at 1 % exactly or under a raised
  loss limit.

The thresholds are a forecast, by linear scaling of the worst room the 10-05 hall bed measured
(Elmia, 0.15 % lost moved T30 by 5.3e-4: 0.36 × the share), so 0.3 % moves T30 about 1e-3 and 1 %
about 3.6e-3 against the 5e-3 limit (`docs/investigations/2026-10-05-b82-b84/FINDINGS.md`;
`results::spps::LOST_SHARE_WARNING`, `LOST_SHARE_REFUSED`). Until version 15 the lost particles'
energy was bounded instead (`ENERGETIC_LOST_ENERGY_RATIO` × lost/emitted following the decay in
energetic mode, a lump from the arrival in random mode) and refused whatever it could move; that
bound refused T20 in BRAS CR2 at 0.03 % lost and no longer refuses anything.

**The range (results version 7; decision-log rows 37 (3) and 39 (3)).** Every value of the eight
parameters of an SPPS band, aggregate or per-source band carries `status`, `lo` and `hi`: the range
`value ± 2.5·mc_sd` (`params::noise::RANGE_Z`, EDT's Z) and `"ok"` when its half-width is within the
quantity's difference limen, `"wide"` when it is not. The limens are ISO 3382-1 Table A.1 as this
project carries them, twice the `monte_carlo` limits (`params::noise::jnd`): 5 % of the value for EDT,
T20 and T30; 1 dB for SPL, C50 and C80; 0.05 for D50; 10 ms for Ts. EDT's are the method's own
(`lo` = `edt.lo_s`, `hi` = `edt.hi_s`, `status` = `edt.status`; `mc_sd` stays `null`). **A value whose
standard deviation alone is above its `monte_carlo` limit is no longer refused: it is shown, `wide`,
with the value and `mc_sd` the refusal carried** (always `wide`: its half-width is at least 1.25
limens). A consumer that shows a value shows its range beside it, and marks a `wide` one. Still refused:
`monte_carlo_noise` when more than 10 of the 200 resamples refuse the value (the spread of the
resamples that gave one does not bound those that did not) or when there is no standard deviation, and
every refusal not about noise, `noise_uncalibrated` and `noise_unknown` included (but see version 9's
stand-ins, next). **Since version 13, an EDT, T20, T30, D50 or Ts whose range reaches below zero is
refused `range_below_zero`** (`value`, `lo`, and `sd`, `null` for EDT), whatever made the range
(decision-log row 48): the quantity cannot be negative, so such a range says the noise is too large for
the value to mean anything. SPL, C50 and C80 keep any range. `status`, `lo` and `hi` are absent from every other value (TCR's, the reference's, `curvature.percent`). The curvature
is still refused with a T20 or T30 that `params` refused for noise, **even though that T20 or T30 is now
shown `wide`** (unless its range reaches below zero, when both are refused): a consumer will see both values beside a `monte_carlo_noise` curvature. A refusal for
`monte_carlo_noise` carries `particle_count`, the particles per source that would bring the value
within its limit, or why none is named: `{"count": "named", "factor", "margin", "particles"}`
(`factor` times the run's particles, `(margin·sd/limit)²`, and that many per source rounded up to
two significant digits), when the standard deviation is above the limit and at most 10 of the 200
resamples refuse the value; `{"count": "resampled", "multiple", "margin", "particles"}` when more
of its resamples refuse it (round 4, R4-3: `multiple` the first of 2, 4, 8, 16, 32 and 64 times the
run's particles at which the model's own resamples of the series, every deposit over the
multiple, refuse it at most 5 times and its calibrated standard deviation times `margin` is within
the limit, and `particles` that many per source); `{"count": "beyond_resampled", "multiple": 64}`
(its resamples still refuse it, or its noise is still above the limit, at 64 times the particles:
no count, and more particles may not help); `{"count": "scaling_not_confirmed"}` (the spread's
fall as `1/√N` was not confirmed for the quantity in this computation method);
`{"count": "no_standard_deviation"}`. The text output shows a named count, of either kind, as
`NE(noise:<count>)`, and `NE(noise)` when none is named. A run outside the
domain its quantity's calibration was measured on is refused `noise_uncalibrated`, with `value`,
`particles`, `crossings_per_particle`, `min_particles`, `max_crossings_per_particle`, and either
`particles_at_least` (too few particles: run that many per source) or
`receiver_radius_scale_at_most` (too many crossings per particle: they grow as the receiver
radius squared and do not fall with more particles, so the radius must shrink to at most this
multiple of the run's); the text output shows `NE(uncal:<count>)` or `NE(uncal:R<=<s>x)`. `code` is a row of `docs/solver-contract.md`, "Parameter refusals"; `error` is the typed
refusal, `why.why` one of `range_not_reached`, `truncated`, `unresolved`, `early_unresolved`,
`range_too_short`, `not_decaying`, `empty_window`, `missing_not_cleared`, `missing_moves`,
`lost_particles` (every quantity of an SPPS band's series, dB(A) and STI, version 16),
`monte_carlo_noise`, `noise_unknown`, `noise_uncalibrated`, `several_sources`, `no_time_series`,
`edt_refused` (EDT only: `reason` is one of `no_energy`, `no_energy_after_arrival`, `run_too_short`,
`direct_only`, `step_too_coarse`, `not_decaying`, `too_few_particles`, `not_decaying_at_run_end`,
`receiver_too_large`), `no_a_weight` (dB(A) only, with the band), `band_missing`, `band_refused`,
`not_octave_bands` (STI only, results version 10) for `params_not_evaluable`
(`docs/params.md`).

**EDT (results version 6).** `edt_s` is EDT v2.1 (`params::edt`, ported from
`docs/investigations/2026-09-27-edt-heldout/frozen2/method.py` and shown equal to it,
`docs/investigations/2026-10-02-edt-port/PORT.md`), read from the raw histogram: `{"value": …,
"mc_sd": null, "status", "lo", "hi"}` for `ok` and `wide` (version 7: the method's status and range),
or refused `edt_refused`. Wherever a histogram gave it (every SPPS
band, aggregate and per-source band; absent for TCR and for `several_sources`), `parameters.edt`
holds `method` (`"edt_v2.1"`), `status` (`ok`: the range is inside 5 %; `wide`: shown with its range,
decision-log row 9; `refused`), `value_s`, `lo_s`, `hi_s` (absent when refused), `reason` (a
refusal code, or the method's detail `hw=…;fit=…;noise=…;tail=…;n=…`), `arrival_s` (what the method
was given: the source's emission delay included, rounded up to the next whole step; absent when not
computed), `validated` and `validation_note`. **`validated` is true only for a single band, in either
computation mode, with a receiver radius up to 1 m and a direct path from the source**, what the
held-out test covered (VERDICT-2, H1-H6; decision-log rows 37 and 38). It is false for receivers over
1 m (5-7 % low in the test, VERDICT-2 ruling 1); for a receiver with no direct path, "start time
uncertain, no direct path from the source (the first arrival is estimated from the first recorded hit;
VERDICT-2 H3, G4 R007)": energetic mode's only failure in the test, 5-9 % low in both modes, and flagged
whenever the method finds no energy from the receiver ball's front to one step past its back, which a
run with too few particles can also give; and for **every aggregate (summed-bands, broadband) EDT
whatever the mode** ("broadband EDT is not covered by the held-out test"). `validation_note` names
every reason that applies, joined by `; `.

**Missing energy (version 15, backlog 84).** Where the solver's floor or the particles left alive at the
end of a complete band can have cost the series energy (`docs/params.md`, "Missing energy"), the method is
run again with that energy added to every backward sum; `edt_s` is refused `missing_moves` when that value
falls outside the method's own `lo`..`hi`, its `limit` the range's relative half-width on that side. `edt`
still holds the method's own value and range. Version 15 also added the lost particles' share (in energetic
mode, the most a share following the decay can move a decay time over 10 dB); since version 16 lost
particles are reported, not bounded ("Lost particles", above).

**The marker rule (decision-log row 20).** Only tested numbers are shown as validated; every EDT that is not
validated carries "not yet validated" on every surface that prints or exports it. `edt_s` stays a value
(`{"value", "mc_sd", "status", "lo", "hi"}`) because consumers read it, so its marker rides beside it: `parameters.edt_validated`
(boolean, present on every `parameters`; true only where `edt.validated` is true, false where there is no `edt`
object). A consumer that shows or exports `edt_s` reads `edt_validated` and marks the value when it is false.
`simpa results` marks such a cell with `*` after the value and prints a legend; a refusal has no value to mark.

**The text output (version 7).** `simpa results` prints a value with a range as `<value>±<h>`, `h` the
larger side's half-width rounded up at the shown precision (never shown narrower than it is), and
`<value>±<h>w` when `status` is `wide`, with a legend; EDT's `*` follows (`1.23±0.07w*`).

#### `curvature` and `decay_curve`

Added by the M7 follow-ups (the M7 critic: the curved-decay flag was computed and dropped, and the
curve the parameters come from was internal), so that M12's decay charts and curved-decay warnings
come from the code that gives the numbers, not from a second implementation.

```
"curvature": {"percent": {"value": 3.1, "mc_sd": 0.9},   // 100·(T30/T20 − 1), % (or a refusal)
              "curved": false,                            // |percent| > limit_percent; null
              "limit_percent": 10.0},                     //   when percent is refused
"decay_curve": {"from_s": 0.01303,           // the absolute time of u = 0: what EDT, T20 and
                                             //   T30 were measured from (the start of the onset
                                             //   bin when the arrival is detected)
                "points": [[0.0, 0.0], [0.0, -0.68], [0.017, -1.91], ...],   // [u s, level dB]
                "tolerance_db": 0.01,
                "knots": 30,                 // the whole curve's knots, before thinning
                "histogram_from_s": 0.00697} // from this u on, every knot is the histogram's own
```

- **`curvature.percent`** is `100·(T30/T20 − 1)` from the reported T20 and T30, with its
  Monte-Carlo standard deviation over the resamples that give both; refused, with T30's refusal
  or else T20's, whenever either is (the refusal's `quantity` is `curvature`). `curved` flags a
  double-slope decay, ISO 3382-2's `|C| > 10 %` as commonly stated. What a curved decay may show is
  M8's and M12's decision (`docs/params.md`, "Decay times").
- **`decay_curve.points`**, joined by straight lines, are the Schroeder curve EDT, T20 and T30 were
  fitted to within `tolerance_db` everywhere: `u` in seconds from `from_s`, the level in dB re the
  curve's value at `u = 0`, the direct sound included. The first point is `[0, 0]`; the second,
  also at `u = 0` when the direct sound steps the curve down, is the level just after it. The last
  is the start of the last bin with energy (the curve then falls to nothing inside one bin, which
  the fits leave out). How the knots are thinned: `docs/params.md`, "The decay curve, for display".
  **Before `histogram_from_s` the curve is one of three readings**, the reverberation continued
  back to the arrival, while each value is midway between the lowest and highest of the three
  (`early_reverberation_unresolved`, always `true` for SPPS). A line drawn on these points from
  0 to −10 dB gives an EDT that can differ from `parameters.edt_s` by up to EDT's limit, 0.5 %;
  T20 and T30 start below that stretch.
- `decay_curve` is `null` when the series is refused, when several sources contribute to the band
  (the seven onset-relative values are refused `several_sources`, and so is `curvature`), and for
  TCR, which writes no series (its `curvature` is refused `no_time_series`).

### `tcr`

| Field | What |
|---|---|
| `bands[]` | per computed band: `freq_hz`, and for `sabine` and `eyring` each `absorption_area_m2`, `reverberation_time_s`, `level_db`, as TCR wrote them |
| `global` | `aggregate` (the label), `sabine_level_db`, `eyring_level_db` |
| `point_receivers[]` | `label`, `file`; `bands[]`, per band `freq_hz`, `direct_db`, `total_sabine_db`, `total_eyring_db` (TCR's own levels), `parameters`, `g_db` and `curvature` (refused `no_time_series`) and `decay_curve` (`null`); `global` (the `Global` row, each column's energetic sum over the bands, **an aggregate**, labelled: `aggregate`, `direct_db`, `total_sabine_db`, `total_eyring_db`; a value that is not finite is refused, `results_value_invalid`); `aggregate` (SPPS's shape, labelled `"none: TCR writes no series to sum"`, `bands_hz` empty, `dba` refused `no_time_series`) |
| `surfaces[]` | as for SPPS, with `field` one of `Direct field`, `Total field (Sabine)`, `Total field (Eyring)` |
| `analytic` | `core::params`' Sabine and Eyring times on the run's own inputs, at the air's volume (version 14): `{"status": "computed", "volume_m3", "area_m2", "bands": [{"freq_hz", "air_m_per_metre", "sabine_s", "eyring_s"}]}`, the two times as `{"value": …, "mc_sd": null}` or `{"not_evaluable": …}`; or `{"status": "not_computed", "why": …}` |

TCR writes steady-state levels, not an energy time series, so `core::params` has nothing to compute
from. A TCR receiver still has the same `bands[].parameters` and `aggregate.parameters` as an SPPS
one, so M12 reads one shape, but each of the eight is refused:

```
{"not_evaluable": {"code": "params_not_evaluable",
                   "message": "params_not_evaluable: SPL: no_time_series: the solver wrote none: ...",
                   "error": {"kind": "not_evaluable", "quantity": {"quantity": "spl"},
                             "why": {"why": "no_time_series", "detail": "TCR writes steady-state levels only; ..."}}}}
```

SPL is refused too: TCR gives two totals, Sabine's and Eyring's, and neither is `params`' SPL of a
series. They are shown as TCR's own, `total_sabine_db` and `total_eyring_db`.

## A refusal

```
{
  "results_version": 5,
  "run_folder": "<as given>",
  "refused": {
    "code": "results_run_failed",
    "detail": "the run's verdict is FAIL at stage pre_launch: mesh_invalid",
    "reasons": [{"code": "mesh_invalid", "detail": "..."}]
  },
  "exit_code": 5
}
```

`reasons` carries the verdict's reasons for `results_run_failed` and `results_run_cancelled`, and
the output signals' reasons for `results_outputs_invalid`; it is empty otherwise.
