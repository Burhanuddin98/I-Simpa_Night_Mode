# Acoustic parameters: `core::params`

**M7, piece A.** Pure functions over a per-band energy histogram, and the analytic references the
later gates need: ISO 9613-1 air attenuation, Sabine and Eyring as TCR computes them, and the DIN
18041 targets. Code: `crates/simpa-core/src/params.rs` and `params/`. Tests:
`crates/simpa-core/tests/params_synthetic.rs` (gate (a)), `params_air.rs` (gate (b)),
`params_room.rs` (Sabine, Eyring, gate (f)), `params_complete.rs` (complete series, lost
particles), `params_floor.rs` (the solver's floor), `params_noise.rs` (Monte-Carlo noise) and
`params_noise_calibration.rs` (its calibration against SPPS's own seeds),
`params_arrival.rs` (the direct sound's spread, an arrival outside the onset bin),
`params_reference.rs` (the reference M8 compares against), `params_upstream_gui.rs` (upstream's
GUI reproduced on tutorial 1, and the steps from its method to ours), and, pre-M8,
`params_lambert.rs` (the diffuse transport and `γ²`) and `params_kuttruff.rs` (Kuttruff's
reference).

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
| H. Kuttruff, *Room Acoustics* | The `γ²` correction of Eyring's formula ("Kuttruff's reference") | **No.** Taken as U. M. Stephenson gives it citing the book (next row) |
| U. M. Stephenson, ICA 2016, paper ICA2016-556 | Kuttruff's `α'' = α'·(1 − γ²·α'/2)` and `γ²`'s definition, eq. (21) | **Yes**, the congress proceedings' PDF (`ica2016.org.ar/ica2016proceedings/ica2016/ICA2016-0556.pdf`, sha256 `ad19678e…`); and its longer version, "Different assumptions - different reverberation formulae" (`arauacustica.com/files/publicaciones/pdf_esp_69.pdf`, sha256 `e9a70206…`), section 6.3, for "γ² … in the order of 0.4…0.6" |
| D. H. Bailey, J. M. Borwein, R. E. Crandall, "Advances in the theory of box integrals", Math. Comp. 79 (2010) 1839-1866 | The closed form of `Δ₃(−2)`, Table 6, and its integral, eq. (69): the cube's exact `γ²` | **Yes**, the authors' copy (`davidhbailey.com/dhbpapers/BoxII.pdf`, sha256 `37cfe469…`), Table 6 rendered and read |
| L. A. Santaló, *Integral Geometry and Geometric Probability* (1976) | The chord power integrals behind `⟨ℓ²⟩`: twice the integral of `r⁻²` over pairs of points in the room, over `π·S` | **No.** The relation was derived here from the Blaschke–Petkantschin formula and Cauchy's; the test checks it on the cube against Bailey, Borwein and Crandall |
| Bies and Hansen, *Engineering Noise Control*, 4th ed., eq. 7.64 | A fit of `γ` for rectangular rooms, and the air term folded into `ᾱ` | **No.** As quoted by Arup's Strutt help (`strutt.arup.com/help/Building_Acoustics/RTInsert.htm`), which was read |
| R. Neubauer, B. Kostek, "Prediction of the Reverberation Time in Rectangular Rooms with Non-Uniformly Distributed Sound Absorption" | That "Kuttruff's correction" also names his correction for unevenly spread absorption, eq. (15)-(18), which is not the one used | **Yes** (`sound.eti.pg.gda.pl/papers/prediction_of_reverberation_time.pdf`, sha256 `5aef45a3…`) |

**The receipts** are in `target/investigate/m7-standards-sources/` of the main checkout
(`B:\repos\I-Simpa_Night_Mode`), copied there on 2026-09-24 from the M7 agents' scratch folders and
kept on Burhan's word ("do not delete; move only with Burhan's permission", its `README.txt`). They
are not committed: the standards' PDFs are copyrighted.
- `m7-params-scratch/`: the ISO 9613-1 preview PDFs (`iso9613-1_sample_en.pdf` and its French and
  SIST versions), GOST 31295.1-2005 (`gost31295-1-2005.pdf`) with renders of its 20 °C and 15 °C
  table pages (`gost_p18_20C*.png`, `gost_p17_15C.png`) and of ISO's own page 8 at 15 °C and
  10 °C (`iso_p8_15C.png`, `iso_p8_10C.png`), `iso9613_check.py`; for DIN 18041 the BNB 2020
  criterion (`bnb_ln2020_314.pdf`), Nocke's article (`laermbekaempfung_2016.pdf`), the DEGA
  journal (`dega_aj_2019_03.pdf`), Ruhe's and fennext's pages, and the standard's preview from
  normenportal (`din18041_normenportal.pdf`);
- `m7-fix-params-scratch/`: the measurement of the DIN figure's line ends, `din_figure.py`
  (sha256 `42eefdc4…`), on Nocke's Bild 2 extracted from the article (`laerm_bild2.png`, stored
  flipped; `laerm_bild2_upright.png`), its text pages (`laerm_p1..6.txt`), and renders of ISO's
  preview pages 7 and 8 (`iso_pdf7_*.png`, `iso_pdf8_*.png`).
- Pre-M8, in the main checkout's `target/agents/pm8-reference-scratch/sources/`: Stephenson's two
  papers, Bailey, Borwein and Crandall's paper with its Table 6 rendered (`boxII_table6_p34.png`),
  Neubauer and Kostek's paper, and `iur_quadrature.py` (numpy and mpmath: the closed form to 30
  digits, the box quadrature at 100 to 800 nodes, and the exact `γ²` of other proportions).

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
  - `Arrival::Known { time_s, half_width_s }`: the time is given, such as the source–receiver
    distance over the speed of sound, with the direct sound spread over `t_a ± half_width_s`
    (a receiver ball of radius `R` crossed at `c`: `R/c`; 0 for an impulse, `Arrival::at`; see
    "The direct sound's spread" below). For **C50, C80, D50 and Ts** it must lie in the onset
    bin, or those four are refused as `params_bad_arrival`: before the bin, the direct sound is
    more than 20 dB below the strongest arrival and its time does not place the onset; after it,
    energy within 20 dB of the maximum came before the direct sound's centre. A time a rounding
    step (10⁻⁹ dt) before the bin is taken as its start. A half-width that is not a finite time
    of at least 0 is refused the same way.
  - **SPL, EDT, T20 and T30 are never refused `params_bad_arrival`** (M7 follow-up; they had
    been, T30 included, although decay times do not depend on where time starts). SPL does not
    use the arrival. The decay times depend only on where in the histogram the direct sound is
    taken to be, and `BandParameters::decay_arrival` says what they were measured from: the given
    arrival when it fits the onset bin, or when it follows the onset bin and the onset bin reaches
    into its spread (`(k₀+1)·dt > t_a − half_width_s`: the onset bin holds the leading edge of the
    direct sound, which a receiver ball starts to catch `R/c` before its centre); otherwise
    (before the onset bin, after it by more than the spread, or not a time) as if no arrival were
    given, `Arrival::Detected`, **where they can still be refused `unresolved`** when the two ends
    of the onset bin give values further apart than their limit. No such receiver was seen on
    tutorial 1 (`docs/results.md`, "The arrival": none had `r/c` before the onset bin).
  - `Arrival::Detected`: not given. Every onset-relative parameter is computed with the arrival
    at **both ends of the onset bin**. The value reported is the mean of the two; when either end
    lies further from it than the parameter's limit (the truncation table below), the parameter is
    refused as `params_not_evaluable` (`unresolved`), with both ends in the detail.
- **Why the decay times do not fall back to `Detected` when the direct sound's leading edge is
  the onset bin**, measured (`params_arrival.rs::decay_times_taken_as_if_no_arrival_were_given_miss_where_only_the_cap_is_in_the_onset_bin`):
  the onset bin then holds only the ball's cap, and the direct sound lies smeared across the next
  bin. From the two ends of the onset bin EDT came out up to 0.62 % off and was accepted in 3 of
  the 18 cases tried (`dt` 10 and 1 ms, `T` 0.3 to 3 s, `D/R` 0.12 to 1); from the arrival and its
  spread every one is exact.
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
  arrival: piece B passes `r/c`, with the spread `R/c`.** Which bin the leading edge `(r − R)/c`
  lands in is measured in `docs/results.md`, "The arrival".
- **Gate (a)'s six decays with the arrival detected** (M7 follow-ups; the M7 critic: the gate had
  been run with the true arrival given only; `params_synthetic.rs::gate_a_decays_with_the_arrival_detected`):
  EDT, T20 and T30 meet the gate's 0.5 % in all six; C50, C80 and D50 are refused `unresolved` in
  all six, the closed form (the bin's start, where the decay begins) one end of the bracket; Ts
  comes through only at `T` = 3 s, `dt` = 1 ms, and meets its bound there. No value outside its
  bound is returned. Gate (a) as the plan words it, C80 and D50 included, therefore holds with the
  arrival given, not detected. Says no: the same decays 1 % off, detected, miss the decay times'
  bound in all six.

### The direct sound's spread

Added by the M7 follow-ups. SPPS adds a particle's energy times the length of its path inside the
receiver ball to the time step in which it travelled that path
(`spps/input_output/reportmanager.cpp:198-225`). The direct sound of a source `r` away therefore
reaches the receiver over `[(r − R)/c, (r + R)/c]`, with the density of a plane front sweeping a
ball, `R² − (ct − r)²`: 1.8 ms at upstream's `R` = 0.31 m.

**What went wrong.** The curve took the direct sound as an impulse at `r/c` inside one bin. When
its spread straddles a bin edge, the next bin holds part of it, which the curve read as decay.
Measured on that direct sound plus an exponential from `r/c` (`params_arrival.rs`, 720 cases:
`dt` 10 and 1 ms, `T` 0.3, 1 and 3 s, `D/R` 0.12, 1 and 3, `r/c` at 40 places in its bin), taken as
an impulse: **EDT up to 26.3 % off and Ts up to 21.5 %, T20 2.0 %, T30 0.85 %, C50 0.016 dB, D50
0.08 points**, all returned as numbers.

**Now.** Given the spread, the curve is the histogram's own from the first bin wholly after the
direct sound, `⌈(t_a + h)/dt⌉` (at least the bin after the arrival's); between the arrival and that
bin it is that bin's decay continued back, and the rest, up to the top, is the direct sound at
`u = 0`. The top is the sum from the onset bin, or from the bin the direct sound starts in,
`⌊(t_a − h)/dt⌋`, when that is earlier: a spread direct sound's leading edge is weak and can fall
below the 20 dB rule, but it is the direct sound all the same. With `h = 0` this is the model
above, unchanged. On the same 720 cases every quantity is exact to 3·10⁻¹³, except EDT where the
direct sound's step leaves under 2 bins of its 10 dB range (`D/R` = 3, 6 dB, at `T` = 0.3 s:
`range_too_short`, 40 cases), and C50, C80, D50 and Ts where `r/c` lies after the onset bin
(`params_bad_arrival`, 324 cases). Says no: the same series with `h = 0`, above.

**What it assumes:** the reverberation continued back to the arrival as the first bin after the
direct sound decays. The second review found this wrong for real runs, and it was: see "The early
reverberation" below. (The earlier text here said M8's bed could not show it, EDT not being
gated. Since the M8 design decision 2 of 2026-09-25 00:20, M8 gates EDT at `dt` 1 ms, with T30, so
its bed tests this reading too.)
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
explain is taken as the direct sound, at `t_a`. A direct sound spread over two bins is handled
with its spread (above). The reverberation's own start is bounded for a solver that declares it
unresolved (next).

### The early reverberation

Added by the M7 follow-ups after their second review (`EnergySeries::with_early_reverberation_
unresolved`; `tests/params_early.rs`). SPPS's reverberation does not start at the direct sound:
at 0.2 ms bins the receivers of the 5×4×3 m room at α 0.4 see the direct sound over 1.8 ms, then
nothing for 1.6 ms, then the first reflections building up to a peak about 8 ms after the
arrival, twice the level the decay settles to by 12 to 15 ms. The curve's continued decay puts
reverberation in that gap, and at a step of 10 ms the stretch it covers, up to 11 ms, is a quarter
of EDT's 10 dB range at `T` = 0.22 s. **Measured** (`docs/results.md`, "What M8 needs", EDT
against the time step): EDT from the continued decay at 10 ms read 4.5 to 4.9 % short of EDT at
0.2 ms in that room, on SPPS and on the independent transport of `tests/lambert_box.rs` alike,
and up to 0.5 % short at α 0.2 and 1.8 % at α 0.4 in the 6×10×3 m room; at 1 ms it agrees with
0.2 ms within 0.15 %. Those values were returned as numbers.

