# I-Simpa upstream, cold read: EDT/RT/C/D/Ts and the SPPS histogram pipeline

Scope: `B:/repos/I-Simpa-upstream`, tag `v1.4.0_snapshot_14_01_2026`, read fresh — no prior
project docs or agent output consulted. All paths below are relative to that checkout.
All line numbers are from the tree as checked out at that tag (verified 2026-09-27).

Companion file: `upstream_edt.py` in this folder — a transcription of the functions cited
in section 1, run on synthetic input in section 7. See "Transcription honesty" (section 7.4)
for exactly what "line-for-line" does and does not mean here.

---

## 0. The two solvers, and which one this is about

I-Simpa ships two acoustics engines behind the same GUI:

- **SPPS** (`src/spps/`) — particle tracing / ray histogram. This is what the task (and
  I-Simpa_Night_Mode) means by "SPPS histograms." Its receiver output is a per-time-bin
  energy histogram; EDT/T/C/D/Ts/STI are all derived from that histogram **after** the
  solver has finished, by GUI post-processing code, not by SPPS itself.
- **CTR/TCR** (`src/ctr/`) — classical statistical-acoustics theory (Eyring/Sabine-family
  formulas plus an explicit analytic direct-field term). Not a histogram method; mentioned
  here only as a contrast in §2.3, because it shows upstream *does* know how to isolate a
  direct field cleanly — just not in SPPS.

The acoustic-parameter math (EDT, T15/T30, C50/C80, D50, Ts, ST) lives in **one file**,
shared by both point-receiver and surface-receiver post-processing:

  `src/isimpa/data_manager/projet_calculation.cpp`

and is invoked from `ProjectManager::OnMenuDoAcousticParametersComputation`
(`projet_calculation.cpp:792`), triggered by the GUI event `IDEVENT_RECP_COMPUTE_ACOUSTIC_PARAMETERS`
(`src/isimpa/data_manager/element.h:339`, dispatched at `src/isimpa/data_manager/projet.cpp:543-545`).

---

## 1. How EDT / T15 / T30 / C / D / Ts / ST are computed

### 1.1 Data types (float vs double) — receipt for "float or double"

| Quantity | Type | Receipt |
|---|---|---|
| SPPS's live per-particle energy, and its two accumulators (`tabEnergyByTimeStep`, `t_Recepteur_P::energy_sum`) | `l_decimal` = **double** | `src/lib_interface/coreString.h:43` `#define l_decimal double` |
| Everything geometric/time-step-related inside SPPS (`decimal`) | **float** | `src/lib_interface/Core/mathlib.h:54` `typedef float decimal;` |
| The `.gabe` file column type SPPS writes its histogram into (`Floatb`) | **float** | `src/lib_interface/input_output/gabe/gabe.h:63` `typedef float Floatb;` — see the explicit downcast at `src/spps/input_output/reportmanager.cpp:435`: `statValues->Set(idstep,(Floatb)(tabEnergyByTimeStep[idstep]*cdtRho));` |
| Every quantity inside the EDT/RT/C/D/Ts pipeline (`projet_calculation.cpp`): `timeTable`, `tab_wj`, `tab_schroeder`, the regression sums, the returned EDT/RT value | `wxFloat32` = **float** | e.g. `projet_calculation.cpp:175` (`ComputeLinearRegression` signature), `:190` (`Compute_TR_Param` signature), `:220` (`dB_Sum_Param`) |

So: SPPS accumulates each receiver's raw energy in **double**, but that double is thrown away
the moment it is written to a `.gabe` file (cast to `float`, receipt above), and the entire
EDT/T/C/D/Ts computation — Schroeder integration, the linear regression, the returned value —
runs in **32-bit float**, in a GUI post-processing pass, not in the solver.

### 1.2 The Schroeder integration

`MakeSchroederArray` (`projet_calculation.cpp:68-83`):

```cpp
void MakeSchroederArray(srcArray, dstArray, funcDbConv = &to_deciBelP0) {
    dstArray = srcArray;                                   // raw copy of the linear-energy histogram
    for each frequency row idFreq:
        sumW = 0
        for idStep from (last bin) down to 0:               // backward cumulative sum
            sumW += dstArray[idFreq][idStep]
            dstArray[idFreq][idStep] = funcDbConv(sumW)      // dB'd IN PLACE, immediately
}
```

This is the textbook backward (Schroeder) integral: bin `i`'s output value is
`10*log10(sum_{j>=i} w_j * p_0)` where `p_0 = 1/(20e-6)^2` (`projet_calculation.cpp:41,46-49`).
It runs on `wxFloat32` throughout, converting to dB **immediately inside the same backward
loop**, one bin at a time — not "sum everything in double, then log once."

### 1.3 Where the fit starts and ends, and which fit

`GetTimeRange` (`projet_calculation.cpp:143-170`) finds the two time bounds `[debT, endT]`
handed to the regression:

