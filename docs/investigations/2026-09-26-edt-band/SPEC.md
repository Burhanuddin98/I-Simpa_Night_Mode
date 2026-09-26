# A model-free band for EDT, Ts, C50, C80 and D50 (round 2)

For Burhan, 2026-09-26. This is the design and its proof, with a reference implementation and tests,
revised after the adversary's round. Everything is in `target/agents/edt-band/`. No solver was
run, and nothing tracked was changed. Round 1's code and every round-1 output, the adversary's
included, are kept unchanged in `round1/`.

- `band_early.py` is the reference implementation (numpy, float64).
- `test_band_early.py` holds tests T1–T7.
- `z3_cases.py` checks the method on the Z3 judge's confirmed cases, with strict containment.
- `bench.py` measures the cost.
- `attack1_*.py` are the adversary's scripts, re-run unchanged against this code.

Section 11 lists every output file.

**The answer in five lines.**
1. Every quantity gets an interval that holds its continuous-time value for every arrangement of energy inside the bins that the recorded sums allow. No shape is assumed.
2. C and D are exact in closed form. Ts is exact up to Dinkelbach's convergence, and every iterate is certified. EDT is a certified outer bound. Every edge is a supremum or infimum over the admissible set, which is reached only in the limit.
3. Round 2 fixes the one wrong lemma: the window-end intervals now satisfy `Ub ≤ 2·Ua`. It also fixes the crash, the Ts certificate, the premise on SPPS's arithmetic, the seed batch and the unbounded work. Section 0 lists each finding and its fix.
4. No truth fell outside its band in any test or re-run attack (section 9).
5. The model-free EDT band is still about `±(1.6–2.1)·dt/U` wide on exponentials, because that is the data's own ambiguity. EDT is therefore refused at τ = L almost everywhere at the 2 ms preset.

---

## 0. Round 2: each finding and what changed

| # | Adversary's finding | Verdict | Fix (method and proof) |
|---|---|---|---|
| P1 | Lemma W: `T3 ≥ 0` is false when `x = U − Ua > Ua` | Correct | Every window-end interval now has `Ub ≤ 2·Ua`. The initial cover is geometric, and bisection keeps the property. Under that precondition `T3 ≥ 0` holds (section 4.3). The E2 counterexample has `x = 5 ms > Ua = 1 ms`, which the method no longer allows. T7 checks the lemma on 4,000 curves. |
| P2 | Lemma W's upper bound is loose | Correct | For `x ≤ Ua` the exact bound is `T3 ≤ x·Ua·ℓ̃(Ua)/2`, and it is attained. G⁺ is now affine in x, and its end functional loses its `ln S(0)` terms. |
| P3 | The tightness figures are measurements, not bounds | Correct | Section 4.6 now states them as measurements. T6 adds an exact extremiser at a = 0 and reports the certified band's excess over the exact band. |
| P4 | Witnesses can sit far inside the true band when there is air | Correct | Section 4.6 now says so. The reported gap is an upper bound on the looseness, not an estimate of it. T5's climb shows how far the true band reaches. |
| P5 | `not_decaying` claims a non-decaying arrangement exists without proof | Correct | The reason is now "not ruled out", unless a witness actually is non-decaying; then that witness is the proof. |
| P6 | The seed batch `[mean lo_k, mean hi_k]` and the linear `±zσ` rule are wrong | Correct | The batch now takes the band of the pooled histogram. Noise is applied to the band's two edges, each treated as a statistic of the histogram (section 8). |
| P7 | P1 with ε = 2^−24 is not what SPPS delivers: the f32 energy chain, and the energy-kill rule | Partly wrong | SPPS keeps particle energy and bin sums in **double** (`sppsTypes.h:72`, `coreString.h:43`, `coreTypes.h:420`, `reportmanager.cpp:224`). Only the output file is f32 (`reportmanager.cpp:855`). So E3's f32 chain does not describe SPPS. The real double chain drifts by at most `(n+1)·2^−53`; replayed on E3's own step factors it drifts by at most 1.2·10⁻¹⁴ over 20,000 steps (`dev/e3_e4_receipts.*`). The rounding sources that are real (the double chain, the double sum of m_n contributions, f32 storage) are now bounded per bin by `spps_rel_eps` (section 1). The kill is real. It is outside discretisation, like Monte-Carlo noise, and section 1.3 explains why no bound on it can come from the histogram. The method accepts a bound on unrecorded energy anywhere in time (P4). |
| P8 | The extremes sit at closure points, so "exact" really means sup/inf | Correct | Lemma A and every theorem now state that the band edges are suprema and infima over the admissible set, reached only in its closure. |
| P9 | P2 and P3 assume one source and one celerity | Correct | New premise P5. A run with several sources or a celerity gradient refuses every quantity `premise_unsupported`. |
| P10 | The I-Simpa-compatible value exists only for EDT | Correct | Still open (section 10). Upstream's C, D and Ts go through `GetTimeDecay` and `GetSumLimit`, and no validated port of them exists. |
| I1 | Crash: `ZeroDivisionError` when `U_min = 0` | Correct | Lemma X now snaps `[U_min, U_max]` onto the support of the reading measure. Every window end is a point of that support, so `U_min > 0` unless the crossing can fall in a bin that holds the direct sound, which refuses. On the E1 histogram the band is now `[0.480, ∞)`, refused `not_decaying`, and a witness proves the refusal. The adversary's search found EDTs from 0.612 s up. |
| I2 | The Ts certificate mixed two Dinkelbach iterates | Correct | Each iterate is certified with the t at which F was evaluated, and the tightest certificate is kept. It also uses `S0_hi` when F has the other sign. T7 caps Dinkelbach at 1, 2 and 3 iterations: 0 of 180 capped bands fail to cover the converged one. |
| I3 | `z3_cases.py` used the truth's convergence gap as slack | Correct | Containment is now strict, and the gap is reported beside it. |
| I4 | The tests do not probe the extremes | Correct | New T5: two-atom splits with the curve held exactly on −10 dB, the rounding sign of every bin, and the late mass and tail. New T6: the exact band at a = 0. New T7: every edge case, run through the code. |
| I5 | Worst-case work was unbounded (about 2.5·10⁵ relaxations) | Correct | There is now a work budget. At most `R + 4·n0 + 4` relaxations run, each O(W), where `R = min(6000, max(64, 2·10⁶/W))` and `n0 ≤ 64` is the size of the initial cover. Any stop is valid, because every relaxation is a valid bound. The measured numpy worst case at the budget is ≤ 2.3 s at every step from 2 to 0.05 ms (`bench_log.txt`). |
| I6 | The adversary's own process-count breach | Theirs | No action. Round 2 ran every pool with at most 8 workers and nothing alongside it. |
| R2 | Found this round: each piece's upper tube edge `S_hi(A)` counted the previous bin's mass at its closure point A | Tightening; validity unchanged | Now `S_hi_open(A)`, which bounds `ρ((A, ∞))`, the right limit (Lemma L). Every EDT band is the same or tighter. |
| R3 | Found this round by T6: when the true supremum slope is exactly 0 (S constant up to U), the certified slope rounded to −2.6·10⁻¹³ dB/s, and the band's finite upper edge missed a non-decaying arrangement (T6 seed 148, `round2b_pre_margin/`) | Floating point | Two fixes (section 4.4). (a) Each chord end gets an outward margin of `10⁻¹⁰·Ua²·(1 + abs(ln φ_lo) + abs(ln S0_hi))`, which is 10⁻¹⁰ of J's natural scale and changes an ordinary slope by about 10⁻⁷ relative. (b) The band always contains every witness, since witnesses are admissible values, and a non-decaying witness forces `s_hi ≥ 0`. T4 and T6 log how often (b) fires. |

