# M8a judge, refs lens: the references and the transport cross-check, recomputed

Judged 2026-09-29 18:10 to 18:45 on the `m8a` worktree (report from build `da6f15c`, bed file sha256
`a9af34ae…b7f181`). Evidence read: `C:/tmp/nm-m8a-bed/20260929T093134Z/report.json`, `beds/m8a.json`,
the code under `crates/simpa-core/src/{bed,params,results}`, and upstream's `Coef_Att_Atmos.cpp`
(read only). Nothing from SPEC.md, RUN.md or the gate log was taken as evidence.

**Verdict: PASS.** Nothing blocking. Every reference in report.json agrees with my own computation to
within 2.9·10⁻⁵ relative, against a blocking threshold of 2·10⁻³. The transport shares no code with SPPS
and reads none of its output. Gate C recomputes exactly, and it still passes in all 32 gated cells when
my own transport replaces the bed's. RUN.md's explanation of A's offsets holds, and I can now say
where the offset comes from. I have one minor finding (E5's 0.6 % has almost no margin) and four notes.

Script: `refs_recompute.py`, beside this file. Output: `refs_recompute.json`. It is one Python process
using numpy 2.5.2 and scipy 1.18.1, and it imports no simpa code. The full run took 15 min 46 s on Grace
(18:22:38 to 18:38:24), most of it the transport. The transport is cached in
`C:/tmp/nm-judge-refs/transport_cache.json`, so `--no-transport` re-runs everything else in about 60 s.

## 1. γ², by my own two methods

**(a) Exact, from integral geometry.** For isotropic uniform random chords of a convex body,
`E[ℓ²] = (2/(π·S))·∫∫_{K×K} |x−y|⁻² dx dy`, and the mean chord is `4V/S`. This follows from
Blaschke–Petkantschin, `dx dy = |t₁−t₂|² dt₁ dt₂ dG`, with the line measure normalised so that the
lines meeting K have measure `πS/2`. I checked that normalisation on the sphere, where it gives
`∫σ⁴dG = 6V²`. For a box I wrote the pair integral through the covariogram,
`8∫₀ᵃ∫₀ᵇ∫₀ᶜ (a−x)(b−y)(c−z)/(x²+y²+z²)`. In spherical coordinates the radial integral has a closed form.
The two angles go by adaptive quadrature (`epsrel` 10⁻¹²), split where the exit face changes. The
same angular quadrature with `r²` in place of 1 gives `V²` to 3·10⁻¹⁶. Lambert reflection in a closed
convex room keeps the field uniform and isotropic, so its free paths are these chords.

**(b) Monte Carlo.** 10⁶ rays start in the stationary state (uniform on the surface, cosine
direction) and each is followed through 64 successive Lambert reflections, giving 6.4·10⁷ paths. The
standard error is taken over 40 independent batches of rays. This differs from the code's method,
which starts rays in the volume and discards 32 burn-in paths, and from the spec's `refs.py`, which
uses independent chords.

| Room | exact γ² | my MC γ² ± SE | report γ² ± SE | report − exact | effect on T_K |
|---|---|---|---|---|---|
| cube (check) | 0.344950 | 0.344976 ± 0.000090 | – | – | – |
| 6×10×3 | **0.388874** | 0.389030 ± 0.000101 | 0.388929 ± 0.000144 | +0.000055 (+0.38 SE, +0.014 %) | ≤ 1.6·10⁻⁵ |
| 5×4×3 | **0.352401** | 0.352419 ± 0.000071 | 0.352297 ± 0.000125 | −0.000105 (−0.84 SE, −0.030 %) | ≤ 2.9·10⁻⁵ |
| 20×8×4 | **0.410059** | 0.410232 ± 0.000137 | 0.409961 ± 0.000175 | −0.000098 (−0.56 SE, −0.024 %) | ≤ 2.8·10⁻⁵ |

- The standard errors are far under the 1 % asked for: my MC's is 0.02 to 0.03 % of γ², and the
  report's own is 0.04 %.
- The report's γ² (the code's `free_paths` on each run's `.cbin`/`.mbin`) is within 0.84 of its SE of
  the exact value in every room. My exact cube and box values reproduce the numbers the code's
  documentation quotes (0.344950, 0.388874, 0.352401), so those claims hold as well.
- My MC reproduces the mean free path to 4V/S within 0.004 %.

## 2. Kuttruff, plain Eyring and Sabine per cell and band

**Formulas used** (K = 24·ln 10/c with c = 343.2 m/s, SPPS's speed at 20 °C,
`Celerite_du_son.cpp`: `343.2·√(T/293.15)`; V = abc; S = 2(ab+bc+ca); ᾱ = α):

```
T_K = K·V / (4·m·V + A_K),  A_K = −S·ln(1−ᾱ)·[1 + (γ²/2)·ln(1−ᾱ)]     (gated reference, γ² exact from §1)
T_E = K·V / (4·m·V − S·ln(1−ᾱ))                                        (plain Eyring)
T_S = K·V / (4·m·V + S·ᾱ)                                              (Sabine)
```

**Source of T_K.** This is Kuttruff's γ²-corrected Eyring formula (H. Kuttruff, *Room Acoustics*,
the reverberation chapter's correction for fluctuating numbers of reflections). I did not open the
book, and the code's own citation is secondary: Stephenson, ICA 2016, eq. (21). I checked the
formula by deriving it instead. With free paths independent, the energy decays as
`E[(1−ᾱ)^N(t)]`. That decay is `e^(−s·c·t)`, where s is the root of the renewal equation
`(1−ᾱ)·E[e^(sℓ)] = 1`. Truncate `ln E[e^(sℓ)]` after its second cumulant, solve to first order in
γ², and the result is exactly A_K above. Air multiplies every path's energy by `e^(−mct)`, so the
air term `4mV` adds outside the wall term.

**Air term m.** I computed m two ways. The first is ISO 9613-1 eqs. (3)–(5) with Annex B for the
humidity, transcribed by me from the standard. The second is upstream's `Coef_Att_Atmos.cpp`,
transcribed by me from the C++ and converted with `m = α_dB·ln10/10`
(`base_core_configuration.cpp:114`). The upstream form is what SPPS computes from the run's
`config.xml` (T 20 °C, 50 %, 101325 Pa, `disable_absatmo_computation="0"`, checked in
`runs/5x4x3-a0.05-random-air-on/s1/…/solve/config.xml`).

| Band (Hz) | 125 | 250 | 500 | 1000 | 2000 | 4000 | 8000 |
|---|---|---|---|---|---|---|---|
| m, upstream form (1/m) | 1.012712e-4 | 3.016001e-4 | 6.282296e-4 | 1.074252e-3 | 2.277120e-3 | 6.832842e-3 | 2.425224e-2 |
| report `air_m_per_metre` / mine − 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| upstream / ISO − 1 | +0.006 % | +0.006 % | +0.009 % | +0.015 % | +0.024 % | +0.031 % | +0.033 % |

The report's m equals my transcription of upstream bit for bit. The SPEC §2.2 table holds the ISO
values (8 kHz 2.424e-2), within 0.035 % of the form the solver uses. As the spec says, the bed does
not read that table.

**Result, over all 32 gated cells × 6 or 7 bands (208 cell-bands):**

| Compared with report.json | worst \|relative difference\| |
|---|---|
| `kuttruff_s` against my T_K (my exact γ², my m) | **2.94·10⁻⁵** (5×4×3 α 0.4; entirely the γ² draw of §1) |
| `a.bands[*].reference_s` (gate A's reference) against my T_K | 2.94·10⁻⁵ (it is `kuttruff_s`) |
| `kuttruff_s` against my T_K computed with the report's own γ² and m | 5.3·10⁻⁸ |
| `eyring_s` against my T_E | 5.5·10⁻⁸ |
| `sabine_s` against my T_S | 5.0·10⁻⁸ |
| `kuttruff_s` against my T_K with ISO m in place of upstream's | 2.05·10⁻⁴ (8 kHz; the air form, not an error) |
| `kuttruff_mc_sd` against `|∂T/∂γ²|·SE(γ²)` | 3.5·10⁻⁸ relative |

The 8 reported 20×8×4 cells show the same: worst 2.8·10⁻⁵ for Kuttruff and 5.5·10⁻⁸ for Eyring.
Air-off examples, as mine against report.json:

| Cell | T_K mine | T_K report | T_E (both) | T_S (both) |
|---|---|---|---|---|
| 6×10×3 α 0.05 | 2.642353 | 2.642357 | 2.616000 | 2.683666 |
| 6×10×3 α 0.4 | 0.291647 | 0.291651 | 0.262679 | 0.335458 |
| 5×4×3 α 0.05 | 2.022020 | 2.022014 | 2.003745 | 2.055574 |
| 5×4×3 α 0.4 | 0.221102 | 0.221096 | 0.201201 | 0.256947 |
| 20×8×4 α 0.4 (reported) | 0.414225 | 0.414213 | 0.370841 | 0.473588 |

No reference differs from mine by more than 0.003 %, against the blocking threshold of 0.2 %.

## 3. The transport (gate C's reference)

**Where it is computed.** `bed/transport.rs::t30` calls `params::lambert::decay`, then `Decay::receiver_t30`,
then `params::decay::decay_time`, with the settings from `beds/m8a.json` `transport`. They are 16
replicas of `16·16384·α/0.05` rays, a duration of 1.4× plain Eyring (with the band's air), seed
`0x6d3863656c6c0000 + 1000·α`, c 343.2 m/s, radius 0.31 m and a 1 ms step. `check::needed_transports`
and `reference_of` use them, keyed on the room, α and m from the bed file.

**Independent of SPPS: yes.**
- The geometry is `Enclosure::shoebox(room.size_m)`, built from the bed file's room size, not from
  the run's mesh.
- The random stream is its own SplitMix64 (`params::noise::Rng`).
- m comes from `bed_air`, computed in Rust from the bed file's atmosphere.
- The arrival is `|q−s|/c ± R/c`, taken from the bed file's positions.
- No function on that path reads a run folder, `simpa results` output or any SPPS file. The bed traces
  it in its own phase (`run.rs::trace_transports`) before the cells are judged.
- E4 in the report holds it to the committed `HIGH` values within 1.86·10⁻¹⁰ s.

**What the transport does share (notes, not blocking):**
- **The T30 estimator.** SPPS's receivers and the transport's both go through `params::decay::decay_time`
  (Schroeder curve with the arrival model, then a −5 to −35 dB fit). So gate C cannot see a bias in
  that estimator. I measured it instead. My transport, read by my own estimator (a least-squares line
  through the Schroeder points at 1 ms bin edges between −5 and −35 dB after the direct sound),
  agrees with the bed's (table below).
- **The ray tracer behind γ².** `params::lambert` produces both the transport and A's γ²
  (`free_paths`), so A's and C's references are not independent of each other. §1's exact γ² settles
  the γ² side of that.

**My own transport.** Written in numpy for the box. It uses the same physics (point source; Lambert
walls with `(1−α)` per reflection; air as `e^(−mct)` per bin; energy × path length inside 0.31 m
balls, spread over the crossing time). None of the code is shared: PCG64 random numbers, rays
started isotropically at the point source, the time bins filled by my own exact partial-bin split,
and my own T30 fit. It ran 16 replicas of `10⁶·α/0.05` rays
(1 to 8 M a cell, a quarter of the bed's count) for the 8 (room, α) pairs. The air-on bands are the
same rays with the band's air applied, cut at 1.4× that band's Eyring time, as the bed does.

| Room α (air off) | own receivers T30 | bed `transport_t` | own/bed − 1 | z | own room T30 | bed room T30 | z |
|---|---|---|---|---|---|---|---|
| 6×10×3 0.05 | 2.646971 ± 0.000649 | 2.647682 ± 0.000411 | −0.027 % | −0.93 | 2.647535 ± 0.000156 | 2.647489 ± 0.000061 | +0.28 |
| 6×10×3 0.1 | 1.304087 ± 0.000555 | 1.304672 ± 0.000243 | −0.045 % | −0.97 | 1.304386 ± 0.000077 | 1.304451 ± 0.000036 | −0.77 |
| 6×10×3 0.2 | 0.630892 ± 0.000252 | 0.631224 ± 0.000110 | −0.053 % | −1.21 | 0.631216 ± 0.000049 | 0.631201 ± 0.000023 | +0.28 |
| 6×10×3 0.4 | 0.291084 ± 0.000140 | 0.290834 ± 0.000069 | +0.086 % | +1.60 | 0.290957 ± 0.000025 | 0.290928 ± 0.000011 | +1.07 |
| 5×4×3 0.05 | 2.025382 ± 0.000435 | 2.025627 ± 0.000196 | −0.012 % | −0.51 | 2.025804 ± 0.000141 | 2.025814 ± 0.000050 | −0.07 |
| 5×4×3 0.1 | 0.997253 ± 0.000185 | 0.996933 ± 0.000122 | +0.032 % | +1.44 | 0.997008 ± 0.000051 | 0.996993 ± 0.000030 | +0.26 |
| 5×4×3 0.2 | 0.481007 ± 0.000125 | 0.481141 ± 0.000060 | −0.028 % | −0.97 | 0.481090 ± 0.000031 | 0.481062 ± 0.000016 | +0.80 |
| 5×4×3 0.4 | 0.219742 ± 0.000053 | 0.219812 ± 0.000026 | −0.032 % | −1.18 | 0.219786 ± 0.000014 | 0.219816 ± 0.000004 | −1.96 |

Over all 64 distinct transports (8 air off plus 56 air-on bands): receivers z mean −0.40, rms 1.02,
max 2.22; room energy z mean +0.14, rms 0.92, max 1.96. My transport and estimator agree with the
bed's within noise. The one hint of a pattern is receivers reading lower by about 0.02 %, 1/25 of
gate C's 0.5 %. My per-replica mean and my pooled-series T30 differ by at most 0.002 %, so neither
estimator has a small-count bias.

**Gate C recomputed** from report.json's per-seed T30s (`seeds[s].t30[r][b].t`) and its per-band
`transport_t`/`transport_se`:
`y_s = mean_{r,b} T_{s,r,b}/T_tr,b − 1`, `d = mean_s y_s`, `SE² = var_s(y)/10 + (mean_b se_b/T_b)²`,
interval `d ± t₀.₉₇₅,₉·SE`, `t₀.₉₇₅,₉ = 2.262157` (scipy; the report uses 2.2621571627982027).
The rule is re-implemented from SPEC §5.1 (PASS when `|d|+t·SE ≤ 0.5 %`; FAIL when the interval
excludes 0 and `|d| > 0.5 %`; otherwise INCONCLUSIVE).

All 32 gated cells, and the 5 of 8 reported cells that were judged, reproduce the report: per-seed
`y_s` to ≤ 4.4·10⁻¹⁶, and d, SE, interval and verdict identically. Of the gated cells:

| Cell | d mine | 95 % interval mine | reach | verdict mine / report |
|---|---|---|---|---|
| 5×4×3 α 0.05 random air on (widest) | −0.165 % | [−0.330, +0.000] % (hi +3.8·10⁻⁶) | 0.330 % | pass / pass |
| 5×4×3 α 0.1 random air on | −0.095 % | [−0.255, +0.065] % | 0.255 % | pass / pass |
| 5×4×3 α 0.4 random air on | +0.086 % | [−0.078, +0.251] % | 0.251 % | pass / pass |
| 6×10×3 α 0.2 random air on | +0.102 % | [−0.015, +0.220] % | 0.220 % | pass / pass |
| 6×10×3 α 0.05 random air on | +0.059 % | [−0.094, +0.211] % | 0.211 % | pass / pass |
| 5×4×3 α 0.4 random air off | +0.003 % | [−0.195, +0.201] % | 0.201 % | pass / pass |
| 6×10×3 α 0.4 energetic air off | +0.034 % | [−0.027, +0.094] % | 0.094 % | pass / pass |
| 5×4×3 α 0.05 energetic air off | −0.000 % | [−0.023, +0.022] % | 0.023 % | pass / pass |

There are no INCONCLUSIVE gated cells and no extension. The reported 20×8×4 α 0.2 random cell is
INCONCLUSIVE in both, d +0.745 % [−0.581, +2.071] %.

**Gate C against my transport in place of the bed's** (`gate_c_own_transport` in the JSON): all 32
cells PASS. The largest reach is 0.302 % (5×4×3 α 0.05 random air on, d −0.131 %) and the largest
|d| is 0.142 %. In one cell the interval excludes 0, 6×10×3 α 0.2 random air on,
d +0.142 % [+0.004, +0.280] %, which still passes by 0.22 %.

## 4. RUN.md's claim: A's offset is the formula-vs-transport gap

**The claim holds, with its size and sign.** Because `(1 + A_b) = (1 + c_b)(1 + g_b)`, where
`g_b = T_tr,b/T_K,b − 1` and `c_b` is C per band, A's band means equal the gap to within C. So the
test is whether (i) the gap has A's size and sign, (ii) C is centred on 0 per band and not only per
cell, and (iii) the gap is real rather than the bed's transport's own error.

| Room α (air off) | renewal/K − 1 | bed transport/renewal − 1 | **bed transport/K − 1** | **own transport/K − 1** | A cell mean, energetic | A cell mean, random | A − own gap (en / rnd) |
|---|---|---|---|---|---|---|---|
| 6×10×3 0.05 | +0.019 % | +0.183 % | +0.202 % | +0.175 % | +0.189 % | +0.210 % | +0.015 / +0.036 % |
| 6×10×3 0.1 | −0.019 % | +0.363 % | +0.344 % | +0.299 % | +0.324 % | +0.257 % | +0.025 / −0.041 % |
| 6×10×3 0.2 | −0.191 % | +0.608 % | +0.416 % | +0.363 % | +0.414 % | +0.459 % | +0.050 / +0.096 % |
| 6×10×3 0.4 | −1.110 % | +0.841 % | −0.280 % | −0.195 % | −0.246 % | −0.233 % | −0.052 / −0.038 % |
| 5×4×3 0.05 | −0.038 % | +0.216 % | +0.179 % | +0.167 % | +0.178 % | +0.167 % | +0.012 / +0.001 % |
| 5×4×3 0.1 | −0.086 % | +0.387 % | +0.301 % | +0.333 % | +0.306 % | +0.340 % | −0.027 / +0.007 % |
| 5×4×3 0.2 | −0.305 % | +0.660 % | +0.355 % | +0.327 % | +0.347 % | +0.326 % | +0.020 / −0.001 % |
| 5×4×3 0.4 | −1.442 % | +0.871 % | −0.580 % | −0.612 % | −0.580 % | −0.578 % | +0.033 / +0.035 % |

- **(i)** The transport sits +0.17 to +0.42 % above Kuttruff at α 0.05 to 0.2, and −0.20 to −0.61 %
  below it at α 0.4, in both rooms and by both transports. A has the same sign in every cell and the
  same size. Over the 32 gated cells (air off and on), the correlation of A's cell mean with my
  transport's gap is 0.984. A − gap averages +0.010 % (sd 0.057 %) with my transport and −0.002 %
  (sd 0.050 %) with the bed's. In energetic cells A − gap lies within −0.052 to +0.050 % (mine) and
  −0.020 to +0.034 % (bed's).
- **(ii)** Per band: `z_b = c_b/SE_b` over 104 random cell-bands has mean −0.01 and sd 1.10, with 9.6 %
  of |z| above 2. Over 104 energetic cell-bands it has mean −0.11 and sd 1.03, with 4.8 % above 2. The
  largest are 2.99 (5×4×3 α 0.05 energetic air on, 125 Hz, c_b −0.035 %) and 2.65 (5×4×3 α 0.1 random
  air on, 2 kHz, −0.257 %). That is noise, not a band pattern.
- RUN.md's extremes are random-mode noise on top of the gap:
  - +0.72 % is 6×10×3 α 0.2 random air on, 500 Hz: gap +0.417 %, c_b +0.301 % ± 0.164 %, z +1.83.
  - −0.76 % is 5×4×3 α 0.4 random air off, 250 Hz: gap −0.580 %, c_b −0.176 % ± 0.150 %, z −1.17.

  Random per-band SEs are 0.06 to 0.34 %; energetic ones are 0.008 to 0.053 %.
- **(iii) Why the gap exists**, from the renewal column, computed from 10⁷ chords per room with every
  cumulant kept and paths independent. Kuttruff's second-order truncation reads high as α grows
  (renewal/K −1.1 % and −1.4 % at α 0.4). Correlation between successive free paths makes the real
  decay slower than independent paths would (transport/renewal +0.18 to +0.87 %; my MC's lag-1
  correlation is 0.067 and 0.075, and the variance of a 64-path sum is 1.22 and 1.24 times γ²·K). The
  two partly cancel, and what is left is the gap A shows. The room energy gives the same gap as the
  receivers, and so does an independently written transport. The offset lies between the formula and
  diffuse transport, not in SPPS.

## Findings

1. **Minor: E5's 0.6 % has almost no margin.**
   - E5 accepts Kuttruff within 0.6 % of the transport in the 8 air-off cells. The worst cell is
     5×4×3 α 0.4 at +0.584 % against the bed's receivers (report `e5`), 97 % of the limit, while the
     transport's own SE there is 0.012 %.
   - The air-on bands of the same cell, reported only (R3), reach +0.599 % at 1 kHz
     (`kuttruff_against_transport`).
   - My independent transport gives +0.616 % ± 0.024 % on the receivers (own T30 0.219742 against
     K 0.221096) and +0.596 % on the room energy.
   - "Kuttruff within 0.6 %" (row 17 (7)) is therefore a draw-dependent statement at the bound, not a
     bound with margin. A is unaffected: it gates at 5 %, and the gap there is 0.58 %.
2. **Note: the T30 estimator is shared.** SPPS and the transport share `params::decay::decay_time`, so
   C tests transport physics, not the estimator. My estimator on my transport agrees with theirs within
   noise (§3): receivers read about 0.02 % lower on average, z mean −0.40 over 64.
3. **Note: γ² and the transport share `params::lambert`.** A's reference and C's reference are not
   independent of each other. The exact γ² in §1 closes that for γ² (0.014 % and 0.030 % of γ²; T_K
   within 2.9·10⁻⁵).
4. **Note: Kuttruff's accuracy is a cancellation, and it depends on the room.** At α 0.4 truncation
   (−1.1/−1.4 %) and path correlation (+0.84/+0.87 %) cancel down to −0.28/−0.58 %. In the reported
   20×8×4 room at α 0.4 the formula is +1.06 % above the transport's receivers and −0.19 % below its
   room energy, and the receivers' T30 is 1.2 % below the room's (0.40987 against 0.41502 s). SPPS follows the receivers (A −0.96 to
   −1.10 %). This is outside the gated rooms, but the 0.6 % does not carry over to other shapes.
5. **Note: the spec's 20×8×4 γ².** `refs.py` gives 0.4104 ± 0.0002, 1.7 SE above the exact 0.410059.
   The bed does not use it: it takes its own `free_paths` value, 0.409961.

## What this lens does not show

- It does not show that SPPS's own T30 values are read correctly from the runs. I took report.json's
  `seeds[*].t30` as given, and the raw decays are another lens.
- It does not show that the T30 estimator is right for SPPS-specific data (noise refusals, the floor,
  lost particles). I checked it only on transport-like decays, where my estimator and theirs agree.
- My transport shares the physical model with the bed's (ideal Lambert, a ball receiver weighted by
  path length, air per bin). It therefore cannot detect an error in that model itself: for example,
  SPPS and both transports could agree on a receiver model that differs from a physical microphone.
- It says nothing about gates A's and B's intervals, D (TCR), E1–E7 beyond E4/E5, N1–N8, or the
  atmospheric validation.
- The γ² and formula checks cover only boxes. For non-convex rooms the chord identity does not hold,
  and the code's `free_paths` would need another reference.