```cpp
void GetTimeRange(fromdB, todB, *timeDeb, *timeEnd, timeTable, row_db) {
    *timeDeb = timeTable[0];              // default: first recorded bin
    *timeEnd = timeTable[last];           // default: last recorded bin
    lastdB = row_db[0];                   // <-- fixed reference: THE FIRST BIN. Never re-read.
    insideBound = false;
    for idStep in row_db:
        if (!insideBound) {
            if (abs(row_db[idStep]-lastdB) >= fromdB) { *timeDeb = timeTable[idStep]; insideBound = true; }
        } else {
            if (abs(row_db[idStep]-lastdB) >= todB)   { *timeEnd = timeTable[idStep]; break; }
        }
}
```

`ComputeLinearRegression` (`projet_calculation.cpp:175-186`) is an ordinary **least-squares
straight-line fit** of the dB'd Schroeder curve vs. time, over `[debT, endT]` inclusive on
both ends (`GetSumLimit`, `projet_calculation.cpp:95-119`, uses `currentTime>=fromTime &&
currentTime<=toTime`). It is computed via the four raw sums `n, Σt, Σy, Σt², Σty`
(`Sx,Sy,Sx2,Sxy`), Ordinary Least Squares closed form:

```
a = (n·Σty − Σt·Σy) / (n·Σt² − (Σt)²)      // slope, dB per ms (time is in ms here)
b = (Σy·Σt² − Σt·Σty) / (n·Σt² − (Σt)²)    // intercept (computed but unused downstream)
```

`Compute_TR_Param(fromdbL, todbL, ...)` (`projet_calculation.cpp:190-219`) then does, per
frequency band: `GetTimeRange(fromdbL, todbL, ...)` → `ComputeLinearRegression(...)` →
`value = (-60/a)/1000` seconds — i.e. the standard ISO-3382 "extrapolate the early slope to
a 60 dB drop" construction. It is called three ways from
`OnMenuDoAcousticParametersComputation` (`:930-936`):

| Call | fromdbL | todbL | Decay range used | Label |
|---|---|---|---|---|
| `Compute_TR_Param(0, 10, ...)` | 0 | 10 | 0→−10 dB from **the first bin's own value** | `"EDT (s)"` |
| `Compute_TR_Param(5, TRParams[i]+5, ...)`, default `TRParams = {15, 30}` | 5 | 20 | −5→−20 dB | `"RT-15 (s)"` |
| same, second default value | 5 | 35 | −5→−35 dB | `"RT-30 (s)"` |

**T30 is exactly the ISO 3382 T30 recipe** (−5 to −35 dB, 30 dB range, extrapolated ×2).
**"T20" is not upstream's shipped default** — the default pair is 15;30 (RT‑15, i.e. −5 to
−20 dB / 15 dB range, and RT‑30). A user can type `15;20;30` in the dialog
(`projet_calculation.cpp:808-812`, free-text, `;`-separated, parsed by `SplitString`) and get
a genuine T20, but the two numbers I-Simpa ships as defaults are **RT‑15 and RT‑30**, not
T20/T30. (Receipt for defaults: `TRdefault="15;30"`, `projet_calculation.cpp:803`.)

Per-band values are shown alongside a `"Global"` band (broadband energy-summed) and an
`"Average"` row that is a **plain, unweighted arithmetic mean of the per-band values across
whatever bands were requested, explicitly excluding the Global band**
(`projet_calculation.cpp:205,209`: `if(idFreq!=tab_schroeder.size()-1) sum+=...; ... sum/(tab_schroeder.size()-1)`).
It is not energy-weighted, not A-weighted.

### 1.4 C50/C80, D50, Ts, ST — and the one thing they do that EDT/RT does not

All four (`Compute_C_Param` `:350-380`, `Compute_D_Param` `:386-412`, `Compute_TS_Param`
`:418-443`, `Compute_ST_Param` `:313-344`) start by finding **the direct-sound arrival**
with a dedicated helper, `GetTimeDecay` (`projet_calculation.cpp:127-139`):

```cpp
wxFloat32 GetTimeDecay(fromdB, timeTable, row_db) {   // called with row_db = RAW ENERGY, not dB
    timeDeb = timeTable[0];
    lastdB = row_db[0];
    for idStep in row_db:
        if (abs(row_db[idStep]-lastdB) >= fromdB) break;
        else timeDeb = timeTable[idStep];
    return timeDeb;                                   // last bin BEFORE energy first jumps
}
```

called as `wt0 = GetTimeDecay(refValue, timeTable, tab_wj[idFreq])` with
`refValue = pow(10,-180.f/10.f) = 1e-18` (an absolute linear-energy noise floor, e.g.
`:357,393,424,325`). `wt0` is then used as the **local time origin** for that band's C/D/Ts/ST
window (`wte = wt0 + te`, etc.) — i.e. these four parameters correctly anchor their
integration windows on the detected direct-sound arrival, per ISO 3382 practice.

