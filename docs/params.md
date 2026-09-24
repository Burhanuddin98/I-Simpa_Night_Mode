# Acoustic parameters: `core::params`

**M7, piece A.** Pure functions over a per-band energy histogram, and the analytic references the
later gates need: ISO 9613-1 air attenuation, Sabine and Eyring as TCR computes them, and the DIN
18041 targets. Code: `crates/simpa-core/src/params.rs` and `params/`. Tests:
`crates/simpa-core/tests/params_synthetic.rs` (gate (a)), `params_air.rs` (gate (b)),
`params_room.rs` (Sabine, Eyring, gate (f)), `params_complete.rs` (complete series, lost
particles), `params_floor.rs` (the solver's floor), `params_noise.rs` (Monte-Carlo noise).

**No number computed here is shown to a user until M8's physics bed passes** (`docs/rebuild-plan.md`,
M12). M7 builds the numbers; it does not publish them.

## Sources, and how far each one is trusted

| Source | What we took from it | Read by us? |
|---|---|---|
| ISO 3382-1 | The definitions of EDT, T20, T30, C50, C80, D50, Ts and the onset rule, **as commonly stated**. Marked [commonly stated] below | **No.** The text is paywalled and was not opened |
| ISO 9613-1:1993(E) | Equations (1)–(6), clauses 5–7, Table 1 (a)–(h) (−20 °C to +15 °C) | **Yes**, pages 1–8 of the iTeh preview (`cdn.standards.iteh.ai/samples/17426/3a2d69b767024b74805b83b063a91445/ISO-9613-1-1993.pdf`, sha256 `cb0e28c6…`). Equations on printed page 3, clause 7 on printed page 4. Annex B and Table 1 (i) (20 °C) are **not** in the preview |
| GOST 31295.1-2005 (ISO 9613-1:1993, MOD) | Table 1 (i), 20 °C: gate (b)'s values | **Yes**, printed page 13 (PDF page 18) of `meganorm.ru/Data2/1/4293849/4293849418.pdf` (sha256 `03f11462…`). It is the Russian adoption, **not ISO's own page.** Its Annex F lists only editorial changes. Its 15 °C, 50 % column equals ISO's own Table 1 (h), page 8, in all 24 bands (both transcribed into `params_air.rs`, where the test asserts they are equal) |
| DIN 18041:2016-03 | The five group-A targets and their volume ranges | **No.** Formulas from BNB V 2020, criterion 3.1.4 (BMI, `bnb-nachhaltigesbauen.de/.../BNB_LN2020_314.pdf`, sha256 `576bdfda…`), pages 4 and 6–8, which quote "nach DIN 18041:2016-03". Ranges from the standard's figure as reproduced by C. Nocke, "Die neue DIN 18041 – Hörsamkeit in Räumen", Lärmbekämpfung 11 (2016) Nr. 2, Bild 2, page 51 (sha256 `5154ce6b…`). Details below |
| Upstream's GUI | How it computes every parameter: `src/isimpa/data_manager/projet_calculation.cpp`, `tree_rapport/e_report_gabe_recp.cpp` | Yes, at `929a5c8` (`B:\repos\I-Simpa-upstream`) |
| Upstream's solvers | What the energy values mean, and TCR's Sabine and Eyring | Yes, same commit |
| Night Mode `main:project/result_parser.cpp` | Nothing. Read only for its +26 dB bug (below) | Yes |

Local copies of every downloaded file are in `target/agents/m7-params-scratch/`, which is not
committed.

## The input: one band's energy histogram

An `EnergySeries` is `(dt, values)`: bin `k` holds the energy that arrived in `[k·dt, (k+1)·dt)`.

- **What a value is.** SPPS writes each point receiver's `.recp` value as
  `energy_sum × c·ρ / V_receiver` (`spps/sppsInitialisation.cpp:86`,
  `lib_interface/input_output/baseReportManager.cpp:183`). Upstream defines the level from it as
  `10·lg(value / p₀²)` with `p₀ = 20 µPa` (`lib_interface/coreTypes.h:417`), so each value is
  a mean-square-pressure contribution in Pa². Gate (c)'s level calibration is what checks this
  against the direct field.
- **Time base.** SPPS labels step `k` as `(k+1)·dt` ms, the end of the bin
  (`baseReportManager.cpp:428`, `spps/input_output/reportmanager.cpp:526`), rounded for display.
  `dt` comes from `pasdetemps` in `config.xml`, never from the labels (`docs/formats/gabe.md`).
- **Refused:**
  - `dt` not finite or ≤ 0: `params_bad_time_step`;
  - no values: `params_series_too_short`;
  - a value that is NaN, ±inf or negative: `params_bad_energy`, naming the first bad index;
  - all values zero: `params_no_energy`. Nothing about a decay can be said from silence.

**Bands are never mixed.** Every parameter is per band. `aggregate` sums bands bin by bin into a
series labelled as an aggregate, as upstream's `Global` row does (`projet_calculation.cpp:898`).
Series with a different `dt` or length are refused, `params_series_mismatch`. Upstream's `Average`
row, the arithmetic mean of the band values (`projet_calculation.cpp:206-209`), is not computed.

**The aggregate is not ISO 3382-1's single-number value.** ISO's single numbers [commonly stated]
are arithmetic means of octave-band values (Annex A: 500 Hz and 1 kHz for EDT and C80, for
instance). The aggregate is one decay of all bands' energy together, so it is weighted by the
source's spectrum: with a flat 100 dB per band it follows the bands that hold the most energy at
the receiver. It must never be shown as "the room's T30".

