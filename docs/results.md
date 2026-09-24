# Run results: `core::results`

**M7, piece B.** Typed results of a verified run, and the per-receiver parameters of piece A
(`docs/params.md`) computed from them. Code: `crates/simpa-core/src/results.rs` and `results/`.
CLI: `simpa results <run-folder> [--json]` (`crates/simpa/src/results_cmd.rs`); its JSON is
`docs/formats/results-json.md`. Tests: `crates/simpa-core/tests/results_load.rs`,
`results_rooms.rs`, `params_complete.rs`, `params_floor.rs`, `params_noise.rs` and
`crates/simpa/tests/cli_results.rs`. Gate: `tools/gates/m7.ps1`. Evidence for M8, run on purpose:
`crates/simpa/tests/m8_evidence.rs` ("What M8 needs", below).

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
   (`results_value_invalid`). One exception (M7 follow-up): a NaN in one of the `.gap`'s two
   lateral columns makes that column unusable (`spps::LateralNaN`), not the run. SPPS computes the
   angle there with an unclamped `acos` (`lib_interface/Core/mathlib.h:176-180`), which a particle
   direction along the receiver's orientation takes past ±1 in `f32`; it happened in one of ten
   tutorial 1 runs at 1,500,000 particles in energetic mode, and refused the whole run, T30
   included, although no parameter reads those columns and the energy beside them is checked
   equal to the `.recp`'s. A negative or infinite lateral value still refuses the run
   (`results_load.rs`, `a_nan_in_a_gap_lateral_column_makes_that_column_unusable_not_the_run`).