**`Compute_TR_Param` (EDT and T15/T30) never calls `GetTimeDecay`.** It uses `GetTimeRange`
instead, whose reference is `row_db[0]` (the Schroeder curve's *first recorded bin*, not the
direct-sound arrival — see §6, I1). The direct-sound-detection machinery exists in this same
file and is used by four of the eight parameters computed here, but not by EDT or RT. This is
the cleanest, most concrete finding in this reading: **upstream's own code demonstrates the
fix for its own EDT bug, immediately above the function that needs it, and simply doesn't
apply it there.**

---

## 2. What happens to the direct sound

### 2.1 How SPPS records it at all

A receiver is a sphere of radius `rayon_recepteurp` (default 0.31 m, `src/isimpa/data_manager/tree_core/e_core_sppscore.h:54`).
Each time a particle's free-flight segment (within a single time step) passes through that
sphere, `ReportManager::ParticuleFreeTranslation` (`src/spps/input_output/reportmanager.cpp:169-247`)
computes the chord length through the sphere (`Lintersect`, via `RaySphere`,
`reportmanager.cpp:26-52`), and adds `energy = particle_energy * Lintersect` into
`currentRecp->energy_sum[freqIndex][particleInfos.pasCourant]` (`reportmanager.cpp:224`) —
**the same estimator, with the same weighting, for the direct path as for every reflected
path.** There is no separate "this is the direct arrival" code path in the main SPPS loop.

### 2.2 The `direct_calc` mode — a different simulation, not a correction

`IPROP_DO_CALC_CHAMP_DIRECT` (config.xml property `direct_calc`,
`src/spps/data_manager/core_configuration.cpp:61`) is read inside `CalculationCore::Movement`
(`src/spps/CalculationCore.cpp:236-243`): when true, **every particle is killed at its first
wall collision** (`energie=0; stateParticule=PARTICULE_STATE_ABS_SURF; return;`). This turns
SPPS into a direct-field-only solver for one whole run — it is not a term added on top of a
normal run's decay curve; it is a separate config the user runs separately if they want a
direct-field export. Nothing in `OnMenuDoAcousticParametersComputation` reads or combines a
`direct_calc` run with a normal run.

### 2.3 Contrast: CTR does have an explicit analytic direct field

The **other** solver, CTR/TCR (`src/ctr/TC_CalculationCore.cpp:160-196`), computes an
explicit `Get_Champ_Direct_Lin` (linear direct field from source power and 1/r²-type
geometry) and adds it to the reverberant level: `Get_Recepteur_P_Champ_Total(ChampDirectLineaire, L_lin)`
(`:185-187`). So upstream *does* know how to keep a direct term separate and explicit — just
not in the SPPS histogram/EDT pipeline this task is about.

### 2.4 Net effect on I2 (direct-sound bin weighting)

There is no dedicated weighting for the direct-sound bin in the SPPS→EDT/T/C/D/Ts pipeline.
It is captured by the same chord-length estimator as any reflection (fine, in itself), but:
- for **EDT/T15/T30** it sits inside a fit window that (per §1.3/§6 I1) is anchored at the
  *first recorded bin*, not at its own arrival time, so pre-arrival silence gets folded into
  the regression;
- for **C/D/Ts/ST** it is correctly used as the window's own origin (`wt0`), but that origin
  is found by an *absolute* raw-energy threshold (`1e-18`) with no noise-floor scaling — a
  single stray particle in an earlier "should be silent" bin (Monte Carlo noise, §6 I5) moves
  `wt0`, and therefore every one of these four parameters' windows, to that stray bin.

---

## 3. Run length, time step, trans_epsilon, and energy after the run ends

### 3.1 Where each is set, and its GUI default

| Config.xml property | Meaning | Consumed at | GUI default | Bound enforced? |
|---|---|---|---|---|
| `duree_simulation` → `FPROP_SIMULATION_TIME` | total simulated time (s) | `base_core_configuration.cpp:93` | **2 s** (`e_core_core_config.h:73`, `AppendPropertyDecimal("duree_simulation", "Simulation length (s)", 2, ..., isMinValue=true, minValue=0.0001, ...)`) | floor of 1e‑4 s only; no maximum, and nothing ties it to expected T30 |
| `pasdetemps` → `FPROP_TIME_STEP` | histogram/particle time step `dt` (s) | `base_core_configuration.cpp:92` | **0.01 s (10 ms)** (`e_core_core_config.h:74`, `AppendPropertyDecimal("pasdetemps","Time step (s)",0.01f,...,isMaxValue=false,isMinValue=false,...)`) | **none** — `isMaxValue`/`isMinValue` both `false` |
| `IPROP_QUANT_TIMESTEP` | number of bins, `N = ceil(duree_simulation/pasdetemps)` | `base_core_configuration.cpp:94` | `ceil(2/0.01) = 200` bins at defaults | derived, no independent check |
| `trans_epsilon` → `FPROP_EPSILON_TRANSMISSION` | particle-kill threshold, **in the exponent of 10**: a particle dies once its own energy has fallen to `initial_energy * 10^-trans_epsilon` | `sppsNantes.cpp:75`: `energie_epsilon = energie * pow(10, -trans_epsilon)` | **5** → particle dies at **−50 dB relative to its own starting energy** (`e_core_sppscore.h:52`, label literally says "ratio 10^n") | none |
| `rayon_recepteurp` | receiver sphere radius (m) | `core_configuration.cpp:64` | 0.31 m | min `EPSILON` only |

