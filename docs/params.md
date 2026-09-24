# Acoustic parameters: `core::params`

**M7, piece A.** Pure functions over a per-band energy histogram, and the analytic references the
later gates need: ISO 9613-1 air attenuation, Sabine and Eyring as TCR computes them, and the DIN
18041 targets. Code: `crates/simpa-core/src/params.rs` and `params/`. Tests:
`crates/simpa-core/tests/params_synthetic.rs` (gate (a)), `params_air.rs` (gate (b)),
`params_room.rs` (Sabine, Eyring, gate (f)).

**No number computed here is shown to a user until M8's physics bed passes** (`docs/rebuild-plan.md`,
M12). M7 builds the numbers; it does not publish them.

## Sources, and how far each one is trusted

| Source | What we took from it | Read by us? |
|---|---|---|
| ISO 3382-1 | The definitions of EDT, T20, T30, C50, C80, D50, Ts and the onset rule, **as commonly stated**. Marked [commonly stated] below | **No.** The text is paywalled and was not opened |
| ISO 9613-1:1993(E) | Equations (1)–(6), clauses 5–7, Table 1 (a)–(h) (−20 °C to +15 °C) | **Yes**, pages 1–8 of the iTeh preview (`cdn.standards.iteh.ai/samples/17426/3a2d69b767024b74805b83b063a91445/ISO-9613-1-1993.pdf`, sha256 `cb0e28c6…`). Annex B and Table 1 (i) (20 °C) are **not** in the preview |
| GOST 31295.1-2005 (ISO 9613-1:1993, MOD) | Table 1 (i), 20 °C: gate (b)'s values | **Yes**, printed page 13 (PDF page 18) of `meganorm.ru/Data2/1/4293849/4293849418.pdf` (sha256 `03f11462…`). It is the Russian adoption, **not ISO's own page.** Its Annex F lists only editorial changes. Its 15 °C, 50 % column equals ISO's own Table 1 (h), page 8, in all 24 bands (both transcribed into `params_air.rs`, where the test asserts they are equal) |
| DIN 18041:2016-03 | The five group-A targets | **No.** Formulas from BNB V 2020, criterion 3.1.4 (BMI, `bnb-nachhaltigesbauen.de/.../BNB_LN2020_314.pdf`, sha256 `576bdfda…`), pages 4 and 6–8, which quote "nach DIN 18041:2016-03" |
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

## Direct-arrival detection (the onset t₀)

- **ISO 3382-1** [commonly stated]: the impulse response starts where the squared response first
  rises to within 20 dB of its maximum.
- **Applied to a histogram:** the onset bin `k₀` is the first bin whose energy is at least
  1/100 of the largest bin, and `t₀ = k₀·dt`, the start of that bin.
  - The comparison is between bins, so it depends on `dt`: a coarse bin mixes the direct sound
    with early reflections. At SPPS's usual steps the direct sound sits in one bin and dominates.
  - Energy before `t₀` is excluded from every onset-relative parameter (EDT, T20, T30, C, D and
    Ts). SPL sums all of it.
- **Upstream differs.** `GetTimeDecay` (`projet_calculation.cpp:127-139`) takes the last time label
  before the energy first changes by 10⁻¹⁸ in absolute value (`refValue`, lines 321, 357, 393,
  424). That is an absolute threshold, not a relative one, and the decay times ignore the onset
  altogether (below).

## Schroeder backward integration

`S(t) = ∫ₜ^∞ E dt`, in the histogram `S_k = Σ_{j≥k} B_j`, which is exactly `S` at `t = k·dt`, the
start of bin `k`. The decay curve is `L_k = 10·lg(S_k / S_{k₀})`, 0 dB at the onset. Sums run from
the last bin backwards, in `f64`.

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
2. **The check.** Every parameter is computed twice: as reported, from the series alone; and with
   `M` added after the last bin with energy. If the two differ by more than the limit below, the parameter is
   refused as `params_not_evaluable` (`truncated`), with both numbers in the detail. An unbounded
   tail refuses everything that depends on it. **The second number is never reported as the
   value.**

   | Quantity | Limit | Why this size |
   |---|---|---|
   | EDT, T20, T30 | 0.5 % relative | 1/10 of the 5 % difference limen, as commonly quoted from ISO 3382-1 Annex A; also gate (a)'s tolerance |
   | C50, C80 | 0.1 dB | 1/10 of 1 dB, likewise |
   | D50 | 0.005 (0.5 points) | 1/10 of 0.05, likewise |
   | Ts | 1 ms | 1/10 of 10 ms, likewise |
   | SPL | 0.1 dB | 1/10 of 1 dB (the limen quoted for G) |

