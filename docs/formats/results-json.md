# `simpa results --json`: the results of a run, for M12

The JSON the Results screen (M12) reads: a verified run's results and, per point receiver and
band, `core::params`' eight parameters, each a value or the reason it has none (for every TCR
receiver, all eight refused `no_time_series`). Written by `core::results::report`
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
| 0 | the run's results were read (some parameters may still be not evaluable) | the report (JSON with `--json`, a table without) | nothing |
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
are not for publication.`

## Numbers

- Every number is a JSON number: a finite `f64`, printed as the shortest decimal that reads back to
  the same `f64`. No value in a report is NaN or infinite: a run holding one is refused, and the
  report itself is walked for non-finite numbers before it is printed (`report::checked_report`,
  exit 6, `results_value_invalid`), because JSON would print one as `null`, indistinguishable from
  an absent value.
- **`null` appears only at these keys:** `spps`, `tcr` (the other solver's), `mc_sd` (a value not
  from a Monte-Carlo histogram), `band_hz` and `aggregate` (a surface file's: the first for the
  `Global` file, the second for a band's), `curved`, `decay_curve`, `field`, `air_m_per_metre`,
  `onset`, `position_m`, `arrival_s`, `decay_arrival`, `floor_db`, `lost_share`, `crossings`,
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
  "results_version": 5,               // 2: mc_sd, noise, floor, lost-share and per-source fields;
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
  "validated_by_bed": false,          // false until M8's bed passes: show nothing
  "run_folder": "<as given>",
  "solver": "spps" | "tcr",
  "status": "OK",                     // always: any other run is refused
  "started": "2026-09-24T11:50:30.959+02:00",
  "bands_hz": [500, 1000],            // the computed bands, ascending
  "spps": { ... } | null,
  "tcr": { ... } | null
}
```

### `spps`

| Field | What |
|---|---|
| `time_step_s`, `duration_s`, `steps` | `pasdetemps` and `duree_simulation` as SPPS stores them (`f32`), and the rows of every per-step table |
| `speed_of_sound_m_s`, `receiver_radius_m` | SPPS's `c` and `rayon_recepteurp` |
| `receiver_crossing_s` | `2R/c`: the direct sound is spread over this long at a receiver |
| `celerity_gradient` | `alog` or `blin` is not 0: no straight-line arrival is computed |
| `computation_method`, `particles_per_source`, `trans_epsilon`, `echogram_per_source` | `computation_method` (0 random, 1 energetic), `nbparticules`, `trans_epsilon` and `output_recp_bysource` as SPPS reads them |
| `monte_carlo` | how every value's noise was judged: `resamples`, `refused_resamples_allowed`, `seed` (a string, `0x` and 16 hex digits), the largest standard deviation allowed, `limit_decay_relative` (EDT, T20, T30), `limit_clarity_db`, `limit_definition`, `limit_centre_time_s`, `limit_spl_db`; `method` (`"random"` or `"energetic"`, the run's computation method, which picks the calibration); `crossings_variable` (`"crossings_per_particle"` or `"crossings_times_lifetime_spread"`: what each band's `crossings_per_particle`, `n`, is for this method); `measured_on` (what the calibration was measured on beyond what the code checks: room shapes, wall kinds, no transmission, no fittings); and `calibration`, per quantity by its name (`spl_db` … `ts_s`): `factor` and `kappa` (the bootstrap's standard deviation is multiplied by `factor·√(1 + kappa·n)`, calibrated against SPPS's own seed-to-seed spread), `min_particles` and `max_crossings_per_particle` (the domain: outside it the value is refused, `noise_uncalibrated`), `margin` (a refusal names the particle count at which the calibrated standard deviation would be the limit over it), `root_n_confirmed` (false: the spread was seen to fall slower than `1/√N` for the quantity in this method, and a refusal names no count), and `lambert_walls` (energetic T20 and T30: the same fields for bands whose every face reflects by Lambert's law with scattering 1, the reference's `lambert_walls`; `null` for the others) (`docs/params.md`, "Monte-Carlo noise"; `docs/investigations/2026-09-25-noise-calibration/`) |
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
              "volume_m3": 180.0, "area_m2": 216.0,           // the .mbin's volume, the .cbin's area
              "speed_of_sound_m_s": 343.20001220703125,       // SPPS's c
              "constant_s_per_m": 0.16101993084580937,        // K = 24·ln 10/c
              "free_paths": {"mean_free_path_m": 3.3340, "mean_free_path_se_m": 0.0004,
                             "gamma2": 0.38883, "gamma2_se": 0.00014,
                             "four_v_over_s_m": 3.3333, "volume_m3": 180.0, "area_m2": 216.0,
                             "paths": 33554432,
                             "settings": {"replicas": 16, "rays_per_replica": 4096,
                                          "burn_in_paths": 32, "paths_per_ray": 512,
                                          "seed": "0x6c616d6265727431"}} | null,
                                                              // always the fixed STANDARD
              "bands": [{"freq_hz": 500, "air_m_per_metre": 0.000628 | null,
                         "mean_absorption": 0.2,              // ᾱ = Σ Sᵢαᵢ / S
                         "lambert_walls": false,              // every face Lambert, scattering 1?
                         "eyring_s": {"value": 0.5957, "mc_sd": null},
                         "kuttruff_s": {"value": 0.6225, "mc_sd": 0.0000103}}, ...]}
                                                              // mc_sd: gamma^2's share only