## Direct-arrival detection: the onset bin and the arrival

- **ISO 3382-1** [commonly stated]: the impulse response starts where the squared response first
  rises to within 20 dB of its maximum.
- **Applied to a histogram**, that finds a bin, not a time: the **onset bin** `k₀` is the first
  bin whose energy is at least 1/100 of the largest bin. The direct sound arrived somewhere in
  `[k₀·dt, (k₀+1)·dt)`. Energy before the onset bin is excluded from every onset-relative
  parameter (EDT, T20, T30, C, D and Ts). SPL sums all of it.
- **Where in the bin matters.** Every onset-relative parameter is measured from the **arrival**
  `t_a`, which the caller passes as an `Arrival`:
  - `Arrival::Known { time_s }`: the time is given, such as the source–receiver distance over the
    speed of sound. It must lie in the onset bin, or the call is refused as `params_bad_arrival`:
    before the bin, the direct sound is more than 20 dB below the strongest arrival and its time
    does not place the onset; after it, energy within 20 dB of the maximum came before the
    direct sound could have. A time a rounding step (10⁻⁹ dt) before the bin is taken as its
    start.
  - `Arrival::Detected`: not given. Every onset-relative parameter is computed with the arrival
    at **both ends of the onset bin**. The value reported is the mean of the two; when either end
    lies further from it than the parameter's limit (the truncation table below), the parameter is
    refused as `params_not_evaluable` (`unresolved`), with both ends in the detail.
- **Why, measured.** The first version of this module took `t_a` as the start of the onset bin.
  For a direct sound as strong as the reverberant decay (`D/R = 1`) at `dt = 10 ms`, arriving 0.05
  to 0.95 of the way into its bin (`params_synthetic.rs::a_direct_sound_measured_from_the_start_of_its_bin_fails`):

  | T | C50 | C80 | D50 | Ts |
  |---|---|---|---|---|
  | 0.3 s | −0.105 to −2.027 dB | −0.101 to −1.930 dB | −0.12 to −2.74 points | +0.25 to +5.96 ms |
  | 1 s | −0.040 to −0.779 dB | −0.036 to −0.693 dB | −0.17 to −3.51 points | +0.25 to +5.08 ms |
  | 3 s | −0.017 to −0.320 dB | −0.015 to −0.294 dB | −0.09 to −1.78 points | +0.25 to +4.86 ms |

  Every case misses the gate's bound on C, D or Ts. From the given arrival, all 30 cases (three
  `T`, two `dt`, five offsets) are exact to 2·10⁻¹³ (`a_direct_sound_inside_a_bin_meets_every_bound_from_its_arrival`).
- **What `Detected` can resolve, measured** (`D/R = 1`, the same five offsets): the decay times
  always, since they do not depend on where the time axis starts. At `dt = 10 ms`, C50, C80, D50
  and Ts are all `unresolved`; at `dt = 1 ms`, C50 and C80 still are, and D50 and Ts come through
  only at `T = 3 s`; at `dt = 0.1 ms` and `T = 3 s` everything comes through within the gate's
  bounds. Where refused, the two ends bracket the closed form. **So a run's C and D need the
  arrival: piece B should pass `r/c`.** For a receiver of radius `R`, energy can reach it at
  `(r − R)/c`; which of the two lands in SPPS's onset bin is for piece B and M8 to establish, not
  assumed here.
- **Upstream differs.** `GetTimeDecay` (`projet_calculation.cpp:127-139`) takes the last time label
  before the energy first changes by 10⁻¹⁸ in absolute value (`refValue`, lines 321, 357, 393,
  424). That is an absolute threshold, not a relative one, on labels that are bin ends; and the
  decay times ignore the onset altogether (below).

## The curve between bin edges

A histogram gives each bin's energy, not where in the bin it arrived. Every onset-relative
parameter is read from one continuous Schroeder curve `S(u)`, `u` the time since the arrival:

- **At every bin edge** after the arrival it is the histogram's backward sum, `S_k = Σ_{j≥k} B_j`,
  exactly.
- **Between two edges** it is log-linear: the energy inside the bin decays exponentially at the
  rate the two edge values imply. For the last bin with energy, when nothing is added after it,
  `S` falls linearly to 0 (energy spread evenly), since a logarithm cannot reach 0.
- **In the onset bin** it is the direct sound, an impulse at `t_a`, plus the decay of the next bin
  continued back to `t_a`: `S` just after the arrival is `S_{k₀+1}·(S_{k₀+1}/S_{k₀+2})^{(t₁−t_a)/dt}`,
  `t₁` the end of the onset bin, at most `S_{k₀}`. The rest of the bin is the impulse.
- **At the arrival itself**, `S(0) = S_{k₀}`, the direct sound included: 0 dB.

The model is exact for an exponential decay from `t_a` and for an impulse followed by one,
wherever in its bin `t_a` falls: gate (a) and its direct-sound cases meet every bound to
3·10⁻¹³, Ts included.

**What it assumes, and where it can be wrong.** Inside a bin, energy arrives as a smooth decay
would bring it. A strong reflection inside the bin that a window edge falls in can put up to that
bin's energy on the wrong side of the edge. In the onset bin, what the next bin's decay does not
explain is taken as the direct sound, at `t_a`. A direct sound spread over two bins (a receiver
whose crossing time is not small against `dt`), or an early reflection in the bin after the onset
bin, breaks that. None of these is bounded here; M8's bed on real runs is where they show.

