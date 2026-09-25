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

`simpa results` exits 6 for steps 1, 3, 4, 5 and 6, and 5 for step 2. The plan's stable exit codes
are "0 OK, 2 validation, 3 geometry, 4 mesh, 5 solver run, 6 result verification, 130 cancelled"
(`docs/rebuild-plan-raw-2026-09-23.json`, `plan.architecture`, the `cli` line; the `cli` component
itself lists no codes); `docs/m5-m6-design.md`, "Exit codes", words 5 as "solver run FAIL or
CRASH" and 130 as "cancelled".
- **A cancelled run is refused with 5, not 130** (corrected after the M7 review, which found the
  earlier text citing "5 solver run" for it without saying why). 130 says that the command itself
  was cancelled: `simpa run` exits 130 when its run is cancelled. `simpa results` was not cancelled;
  it read to the end a folder whose run did not finish, a solver run that did not succeed, which is
  what 5 reports for FAIL and CRASH too. A caller that sees 5 knows the run's solver did not
  succeed, whichever way, and the refusal's code (`results_run_failed`, `results_run_cancelled`)
  says which.
- **A run whose verdict is OK but whose folder no longer verifies** is not a failed run but results
  that fail verification: 6.

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
- **Surface receivers and cutting planes are kept apart by name** (the plan's `core::results`):
  the file named `recepteurss_cut_filename` (`rs_cut.csbin`) is the cutting planes', the other
  (`Sound level.csbin`) the scene receivers', per band and in `Global`. Each must hold only
  receivers of its own kind, told by their `xmlIndex`, the id `config.xml` gives each
  `recepteur_surfacique` and `recepteur_surfacique_coupe`; a file holding the other kind's is
  refused, `results_file_invalid`, not read as it. The JSON gives each receiver's `id`. Tested on
  the committed run `results/outputs_spps` (`rooms/outputs_box.simpa`: the Seat box with the floor's
  receiver `Receiver`, id 0, and a cutting plane `Cut`, id 1, 1.2 m up over 5 × 9 m in 1 m cells):
  all six files read as their kind (`results_load.rs`,
  `cutting_planes_and_surface_receivers_are_kept_apart_by_name`); says no: the two 500 Hz files
  swapped, or the cutting plane's `Global` file copied over the receiver's, is refused. The M7
  critic found no fixture with a cutting plane.
- **Saved particles** (`nbparticules_rendu`): each band's `Particles/<f>/particles.pbin`, decoded
  (`docs/formats/pbin.md`) and held to its run: its time step is `pasdetemps`'s `f32` bit for bit,
  its step count the run's, it holds at most `nbparticules_rendu` per source, each particle has at
  least one step and none past the last (`results_file_invalid`), and every position and energy is
  finite and every energy at least 0 (`results_value_invalid`). The JSON gives each file's particles
  and step records. `results/outputs_spps` saves 10 per source: 8 particles in each band, 28 and
  39 step records (`saved_particles_are_read_through_the_results_and_held_to_their_run`, which
  compares with the format reader's own read); says no: a `.pbin` removed
  (`results_outputs_invalid`), cut short by one record, with another time step, or with a particle
  past the last step (`results_file_invalid`), or with a NaN energy (`results_value_invalid`). The
  M7 critic found no fixture that saved particles.

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

**What gate M7(d) can detect, measured** (M7 follow-ups; `cli_results.rs`,
`gate_d_says_no_through_the_code_to_one_surface_and_to_the_air_term`). The M7 critic: the gate's
say-NO was specified as one surface's α 5 % higher and run as the whole Walls group (96 of
216 m²), and the air term `4mV` had none.
- **One surface.** Each of the box's six planes gets a group and a copy of its material of its own
  with α scaled, is run through TCR, and its analytic times (`results::tcr::analytic` on that
  run's inputs) are held against the unchanged run's TCR times. The smallest α increase that fails
  the gate (either theory outside 0.5 % in some band), found on the project and confirmed through
  TCR 2 % below it (passes in all 27 bands) and 2 % above (fails in 15):

  | Plane | Area | α | +5 % fails the gate in | Smallest increase caught |
  |---|---|---|---|---|
  | floor | 60 m² | 0.1 | 24 of 27 bands | +3.21 % |
  | ceiling | 60 m² | 0.3 | 27 of 27 | +1.07 % |
  | walls x = 0 and x = 6 | 30 m² each | 0.2 | 24 of 27 | +3.21 % |
  | walls y = 0 and y = 10 | 18 m² each | 0.2 | **0 of 27** | +5.35 % |

  Every threshold is the same error in absorption area, `S·Δα` = 0.193 m², 0.45 % of `A` in the
  low bands where the air adds least; above them `4mV` dilutes it. **One 18 m² wall 5 % more
  absorbing passes the gate**: it moves the times by 0.42 %, as the critic computed. The gate
  resolves one surface's error only when it moves `A` by more than 0.19 m² in some band.
- **The air term**, through the code: `results::tcr`'s analytic references on the same run's inputs
  with a test-only fault (`simpa_core::faults`). Dropping `4mV` fails the gate in 20 of 27 bands,
  from 250 Hz up (+0.50 % there, +201 % at 20 kHz). Taking `m` from ISO 9613-1 at the exact
  midband frequency instead of the solver's value at the nominal one fails it in 2 bands only,
  12.5 kHz (−0.59 %) and 16 kHz (+0.96 %); at 8 kHz it moves the times by 0.39 %, inside the gate.
  So gate (d) catches a missing air term, but not the choice of midband frequency below 12.5 kHz on
  this room (`docs/params.md`, "The reference M8 compares against", measures that choice in M8's
  rooms).

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

Beside them (M7 follow-ups, for M12; the M7 critic): the **curvature** of the decay, T20 against
T30 with its flag (`docs/params.md`, "Decay times"), and the **Schroeder curve** the decay times
were fitted to, thinned to within 0.01 dB (`docs/params.md`, "The decay curve, for display"), so
that M12's decay charts and curved-decay warnings come from the code that gives the numbers. With
several sources in a band both are withheld as the seven onset-relative values are
(`several_sources`); for TCR the curvature is refused `no_time_series` and there is no curve.

**Upstream's GUI on the same tutorial 1 run**: `docs/params.md`, "Upstream's GUI reproduced on
tutorial 1", ports its algorithm, reproduces its stored table and measures each step from its
method to ours.

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
and the arrival is left to `Arrival::Detected`. The JSON gives, per band, the `arrival` C50, C80,
D50 and Ts were measured from and the `decay_arrival` EDT, T20 and T30 were (`results_load.rs`
checks that every SPPS band carries `r/c` with the spread `R/c`).

**The early reverberation** (second review). Every SPPS series is given to `params` with its early
reverberation unresolved (`EnergySeries::with_early_reverberation_unresolved`; the JSON's
`early_reverberation_unresolved`): SPPS's reverberation begins with the first reflection and builds
up, so each value is read with it beginning at the arrival, at the first bin wholly after the direct
sound and at that bin's end, reported midway, and refused `early_unresolved` when those differ by
more than its limit (`docs/params.md`, "The early reverberation"). At the default step of 10 ms that
refuses EDT in M8's cells from α 0.1 (5×4×3 m) or 0.2 (6×10×3 m) up, where it had read up to 4.9 %
short ("What M8 needs", "EDT and the time step").

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
  for their noise (and T30 for its range at 10 ms), never for the arrival (counted before the early
  reverberation was bounded, below; EDT at 10 ms can now also be refused `early_unresolved`).
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
bin, but what unfinished paths would have brought, the series is given to `params` as complete
(`EnergySeries::complete`, `tests/params_complete.rs`): SPPS in random mode with `trans_epsilon`
above 0, and at most one particle in a million remaining at the end of the calculation
(`spps::REMAINING_UNFINISHED_SHARE`). A particle counts as remaining only when the time steps run
out while it is alive (`spps/CalculationCore.cpp:49, 88-92`), and in random mode a particle is
absorbed whole, never dwindles (`CalculationCore.cpp:62-67, 147-155, 288-300`). `trans_epsilon` 0
drops every particle at its first surface in random mode too (`CalculationCore.cpp:305`).
Energetic mode drops a particle once its energy falls below `10^-trans_epsilon` of its start
(`sppsNantes.cpp:75`), energy no histogram holds, so it never claims completeness: `params` bounds
its tail and its floor. The JSON reports the claim per band (`complete`); `SppsResults::
band_complete`'s unit tests say no for energetic mode, more than one particle in a million
remaining, and `trans_epsilon` 0 or NaN, and the committed energetic run (below) is refused
completeness with every particle accounted for.

**The few left alive** (M7 follow-ups, second review). The first rule was "none remaining". At
M8's counts SPPS leaves about one particle in 10⁸ to 10⁹ alive at the end in the 6×10×3 m room,
trapped, reaching no receiver ("What M8 needs", "Particles left alive at the end"), and each one
refused its band's every onset-relative quantity through the tail of a ragged random-mode end. A
remaining particle's path is unfinished as a lost one's is, so up to one in a million are now
bounded with the lost ones, `(lost + remaining)/(N·f)` of the energy from the arrival
(`SppsResults::lost_share`, below); more than that is a run cut short, left incomplete, its tail
bounded from the series (`a_particle_in_a_million_left_alive_is_bounded_as_unfinished_not_refused`
says no at 11 in 10 million).

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
  (`EnergySeries::with_lost_share`; `docs/params.md`, "Missing energy"). In a complete band the
  few particles left alive at the end are counted with them ("Complete series").

The JSON gives the share per band (`lost_share`). The report path is tested on the committed runs
(`results_load.rs`): the random-mode Seat run with 20 lost planted at 500 Hz gives `n/(N·f)`, not
following the decay.

**Energetic mode** (M7 follow-up). There every particle is kept until the floor, its energy falling
with the room's, so a particle lost at `t` carries about the mean energy of the particles then,
and what it would still have brought is its share of what they all bring after `t`: it falls with
the decay. `n` lost of `N` emitted, each carrying at most `ρ` times the mean energy when it was
lost, take at most `ρ·n/N` of the energy the receiver gets from every time on
(`SppsResults::lost_share_following_decay`; `params::EnergySeries::with_lost_share_following_decay`,
which holds every quantity to the most a scaling of the curve by `1 + ρ·n/N` can move it, added to
what the floor moves it by: `docs/params.md`, "Missing energy"). The JSON says so with
`lost_follows_decay`; the report path is tested on the committed energetic run (`results_load.rs`:
its 2 lost of 50,000 at 500 Hz give `10·2/50,000`, and 200 planted give 0.04, which refuses SPL by
the 0.17 dB it can move every level, where random mode's lump would have passed it).
**`ρ` is an empirical cap, not a bound.** Measured, because the random-mode bound refused T30
wholesale in energetic mode (`crates/simpa/tests/m8_evidence.rs`, run on purpose; logs kept in the
scratch folder):
- **what lost particles carried**, from every particle's saved trajectory (tutorial 1, energetic,
  150,000 particles, all saved, 3 seeds, 6 octave bands): SPPS counted 24 lost; the 17 found in
  the trajectories (stopped before the end with more than 10⁻⁴ of their start energy; no particle
  the floor dropped ended above 4.6·10⁻⁵) were lost between 40 and 500 ms and carried **0.16 to
  2.02 times the mean energy** of the particles then (median 0.88). The other 7 ended below 10⁻⁴ of
  their start energy, where they cannot be told from particles the floor dropped: their ratio is
  not known;
- **in an M8 cell** (second review: the 5×4×3 m room at α 0.4, energetic, `trans_epsilon` 9,
  300,000 particles all saved, 3 seeds): of 76 lost, the 44 that stopped with more than 10⁻⁵ of
  their start energy carried **0.04 to 4.4 times** the mean (median 1.0); the rest are censored
  the same way. `ρ` is taken as 10 (`ENERGETIC_LOST_ENERGY_RATIO`), twice the largest measured;
- **what that does to T30**: with each found particle's own energy and loss time, and what it would
  still have brought **modelled** as following the decay (a trajectory stops where the particle
  was lost, so its future is not measured), T30 moved by at most 4.4·10⁻⁶ (tutorial 1) and
  3.0·10⁻⁶ (the M8 cell) in any receiver-band, where the random-mode bound claimed up to 2.1 % and
  1.5 % and refused it. On tutorial 1 what they would have brought from the arrival on was 2 % of
  that bound's (median; 18 % at most);
- at tutorial 1's 150,000 particles the random-mode bound refused T30 in 212 of 360
  receiver-bands on its own, and at 1,500,000 in 308 of 324;
- **where `ρ` matters**: in energetic cells of 1.5 M particles or more, `ρ·n/N` is about 10⁻⁴ to
  10⁻³ and moves T30 by at most 0.01 to 0.1 %. The lost count `n` grows with `N`, so the share does
  not fall as the particles grow, and no count clears a quantity it refuses. Until 2026-09-25 it
  refused C80 at the 0.01 dB limit then held in the 6×10×3 m room at α 0.05, where 125 lost a band
  in a 4 s run give 8.3·10⁻⁴ (0.011 dB); at the aligned 0.1 dB limit C80 and C50 come through
  there in every receiver-band and seed ("What M8 needs", "The C and D limits aligned").

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
and again at 9, every run read (second review: the first count left out runs refused for a
lateral NaN, and quoted two separate maxima as one receiver-band's). T30 from the series alone is
**0.338 ± 0.025 % shorter on average at 5 than at 9**; the largest difference in one receiver-band
is 0.56 %, 4.2 of its own standard errors; 6 of the 36 differ by more than the 0.5 % limit; the
largest in standard errors, in some receiver-band, is 5.7. The floor's bound refuses T30 at 5 in all
36 receiver-bands of the seed checked, and in none at 9. So energetic mode needs a floor below
upstream's default for T30; at 9 a run takes 300 s against 182 s at 5 (20 at a time on Grace).

### Monte-Carlo noise

Every SPPS value carries its estimated Monte-Carlo standard deviation (`mc_sd`), calibrated against
SPPS's own seed-to-seed spread for the run's computation method, and a value whose noise is above
half its limen is refused, `monte_carlo_noise`, naming the particles per source that would bring
it within its limit; a value from a run outside what the calibration measured (too few particles,
or too many crossings of a receiver per particle) is refused, `noise_uncalibrated`, naming the
particles or the receiver radius that would bring it inside (`params::noise`; `docs/params.md`,
"Monte-Carlo noise"; `docs/investigations/2026-09-25-noise-calibration/`). The mean deposit of one
crossing,
`W·ρc/(N·πR²)`, comes from the run: `W` is each source's band power as SPPS computes it from
`config.xml` (`10⁻¹²·10^(db/10)` in `f32`, `base_core_configuration.cpp:140-150`), `ρc` the
`.gap`'s sources' power times `ρc` over their summed power, `N` `nbparticules`, `R`
`rayon_recepteurp`. The JSON gives the model per band (`noise_model`, with the computation method,
the particle count and what the calibration takes from the run: the least deposit, the spread of
the particles' lifetimes from the room table, the kind of walls), the crossings it implies
(`crossings`) and per particle (`crossings_per_particle`), and the calibration per quantity with
its domain (`monte_carlo.calibration`). A directivity balloon (`directivite` 5) scales each particle's energy
by its direction (`sppsNantes.cpp:115-127`), so no deposit is known and every value is refused,
`noise_unknown`.

**The calibration** (pre-M8; Burhan's decision 3 of 2026-09-24 17:45; four pre-registered
rounds, round 4 shipped): 94 cells of ten seeds each, box rooms of 60 to 1,000 m³ with Lambert,
specular, partly scattering and one-surface absorption, dead walls and the direct field alone, in
random and energetic mode, 5,000 to 15,000,000 particles, receivers of 0.31 to 1.4 m, steps of 1
and 10 ms; round 3's numbers from 51 calibration cells checked on 19 held-out cells, round 4's from
all 70 checked on 24 new ones. Each value's standard deviation is the bootstrap's times
`k·√(1 + κ·n)`, `n` the run's own crossings of the receiver per particle times its particles'
lifetime spread. Random mode: `k` 1.2 to 1.6 and `κ` 1 to 3 (a particle crosses a receiver again
and again; with 0.9 m receivers in a live 60 m³ room the spread is up to 2.6 times the
bootstrap's). Energetic mode: `k` 0.84 to 1.1 for SPL, EDT, C50, C80, D50 and Ts; T20 and T30
0.098 and 0.052 in uniform Lambert rooms up to a mean absorption of 0.2 (M7's model overstated
them 11 to 38 times; above 0.2 they failed two long rooms and no longer apply), and elsewhere 1.4
and 1.3 of a structure that reads each bin's deposit from the series' own roughness (round 4;
M7's structure, which round 3 kept there, overstated tutorial 1's T30 14 times, this one 2.4
times).

**What it changes at upstream's default** (tutorial 1 as shipped: 27 third-octave bands, both
receivers, 150,000 particles, `dt` 10 ms, seeds 1 to 10; 540 receiver-bands; M7's model against
this one, the same runs read twice):

| Method | T30 | T20 | EDT | C80 | D50 |
|---|---|---|---|---|---|
| random | 0 → 0 → 0 → 0 | 0 → 0 → 0 → 0 | 0 → 0 → 0 → 0 | 479 → 472 → 458 → 458 | 540 throughout |
| energetic | 0 → 0 → 0 → 66 | 0 → 0 → 0 → 206 | 0 → 0 → 0 → 0 | 480 → 490 → 484 → 484 | 540 throughout |

(M7's model → rounds 1 and 2 → round 3 → round 4.) Up to round 3 the noise model was one of the
things holding energetic T20 and T30 back there: tutorial 1's walls are specular, and its T20 and
T30 kept M7's structure at factor 1, 9 and 14 times what the seeds show, which refused every T20 and
the 68 T30 values the floor let through. Round 4's roughness structure gives 206 T20 and 66 T30
values (the remaining 334 T20 name 5,200,000 to 22,000,000 particles, median 8,500,000: counts are
named from M7's structure, an upper bound, while the same room at 1,200,000 gave 245 of 360 T20
values). What holds the
rest back is not the noise model: EDT is refused `early_unresolved` in every receiver-band at the
10 ms step, energetic T30 `missing_moves` for the floor at `trans_epsilon` 5 (472 of 540), and
random T30 at 150,000 particles is as noisy as refused (285 for noise, 253 for its range; 8.7 %
relative spread in the Lambert box of the same size). Round 3's random C80 factor, 1.4 with the
dead-walls room among its cells, refuses 21 more C80 values than M7's model did.

**Earlier measurements** (M7 review and follow-ups), which the calibration re-measured over more
rooms and now covers: over 20 seeds of tutorial 1, M7's model matched the spread at 150,000
particles (pooled 0.95 to 1.05) and ran 15 to 21 % low at 1,500,000 for EDT, T20 and Ts
(`cli_results.rs`, `noise_estimate_against_the_spread_of_twenty_seeds`); in energetic mode at
upstream's defaults it over-stated T30's noise 14 times and T20's 9 times (`m8_evidence.rs`,
`energetic_noise_against_ten_seeds`), and 30 to 40 times T30's in M8's cells. On tutorial 1,
Receiver 1, at 150,000 particles (`cli_results.rs`, `tutorial1_parameters_beside_upstreams`): about
3,300 crossings per band; SPL, C50, C80, D50 and Ts come out in most of the 27 bands, T20 carries
5–22 % and T30 is refused for its noise or its range everywhere. Before the M7 rule, on the 2019
run, T30 came out at 2.40 s at 1.6 kHz where TCR's Sabine time is 0.66 s.

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
  remaining in every band, which `gate_c_...` asserts, and nothing reaches a receiver after the
  direct sound has passed it. The receivers see the direct field and nothing else. **Says no**
  (M7 follow-ups; the M7 critic: the check had no partner): the same box with its walls
  reflecting (α 0.5, `direct_calc` off) over 120 ms, 100,000 particles, through SPPS
  (`gate_c_says_no_to_a_run_whose_walls_are_reached`): the statistics count 86,945 to 87,155 of
  100,000 absorbed by the materials and 12,845 to 13,055 remaining, so the check fails in every
  band; what it keeps out, energy after the direct sound, is 4.0 to 6.3 % of the total at 2 m and
  14.4 to 18.2 % at 4 m.
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

Says no, **through the code** (M7 follow-ups; the M7 critic: the first partners added 26 dB and
1 dB to the SPL the correct run had produced). The test computes the run's report again in its
own process, by the code the CLI runs (`results::load`, `report::checked_report`; without a fault
it is the CLI's report, which the test asserts), with the level code path's calibration constant
replaced through a test-only seam (`simpa_core::faults::Fault::LevelReference`, compiled only into
test builds): Night Mode's `.gap` reference, the intensity reference 10⁻¹² instead of `p₀²`
(`main:project/result_parser.cpp:486`), reads +26.02 dB and misses the gate in all 12
receiver-bands; `p₀²` 1 dB off either way reads ±1.00 dB and misses it in all 12.

**The exact free field** (M7 review). Because its reference sits 0.11–0.30 dB below what SPPS should
give, the gate's ±0.5 dB lets a calibration error between about −0.6 and +0.2 dB through. The same
run is therefore also held to the exact value, `W·ρc·⟨1/d²⟩/(4π·p₀²)`: `ρc` as SPPS computes it from
the run's temperature and pressure (413.25 at 20 °C and 101 325 Pa), and `⟨1/d²⟩` the mean over
the receiver ball in closed form, `3/(2r·R³)·[(R² − r²)/2·ln((r+R)/(r−R)) + r·R]` (+0.0554 dB at
2 m and +0.0137 dB at 4 m above `1/r²`; `cli_results.rs`,
`the_ball_average_of_the_inverse_square_is_its_closed_form`). Every band must lie within 4 of its
`mc_sd`, and the mean of the 12, weighted by `1/mc_sd²`, within 4 of its standard deviation. On
seed 1 that mean is +0.028 dB with a standard deviation of 0.017 dB; it catches an offset above
+0.039 dB or below −0.094 dB. Through the same seam, `p₀²` 0.15 dB off either way (weighted means
+0.178 and −0.122 dB, against a bound of ±0.066 dB), and the reference as it would read with
`ρc` = 400 (−0.114 dB), are caught. (Before the pre-M8 noise calibration the standard deviation
was 0.014 dB and the partners were `p₀²` 0.1 dB off either way. SPL's `mc_sd` is 1.2 times M7's
since round 3 (1.3 in rounds 1 and 2), M7's having been below the seeds' spread, so the window is
wider, and seed 1 sits 0.028 dB high: `p₀²` 0.1 dB high reads −0.072 dB, caught by 0.0004 dB in
rounds 1 and 2 and by 0.006 dB in round 3. The partners are 0.15 dB since, caught by 0.056 and
0.111 dB. Four million particles, which would halve the window instead, lost one particle to a
meshing problem at 2 kHz and so failed the keep-out check above; the gate was not changed for it.)

**Over ten seeds** (`cli_results.rs`, `level_box_over_ten_seeds`, run on purpose: seeds 1–10,
1,000,000 particles each): each seed's weighted mean difference from the exact free field runs from
−0.024 to +0.028 dB, and their mean is **+0.0028 dB with a standard error of 0.0040 dB**, taken
from the spread over the seeds, not from `mc_sd`. No level bias is resolved at the 0.01 dB scale;
seed 1's +0.028 dB is chance. The SPL spread over the seeds is 1.12 times the mean `mc_sd` pooled
over the 12 receiver-bands (0.66 to 1.57 per receiver-band, each from 10 seeds), as M7's model gave
it; round 3's, 1.2 times that (the direct field alone is among its cells), gives 0.93.
**M8's direct-field checks** (its "SPL: direct-field calibration") take their tolerance from such a
spread over seeds, as this does, not from `mc_sd`: `k` is the largest over every room, so a window
from it is wider than the solver's own spread there.

## What M8 needs (M7 follow-ups, 2026-09-24)

Measured with `crates/simpa/tests/m8_evidence.rs` (`m8_cells`, `m8_tcr_cells`) and
`crates/simpa-core/tests/lambert_box.rs`, run on purpose on Grace; every SPPS run read again with
this commit's `simpa results` (`$SIMPA_M8_FROM`); the transcripts are committed in
`docs/investigations/2026-09-24-m8-evidence/`, the run folders stay in scratch. Each cell: the room with every surface at α,
Lambert reflection with scattering 1, 3 receivers each at least 1 m from the walls and the source,
`dt` 10 ms unless stated; air absorption off and octave bands 125 Hz to 4 kHz (18 receiver-bands),
or, for M8's second table, air on at 20 °C and 50 % and octave bands to 8 kHz (21). Seeds 1 to 3,
and 1 to 20 in two cells. The bands differ only in their random numbers, so they are replicas; the
three receivers of a band share its particles. **Information for M8's design, not a gate.** The
refusal limits are Burhan's strict ones (2026-09-24), as the code held them then: C and D at gate
(a)'s 0.01 dB and 0.1 points, ten and five times stricter than his rule; they were aligned to it
on 2026-09-25 ("The C and D limits aligned", below), which changes one cell's C80. The wall times
are per run with 19 to 27 runs at once on Grace's 28 threads.

**What these tables do not show** (the M7 follow-ups' critic): every row is at `dt` 10 ms, where
EDT is refused `early_unresolved` at every count in the 5×4×3 m room from α 0.1 and in the
6×10×3 m room from α 0.2. No configuration measured gives T30, EDT, C80 and D50 together there.
At 1 ms, M8's step (Burhan, 23:14), only the EDT probe was run ("EDT and the time step": two
energetic cells at 1.5 M), and which of M8's own receivers lose C50, C80, D50 and Ts to
`params_bad_arrival` at 1 ms was not counted (the 81 % above is tutorial 1's 200 random
positions). The second table, air on, covers 2 of the 8 room-α cells per method, the two with the
smallest excess over Eyring; nothing was run with air at α 0.2 or 0.4, and the Kuttruff reference
with `4mV` is checked against the independent transport alone (`docs/params.md`, "Kuttruff's
reference", the air term), not against SPPS. The M8 bed measures all three.

"Through" counts the receiver-bands where every seed gives the quantity. From each series alone,
refused or not: "σ" is the relative standard deviation of T30 over the seeds, pooled over the
receiver-bands, with its own uncertainty; "range" the largest (max − min)/mean over seeds 1 to 3 of
any receiver-band; "range of the mean" the same for the mean over the cell's receiver-bands (the
same receiver-bands in every seed). "vs Eyring" is the mean T30 against
`24·ln10/343.2 · V/(−S·ln(1 − α))`. **The counts are counts that passed, not minima.** They were
counted with M7's noise model. Since the pre-M8 calibration ("Monte-Carlo noise" above; rounds 3
and 4),
random mode's T30 standard deviation is 1.6·√(1 + 3n) times M7's (`n` about 0.01 to 0.26 in
these rooms at 0.31 m receivers, the most in the 5 × 4 × 3 m room at α 0.05: 1.6 to 2.1 times), so
a random-mode count that passed near its limit may now need up to 4.4 times the particles; energetic T20 and T30 in these uniform Lambert rooms are
0.098 and 0.052 of M7's, so the energetic counts below are far higher than the noise now needs,
and the bed should count again ("Constraints on M8 from the noise calibration", below).

### Random mode

| Room | α | Particles | Duration | Through: T30 / EDT / C80 / D50 | σ | Range | Range of the mean | vs Eyring | Wall |
|---|---|---|---|---|---|---|---|---|---|
| 6×10×3 | 0.05 | 1.5 M | 4 s | 18 / 18 / 18 / 18 | 1.89 ± 0.22 % | 7.97 % | 0.57 % | +1.21 % | 97 s |
| 6×10×3 | 0.05 | 12 M | 4 s | 18 / 18 / 18 / 18 | 0.62 ± 0.07 % | **1.94 %** | 0.21 % | +1.01 % | 648 s |
| 6×10×3 | 0.1 | 1.5 M | 3 s | **5** / 18 / 18 / 18 | 2.16 ± 0.25 % | 9.47 % | 0.88 % | +2.79 % | 48 s |
| 6×10×3 | 0.1 | 6 M | 3 s | 18 / 18 / 18 / 18 | 1.35 ± 0.16 % | 5.90 % | 0.20 % | +2.46 % | 198 s |
| 6×10×3 | 0.1 | 40 M | 3 s | 18 / 18 / 18 / 18 | 0.40 ± 0.05 % | **1.73 %** | 0.21 % | +2.26 % | 1165 s |
| 6×10×3 | 0.2 | 1.5 M | 2 s | **0** / 0 / 18 / 18 | 3.74 ± 0.44 % | 13.2 % | 0.55 % | +5.05 % | 23 s |
| 6×10×3 | 0.2 | 8 M, 20 seeds | 2 s | 18 / 0 / 18 / 18 | 1.60 ± 0.06 % | 4.49 % | 0.11 % | +5.02 % | 119 s |
| 6×10×3 | 0.2 | 80 M | 2 s | 18 / 0 / 18 / 18 | 0.45 ± 0.05 % | **1.53 %** | 0.34 % | +4.95 % | 1151 s |
| 6×10×3 | 0.4 | 1.5 M | 1 s | **0** / 0 / 18 / 18 | 4.96 ± 0.58 % | 15.9 % | 1.77 % | +10.63 % | 11 s |
| 6×10×3 | 0.4 | 16 M | 1.5 s | **17** / 0 / 18 / 18 | 2.09 ± 0.25 % | 7.43 % | 1.02 % | +10.84 % | 98 s |
| 6×10×3 | 0.4 | 48 M | 1 s | 18 / 0 / 18 / 18 | 1.14 ± 0.13 % | 4.49 % | 0.72 % | +10.72 % | 336 s |
| 6×10×3 | 0.4 | 260 M | 1.5 s | 18 / 0 / 18 / 18 | 0.42 ± 0.05 % | **1.47 %** | 0.03 % | +10.73 % | 1748 s |
| 5×4×3 | 0.05 | 1.5 M | 3 s | 18 / 18 / 18 / 18 | 1.16 ± 0.14 % | 4.45 % | 0.58 % | +1.27 % | 98 s |
| 5×4×3 | 0.05 | 10 M | 3 s | 18 / 18 / 18 / 18 | 0.47 ± 0.06 % | **1.48 %** | 0.17 % | +1.04 % | 583 s |
| 5×4×3 | 0.1 | 1.5 M | 2 s | 18 / 0 / 18 / 18 | 1.62 ± 0.19 % | 5.71 % | 0.58 % | +2.15 % | 48 s |
| 5×4×3 | 0.1 | 18 M | 2 s | 18 / 0 / 18 / 18 | 0.53 ± 0.06 % | **1.91 %** | 0.24 % | +2.18 % | 528 s |
| 5×4×3 | 0.2 | 1.5 M | 1.5 s | **9** / 0 / 18 / 18 | 2.25 ± 0.27 % | 8.61 % | 0.15 % | +4.50 % | 23 s |
| 5×4×3 | 0.2 | 8 M | 1.5 s | 18 / 0 / 18 / 18 | 1.05 ± 0.12 % | 3.43 % | 0.52 % | +4.68 % | 143 s |
| 5×4×3 | 0.2 | 40 M | 1.5 s | 18 / 0 / 18 / 18 | 0.42 ± 0.05 % | **1.56 %** | 0.26 % | +4.45 % | 585 s |
| 5×4×3 | 0.4 | 1.5 M | 1 s | **0** / 0 / 18 / 18 | 2.62 ± 0.31 % | 8.50 % | 0.59 % | +9.37 % | 11 s |
| 5×4×3 | 0.4 | 8 M, 20 seeds | 1 s | 18 / 0 / 18 / 18 | 1.56 ± 0.06 % | 4.83 % | 0.56 % | +9.32 % | 59 s |
| 5×4×3 | 0.4 | 60 M | 1 s | 18 / 0 / 18 / 18 | 0.52 ± 0.06 % | **1.55 %** | 0.32 % | +9.28 % | 431 s |

T30 refused at the lower counts is `monte_carlo_noise`; EDT refused is `early_unresolved` ("EDT
and the time step", below).

### Energetic mode

| Room | α | Particles | Duration | `trans_epsilon` | Through: T30 / EDT / C80 / D50 | σ | Range | Range of the mean | vs Eyring | Wall |
|---|---|---|---|---|---|---|---|---|---|---|
| 6×10×3 | 0.05 | 1.5 M | 4 s | 7 | 18 / 18 / 18 (**5** until the limits were aligned) / 18 | 0.04 % | 0.17 % | 0.02 % | +1.19 % | 1330 s |
| 6×10×3 | 0.1 | 6 M | 2 s | 7 | 18 / 18 / 18 / 18 | 0.03 % | 0.10 % | 0.01 % | +2.42 % | 2605 s |
| 6×10×3 | 0.2 | 1.5 M | 1 s | 9 | **0** / 0 / 18 / 18 | 0.09 % | 0.27 % | 0.02 % | +4.95 % | 356 s |
| 6×10×3 | 0.2 | 4.5 M | 1 s | 9 | 18 / 0 / 18 / 18 | 0.07 % | 0.26 % | 0.02 % | +4.96 % | 906 s |
| 6×10×3 | 0.4 | 1.5 M | 0.5 s | 9 | **0** / 0 / 18 / 18 | 0.18 % | 0.65 % | 0.08 % | +10.73 % | 180 s |
| 6×10×3 | 0.4 | 13 M | 0.5 s | 9 | 18 / 0 / 18 / 18 | 0.06 % | 0.19 % | 0.04 % | +10.74 % | 1102 s |
| 5×4×3 | 0.05 | 1.5 M | 3 s | 7 | 18 / 18 / 18 / 18 | 0.03 % | 0.12 % | 0.01 % | +1.10 % | 1345 s |
| 5×4×3 | 0.1 | 1.5 M | 2 s | 7 | 18 / 0 / 18 / 18 | 0.04 % | 0.13 % | 0.02 % | +2.22 % | 663 s |
| 5×4×3 | 0.2 | 1.5 M | 0.8 s | 9 | 18 / 0 / 18 / 18 | 0.07 % | 0.24 % | 0.07 % | +4.45 % | 354 s |
| 5×4×3 | 0.2 | 1.5 M | 0.8 s | 7 | 18 / 0 / 18 / 18 | 0.06 % | 0.16 % | 0.02 % | +4.43 % | 330 s |
| 5×4×3 | 0.4 | 1.5 M | 0.4 s | 9 | **0** / 0 / 18 / 18 | 0.12 % | 0.49 % | 0.04 % | +9.24 % | 180 s |
| 5×4×3 | 0.4 | 3.5 M | 0.4 s | 9 | 18 / 0 / 18 / 18 | 0.08 % | 0.29 % | 0.01 % | +9.24 % | 405 s |

The α 0.05 and 0.1 rows are new (second review: they had not been run). T30 refused is
`monte_carlo_noise`: the estimate was M7's structure, 30 to 40 times what the seeds show here.
Round 3 of the pre-M8 calibration gives uniform Lambert rooms such as these their own factor,
0.052 for T30 (0.098 for T20), calibrated on 9 such cells (10 for T20) and validated on 3 held
out; round 4 checked them at a mean absorption of 0.3 to 0.4 in four other rooms, where they
failed the two long ones, so they now hold up to 0.2 only, and above it, as in every room not
uniform Lambert, the noise is read from the series' own roughness (round 4,
`docs/investigations/2026-09-25-noise-calibration/`). C80 at α 0.05 in the 6×10×3 m room was
refused `missing_moves` in 13 of 18 receiver-bands until 2026-09-25: 125 lost particles a band in
a 4 s run make the energetic lost share `10·125/1,500,000` = 8.3·10⁻⁴, which moves C80 (about
−2.9 dB) by up to 0.011 dB against the 0.01 dB limit then held. Read again with the aligned
limits, 0.1 dB, it comes through in all 18 in every seed, and C50 (0 of 18 before) too ("The C and
D limits aligned", below).

### The C and D limits aligned (2026-09-25)

The M8 design decision 3 of 00:20 aligned the truncation limits of C50, C80 and D50 to the rule
Burhan confirmed on 2026-09-24, 1/10 of a difference limen: 0.1 dB and 0.5 points, where the code
held gate (a)'s 0.01 dB and 0.1 points (`docs/params.md`, "Truncation"). Measured on the runs
already made, read with the build before and after the change and nothing else changed
(`docs/investigations/2026-09-25-cd-limits/`):
- **The M8 cell above** (6×10×3 m, α 0.05, energetic, 1.5 M, three seeds): C80 through in every
  seed 5 → 18 of 18 receiver-bands, C50 0 → 18; nothing else moves.
- **The 960 runs of the noise calibration** (rounds 1 to 4 and tutorial 1 at upstream's default):
  receiver-band values given, energetic C50 10,536 → 12,174 of 21,060, C80 10,880 → 11,704, D50
  12,867 → 13,008; random C50 7,136 → 7,213 of 13,860, C80 6,937 → 7,018, D50 7,644 → 7,680. What the
  new values were refused for before: energetic `missing_moves` (C50 1,394, C80 517, D50 27) and
  `truncated` (244, 307, 114); random `truncated` (77, 81, 36). Tutorial 1 at upstream's default
  gives the same counts as before in both methods (C80 458 random, 484 energetic; D50 540). No
  value given before is refused after, and no other quantity changes in any run.
- **No bias is resolved in what now comes through**: each new value against the mean of the same
  receiver-band's values the other seeds gave before, where at least three did, is off by
  −0.002 ± 0.002 dB (C50, 1,116 values), +0.002 ± 0.003 dB (C80, 721) and −0.03 ± 0.04 points (D50,
  170); the spread about that mean (95th percentile 0.15 dB, 0.16 dB and 1.1 points) is the seeds'
  Monte-Carlo noise, which the noise limits bound apart (a standard deviation of at most 0.5 dB
and 2.5 points, "Monte-Carlo noise" above).
- **What it costs:** a value is now known only to within the rule's 0.1 dB or 0.5 points of what
  the unknown tail, missing energy or arrival could make it, where it was 0.01 dB or 0.1 points.
  On gate (a)'s exact decays with the arrival detected, 16 of the 40 values now given at 1 ms lie
  between gate (a)'s bound and the limit (`params_synthetic.rs`).

### The second table: air absorption on

Octave bands 125 Hz to 8 kHz, 20 °C, 50 %; "vs Eyring" here is against `K·V/(A + 4mV)` with the air
term SPPS applies (`params::air::solver_air_absorption_per_m`), per band from 125 Hz to 8 kHz.

| Method | Room | α | Particles | Through: T30 | σ | Range | vs Eyring + 4mV, 125 Hz … 8 kHz | Wall |
|---|---|---|---|---|---|---|---|---|
| random | 6×10×3 | 0.05 | 6 M, 4 s | 21 / 21 | 0.94 ± 0.10 % | 3.70 % | +1.30, +1.51, +0.93, +1.31, +0.72, −0.07, −0.37 % (± 0.3 %) | 345 s |
| energetic, ε 7 | 6×10×3 | 0.05 | 4 M, 4 s | 21 / 21 | 0.03 % | 0.11 % | +1.20, +1.20, +1.15, +1.11, +1.03, +0.82, +0.46 % | 3428 s |
| random | 5×4×3 | 0.1 | 4 M, 2 s | 21 / 21 | 1.01 ± 0.11 % | 4.74 % | +2.94, +2.84, +1.89, +2.03, +2.33, +2.04, +1.33 % (± 0.3 %) | 108 s |
| energetic, ε 7 | 5×4×3 | 0.1 | 4 M, 2 s | 21 / 21 | 0.03 % | 0.14 % | +2.21, +2.19, +2.17, +2.12, +2.08, +1.88, +1.37 % | 1864 s |

Air adds its rate to the room's exactly, in SPPS (a factor `e^(−m·c·dt)` a step on the energy or on
survival, `CalculationCore.cpp:57, 64`) as in Eyring's `4mV`, while the excess over Eyring comes
from the walls alone (below). So the excess falls as air takes a larger share: predicted from the
independent transport's T without air plus the air rate, +0.46 % at 8 kHz in the 6×10×3 m room and
+1.37 % in the 5×4×3 m one, which is what energetic SPPS gives to 0.01 %. Every band is within 5 %.

**TCR on every cell** (`m8_tcr_cells`: both rooms, the four α, air off and on): TCR's Eyring time
equals the analytic value, TCR's constant 0.163 with the solver's `m`, within 4.4·10⁻⁷ relative
in every band. Against Eyring with SPPS's constant it is +1.23 % everywhere, the constant alone.

### T30 against Eyring: the reference, measured apart from SPPS

Second review: the first version of this section read "random and energetic agree, so this is the
reference, not the solver". It was not evidence: both methods share SPPS's whole transport. This
is. `crates/simpa-core/tests/lambert_box.rs` is a transport written from scratch for it, sharing no
code with SPPS: straight rays in the box, Lambert (cosine) reflection, the energy multiplied by
`1 − α` at each reflection, receiver balls of 0.31 m collecting energy times path length per bin,
4,000,000 rays per cell in 8 independent replicas, T30 read through `params` as SPPS's are.

| Room | α | `γ²` | Transport | SPPS energetic | SPPS random (highest count) | Kuttruff with this `γ²` |
|---|---|---|---|---|---|---|
| 6×10×3 | 0.05 | 0.388 | +1.20 % | +1.19 % | +1.01 % | +1.01 % |
| 6×10×3 | 0.1 | 0.388 | +2.44 % | +2.42 % | +2.26 % | +2.09 % |
| 6×10×3 | 0.2 | 0.387 | +4.96 % | +4.96 % | +4.95 % | +4.51 % |
| 6×10×3 | 0.4 | 0.384 | +10.78 % | +10.74 % | +10.73 % | +10.86 % |
| 5×4×3 | 0.05 | 0.352 | +1.11 % | +1.10 % | +1.04 % | +0.91 % |
| 5×4×3 | 0.1 | 0.352 | +2.20 % | +2.22 % | +2.18 % | +1.89 % |
| 5×4×3 | 0.2 | 0.351 | +4.47 % | +4.45 % | +4.45 % | +4.08 % |
| 5×4×3 | 0.4 | 0.350 | +9.25 % | +9.24 % | +9.28 % | +9.83 % |

- The transport's mean free path is `4V/S` within 0.3 % (3.334 against 3.333 m; 2.554 against
  2.553 m), and its room energy decays at the rate its receivers give (T within 0.05 %).
- **SPPS reproduces it**: energetic within 0.04 % in every cell (the transport's standard error is
  0.01 to 0.07 %), random within 0.2 % (its cell mean carries about 0.1 % of noise, and the three
  receivers of a band share their particles). The excess over Eyring is the Lambert box's, not
  SPPS's: a 5 % tolerance against Eyring fails the α 0.4 cells for physics, and α 0.2 in the
  6×10×3 m room sits at it.
- Kuttruff's correction with the transport's own `γ²` comes within 0.6 % of the transport. Its
  `γ²` depends on the room (0.388 and 0.352 here), so it would need computing for each room, apart
  from SPPS. The options are Burhan's (`docs/params.md`, "The reference M8 compares against").

**Since (pre-M8, after Burhan's decision of 23:14).** The transport is core code
(`params::lambert`), and Kuttruff's formula with its `γ²` (`params::room::kuttruff_rt`); every SPPS
report carries both references, labelled and not validated (`spps.reference`,
`docs/formats/results-json.md`). Two corrections to the table above, measured
(`docs/params.md`, "Kuttruff's reference"):
- **`γ²`'s exact values** are 0.388874 and 0.352401, from integral geometry (the table's 0.388 and
  0.352 agree to their third decimal, and its lower values at higher α came from counting paths
  from the first reflection of rays started at the source: shorter runs, more of the ray's memory
  of where it started).
- **Kuttruff with the exact `γ²`**, against the core transport's receivers at 4 M to 34 M rays a
  cell: −0.201, −0.343, −0.415, +0.279 % (6×10×3 m) and −0.178, −0.300, −0.352, +0.587 %
  (5×4×3 m) at α 0.05, 0.1, 0.2 and 0.4, each ±0.01 to 0.02 %; against the room's energy the
  same within 0.04 %, ±0.002 to 0.004 %. The core transport's own T30 against Eyring reproduces
  the "Transport" column within 0.07 %. The table's `γ²` values are its counting's: the core
  transport, counting as `lambert_box.rs` counted, gives 0.38812 and 0.35187.
- **The reference as shipped** (`kuttruff_s`, with the transport's `γ²` at its fixed settings) is
  within 0.6 % of the core transport's receivers and room energy in every cell: worst +0.595 % and
  +0.593 % (5×4×3 m, α 0.4). Nothing here is validated; M8 has not run.

### Constraints on M8 from the noise calibration (pre-M8 review, 2026-09-25)

`docs/investigations/2026-09-25-noise-calibration/` (rounds 3 and 4):
- **Do not average only what came through.** A seed whose decay reads long has the larger spread
  of its own, so where the noise refusal passes some seeds or receiver-bands of a cell and refuses
  others, the passing mean sits low: random T30 −0.82 % ± 0.13 %, T20 −0.47 %, EDT −0.27 %;
  energetic T30 −1.23 %, T20 −0.75 % (receiver-bands of the receipt's cells partly refused under
  round 4's model). That is more than
  the 0.2 % by which SPPS and the independent transport agree, on which M8's tight cross-check
  rests. M8 either requires every receiver-band and every seed of a cell it judges to come
  through, or judges bias on every seed's value whatever its refusal for noise, with the spread
  over the seeds as the uncertainty.
- **Direct-field tolerances from the seeds' spread.** M8's SPL direct-field calibration takes its
  window from the spread over seeds (`level_box_over_ten_seeds`), not from `mc_sd`, whose factor is
  the largest over every room ("Level calibration" above).
- **Count again.** The counts above were counted with M7's model; round 3's gives energetic T30 in
  M8's uniform Lambert rooms 0.052 of M7's standard deviation, and random T30 1.6 to 2.1 times it.
  Since round 4 that 0.052 holds up to α 0.2 only: M8's α 0.4 cells take the roughness structure
  (T30 at 1.3 of it, which read 0.90 to 1.02 of the seeds' spread in the 0.4 boxes, T20 at 1.4),
  and the α 0.2 cells stay at the bound.
- **The refused-resamples rule** refused energetic T30 in uniform Lambert rooms at α 0.2 to 0.4
  below about a million particles although its calibrated noise was within the limit (the
  resamples carry M7's structure, 20 to 38 times T30's real noise there): 57 of 360 through at
  150,000 particles (5 × 4 × 3 m, α 0.4, 1 ms), 360 of 360 at 2,400,000. Since round 4 the α 0.4
  bands are judged by the roughness (F6) and give all 360 at 150,000; at α 0.2 the rule still
  refuses (163 of 360 through in a 20 × 4 × 3 m room at 300,000), and each such refusal names the
  count at which the model's resamples clear (R4-3). M8's energetic cells run 1.5 million and more;
  the bed counts what comes through.
- **Partly refused receiver-bands are common in energetic mode now**: the roughness lets many
  more decay times through, and where some seeds of a receiver-band pass and others are refused,
  the passing mean sits low (energetic T30 −1.23 % ± 0.13 % over 190 such receiver-bands, T20
  −0.75 %; `selection.txt`).

### Seed spread: what "≤ 2 %" asks for

The range of three seeds is a poor statistic: for normal noise its mean is 1.69 σ and its 95 %
point 3.31 σ, so a single range is uncertain by about half. The σ above, pooled over 18
receiver-bands (36 degrees of freedom), is uncertain by about 12 %; over 20 seeds (342), by 4 %.
- **Over 20 seeds at 8 M** (6×10×3 α 0.2 and 5×4×3 α 0.4, every triple of the 20, 1140): a
  receiver-band's 3-seed range is within 2 % in 37 % of cases, never in all 18 of a triple (the
  worst of 18 has median 5.7 %, 5–95 % 4.1 to 7.9 %), and **the cell's mean in every triple** (its σ
  0.39 % and 0.34 %).
- **σ falls as `1/√N`**: from the 20-seed σ, 0.51 % and 0.57 % are predicted at 80 M and 60 M;
  0.45 ± 0.05 % and 0.52 ± 0.06 % were measured.
- **Per receiver-band, measured**: at the bold counts every receiver-band of every random cell
  came within 2 % over seeds 1 to 3 (worst 1.47 to 1.94 %). From σ the chance of that is 0.35 to
  0.98 (6×10×3: 0.35 at 12 M, 0.98 at 40 M, 0.91 at 80 M, 0.96 at 260 M; 5×4×3: 0.87 at 10 M, 0.68
  at 18 M, 0.96 at 40 M, 0.74 at 60 M), so some passed by luck. **For a 95 % chance that all 18
  pass**: 6×10×3: 25 M, 35 M, 90 M and 252 M at α 0.05, 0.1, 0.2 and 0.4; 5×4×3: 12 M, 28 M, 39 M
  and 87 M; random-mode wall time grows in proportion.
- **On the cell's mean**: met from 1.5 M in every cell.
- **Energetic**: every receiver-band within 0.65 % from 1.5 M, in every cell.

### EDT and the time step

The second review's probe (`docs/params.md`, "The early reverberation"): the same cells at `dt`
10, 1 and 0.2 ms, energetic, 1.5 M, seeds 1 to 3, EDT per receiver from the curve with the
reverberation continued back (the model alone) over its 18 band-seeds, against 0.2 ms:

| Cell | 10 ms | 1 ms | T30 at 10 ms |
|---|---|---|---|
| 5×4×3, α 0.4 | **−4.50, −4.47, −4.89 %** (42 to 53 standard errors) | −0.01, +0.10, −0.15 % | within 0.05 % |
| 6×10×3, α 0.2 | −0.17, −0.49, −0.36 % (up to 5.5) | −0.09, +0.13, −0.00 % | within 0.03 % |

The independent transport gives the same (5×4×3 α 0.4: −4.47 to −4.75 %; 6×10×3 α 0.4: +0.12 to
−1.75 %), so the bias is the histogram's, not SPPS's. Read three ways, as SPPS's series now are,
EDT at 10 ms is refused `early_unresolved` in every cell but α 0.05 (both rooms) and α 0.1 in the
6×10×3 m room, where the transport shows it within 0.30 % of 0.2 ms. A step of 1 ms costs 1.2 to
1.6 times the time of 10 ms, 0.2 ms 2.9 to 3.6 times (131, 161 and 382 s; 294, 458 and 1058 s).

### Particles left alive at the end

Random mode at high counts leaves a few particles alive at the end in the 6×10×3 m room, about one
in 10⁸ to 10⁹ particle-bands (18 in 4.7·10⁹ at 260 M), and none in 2·10⁹ in the 5×4×3 m room. They
cannot be survivors: at α 0.4 surviving the 103 reflections of 1 s has a chance of 10⁻²³.
**Trapped, measured**: seed 2 at 48 M kept one particle alive at 500 Hz and one at 1 kHz after 1 s,
and the same run to 1.5 s kept them again, in the same two bands (a survivor at 1 s would be
absorbed by 1.5 s with a chance of 1 − 10⁻¹¹). The runs are deterministic: the 1.5 s run's
receiver histograms equal the 1 s run's bin for bin in the first second, and hold nothing after
it, so the trapped particles reach no receiver. Before this commit each one made its band
incomplete and its ragged random-mode end refused T30, EDT, C80 and D50 as `truncated` (15 of 18
receiver-bands at 48 M; 8 of 18 at 8 M over 20 seeds). Now a band with at most one particle in a
million alive is complete, those few bounded with the lost ones as unfinished paths ("Complete
series"). The mechanism inside SPPS is not known; an upstream finding, not raised.

### Lost particles in an M8 cell

The second review asked whether `ρ` = 10, measured on tutorial 1, holds in M8's rooms. Measured
in the 5×4×3 m room at α 0.4, energetic, `trans_epsilon` 9, 300,000 particles with every
trajectory saved, 3 seeds (`energetic_lost_particles_from_saved_trajectories` with
`$SIMPA_LOST_CELL`): SPPS counted 76 lost; the 44 that stopped with more than 10⁻⁵ of their start
energy, ten thousand times the floor, carried **0.04 to 4.4 times the mean** (median 1.0). The rest
ended within 10⁴ of the floor, where particles the floor dropped (up to 13 times above it at five
reflections a step) cannot be told from them. `ρ` = 10 is twice the largest measured.

### For Burhan's decisions

- **The reference** (above; `docs/params.md`): Eyring and a tolerance or α set that allows for the
  Lambert box's slower decay, or Kuttruff with a `γ²` computed apart from SPPS, or the independent
  transport. SPPS matches the transport to 0.04 % (energetic).
- **Seed spread**: per receiver-band needs the counts above (12 to 252 M in random mode for a 95 %
  chance in every receiver-band); on the cell's mean it is met from 1.5 M.
- **Energetic mode's noise model** over-stated T30's noise 30 to 40 times with M7's structure;
  it alone set the energetic counts (1.5 to 13 M, 5 to 57 minutes a run under load). Round 3 of
  the pre-M8 calibration gives M8's uniform Lambert rooms their own factor (0.052 for T30, 0.098
  for T20), validated on held-out rooms; rooms with concentrated absorption or specular walls keep
  a factor near 1 (`docs/investigations/2026-09-25-noise-calibration/`, "Open").
- **EDT**: at 10 ms it comes out only where the early decay is slow against the step; at 1 ms C50,
  C80, D50 and Ts are refused `params_bad_arrival` at most receivers (the strict rule of
  2026-09-24).
- **`trans_epsilon`**: 7 or more for energetic cells (5 biases T30, "Energetic mode: the solver's
  floor").

### Runs refused for a NaN

Before the lateral-column fix ("Verified runs only", step 6), 2 of the 3 energetic runs in the
5×4×3 m room at α 0.2 and 1.5 M, and 1 of 10 energetic runs of tutorial 1 at 1.5 M, were refused
whole for a NaN in a `.gap` lateral column; every table here reads them with the fix.
