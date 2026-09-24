# `simpa results --json`: the results of a run, for M12

The JSON the Results screen (M12) reads: a verified run's results and, per point receiver and
band, `core::params`' eight parameters, each a value or the reason it has none (for every TCR
receiver, all eight refused `no_time_series`). Written by `core::results::report`
(`crates/simpa-core/src/results/report.rs`); what is read and refused is `docs/results.md`.

**The JSON Schema is `docs/formats/results-json.schema.json`**, generated from the same Rust types
that serialise the output (`simpa results --schema`; object `report` for a run read, `refusal`
for a run refused). `crates/simpa/tests/cli_results.rs` fails when the committed file differs from
what the binary prints, and when the report prints a top-level key the schema does not have.

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

5 and 6 are the plan's stable codes, "5 solver run, 6 result verification" (raw plan JSON, the
`cli` component; `docs/m5-m6-design.md`, "Exit codes", which gives 6 to M7). A run whose verdict is
FAIL, CRASH or CANCELLED is a failed solver run: 5. A folder whose verdict says OK but whose
manifest, inputs or outputs no longer verify is not a failed run but results that fail
verification: 6. `simpa run` itself exits 130 for a cancel; `simpa results` refuses the cancelled
run's folder with 5, because it is a solver run that did not finish.

Without `--json` the table starts with `UNVALIDATED: M8's physics bed has not passed; these numbers
are not for publication.`

## Numbers

- Every number is a JSON number: a finite `f64`, printed as the shortest decimal that reads back to
  the same `f64`. No value in a report is NaN or infinite: a run holding one is refused, and the
  report itself is walked for non-finite numbers before it is printed (`report::checked_report`,
  exit 6, `results_value_invalid`), because JSON would print one as `null`, indistinguishable from
  an absent value.
- **`null` appears only at these keys:** `spps`, `tcr` (the other solver's), `mc_sd` (a value not
  from a Monte-Carlo histogram), `band_hz`, `field`, `air_m_per_metre`, `onset`, `position_m`,
  `arrival_s`, `floor_db`, `lost_share`, `crossings`, and inside a refusal's typed `error`, `sd`,
  `with_tail` and `with_missing` (`cli_results.rs`, `every_null_in_a_report_is_at_a_nullable_key`).