## Schroeder backward integration and the unseen tail

The decay curve is `L(u) = 10·lg(S(u) / S(0))`, 0 dB at the arrival. Sums run from the last bin
backwards, in `f64`.

**Truncation.** A simulation stops at a finite time. The integral from the last bin misses the
energy that would have arrived after it, so the curve bends down near the end, and the fitted slope
is too steep: decay times come out short. SPPS has no noise floor to compensate as ISO 3382-1 does
for measurements, and **we add nothing to the curve**; adding the missing tail would be an
extrapolation. Instead each parameter carries a bound:

1. **The tail estimate.** Over the last `2w` bins up to the last bin with energy (`w` = a tenth of
   the bins from the onset to there, at least 1), with `W₁` and `W₂` the sums of the two halves:
   - `W₂ ≥ W₁` or `W₁ = 0`: the series is not decaying at its end, and the tail is unbounded;
   - otherwise, with `r = W₂/W₁`: `M = W₂·r / (1 − r)`, the energy after the last bin with energy
     if the decay goes on as it did over the last window. It is exact for an exponential.
   - **Trailing empty bins are not taken as proof that the decay ended.** A histogram that stops
     abruptly, such as one padded with zeros, is bounded from the energy before it stopped. A
     1 s decay stopped at −20 dB and padded is refused, with −20 dB as the depth reached
     (`decay.rs`, `a_histogram_that_stops_abruptly_is_refused_not_trusted`). The cost: a random-mode
     histogram that runs out of particles is bounded as if more were to come, which only refuses
     more.
   - Fewer than 2 bins from the onset to the last with energy: `params_series_too_short`.
   - **A series known to be complete** (`EnergySeries::complete`; `Tail::Complete`) has no tail:
     nothing is estimated, added or refused for it. Its caller must hold evidence that no energy
     arrives after the last bin; `core::results` claims it only for SPPS in random mode when the
     run's statistics count no particle remaining at the end (`docs/results.md`, "Complete
     series"). A decay time still needs the curve to reach the bottom of its range before the
     last bin with energy, inside which the curve has no shape: otherwise `range_not_reached`,
     with the level at the start of that bin. Added by M7 piece B, with its tests in
     `tests/params_complete.rs`: without it, random-mode runs were refused wholesale for a tail
     they do not have. **Completeness says nothing about noise**: a complete series is exact for
     its particles, not for the room, and its noise is judged separately ("Monte-Carlo noise",
     below). Particles the solver lost mid-path are bounded separately too ("Missing energy").