3. **The depth reached.** A decay time needs the curve to reach the bottom of its range. The depth
   is read from the curve **with** the tail added: that is how far the decay had fallen when the
   series ended. Without the tail the curve dives at the last bins and reaches almost any depth.
   Not deep enough: `params_not_evaluable` (`range_not_reached`), with the depth reached.

**Why the depth check alone is not enough, measured:** an exact exponential with `T = 1 s`,
`dt = 1 ms`, cut where the true decay reaches `D` dB. T30 is fitted over the points of the
truncated curve in −5 … −35 dB with nothing added, as upstream does bar its one extra point:

| D at the end | 30 dB | 35 dB | 40 dB | 45 dB | 50 dB | 55 dB | 60 dB |
|---|---|---|---|---|---|---|---|
| T30 error | −12.2 % | −6.1 % | −2.4 % | −0.85 % | −0.28 % | −0.09 % | −0.03 % |

At D = 30 dB the truncated curve still passes −35 dB, in 5 of the 6 `(T, dt)` cases of gate (a),
so a T30 is produced, 12 % short, without a word. The same holds at `T = 0.3` and `3 s` and at
`dt = 10 ms` to within a point. With our bound, T30 at `T = 1 s`, `dt = 1 ms` is first accepted at
48 dB of true decay. That is close to the 45 dB of dynamic range usually quoted for T30 from
ISO 3382-1. Both the table and the 48 dB are asserted by
`params_synthetic.rs::the_documented_truncation_table_holds`.

**Upstream differs.** `MakeSchroederArray` (`projet_calculation.cpp:68-83`) integrates backwards
without any tail handling. `GetTimeRange` (lines 143-170) sets the end of the regression to the
last time step when the bottom is never reached (line 150), so a decay that stops at −20 dB gets a
T30 regressed down to its end, silently.

## Decay times: EDT, T20 and T30

[commonly stated] A least-squares line through the decay curve between the range limits,
extrapolated to 60 dB: `T = −60 / slope`.

| Quantity | Range | Same as |
|---|---|---|
| EDT | 0 dB to −10 dB | 6 × the 10 dB decay time |
| T20 | −5 dB to −25 dB | 3 × the 20 dB decay time |
| T30 | −5 dB to −35 dB | 2 × the 30 dB decay time |

- **Points.** Every `(k·dt, L_k)`, `k ≥ k₀`, with `bottom ≤ L_k ≤ top`. At least 3 points are
  needed, else `params_not_evaluable` (`too_few_points`). A slope that is not negative is
  `params_not_evaluable` (`not_decaying`).
- **Refused:** the depth reached is above the bottom (`range_not_reached`, with the depth); the
  truncation bound is exceeded (`truncated`).
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
  - Upstream fits in `f32`, on times in ms.
  - It also computes T15 (`TRdefault = "15;30"`, line 803), which we do not.

## Clarity C50 and C80, definition D50, centre time Ts

[commonly stated], with `t` measured from the onset and `E` the energy histogram:

- **C_te** = `10·lg( ∫₀^te E dt / ∫_te^∞ E dt )` dB, te = 50 or 80 ms.
- **D50** = `∫₀^50ms E dt / ∫₀^∞ E dt`, a fraction (shown in %).
- **Ts** = `∫₀^∞ t·E dt / ∫₀^∞ E dt`, in seconds.

How they are evaluated:
- **A window edge inside a bin** splits the bin in proportion, as if its energy were spread
  evenly over it. With `t₀` on a bin edge and `te` a multiple of `dt`, no bin is split.