Unit-test config for reference (not a shipped default, but a real receipt of what upstream
itself runs in CI): `src/spps/tests/config.xml` — `duree_simulation="0,100000"` (0.1 s!),
`pasdetemps="0,000500"` (0.5 ms), `trans_epsilon="5,000000"`, `temperature="20,000000"`.

### 3.2 What `trans_epsilon` actually bounds — and what it does not

`trans_epsilon` only applies in the **Energetic** computation method
(`computation_method` — in **Random** mode particles are killed by a stochastic absorption
draw instead, `CalculationCore.cpp:287-303`, and `energie_epsilon` is unused there except for
the transmission-duplication guard at `:264,272`). In Energetic mode, each particle's energy is
multiplied by `(1-alpha)` at each wall hit and by the atmospheric-absorption survival
probability each step (`CalculationCore.cpp:57,141,284`); it is killed
(`PARTICULE_STATE_ABS_ATMO` / `_ABS_SURF` / `_ABS_ENCOMBREMENT`) the moment its own energy
drops at or below `energie_epsilon` — **−50 dB below *that particle's own starting energy*,
not below the ensemble's late-time level or the receiver's noise floor.** With ~150,000
particles per source per band (default `nbparticules`, `e_core_sppscore.h:158`) surviving to
different times depending on how many/how absorptive the surfaces they hit were, this bounds
individual trajectory length for compute cost, not the dynamic range of the exported decay
curve.

### 3.3 What happens to energy after the run ends (I4)

The whole-particle loop: `while(stateParticule==ALIVE && pasCourant<confEnv.nbPasTemps)`
(`CalculationCore.cpp:49`). The moment `pasCourant` reaches `nbPasTemps` (i.e., the run's
clock runs out), the loop condition goes false and `Run()` falls straight into its
end-of-function `switch(configurationP.stateParticule)` (`:88-108`). If the particle was
still `PARTICULE_STATE_ALIVE` at that instant, it is counted only as
`reportTool->statReport.partAlive++` (`:91`) — **a single integer counter, with no receiver
attribution, no energy quantity, and no propagation into the exported histogram.** Whatever
energy that particle still carried simply stops existing in the record. There is:
- no "energy remaining in flight" bucket added to the last time bin,
- no analytic reverberant-tail extrapolation,
- no flag surfaced anywhere in the acoustic-parameters computation
  (`OnMenuDoAcousticParametersComputation` never reads `t_Stats`/`partAlive`) that says "N
  particles were still alive when the clock ran out, treat parameters that need more decay
  range with suspicion."

`GetColStats()` (`reportmanager.cpp:345-359`) does export `partAbsAtmo/_Surf/_Encombrement/_Loop/_Lost/Alive/Total`
per frequency band to a separate `statsSPPS.gabe` file (config property `stats_filename`,
`core_configuration.cpp:20`) — but that file is disconnected from the EDT/T/C/D/Ts pipeline
in §1; nothing cross-checks the two.

---

## 4. Guards on the decay curve — there are none

Read the whole call chain `Compute_TR_Param → GetTimeRange → ComputeLinearRegression →
GetSumLimit` (`projet_calculation.cpp:95-219`) end to end. There is:

- **No check that the curve ever crosses `fromdB`.** If it never does, `GetTimeRange`'s
  defaults stand (`debT=timeTable[0]`, `endT=timeTable[last]`) — a flat or non-decaying curve
  is silently fit over the *entire* recording as if that were the intended window.
- **No check that the curve reaches `todB` before the recording ends** (the "run too short"
  case). Same silent fallback to `endT=timeTable[last]`.
- **No check on regression sample count.** `ComputeLinearRegression`'s denominator is
  `n*Sx2 - Sx*Sx`. With `n==1` this is *identically* zero (`Sx2 = t² = Sx*Sx`), producing
  `a = NaN`, `b = NaN` — silently. `Compute_TR_Param` then writes
  `(-60/NaN)/1000 = NaN` straight into the `"EDT (s)"`/`"RT-x (s)"` column of the `.gabe`
  file with no check, no log message, nothing (verified directly — see §7.5, and
  `upstream_edt.py::degenerate_curve_demo`).
- **No sign check on the fitted slope.** If particle noise makes the local slope
  non-negative (curve flat or rising over the window — plausible at low SNR, or for a
  literally non-decaying/rising synthetic curve), `-60/a` is silently either negative,
  `+inf` (`a==0`), or a small positive number that reads as a plausible EDT while meaning
  the opposite of what it claims.
- **No R²/goodness-of-fit is computed or stored anywhere in this file.** The slope `a` and
  intercept `b` are the entire output of the regression; nothing downstream ever asks how
  well the line fit the points.
