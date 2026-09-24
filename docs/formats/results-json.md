# `simpa results --json`: the results of a run, for M12

The JSON the Results screen (M12) reads: a verified run's results and, per SPPS point receiver and
band, `core::params`' eight parameters. Written by `core::results::report`
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

Without `--json` the table starts with `UNVALIDATED: M8's physics bed has not passed; these numbers
are not for publication.`

## Numbers

- Every number is a JSON number: a finite `f64`, printed as the shortest decimal that reads back to
  the same `f64`. No value in a report is NaN or infinite: a run holding one is refused.
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
  "results_version": 1,
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
| `sources[]` | `name`, `position_m` (`null` when not read), `emission_s` (`ceil(delay/dt)·dt`) |
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
| `arrival_s` | the direct sound's arrival at the centre, which every onset-relative parameter is measured from; `null` when not computed, and the parameters then detect it |
| `bands[]` | per computed band: `freq_hz`; `complete` (SPPS's statistics show nothing arrives after the series); `energy_pa2` (the `.recp` series, one per step) and `total_pa2`; `source_power_rho_c` (Pa²·m², the free field at `r` is this over `4πr²`); `background_noise_db`; `onset` (`index`, `bin_start_s`, `bin_end_s`, or `null`); `parameters` |
| `aggregate` | `aggregate` (the label), `bands_hz` (the bands summed), `parameters` |
| `by_source[]` | `source` and its `energy` per band, Pa² |

`parameters` holds `spl_db`, `edt_s`, `t20_s`, `t30_s`, `c50_db`, `c80_db`, `d50` and `ts_s`, each
exactly one of:

```
{"value": 64.3876}
{"not_evaluable": {"code": "params_not_evaluable",
                   "message": "params_not_evaluable: T30: range_not_reached: ...",
                   "error": {"kind": "not_evaluable", "quantity": {"quantity": "t30"},
                             "why": {"why": "range_not_reached", "needed_db": -35.0, "reached_db": -19.1}}}}
```

`code` is a row of `docs/solver-contract.md`, "Parameter refusals"; `error` is the typed refusal,
`why.why` one of `range_not_reached`, `truncated`, `unresolved`, `range_too_short`,
`not_decaying`, `empty_window` for `params_not_evaluable` (`docs/params.md`).

### `tcr`

| Field | What |
|---|---|
| `bands[]` | per computed band: `freq_hz`, and for `sabine` and `eyring` each `absorption_area_m2`, `reverberation_time_s`, `level_db`, as TCR wrote them |
| `global` | `aggregate` (the label), `sabine_level_db`, `eyring_level_db` |
| `point_receivers[]` | `label`, `file`; per band `direct_db`, `total_sabine_db`, `total_eyring_db`; `global_direct_db`, `global_total_sabine_db`, `global_total_eyring_db` (`null` when not finite) |
| `surfaces[]` | as for SPPS, with `field` one of `Direct field`, `Total field (Sabine)`, `Total field (Eyring)` |
| `analytic` | `core::params`' Sabine and Eyring times on the run's own inputs: `{"status": "computed", "volume_m3", "area_m2", "bands": [{"freq_hz", "air_m_per_metre", "sabine_s", "eyring_s"}]}`, the two times as `{"value": …}` or `{"not_evaluable": …}`; or `{"status": "not_computed", "why": …}` |

TCR writes no time series, so a TCR report has no per-receiver `parameters`.

## A refusal

```
{
  "results_version": 1,
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