---

## 1. What is known, and what is not

### 1.1 The histogram and SPPS's arithmetic

SPPS writes `B[n]`, the energy recorded in step `n`, for `n = 0 … N−1`. Bin `n` covers `[n·dt, (n+1)·dt)` on SPPS's own time axis. Emission falls on a whole step (`sppsNantes.cpp:91`).

**The air, exactly as SPPS applies it.**
- At the start of every step, SPPS multiplies each particle's energy by `p = expf(−m·c·dt)` (`CalculationCore.cpp:57`, `base_core_configuration.cpp:115`).
- p is an f32. The energy is a double (`l_decimal`, `#define l_decimal double`, `coreString.h:43`), so the product is formed in double.
- Step `n` therefore carries `p^(n+1)` counted from emission, to within `(n+1)` double roundings.

The band's air rate is `a = −ln(p_f32)/dt`. With it, the per-step factor and the band's `e^{−a·dt}` are the same number. If the caller passes `m·c` instead, the difference is `|ln p_f32 + m·c·dt|` per step. The Z3 check shows this changes no verdict.

Energy that truly arrived at τ in bin `n`, with continuous air `e^{−aτ}`, was recorded with the factor `e^{−a((n+κ)dt − τ)}`, with κ = 1.

**Rounding.** A punctual receiver adds `energie·L` for each crossing into a double `energy_sum[n]` (`reportmanager.cpp:223-224`). The file stores `f32(energy_sum·cdt_vol)` (`reportmanager.cpp:855`); `cdt_vol` is a constant, and every quantity here is invariant to scale.