- **Ts** puts each bin's energy at the bin's midpoint. For an exponential this reads long by
  `(dt/τ)²/12` of τ: 1.8 % at `T = 0.3 s`, `dt = 10 ms`, and 0.02 % at `dt = 1 ms`. The test
  checks the histogram's own closed form, `dt·(1/(e^{dt/τ} − 1) + 1/2)`, and that the
  difference to τ is that bias.
- **Refused:**
  - the series ends at or before `t₀ + te`: `params_series_too_short`;
  - there is no energy after `t₀ + te`: `params_not_evaluable` (`empty_window`);
  - the truncation bound is exceeded: `truncated`.
- **Upstream differs:**
  - `GetSumLimit` includes both window edges (`projet_calculation.cpp:102`), on time labels that
    are bin ends. So the bin that ends at `t₀ + te` counts as early and as late (lines 366, 401).
  - Its `t₀` is the absolute-threshold time above.
  - **Its Ts is not measured from the onset.** Line 432 weights by the absolute time label, so
    upstream's Ts includes the propagation delay `r/c` and half a bin more.

## Sound pressure level

`SPL = 10·lg( Σ_k B_k / p₀² )`, with `p₀² = (20 µPa)² = 4·10⁻¹⁰ Pa²`, over every bin of the band.
It is the steady-state level of a source emitting its power continuously. This is upstream's
`dB_Sum_Param` (`projet_calculation.cpp:220-240`, with `p_0` at line 41) and its receiver view
(`e_report_gabe_recp.cpp:56, 145`).

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
attenuation `I(x) = I₀·e^(−m·x)` (`base_core_configuration.cpp:114`). SPPS kills a particle over
one step with probability `1 − e^(−m·c·dt)` (line 115), and TCR adds `4·m·V` to the absorption area
(`TC_CalculationCore.cpp:138`). The header comment calls it "dB/m" (`coreTypes.h:47`); it is not.
A user-set `absatmo` is used verbatim as `m` (`docs/formats/config_xml.md`).

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

`T_soll`, in s, with `V` in m³ (BNB V 2020, 3.1.4, "nach DIN 18041:2016-03"):

| Group | Use | T_soll | Page | Volume range |
|---|---|---|---|---|
| A1 | Music | 0.45·lg V + 0.07 | 6 | ≤ 5000 m³ |
| A2 | Speech, lecture | 0.37·lg V − 0.14 | 7 | ≤ 5000 m³ |
| A3 | Teaching, communication | 0.32·lg V − 0.17 | 7 | ≤ 5000 m³ |
| A4 | Teaching, communication, inclusive | 0.26·lg V − 0.14 | 4 | ≤ 5000 m³ |
| A5 | Sport | 0.75·lg V − 1.00 for 200 ≤ V ≤ 10 000 m³; 2.0 s above | 8 | from 200 m³ |

- **Gate (f):** A3 at 180 m³ gives 0.5517 s, within 0.552 ± 0.001, and 0.55 s in the design README.
- **The upper limit of A1–A4** comes from the DEGA Akustik Journal 03/19, page 17 (sha256
  `fb2ddd25…`): above 5000 m³ DIN 18041 sets requirements only for sports and swimming halls.
  Such a volume is refused, `params_din_out_of_range`.
- **The lower limits of A1–A4 are not sourced.** A volume is accepted down to where the formula
  would give a target of 0 s or less, which is refused. **Open:** read the standard's own ranges.
- The same article's example, 245 m³ in A3, prints 0.60 s. The formula gives 0.5945 s, which is
  0.6 to one decimal but 0.59 to two. It confirms the formula to 0.01 s at best, and is not used
  as a check.

## Refusal codes

Each code is a row of `docs/solver-contract.md`, Part B, "Parameter refusals". A refusal is a
typed `ParamError`, never a warning and never a number. `params_not_evaluable` carries the quantity
and one of `range_not_reached` (with the depth reached), `truncated` (with the value and the value
with the tail added; no second value when the tail is unbounded, or when the curve with the tail
has too few points in the range to fit), `too_few_points`, `not_decaying` or `empty_window`.

The citations of upstream's lines in `params::air` and `params::room` are checked against the
source at `929a5c8` by `params_air.rs` and `params_room.rs`, so a citation that drifts fails a test.