`simpa results` exits 6 for steps 1, 3, 4, 5 and 6: the plan's stable exit codes give 5 to a
solver run that failed and 6 to result verification (`docs/rebuild-plan-raw-2026-09-23.json`,
the `cli` component; `docs/m5-m6-design.md`, "Exit codes"). A run whose verdict is OK but whose
folder no longer verifies is not a failed run, and a caller that sees 5 knows the solver failed.
A report holding a number that is not finite is refused the same way, `results_value_invalid`
(`report::checked_report`), since JSON would print it as `null`. Every code is produced by a spoiled copy of a
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
| `<source name>/<receiversp_filename>`, with `output_recp_bysource` (`reportmanager.cpp:620-656`; upstream's GUI shows it as a `.recp`, `e_report_file.cpp:304-307`) | each source's own echogram per band | the receiver folder's subfolders are exactly the sources' names, none with the switch off; laid out as the `.recp`; the sources' echograms sum to the `.recp` bin by bin within `f32` rounding, each to its `.recps` total ("Several sources", below) |
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
levels per band; every `.csbin` of the three fields. Each TCR receiver still carries the eight
parameters per band and for its aggregate, every one refused `params_not_evaluable`,
`no_time_series`, so the JSON has one shape of receiver for both solvers (M7 review).

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

**Tutorial 1's box cannot tell how the absorption is combined** (M7 review). Its floor (α 0.1) and
ceiling (0.3) have equal areas, 60 m², and average to the walls' 0.2, so a mean weighted by area, a
mean over the 12 faces and a mean over the three materials are all 0.2: the last two also equal
TCR within 3.3·10⁻⁷ in every band (`gate_d_tcr_equals_…` prints it). Gate M7(d) therefore also
runs `rooms/tutorial1_box_asymmetric.simpa`: the floor's α rising from 0.15 to 0.80 over the bands,
the walls 0.1, the ceiling 0.3 (`results_rooms.rs`, `asymmetric_box`). TCR equals ours within
7.2·10⁻⁷ in every band, and each wrong way misses it by more than 0.5 % in all 27 bands, both
theories (`gate_d_the_asymmetric_room_…`): a mean over faces by at least 13.3 %, a mean over
materials by 6.6 %, the floor's and the walls' materials swapped by 4.9 %, each band's neighbour's
absorption by 0.92 %. A swap of the floor and the ceiling cannot show in a box like this, nor in
Sabine or Eyring at all when the two have equal areas: they see only `A` and `S`.

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
bands summed bin by bin (`params::aggregate`). For a TCR receiver each of the eight is refused,
`no_time_series` (above).

**The arrival.** Every onset-relative parameter is measured from the direct sound's arrival at the
receiver's centre, `params::decay::Arrival::Known`, with the direct sound spread over the time a
particle takes to cross the receiver ball, `±R/c` (`docs/params.md`, "The direct sound's
spread"): the earliest over the sources of emission plus distance over `c`, where
- positions are as SPPS stores them (`f32`, `run::locate::to_float`);
- `c` is SPPS's `343.2·√((T + 273.15)/293.15)` stored as `f32`
  (`base_core_configuration.cpp:64`; `Celerite_du_son.cpp:46`);
- a source's particles start at step `ceil(delay/dt)` cast to `u16` (`sppsNantes.cpp:91`), so its
  emission time is that step times `dt`.

With a celerity gradient (`alog` or `blin` not 0) sound does not travel in straight lines at `c`,
and the arrival is left to `Arrival::Detected`.

**Measured, and piece A's open question answered.** A particle deposits energy while it crosses the
receiver sphere, so the direct sound is spread over `[(r − R)/c, (r + R)/c]`, `2R/c` long (the JSON
gives it as `receiver_crossing_s`). When `(r − R)/c` falls in the bin before the one holding `r/c`,
that bin holds the cap of the sphere the wavefront has crossed; once the cap holds 1 % of the
largest bin, it is the onset bin and `r/c` lies after it. Then C50, C80, D50 and Ts are refused,
`params_bad_arrival`; **SPL, EDT, T20 and T30 are not** (M7 follow-up: the M7 review's version
refused all seven onset-relative parameters, T30 included). The decay times are read from the
arrival with the cap counted in the direct sound, which is exact on the synthetic series of
`params_arrival.rs`.
- **Measured over receiver positions** (`crates/simpa/tests/m8_evidence.rs`,
  `arrival_outside_the_onset_bin_over_receiver_positions`, run on purpose): tutorial 1 at upstream's
  defaults (150,000 particles, random mode, `R` = 0.31 m, seed 1), octave bands 125 Hz to 4 kHz, 200
  receivers uniform over the box at least 0.5 m from the walls and 1 m from the source.
  | `dt` | Receivers with `r/c` after the onset bin | Receiver-bands | Leading edge in the bin before |
  |---|---|---|---|
  | 10 ms (upstream's default) | **16 of 200, 8.0 %** | 96 of 1200 | 20 (10.0 %; `R/(c·dt)` = 9.0 %) |
  | 1 ms | **162 of 200, 81.0 %** | 953 of 1200 | 179 (89.5 %; 90.3 %) |

  None had `r/c` before the onset bin. At a receiver the refusal holds in every band (the direct
  sound is the same in all of them). This replaces the M7 review's derived "about 8 %", which the
  measurement confirms at 10 ms. Where refused, the decay times at 150,000 particles were refused
  for their noise (and T30 for its range at 10 ms), never for the arrival.
- **For M8:** at a step of 1 ms, C50, C80, D50 and Ts are refused at four receivers in five by this
  rule alone. With the spread given, the same curve that gives the decay times gives them exactly
  on the synthetic series; refusing them is Burhan's decision of 2026-09-24 (keep the strict
  rule), not a limit of the model.
- On the level box (`dt` = 0.2 ms, `R` = 0.5 m, `2R/c` = 2.9 ms), the first bin with energy is bin 21
  at 2 m, which is `(r − R)/c` = 4.37 ms; the onset bin (the first within 20 dB of the largest) is
  bin 22; `r/c` = 5.83 ms is bin 29. So `r/c` lies after the onset bin, and C50, C80, D50 and Ts
  are refused, `params_bad_arrival`; SPL is not. The same at 4 m: bins 50, 51 and 58. The decay
  times are read from the arrival over the spread (bins 22 to 36 are the direct sound); a free
  field has nothing after it, so they are refused, `range_too_short` (T30 at 4 m
  `range_not_reached`), never measured from the direct sound's own shape (a run of the level box,
  seed 1, both receivers, 125 Hz and 4 kHz checked).

**Complete series.** When SPPS's own statistics show that nothing arrives after a band's last
bin, the series is given to `params` as complete (`EnergySeries::complete`, `tests/params_complete.rs`):
SPPS in random mode with `trans_epsilon` above 0, and no particle remaining at the end of the
calculation. A particle counts as remaining only when the time steps run out while it is alive
(`spps/CalculationCore.cpp:49, 88-92`), and in random mode a particle is absorbed whole, never
dwindles (`CalculationCore.cpp:62-67, 147-155, 288-300`). `trans_epsilon` 0 drops every particle at
its first surface in random mode too (`CalculationCore.cpp:305`). Energetic mode drops a particle
once its energy falls below `10^-trans_epsilon` of its start (`sppsNantes.cpp:75`), energy no
histogram holds, so it never claims completeness: `params` bounds its tail and its floor. The JSON
reports the claim per band (`complete`); `SppsResults::band_complete`'s unit test says no for
energetic mode, a particle remaining, and `trans_epsilon` 0 or NaN, and the committed energetic run
(below) is refused completeness with every particle accounted for.

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

### Lost particles

SPPS loses a few particles to infinite loops and meshing problems (`partLoop`, `partLost`,
`CalculationCore.cpp:102-107`), and the verdict accepts up to 1 % (`run/verdict.rs`). A lost
particle stops mid-path; its unfinished path is energy the histogram never holds, so "complete" is
true only up to it (M7 review). The first fix, completeness only with none lost, refused almost
everything: tutorial 1 at 150,000 particles loses 1 in most bands, and with the claim gone random
mode's ragged last window refused all eight parameters, SPL included. So the loss is bounded
instead (`SppsResults::lost_share`):
- a lost particle would have brought, on average, what any particle alive at the arrival brings;
- the room table (`<cumul_filename>`) holds the energy of the particles alive at the end of each
  step times `ρc` (`reportmanager.cpp:155-166, 426-437`), and the `.gap` the sources' power times
  `ρc` (`reportmanager.cpp:807-816`), so their ratio at the arrival's step is the share of the
  emitted energy still alive then, `f`;
- with `n` lost of `N` emitted, they take at most `n/(N·f)` of the energy from the arrival on,
  which `params` adds as missing energy and refuses whatever it moves beyond its limit
  (`EnergySeries::with_lost_share`; `docs/params.md`, "Missing energy").

The JSON gives the share per band (`lost_share`).

**Energetic mode** (M7 follow-up). There every particle is kept until the floor, its energy falling
with the room's, so a particle lost at `t` carries about the mean energy of the particles then,
and what it would still have brought is its share of what they all bring after `t`: it falls with
the decay. `n` lost of `N` emitted, each carrying at most `ρ` times the mean energy when it was
lost, take at most `ρ·n/N` of the energy the receiver gets from every time on
(`SppsResults::lost_share_following_decay`; `params::EnergySeries::with_lost_share_following_decay`,
which holds every quantity to the most a scaling of the curve by `1 + ρ·n/N` can move it). The JSON
says so with `lost_follows_decay`. **Measured**, because the random-mode bound refused T30 wholesale
in energetic mode (`crates/simpa/tests/m8_evidence.rs`, run on purpose):
- **what lost particles carried**, from every particle's saved trajectory (tutorial 1, energetic,
  150,000 particles, all saved, 3 seeds, 6 octave bands): SPPS counted 24 lost; the 17 found in
  the trajectories (stopped before the end with more than 10⁻⁴ of their start energy; no particle
  the floor dropped ended above 4.6·10⁻⁵) were lost between 40 and 500 ms and carried **0.16 to
  2.02 times the mean energy** of the particles then (median 0.88). `ρ` is taken as 10
  (`ENERGETIC_LOST_ENERGY_RATIO`). The other 7 ended below 10⁻⁴ of their start energy;