**Now**, for a series whose caller declares the early reverberation unresolved, as
`core::results` does for every SPPS series, the curve is read three ways: the reverberation
beginning at the arrival (the continued decay above, exact when the reverberation runs from the
arrival), at the start of the first bin wholly after the direct sound (the curve flat at that
bin's sum from the arrival: everything before it is direct sound), and at that bin's end (its
energy arriving late, as a reverberation building up through it brings it). Each quantity is
reported midway between the lowest and highest reading, and refused `params_not_evaluable`
(`early_unresolved`) when they lie further than its limit from that, with the readings in the
detail. A series that does not declare it (gate (a)'s synthetic decays, which do start at the
arrival) is read the one way, as before.
- **Says no:** on a synthetic direct sound followed by reverberation that starts 3 ms later and
  builds up over 8 ms (`params_early.rs`), the continued decay alone accepts EDT **10.2 %** off
  the truth at 10 ms; read three ways every accepted value is within its limit (EDT 0.20 %, T30
  0.005 %, Ts 0.10 % at worst), EDT is refused in 48 of 72 cases at 10 ms and in none at 1 ms, and
  where refused the truth lies within the readings. Two readings (the arrival and the first bin)
  were not enough: a build-up reaching into that bin put an accepted EDT 1.6 % off.
- **On M8's rooms** (`lambert_box.rs`, 4,000,000 rays per cell): at 10 ms EDT is refused wherever
  the continued decay was off by more than a quarter of a percent, and accepted, within 0.30 % of
  the 0.2 ms value, at α 0.05 in both rooms and α 0.1 in the 6×10×3 m room. T30, C80 and D50 are
  not moved: their windows start after the stretch, or below it.
- **The cost:** EDT at the default step of 10 ms comes out only in rooms whose early decay is slow
  against the step. A step of 1 ms gives it everywhere measured, and then C50, C80, D50 and Ts are
  refused `params_bad_arrival` at most receivers (`docs/results.md`, "The arrival"). Which to
  prefer is M8's and M12's choice.

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
     run's statistics count at most one particle in a million remaining at the end
     (`results::spps::REMAINING_UNFINISHED_SHARE`), those few bounded with the lost ones as
     unfinished paths (`docs/results.md`, "Complete series"). A decay time still needs the curve to reach the bottom of its range before the
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
   energy missing from a series can move ("Missing energy", below). Each is 1/10 of a difference
   limen. **The limens:** ISO 3382-1:2009 Table A.1, as commonly reproduced (not read here), gives
   them for G (1 dB), EDT (5 %), C80 (1 dB), D50 (0.05) and Ts (10 ms) only. T20 and T30 have none
   there; they take EDT's 5 %, and C50 takes C80's 1 dB. Those two are extensions, not the
   standard's:

   | Quantity | Limit | 1/10 limen | Gate (a), on exact decays |
   |---|---|---|---|
   | EDT | 0.5 % relative | 0.5 % (of EDT's 5 %) | 0.5 % |
   | T20, T30 | 0.5 % relative | 0.5 % (of EDT's 5 %, extended) | 0.5 % |
   | C80 | 0.1 dB | 0.1 dB (of 1 dB) | 0.01 dB |
   | C50 | 0.1 dB | 0.1 dB (of C80's 1 dB, extended) | none named; held as C80 |
   | D50 | 0.005 (0.5 points) | 0.005 (of 0.05) | 0.1 points |
   | Ts | 1 ms or 0.5 % of Ts, the tighter | 1 ms (of 10 ms) | none named; the test holds Ts to 0.5 % |
   | SPL | 0.1 dB | 0.1 dB (of the 1 dB given for G) | none (gate (c) is ±0.5 dB) |

   **Confirmed by Burhan, 2026-09-24 14:11:** the strict rule, 1/10 of the limen (0.5 % for decay
   times, 0.1 dB for clarity), to be revisited after M8. **C and D aligned to it on 2026-09-25**
   (the M8 design decision 3 of 00:20, on Burhan's "what would be best for the people using
   this"): from the M7 review until then, C was held to gate (a)'s 0.01 dB and D50 to its 0.1
   points, ten and five times stricter than the rule he confirmed. Gate (a) is a test of exact
   decays, which no unknown moves, and it still holds them to its bounds with the arrival given
   (`params_synthetic.rs`). What the alignment costs: a value refused before because an unknown
   could move it by more than 0.01 dB (C) or 0.1 points (D50) now comes through, known only to
   within the rule's 0.1 dB or 0.5 points. On gate (a)'s decays with the arrival detected, 40 C, D
   and Ts values now come through at 1 ms where 10 did, 16 of them outside gate (a)'s bound and
   inside their limit (`an_arrival_not_given_leaves_c_d_and_ts_unresolved`); what it changes on
   SPPS runs is measured in `docs/results.md`, "What M8 needs".

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
  `docs/results.md`, "Lost particles");
- **or, when the lost share follows the decay** (`EnergySeries::with_lost_share_following_decay`,
  M7 follow-up: SPPS in energetic mode, where a lost particle carries about the mean energy of the
  particles when it was lost), at most `s·S(u)` of the energy from every time `u` on. The true
  curve is then `S(u)·(1 + a(u))` with `a(u)` anywhere in `[0, s]`, so every level moves by at most
  `Δ = 10·lg(1 + s)` dB, and each quantity is held to the most that can move it: a decay time
  fitted over a range the curve covers `R` dB of, `9Δ/R` relative (a least-squares slope over a
  span moves by at most `3Δ` over the span for a perturbation within `±Δ`, and the range's ends,
  moving `Δ` in level, by at most `6Δ` more); C from `r = S(0)/S(te)`, the larger of
  `10·lg((r(1+s) − 1)/(r − 1))` and `10·lg((r − 1)/(r/(1+s) − 1))`; D50 `s·(1 − D50)`; Ts `s·Ts`;
  SPL `Δ`. Beyond its limit: `missing_moves`, with the value moved by that much
  (`tests/params_complete.rs::an_energetic_lost_share_that_follows_the_decay_is_bounded_by_it`:
  `10·4/150,000` lets every quantity through within its limit of the truth, whenever the particles
  were lost; the same share as a lump refuses T30; a share of 3 % refuses T30 and C80).
- **The floor and a share that follows the decay are bounded together** (second review: the first
  version held each to the whole limit alone). Energetic mode has both, and both can be missing at
  once, so what the floor's lump moves a value by (with the tail) and the most the share can move
  it are added, and the sum is held to the limit
  (`params_complete.rs::the_floor_and_a_lost_share_that_follows_the_decay_are_bounded_together`:
  a floor and a share that each move T30 by a little more than half the limit pass alone and are
  refused together; the first version accepted them).

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
room: at tutorial 1's 150,000 particles T30 came out at 0.8 to 2.4 s, and was reported as a
number. What that is to be read against, each value with its source: **Sabine's formula** gives
0.67 s (`V` 180 m³, `S` 216 m², α 0.2 on every face, `K` 0.161, no air), and Eyring's 0.60 s;
neither is the room's time, since tutorial 1's walls are specular and its decay is not a diffuse
field's. **SPPS's own T30, converged**, in that box with its materials and air, mean over six
receivers of each one's mean over ten seeds (`docs/investigations/2026-09-25-noise-calibration/
calibration.json`): 0.98 s at 125 Hz to 0.79 s at 4 kHz in cell C-R6 (random, 1.5 M particles, `dt`
1 ms), 0.95 s to 0.78 s in V4-R2 (random, 6 M) and in V4-E14 (energetic, 1.2 M)
(`params_noise_calibration.rs`, `tutorial_ones_t30_is_spps_converged_not_sabines`). The first
version of this text called 0.67 s "the room's time"; it is Sabine's. `params::noise` estimates each value's Monte-Carlo standard
deviation, calibrates it against SPPS's own seed-to-seed spread, and refuses the value when it is
too large, or when the run lies outside what the calibration measured. Calibrated pre-M8 (Burhan,
2026-09-24 17:45: "never below the observed noise", and a refusal names the particles it needs),
in four pre-registered rounds; every number is in
`docs/investigations/2026-09-25-noise-calibration/`.

- **The model.** SPPS adds, for every particle crossing a receiver sphere of radius `R`, its
  energy times its chord through the sphere (`spps/input_output/reportmanager.cpp:223-224`),
  and writes the sum times `ρc/V` (`baseReportManager.cpp:183`). In random mode a particle keeps
  its start energy `W/N` (`sppsNantes.cpp:73`) until it is absorbed whole, so one crossing adds
  `W/N · ℓ · ρc/V`. A uniform beam crossing a sphere gives chords of density `ℓ/(2R²)` on
  `[0, 2R]`: mean `4R/3`, `E[ℓ²]/E[ℓ]² = 9/8`. The mean deposit is `d̄ = W·ρc/(N·πR²)`; a bin
  holding `E` holds about `E/d̄` crossings; counted as independent Poisson crossings, its variance
  is `(9/8)·d̄·E`. In energetic mode a particle's energy only falls from `W/N`, so the same `d̄`
  bounds each deposit from above. **Neither bounds the variance**: one particle crosses a receiver
  at several times, and its crossings share its lifetime (random mode) or its energy (energetic
  mode), so SPPS's spread can exceed the structure in either mode; only the calibration says by
  how much, and only where it was measured. (A deposit scaled step by step by the particles' mean
  energy, from the room table, was measured as energetic mode's other structure; calibrated, it
  overstated more, and is not used.)
- **The roughness structure** (round 4, `noise::Structure::Roughness`): for energetic T20 and T30
  outside uniform Lambert rooms, each bin's deposit is read from the series itself
  (`noise::roughness_deposits`). Late in an energetic decay a crossing brings far less than `d̄`,
  by how much depending on how far the particles' energies have spread apart; SPPS does not write
  that spread, but it shows as the series' roughness. Over blocks of at least one receiver
  crossing (`2R/c`, since SPPS splits a crossing over the steps it spans), each block's departure
  from its neighbours' geometric mean, `r = B_j/√(B_{j−1}·B_{j+1}) − 1`, over the blocks within six
  of it, gives its relative variance `mean(r²)/1.5` and so a deposit `mean(r²)/1.5·B_j/(9/8)`, at
  most `d̄`. The roughness also holds the true curve's fine structure (specular echoes), so it
  reads high where that is strong: on tutorial 1 at 150,000 particles T30 is overstated 2.4 times,
  against 14 times under M7's structure, and at 1,500,000 5.6 times.
- **The estimate.** A parametric bootstrap (`noise::bootstrap`): 200 series drawn from the model
  around the series, each bin a compound Poisson sum of `E/d̄` expected crossings with chord
  deposits (a normal draw of the same mean and variance above 30), each evaluated as it is
  (complete, nothing missing: the tail, the floor and lost particles are judged once, on the
  series); the standard deviation over them is the model's. The seed is fixed, so a series gives
  the same estimate every time.
- **The calibration** (`noise::calibration`, round 3, and round 4 for energetic T20 and T30
  outside uniform Lambert rooms): the model's standard deviation times
  `k·√(1 + κ·n)`, per computation method and quantity. `n` is the run's own crossings of the
  receiver per particle, `n1 = Σ E/(d̄_least·N)` (`noise::crossings_per_particle`), times the
  spread of the particles' lifetimes, `Var L/(E L)²` from the band's room table
  (`noise::lifetime_cv2`; 1 for an exponential decay, more for a double slope): the correction for a
  particle's crossings of one receiver at several times, which grow with the receiver's size
  (`n ∝ R²`) and a room's reverberance. `k` is the largest one-sided 95 % upper bound over the
  calibration cells of the ratio of SPPS's seed-to-seed spread to `√(1 + κ·n)` times the model
  (ten seeds a cell, on effective degrees of freedom), `κ` the one of 0 to 3 (0 to 6 in round 4)
  that overstates least. Energetic T20 and T30 take theirs by kind of band: every face Lambert with
  scattering 1 and the same absorption, up to a mean absorption of 0.2 ("uniform Lambert"; 0.4 in
  round 3, lowered by F6), under M7's structure; every other band under the roughness structure. Pre-registered;
  round 3 calibrated on 51 cells and validated on 19 no rule saw, round 4 calibrated on all 70 and
  validated on 24 new ones (`docs/investigations/2026-09-25-noise-calibration/`):

  | Method | SPL | EDT | T20 | T30 | C50 | C80 | D50 | Ts |
  |---|---|---|---|---|---|---|---|---|
  | random: k (κ) | 1.2 (1.75) | 1.3 (2) | 1.5 (2.75) | 1.6 (3) | 1.2 (1.25) | 1.4 (1) | 1.2 (1.25) | 1.4 (3) |
  | energetic: k (κ) | 1.1 (0) | 0.86 (0) | uniform Lambert 0.098 (0), up to ᾱ 0.2; other bands 1.4 (5.25), roughness | uniform Lambert 0.052 (0), up to ᾱ 0.2; other bands 1.3 (4.75), roughness | 0.96 (0.25) | 0.86 (0.5) | 0.96 (0.25) | 0.84 (0.5) |

  - **Random mode**: the correction is large. With receivers of 0.9 m in a 60 m³ room at α 0.05
    (up to 2.2 crossings per particle), the seeds' spread was 1.7 (SPL) to 2.6 (T30) times the
    bootstrap's; with SPPS's default 0.31 m it is 1.0 to 1.4 times, highest with specular walls
    and the absorption on one surface.
  - **Energetic mode**: in uniform Lambert rooms every particle meets the same absorption at every
    reflection, so their energies spread apart only as their reflection counts do, and T30's
    spread is 0.026 to 0.048 of M7's structure (13 cells): factor 0.052. Elsewhere (specular,
    partly scattering, or Lambert with the absorption concentrated) the particles' energies spread
    apart far more, from 0.048 to 0.63 of M7's structure for T30 with nothing in the run's
    configuration to tell which, so round 3 kept factor 1 there; round 4's roughness structure
    reads it from the series (geometric-mean overstatement 2.0 for T30, 1.8 for T20 over 23 and 24
    calibration cells). Its correction for repeated crossings is large (κ 4.75 and 5.25), set by a
    room with receivers of 1.2 m. The uniform-Lambert factors failed two held-out long rooms at
    ᾱ 0.4 (T30 2.88 times the prediction in a 30 × 4 × 3 m corridor), so they hold only up to ᾱ
    0.2 (F6), and above it the roughness decides; there T20's fitted 1.2 claimed less noise than a
    5 × 4 × 3 m box at 0.4 showed, so it ships 1.4. SPL is above 1 because the direct field alone
    is among the cells: with every particle stopped at its first surface, energetic mode runs
    random mode's transport.
  - Checked in the suite on the committed receipt (`tests/params_noise_calibration.rs`): every
    number of the code is what the rules give; every validation cell passes; halved factors (a
    model twice too optimistic, through a fault seam) fail in every validation cell; a quantity's
    spread raised above its prediction fails its check alone; without the correction the cells
    with large receivers fail.
- **The domain.** A calibration holds where it was measured. Each quantity (and kind of band) has
  the fewest particles per source and the most crossings per particle its calibration saw with at
  least one band's six receivers: random SPL, C50, C80 and D50 from 5,000 particles, the decay
  times and Ts from 50,000; energetic likewise, with T20 outside uniform Lambert rooms from 15,000
  and T30 there from 150,000; `n` up to 2.16 (random) and 2.13 (energetic), and 1.37 for energetic
  T20 and T30 outside uniform Lambert rooms (and uniform ones above ᾱ 0.2). **A value outside is
  refused, `noise_uncalibrated`**, with the particles to run to reach the domain
  (`particles_at_least`), or, since more particles do not lower `n`, the most the receiver radius
  may be as a multiple of the run's (`receiver_radius_scale_at_most`, `√(max/n)`). What the code
  cannot check is stated beside every report's calibration (`monte_carlo.measured_on`): box rooms
  of 60 to 1,000 m³; no transmission, no fitting zones, no coupled volumes; steps of 1 and 10 ms.
- **The refusal:** `monte_carlo_noise`, when the calibrated standard deviation is above the limit
  or more than 10 of the 200 resamples refuse the quantity themselves. The limit is half the
  limen: twice the standard deviation, about a 95 % interval, stays within one limen.

  | Quantity | Largest standard deviation |
  |---|---|
  | EDT, T20, T30 | 2.5 % relative (half of EDT's 5 %; T20 and T30 extended) |
  | C50, C80 | 0.5 dB |
  | D50 | 0.025 |
  | Ts | 5 ms |
  | SPL | 0.5 dB |

  M8's bed asks more of three seeds (a spread of at most 2 %); this limit is what one run may
  show, not the bed's. The resamples carry the model's structure, not its calibration: in
  energetic mode, where M7's structure is 20 to 38 times T30's real noise in uniform Lambert rooms,
  more than 10 resamples can refuse T30 (α 0.2 to 0.4 below about a million particles) although
  its calibrated noise is within the limit; the value is then refused naming the count at which
  the resamples clear (below).
- **The particle count a refusal names** (`particle_count`): SPPS's spread falls as `1/√N` or
  faster on every one of twelve pairs of cells that differ only in `N` (5,000 against 50,000 up to
  150,000 against 15,000,000; no pair showed it falling slower by more than two standard errors,
  the one direction in which a count named from it would be too low), so the count at which the
  calibrated standard deviation is the limit over a margin is `N·(margin·sd/limit)²`, rounded up
  to two significant digits. The margin covers the scatter of one run's own estimate: about 5 %
  for most quantities, but up to about 40 % for random-mode T20 and T30 (median over a cell's
  receiver-bands), so margins of 1.5 there; 1.2 for random EDT, C50, C80 and D50 and energetic EDT;
  1.2 or 1.3 for energetic T20 and T30; 1.1 elsewhere. On the pairs, the
  refusals whose named count the higher count reached gave their value there in 98.2 % to 100 % of
  its seeds (round 3). **A value its resamples refuse** (more than 10 of 200) names the first of 2,
  4, 8, 16, 32 and 64 times the run's particles at which the model's own resamples of the series,
  every deposit over that multiple, refuse it at most 5 times and its calibrated standard
  deviation times the margin is within the limit (round 4, R4-3; `particle_count`
  `{"count": "resampled"}`); when none does, no count is named (`beyond_resampled`: more particles
  may not help). On the pairs these counts gave the value at the higher count in 97 to 100 % of its
  seeds for T30, but for energetic C50, C80, D50 and Ts in 64.7 % (a receiver whose arrival sits at
  a 1 ms bin's edge keeps refusing them), so those name none (`resampled_not_confirmed`, F8).
  **Every count is named from M7's structure**, whose deposits fall exactly as `1/N`: for the
  roughness structure, which also holds the curve's fine structure and does not fall as `1/N`, that
  is an upper bound (counts named from the roughness itself gave the value in only 36 to 44 % of
  the higher count's seeds).
- **Several sources:** the largest of their mean deposits, an upper bound, and crossings per
  particle counted at the smallest, at least each source's own. **A directivity balloon** scales
  each particle's energy by its direction, so `d̄` is not known: every value is refused,
  `noise_unknown`.
- **Checked against the model's own runs** (`tests/params_noise.rs`, its own generator): the
  bootstrap's estimate is the spread of 60 independent runs within a factor 1.4 at 40,000 and
  400,000 crossings; the spread falls as `1/√N` from 10,000 to 160,000 crossings (400 runs each,
  within three standard errors); at 4,000 crossings, tutorial 1's count at 150,000 particles, 5 of
  10 runs give a T30 more than 5 % off from the series alone, and every one is refused. At 4,000
  crossings the estimate of T30's spread runs 11 % low on average and scatters 35 % from run to
  run (`the_bootstrap_against_the_true_spread_at_low_counts`, run on purpose); the calibration's
  cells at low counts carry that, and below the domain the value is refused.
- **Selection by the refusal.** A seed whose decay reads long also has the larger spread of its
  own, so where some seeds of a receiver-band are refused for their noise and others pass, the
  passing mean sits low: random T30 −0.82 % ± 0.13 %, T20 −0.47 %, EDT −0.27 %, energetic T30
  −1.23 % and T20 −0.75 % over the receipt's cells (round 4's model). A mean over the values that came through is not a mean over runs; M8 must not take one
  (`docs/results.md`, "What M8 needs").
- **The M7 review's check** (`crates/simpa/tests/cli_results.rs`,
  `noise_estimate_against_the_spread_of_twenty_seeds`, run on purpose; tutorial 1, seeds 1 to 20):
  the uncalibrated model matched the spread at 150,000 particles (pooled 0.95 to 1.05) and ran
  15 to 21 % low at 1,500,000 for EDT, T20 and Ts. The calibration measured the same thing over
  70 cells, and the factors now cover it.

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
  `simpa results --json` carries it per band, per source and for each aggregate (M7 follow-ups; the
  M7 critic found it computed and dropped): `curvature.percent` from the reported T20 and T30 with
  its Monte-Carlo standard deviation over the resamples that give both (no limit of its own: its
  noise follows from theirs, and both passed theirs), refused with T30's refusal or else T20's
  when either is refused; `curvature.curved`, `null` when it is refused. Checked on a noisy decay
  (`params_noise.rs`): a single slope reads 0.3 ± 1.0 %, a decay twice as slow below −20 dB
  30.9 ± 0.2 % and is flagged.

### The decay curve, for display

`params::decay::decay_curve` gives the Schroeder curve EDT, T20 and T30 are fitted to, the same
curve from the same code (M7 follow-ups; the M7 critic: M12's decay charts would otherwise need a
second implementation), as `[u, level]` points: `u` in seconds from the arrival the decay times
are measured from, the level in dB re the curve at `u = 0`, which includes the direct sound.
- **The knots** are `[0, 0]`, the level just after the direct sound (the curve steps down by it at
  the arrival), and the end of every bin up to the start of the last bin with energy, where the
  curve falls to nothing and the fits leave it out. Between knots the curve is straight in dB.
- **Thinned** for the JSON: a knot is left out when the straight line through the points kept
  either side of it passes within 0.01 dB of it and of every other knot left out between them (the
  slopes each knot allows, intersected as the line is extended; one pass). Straight in dB between
  knots, the curve is then everywhere within 0.01 dB of the thinned line. An exponential decay is
  one line from the direct sound's step down to where the series' end bends the backward sums (in
  the test, below −150 dB); a ragged random-mode curve keeps most of its knots.
- **Before the first bin wholly after the direct sound** (`histogram_from_s`) the curve is the
  model's reading, the decay of that bin continued back to the arrival; from there on every knot is
  the histogram's own backward sum. When the early reverberation is unresolved, the values are
  read two more ways over that stretch ("The early reverberation"), and **the value reported is
  midway between the lowest and highest of the three readings, while the curve shown is this one
  reading**: a line drawn on the curve from 0 to −10 dB then gives an EDT that is not the one
  displayed, off by up to EDT's limit, 0.5 % (the M7 follow-ups' critic). SPPS's histograms are
  marked so in every run tried, since the direct sound lies inside a bin. M12 must say which of
  the two a chart shows. With the arrival detected, it is read from the start of the onset bin.
- **Tested** (`decay.rs`, `the_decay_curve_is_the_fitted_curve_thinned_within_its_tolerance`): the
  step is the direct sound's, the line's slope gives EDT to 10⁻³, every knot of a double-slope decay
  lies within 0.01 dB of the thinned line and the knee is kept; says no: a kept point moved by
  twice the tolerance leaves knots off the line.
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
  - Its `t₀` is the absolute-threshold label above: the end of the bin before the first bin whose
    energy differs from the first bin's by the threshold, in Pa², so the start of that bin.
  - **Its Ts is not measured from the onset.** Line 432 weights by the absolute time label, so
    upstream's Ts includes the propagation delay `r/c` and half a bin more.

### Upstream's GUI reproduced on tutorial 1 (measured)

The M7 critic: on the same 2019 `.recp`, upstream's stored `Acoustic parameters.gabe` gives C80,
D50 and Ts that differ from ours (5.55 against 6.57 dB and 65.9 against 67.6 % at 250 Hz), and
nothing showed that the method explains all of it. `tests/params_upstream_gui.rs` ports
`OnMenuDoAcousticParametersComputation` and what it calls line by line, in `f32` as upstream
computes, and runs it on the `.recp` stored in `tutorial_1.proj` (Receiver 1; Receiver 2's folder
holds no stored table):
- **It reproduces the stored table**, all 216 values (27 bands × SPL, C50, C80, D50, Ts, RT-15,
  RT-30, EDT): 195 bit for bit, 37 of them NaN where the table holds NaN; the other 21 within 2
  units in the last place of `f32` (6·10⁻⁶ relative for a decay time, whose regression carries it),
  the GUI's C runtime rounding `log10f` differently from Rust's. Upstream's stored Schroeder curves
  agree with the port's to 7.6·10⁻⁶ dB.
- **Only with the 2019 GUI's threshold.** The table was written by a GUI whose `GetTimeDecay` took
  `EPSILON`, `(decimal)0.000001` (`lib_interface/Core/mathlib.h:56`), as its threshold; upstream's
  commit `f50c36febd` (2020-12-04, "about issue #7 set epsilon value as low as possible in order to
  not skip first sound wave") replaced it with `pow(10, -180.0f/10.0f)`, 10⁻¹⁸, the pinned
  commit's. The file at `e9da8b3f12`, the last change before (fetched from GitHub; committed as
  `docs/investigations/2026-09-24-m8-evidence/gui-2019/projet_calculation_e9da8b3f12.cpp`,
  sha256 `0f2529aa…`),
  differs from `929a5c8`'s in that threshold and in translation macros only. At 10⁻⁶ Pa², an
  absolute level, no bin of the 50 to 125 Hz bands (levels 36 to 40 dB) ever differs that much from
  the first: `t₀` becomes the last label, and C, D and Ts come out 0/0, the stored NaN. The pinned
  GUI gives numbers there, and the same as 2019's in every other band (`t₀` 10 ms in both).
- **From upstream's method to ours, one step at a time** (22 bands where the stored C80 is a
  number; mean, and the range over the bands):

  | Step | C80 | D50 | Ts |
  |---|---|---|---|
  | the 2019 GUI to the pinned one | 0 | 0 | 0 |
  | the boundary bin counted once | +1.05 dB (+0.71 to +1.96) | (counted once already) | |
  | time zero at `r/c` = 13.03 ms instead of the 10 ms label, the edge's bin split in proportion | +0.32 dB (+0.22 to +1.22) | +1.51 points (+0.54 to +1.83) | −13.03 ms (upstream weights by the absolute time) and −5.00 ms (its labels are the bins' ends) |
  | ours: the curve inside a bin (`params::decay`) | +0.06 dB (+0.01 to +0.94) | +0.11 points (+0.07 to +0.17) | −0.17 ms (−0.60 to −0.09) |

  The last step is the model inside the bin the window's edge falls in: the test holds our C80
  and D50, and the proportional split's, between that bin counted wholly early and wholly late,
  in every band. Its largest, 0.94 dB, is at 20 kHz, where the decay is fastest. **So the gap is
  the method**: upstream counting C's boundary bin twice (the largest part), its time zero at a bin
  edge by an absolute threshold rather than at the direct sound, its Ts from the source's emission
  on bin-end labels; no defect of ours was found. Says no: the pinned GUI's threshold does not give
  the stored NaN, and C with the boundary bin counted once is at least 0.5 dB off the stored C80,
  in every band.

## Sound pressure level

`SPL = 10·lg( Σ_k B_k / p₀² )`, with `p₀² = (20 µPa)² = 4·10⁻¹⁰ Pa²`, over every bin of the band.
It is the steady-state level of a source emitting its power continuously. This is upstream's
`dB_Sum_Param` (`projet_calculation.cpp:220-240`, with `p_0` at line 41) and its receiver view
(`e_report_gabe_recp.cpp:56, 145`). It does not depend on the arrival.

**Night Mode's +26 dB bug:** its `.gap` reader divided the energy by `10⁻¹²`, the intensity
reference, instead of multiplying by `1/p₀²` (`main:project/result_parser.cpp:486`).
`10·lg(4·10⁻¹⁰ / 10⁻¹²) = 26.02 dB`. The SPL function takes no reference argument, so the mistake
cannot be made through it; gate (c) checks the level end to end. Its say-NO puts the mistake into
this code path (M7 follow-ups): a test-only seam replaces `p₀²` by 10⁻¹², and by `p₀²` 1 dB off
either way, and the level box's report computed again with it misses the gate in all 12
receiver-bands (`docs/results.md`, "Level calibration").

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

### The reference M8 compares against (for Burhan's decision)

**Decided since** (Burhan, 2026-09-24 23:14): Kuttruff's corrected Eyring with `γ²` from the
geometry, 5 %, with the transport as a cross-check and plain Eyring reported only; Michael to
ratify. It is core code now: "Kuttruff's reference", below. What follows is the M7 follow-ups'
evidence as they wrote it.

Added by the M7 follow-ups; nothing is decided here. `T_Eyring = K·V/(A + 4mV)` has two open
choices, computed on tutorial 1's box (floor 60 m² at α 0.1, ceiling 60 m² at 0.3, walls 96 m² at
0.2, 180 m³; 20 °C, 50 %, 101.325 kPa) in every third-octave band from 50 Hz to 8 kHz
(`tests/params_reference.rs`):

| Choice | What it is | Moves T_Eyring against TCR's |
|---|---|---|
| `K` = 0.163 (TCR) | TCR's rounded constant (`TC_CalculationCore.cpp:138`) | 0 |
| `K` = `24·ln(10)/c` with SPPS's `c` | 0.16102 at SPPS's `c` = 343.2 m/s at 20 °C (`Celerite_du_son.cpp:46`): the decay a diffuse field has when its particles move at `c` | **−1.215 % in every band**, a quarter of M8's 5 % |
| `m` at the nominal band frequency, upstream's form | what SPPS and TCR apply (`base_core_configuration.cpp:114`; `air::solver_air_absorption_per_m`) | 0 |
| `m` at ISO 9613-1's exact midband frequency | the standard's table (`air::attenuation_db_per_m`) | **at most 0.36 %** on tutorial 1 (8 kHz, where air is a quarter of the absorption); under 0.1 % in every band below 8 kHz. **In M8's rooms more**, since air is a larger share where the surfaces absorb less: up to **+0.83 %** at 8 kHz (6×10×3 m, α 0.05), +0.74 % (5×4×3 m, α 0.05), +0.50 % (5×4×3 m, α 0.1), under 0.4 % from α 0.2 (`what_the_air_term_moves_in_m8s_rooms`; second review) |

Both together stay within 1.3 % of TCR's value in every band on tutorial 1. At 80 kPa, where
upstream's humidity lacks the pressure factor, the air term's form alone moves the 8 kHz band by
6.6 % (the same test, as the check's say-no).

**A third choice the M8 cells showed: Eyring itself** (second review, `docs/results.md`, "What M8
needs"). Eyring's formula takes every free path to be the mean, `4V/S`. With Lambert reflection
the free paths spread about it, relative variance `γ²`, and the decay is slower than Eyring's the
more the surfaces absorb (Kuttruff's correction `A = −S·ln(1 − α)·(1 + (γ²/2)·ln(1 − α))`; when
this was written it was taken as commonly stated, unread; it has since been read as Stephenson
gives it citing Kuttruff, "Kuttruff's reference", below). A transport written from scratch for these tests (`tests/
lambert_box.rs`: straight rays, Lambert reflection, `(1 − α)` per reflection, SPPS's receiver
balls; nothing of SPPS) measures `γ²` = 0.388 in the 6×10×3 m box and 0.352 in the 5×4×3 m one,
the mean free path equal to `4V/S` within 0.3 %, and T30 above Eyring by +1.2, +2.4, +5.0 and
+10.8 % (6×10×3 m) and +1.1, +2.2, +4.5 and +9.3 % (5×4×3 m) at α 0.05, 0.1, 0.2 and 0.4. SPPS
gives the same within 0.3 %. So a 5 % tolerance against Eyring fails the α 0.4 cells and grazes
α 0.2 for physics, not for SPPS. Kuttruff's formula with the measured `γ²` comes within 0.6 % of
the transport's T (−0.5 % at α 0.2 in the 6×10×3 m room, +0.6 % at α 0.4 in the 5×4×3 m one). The
options, for Burhan: Eyring and a tolerance or α set that allows for it; Kuttruff with a `γ²`
computed apart from SPPS for each room (within 0.6 % here); or the independent transport itself as
the reference (within its own noise, 0.02 % at 4,000,000 rays). **Decided by Burhan on 2026-09-24
23:14:** Kuttruff's with `γ²` computed from the geometry by the transport, never fitted to SPPS,
5 %, with the transport as the tight cross-check and plain Eyring reported only; Michael ratifies
the gate text ("Kuttruff's reference", below).

**Recommendation, not a decision.** For SPPS's T30: `K = 24·ln(10)/c` with SPPS's own `c`, and `m`
as the solver applies it (nominal frequency, upstream's form). M8 asks whether SPPS's transport
reproduces the diffuse-field decay of the room it was given; that room's air is the `m` the solver
used, and its speed is the `c` the solver moved at. TCR's 0.163 would bias the reference 1.2 % long
before any solver is judged. For TCR against the analytic value (M8's 0.5 % check), TCR's own
0.163 and the solver's `m`, as gate M7(d) already does: that checks TCR computes what its formula
says. Whether upstream's air term is right (the pressure factor, the nominal frequency) is gate
M7(b)'s question and the second table's, and at sea level it moves T_Eyring by at most 0.36 % up
to 8 kHz on tutorial 1 and 0.83 % in M8's rooms. On the third choice this document recommended
nothing: the numbers are above, and Burhan's decision after them.

## Kuttruff's reference

**Decided** (Burhan, 2026-09-24 23:14, his choice verbatim: "Kuttruff + cross-check
(Recommended)"): M8 gates SPPS's T30 against Kuttruff's corrected Eyring with `γ²` computed from
the room's geometry, never fitted to SPPS, at 5 %; the independent diffuse transport is the
cross-check; plain Eyring is reported only. It changes the plan's M8 gate text, so **Michael
ratifies** it (BuSha note). This section is the pre-M8 piece that makes it core code:
`params::room::kuttruff_rt`, `params::lambert` (the transport) and `results::reference`.
**Nothing here is validated**, and no number of it reaches a user before M8 passes.

### The formula, and where it comes from

```
A_K = −S·ln(1 − ᾱ)·[1 + (γ²/2)·ln(1 − ᾱ)],   ᾱ = Σ Sᵢ·αᵢ / S
T   = K·V / (4·m·V + A_K),                   K = 24·ln 10 / c
```

- **The wall term** is Kuttruff's, as U. M. Stephenson gives it citing H. Kuttruff, *Room
  Acoustics* (Elsevier, Barking; he names no edition): `α'' = α'·(1 − γ²·α'/2)`, `α' = −ln(1 − α)`,
  `γ² = (⟨ℓ²⟩ − ⟨ℓ⟩²)/⟨ℓ⟩²`, the relative variance of the free path lengths ("A rigorous definition
  of the term 'diffuse sound field' and a discussion of different reverberation formulae", ICA 2016,
  paper ICA2016-556, eq. (21); read, from the congress proceedings, sha256 `ad19678e…`). Substituting
  `α'` gives the factor above. **Kuttruff's book itself was not opened.**
- **The air term** is not in Stephenson's eq. (21). It is added outside the wall term, as TCR adds
  it to Eyring's (`TC_CalculationCore.cpp:138`): air multiplies every path's energy by `e^(−m·c·t)`
  whatever its reflections, so its decay rate adds to the walls' exactly. The other form in print,
  Bies and Hansen's (*Engineering Noise Control*, 4th ed., eq. 7.64, as Arup's Strutt help quotes
  it; the page read, the book not), folds `4·m·V/S` into `ᾱ` inside both logarithms. The transport
  tells them apart (below).
- **What it is**: the energy after time `t` is `⟨(1 − ᾱ)^n⟩` over the number `n` of reflections,
  whose mean is `c·t·S/(4V)` and, for independent free paths, whose variance is `γ²` times it; the
  formula keeps the first two cumulants of `n`. Two approximations, both measured below: the free
  paths in a room with flat walls are **not independent** (a long path tends to end where the next
  is long too), so `n`'s variance is about a quarter larger than `γ²·n` in M8's boxes and the
  formula reads low at small `ᾱ`; and the second-order truncation reads high as `ᾱ` grows. In M8's
  cells the two partly cancel.
- **Not the other "Kuttruff formula".** Kuttruff also corrected Eyring for absorption spread
  unevenly over the walls (the reflection-coefficient term `Δ`; R. Neubauer and B. Kostek, eq.
  (15)-(18), read). That one is not used here: M8's cells absorb evenly.
- **Refused** (`params_bad_room`) as Eyring's is, and for surfaces whose total area is not the
  room's the free paths were traced in (within 10⁻⁶: `γ²` of another room cannot be used), for
  `ᾱ` = 1, and where `1 + γ²·ln(1 − ᾱ)` is not positive (`ᾱ` above about 0.92 at `γ²` 0.4): past
  that, the second-order correction would give a longer time for more absorption. `V` and `γ²`
  both come from the traced room (`FreePaths`); the surfaces give only `ᾱ`, so they cannot bring
  another room's volume or shape.

### `γ²` from the geometry: `params::lambert`

A diffuse ray transport, written from scratch and promoted from the M7 follow-ups' evidence test
`tests/lambert_box.rs` (which stays, as it was run). Nothing of SPPS is used: no mesh walking, no
time stepping, no random generator of its, no output of any run.
- **The room** is a closed surface of triangles (any shape; faces need not be oriented: a ray is
  reflected to the side it came from) and tetrahedra that fill it (`Enclosure::from_mesh`; a box's
  six are built in). A ray runs straight to the nearest face (a bounding-volume hierarchy,
  Möller–Trumbore), is reflected there by Lambert's law, and starts again 10⁻¹⁰ of the room's size
  off the face.
- **`free_paths`** gives the mean free path and `γ²`, each with its standard error **over the
  rays**: successive paths of one ray are correlated, so each ray's paths are summed first and the
  ray is the independent unit; `γ²`'s error is the delta method's over them. Over 65,536 rays each
  error is itself known to a fraction of a per cent. (Until the pre-M8 review's second round the
  error was the spread of 16 replicas, known only to about a fifth; measured over 96 seeds, the
  rays' error matches the seed-to-seed spread, 0.00070 against 0.00076 and 0.00081 against 0.00079
  at the first settings, and in a sphere it is the closed form's `√(0.0422/n)`.) **It takes the
  room and nothing else**: its rays, paths and seed are fixed (`FreePathSettings::STANDARD`).
  `FreePaths` has private fields, no other constructor
  and no `Deserialize`, and `kuttruff_rt` takes only a `FreePaths`. So a `γ²` fitted to a solver
  cannot reach it, by construction, and neither can one fished for through the transport's
  settings. The pre-M8 review showed that the earlier API, which took settings, allowed this:
  searching over seeds at tiny ray counts gave any `γ²` from 0.10 to 0.60 for the 6×10×3 m room
  (exact 0.389), and Kuttruff's time moved by up to 7.5 %, past M8's 5 %. Three doctests hold the
  construction: the room alone compiles; the same call with settings does not; a `FreePaths`
  built by hand does not. Stable rustdoc does not check a `compile_fail` example's error code (a
  doctest failing with another error passed as `E0451`, tried), so each failing example differs
  from compiling code only in what it tests: the settings call from the passing example by its
  second argument, and the literal from the same literal compiled inside the module by a unit
  test.
- **What a caller can still vary is the room's description, in two parts.** Neither fits `γ²` to a
  solver, and `results::reference` takes both from the run's own `.cbin` and `.mbin`.
  - *Its surface* (placement, faces): each description draws the transport's randomness afresh,
    so a search over them picks among `γ²`'s own scatter. Measured: 16 descriptions of the
    6×10×3 m room (turned about the vertical and moved) gave `γ²` from 0.38866 to 0.38904 (exact
    0.38887), and Kuttruff's time at α 0.4 within 0.011 %
    (`gamma2_cannot_be_fitted_through_what_reaches_kuttruff`; the same test repeats the review's
    seed search through the test-only study, which reaches 0.009 to 2.1, so the search has
    power).
  - *Its tetrahedra*, which give `V`, to which Kuttruff's time is proportional: the
    mean-free-path check holds `V` to 6 of the mean free path's relative standard errors, 0.012 %
    in M8's rooms at the fixed settings, so to about ±0.07 %, and the time moves by as much.
    Measured in the 6×10×3 m room: a volume 0.03 % off either way is accepted (Kuttruff at α 0.4
    −0.033 % and +0.030 %), and 0.12 % off either way refused, 10.6 and 9.4 standard errors from
    `4V/S` (`the_volume_is_held_to_the_mean_free_paths_error`). At the first settings, with the
    replicas' error, the pre-M8 review found a volume 0.5 % off accepted and 0.4 % off refused; the
    rays' error ended that lottery, and the fixed settings' rays narrowed the window.
  `free_paths` also refuses a `γ²` whose standard error is above 0.002 (`GAMMA2_SE_LIMIT`): a room
  whose free paths the fixed rays cannot pin down. The fixed settings give 0.00012 to 0.00014 in
  boxes (0.00014 on the committed fixture, tutorial 1's box as TetGen meshed it), 0.0001 in the
  L-shaped room and 0.00004 in the sphere, so no room tried comes near it; the settings the
  review fished with are refused for it (a unit test, through the module's private entry).
- **Studies of the transport** at other settings or from one start point exist in test builds
  only (`free_path_study`, the `transport-study` feature that only `simpa-core`'s own tests turn
  on) and give a `FreePathStudy`, which no function of `params::room` takes. `free_paths` equals
  the study at `STANDARD` to the bit.
- **It checks itself**: it refuses (`params_transport_refused`) a mean free path further than 6
  standard errors from `4V/S` (about ±0.07 % in M8's rooms), which a closed room with Lambert
  walls must give whatever its shape; and a ray that leaves the room. Either means a surface that
  is not closed, tetrahedra that are not the surface's volume, a face with the room on both sides
  (it reflects on both, so the field sees its area twice), parts of a room that do not mix within
  the rays' paths, or reflection that is not Lambert's.
- **Where rays start matters.** A field started at one point is not yet diffuse, and a ray's first
  reflections remember it. Measured with this transport when it started every ray at one point
  (the pre-M8 WIP, and `lambert_box.rs`'s way): counted from the first reflection, 64 paths a ray,
  `γ²` read 0.3840 ± 0.0008 in the 6×10×3 m room and 0.3489 ± 0.0007 in the 5×4×3 m one (exact:
  0.3889 and 0.3524), the mean free path 0.16 % and 0.27 % long; in an L-shaped room with the rays
  started in its narrow wing, the mean free path was 0.28 % short (10 standard errors) after 16
  paths left out. So rays now start at points drawn evenly from the tetrahedra (Rocchini and
  Cignoni's folding), and each ray's first 32 paths after its first are left out. Even so the
  first reflection is weighted by path length: with none left out `γ²` reads 4.3 standard errors
  low in the box, and in the L-shaped room the mean free path sits 7.9, 1.5 and 0.2 standard
  errors from `4V/S` with 0, 16 and 64 left out (8.4 M paths each, errors over the rays).
- **Successive free paths are correlated** in a box, and not in a sphere (a study over 256 paths
  a ray, `successive_free_paths_are_correlated_in_a_box_and_independent_in_a_sphere`): the lag-1
  correlation is 0.0669 ± 0.0004 in the 6×10×3 m room and 0.0748 ± 0.0002 in the 5×4×3 m one,
  and the variance of a ray's length over its 256 paths, divided by `256·⟨ℓ⟩²`, is 0.480 ± 0.007
  and 0.440 ± 0.005 against `γ²` 0.389 and 0.353 (+24 % and +25 %). In a sphere of 5,120 faces,
  where each chord is `2R·cos θ` with a fresh `θ`: −0.0003 ± 0.0006, and 0.1266 ± 0.0012 against
  `γ²` 0.1253, the say-NO of the estimators. Kuttruff's formula takes `n`'s variance to be `γ²·n`.
- **`FreePathSettings::STANDARD`**, what `free_paths` always uses: 16 × 4096 rays, 1 + 32 + 512
  paths each, 33,554,432 paths counted, a fixed seed. **Its precision was set by a stated target before
  its draw was seen** (the pre-M8 review, second round, found the first settings, 16 × 1024 rays
  of 64 paths, put the shipped reference 0.005 points above 0.6 % in one cell from `γ²`'s draw
  alone; changing them until a draw fell below would have been fishing). The target: in every one
  of M8's cells, three of the standard deviations Kuttruff's time inherits from `γ²` fit between
  the formula's own error (at most +0.587 %) and 0.6 %, so `σ(γ²) ≤ 0.000154` in the 5×4×3 m
  room. The cost was measured at another seed (`σ(γ²)·√paths` 0.72 there and 0.83 in 6×10×3 m,
  at 64 or 512 paths a ray alike); the smallest power-of-two multiple of the first settings'
  paths that meets it is 32. The settings were committed on their own (`64c1d5a`, "Pre-register
  the transport's precision…") before any run at the reference's seed, the seed was kept, and
  the draw was taken as it came: `γ²` 0.35269 ± 0.00012 in the 5×4×3 m room, +2.3 standard
  errors from the exact value, which the target absorbs (the shipped time +0.593 % and +0.595 %,
  below). Over 16 other seeds that room's draws average −0.02 ± 0.19 standard errors from the exact
  value: no bias. `γ²` to about ±0.00012 to ±0.00014 in boxes, which moves Kuttruff's T by at most
  0.004 % in M8's cells. 512 paths a ray rather than 64 spends 6 % of the tracing on the paths
  left out instead of 34 %. About 0.55 s for a box in a release build (28 threads), about 2.3 s in
  a debug build.
- **`decay`** traces the energy of the room and of receiver balls (energy times path length inside
  the ball, per time bin, as SPPS's receivers collect it) from a point source, for M8's
  cross-check; air is applied per bin as `e^(−m·c·t)`.

### Known answers (`tests/params_lambert.rs`)

**The exact `γ²`.** Lambert reflection keeps a uniform, isotropic field, so in a **convex** room the
free paths are the chords of lines uniform and isotropic in space, whose mean square is
`⟨ℓ²⟩ = (2/(π·S))·∫∫ |x − y|⁻² dx dy` over pairs of points in the room: derived here from the
Blaschke–Petkantschin formula (`dx dy = |t₁ − t₂|² dt₁ dt₂ dG` for two points on a line `G`) and
Cauchy's `∫ dG = π·S/2` over the lines meeting the room. For a box the six-dimensional integral
folds into three smooth two-dimensional ones, one per far face, integrated by Gauss–Legendre; for
the unit cube it is D. H. Bailey, J. M. Borwein and R. E. Crandall's box integral `Δ₃(−2)`, whose
closed form they give ("Advances in the theory of box integrals", Math. Comp. 79 (2010)
1839-1866, Table 6; read in the authors' copy): the quadrature equals it to 10⁻¹². For a sphere
the chord is `2R·cos θ` with Lambert's `θ`, so `γ²` = 1/8 exactly.

| Room | Exact `γ²` | `free_paths` (33.5 M paths) | A study at its own seed (8.4 M paths) | Its mean free path against `4V/S` | `lambert_box.rs` | Transport counting as `lambert_box.rs` | Bies and Hansen's fit |
|---|---|---|---|---|---|---|---|
| cube | 0.344950 | 0.34505 ± 0.00012 | 0.34493 ± 0.00024 | +0.6 SE | | | 0.3383 |
| 6×10×3 m | 0.388874 | 0.38866 ± 0.00014 | 0.38885 ± 0.00028 | −1.1 SE | 0.388 | 0.38812 ± 0.00015 | 0.3959 |
| 5×4×3 m | 0.352401 | 0.35269 ± 0.00012 | 0.35268 ± 0.00025 | −0.6 SE | 0.352 | 0.35187 ± 0.00013 | 0.3560 |
| sphere, 20,480 faces | 0.125 (+0.00004 for the facets) | 0.12504 ± 0.00004 | 0.12492 ± 0.00010 (4.2 M paths) | +0.7 SE | | | |
| L-shaped, 6×4 + 2×4, 3 m | none known | 0.3904 ± 0.0001 | 0.3903 ± 0.0003 | +0.2 SE | | | |

- **`lambert_box.rs`'s values are its counting's.** It started every ray at the source and
  counted every path from the first reflection until the ray's time ran out, 482 paths a ray at
  α 0.05, and stated no uncertainty. Counting the same way (a study from the source, none left
  out, 482 paths), this transport gives 0.38812 ± 0.00015 and 0.35187 ± 0.00013: lambert_box's
  0.388 and 0.352 to their rounding. The exact values lie 5.0 and 4.1 standard errors above those
  (the errors over the rays; the pre-M8 review's own delta method gave the same 0.00015 and
  0.00013, where the replicas' spread had read 0.00009 and 0.00010 and made it 8 and 5), and
  0.38887 is outside 0.388's rounding: the difference is the rays' memory of their start (above),
  not the transport. Run as `free_paths` runs, the same transport gives the exact values
  (`lambert_boxs_values_are_its_counting_and_the_exact_ones_are_the_diffuse_fields`).
- **Bies and Hansen's fit** for rectangular rooms, `γ = √(0.0179·(L + W)/H − 0.0001·(L − W)/H −
  0.0011·((L − W)/H)² + 0.3025)`, H the height (as Strutt quotes it), lies within 0.007 of the exact
  values; the page states no accuracy for it. Stephenson's "for typical proportions from 1:1:1 to
  1:10:10, the variance γ² is in the order of 0.4…0.6 (as found by numerical experiments)" (the
  longer version of his paper, section 6.3) compares with exact values from 0.345 (1:1:1) to 0.653
  (1:10:10): the same order.
- The sphere's facets add to its `γ²` a quarter as much with each subdivision (+0.0172, +0.0044,
  +0.0011, +0.00025, +0.00004 from 80 to 20,480 faces, measured).
- **Say-NO partners**: reflection uniform over the hemisphere, through the code
  (`Fault::LambertUniformReflection`): mean free path 2.97 m against 3.33 m, 115 to 175 standard
  errors, refused; tetrahedra 1 % short of the room or 1 % over it: refused; one floor triangle
  removed, or the tetrahedra moved below the floor: rays leave, refused; the L-shaped room with
  only its narrow wing's tetrahedra: refused; an 80-face sphere: `γ²` 0.017 off 1/8; the
  review's fishing settings: refused for `γ²`'s standard error; `lambert_box.rs`'s counting: 5.0
  and 4.1 standard errors below the exact value; a volume 0.12 % off either way: refused.

### Kuttruff against the transport in M8's cells (`tests/params_kuttruff.rs`)

M8's cells: the two rooms at α 0.05, 0.1, 0.2 and 0.4 on every wall, Lambert reflection, air off,
M8's source and three receivers (radius 0.31 m), `dt` 1 ms, `c` 343.2 m/s. "Receivers" is the T30
of the transport's receivers' energy, read through `params` from the arrival, the mean of the
three receivers in each of 16 replicas, the error that of the 16 means: what M8 compares SPPS's
T30 with. "Room energy" is the T30 of the room's total energy, over the same replicas, several
times less noisy. **The formula is measured with each box's exact `γ²`** (integral geometry,
above), so that the comparison carries the transport's noise only; the reference as shipped
(`free_paths`' `γ²`) is held to the same 0.6 % and compared with the formula separately. Measured
at 4 M to 34 M rays a cell in a release build (`kuttruff_against_the_transport_at_high_counts`,
ignored with its reason and run on purpose, about 300 s). It asserts: the errors below 0.03 % and
0.006 %; the receivers and the room energy one decay within their paired noise; the formula
within the bare 0.6 % of both at the point estimate, and of the room energy also at one-sided
95 %; the reference as shipped within the bare 0.6 % of both; plain Eyring and the formula with
its `½` dropped missing; and that its T30s are the ones committed in the test (`HIGH`, which the
suite uses, below), to 10⁻⁹ s:

| Room | α | Receivers against Eyring | Kuttruff against the receivers | Room energy against Eyring | Kuttruff against the room energy |
|---|---|---|---|---|---|
| 6×10×3 | 0.05 | +1.211 % | −0.201 ± 0.016 % | +1.204 % | −0.194 ± 0.002 % |
| 6×10×3 | 0.1 | +2.443 % | −0.343 ± 0.019 % | +2.425 % | −0.326 ± 0.003 % |
| 6×10×3 | 0.2 | +4.971 % | −0.415 ± 0.017 % | +4.967 % | −0.411 ± 0.004 % |
| 6×10×3 | 0.4 | +10.718 % | +0.279 ± 0.024 % | +10.754 % | +0.247 ± 0.004 % |
| 5×4×3 | 0.05 | +1.092 % | −0.178 ± 0.010 % | +1.101 % | −0.187 ± 0.003 % |
| 5×4×3 | 0.1 | +2.198 % | −0.300 ± 0.012 % | +2.204 % | −0.305 ± 0.003 % |
| 5×4×3 | 0.2 | +4.461 % | −0.352 ± 0.012 % | +4.444 % | −0.336 ± 0.003 % |
| 5×4×3 | 0.4 | +9.250 % | **+0.587 ± 0.012 %** | +9.252 % | **+0.585 ± 0.002 %** |

- **Kuttruff's formula is within 0.6 % of the transport in every cell**, at the point estimate on
  both. On the room energy also at one-sided 95 % confidence (point + 1.645 SE, worst 0.589 %); on
  the receivers the worst cell's bound is 0.607 %. **The two are one decay in M8's cells**: the
  receivers' T30 minus the room energy's, over the replicas' paired differences, is within 1.4
  standard errors in every cell (−0.032 % to +0.017 %; in the worst cell −0.002 ± 0.015 %), and
  4.9 to 31 standard errors in the elongated room below, the check's partner. So in M8's cells the
  room energy measures the same T30 six times more precisely, and the one-sided 95 % statement is
  its 0.589 %; the receivers' 0.607 % is their own noise, ±0.012 %, about a value the room energy
  puts at 0.585 ± 0.002 %. The 5×4×3 m room at α 0.4 is 0.013 points inside: at that absorption
  the second-order formula is at its limit. Independent numpy transports in the two pre-M8 reviews
  gave that cell's room energy +0.591 ± 0.006 % and +0.581 ± 0.009 %.
- **The 0.6 % is not a held-out bound, and nothing supports it beyond M8's two boxes.** It was read
  off `lambert_box.rs`'s Kuttruff column for these same cells. The formula has no fitted
  parameter, but the limit was chosen on the data it is checked against, and the one room held
  out, 20×4×3 m (below), is outside it from α 0.2. It is a check that this code computes
  Kuttruff's formula with the room's `γ²` in M8's rooms, not an accuracy of the formula.
- **Why the pattern**: low at small α, high at 0.4. Successive free paths are correlated (lag 1
  0.067 and 0.075, above), so `n`'s variance is about a quarter above `γ²·n`. The formula, fed `γ²`,
  underestimates the spread and so the time: with the measured effective variance (over 256 paths)
  in place of `γ²` the α 0.05 cells would read about 0.23 % longer, close to the 0.18 to 0.20 %
  they miss by. As α grows the second-order truncation adds the other way. The two partly cancel
  in these rooms. (Using the effective variance would be another formula than the one decided;
  not done.)
- **The reference as shipped** carries `free_paths`' `γ²` (0.38866 ± 0.00014 and 0.35269 ± 0.00012,
  −1.5 and +2.3 standard errors from the exact values), and **is within the bare 0.6 % in every
  cell on both**: worst +0.595 % of the receivers and +0.593 % of the room energy (5×4×3 m,
  α 0.4), where the draw adds +0.008 % (`mc_sd` 0.0035 %). It agrees with the formula within 3 of
  the `mc_sd` it reports in every cell. At the first fixed settings, 32 times fewer paths, the same
  cell was +0.607 % and +0.605 %; the settings changed by the precision target stated under
  "`γ²` from the geometry" and committed before their draw, not by where a draw fell.
- The receivers' excess over Eyring reproduces `lambert_box.rs`'s (+1.20, +2.44, +4.96, +10.78 %;
  +1.11, +2.20, +4.47, +9.25 %) within 0.07 %, and so SPPS's (`docs/results.md`, "T30 against
  Eyring"). The room energy is within 0.04 % of the receivers in every M8 cell.
- **The gate in the test suite** (`kuttruff_reproduces_the_transports_t30_in_m8s_cells`, 0.26 M to
  2.1 M rays a cell in a debug build) **asserts the bare 0.6 %**. A debug build cannot trace
  enough rays to judge a cell 0.013 points inside 0.6 % by its own transport, so the high-count
  T30s are committed in the test (`HIGH`), and in every cell: (a) the suite's own transport
  reproduces them within 4 of their combined standard errors (4 for sixteen comparisons; with
  its errors below 0.12 % on the receivers and 0.02 % on the room energy, so a change of the
  transport of 0.08 % in the room energy fails; measured −2.0 to +2.1 standard errors on the
  receivers and −0.8 to +1.3 on the room energy); (b) the formula with the exact `γ²` is within
  the bare 0.6 % of both committed T30s; (c) the shipped reference is too, and is the formula
  within 3 of its `mc_sd`; (d) the fixed settings meet their precision target, `3·mc_sd` within
  the formula's margin to 0.6 % (worst: 5×4×3 m at α 0.4, 0.0105 % against a margin of
  0.0132 %). Its partners: plain Eyring misses (b) in all 8 cells (−1.1 % to −9.7 % of the room
  energy); the formula with its `½` dropped, through the code (`Fault::KuttruffFullVariance`,
  with `free_paths`' `γ²`), misses (b) and (c) in all 8 (+0.73 % to +12.7 %); the transport
  reflecting evenly over the hemisphere, through the code (`Fault::LambertUniformReflection`),
  misses (a) in the worst cell by 525 standard errors; and the first fixed settings miss (d)
  there (`3·mc_sd` 0.060 % against a margin of 0.013 %).
- **An elongated room, for information** (20×4×3 m, `γ²` exact 0.40791; the same test, printed
  only; M8 does not use it): the formula against the room energy −0.22, −0.40, −0.62 and −0.89 %
  at α 0.05 to 0.4, **outside 0.6 % from α 0.2**; against the receivers −0.12, +0.02, +0.04 and
  −0.10 %. There the receivers' T30 and the room energy's differ by up to 0.8 % (+11.74 % and
  +12.63 % against Eyring at α 0.4): the field decays unevenly along the room. Neither 0.6 % nor
  the receivers' closeness carries over to other shapes; M12's rooms need their own evidence.
- **The air term** (`the_air_term_is_added_outside_as_the_transport_decays`): M8's second-table
  cells at 8 kHz, 20 °C, 50 %, with the solver's `m` = 0.02425 /m (air 1.6 and 0.6 times the walls'
  absorption): Kuttruff + `4mV` −0.03 % (6×10×3, α 0.05) and −0.18 % (5×4×3, α 0.1) of the
  transport with the same air; the air folded into `ᾱ` (`Fault::KuttruffAirInsideMean`) −3.54 %
  and −3.57 %, which the test requires to miss.

### In `simpa results --json`

Every SPPS report carries `spps.reference` (`results::reference`; `docs/formats/results-json.md`):
the room's volume and area, SPPS's `c` and `K`, the transport's `free_paths`, and per computed band
`air_m_per_metre`, `mean_absorption`, `lambert_walls`, **`kuttruff_s` (M8's reference, with the
`mc_sd` it inherits from `γ²`: only that, not the formula's own error above, which no ray count
reduces) and `eyring_s` (plain, reported only)**, under a `label` that says
both are an analytic reference for a diffuse field and not validated. The room is read as TCR's
`analytic` reads it (the `.cbin` faces, the `.mbin`'s tetrahedra, `config.xml`'s materials and
air; TCR's per-face rule for fittings, `TC_CalculationCore.cpp:11-17`, is not emulated, so a scene
with fitting faces is `not_computed`); a celerity gradient is `not_computed` too. `lambert_walls` is false where any face
is not Lambert with scattering 1 in the band: then neither time describes the run's field, and
**Kuttruff's time is not computed there**, refused `params_reference_not_applicable`; the
transport is run only when at least one computed band has Lambert walls, `free_paths` `null`
otherwise. `eyring_s` is given whatever the walls. The text output prints the same beside "NOT
VALIDATED". The committed fixtures have specular walls, so none carries `γ²`; the Seat run's
inputs with Lambert walls at 500 Hz give 0.38883 ± 0.00014, the exact 0.38887 within its error,
a Kuttruff time at 500 Hz and the refusal at 1000 Hz (`results_load.rs`,
`the_reference_is_computed_only_for_bands_with_lambert_walls`).

**What it costs** (pre-M8, `docs/investigations/2026-09-25-reference-cost/`): computed for every
band, the transport added about 5 s to `simpa results` on the corrected Elmia hall (release build,
Grace: 6.2 to 6.3 s against 0.8 to 1.1 s for the committed room's own 1,000,000-particle run; 5.9
to 6.0 s against 0.7 to 0.8 s for M6's loss-gate run), for a `γ²` (0.555) describing none of its
bands, which scatter 0.15 to 0.3 or reflect specularly. Hence the rule above. Where the reference
applies (the Seat box with Lambert walls) a call takes 1.0 to 1.2 s either way.

**How a band's scattering is read.** `lambert_walls` reads each material's `diffusion` as SPPS
reads it (`atof`, through `run::locate::to_float`): a text that is not a number reads 0, specular,
so that band is not Lambert and neither time describes its field; a value that reads as NaN or
infinite, or a band with no `loi`, makes the reference `not_computed`
(`results::reference`'s tests, `lambert_walls_need_every_face_lambert_with_scattering_1_in_the_band`).
So where a band's scattering is not a number, `lambert_walls` is false and the Kuttruff reference
does not apply to that band. A project of this program never gets that far: its scattering is a
number when written, and the `config.xml` importer refuses a `diffusion` that is not one,
`invalid_value` (`config_xml::import`, through `num::solver_real`;
`config_xml_import.rs`, `a_scattering_that_is_not_a_number_is_refused_on_import`, whose partner
reads the solvers' comma decimal `0,5` as 0.5). Only a run folder given as it is can carry one.

**Open, for M8 and later**: the bed itself (Kuttruff at 5 %, the transport's T30 as the tight
cross-check, `dt` 1 ms); Michael's ratification of the gate text; M12's use of `lambert_walls`;
rooms whose parts barely exchange sound (coupled volumes), and faces with the room on both sides
(thin reflectors), which the mean-free-path check refuses rather than describe (the corrected
Elmia hall, with hanging reflectors, was not refused: `γ²` 0.55499 ± 0.00024; whether its
reflectors are such faces was not checked, and no band of it has Lambert walls); the formula's
error in rooms of other shapes (−0.89 % already in a 20×4×3 m box at α 0.4), which `mc_sd` does
not carry; and whether Burhan keeps the precision target that set the fixed settings (32 times the
first settings' paths, about 0.55 s a box in a release build), or states 3c for the formula alone
and takes the cheaper settings back.

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
  Says no through the code (M7 follow-ups; the first partners recomputed the formulas beside it):
  the natural logarithm put into `din18041`'s `lg` by a test-only fault seam
  (`simpa_core::faults::Fault::DinNaturalLog`, compiled only into test builds) gives 1.4917 s, and
  the code's own A2 and A4 formulas give 0.6945 s and 0.4464 s; each misses
  (`params_room.rs::gate_f_din_18041_a3_at_180_m3_is_0_552_s`).
- **The formulas** are BNB V 2020, criterion 3.1.4, which quotes them "nach DIN 18041:2016-03".
- **The volumes** are where each group's line is drawn solid in the standard's figure of `T_soll`
  against `V`, as reproduced by Nocke (Lärmbekämpfung 11 (2016) Nr. 2, Bild 2, page 51); the
  article calls the solid stretches the typical volumes of each use, and draws the rest dotted.
  The ends were measured on the figure (`din_figure.py`, receipts above) against its 50, 100, 200,
  500, 1000, 5000 and 10 000 m³ grid lines and the axis's ends at 30 and 30 000 m³; each falls
  within a line's end cap (under 4 % in volume) of one of them (A5's upper end, measured at
  29 911 m³, is the axis's right end). The text sources agree wherever they speak:
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
- `early_unresolved`, with the value read with the reverberation beginning at the arrival, the
  lowest and highest of the three readings of the early reverberation (beginning at the arrival,
  at the first bin wholly after the direct sound, or at that bin's end; either missing when that
  curve gives no value), their midpoint and the limit ("The early reverberation", above);
- `range_too_short`, with the time spent in the range and the time needed;
- `not_decaying`;
- `empty_window`;
- `missing_not_cleared` and `missing_moves`, with the floor and the lost share, and the depth
  reached or the value with the missing energy added ("Missing energy");
- `monte_carlo_noise`, with the value, its calibrated standard deviation, the limit, the resamples
  that refused it and the particle count that would bring it within the limit, or why none is
  named; `noise_unknown`, with why; `noise_uncalibrated`, with the run's particles and crossings
  per particle, the domain its quantity's calibration was measured on, and the particles to run
  or how far to shrink the receiver radius ("Monte-Carlo noise");
- `several_sources`, with the sources: made by `core::results`, not by `params`
  (`docs/results.md`, "Several sources");
- `no_time_series`, with where the solver's own values are: made by `core::results` for every
  parameter of a TCR receiver, which has steady-state levels and no series
  (`docs/formats/results-json.md`, "`tcr`").

`params_bad_noise_input` refuses a floor, a share alive or lost, a mean or least deposit, a
lifetime spread, a particle count or a band count that is not a finite number in its domain.

`params_transport_refused` refuses the diffuse transport's own result: inputs it cannot run with,
a ray that left the room, or a mean free path that is not `4V/S` within its error ("Kuttruff's
reference"). `params_bad_room` also refuses Kuttruff's inputs: surfaces of another room than the
free paths', `ᾱ` = 1, and `1 + γ²·ln(1 − ᾱ)` not positive.

The citations of upstream's lines in `params::air` and `params::room` are checked against the
source at `929a5c8` by `params_air.rs` and `params_room.rs`, so a citation that drifts fails a test.
