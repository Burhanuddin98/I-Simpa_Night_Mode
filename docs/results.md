# Run results: `core::results`

**M7, piece B.** Typed results of a verified run, and the per-receiver parameters of piece A
(`docs/params.md`) computed from them. Code: `crates/simpa-core/src/results.rs` and `results/`.
CLI: `simpa results <run-folder> [--json]` (`crates/simpa/src/results_cmd.rs`); its JSON is
`docs/formats/results-json.md`. Tests: `crates/simpa-core/tests/results_load.rs`,
`results_rooms.rs`, `params_complete.rs` and `crates/simpa/tests/cli_results.rs`. Gate:
`tools/gates/m7.ps1`.

**No number read or computed here is shown to a user until M8's physics bed passes**
(`docs/rebuild-plan.md`, M12). The JSON says so: `"validated_by_bed": false`.

## Verified runs only

`results::load(<run folder>)` reads a folder that `simpa run` or `simpa run-folder` wrote, and
returns `RunResults` only when all of this holds, in this order. Otherwise it refuses with a code
of `docs/solver-contract.md`, Part B, "Result refusals", and nothing is read in part.

1. `run.json` exists (`results_manifest_missing`) and reads as this core's manifest, version 1,
   from the pinned solver commit (`results_manifest_invalid`).
2. Its verdict is OK. FAIL or CRASH is `results_run_failed`, CANCELLED `results_run_cancelled`;
   the verdict's reasons are carried. `simpa results` exits 5 for these.
3. The OK verdict agrees with the rest of the manifest: no reasons, stage `solve`, exit class 0,
   a solver outcome of exit 0 without a cancel, and the one argument `config.xml`
   (`results_manifest_invalid`).
4. Every input the manifest hashed before launch is in `solve/` with the same sha256
   (`results_inputs_changed`). The expectation that follows is therefore the run's own.