- **what that does to T30**: with each lost particle's own energy and loss time, what they would
  still have brought moved T30 by **at most 4.4·10⁻⁶** (relative) in any receiver-band, where the
  random-mode bound claimed up to **2.1 %** and refused it. What they would have brought from the
  arrival on was 2 % of that bound's (median; 18 % at most);
- at tutorial 1's 150,000 particles the random-mode bound refused T30 in 212 of 360
  receiver-bands on its own, and at 1,500,000 in 308 of 324.

Random mode keeps its bound: there a lost particle carries its whole start energy until it is
absorbed, and its worth does not fall with the decay.

### Energetic mode: the solver's floor

Energetic mode drops each particle at `10^-trans_epsilon` of its start. `params` bounds what that
drop can have cost, `10^{-trans_epsilon}/f` of the energy from the arrival on, with `f` as above
(`EnergySeries::with_solver_floor`; `docs/params.md`, "Missing energy", where the rule is held to
the reviewer's model over α 0.05–0.9, ε 1–7). The committed run `results/energetic_spps`
(`rooms/energetic_box.simpa`: the Seat box in energetic mode, `trans_epsilon` 3, 50,000
particles) has every particle dropped or absorbed by the end, 0 remaining, and still no band
complete; its T30 is refused `missing_not_cleared` in 3 of 4 receiver-bands (the fourth is refused
earlier, its tail not decaying).

**The floor's bound is earned at upstream's default** (M7 follow-up, `m8_evidence.rs`,
`energetic_floor_against_a_lower_floor`, run on purpose): tutorial 1 in energetic mode, 6
receivers, octave bands 125 Hz to 4 kHz, 1,500,000 particles, seeds 1 to 10 at `trans_epsilon` 5
and again at 9. T30 from the series alone is **0.31 % shorter on average at 5 than at 9, and up to
0.74 % in a receiver-band** (5.4 standard errors): more than the 0.5 % limit. The floor's bound
refuses T30 at 5 in all 36 receiver-bands of the seed checked, and in none at 9. So energetic mode
needs a floor
below upstream's default for T30; at 9 a run takes 300 s against 182 s at 5 (20 at a time on
Grace).