| {"status": "not_computed", "why": "..."}
```

- **`kuttruff_s` is M8's reference**: Kuttruff's corrected Eyring,
  `K·V/(4·m·V − S·ln(1 − ᾱ)·[1 + (γ²/2)·ln(1 − ᾱ)])`, with `γ²` from `free_paths`, which a diffuse
  ray transport computed from the room's geometry alone, at fixed settings. Its `mc_sd` is **only**
  the standard deviation it inherits from `γ²`'s standard error, not its total uncertainty: it
  leaves out the formula's own error against a diffuse room, which no ray count reduces (−0.41 %
  to +0.59 % in M8's cells, `docs/params.md`, "Kuttruff's reference"; not measured in other
  rooms). A comparison that divides by `mc_sd` alone would fail a correct solver for the
  formula's error. **`eyring_s` is plain Eyring, reported only.** Both use SPPS's `c` in `K` and
  the solver's own air term `m` (`null` with air absorption off).
- **Both describe a diffuse field.** `lambert_walls` says whether every face reflects by Lambert's
  law with scattering 1 in the band, the only walls the transport's `γ²` describes; with specular
  or partly specular walls neither time describes the run's field.
- `free_paths` is `null` when the transport refused (a ray left the room, its mean free path is
  not `4V/S` within its error, or `γ²`'s standard error is above 0.002); every band's
  `kuttruff_s` then carries that refusal, `params_transport_refused`, and `eyring_s` is still
  given. `free_paths.settings` is always the transport's fixed `STANDARD`: nothing a caller
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
| `arrival_s` | the direct sound's arrival at the centre, which every onset-relative parameter is measured from, the direct sound spread over `±receiver_crossing_s/2` about it; `null` when not computed, and the parameters then detect it. When it lies outside a band's onset bin, C50, C80, D50 and Ts are refused `params_bad_arrival`; SPL, EDT, T20 and T30 are not |
| `bands[]` | per computed band: `freq_hz`; `complete` (random mode, `trans_epsilon` above 0, and SPPS's statistics count at most one particle in a million remaining when the steps ran out, so no tail after the series is bounded; lost particles, and those few remaining, do not make a band incomplete, their unfinished paths are bounded by `lost_share`); `floor_db` (energetic mode's `-10·trans_epsilon`, or `null`); `lost_share` (the share of the energy from the arrival on that unfinished particles can have taken, or `null` when there are none); `lost_follows_decay` (energetic mode: the share bounds the energy from every time on, since what a lost particle would still have brought falls with the decay; `docs/results.md`, "Lost particles"); `early_reverberation_unresolved` (always `true` for SPPS: each value is midway between the reverberation beginning at the arrival, at the first bin wholly after the direct sound and at that bin's end, or refused `early_unresolved`; `docs/params.md`, "The early reverberation"); `arrival` (what C50, C80, D50 and Ts are measured from: `{"arrival": "known", "time_s": …, "half_width_s": …}`, the direct sound at `arrival_s` spread over `±receiver_crossing_s/2`, or `{"arrival": "detected"}`); `decay_arrival` (what EDT, T20 and T30 are measured from, the same shape, or `null` when the series is refused); `contributing_sources` (the sources whose `.recps` total is above 0: with more than one, the seven onset-relative parameters are refused, `several_sources`); `noise_model` (`{"model": "crossings", "mean_deposit": …, "method": "random" | "energetic", "particles": …, "run": {"particles", "least_deposit", "lifetime_cv2", "lambert_walls", "bands"}}`, the mean deposit in Pa², the particles per source, and what the calibration's correction and domain take from the run: the least of the contributing sources' deposits, the spread of the particles' lifetimes from the band's room table, whether every face is Lambert with scattering 1 in the band, and the bands summed; or `{"model": "unknown", "detail": …}`); `crossings` (the receiver crossings the model implies, or `null`); `crossings_per_particle` (`n`, the crossings of the receiver per particle as the calibration measures them, which its correction and domain take, or `null`); `energy_pa2` (the `.recp` series, one per step) and `total_pa2`; `source_power_rho_c` (Pa²·m², the free field at `r` is this over `4πr²`); `background_noise_db`; `onset` (`index`, `bin_start_s`, `bin_end_s`, or `null`); `parameters`; `curvature` and `decay_curve` (below) |
| `aggregate` | `aggregate` (the label), `bands_hz` (the bands summed), `crossings_per_particle` (the aggregate's own `n`: its bands' particles together, at the least deposit of any band and the largest lifetime spread; `null` for TCR), `parameters`, `curvature`, `decay_curve`. **Not ISO 3382-1's single-number value** (a mean of band values): one decay of all bands' energy, weighted by the source spectrum. Never show it as the room's value |
| `by_source[]` | `source` and its `energy` per band, Pa² |
| `per_source[]` | with `echogram_per_source`, one per source in `config.xml`'s order: `source`, `file`, `arrival_s` (from that source alone), `bands[]` (`freq_hz`, `arrival`, `decay_arrival`, `noise_model`, `crossings`, `crossings_per_particle`, `energy_pa2`, `total_pa2`, `onset`, `parameters`, `curvature`, `decay_curve`) and `aggregate`: the parameters of that source–receiver pair. Empty otherwise |

`parameters` holds `spl_db`, `edt_s`, `t20_s`, `t30_s`, `c50_db`, `c80_db`, `d50` and `ts_s`, each
exactly one of:

```
{"value": 64.3876, "mc_sd": 0.041}
{"not_evaluable": {"code": "params_not_evaluable",
                   "message": "params_not_evaluable: T20: monte_carlo_noise: ...",
                   "error": {"kind": "not_evaluable", "quantity": {"quantity": "t20"},
                             "why": {"why": "monte_carlo_noise", "value": 0.83, "sd": 0.099,
                                     "limit": 0.025, "resamples": 200, "refused_resamples": 3,
                                     "particle_count": {"count": "named", "factor": 22.1,
                                                        "margin": 1.4,
                                                        "particles": 3400000}}}}}