With `m_n` contributions to bin n and at most `r` reflections per path, there are at most `k = m_n + n + r + 3` double roundings. Let `g = k·2^−53/(1 − k·2^−53)` (Higham's γ_k). Then the exact value `X_n` satisfies

`|X_n/B_n − 1| ≤ ε_n = (2^−24 + g)/(1 − g)`.

`spps_rel_eps(N, m, r)` returns this ε_n. For `m ≤ 10⁹` it is under `1.7·10⁻⁷`. The implementation takes ε per bin.

SPPS does not report `m_n`. A safe bound is `N_particles × (1 + the most reflections a particle can make in one step)`, because each straight segment crosses the ball at most once.

The adversary's E3 modelled the chain in f32, where it drifts by up to 5.1·10⁻⁶. Replayed as SPPS runs it, a double multiplied by the f32 p, it drifts by at most 1.2·10⁻¹⁴ over 20,000 steps (`dev/e3_e4_receipts.py`, `.json`).

### 1.2 The arrival

- The direct sound reaches the receiver ball during `[t_a − h, t_a + h]`, where `t_a` is the emission step plus `d/c` and `h = R/c`.
- Every reflected path is at least `d` long, so reflected energy arrives at `τ ≥ t_a − h`.
- An optional `t_refl_min ≥ t_a − h` can raise that floor: for example `emission + (d1 − R)/c`, the triangle-inequality bound over the face planes.

Both statements need one point source and one celerity (P5 below).

### 1.3 What the histogram does not hold

**Energy SPPS dropped.** In the energetic method, a particle whose energy falls to `energie_epsilon = E0·10^−trans_epsilon` is killed (`CalculationCore.cpp:58`, `sppsNantes.cpp:75`; the default trans_epsilon is 5, `e_core_sppscore.h:52`). Its later arrivals are never recorded.

The histogram cannot bound this energy. A killed particle would have added `energy × length inside the ball` at every later crossing. The only bound on that length is `c` × the remaining time, and it gives a useless bound: of the order of the whole recorded energy.

So the kill is a bias of SPPS's estimator, like its Monte-Carlo noise, not a discretisation effect. The band covers the energy SPPS propagated. P4 accepts a caller's bound on energy missing anywhere in time, if one is ever established. Raising trans_epsilon moves the kill beyond the dynamic range, which is the real fix.

**The random ("Aléatoire") method** kills particles at random instead of attenuating them (`CalculationCore.cpp:64`). P1 then holds for the expected histogram, not for a single run. Section 8 covers that case.

**Unrecorded energy.** P4 covers energy after the series end (a truncated series), and any other bounded unrecorded energy.

**What is not known:**
- where inside its bin any energy arrived;
- how the energy in the direct window splits into direct sound and reflections;
- the air factor of each piece of energy, which follows from where it arrived;
- where the unrecorded energy lies.

## 2. The admissible set, and what the band guarantees

**Definition A: the continuous-time value.** This is the ISO reading and the Z3 judge's truth A:
- t = 0 at the direct arrival `t_a`;
- the direct sound is an impulse at `u = 0`;
- reflected energy is read at its own time `u = τ − t_a`;
- energy that arrives before `t_a` is read at `u = 0`.

EDT, Ts, C and D are the ISO functionals of that reading `ρ`, a measure on `u ≥ 0`.

**Premises.** μ is the energy SPPS propagated to the ball, with continuous air.

| | Premise | Source |
|---|---|---|
| P1 | `X_n = ∫_{bin n} e^{−a((n+κ)dt − τ)} dμ(τ) ∈ [B_n(1−ε_n), B_n(1+ε_n)]` | SPPS, section 1.1 |
| P2 | Direct energy only in `[t_a − h, t_a + h]` | straight lines from a point source to a ball |
| P3 | Reflected energy only at `τ ≥ t_refl_min` (≥ `t_a − h`) | triangle inequality |
| P4 | Energy of μ that is not in B (after the series end, or any other) totals at most `T_max`, and arrives within `[t_u,min, t_u,max]`. The defaults are the series end and ∞, and `t_u,min ≥ t_a` | caller |
| P5 | One point source, and a medium of one celerity | scope; anything else refuses `premise_unsupported` |

P5 exists for two reasons.
- With a celerity gradient (`CalculationCore.cpp:80-83`, `OnChangeCelerite`), rays bend. The direct sound then leaves `[t_a − h, t_a + h]`, and reflected paths are no longer bounded below by `d/c`.
- With several sources, the direct arrival is not unique, so Definition A's `t = 0` is undefined.

**Per-bin options.** The possible true readings of bin `n` are the mixtures of single atoms. Write `θ` for true energy over recorded energy:
- **u = 0 option.** True mass `B·θ`, with `θ` anywhere in `[θ0_lo, θ0_hi]`, the values of `e^{a((n+κ)dt − τ)}` over τ in the bin's part of the direct window.
- **Reflected option.** An atom at `u ∈ [r0, r1) = [max(t_n, t_a, t_refl_min) − t_a, t_{n+1} − t_a)`, with true mass `B·θ(u)`, where `θ(u) = e^{a(c_n − u)}` and `c_n = (n+κ)dt − t_a`.
- In both options, `B ∈ [B⁻, B⁺] = B_n·[1−ε_n, 1+ε_n]`.
- **Unrecorded energy.** An atom of true mass in `[0, T_max]` anywhere in `[v0, v1] = [max(t_u,min, t_refl_min) − t_a, t_u,max − t_a]`, with θ = 1.

**Lemma A (representation).**
- For every μ satisfying P1–P5, and every split of the window energy into direct and reflected, the reading ρ is a sum over bins of mixtures of these atoms, plus unrecorded atoms.
- Conversely, every such mixture is the reading of some admissible μ.
- The admissible set's closure adds atoms at `u = r1`, a point that physically belongs to the next bin.

*Proof.* Take bin `n` and energy dμ at τ. It was recorded as `e^{−a((n+κ)dt−τ)} dμ`. Its reading is `u = 0` if it is direct or `τ ≤ t_a`, and `u = τ − t_a` otherwise. P2 and P3 restrict τ to the two ranges above. P1 puts the bin's recorded total in `[B⁻, B⁺]`. Mixtures of atoms are exactly the measures on those ranges. ∎

- The per-bin set is convex, and its extreme points are single atoms.
- The `u = 0` option covers both readings of window energy, direct or reflected before `t_a`. So the band contains truth A whatever the split.
- Energy that P2 and P3 exclude is detected, and every quantity is refused `arrival_misfit`.

**The guarantee.** For every admissible ρ, `Q(ρ) ∈ [lo, hi]`. Each edge is the supremum or infimum of Q over the admissible set. For C, D and Ts the edges are exactly those values. For EDT they are certified outer bounds. An edge may be reached only by a limit of admissible arrangements. The band covers discretisation only; section 8 combines it with Monte-Carlo noise.

**Lemma P (separable linear optimisation).** For any ψ, the supremum of `∫ψ dρ` over the admissible set is

`Σ_n sup_{B∈[B⁻,B⁺]} B · sup_options θ·ψ + sup_{T∈[0,T_max], v∈[v0,v1]} T·ψ(v)`,

and single atoms approach it.

*Proof.* Write `∫ψ dρ_n = B·∫θψ dr`, with `r` a probability measure on the options. This is at most `B·sup θψ`, and atoms at maximising points approach it. Choosing `B` is then a sign test. ∎

## 3. C_te and D_te: exact, closed form (Theorem CD)

Write E for the early energy `ρ([0, te))` and L for the late energy `ρ([te, ∞))`. Then `D = E/(E+L)` rises with E and falls with L, and `C = 10·lg(D/(1−D))` rises with D.
- A bin can be early if it has the `u = 0` option or `r0 < te`, and late if `r1 ≥ te`.
- Unrecorded energy can be early if `v0 < te`, and late if `v1 ≥ te`.

**Theorem CD.**
- **`D_hi` (sup):** every bin that can be early goes early, at its largest early θ and `B⁺`. Every other bin goes late, at θ(r1) and `B⁻`. The unrecorded energy is early at `T_max` if it can be; otherwise it is 0.
- **`D_lo` (inf):** every bin that can be late goes late, at θ(max(r0, te)) and `B⁺`. Every other bin goes early, at its least θ and `B⁻`. The unrecorded energy is late at `T_max` if it can be; otherwise it is 0.

*Proof.* Every admissible ρ has `E(ρ) ≤ E*` and `L(ρ) ≥ L*`: a bin that cannot be early puts all its mass, at least `B⁻θ(r1)`, into L. So `D(ρ) ≤ D(E*, L(ρ)) ≤ D(E*, L*)`. The arrangement described reaches this, in the closure. The lower edge is symmetric. ∎

- **Cost.** O(N), one pass.
- **Refusal.** C is `unbounded` when L can be 0. The round-1 refusal `truncated` for C and D is gone: a series that ends before te is covered exactly by its unrecorded-energy bound.

## 4. EDT (Theorem E)

### 4.1 The functional

Write `S(u) = ρ([u, ∞))` for `u > 0`; this excludes the mass at `u = 0`. `S(0)` is the total, and `φ = S(0)/10`.

S is non-increasing and left-continuous. EDT's range is `[0, U]`, where `U = sup{u > 0 : S(u) ≥ φ}`, and `S(U) ≥ φ ≥ S(U+)`.

**Lemma S (slope).** Let `J(U) = ∫_0^U (u − U/2)·ln S(u) du`. Then `slope = (10/ln 10)·12·J(U)/U³` and `EDT = −60/slope`. Since `∫_0^U (u − U/2) du = 0`, adding a constant to ln S leaves J unchanged, so `S(0)` enters only through U and φ.

*Proof.* This is the least-squares normal equation on an interval. ∎

### 4.2 The tube and the crossing range

**Lemma T (tube).** For `x > 0`:
- `S_lo(x)` sums `B⁻θ(r1)` over the bins whose every option is at `u ≥ x`.
- `S_hi(x)` is `T_max` plus the sum of `B⁺θ(max(x, r0))` over the bins with `r1 ≥ x`.
- `S(0) ∈ [S0_lo, S0_hi]`: the sums at every bin's least and largest θ, with `T_max` added to the upper one.

Every admissible ρ satisfies `S_lo(x) ≤ S(x) ≤ S_hi(x)`, termwise, because θ is monotone. Suffix sums make each bound O(1) after O(N) preparation.

**Lemma X (crossing range).** Unless it refuses, every admissible ρ has a well-defined window end `U ∈ [U_min, U_max]` with `U_min > 0`. Here:
- `Σ` is the closed support set: the union of `[r0, r1]` over the bins with reflected energy, and `[v0, v1]` when `T_max > 0`;
- `U_min = min{x ∈ Σ : x ≥ U_min_raw}`, where `U_min_raw = inf{x : S_lo(x+) ≤ φ_hi}`;
- `U_max = max{x ∈ Σ : x ≤ U_max_raw}`, where `U_max_raw = sup{x : S_hi(x) ≥ φ_lo}`;
- `φ_lo = S0_lo/10` and `φ_hi = S0_hi/10`.

*Proof.*
- **U lies in [U_min_raw, U_max_raw].** `S(U) ≥ φ ≥ φ_lo`, so `S_hi(U) ≥ φ_lo` and `U ≤ U_max_raw`. Also `S(U+) ≤ φ ≤ φ_hi`, so `S_lo(U+) ≤ φ_hi` and `U ≥ U_min_raw`.
- **U lies in the support of ρ on (0, ∞), and hence in Σ.** Suppose an open interval around `U > 0` carried no mass. Then S would be constant on `(U − η, U + η]`, so `S(U + η/2) = S(U) ≥ φ`, which contradicts U being the supremum.
- **U exists.** Suppose the method did not refuse. Then `S(0+) ≥ S_lo(0+) ≥ φ_hi ≥ φ`. The set `{u > 0 : S(u) ≥ φ}` could be empty only if `S(0+) = φ` and mass accumulated at `0+`. That needs a reflected range starting at `u = 0`, which only the arrival bin has. In that case `U_min = 0` and the arrival bin is a crossing candidate with a direct option, which refuses.
- **U_min > 0.** Every candidate bin has no direct option, so it starts after `t_a + h`, and `U_min ≥` its `r0 > 0`. ∎

**Refusals.**
- `range_not_reached`: `T_max ≥ φ_lo`.
- `range_in_arrival`, in four cases:
  - `S_lo(0+) < φ_hi`;
  - a crossing-candidate bin has a `u = 0` option;
  - Σ has no point in the range;
  - `U_max/U_min > 2^64`.

### 4.3 A window-end interval (Lemma W, corrected)

Fix `[Ua, Ub] ⊂ [U_min, U_max]` with `δ = Ub − Ua ≤ Ua`, that is `Ub ≤ 2·Ua`. Let `F` be the admissible ρ with `S(Ua) ≥ φ ≥ S(Ub+)`. Every ρ whose window ends in `[Ua, Ub]` is in F.

Let `ℓ̃(u) = ln(S(u)/φ)`, which is ≥ 0 on `(0, U]`, and let `x = U − Ua ∈ [0, δ]`. Write:
- `N_a = ∫_0^{Ua} (u − Ua/2)·ℓ̃ du`;
- `I = ∫_0^{Ua} ℓ̃ du`;
- `T3 = ∫_{Ua}^{U} (u − U/2)·ℓ̃ du`.

**Lemma W.** `J(U) = N_a − x·I/2 + T3`, and for `x ≤ Ua`:

`0 ≤ T3 ≤ x·Ua·ℓ̃(Ua)/2`.

*Proof.*
- **The identity.** Split `∫_0^U (u − U/2)ℓ̃` at Ua. On `[0, Ua]`, `u − U/2 = (u − Ua/2) − x/2`.
- **The bounds.** On `[Ua, U]` the weight satisfies `u − U/2 ≥ Ua − U/2 = (Ua − x)/2 ≥ 0`. There `0 ≤ ℓ̃(u) ≤ ℓ̃(Ua)`, because S is non-increasing and `S(u) ≥ S(U) ≥ φ`. So `0 ≤ T3 ≤ ℓ̃(Ua)·∫_{Ua}^{U}(u − U/2)du = ℓ̃(Ua)·x·Ua/2`. ∎

The upper bound is attained when S is constant on `[Ua, U]`; T7 finds equality to 10⁻¹⁶.

Without the precondition the weight is negative on `[Ua, U/2]`, and `T3 < 0` is possible. That is the adversary's E2 (`x = 5 ms`, `Ua = 1 ms`), which the method no longer allows.

For each ρ ∈ F, define two functions of x, both affine:
- `G⁻_ρ(x) = N_a − x·I/2 ≤ J(U)`;
- `G⁺_ρ(x) = N_a − x·I/2 + x·Ua·ℓ̃(Ua)/2 ≥ J(U)`.

Since `∫_0^{Ua}(u − Ub/2) du = −Ua·δ/2`, their end values are:
- `G±(0) = ∫_0^{Ua} (u − Ua/2) ln S du`;
- `G⁺(δ) = ∫_0^{Ua} (u − Ub/2) ln S du + (δ·Ua/2)·ln S(Ua)`;
- `G⁻(δ) = ∫_0^{Ua} (u − Ub/2) ln S du + (δ·Ua/2)·(ln S(0) + ln 0.1)`.

**The chord step.** Let `M⁺(0) ≥ sup_F G⁺(0)` and `M⁺(δ) ≥ sup_F G⁺(δ)` be certified bounds (section 4.4). For every ρ ∈ F:

`G⁺_ρ(x) = (1 − x/δ)·G⁺_ρ(0) + (x/δ)·G⁺_ρ(δ) ≤ chord⁺(x)`, the line through `M⁺(0)` and `M⁺(δ)`.

So `slope(ρ) ≤ (10/ln 10)·12·max_{x∈[0,δ]} chord⁺(x)/(Ua + x)³`. The ratio of an affine function to `(Ua + x)³` has at most one stationary point, `x = (s·Ua − 3m0)/(2s)`, so the maximum is exact. The lower bound mirrors this with `M⁻` and G⁻.

This argument works per arrangement, so it needs no convexity of `M±`. Round 1 got that step wrong.

### 4.4 The relaxation (Lemmas L, D, P)

**Lemma L (tangent and chord).** ln is concave, so for S in `[lo, hi]`:
- ln S is at most any tangent;
- ln S is at least the chord of ln over `[lo, hi]`.

Split `[0, Ua]` at bin edges and where the weight `u − Uw/2` changes sign. On each open piece `(A, B]`, the tube is valid on F:
- `lo = max(S_lo(B), φ_lo)`, because `S(u) ≥ S(B)` and `S ≥ S(Ua) ≥ φ` there;
- `hi = S_hi_open(A)`, an upper bound on `S(A+) = ρ((A, ∞))`: `T_max`, plus `B⁺θ(max(A, r0))` summed over the bins with `r1 > A`.

A bin whose range closes at A (`r1 = A`) cannot put energy into `(A, ∞)`, so it is left out. Every piece starts at a bin edge, which is the previous bin's closure point.

Round 1 used `S_hi(A)`, which counts that bin's mass. That was valid but loose: when the previous bin is the arrival bin holding the direct sound, the tube ratio went from about 1.3 to 8.4. On T7's wide series 53 the fix cuts the upper edge from 1.434 s to 0.846 s (`dev/wide_diag.py`, `dev/wide_probe.py`).

The rules:
- **Upper bounds:** the tangent where the weight is ≥ 0, the chord where it is < 0.
- **Lower bounds:** the reverse.
- `(δUa/2)·ln S(Ua)` takes a tangent.
- `ln S(0)` takes the chord over `[S0_lo, S0_hi]`.

By Fubini, `∫ w·c·S du = ∫ Ω(min(v, Ua)) dρ(v)` over `v > 0`, where Ω is piecewise quadratic. The result is a linear functional of ρ.

**Lemma D (the crossing constraints).** For every `λ ≥ 0`, `sup_F Λ ≤ sup_{admissible}[Λ + λ1·(S(Ua) − S(0)/10) + λ2·(S(0)/10 − S(Ub+))]`. The right side is separable (Lemma P).
- Every λ gives a valid bound. λ is searched by bisection on the subgradient sign (Danskin).
- Any tangent point is valid too, so every relaxation evaluated is a valid bound.

**Per-bin maximum.** On a bin's reflected range, ψ is piecewise quadratic, with jumps at Ua and Ub, and `θ = e^{a(c_n − u)}`.
- The extreme of `θ·q` on a quadratic piece q lies at an end, or where `q′ − a·q = 0` (at most two roots).
- At the jumps, the side that includes the jump is the better one in both directions, so it is evaluated.
- Bins read wholly after Ub have a constant ψ and are summed with suffix sums. A bin after Ub that also has a `u = 0` option is optimised on its own.
- The unrecorded energy (θ = 1) is evaluated at every piece end, the vertex, `v0`, `v1`, Ua, Ub and `Ub+`.

**Theorem E.** Over a cover of `[U_min, U_max]` by window-end intervals, each with `Ub ≤ 2·Ua`, Lemmas W, L, D and P give certified slope bounds. Then:

`EDT ∈ [−60/s_lo, −60/s_hi]`,

where `s_hi` is the largest certified upper slope and `s_lo` the least lower one. ∎

**Floating point.** The theorem holds in exact arithmetic. The implementation adds two safeguards:
- It widens each chord end outward by `η = 10⁻¹⁰·Ua²·(1 + |ln φ_lo| + |ln S0_hi|)`. The relaxation's value is a sum of terms of size up to `Ua²·(1 + |ln S|)`, so η is 10⁻¹⁰ of that.
- It widens the band to contain every witness.

Neither is interval arithmetic, so rounding is still not bounded formally (section 10).

**The cover and its refinement.**
- The initial cover is geometric: `[U_min·2^j, min(U_min·2^{j+1}, U_max)]`, `n0 = ⌈log₂(U_max/U_min)⌉ ≤ 64` intervals. It is one interval in every case except those with `U_max > 2·U_min`.
- Refinement bisects the interval that holds the active extreme, which keeps `Ub ≤ 2·Ua`.
- It stops when one of these holds:
  - the bound is within `2·10⁻⁴` of a witness;
  - the interval is narrower than `dt/1024`;
  - four splits recovered less than 10 % of the gap;
  - the work budget is spent.

### 4.5 Refusals specific to EDT

`not_decaying` when `s_hi ≥ 0`. Its reason is one of two:
- "an admissible arrangement (a witness) has a slope ≥ 0", when a witness is non-decaying: then the witness is the proof;
- otherwise, "the certified upper slope bound is ≥ 0: a non-decaying arrangement is not ruled out".

### 4.6 How tight it is (Proposition W, restated as measurement)

Every maximiser the relaxation returns is an admissible arrangement, or a limit of them. So is every mixture across a bisection bracket, because the per-bin sets are convex.

Their exact EDTs lie in the closure of the true band. So:

`[min witness, max witness] ⊆ closure(true band) ⊆ [lo, hi]`.

The reported gap, certified edge against best witness, is therefore an **upper bound on the looseness, not an estimate of it**. With air, the witnesses can sit well inside the true band: the adversary found admissible EDTs 19 % beyond a witness. T5's climb measures how far the true band actually reaches.

**Nothing here bounds the looseness.** The following are measurements:

| Where measured | What |
|---|---|
| T6 (exact band at a = 0, 240 series) | certified over exact, per edge (section 9.1): lower edge within 1.7·10⁻⁵, half-width at most 0.019 L above the exact one |
| `attack1_exact_a0` (800 scenarios) | certified over exact (section 9.3): half-width at most 0.022 L above the exact one |
| `bench_log.txt` (exponentials) | certified over witness |
| T5 and the adversary's searches | how far a climb reaches into the band |

Two regimes are loose, and both are refused by a wide margin:
- **Near-flat curves**, with EDT ≫ U: a small absolute error in the slope becomes a large relative error in EDT. At EDT around 10⁵ s, the 10⁻¹⁰ rounding margin alone accounts for about 1 %.
- **A dominant direct sound with a short window end**: T7's wide series, and the adversary's hard set. The arrival bin's energy may be direct (at u = 0) or reflected later in the same bin, so on the arrival bin's piece S is only known within a factor of about 7. The chord over that tube is the best lower bound that is linear in S, and it is loose when S sits just above the tube's floor.
  - On T7's wide series 53, the certified upper edge is 0.846 s against 0.538 s for the best arrangement found by T5's climb, the witnesses and the adversary's search (`dev/wide_diag.py`, `dev/wide_diag2.py`, `dev/wide_probe.py`). More splitting, more tangent passes and more λ rounds change nothing.
  - The remedy is to branch on the arrival bin's split between direct and reflected energy (section 10). It is not implemented.

## 5. Ts: exact up to convergence, certified at every iterate (Theorem TS)

`Ts = N/D`, where `N = ∫u dρ` and `D = ∫dρ`. Both are linear.

Let `F(t) = sup[N − t·D]`. It is convex and decreasing, and `Ts_hi` is its root. By Lemma P, F(t) is computed exactly per bin:
- θ(u)(u − t) is unimodal on `[r0, r1]`, with its peak at `u = t + 1/a`;
- the `u = 0` option contributes `−θt`;
- the unrecorded energy contributes `T_max·(v1 − t)`, when that is positive.

**Certificate at every iterate.** For any t, every ρ has `N − t·D ≤ F(t)` with `D ∈ [S0_lo, S0_hi]`. So:
- `Ts ≤ t + F(t)/S0_lo` if `F(t) ≥ 0`;
- `Ts ≤ t + F(t)/S0_hi` if `F(t) < 0`.

The implementation takes each certificate with the same t at which F was evaluated, and keeps the tightest. Stopping at any iteration therefore gives a valid band.

Dinkelbach's step `t ← N(ρ*)/D(ρ*)` converges superlinearly: 3–6 steps here, and at its root the certificate equals the supremum. The minimum is symmetric.

Ts is refused `truncated` when `T_max > 0` and there is no latest arrival time for the unrecorded energy.

## 6. Degenerate cases, point value, refusal

| Code | When |
|---|---|
| `premise_unsupported` | several sources, or a celerity gradient (P5) |
| `arrival_misfit` | more than `misfit_tol` (10⁻⁶) of the energy lies where no direct sound and no reflection can arrive |
| `range_in_arrival` | the −10 dB point cannot be kept away from the arrival (section 4.2) |
| `range_not_reached` | the unrecorded energy may hold 10 % of the total |
| `not_decaying` | the certified upper slope is ≥ 0 (section 4.5) |
| `truncated` | Ts with unrecorded energy and no latest arrival time |
| `unbounded` | C when the late energy can be 0 |
| `band_too_wide` | the half-width exceeds τ |

**Point value.**
- **EDT:** `v = 2·lo·hi/(lo+hi)`. It minimises the largest relative error to any point of the band, and that error, `(hi−lo)/(hi+lo)`, is the reported relative half-width.
- **Ts, C, D:** the midpoint, with an absolute half-width.

**Refusal.** A value is refused only when its half-width exceeds τ, a parameter. The defaults:
- EDT: 0.5 %;
- C: 0.1 dB;
- D: 0.005;
- Ts: `min(1 ms, 0.5 % of Ts)`, the rule of record in `decay.rs limits`.

Half-widths are reported in %, in L and in JND.

**The I-Simpa-compatible value** is upstream's exact algorithm, through the validated port `upstream-edt/uphunt_upstream_edt.py`, imported read-only. It is reported next to the EDT band, never used as the value.

## 7. Cost

| Quantity | Algorithm | Cost | Exact? |
|---|---|---|---|
| C, D | Closed form (Theorem CD) | O(N), one pass | Yes (sup/inf) |
| Ts | Dinkelbach on per-bin closed forms | O(N) per step, 3–6 steps | Yes at convergence; certified at every step |
| EDT | Theorem E | O(N) preparation, then O(W) per relaxation, with at most `R + 4·n0 + 4` relaxations | Certified outer bound |

- `W` is the number of bins up to `U_max`. Bins after it are aggregated in O(1).
- `n0 ≤ 64` is the size of the initial cover, and is 1 unless `U_max > 2·U_min`.
- `R = min(max_relax, max(64, max_work/W))` is the budget. The defaults are `max_relax = 6,000` and `max_work = 2·10⁶` bin-relaxations.

**The worst case is now bounded.**
- Each λ search is capped at 40 doublings and 18 bisections, and the tangent pass runs at most twice.
- The budget R stops refinement and every λ search once it is spent.
- The initial cover always completes, at one relaxation or more per bound.
- Every relaxation is a valid bound (Lemma D), so the band is valid wherever it stops.

The worst-case work is therefore `O(N + min(6000·W, 2·10⁶ + 64·W) + (4·n0 + 4)·W)`: O(N) plus a constant for any W up to 31,250 bins, and O(W) beyond that. The measured counts and times are in section 9.4. The Rust figures there are estimates; nothing was run in Rust.

**Fastest exact algorithms found.**
- C and D: the closed form.
- Ts: Dinkelbach.
- EDT with no air and no rounding: the layer-cake argument gives a finite exact algorithm for a fixed window end: one atom per bin at an end, at `u = 0` or at `clip(U/2)`, and two atoms in the crossing bin. T6 uses it as the reference.
- EDT with air: none is known, because ln S couples the bins non-convexly. Theorem E is the fastest certified method found.

## 8. How the band combines with Monte-Carlo noise

The band maps a histogram B to `[lo(B), hi(B)]`. The target is the continuous-time value of the **expected** echogram μ̄. Its histogram is `B̄ = E[B]`, since recording is linear. So:

**`Q(μ̄) ∈ [lo(B̄), hi(B̄)]`, exactly.** This needs P1 for B̄, which holds in both of SPPS's methods: exactly in the energetic one, and in expectation in the random one.

A run gives `lo(B)` and `hi(B)`, not `lo(B̄)` and `hi(B̄)`. The two edges are statistics of the histogram, like any histogram-derived value. **The existing noise model is applied to each edge, unchanged:**

`Q(μ̄) ∈ [lo(B) − z·σ_lo, hi(B) + z·σ_hi]`, with a failure rate of at most two one-sided tails.

- **Seed batch (SB-2 … SB-8).** Take the band of the **pooled histogram** `B_pool = (1/k)·Σ B_k`, not the mean of the per-seed edges. B_pool is itself a histogram of the pooled particles, so P1 holds for it. Take σ_lo and σ_hi by SB-5 from the per-seed edges `lo(B_k)` and `hi(B_k)`: `U = s·√((k−1)/χ²)/√k`, floored as SB-5 says. To first order, the standard error of `lo(B_pool)` is that of the mean of the `lo(B_k)`. That is the same delta-method approximation SB-5 already makes; its averaging bias is of order sd², as in SB-7.
- **A single run.** The round-4 model is calibrated for T20 and T30, not for EDT's band edges (decision D8). Until it is calibrated for them, a single run's EDT is refused on noise by the existing rule (`noise_uncalibrated`), whatever its band.
- **The kill bias** (section 1.3) belongs with the noise, and is not in either term.

Each refusal keeps its own code: `band_too_wide` for discretisation, and the existing `monte_carlo_noise` and `noise_uncalibrated` for noise.

## 9. Evidence (all Python; no solver)

Every figure below is from the final code, `band_early.py` as it stands. Pools ran with at most 8 workers, one at a time, and nothing ran alongside them.

### 9.1 Tests (`test_band_early.py`)

Output: `run_all_stdout.txt`, `test_log_t1_t2_t3_t4_t5_t6_t7.txt` and `results_t1.json` … `results_t7.json`. Result: **0 failures, EXIT 0.**

| Test | What | Result |
|---|---|---|
| T1 | Exact exponential, closed-form truth: dt 2/1/0.5/0.2/0.1 ms × no air, 1, 16, 20 kHz × impulse or SPPS ball pulse × with or without `t_refl_min` × f64 or f32 | 160 rows, 0 truths outside |
| T2 | Single impulse: a lone reflection, the direct sound alone, one on a decay, energy before the arrival | 32 cases, 0 outside; refusals as designed |
| T3 | Two impulses straddling a bin edge | 90 cases, 0 outside |
| T4 | 400 random series, a third truncated with a rigorous tail bound, 40 arrangements each at 1 µs, 5 quantities | 80,000 checks, 0 outside. The rounding guard fired in 0 of 400 |
| T5 | Climb: two-atom splits held on −10 dB, eps signs, late mass and tails, on 40 series with eps up to 10⁻² | 25 climbed, 15 skipped (14 at dt < 0.5 ms, 1 refused). 0 escapes. Upper reach: median 0.997 of the band, least 0.896. Lower reach: median 0.001, most 0.038. The batched evaluator matches `exact_params` to 4.4·10⁻¹⁶ |
| T6 | Exact band at a = 0, ε = 0 (layer-cake extremiser, explicit arrangements), 240 series | 239 exact bands, **0 violations**. Certified over exact, relative: lower edge median 4·10⁻⁹, max 1.7·10⁻⁵; upper edge median 1·10⁻⁸, max 2.4·10⁻² (at EDT ≈ 4·10⁵ s, near-flat). Half-width excess over the exact half-width: median 0.000 L, 90th percentile 0.012 L, max 0.019 L. Guard fired in 0 of 240 |
| T7 | Edge cases through the code | 6 of 6 pass (below) |

T7 in detail:
- **E1 histogram.** Band `[0.480, ∞)`, refused `not_decaying`, with a non-decaying witness as proof. The adversary's search found 0.612–2213 s.
- **Lemma W** on 4,000 random curves with `x ≤ Ua`. `J − G⁻ ≥ 7·10⁻⁷·U²`. `G⁺ − J ≥ −1·10⁻¹⁶·U²`: attained, to rounding.
- **Capped Ts.** Dinkelbach capped at 1, 2 and 3 iterations: 0 of 180 bands fail to cover the converged band.
- **Per-bin ε with unrecorded energy.** ε from 10⁻⁴ to 2·10⁻², and up to 6 % unrecorded energy allowed anywhere from `t_a` + 2–80 ms: 6,000 checks, 0 outside.
- **Two sources.** Every quantity is refused `premise_unsupported`.
- **U_max > 2·U_min.** 16 series. 8 are refused `not_decaying`, and the 8 climbed have 0 escapes.

**EDT acceptance at τ = L (T4):**

| Step | Accepted | Median half-width |
|---|---|---|
| 0.1 ms | 38 of 74 | 0.89 L |
| 0.2 ms | 29 of 85 | 1.26 L |
| 0.5 ms | 0 of 77 | 4.64 L |
| 1 ms | 0 of 94 | 10.33 L |
| 2 ms | 0 of 68 | 22.65 L |

Upstream's EDT is inside the band in 34 of 399 series. Its largest error against an arrangement has a median of 36.5 %.

### 9.2 The Z3 judge's cases (`z3_cases.py`, strict)

Output: `z3_cases.json`, `z3_cases_log.txt`, `dev/z3_cases_stdout.txt`.

- 230 confirmed W1G false accepts, rebuilt with the judge's own generator.
- **Truth A is inside the band in 230 of 230, strictly**, in both variants.
- Least margin of a truth to a band edge: EDT 2.754 L (median 11.4 L); Ts 0.087 L (median 0.22 L).
- All 230 are refused `band_too_wide`. EDT half-width: median 16.8 L, least 5.5 L.
- Upstream's EDT on the same runs is off by a median of 43.5 L, at most 669.6 L, and is within L in 1 of 168.

### 9.3 The adversary's attacks, re-run unchanged against the fixed code

Summary: `dev/summarise_round2.py` → `dev/summary_round2.json` and `dev/summary_round2_stdout.txt`. Round 1's outputs are in `round1/`.

| Script (arguments as in round 1) | Round 1 | Round 2 |
|---|---|---|
| `attack1_exact_a0.py 800 8` | 794 exact, 0 violations; upper edge over exact up to 235 % (seed 53), 4.75 % (642), 0.455 % (363) | 794 exact, **0 violations**. Upper edge over exact: median 9·10⁻⁹, max 1.44 % (sparse, EDT ≈ 2.8·10⁵ s). Seeds 53, 642 and 363 are now at 0.23 %, 0.012 % and 0.013 %. Half-width excess: max 0.022 L, against 0.362 L in round 1 |
| `attack1_witness.py` (6 seeds) | 12 witnesses admissible | **12 of 12 admissible**. Each band's lower edge equals the exact one to 10⁻⁵ |
| `attack1_lp.py 96 8 48` | 762 optima, 0 violations | 768 optima, **0 violations**: more bands, since C and D no longer refuse `truncated`. Edges match the LP to a median of 10⁻¹⁵ (Ts, D50) |
| `attack1_z3.py 8` (217 robust, 3 variants) | 0 outside, strict; least margin 2.759 L (EDT), 0.087 L (Ts) | **0 outside, strict**, in all 3 variants; least margin 2.754 L (EDT), 0.087 L (Ts) |
| `attack1_edge.py` | E1 crash; E2 lemma false; E3 f32 drift; E4 19 of 50 | E1: no crash, band `[0.480, ∞)`, `not_decaying`. E2: unchanged, and outside the new precondition. E3: unchanged, and not SPPS's arithmetic (receipt below). E4: unchanged, because it builds the mismatched certificate by hand. Through the code, 0 of 75 capped bands fail to cover (`dev/e3_e4_receipts.*`) |
| `attack1_edge_e1_search.py` | 0.6125–2213 s found; the band crashed | Same arrangements found, all inside `[0.480, ∞)` |
| `attack1_search.py 160 8 100 240 0 _r1` | 128 searched, 0 violations | 128 searched, **0 violations**. Reach: median 0.002–0.994 of the band. Width against round 1: median 0.998, range 0.53–1.25 |
| `attack1_search.py 96 8 100 240 5000 _hard hard` | 74 searched, 0 violations | 74 searched, **0 violations**. Reach: median 0.015–0.947. Width against round 1: median 0.985, range 0.80–1.59 |

**Where round 2 is wider than round 1:** 3 of 128 general and 2 of 74 hard scenarios, all with plateaus or anechoic decay, 20 kHz or `a = 150/s` air, at 1–2 ms. In all five the work budget was spent (6,000 relaxations). Round 1 used up to 11,903.

With the budget lifted, round 2 equals or beats round 1 in every one of them. For example, hard seed 5065 gives `[0.7093, 0.9610]` against round 1's `[0.7084, 0.9637]` (`dev/widen_check.py`, `.json`). This is the price of a bounded worst case, and every one of these bands is refused, at 10–40 L.

**The receipt against E3** (`dev/e3_e4_receipts.py`). SPPS multiplies a double by the f32 p. Over 20,000 steps that chain drifts from `p_f32^k` by at most 1.2·10⁻¹⁴ (at most 111·2⁻⁵³). E3's f32 chain drifts by 5.1·10⁻⁶. Source: `sppsTypes.h:72`, `coreString.h:43`, `CalculationCore.cpp:57`, `coreTypes.h:59`, `coreTypes.h:420`, `reportmanager.cpp:224`, `reportmanager.cpp:855`.

### 9.4 Cost (`bench.py`, `bench.json`, `bench_log.txt`)

Measured in numpy:

| Case | Relaxations | Time |
|---|---|---|
| dt ≤ 0.1 ms, any air | 8 | 3–50 ms |
| No air, 2 ms | 16–40 | 4–8 ms |
| 8 kHz, 1–2 ms | 192–706 | 36–131 ms |
| 20 kHz, 2 ms | 3,444–5,220 | 0.62–0.72 s |

- A relaxation costs 0.55–5.8 µs per bin in numpy, dominated by overhead at small W.
- The worst case the budget allows, `(R + 4·n0 + 4)` × the measured cost of one relaxation, is ≤ 2.3 s at every step from 2 ms to 0.05 ms and every T60 and air setting.
- Ts takes 0.3–28 ms, and C with D 0.1–3 ms.

**Rust, estimated only.** Nothing was run in Rust. Take about 8 candidates per bin at about 5 ns each, so about 40 ns per bin per relaxation. The accept regime (dt ≤ 0.2 ms: 8 relaxations, W ≤ 5,500) would then take 2 ms or less. The 20 kHz, 2 ms case would take about 8 ms. The worst case the budget allows, 2·10⁶ bin-relaxations, would take about 80 ms.

## 10. What this does not settle

1. **EDT is refused almost everywhere at the 2 ms preset.** The model-free half-width is `(1.6–2.1)·dt/U` on exponentials: 22.65 L median at 2 ms in T4. Acceptance needs about 0.1–0.2 ms (38 of 74 and 29 of 85 accepted), or more physics: `t_refl_min`, the direct sound's energy from geometry and source power, or a looser τ. This is the data's own ambiguity.
2. **Looseness where the direct sound dominates and U is short.** The chord over the arrival bin's tube (section 4.6) leaves the upper edge up to 1.6× the best known arrangement on T7's wide series 53. The fix, not implemented, is to branch on the arrival bin's split between direct and reflected energy. It would add a factor of 2–4 in work in that regime only. Those bands are refused at any τ near L.
3. **The work budget trades tightness for a bounded worst case.** In 5 of 202 searched scenarios, all refused, the band is up to 1.6× wider than without a budget (section 9.3).
4. **SPPS's energy kill** (`energie_epsilon`, section 1.3) is outside the band, and the histogram cannot bound it. Raising trans_epsilon is the real fix. P4 accepts a bound if one is ever established.
5. **The random ("Aléatoire") method** satisfies P1 only in expectation. The band then applies through section 8's edge-noise rule.
6. **m_n** (contributions per bin), which `spps_rel_eps` needs, is not in SPPS's output. The caller must bound it: `N_particles × (1 + the most reflections per step)`.
7. **The I-Simpa-compatible value exists for EDT only.** Upstream's C, D and Ts (`projet_calculation.cpp:320-470`, through `GetTimeDecay` and `GetSumLimit`) have no validated port. A port needs an oracle from upstream's own output, as the EDT port had.
8. **Single-run noise for EDT's band edges is not calibrated** (decision D8). Until it is, a single run's EDT is refused on noise by the existing rule, and only the seed batch (section 8) can pass it.
9. **Floating point is not bounded formally.** The certified bound adds an outward margin of 10⁻¹⁰ of J's scale and contains every witness (section 4.4). That is not interval arithmetic. Tests allow 10⁻⁹ relative.
10. **Ts tolerance is a policy call:** a flat 1 ms, or `min(1 ms, 0.5 % Ts)`. The band is the same either way.
11. **Not tested:**
    - real SPPS runs, since no solver may be run;
    - non-box rooms and diffuse walls, though the admissible set does not depend on the room;
    - directional sources;
    - a receiver ball cut by a wall.

    Several sources and celerity gradients are refused (P5), not handled.

## 11. Output files

All are in `target/agents/edt-band/`. Nothing was deleted. Nothing outside this folder was written. No tracked file changed.

**Code (round 2):**
- `SPEC.md`, `band_early.py`, `test_band_early.py`, `z3_cases.py`, `bench.py`
- `dev/smoke2.py`, `dev/wide_probe.py`, `dev/wide_diag.py`, `dev/wide_diag2.py`, `dev/e3_e4_receipts.py`, `dev/widen_check.py`, `dev/summarise_round2.py`
- `dev/_t1_t4_base.py`, `dev/_t5_t7.py` and `dev/spec_tail_r2.md`: the pieces the test file and this section were assembled from

**Results of the final code:**
- `run_all_stdout.txt`, `test_log_t1_t2_t3_t4_t5_t6_t7.txt`, `results_t1.json` … `results_t7.json`
- `z3_cases.json`, `z3_cases_log.txt`, `dev/z3_cases_stdout.txt`
- `bench.json`, `bench_log.txt`
- `dev/wide_probe.json`, `dev/e3_e4_receipts.json`, `dev/widen_check.json`, `dev/summary_round2.json`, `dev/summary_round2_stdout.txt`, `dev/searches_done.txt`

**The adversary's scripts, re-run** (their outputs are rewritten in place; round 1's copies are in `round1/`):
- `attack1_exact_a0.json` / `_log.txt`, `attack1_witness.json` / `_log.txt`, `attack1_lp.json` / `_log.txt`
- `attack1_z3.json` / `_log.txt`, `attack1_edge.json` / `_log.txt`, `attack1_edge_e1_search.json` / `_log.txt`
- `attack1_search_r1.json` / `_log.txt` / `_stdout.txt`, `attack1_search_hard.json` / `_log.txt` / `_stdout.txt`
- `dev/attack1_*_stdout_r2.txt`

**Archives:**
- `round1/`: round 1's code and all 34 round-1 outputs, the adversary's included.
- `round2a_pre_tubefix/`, `round2b_pre_margin/`, `round2c_pre_budgetW/`: the three earlier round-2 test runs, each with a README naming the code it ran. `round2a` also holds that code.