### Monte-Carlo noise

Every SPPS value carries its estimated Monte-Carlo standard deviation (`mc_sd`), and a value whose
noise is above half its limen is refused, `monte_carlo_noise` (`params::noise`; `docs/params.md`,
"Monte-Carlo noise"). The mean deposit of one crossing, `W·ρc/(N·πR²)`, comes from the run: `W` is
each source's band power as SPPS computes it from `config.xml` (`10⁻¹²·10^(db/10)` in `f32`,
`base_core_configuration.cpp:140-150`), `ρc` the `.gap`'s sources' power times `ρc` over their
summed power, `N` `nbparticules`, `R` `rayon_recepteurp`. The JSON gives the model per band
(`noise_model`) and the crossings it implies (`crossings`). A directivity balloon (`directivite`
5) scales each particle's energy by its direction (`sppsNantes.cpp:115-127`), so no deposit is
known and every value is refused, `noise_unknown`.

**Against real seeds** (M7 review): over 20 SPPS seeds of tutorial 1 the estimate matches the
spread of every quantity at 150,000 particles (pooled ratio 0.95–1.05), and at 1,500,000 runs low
by 15–21 % for EDT, T20 and Ts; `docs/params.md`, "Monte-Carlo noise", has the table and what it
means at the refusal limit.

**Tutorial 1, Receiver 1, at 150,000 particles** (`cli_results.rs`,
`tutorial1_parameters_beside_upstreams`): about 3,300 crossings per band. SPL, C50, C80, D50 and
Ts come out in most of the 27 bands; EDT carries 2.4–3.4 % and passes in 2 bands; T20 carries
5–22 % and T30 is refused for its noise or its range everywhere. The reviewer's model gave the same
order (EDT 2.0 %, T20 5.5 %, T30 8.4 %). Before this rule, on the 2019 run, T30 came out at 2.40 s
at 1.6 kHz where TCR's Sabine time is 0.66 s.

**Energetic mode at upstream's default** (tutorial 1, `trans_epsilon` 5, 150,000 particles, our
run): SPL, C50, C80 and D50 in almost every band, Ts in some; EDT refused for its noise (the
random-mode bound, loose here); T20 and T30 refused `missing_moves`, mostly for 17 lost particles
at 1 kHz whose bound, `1.8·10⁻⁴` of the energy, is conservative in energetic mode.

**Energetic mode against real seeds** (M7 follow-up; `m8_evidence.rs`,
`energetic_noise_against_ten_seeds`, run on purpose). Tutorial 1 in energetic mode at upstream's
defaults, 6 receivers, octave bands 125 Hz to 4 kHz, seeds 1 to 10; per quantity, the spread of
the values over the seeds against the root-mean-square of the estimates, pooled over the
receiver-bands where every seed gives a value (each series evaluated as the bootstrap takes it,
complete and nothing missing, so that the floor and lost particles do not hide the noise):

| Quantity | 150,000 particles, 10 seeds | 1,500,000 particles, 9 seeds |
|---|---|---|
| SPL | 0.71 | 0.73 |
| EDT | 0.37 | 0.36 |
| T20 | 0.11 | 0.12 |
| T30 | 0.07 | 0.07 |
| C50 | 0.54 | 0.53 |
| C80 | 0.40 | 0.41 |
| D50 | 0.54 | 0.53 |
| Ts | 0.46 | 0.48 |

The estimate is the random-mode model, an upper bound in energetic mode (`docs/params.md`,
"Monte-Carlo noise"), and the seeds say it is one: no quantity's pooled ratio is above 1, and the
largest single receiver-band's is 1.06 at 150,000 and 1.20 at 1,500,000 (SPL), within what 10
seeds leave uncertain (about 23 %). **It over-states the noise of T30 14 times and of T20 9
times**, the same at both counts. Nothing is changed: the bound is sound, and the particle counts
M8 needs under it are measured below ("What M8 needs"). A model that is not an upper bound but
matches energetic mode would need the spread of the particles' energies at each time, which SPPS
writes only when trajectories are saved; that is for Burhan to decide. Seed 7 at 1,500,000
particles was refused: its `.gap` held a NaN (below, "What M8 needs").