2. **The check.** Every parameter is computed twice: as reported, from the series alone; and with
   `M` added to every backward sum and continued after the last bin with energy as an exponential
   at the window's rate. If the two differ by more than the limit below, the parameter is refused
   as `params_not_evaluable` (`truncated`), with both numbers in the detail. An unbounded tail
   refuses everything that depends on it. **The second number is never reported as the value.**

   The same limits bound what `Arrival::Detected` leaves open (`unresolved`, above), and what the
   energy missing from a series can move ("Missing energy", below). Each is the tighter of 1/10 of
   a difference limen and gate (a)'s bound. **The limens:** ISO 3382-1:2009 Table A.1, as commonly
   reproduced (not read here), gives them for G (1 dB), EDT (5 %), C80 (1 dB), D50 (0.05) and Ts
   (10 ms) only. T20 and T30 have none there; they take EDT's 5 %, and C50 takes C80's 1 dB. Those
   two are extensions, not the standard's:

   | Quantity | Limit | 1/10 limen | Gate (a) |
   |---|---|---|---|
   | EDT | 0.5 % relative | 0.5 % (of EDT's 5 %) | 0.5 % |
   | T20, T30 | 0.5 % relative | 0.5 % (of EDT's 5 %, extended) | 0.5 % |
   | C80 | 0.01 dB | 0.1 dB (of 1 dB) | 0.01 dB |
   | C50 | 0.01 dB | 0.1 dB (of C80's 1 dB, extended) | none named; held as C80 |
   | D50 | 0.001 (0.1 points) | 0.005 (of 0.05) | 0.1 points |
   | Ts | 1 ms or 0.5 % of Ts, the tighter | 1 ms (of 10 ms) | none named; the test holds Ts to 0.5 % |
   | SPL | 0.1 dB | 0.1 dB (of the 1 dB given for G) | none (gate (c) is ±0.5 dB) |

   The first version used 1/10 of the limen alone, 0.1 dB for C and 0.5 points for D50, so a
   truncated series could return a C80 0.1 dB off as a number while gate (a) asks for 0.01 dB.

3. **The depth reached.** A decay time needs the curve to reach the bottom of its range. The depth
   is read from the curve **with** the tail added: that is how far the decay had fallen when the
   series ended. Without the tail the curve dives at the last bins and reaches almost any depth.
   Not deep enough: `params_not_evaluable` (`range_not_reached`), with the depth reached.

**Why the depth check alone is not enough, measured:** an exact exponential with `T = 1 s`,
`dt = 1 ms`, cut where the true decay reaches `D` dB. T30 is fitted over the bin-edge points of the
truncated curve in −5 … −35 dB with nothing added, as upstream does bar its one extra point:

| D at the end | 30 dB | 35 dB | 40 dB | 45 dB | 50 dB | 55 dB | 60 dB |
|---|---|---|---|---|---|---|---|
| T30 error | −12.2 % | −6.1 % | −2.4 % | −0.85 % | −0.28 % | −0.09 % | −0.03 % |

At D = 30 dB the truncated curve still passes −35 dB, in 5 of the 6 `(T, dt)` cases of gate (a),
so a T30 is produced, 12 % short, without a word. The same holds at `T = 0.3` and `3 s` and at
`dt = 10 ms` to within a point. With our bound, T30 at `T = 1 s`, `dt = 1 ms` is first accepted at
48 dB of true decay. That is close to the 45 dB of dynamic range usually quoted for T30 from
ISO 3382-1. Both the table, each entry to half a unit of its last printed digit, and the 48 dB are
asserted by `params_synthetic.rs::the_documented_truncation_table_holds`.

**Upstream differs.** `MakeSchroederArray` (`projet_calculation.cpp:68-83`) integrates backwards
without any tail handling. `GetTimeRange` (lines 143-170) sets the end of the regression to the
last time step when the bottom is never reached (line 150), so a decay that stops at −20 dB gets a
T30 regressed down to its end, silently.

## Missing energy: the solver's floor and lost particles

Added after the M7 review (2026-09-24). Energy can be missing from inside a series, not only after
its end, and the tail bound cannot see it:

- **The floor.** SPPS in energetic mode drops each particle once its energy is at or below
  `10^-trans_epsilon` of its start (`spps/sppsNantes.cpp:75`; `CalculationCore.cpp:57-60,
  141-146, 305-310`). The histogram then ends in a cliff. Inside it the tail estimate's last
  window falls steeply, `M` comes out near 0, and the curve reads as deep as it likes: in the
  reviewer's model (below) `trans_epsilon` 3 gave T30 11.8 % short and T20 5.1 % short, both
  accepted.
- **Lost particles.** SPPS counts particles lost to infinite loops and meshing problems
  (`partLoop`, `partLost`, `CalculationCore.cpp:102-107`); each stops mid-path, and what it would
  still have brought is missing.

`EnergySeries::with_solver_floor(floor_db, alive_share)` and `with_lost_share(share)` say so. The
most energy that can be missing is bounded, as a share of `S(onset)`, the energy the receiver
gets from the arrival on:

- a dropped particle carries at most `10^{floor/10}` of its start energy, and what it would still
  have brought is, on average, what that much energy brings from any particle alive then. From
  the arrival on, the receiver gets `S(onset)` from the `alive_share` of the emitted energy the
  room still held then, so the dropped particles together would have brought at most
  `10^{floor/10} / alive_share · S(onset)`;
- a lost particle would have brought what an average particle alive at the arrival brings, so `n`
  of `N` lost take at most `n / (N · alive_share)` of `S(onset)` (`core::results` computes it,
  `docs/results.md`, "Lost particles").

Every quantity is computed once more with that energy added to every backward sum and, after the
end, continued at the tail's rate (a lump at the end for a complete series, the latest it can
be). Moved beyond its limit: `params_not_evaluable` (`missing_moves`). A decay time must also reach
the bottom of its range with it added: otherwise `missing_not_cleared`. Adding a constant to the
backward sums is the worst case for a decay time: the true missing energy after `u` falls with
`u`, and adding more at later times bends the fit more.

**Tested against the reviewer's model** (`tests/params_floor.rs`), in closed form: tutorial 1's
room, reflections a Poisson process at `c/(4V/S)`, a particle's energy `(1 − α)^N`, dropped at
`10^-ε`, a direct sound of `S·α/(16πr²)` of the reverberant energy. Over α from 0.05 to 0.9, ε
from 1 to 7 and r of 2 and 8 m, 487 values are accepted and none is further from the model without
the drop than its limit; 368 are refused, 306 of which the series alone gets wrong. At upstream's
default ε = 5, α = 0.2, T30 is accepted (0.6688 s against 0.6709 s). Lost particles, in
`tests/params_complete.rs`: 100 of 150,000 lost at 0.5 s, two thirds of those then alive, make
T30 wrong from the series alone, and their share refuses it; one lost particle moves nothing.

**What it assumes:** that a dropped or lost particle's future is, on average, an alive particle's.
In a diffuse room that holds; a particle lost where it would have crossed the receiver more than
most is not covered.

## Monte-Carlo noise

Added after the M7 review. A complete random-mode series is exact for its particles, not for the
room: at tutorial 1's 150,000 particles T30 came out at 0.8 to 2.4 s where the room's time is
0.67 s, and was reported as a number. `params::noise` estimates each value's Monte-Carlo standard
deviation and refuses the value when it is too large.

- **The model.** SPPS adds, for every particle crossing a receiver sphere of radius `R`, its
  energy times its chord through the sphere (`spps/input_output/reportmanager.cpp:223-224`),
  and writes the sum times `ρc/V` (`baseReportManager.cpp:183`). In random mode a particle keeps
  its start energy `W/N` (`sppsNantes.cpp:73`) until it is absorbed whole, so one crossing adds
  `W/N · ℓ · ρc/V`. A uniform beam crossing a sphere gives chords of density `ℓ/(2R²)` on
  `[0, 2R]`: mean `4R/3`, `E[ℓ²]/E[ℓ]² = 9/8`. The mean deposit is `d̄ = W·ρc/(N·πR²)`; a bin
  holding `E` holds about `E/d̄` crossings; crossings are Poisson, so its variance is
  `(9/8)·d̄·E`.
- **Energetic mode.** A particle's energy only falls from `W/N`, so the same `d̄` bounds each
  deposit and the variance from above. The energetic estimator is the random one averaged over the
  absorption draws, so its variance is at most random mode's: the bound is sound, and loose.
- **The estimate.** A parametric bootstrap: 200 series drawn from the model around the series,
  each bin a compound Poisson sum of `E/d̄` expected crossings with chord deposits (a normal draw
  of the same mean and variance above 30), each evaluated as it is (complete, nothing missing:
  the tail, the floor and lost particles are judged once, on the series); the standard deviation
  over them is the value's. The seed is fixed, so a series gives the same estimate every time.
- **The refusal:** `monte_carlo_noise`, when the standard deviation is above the limit or more
  than 10 of the 200 resamples refuse the quantity themselves. The limit is half the limen: twice
  the standard deviation, about a 95 % interval, stays within one limen.

  | Quantity | Largest standard deviation |
  |---|---|
  | EDT, T20, T30 | 2.5 % relative (half of EDT's 5 %; T20 and T30 extended) |
  | C50, C80 | 0.5 dB |
  | D50 | 0.025 |
  | Ts | 5 ms |
  | SPL | 0.5 dB |

  M8's bed asks more of three seeds (a spread of at most 2 %); this limit is what one run may
  show, not the bed's.
- **Several sources:** the largest of their mean deposits, an upper bound. **A directivity
  balloon** scales each particle's energy by its direction, so `d̄` is not known: every value is
  refused, `noise_unknown`.
- **Checked** (`tests/params_noise.rs`), against 60 independent runs of the model with its own
  generator: at 40,000 and 400,000 crossings the estimate is the spread of the runs within a
  factor 0.85 to 1.25 for all eight quantities. At 4,000 crossings, tutorial 1's count at
  150,000 particles, 5 of 10 runs give a T30 more than 5 % off from the series alone, and every
  one is refused.
- **Checked against real SPPS runs** (M7 review; `crates/simpa/tests/cli_results.rs`,
  `noise_estimate_against_the_spread_of_twenty_seeds` and `level_box_over_ten_seeds`, run on
  purpose). Tutorial 1, seeds 1 to 20, octave bands 125 Hz to 4 kHz, both receivers (12
  receiver-bands); in each, the standard deviation of the values over the seeds against the
  root-mean-square of the estimates, pooled over the receiver-bands where every seed gives a value:

  | Quantity | 150,000 particles | 1,500,000 particles |
  |---|---|---|
  | SPL | 1.05 | 1.04 |
  | EDT | 0.98 | 1.16 |
  | T20 | 0.97 | 1.15 |
  | T30 | no receiver-band with a value in every seed | 1.03 |
  | C50 | 1.02 | 1.06 |
  | C80 | 0.95 | 1.09 |
  | D50 | 1.02 | 1.06 |
  | Ts | 1.05 | 1.21 |

  The level box over seeds 1 to 10 gives 1.12 for SPL. With 20 seeds a pooled ratio is uncertain
  by about 5 %, so at 150,000 particles the estimate matches. **At 1,500,000 it runs 15–21 % low
  for EDT, T20 and Ts**, about 3 standard errors: the spread falls more slowly than `1/√N`. Read
  as a component that does not fall with `N`, it is 0.5 % for EDT and 1.4 % for T20. At the
  limit the refusal acts on, 2.5 %, that makes the true standard deviation of an accepted EDT at
  most 1.02 times the limit and of a T20 1.15 times; the other quantities 1.00 to 1.003 times. Not
  explained: candidates are correlation between bins through a particle's shared path, which the
  model leaves out, and SPPS's generator: `rand()/RAND_MAX` (`spps/sppsTypes.h:24-27`; upstream's
  CMake defines no `__USE_BOOST_RANDOM_GENERATOR__`), 15 bits under the MSVC 19.44 runtime our
  solvers are built with (`solvers/manifest.json`). M8's seed spread is the evidence that settles
  it.
- **What it assumes:** crossings independent between bins. A particle crossing twice is counted
  twice; at tutorial 1's counts (0.03 crossings per particle) that correlation is about 3 %.

## Decay times: EDT, T20 and T30

[commonly stated] A least-squares line through the decay curve between the range limits,
extrapolated to 60 dB: `T = −60 / slope`.

| Quantity | Range | Same as |
|---|---|---|
| EDT | 0 dB to −10 dB | 6 × the 10 dB decay time |
| T20 | −5 dB to −25 dB | 3 × the 20 dB decay time |
| T30 | −5 dB to −35 dB | 2 × the 30 dB decay time |

- **The fit is over time, not over samples.** ISO 3382-1's regression runs over a finely sampled
  curve; ours runs over all the time `L(u)` spends inside the range, on the curve above, each
  stretch weighted by its length. Between bin edges `L` is linear, so the sums are exact. The
  last bin's linear fall to nothing is left out.
- **The direct sound is a step.** At the arrival the curve drops by the direct sound, `10·lg(1 + D/R)`
  dB, in no time, so it adds nothing to the fit, as in a finely sampled impulse response. The
  first version fitted the bin-edge points from the start of the onset bin, where one point at
  0 dB then stands for a whole bin: with `D/R = 1` at `dt = 10 ms`, its EDT read −6.0 % to −30.4 %
  short over `T` from 0.3 to 3 s and five offsets in the bin; ours reads exact to 4·10⁻¹⁵
  (`params_synthetic.rs::the_first_versions_edt_regression_fails_with_a_direct_sound`).
- **Refused:**
  - the curve spends less than 2 bin widths inside the range: `params_not_evaluable`
    (`range_too_short`). A direct sound more than 10 dB above the decay jumps over EDT's whole
    range, so EDT is refused rather than fitted to the jump;
  - a slope that is not negative: `not_decaying`;
  - the depth reached is above the bottom (`range_not_reached`, with the depth); the truncation
    bound is exceeded (`truncated`); and, detected, the two ends of the onset bin disagree
    (`unresolved`).
- **Curvature.** `C = 100·(T30/T20 − 1)` %, from ISO 3382-2 as commonly stated, which
  reads a value above 10 % as a curved (double-slope) decay. We flag `|C| > 10 %`. The flag is a
  typed field of the band's result, not a log line: M8 and M12 decide what a curved decay may show.
- **Upstream differs.**
  - EDT's regression starts at the first time step, not at the onset (`GetTimeRange` with
    `fromdB = 0` takes `timeTable[0]`, line 149). Before the direct sound the curve is flat at
    0 dB, so upstream's EDT includes a flat stretch the length of the propagation delay, and reads
    long.
  - Each range ends at the first step past the bottom (lines 163-166), so one point below the
    range is included.
  - Upstream fits in `f32`, on bin-edge points in ms.
  - It also computes T15 (`TRdefault = "15;30"`, line 803), which we do not.

## Clarity C50 and C80, definition D50, centre time Ts

[commonly stated], with `u` the time since the arrival and `E` the energy:

- **C_te** = `10·lg( ∫₀^te E du / ∫_te^∞ E du )` dB, te = 50 or 80 ms.
- **D50** = `∫₀^50ms E du / ∫₀^∞ E du`, a fraction (shown in %).
- **Ts** = `∫₀^∞ u·E du / ∫₀^∞ E du`, in seconds.

How they are evaluated, all from the curve `S(u)`:
- **Windows.** The early energy is `S(0) − S(te)`, the late `S(te)`, the total `S(0)`. The direct
  sound, and the whole onset bin with it, is early. A window edge inside a bin splits the bin as
  the curve runs there, exponentially.
- **Ts** is `∫₀^∞ S(u) du / S(0)`, the same integral by parts; the direct sound, at `u = 0`, adds
  nothing to it. For an exponential this is `τ` exactly. (The first version put each bin's energy
  at the bin's midpoint, which reads long by `(dt/τ)²/12` of τ: 1.8 % at `T = 0.3 s`,
  `dt = 10 ms`.)
- **Refused:**
  - the series ends at or before `t_a + te` (from either end of the onset bin, when detected):
    `params_series_too_short`;
  - there is no energy after `t_a + te`: `params_not_evaluable` (`empty_window`);
  - the truncation bound is exceeded: `truncated`; detected, the two ends of the onset bin disagree:
    `unresolved`.
- **Upstream differs:**
  - `GetSumLimit` includes both window edges (`projet_calculation.cpp:102`), on time labels that
    are bin ends. So for C the bin that ends at `t₀ + te` counts as early and as late (line 366).
    D (line 401) divides the early sum by the sum from `t₀`, so its boundary bin is counted once in
    each, which is right; D differs from ours only in its `t₀`.
  - Its `t₀` is the absolute-threshold label above, the end of a bin.
  - **Its Ts is not measured from the onset.** Line 432 weights by the absolute time label, so
    upstream's Ts includes the propagation delay `r/c` and half a bin more.

## Sound pressure level

`SPL = 10·lg( Σ_k B_k / p₀² )`, with `p₀² = (20 µPa)² = 4·10⁻¹⁰ Pa²`, over every bin of the band.
It is the steady-state level of a source emitting its power continuously. This is upstream's
`dB_Sum_Param` (`projet_calculation.cpp:220-240`, with `p_0` at line 41) and its receiver view
(`e_report_gabe_recp.cpp:56, 145`). It does not depend on the arrival.

**Night Mode's +26 dB bug:** its `.gap` reader divided the energy by `10⁻¹²`, the intensity
reference, instead of multiplying by `1/p₀²` (`main:project/result_parser.cpp:486`).
`10·lg(4·10⁻¹⁰ / 10⁻¹²) = 26.02 dB`. The SPL function takes no reference argument, so the mistake
cannot be made through it; gate (c) checks the level end to end.

## ISO 9613-1: air attenuation

**The equations, from the standard's own page 3**, α in dB/m, `p_r = 101.325 kPa`,
`T₀ = 293.15 K`, `h` the molar concentration of water vapour in %:

- (3) `f_rO = (p_a/p_r)·(24 + 4.04·10⁴·h·(0.02 + h)/(0.391 + h))`
- (4) `f_rN = (p_a/p_r)·(T/T₀)^(−1/2)·(9 + 280·h·exp(−4.170·((T/T₀)^(−1/3) − 1)))`
- (5) `α = 8.686·f²·[1.84·10⁻¹¹·(p_a/p_r)⁻¹·(T/T₀)^(1/2) + (T/T₀)^(−5/2)·(0.01275·e^(−2239.1/T)/(f_rO + f²/f_rO) + 0.1068·e^(−3352.0/T)/(f_rN + f²/f_rN))]`
- (6) Table 1 is computed at the **exact** midband frequencies `f_m = 1000·10^(k/10)` Hz, not at the
  nominal ones (note 5, page 3).

**Humidity to `h`** is Annex B, which we could not open. We use the saturation pressure upstream
uses (`lib_interface/data_manager/data_calculation/Coef_Att_Atmos.cpp:54-55`), as commonly stated
for Annex B:
- `p_sat/p_r = 10^C`, with `C = −6.8346·(T₀₁/T)^1.261 + 4.6151` and `T₀₁ = 273.16 K`;
- `h = h_r·(p_sat/p_r)/(p_a/p_r)`. The division by the ambient pressure is the standard's own:
  `h` is the partial pressure of water vapour over the atmospheric pressure (clause 6.1, note 3,
  page 2).

**The accuracy the standard states** (clause 7, page 4), each class also below 200 kPa and at a
frequency-to-pressure ratio of 4·10⁻⁴ to 10 Hz/Pa:

| Class | `h` | Temperature |
|---|---|---|
| ±10 % (7.1) | 0.05 % to 5 % | −20 °C to +50 °C |
| ±20 % (7.2) | 0.005 % to 0.05 %, or above 5 % | −20 °C to +50 °C |
| ±50 % (7.3) | below 0.005 % | above 200 K |

Outside all three the standard states no accuracy. `air::attenuation` returns α with its class,
or `None`; `air::stated_accuracy` gives the class alone. At 101.325 kPa the frequency ratio
excludes the bands below 40.5 Hz, so a 25 Hz or 31.5 Hz band has no stated accuracy
(`params_air.rs::the_stated_accuracy_follows_clause_7_and_says_none_outside_it`). The first
version checked only that the inputs were physical.

**Gate (b)**, at 20 °C, 50 % RH and 101.325 kPa, against GOST 31295.1-2005 Table 1 (i). Our
equations at the exact midband frequencies are within **0.29 %** in every band from 50 Hz to
10 kHz, worst at 2.5 kHz, where the table's third digit rounds. Against the standard's own
page 8, at 50 % RH, the worst band is within **0.33 %** at 15 °C (Table 1 (h)) and **0.29 %** at
10 °C (Table 1 (g)). All in `params_air.rs`.
- Moving 1 °C puts 21 of the 24 bands outside 1 %, and 5 % RH puts 22 outside.
- The same equations at the nominal frequencies miss the table by up to 1.6 % (160 Hz; also 80,
  125, 1600 and 8000 Hz over 1 %), because α goes as f² at high frequencies.

**Upstream's version** (`Coef_Att_Atmos.cpp:48-75`) writes each relaxation term as
`1.559·X·(θ/T)²·e^(−θ/T)·(f/c)·2(f/f_r)/(1 + (f/f_r)²)`, with `c = 343.2·√(T/293.15)`
(`Celerite_du_son.cpp:46`). That is algebraically the same as (5): 1.559 × 0.209 × (2239.1/293.15)²
× 2/343.2 = 0.11077 against 8.686 × 0.01275 = 0.11075. It agrees with ours within 0.035 % at
101.325 kPa. It differs in two places:
- **`h` without the pressure factor** (`hmol = H·Ps/Pref`, line 56): the same at 101.325 kPa,
  and 24.6 % off in the worst band at 80 kPa, the pressure about 2000 m up. A finding for M8,
  not yet raised upstream.
- **It is evaluated at the nominal band frequency**, the integer `freqValue`
  (`base_core_configuration.cpp:114`). So SPPS and TCR use values up to 1.6 % off the table.

`air::solver_air_absorption_per_m` reproduces what the solvers use: upstream's form at the nominal
frequency, converted. The gates that compare against TCR (M7 (d), M8) use it. `air::attenuation_db_per_m`
is the standard's.

**The conversion SPPS and TCR use.** Upstream stores `m = α·ln(10)/10` per metre, the energy
attenuation `I(x) = I₀·e^(−m·x)` (`base_core_configuration.cpp:114`), and the per-step factor
`e^(−m·c·dt)` (line 115). SPPS applies that factor two ways: in energetic mode it multiplies the
particle's energy by it (`spps/CalculationCore.cpp:57`); in random mode the particle survives the
step with that probability and is otherwise destroyed (line 64). TCR adds `4·m·V` to the
absorption area (`TC_CalculationCore.cpp:138`). The header comment calls it "dB/m"
(`coreTypes.h:47`); it is not. A user-set `absatmo` is used verbatim as `m`
(`docs/formats/config_xml.md`).

## Sabine and Eyring, as TCR computes them

From `src/ctr/TC_CalculationCore.cpp`:

- **Surfaces** are every face of the scene except fitting faces whose material's first band has
  `τ ≠ 1` (`isTransparent`, lines 11-17). The caller passes the surfaces; `params::room` does not
  choose them.
- **Sabine:** `A = Σ Sᵢ·αᵢ` (lines 87-102).
- **Eyring:** `A = −S·ln(1 − ᾱ)`, with `S = Σ Sᵢ` and `ᾱ = Σ Sᵢ·αᵢ / S` (lines 104-133).
- **Reverberation time:** `T = K·V / (4·m·V + A)` when `abs_atmo_calc` is on, `K·V / A` when it is
  off (lines 135-141). `V` is the tetrahedral mesh's volume (lines 199-209).
- **`K = 0.163` in TCR.** The physical value is `24·ln(10)/c` = 0.16102 at TCR's own
  `c = 343.2 m/s`; 0.163 is 1.23 % above it (the plan's 1.24 % rounds 0.16102 to 0.1610 first).
  `RtConstant::Tcr` reproduces TCR, as the gates that compare with TCR require. `RtConstant::Physical { c }` gives the physical value for M8's analytic references
  (`docs/rebuild-plan.md`, critic's flaw 1).
- TCR keeps `A` in `f32` (line 223); the effect is below 10⁻⁶.

Refused (`params_bad_room`):
- a volume that is not finite and positive;
- an area that is negative or not finite;
- α outside [0, 1];
- an air term `m` that is negative or not finite;
- no surface area at all.

`A + 4mV = 0` is `params_no_absorption`: the reverberation time is infinite. `ᾱ = 1` gives an
Eyring time of 0 s, a fully absorbing room.

## DIN 18041 targets

`T_soll`, in s, with `V` in m³:

| Group | Use | T_soll (BNB page) | Volumes |
|---|---|---|---|
| A1 | Music | 0.45·lg V + 0.07 (6) | 30 to 1000 m³ |
| A2 | Speech, lecture | 0.37·lg V − 0.14 (7) | 50 to 5000 m³ |
| A3 | Teaching, communication | 0.32·lg V − 0.17 (7) | 30 to 5000 m³ |
| A4 | Teaching, communication, inclusive | 0.26·lg V − 0.14 (4) | 30 to 500 m³ |
| A5 | Sport | 0.75·lg V − 1.00 to 10 000 m³, then 2.0 s (8) | 200 to 30 000 m³ |

- **Gate (f):** A3 at 180 m³ gives 0.5517 s, within 0.552 ± 0.001, and 0.55 s in the design README.
- **The formulas** are BNB V 2020, criterion 3.1.4, which quotes them "nach DIN 18041:2016-03".
- **The volumes** are where each group's line is drawn solid in the standard's figure of `T_soll`
  against `V`, as reproduced by Nocke (Lärmbekämpfung 11 (2016) Nr. 2, Bild 2, page 51); the
  article calls the solid stretches the typical volumes of each use, and draws the rest dotted.
  The ends were measured on the figure against its 30, 50, 100, 200, 500, 1000, 5000 and 10 000
  m³ grid lines; each falls within a line's end cap (under 4 % in volume) of one of them. The
  text sources agree wherever they speak:
  - A4 is not suitable for rooms above 500 m³: Nocke, Table 1, page 52; also fennext.eu,
    "DIN 18041 Hörsamkeit in Räumen" (sha256 `9d9498f3…`).
  - The equations apply between 30 and 5000 m³: C. Ruhe, "Akustik in Bildungsbauten", 5.2
    (`carsten-ruhe.de/downloads/akustik-in-bildungsbauten/5-2-raumgruppe-a/`, sha256 `f4361626…`).
  - A5 is the formula from 200 to 10 000 m³ and 2.0 s above: BNB, page 8. Sports halls are
    covered up to 30 000 m³: fennext.eu.
  - Above 5000 m³ only sport and swimming halls have requirements: DEGA Akustik Journal 03/19,
    page 17 (sha256 `fb2ddd25…`). The same page says the exact formulas and volume ranges are in
    the standard's text. **Open:** check the ranges, and whether their ends are inclusive,
    against that text. Both ends are accepted here.
  - A volume outside its group's range is refused, `params_din_out_of_range`. The first version
    accepted A1–A4 up to 5000 m³ and down to where the formula reached 0 s (3.4 m³ for A3), and
    A5 at any volume above 10 000 m³, and credited the 5000 m³ limit to the DEGA page, which
    gives no per-group limit.
- **What a target applies to.** `T_soll` is for the room **80 % occupied**, at mid frequencies
  (Nocke, page 51: "beziehen sich auf den besetzten Zustand … 80%igen Ausschöpfung der
  Regelbesetzung"; Ruhe, 5.2: "bei mittleren Frequenzen … für den zu 80 % besetzten Zustand"),
  and `V` is the effective room volume (Ruhe, 5.2). An unoccupied model's T30 compared with
  `target_s` is off by the audience's absorption.
- The DEGA article's example, 245 m³ in A3, prints 0.60 s. The formula gives 0.5945 s, which is
  0.6 to one decimal but 0.59 to two. It confirms the formula to 0.01 s at best, and is not used
  as a check.

## Refusal codes

Each code is a row of `docs/solver-contract.md`, Part B, "Parameter refusals". A refusal is a
typed `ParamError`, never a warning and never a number. `params_not_evaluable` carries the quantity
and one of:
- `range_not_reached`, with the depth reached;
- `truncated`, with the value and the value with the tail added (no second value when the tail is
  unbounded, or when the curve with the tail spends too little time in the range to fit);
- `unresolved`, with the values from both ends of the onset bin;
- `range_too_short`, with the time spent in the range and the time needed;
- `not_decaying`;
- `empty_window`;
- `missing_not_cleared` and `missing_moves`, with the floor and the lost share, and the depth
  reached or the value with the missing energy added ("Missing energy");
- `monte_carlo_noise`, with the value, its standard deviation, the limit and the resamples that
  refused it; `noise_unknown`, with why ("Monte-Carlo noise");
- `several_sources`, with the sources: made by `core::results`, not by `params`
  (`docs/results.md`, "Several sources");
- `no_time_series`, with where the solver's own values are: made by `core::results` for every
  parameter of a TCR receiver, which has steady-state levels and no series
  (`docs/formats/results-json.md`, "`tcr`").

`params_bad_noise_input` refuses a floor, a share alive or lost, or a mean deposit that is not a
finite number in its domain.

The citations of upstream's lines in `params::air` and `params::room` are checked against the
source at `929a5c8` by `params_air.rs` and `params_room.rs`, so a citation that drifts fails a test.