- **No distinction between "two sources" or "direct-only" receivers.** The pipeline has no
  branch for either case; whatever the histogram contains gets the same treatment.

---

## 5. What reaches the user

Per receiver, `OnMenuDoAcousticParametersComputation` writes one file,
`"Acoustic parameters.gabe"` (`projet_calculation.cpp:959-960`), with one column per requested
parameter and one row per frequency band + `"Global"` + `"Average"`
(`:881,901-902,908-957`). Every cell is a **single `float`** value
(`formatGABE::GABE_Data_Float`, backed by `Floatb`=`float`, `gabe.h:63`), formatted for
display with a fixed digit count and no unit of uncertainty:

- `COMMA_PRECISION_TIME_S = 2` decimal digits for EDT/RT (seconds) — `mathlib.h:43`
- `COMMA_PRECISION_TIME_MS = 1` for Ts — `mathlib.h:44`
- `COMMA_PRECISION_DB = 1` for level/C/ST — `mathlib.h:42`
- `COMMA_PRECISION_PERCENT = 1` for D — `mathlib.h:45`

There is **no range, no confidence interval, no per-band or per-parameter warning flag**,
anywhere in this file or in the GUI code that displays it (`e_report_gabe_recp.cpp`,
read in full — its only post-processing is the dB conversion and a Schroeder-curve plot,
`:53-277`; it does not touch the acoustic-parameters `.gabe` file at all, only the raw
receiver-level one). The separate `statsSPPS.gabe` particle-survival counts (§3.3) are never
joined to this output. A user opens "Acoustic parameters," sees e.g. `EDT (s) = 1.01`, and has
no way — from anything upstream computes or displays — to tell that value apart from one that
is off by 20% because of I1, or is `NaN`/negative because of a degenerate window, without
independently inspecting the Schroeder-curve plot by eye.

---

## 6. The seven issues, upstream-by-upstream

**I1 — fit origin (emission t=0 vs. direct-sound arrival; pre-arrival bins).**
Not handled for EDT/T15/T30. `GetTimeRange`'s reference is `row_db[0]`
(`projet_calculation.cpp:149`, first *recorded* bin, i.e. `t=dt`, not `t=0` and not the
direct-sound arrival), and for EDT specifically `fromdbL=0` makes the very first sample
satisfy `abs(0)>=0` trivially, so `debT` is *always* `timeTable[0]`
(`Compute_TR_Param(0,10,...)`, `:936`). Pre-arrival bins (silence before the direct sound
physically arrives) are included in the regression window whenever the receiver is far
enough that `r/c > dt`. **Is handled — but only for C/D/Ts/ST** — via `GetTimeDecay`
(`:127-139`), which the EDT/RT function never calls (§1.4). See §7 for the quantified bias
this causes.

**I2 — direct sound weighting.**
Not distinguished. The direct-sound bin is captured by the same chord-length
(`Lintersect`) estimator as any reflection (`reportmanager.cpp:219-224`), with no separate
weight, spike-isolation, or analytic correction inside SPPS. A distinct analytic direct
field exists only in the other solver, CTR (`TC_CalculationCore.cpp:160-196`), not in the
SPPS→EDT pipeline. For C/D/Ts/ST it correctly anchors the window (`wt0`) but via an absolute,
un-scaled noise threshold (`1e-18` on raw energy, `:357` etc.) that a single noisy early
particle can fool.