### Several sources, and the echogram per source

With more than one source, the `.recp` adds every source's particles into one series
(`reportmanager.cpp:223-224`), measured here from the earliest arrival. ISO 3382-1 defines EDT,
T20, T30, C50, C80, D50 and Ts per source–receiver pair, so on a band where more than one source's
`.recps` total is above 0 those seven are refused, `several_sources`; SPL, the level of all of
them together, is not (M7 review). Upstream's GUI computes them anyway from the same file.

With `output_recp_bysource` on, SPPS also writes each source's own echogram,
`<receiver folder>/<source name>/<receiversp_filename>` (`reportmanager.cpp:620-656`), which
upstream's GUI shows as a `.recp` (`e_report_file.cpp:304-307`). They are read and checked: the
receiver folder's subfolders are exactly the sources' names (none when the switch is off); each
file is laid out as the `.recp`; bin by bin the sources' echograms sum to the `.recp` within the
`f32` rounding of their sum; each sums over time to its `.recps` total. The JSON gives each
source's parameters, measured from that source's own arrival (`per_source`). The committed run
`results/sources2_spps` (`rooms/sources2_box.simpa`: the Seat box with a second source, 3 dB
weaker and 20 ms late, and the switch on) holds it; spoiled copies with a source's echogram
removed, one value changed by 1 %, or a folder no source names are refused.

## Level calibration (gate M7(c))

`rooms/level_box_20m.simpa`: a 20 × 20 × 20 m box, SPPS with `direct_calc = 1` and air absorption
off, an omni source of 100 dB per band (octaves 125 Hz–4 kHz), receivers 2 m and 4 m away.
- **What keeps the reverberant field out is the duration** (corrected after the M7 review). The
  source is 9.97 m from the nearest wall and a particle covers `c·20 ms` = 6.86 m in the whole run,
  so none reaches a surface: the statistics count 0 absorbed by the materials and every particle
  remaining in every band, which `gate_c_...` asserts. The receivers see the direct field and
  nothing else.
- **Two guards the run does not exercise.** `direct_calc` stops a particle at its first surface
  hit (`spps/CalculationCore.cpp:236-242`), and α = 1 does the same without it in both computation
  methods (`CalculationCore.cpp:249-261, 288-300`), with no transmission. The earlier text gave
  these as the reason; with no particle reaching a surface, the gate would pass with both broken.
  They matter only if the duration is made longer.
- **Why 0.2 ms steps and 0.5 m receivers:** see `results_rooms.rs`. The level does not depend on
  either; `params`' tail bound, which it passes, does. No band is complete (every particle
  remains), so SPL goes through the tail bound, not the complete path.

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

**The exact free field** (M7 review). Because its reference sits 0.11–0.30 dB below what SPPS should
give, the gate's ±0.5 dB lets a calibration error between about −0.6 and +0.2 dB through. The same
run is therefore also held to the exact value, `W·ρc·⟨1/d²⟩/(4π·p₀²)`: `ρc` as SPPS computes it from
the run's temperature and pressure (413.25 at 20 °C and 101 325 Pa), and `⟨1/d²⟩` the mean over
the receiver ball in closed form, `3/(2r·R³)·[(R² − r²)/2·ln((r+R)/(r−R)) + r·R]` (+0.0554 dB at
2 m and +0.0137 dB at 4 m above `1/r²`; `cli_results.rs`,
`the_ball_average_of_the_inverse_square_is_its_closed_form`). Every band must lie within 4 of its
`mc_sd`, and the mean of the 12, weighted by `1/mc_sd²`, within 4 of its standard deviation. On
seed 1 that mean is +0.028 dB with a standard deviation of 0.014 dB; it catches an offset above
+0.028 dB or below −0.083 dB, so the SPL moved 0.1 dB either way, or computed with `ρc` = 400
(−0.14 dB), is caught.

**Over ten seeds** (`cli_results.rs`, `level_box_over_ten_seeds`, run on purpose: seeds 1–10,
1,000,000 particles each): each seed's weighted mean difference from the exact free field runs from
−0.024 to +0.028 dB, and their mean is **+0.0028 dB with a standard error of 0.0040 dB**, taken
from the spread over the seeds, not from `mc_sd`. No level bias is resolved at the 0.01 dB scale;
seed 1's +0.028 dB is chance. The SPL spread over the seeds is 1.12 times the mean `mc_sd` pooled
over the 12 receiver-bands (0.66 to 1.57 per receiver-band, each from 10 seeds).