- Values read from the solvers' files are their `f32`, widened exactly. A reader that is not
  correctly rounded (serde_json's default is not) can come back one `f64` unit off; compare such
  values as `f32`. JavaScript's `JSON.parse` is correctly rounded.
- Units are in the field names: `_s` seconds, `_db` decibels, `_m` metres, `_m2`, `_m3`, `_hz`,
  `_pa2` pascals squared. `d50` is a fraction from 0 to 1, shown as a percentage.
- **Aggregates are labelled.** A value over all bands is never in a band's place: SPPS's
  `aggregate` object says `"aggregate": "all computed bands summed bin by bin"`; TCR's `global`
  object says `"aggregate": "energetic sum of the band levels"`; a surface file with `"band_hz":
  null` is the `Global` file of all bands.

## A report

```
{
  "results_version": 4,               // 2: mc_sd, noise, floor, lost-share and per-source fields;
                                      // 3: TCR receivers carry parameters and an aggregate;
                                      // 4: lost_follows_decay; an arrival outside the onset
                                      //    bin refuses C50, C80, D50 and Ts only
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
| `monte_carlo` | how every value's noise was judged: `resamples`, `refused_resamples_allowed`, `seed`, and the largest standard deviation allowed, `limit_decay_relative` (EDT, T20, T30), `limit_clarity_db`, `limit_definition`, `limit_centre_time_s`, `limit_spl_db` (`docs/params.md`, "Monte-Carlo noise") |
| `sources[]` | `name`, `position_m` (`null` when not read), `emission_s` (`ceil(delay/dt)·dt`), `band_power_w` (per computed band, W, as SPPS computes it), `balloon` (a directivity balloon: its values are refused, `noise_unknown`) |
| `particles` | the statistics per band: absorbed by the atmosphere, the materials, the fittings; lost by loops and meshing; remaining; total |
| `total_energy[]` | per band, the room's energy per step (`<cumul_filename>`) |
| `point_receivers[]` | below, in the order of their folder names |
| `surfaces[]` | each surface-receiver and cutting-plane file, summarised: `path`, `field` (`null`), `band_hz` (`null` for `Global`), `cutting_plane`, `record_type`, `time_steps`, `time_step_s`, and per receiver its `name`, `faces`, `records` and `value_sum`. The values stay in the `.csbin` |
| `particle_files[]` | when particles were saved: `path`, `freq_hz`, `particles`, `recorded_steps` |

A point receiver:

| Field | What |
|---|---|
| `label`, `folder` | the folder's name, which is exactly one `config.xml` label, and its path under `solve/` |
| `position_m` | as SPPS stores it; `null` when not read |
| `arrival_s` | the direct sound's arrival at the centre, which every onset-relative parameter is measured from, the direct sound spread over `±receiver_crossing_s/2` about it; `null` when not computed, and the parameters then detect it. When it lies outside a band's onset bin, C50, C80, D50 and Ts are refused `params_bad_arrival`; SPL, EDT, T20 and T30 are not |
| `bands[]` | per computed band: `freq_hz`; `complete` (random mode, `trans_epsilon` above 0, and SPPS's statistics count no particle remaining when the steps ran out, so no tail after the series is bounded; lost particles do not make a band incomplete, their unfinished paths are bounded by `lost_share`); `floor_db` (energetic mode's `-10·trans_epsilon`, or `null`); `lost_share` (the share of the energy from the arrival on that lost particles can have taken, or `null` when none was lost); `lost_follows_decay` (energetic mode: the share bounds the energy from every time on, since what a lost particle would still have brought falls with the decay; `docs/results.md`, "Lost particles"); `contributing_sources` (the sources whose `.recps` total is above 0: with more than one, the seven onset-relative parameters are refused, `several_sources`); `noise_model` (`{"model": "crossings", "mean_deposit": …}` in Pa², or `{"model": "unknown", "detail": …}`); `crossings` (the receiver crossings the model implies, or `null`); `energy_pa2` (the `.recp` series, one per step) and `total_pa2`; `source_power_rho_c` (Pa²·m², the free field at `r` is this over `4πr²`); `background_noise_db`; `onset` (`index`, `bin_start_s`, `bin_end_s`, or `null`); `parameters` |
| `aggregate` | `aggregate` (the label), `bands_hz` (the bands summed), `parameters`. **Not ISO 3382-1's single-number value** (a mean of band values): one decay of all bands' energy, weighted by the source spectrum. Never show it as the room's value |
| `by_source[]` | `source` and its `energy` per band, Pa² |
| `per_source[]` | with `echogram_per_source`, one per source in `config.xml`'s order: `source`, `file`, `arrival_s` (from that source alone), `bands[]` (`freq_hz`, `noise_model`, `crossings`, `energy_pa2`, `total_pa2`, `onset`, `parameters`) and `aggregate`: the parameters of that source–receiver pair. Empty otherwise |

`parameters` holds `spl_db`, `edt_s`, `t20_s`, `t30_s`, `c50_db`, `c80_db`, `d50` and `ts_s`, each
exactly one of:

```
{"value": 64.3876, "mc_sd": 0.041}
{"not_evaluable": {"code": "params_not_evaluable",
                   "message": "params_not_evaluable: T30: monte_carlo_noise: ...",
                   "error": {"kind": "not_evaluable", "quantity": {"quantity": "t30"},
                             "why": {"why": "monte_carlo_noise", "value": 0.83, "sd": 0.099,
                                     "limit": 0.025, "resamples": 200, "refused_resamples": 107}}}}
```

`mc_sd` is the value's estimated Monte-Carlo standard deviation in its unit; every SPPS value
carries one, and no value is reported whose standard deviation exceeds the run's `monte_carlo`
limits. `code` is a row of `docs/solver-contract.md`, "Parameter refusals"; `error` is the typed
refusal, `why.why` one of `range_not_reached`, `truncated`, `unresolved`, `range_too_short`,
`not_decaying`, `empty_window`, `missing_not_cleared`, `missing_moves`, `monte_carlo_noise`,
`noise_unknown`, `several_sources`, `no_time_series` for `params_not_evaluable` (`docs/params.md`).

### `tcr`

| Field | What |
|---|---|
| `bands[]` | per computed band: `freq_hz`, and for `sabine` and `eyring` each `absorption_area_m2`, `reverberation_time_s`, `level_db`, as TCR wrote them |
| `global` | `aggregate` (the label), `sabine_level_db`, `eyring_level_db` |
| `point_receivers[]` | `label`, `file`; `bands[]`, per band `freq_hz`, `direct_db`, `total_sabine_db`, `total_eyring_db` (TCR's own levels) and `parameters`; `global_direct_db`, `global_total_sabine_db`, `global_total_eyring_db` (the `Global` row, each column's energetic sum over the bands, **an aggregate**; a value that is not finite is refused, `results_value_invalid`); `aggregate` (as SPPS's, `bands_hz` empty) |
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
  "results_version": 4,
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