5. The outputs pass the verdict's own output signals again, judged now
   (`run::verdict::output_reasons`, the same code `judge` runs): the statistics, every expected
   file present and non-empty, and for TCR every displayed value finite and every table readable
   (`results_outputs_invalid`, with the signals' reasons). A file removed, emptied or spoiled after
   the run is caught here.
6. Every file read below decodes, is laid out as the solver writes it and agrees with its sibling
   (`results_file_invalid`), and holds no NaN, infinity or negative energy
   (`results_value_invalid`).

`simpa results` exits 6 for steps 1, 3, 4, 5 and 6. Every code is produced by a spoiled copy of a
committed run in `results_load.rs`; the CLI's exits in `cli_results.rs`, among them a real FAIL
run (`run-folder` on the negative fixture `spps_nomesh`) and a real cancelled one.

## What is read

### SPPS

Point receivers get what upstream's GUI reads for them (`isimpa/data_manager/tree_rapport/
e_report_file.cpp:296-321` maps each extension to its report class):

| File | Read as | Checked |
|---|---|---|
| `<receiversp_filename>` (`.recp`, `e_report_gabe_recp.cpp`) | per band, the energy series `params` reads | a label column, then one float column per computed band labelled `<f> Hz` in band order, every column as long as the room table's |
| `<receiversp_filename_adv>` (`.gap`, `e_report_gabe_gap.cpp`) | per band, `E·cos²φ` and `E·\|cos φ\|`; the sources' power times `ρ·c`; the background noise | its index column is `[1, 2, 3, 4, 5, bands, 3, steps]`; its time step is `pasdetemps`'s `f32`, bit for bit; its bands are the run's; **its energy columns equal the `.recp`'s bit for bit**, since SPPS writes both from `energy_sum × cdt_vol` (`baseReportManager.cpp:183`; `spps/reportmanager.cpp:855`) |
| `Sound level per source.recps` (`e_report_gabe_recps.cpp`) | each source's total per band | one row per source, labelled with its name in `config.xml`'s order |
| `Punctual receiver intensity.gabe` | the intensity vector per band and step | `steps + 1` rows (the last is `Sum`); columns `<f> Hz\n{x,y,z}` |

Besides them: `<cumul_filename>` (the room's energy per band and step), the statistics, every
surface-receiver and cutting-plane `.csbin`, each band's `Intensity.rpi` (decoded to check it
reads, not kept) and, when particles are saved, each band's `.pbin` (summarised).

**What a `.recp` value is.** `energy_sum × c·ρ / V_receiver` (`baseReportManager.cpp:183`;
`sppsInitialisation.cpp:86`), where `energy_sum` adds each particle's energy `W/N`
(`sppsNantes.cpp:73`) times its path length inside the receiver sphere
(`spps/input_output/reportmanager.cpp:223-224`). For the direct sound of a source of power `W`,
`N·πR²/(4πr²)` particles cross a sphere of radius `R` at distance `r`, with a mean chord of `4R/3`,
so the values sum to `ρ·c·W/(4πr²)`: the free field's mean-square pressure, in Pa². Upstream's GUI
turns it into a level with `10·lg(value / p₀²)`, `p₀ = 20 µPa` (`e_report_gabe_recp.cpp:56, 145`;
`projet_calculation.cpp:41, 220-240`), and so do we (`params::decay::spl_db`).

**A finding for upstream's viewer, not yet raised.** In the `.gap`, column +1 of each band holds
`E·cos²φ` and column +2 `E·|cos φ|` (`spps/input_output/reportmanager.cpp:226-227, 388-389,
412-413, 862, 867`); the variables are named the other way round, and upstream's viewer labels
column +1 `E*cos theta` and +2 `E*cos^2 theta` (`e_report_gabe_gap.cpp:80-81`).

### TCR

On tutorial 1 (27 bands, 2 point receivers, the floor's surface receiver) TCR writes 87 files
beside its three inputs (`config.xml`, `mesh.cbin`, `tetramesh.mbin`), all of them read here:
- `Main results.gabe`;
- `Punctual receivers/Receiver 1.gabe` and `Receiver 2.gabe`;
- `<field>/Surface receiver/<f> Hz/Sound level.csbin` for each of the 27 bands and `Global`, under
  each field `Direct field`, `Total field (Sabine)` and `Total field (Eyring)`: 84 files.

TCR writes no time series, so `core::params` has nothing to evaluate from it; its own values are
read as it wrote them: `Main results.gabe`'s per-band area, reverberation time and level of Sabine
and Eyring, and its `Global` levels (the areas and times of `Global` are NaN by design,
`ctr/input_output/reportmanager.cpp:208`); each `Punctual receivers/<lbl>.gabe`'s direct and total
levels per band; every `.csbin` of the three fields.

**The analytic references on the run's own inputs** (`results::tcr::analytic`), what gate M7(d) and
M8 compare TCR with: `params::room`'s Sabine and Eyring times with TCR's constant 0.163, over every
`.cbin` face with its material's absorption in the band (a material's `bfreq` children map to the
sorted bands by position, `base_core_configuration.cpp:202-221`), the `.mbin`'s volume summed over
its tetrahedra (`TC_CalculationCore.cpp:199-209`), and the air term TCR adds as `4·m·V`: with
`abs_atmo_calc` on, `absatmo` itself when `disable_absatmo_computation` is 1, otherwise
`params::air::solver_air_absorption_per_m` at the nominal band frequency. A scene with fitting
faces (`idEn`) is not computed: TCR keeps or drops each such face by its material's transmission
(`TC_CalculationCore.cpp:11-17`), which is not emulated.

Measured on tutorial 1's box: TCR equals the analytic values within 3.3·10⁻⁷ in all 27 bands, both
theories, both from the project and from the run's inputs (`gate_d_…`); and the 2019 run stored in
`tutorial_1.proj` gives the same reverberation times as ours to 4 decimals in every band.

## Receivers are enumerated by folder

SPPS writes each point receiver into a folder named by its label, TCR into a file `<lbl>.gabe`.
The receivers are the folders (files), sorted by name, and each must be exactly one
`recepteur_ponctuel@lbl`: a folder no label names, a label with no folder, or two receivers with one
label is `results_file_invalid`. No label is ever matched by prefix. Gate M7(e) holds this with the
committed runs of `rooms/seats_box.simpa`, whose receivers are `Seat` and `Seat2`: each is read from
its own folder (and file), a copy with the two folders' contents swapped reads the swapped series,
and a copy with a third folder `Seat3` is refused (`cli_results.rs`, `results_load.rs`).

## Per-receiver parameters

For each SPPS point receiver and band, `report::parameters` gives piece A's SPL, EDT, T20, T30, C50,
C80, D50 and Ts, each a value or its refusal; the same for an explicitly labelled aggregate of all
bands summed bin by bin (`params::aggregate`).

**The arrival.** Every onset-relative parameter is measured from the direct sound's arrival at the
receiver's centre, `params::decay::Arrival::Known`: the earliest over the sources of emission plus
distance over `c`, where
- positions are as SPPS stores them (`f32`, `run::locate::to_float`);
- `c` is SPPS's `343.2·√((T + 273.15)/293.15)` stored as `f32`
  (`base_core_configuration.cpp:64`; `Celerite_du_son.cpp:46`);
- a source's particles start at step `ceil(delay/dt)` cast to `u16` (`sppsNantes.cpp:91`), so its
  emission time is that step times `dt`.

With a celerity gradient (`alog` or `blin` not 0) sound does not travel in straight lines at `c`,
and the arrival is left to `Arrival::Detected`.

**Measured, and piece A's open question answered.** A particle deposits energy while it crosses the
receiver sphere, so the direct sound is spread over `[(r − R)/c, (r + R)/c]`, `2R/c` long (the JSON
gives it as `receiver_crossing_s`).
- At upstream's defaults (`dt` = 10 ms, `R` = 0.31 m, `2R/c` = 1.8 ms) both ends usually fall in one
  bin. On tutorial 1 the arrival `r/c` lies in the onset bin at both receivers in all 27 bands, and
  every parameter comes out.
- On the level box (`dt` = 0.2 ms, `R` = 0.5 m, `2R/c` = 2.9 ms), the first bin with energy is bin 21
  at 2 m, which is `(r − R)/c` = 4.37 ms; the onset bin (the first within 20 dB of the largest) is
  bin 22; `r/c` = 5.83 ms is bin 29. So neither `r/c` nor `(r − R)/c` lies in the onset bin, and
  every onset-relative parameter is refused, `params_bad_arrival`; SPL, which does not depend on
  the arrival, is not. The same at 4 m: bins 50, 51 and 58. This is piece A's documented case of a
  direct sound spread over several bins, which its model does not cover; M8 decides what a bed
  uses there.

**Complete series.** When SPPS's own statistics show that nothing arrives after a band's last
bin, the series is given to `params` as complete (`EnergySeries::complete`, the one change this
piece made to piece A, `tests/params_complete.rs`): SPPS in random mode, and no particle remaining
at the end of the calculation. A particle counts as remaining only when the time steps run out while
it is alive (`spps/CalculationCore.cpp:49, 88-92`), and in random mode a particle is absorbed whole,
never dwindles (`CalculationCore.cpp:62-67, 147-155, 288-300`). Energetic mode drops a particle once
its energy falls below `10^-trans_epsilon` of its start (`sppsNantes.cpp:75`;
`CalculationCore.cpp:305`), energy no histogram holds, so it never claims completeness and piece A
bounds its tail. The JSON reports the claim per band (`complete`).

Without it, random mode's scattered last particles make the last window not decaying, and piece A
refused everything in most bands: on the M6 gate's seeded tutorial box (10,000 particles), all
eight parameters, SPL included, in 22 of 27 bands at Receiver 1 and 20 of 27 at Receiver 2, although
the statistics count 0 particles remaining in every band. With it, none is refused for its tail.

A complete series still has no shape inside its last bin with energy, where the curve falls to
nothing and the fit leaves it out. A decay time whose range ends there is refused as
`range_not_reached`, with the level at the start of that bin as the depth reached
(`params_complete.rs::a_complete_series_is_fitted_only_down_to_its_last_bin`). Without that check
the committed Seat run gave T30 equal to T20 to every digit at 500 Hz: its last bin with energy
starts at −19.1 dB, so both were fitted over the same −5 to −19.1 dB. With 2,000 particles neither
reaches its bottom before that bin, and both are refused.

**What nothing here bounds: Monte-Carlo noise.** A complete series is exact for its particles, not
for the room. On tutorial 1's stored 2019 `.recp` (150,000 particles, Receiver 1), our T30 comes
out in the same 10 of 27 bands as upstream's GUI gives a number in (both refuse or give NaN in the
other 17), 0.71–2.40 s against upstream's 0.67–2.42 s, where TCR's Sabine time in those bands is
0.61–0.68 s. The 2.40 s at 1.6 kHz is in both. The seed spread M8 gates (`docs/rebuild-plan.md`,
M8) is what bounds this; a rule on the particles behind each part of the curve is an open decision.

## Level calibration (gate M7(c))

`rooms/level_box_20m.simpa`: a 20 × 20 × 20 m box, SPPS with `direct_calc = 1` and air absorption
off, an omni source of 100 dB per band (octaves 125 Hz–4 kHz), receivers 2 m and 4 m away.
- **Every surface absorbs everything** (α = 1). `direct_calc` alone already stops a particle at its
  first surface hit (`spps/CalculationCore.cpp:236-242`); α = 1 makes the same true without it, in
  both computation methods (`CalculationCore.cpp:249-261, 288-300`), with no transmission. So only
  the direct field reaches a receiver whichever of the two a change broke, and the reverberant
  field cannot mask the check.
- **Why 0.2 ms steps and 0.5 m receivers:** see `results_rooms.rs`. The level does not depend on
  either; `params`' tail bound, which it passes, does.

The gate's reference `Lw − 20·lg r − 11` rounds `10·lg(4π)` = 10.99 and assumes `ρc = 400`.
SPPS's `ρc` at 20 °C and 101 325 Pa is 413.3 (`Masse_volumique_air.cpp:45-52`), 0.14 dB more, and
averaging `1/d²` over a sphere of radius `R` reads `10·lg(1 + R²/(5r²))` higher: 0.054 dB at 2 m,
0.014 dB at 4 m. Measured, seed 1, 1,000,000 particles per band (`cli_results.rs`,
`gate_c_level_calibration_and_the_offsets_it_catches`):

| Receiver | SPL − gate reference | SPL − exact free field |
|---|---|---|
| 2 m | +0.19 to +0.28 dB | −0.01 to +0.08 dB |
| 4 m | +0.11 to +0.30 dB | −0.05 to +0.14 dB |

Says no: Night Mode's `.gap` echogram level, the energy over the intensity reference 10⁻¹²
instead of `p₀²` (`main:project/result_parser.cpp:486`), reads 26.02 dB high and misses the gate in
every band; so does the SPL moved 1 dB either way.