**I3 — time-step discretisation (energy's position inside a bin of width dt is unknown).**
Not handled, and not even approximated: `ReportManager::RecordTimeStep`
(`reportmanager.cpp:155-167`) and the receiver-collision code
(`reportmanager.cpp:196-246`) bin strictly by the integer `pasCourant`; the intra-bin time
(`elapsedTime`, `mu1/mu2` along the segment) is used to *find* the intersection but is
**discarded** for the purposes of which bin the energy lands in — only the bin index survives
to the exported histogram (and the label attached to the bin is `(idstep+1)*dt`,
`reportmanager.cpp:526`, i.e. the bin's end time, not the mean/weighted arrival time of the
energy inside it). This is a straightforward, un-remarked, up-to-`dt` timing error on every
single bin — including whichever bin happens to contain the direct sound.

**I4 — unrecorded late energy (post-run energy; particles killed below `-trans_epsilon` dB).**
Not handled on either count. Particles still alive when `pasCourant` reaches `nbPasTemps` are
folded into one aggregate `partAlive` counter (`CalculationCore.cpp:91`) with no energy or
receiver attribution and no propagation into the exported curve or a warning (§3.3).
Particles killed for crossing `energie_epsilon` (§3.2) simply vanish — no accounting of how
much energy was discarded this way is exposed per receiver, only aggregate counts in a
disconnected `statsSPPS.gabe` file that the acoustic-parameters computation never reads.

**I5 — particle noise at the receiver.**
Not modelled or reported at all — no variance, no particle count, no confidence interval is
computed or stored anywhere in `projet_calculation.cpp` or `reportmanager.cpp`. The chord-length
energy estimator (§2.1) is a Monte Carlo estimator with no accompanying error estimate. Its
practical severity is receiver-distance-dependent (more particles cross a distant receiver's
direct-path solid angle proportionally less often — a standard 1/r²-type falloff in
particle-tracing hit counts), which is exactly the regime where I1's bias is also largest
(§7): the two failures compound at long range rather than being independent nuisances.

**I6 — degenerate curves (truncated runs, direct-only, empty bins, non-decaying, two sources).**
Not handled — see §4 in full. Concretely reproduced in §7.5: a single-bin regression window
produces `NaN` written straight to the exported "EDT (s)" cell, no guard anywhere in the call
chain.

**I7 — display: one number, and (per decision-log row 9, "Show the range always") its range.**
Not handled. See §5: one `float`, fixed decimal precision, no range, no uncertainty, no flag,
anywhere upstream computes or displays these parameters.

---

## 7. Quantified error, on a simple synthetic case

### 7.1 The case, as specified

A single, noise-free (infinite-particle, i.e. this isolates I1/I3, not I5) synthetic energy
impulse response:

```
w(t) = 0                        for t < tau
w(t) = exp(-k (t - tau))        for t >= tau,   k = 6*ln(10)/EDT_true   (so the Schroeder
                                                  decay of this response has EDT = EDT_true
                                                  exactly, for a perfect single-slope decay
                                                  every T-parameter — EDT, T15, T20, T30 — is
                                                  the same number, by construction)
```

with `EDT_true = 1 s`, `tau = r/c` the direct-sound arrival time, `c = 343.2 m/s`
(20 °C, receipt: `Celerite_du_son.cpp:44-48`, `c = 343.2*sqrt(K/Kref)`, `Kref=293.15 K`
i.e. 20 °C exactly gives 343.2 m/s, matching the 20 °C default in
`src/spps/tests/config.xml`), for `r ∈ {2, 20} m` and `dt ∈ {10, 1} ms`.

Each histogram bin is filled with the **exact** integral of `w(t)` over its `[i·dt,(i+1)·dt)`
interval (this is what an infinite-particle SPPS run would produce — real SPPS approximates
this integral via chord-length-weighted particle hits, §2.1, so this isolates the *algorithm's*
bias from Monte Carlo noise). Bin `i` is labelled `(i+1)·dt` in ms, exactly matching
`reportmanager.cpp:526`.

### 7.2 Hand calculation

Because `EDT_true` fixes the *true* Schroeder slope at exactly `-60/EDT_true = -60 dB/s`, and
upstream's `GetTimeRange(0,10,...)` (§1.3/§6 I1) always sets `debT = timeTable[0] ≈ 0` for
EDT regardless of `tau`, the −10 dB endpoint `endT` is reached exactly `L = 10/60·EDT_true =
166.67 ms` of *real decay* after `tau` (the flat pre-arrival part contributes no dB drop, so
it doesn't move `endT`, only the window's *start* is wrong). The regression is fit over a
"flat at 0 dB for `[0,tau]`, then a straight line dropping to −10 dB over `[tau, tau+L]`"
curve, i.e. a broken-stick (hinge) function, not the true single straight line.

Treating the fit continuously (dt → 0; the discrete cases are confirmed against this in §7.3),
an ordinary-least-squares line through a flat-then-linear curve on `[0, T]` (`T = tau+L`) has,
by direct calculation of the OLS moments (`Var(t)=T²/12`, and the two-piece integrals for
`E[y]`, `E[ty]`):

```
a_hat = Cov(t,y) / Var(t),   Cov(t,y) = -k[ (L³/3 + tau·L²/2)/T - L²/4 ],   k = 60/EDT_true (dB/s)
EDT_measured = -60 / a_hat
```

(full derivation and the four resulting numbers are in `analytic_biased_edt()` in
`upstream_edt.py`). This predicts, independent of `dt`:

| r (m) | tau (ms) | continuum-predicted measured EDT (s) | vs. true 1.00 s |
|---|---|---|---|
| 2 | 5.83 | **1.0034** | **+0.3%** |
| 20 | 58.28 | **1.1999** | **+20.0%** |

The bias is a pure function of `tau/EDT_true` (how much of the fit window is silence): small
and almost negligible for a near receiver, **a full 20% overestimate of EDT for a receiver
20 m from the source**, growing without bound as `r` grows relative to `EDT_true·c`.

### 7.3 Confirmation by running the transcription

`upstream_edt.py`, run single-process, no workers, no multiprocessing (python.exe count
checked at 0 immediately before running, both times):

```
  f32   r(m)  dt(ms)  tau(ms)    debT    endT  measured EDT(s)  analytic EDT(s)  error(s)  error(%) #flat bins  arrival label err(ms)
-------------------------------------------------------------------------------------------------------------------------------------
False    2.0    10.0     5.83   10.00  190.00          1.00894          1.00336    0.0089      0.89          0                  4.172
False    2.0     1.0     5.83    1.00  174.00          1.00352          1.00336    0.0035      0.35          5                  0.172
False   20.0    10.0    58.28   10.00  240.00          1.20658          1.19986    0.2066     20.66          5                  1.725
False   20.0     1.0    58.28    1.00  226.00          1.20115          1.19986    0.2012     20.12         58                  0.725
```

(`f32=True` rows are within 1e-5 s of the `f32=False` rows — see §7.4.) The discrete
simulation and the continuum hand-derivation agree to within the residual discretisation
effect (a few tenths of a percent at `dt=10ms`, closing further at `dt=1ms`), i.e. **the
transcription confirms the hand calculation**: the r=20 m receiver's EDT comes back roughly
20% too long, purely from where upstream's own algorithm decides the decay curve "starts,"
with the sub-percent residual on top of that coming from ordinary bin quantization (I3).

`dt` itself barely moves the answer (0.89%→0.35% at r=2m going 10ms→1ms; 20.66%→20.12% at
r=20m) — confirming the bias is structural (which bin the fit *starts from*), not a
discretisation artifact; discretisation (I3) is a much smaller, second-order effect on top of it.

**I3 in isolation** — the "arrival label error" column above is exactly the up-to-`dt` timing
error described in §6/I3: at `r=2m, dt=10ms` the bin labelled "10 ms" actually captures energy
whose true arrival was at 5.83 ms — a 4.17 ms (72%-of-arrival-time) labelling error purely from
bin width; at `dt=1ms` this shrinks to 0.17 ms, as expected (scales with `dt`).

### 7.4 Transcription honesty

`upstream_edt.py` transcribes, statement-for-statement, the control flow, comparisons
(`>=` not `>`), the fixed unread-again reference `row_db[0]`, the inclusive bin selection in
`GetSumLimit`, and the closed-form OLS regression, of `MakeSchroederArray`, `GetSumLimit`,
`GetTimeRange`, `ComputeLinearRegression`, and `Compute_TR_Param`
(`projet_calculation.cpp:68-219`). It is **not** a bit-for-bit reproduction of the compiled
C++: Python's native `float` is 64-bit (C++ `double`), while every upstream local here is
declared `wxFloat32` (32-bit). The script's `f32=True` mode rounds every intermediate result
to `numpy.float32` after each arithmetic step to bound that gap — run head-to-head in §7.3,
the two modes differ by ≤ ~9e-6 s (< 0.001% of the effect being measured), confirming the 0.3%
– 20% biases reported above are the algorithm's, not a floating-point artifact. Two smaller,
known gaps versus the literal C++: (a) `GetSumLimit` is called four separate times per
regression exactly as upstream does it (once per `SUM_OPERATION`, re-walking the same index
range each time) — deliberately left in rather than optimized away, to match upstream's
control flow, not to flatter it; (b) upstream's `abs()` calls on `wxFloat32` arguments
(`GetTimeRange`/`GetTimeDecay`) are **not verified against the compiled binary** — if the
translation unit's `abs` resolved to `<cstdlib>`'s `int abs(int)` rather than a floating-point
overload, small (<1 dB) differences would silently truncate to zero, which would materially
change the threshold-crossing logic. This was not build-checked (the task forbids building
the solver, and `<cmath>` is included transitively by every math call in the file, making the
correct overload the likely — but not solver-build-confirmed — resolution on the project's
MSVC toolchain). This transcription assumes the correct floating-point `abs`, as upstream's
apparent intent does.

### 7.5 Degenerate-curve reproduction (I6)

```
I6 degenerate-curve demo (single-bin window -> denom=0, no guard in upstream):
  debT=100.0 endT=100.0 slope(dB/ms)=nan -> EDT written to the .gabe file = nan
```

A one-bin regression window (`n=1` ⇒ `Sx² = Σt² = (Σt)² ` identically ⇒ denominator `n·Sx²−Sx²=0`)
produces `NaN` with zero intervention anywhere in `Compute_TR_Param`/`GetTimeRange`/
`ComputeLinearRegression`. This is not a contrived edge case reachable only in theory: it is
exactly what a run cut short enough that only the run's very last bin satisfies `[debT,endT]`
would produce (e.g. `duree_simulation` set below what T30 needs at the shipped default
`pasdetemps=0.01s` — §3.1 — for any room whose real T30 exceeds `2s·(35/60)≈1.17s`, which is
most concert halls and many ordinary rooms).

---

## 8. Summary: what upstream actually does, in one pass per item requested

1. **EDT/T/C/D/Ts**: one shared file, `projet_calculation.cpp`; backward Schroeder
   integration in float32, dB'd bin-by-bin; EDT/T15/T30 fit `[debT,endT]` from an
   ordinary least-squares line on the dB curve, `debT/endT` found by comparing every bin
   to the **first recorded bin's own value** (not 0 dB at the direct arrival); C/D/Ts/ST
   instead anchor on a detected direct-sound arrival bin (`GetTimeDecay`) that EDT/T never
   uses. All arithmetic float32; the double-precision energy accumulator inside SPPS is
   downcast to float32 the moment it's written to disk.
2. **Direct sound**: recorded by the same chord-length energy estimator as any reflection,
   with no separate weighting inside SPPS; a genuinely separate analytic direct field exists
   only in the other solver (CTR), unconnected to SPPS's histogram/EDT pipeline.
3. **Run length/dt/trans_epsilon**: plain config numbers (`duree_simulation`, `pasdetemps`,
   `trans_epsilon`) with essentially no bounds enforced (a floor on `duree_simulation` only);
   `trans_epsilon` bounds each particle's own energy loss (default −50 dB from its own start),
   not the receiver's dynamic range; energy still in flight when the run clock expires is
   folded into one integer counter with no receiver or energy attribution and never reaches
   the exported curve.
4. **Guards**: none, anywhere in the regression call chain — not on curve shape, not on
   sample count (`NaN` is reachable and reproduced), not on slope sign, no goodness-of-fit.
5. **What reaches the user**: one float per band, two decimal digits, no range, no
   uncertainty, no warning flag, ever.

---

## Appendix — file:line index of every receipt used above

- `src/isimpa/data_manager/projet_calculation.cpp` — 41-52 (dB helpers, `p_0`), 68-83
  (`MakeSchroederArray`), 87-119 (`SUM_OPERATION`, `GetSumLimit`), 120-139
  (`LinearRegressionResult`, `GetTimeDecay`), 143-170 (`GetTimeRange`), 175-186
  (`ComputeLinearRegression`), 190-219 (`Compute_TR_Param`), 220-303 (`dB_Sum_Param`,
  `dB_Sum_Param_surf`, `dBa_Sum_Param`), 306-344 (`Compute_ST_Param`), 345-380
  (`Compute_C_Param`), 381-412 (`Compute_D_Param`), 413-443 (`Compute_TS_Param`), 792-998
  (`OnMenuDoAcousticParametersComputation`), 1001-1106 (surface-receiver equivalent,
  `OnMenuRecepteurSurfDoAcousticParametersComputation`, same functions reused).
- `src/isimpa/data_manager/element.h:339-344` — event IDs.
- `src/isimpa/data_manager/projet.cpp:543-547` — event dispatch.
- `src/isimpa/data_manager/tree_rapport/e_report_gabe_recp.cpp` — full file read; dB
  conversion + Schroeder-curve plot only, does not touch the acoustic-parameters file.
- `src/isimpa/resources/SystemScript/recp_res_tool/__init__.py:23` — the
  `IDEVENT_RECP_COMPUTE_ACOUSTIC_PARAMETERS` call site from the Python side, `{"TR":"15;30","EDT":"","D":""}`.
- `src/isimpa/data_manager/tree_core/e_core_core_config.h:70-75` — `duree_simulation`/`pasdetemps` GUI defaults.
- `src/isimpa/data_manager/tree_core/e_core_sppscore.h:49-55,158` — `trans_epsilon`,
  `rayon_recepteurp`, `nbparticules` GUI defaults.
- `src/lib_interface/data_manager/base_core_configuration.cpp:92-94` — `FPROP_TIME_STEP`,
  `FPROP_SIMULATION_TIME`, `IPROP_QUANT_TIMESTEP` load + derivation.
- `src/spps/data_manager/core_configuration.cpp:61-68` — `direct_calc`, `trans_epsilon`,
  `rayon_recepteurp` load.
- `src/spps/CalculationCore.cpp:41-111` (`Run`, incl. end-of-run `switch`), `113-354`
  (`Movement`, incl. `direct_calc` branch `:236-243`, energy-epsilon kills `:58,142,305`).
- `src/spps/sppsNantes.cpp:75` — `energie_epsilon = energie*pow(10,-trans_epsilon)`; `:91-93`
  — `pasCourant` initial value from source delay; `:333-357` — `nbTimeStep`/`nbPasTemps` wiring.
- `src/spps/input_output/reportmanager.cpp:155-167` (`RecordTimeStep`), 169-247
  (`ParticuleFreeTranslation`, receiver chord-length hit + bin assignment, line 224 is the
  key accumulation), 254-286 (`ParticuleCollideWithSceneMesh`), 345-359 (`GetColStats`),
  426-438 (`GetSumEnergy`, double→float downcast at 435), 482-529 (`SaveThreadsStats`, time
  labels `(idstep+1)*timeStep*1000` at line 526).
- `src/lib_interface/coreString.h:43` — `l_decimal`=double.
- `src/lib_interface/Core/mathlib.h:42-45,54` — `decimal`=float, display precisions.
- `src/lib_interface/input_output/gabe/gabe.h:63` — `Floatb`=float.
- `src/lib_interface/data_manager/data_calculation/Celerite_du_son.cpp:44-48` — speed of sound.
- `src/ctr/TC_CalculationCore.cpp:160-196` — CTR's explicit analytic direct field (contrast, §2.3).
- `src/spps/tests/config.xml` — real receipt of upstream's own test-run numbers
  (`duree_simulation=0.1s`, `pasdetemps=0.5ms`, `trans_epsilon=5`, `temperature=20°C`).