```

`mc_sd` is the value's estimated Monte-Carlo standard deviation in its unit, calibrated against
SPPS's own seed-to-seed spread (`monte_carlo.calibration`); every SPPS value carries one, and no
value is reported whose standard deviation exceeds the run's `monte_carlo` limits. A refusal for
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
`monte_carlo_noise`, `noise_unknown`, `noise_uncalibrated`, `several_sources`, `no_time_series` for
`params_not_evaluable` (`docs/params.md`).

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
- `decay_curve` is `null` when the series is refused, when several sources contribute to the band
  (the seven onset-relative values are refused `several_sources`, and so is `curvature`), and for
  TCR, which writes no series (its `curvature` is refused `no_time_series`).

### `tcr`

| Field | What |
|---|---|
| `bands[]` | per computed band: `freq_hz`, and for `sabine` and `eyring` each `absorption_area_m2`, `reverberation_time_s`, `level_db`, as TCR wrote them |
| `global` | `aggregate` (the label), `sabine_level_db`, `eyring_level_db` |
| `point_receivers[]` | `label`, `file`; `bands[]`, per band `freq_hz`, `direct_db`, `total_sabine_db`, `total_eyring_db` (TCR's own levels), `parameters`, `curvature` (refused `no_time_series`) and `decay_curve` (`null`); `global` (the `Global` row, each column's energetic sum over the bands, **an aggregate**, labelled: `aggregate`, `direct_db`, `total_sabine_db`, `total_eyring_db`; a value that is not finite is refused, `results_value_invalid`); `aggregate` (SPPS's shape, labelled `"none: TCR writes no series to sum"`, `bands_hz` empty) |
| `surfaces[]` | as for SPPS, with `field` one of `Direct field`, `Total field (Sabine)`, `Total field (Eyring)` |
| `analytic` | `core::params`' Sabine and Eyring times on the run's own inputs: `{"status": "computed", "volume_m3", "area_m2", "bands": [{"freq_hz", "air_m_per_metre", "sabine_s", "eyring_s"}]}`, the two times as `{"value": …, "mc_sd": null}` or `{"not_evaluable": …}`; or `{"status": "not_computed", "why": …}` |

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
