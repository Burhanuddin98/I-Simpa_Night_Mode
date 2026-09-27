"""Model-free bands for the early room-acoustic parameters of an SPPS energy histogram.

Reference implementation of edt-band/SPEC.md (round 2). Pure numpy, float64. No solver is run.

For a histogram B[n] (the energy SPPS recorded in [n dt, (n+1) dt), with its per-step air p^(n+kappa)
applied), a direct arrival t_a with half-spread h = R/c, and the band's air rate a = m c, every
quantity is returned as an interval [lo, hi] that contains its continuous-time value for EVERY
non-negative arrangement of energy inside the bins that is consistent with the recorded sums
(SPEC.md section 2). No shape is assumed anywhere.

  EDT : certified outer bound (SPEC Theorem E: window-end intervals with Ub <= 2 Ua, tangent/chord
        linearisation, weak duality for the -10 dB crossing), with witness arrangements whose exact
        EDTs lie in the closure of the true band;
  Ts  : exact up to Dinkelbach's convergence, every iterate certified (SPEC Theorem TS);
  C, D: exact, closed form (SPEC Theorem CD).
Every edge is the supremum / infimum over the admissible set (its closure), not an attained value.

Public entry points: Setup, Setup.edt_band, Setup.ts_band, Setup.cd_band, Setup.all_bands,
exact_params (the continuous-time values of an explicit arrangement), spps_rel_eps (SPPS's per-bin
recording tolerance, SPEC section 1), upstream_edt (the I-Simpa-compatible value, through the
validated port in target/agents/upstream-edt).
"""
import importlib.util
import math
import os
import sys

import numpy as np

LN10 = math.log(10.0)
DB = 10.0 / LN10                 # 10 lg x = DB ln x
LN_TENTH = math.log(0.1)
TINY = 1e-300
F32_U = 2.0 ** -24               # unit roundoff, float32 (round to nearest)
F64_U = 2.0 ** -53               # unit roundoff, float64

# 1/10 of a just-noticeable difference (the strict rule, docs/params.md "Truncation"): the units
# every half-width is reported in. The default refusal tolerance tau is the same, except Ts, whose
# default follows decay.rs `limits`: min(1 ms, 0.5 % of Ts).
LIMIT = {'edt': 0.005, 'ts': 0.001, 'c': 0.1, 'd': 0.005}
JND = {'edt': 0.05, 'ts': 0.010, 'c': 1.0, 'd': 0.05}
TS_RELATIVE = 0.005
MAX_INITIAL_INTERVALS = 64       # geometric cover of [U_min, U_max]: Ub <= 2 Ua on each interval


def _suffix_excl(x):
    """out[k] = sum_{m > k} x[m]."""
    c = np.cumsum(x[::-1])[::-1]
    return np.concatenate([c[1:], [0.0]])


def air_rate_mismatch_bound(dt, n_f32_ops=4):
    """A priori worst-case bound on |a_f32 - m c| (SPEC 1.1, GATE 1), with no knowledge of m or c.

    SPPS's per-step air factor is p_f32 = fl32(exp(fl32(-fl32(m) * fl32(fl32(c) * fl32(dt)))))
    (base_core_configuration.cpp:114-115): the exact value x = -m c dt goes through at most
    n_f32_ops = 4 float32 roundings (c*dt, m*(...), the negate is exact, exp). Each elementary f32
    op has relative error at most u32 = 2^-24, so to the accuracy of first-order accumulation,
    p_f32 = exp(x) (1 + delta) with |delta| <= (1 + u32)^n_f32_ops - 1. band_early's air rate is
    a_f32 = -ln(p_f32)/dt = m c - ln(1 + delta)/dt, so

        |a_f32 - m c| = |ln(1 + delta)| / dt <= (|delta| / (1 - |delta|)) / dt =: Delta(dt).

    Delta(dt) is a pure function of the step (not of m, c): it blows up as dt -> 0, because the f32
    rounding of p turns into an ever larger rate error once fewer bits of the exponent survive.
    This is why the gap concentrates at the smallest grid steps (0.1-0.2 ms), not the largest.
    When the caller already has both rates (e.g. it ran the f32 chain itself, as eval_diag_p32.py
    does), the exact |a_f32 - m c| is tighter than Delta(dt) and should be used instead
    (air_rate_true_diff below); Delta(dt) is the bound to use with no other information.
    """
    u32 = F32_U
    delta = (1.0 + u32) ** n_f32_ops - 1.0
    if delta >= 1.0:
        raise ValueError('n_f32_ops too large for this bound')
    return (delta / (1.0 - delta)) / float(dt)


def widen_eps_for_air_rate(rel_eps, n_bins, dt, air_rate_true_diff):
    """Compound a rate-mismatch widening onto an existing per-bin (or scalar) recording tolerance
    (GATE 1, SPEC 1.1). See spps_rel_eps's air_rate_true_diff for the derivation; this is the same
    formula, usable with any base tolerance (not only spps_rel_eps's own gamma-bound one), so a
    caller with its own recording tolerance still gets the fix as one call, not a hand-rolled one.

        eps_n_fix = (1 + eps_n) * exp(|a_f32 - m c| * (n + 1) * dt) - 1.
    """
    n = np.arange(int(n_bins), dtype=np.float64)
    base = np.broadcast_to(np.asarray(rel_eps, np.float64), n.shape)
    da = abs(float(air_rate_true_diff))
    rate_eps = np.expm1(da * (n + 1.0) * float(dt))
    return (1.0 + base) * (1.0 + rate_eps) - 1.0


def spps_rel_eps(n_bins, contributions_per_bin, max_reflections=0, storage='f32', dt=None,
                  air_rate_true_diff=None):
    """Rigorous per-bin relative tolerance eps_n of SPPS's recorded values (SPEC section 1, P1).

    SPPS keeps each particle's energy in double (sppsTypes.h:72, l_decimal = double,
    coreString.h:43), multiplies it by the f32 air factor p once per step (CalculationCore.cpp:57)
    and by (1 - alpha) at each reflection, multiplies by the in-ball length once, and accumulates
    the bin in double (coreTypes.h:420, reportmanager.cpp:224). The bin is stored as f32 after one
    double product with the constant cdt_vol (reportmanager.cpp:855). With k = m_n + n + r + 3 double
    roundings (m_n contributions, n + 1 air products, r reflections, the length product, the cdt_vol
    product) g = k u64 / (1 - k u64) bounds the double part (Higham's gamma_k), and one f32 rounding
    is at most u32 relative to the stored value. The exact value X_n then satisfies
    |X_n / B_n - 1| <= (u32 + g) / (1 - g) = eps_n.  storage='f64' for a double output file.

    air_rate_true_diff (GATE 1): band_early's air rate a is a_f32 = -ln(p_f32)/dt (Setup's
    air_rate docstring), which is what SPPS actually propagates. A caller checking against a
    truth built on the physical rate m c (rather than SPPS's own a_f32) needs a wider tolerance,
    because P1 as stated only bounds the recording of the a_f32 series, not its distance to m c
    (SPEC 1.1). Pass |a_f32 - m c| here (air_rate_mismatch_bound(dt) if only dt is known, tighter
    if the caller has both rates) and dt; every bin gets its rate error compounded onto the
    recording tolerance:

        eps_n_fix = (1 + eps_n) * exp(|a_f32 - m c| * (n + 1) * dt) - 1,

    which is exact because energy read in bin n arrived at some tau <= (n + 1) dt since emission
    (Setup's cn = (n + kappa) dt - t_a, kappa = 1), so the true-rate value differs from the
    a_f32-rate value by at most the factor exp(|a_f32 - m c| * (n + 1) * dt) at every tau in the
    bin's admissible range; compounding it onto (1 +- eps_n) keeps a single interval that contains
    both the a_f32-consistent and the m-c-consistent true energy. Widening eps only enlarges the
    admissible set (SPEC section 2), so this can only add truths a band holds, never drop one.
    """
    n = np.arange(int(n_bins), dtype=np.float64)
    m = np.broadcast_to(np.asarray(contributions_per_bin, np.float64), n.shape)
    k = m + n + float(max_reflections) + 3.0
    ku = k * F64_U
    if np.any(ku >= 0.5):
        raise ValueError('too many roundings for the gamma bound')
    g = ku / (1.0 - ku)
    s = F32_U if storage == 'f32' else F64_U
    eps = (s + g) / (1.0 - g)
    if air_rate_true_diff is not None:
        if dt is None:
            raise ValueError('dt is required with air_rate_true_diff')
        eps = widen_eps_for_air_rate(eps, n_bins, dt, air_rate_true_diff)
    return eps


# ------------------------------------------------------------------------------------------------
# GATE 2: production inputs (tail_max, eps_n) from a finished run's own outputs, and the run-length
# rule that refuses a run too short to trust. SPEC.md section 1.3/P4 and P1; GATE2.md.
# ------------------------------------------------------------------------------------------------
ENERGETIC_METHOD = 1     # config.xml / results-json computation_method: 1 energetic, 0 random

# Decision-log row 14 (2026-09-27 01:50, Jarvis, technical call): turning the room-wide unrecorded
# fraction f_end into a bound on ONE receiver's future deposit needs a diffuse-field assumption
# (docs/results.md "Lost particles"), which is not model-free. GATE2_TAIL_SAFETY_FACTOR inflates
# the diffuse-field bound before it is used as Setup's tail_max, so the band still holds even if
# the true receiver capture is up to this many times worse than the diffuse-field estimate.
GATE2_TAIL_SAFETY_FACTOR = 1000.0

# Decision-log row 14: if even the inflated bound already eats a material share of the quantity's
# own recording tolerance, the run is too short to trust rather than silently widened further.
#
# GATE 4b fix (2026-09-27): the original refusal compared tail_max (an absolute energy, units of
# sum(B)) against GATE2_TAIL_REFUSAL_TOLERANCE_SHARE * quantity_tolerance (a half-width, in the
# quantity's own units -- seconds for Ts, relative for EDT, dB for C, dimensionless for D). Those
# units do not agree, so the comparison was dimensionally meaningless and never a real gate
# (production_inputs() no longer takes quantity_tolerance at all; passing it raises TypeError).
# The replacement, applied in Setup.all_bands(), stays entirely in each quantity's own units: solve
# the same band twice from the same B/eps/everything-else, once with the caller's (x1000-inflated)
# tail_max and once with tail_max = 0, and take tail_widening_q = halfwidth_with_tail -
# halfwidth_without_tail in the units all_bands() already reports and already compares against tau
# for that quantity. If tail_widening_q > GATE2_TAIL_REFUSAL_TOLERANCE_SHARE * tau_q, that quantity
# (only that quantity -- others are judged independently) is refused 'run_too_short'. The 1% share
# is unchanged from the original rule: below it, even a x1000-wrong diffuse-field assumption cannot
# move the reported range by more than a hundredth of a JND-scale unit, so the run is trusted;
# above it, the guarantee is resting on the diffuse-field assumption GATE2.md documents as not
# model-free, so the quantity is outside the method's premises and is refused rather than reported.
GATE2_TAIL_REFUSAL_TOLERANCE_SHARE = 0.01


def safe_contributions_per_bin(particles_per_source, n_sources=1):
    """A model-free, run-derived upper bound on m_n (SPEC P1), with no m_n in the output.

    SPPS records one deposit per particle per time step it is tracked (`ReportManager::
    RecordTimeStep`, called at most once per particle per step, `CalculationCore.cpp:78`; a
    transmitted particle in the energetic method is a second, independent particle, already
    counted in the total emitted). So no bin's true contribution count can exceed the number of
    particles ever emitted, `particles_per_source * n_sources`: a particle contributes to a given
    bin at most once (it occupies one step), so summing over all particles bounds every bin's m_n
    at once, with no assumption on where in time any particle's contributions fall. This is the
    'smallest safe substitute' SPEC 1 asks for when m_n itself is not in the output: it is loose
    (almost every bin's true m_n is far smaller), but it never underestimates, so spps_rel_eps
    only widens, never narrows, the true tolerance.
    """
    return int(particles_per_source) * int(n_sources)


def production_tail_max(total_energy, onset_step, trans_epsilon=None, computation_method=ENERGETIC_METHOD,
                         killed_particles=0, particles_per_source=1, n_sources=1, receiver_recorded_from_onset=None):
    """The production (a, b) tail bound of GATE 2's brief, as a fraction of S(onset) (the energy the
    receiver records from the arrival on) -- multiply by that sum to get Setup's tail_max in the
    histogram's own units.

    (a) Energy still alive in the room when the series ends, unaccounted in any bin.
        `total_energy[]` (results-json.md, `spps.total_energy`) is the room's total energy per step,
        in the same units up to a single factor (rho c, reportmanager.cpp:155-166 and 807-816: the
        room table and the source .gap both carry it, so it cancels in a ratio). Let
        f_end = total_energy[-1] / total_energy[onset_step]: the share of the energy alive at the
        arrival that is STILL alive, and so still able to reach the receiver, when the series ends.

    (b) Energy SPPS killed at the trans_epsilon floor (energetic mode only; SPEC 1.3,
        CalculationCore.cpp:57-60, 141-146, 305-310; sppsNantes.cpp:75). A killed particle's energy
        at the instant of the kill is at most `E0 * 10^-trans_epsilon` (E0 its start energy; every
        particle from one source starts equal, `sppsNantes.cpp:75`'s `confPartFrame.energie`).
        `killed_particles` (the caller's count; the safe over-count is
        `absorbed_by_atmosphere + absorbed_by_materials + absorbed_by_fittings`, since only the
        energetic-mode branches of those three states can be a floor kill and the others are
        counted anyway, per-band, `run/stats.rs::BandStats`) of `N = particles_per_source *
        n_sources` total, each at most `10^-trans_epsilon` of `E0 = 1/N` (normalised so the total
        emitted is 1), together are at most `killed_particles * 10^-trans_epsilon / N` of the total
        emitted energy.

    **What this bound is NOT.** Both (a) and (b) answer "how much energy is unaccounted for",
    not "how much of it could still reach one small receiver ball". Converting a room-wide energy
    total into a bound on one receiver's future deposit needs either a spatial assumption (a
    diffuse field: energy arrives at the receiver in proportion to its share of the room, which is
    the assumption `docs/results.md` "Lost particles" and "the solver's floor" make, calibrated
    there against a closed-form model over alpha 0.05-0.9, trans_epsilon 1-7) or a hard geometric
    worst case, which SPEC 1.3 already rejects as useless ("of the order of the whole recorded
    energy": nothing rules out every remaining joule funnelling through the ball). **No fully
    rigorous, model-free (assumption-free) bound on either (a) or (b) is possible from SPPS's
    outputs alone**; see GATE2.md. This function returns the best available run-derived bound,
    reusing the diffuse-field assumption `docs/results.md` already ships and has tested
    (`tests/params_floor.rs`, 487 of 855 cells accepted, none of the accepted further from the
    model than its limit), converted to Setup's Tmax convention (a bound on TRUE energy, not a
    share of it): multiply the returned fraction by `receiver_recorded_from_onset` (S(onset), the
    caller's own sum of recorded bins from the onset bin on) to get an absolute tail_max.

    Returns (tail_fraction, detail dict). tail_fraction is dimensionless: the bound on unaccounted
    TRUE energy as a multiple of S(onset). Pass None for trans_epsilon (or killed_particles=0) to
    skip (b) (random mode has no floor kill).
    """
    total_energy = np.asarray(total_energy, dtype=np.float64)
    if total_energy.ndim != 1 or len(total_energy) == 0:
        raise ValueError('total_energy must be a non-empty 1-D series')
    if not (0 <= onset_step < len(total_energy)):
        raise ValueError('onset_step out of range')
    e_onset = float(total_energy[onset_step])
    if not e_onset > 0:
        raise ValueError('total_energy at onset must be > 0: alive_share is undefined')
    f_end = float(total_energy[-1]) / e_onset
    detail = dict(f_end=f_end, e_onset=e_onset, e_end=float(total_energy[-1]))
    kill_frac = 0.0
    if computation_method == ENERGETIC_METHOD and trans_epsilon is not None and killed_particles > 0:
        N = int(particles_per_source) * int(n_sources)
        if N <= 0:
            raise ValueError('particles_per_source * n_sources must be > 0')
        kill_frac = float(killed_particles) * (10.0 ** (-float(trans_epsilon))) / N
        detail.update(killed_particles=int(killed_particles), N=N, kill_frac=kill_frac)
    tail_fraction_raw = f_end + kill_frac
    tail_fraction = GATE2_TAIL_SAFETY_FACTOR * tail_fraction_raw
    detail['tail_fraction_raw'] = tail_fraction_raw
    detail['tail_fraction'] = tail_fraction
    detail['safety_factor'] = GATE2_TAIL_SAFETY_FACTOR
    if receiver_recorded_from_onset is not None:
        detail['tail_max_abs'] = tail_fraction * float(receiver_recorded_from_onset)
    return tail_fraction, detail


def production_inputs(B, dt, onset_step, total_energy, particles_per_source, n_sources=1,
                       trans_epsilon=None, computation_method=ENERGETIC_METHOD, killed_particles=0,
                       max_reflections=0, storage='f32', air_rate_true_diff=None, quantity_tolerance=None):
    """GATE 2's entry point: production_inputs(run-derived fields) -> (tail_max, eps array), in
    Setup's own units (tail_max absolute, eps per-bin), built only from what a finished run's JSON
    (results-json.md) and particle-statistics table (`run/stats.rs`) give -- no oracle, no fit.

    B            : the recorded bin series (Setup's own B), used only for S(onset) = sum(B).
    onset_step   : the onset bin's index into `total_energy` (results-json.md's per-step room
                   table starts at step 0; the onset bin is the same index into B).
    quantity_tolerance : REMOVED (GATE 4b, 2026-09-27): the old refusal compared this absolute-energy
                   tail_max against a per-quantity half-width tolerance, which do not share units --
                   see the comment above GATE2_TAIL_REFUSAL_TOLERANCE_SHARE. Pass a non-None value
                   here and this function raises TypeError. The run-too-short refusal now happens in
                   Setup.all_bands(), per quantity, by comparing the band's own tail-widening against
                   that quantity's own tau, both in the quantity's own units.
    """
    if quantity_tolerance is not None:
        raise TypeError(
            "production_inputs() no longer takes quantity_tolerance -- the tail_max vs. "
            "quantity_tolerance comparison mixed an absolute energy with a per-quantity half-width "
            "and was dimensionally meaningless (GATE 4b). The run_too_short refusal is now computed "
            "per quantity by Setup.all_bands(), from tail_widening vs. that quantity's own tau; see "
            "the comment above GATE2_TAIL_REFUSAL_TOLERANCE_SHARE in this module.")
    B = np.asarray(B, dtype=np.float64)
    s_onset = float(B.sum())
    tail_fraction, detail = production_tail_max(
        total_energy, onset_step, trans_epsilon=trans_epsilon, computation_method=computation_method,
        killed_particles=killed_particles, particles_per_source=particles_per_source, n_sources=n_sources,
        receiver_recorded_from_onset=s_onset)
    tail_max = tail_fraction * s_onset
    m_n = safe_contributions_per_bin(particles_per_source, n_sources)
    eps = spps_rel_eps(len(B), m_n, max_reflections=max_reflections, storage=storage, dt=dt,
                        air_rate_true_diff=air_rate_true_diff)
    return tail_max, eps, detail


def min_run_length(dt, alive_share_curve, tolerance_fraction, trans_epsilon=None,
                    computation_method=ENERGETIC_METHOD, killed_particles_rate=0.0,
                    particles_per_source=1, n_sources=1, max_steps=2_000_000):
    """The run-length rule (decision-log row 12): the fewest steps for `production_tail_max`'s
    fraction to fall, and STAY (monotonically, since alive_share_curve is taken non-increasing), at
    or below `tolerance_fraction`. Refuses 'run_too_short' rather than assume 0, and never
    extrapolates the curve past what it is given (no log-linear fit): it only reads the alive-share
    curve the caller already measured or is willing to keep measuring, one step at a time.

    alive_share_curve : total_energy[n] / total_energy[onset_step] for n = onset_step, onset_step+1,
                         ..., as far as the caller has it (or is willing to run it). A callable
                         step -> share is also accepted, for a caller that can extend the run.
    killed_particles_rate : killed particles per step (energetic mode), for the (b) term's growth;
                         0 to ignore (b), e.g. when trans_epsilon is None or mode is random.

    Returns (steps_needed, 'ok') or (None, 'run_too_short') if max_steps is reached first.
    """
    get = alive_share_curve if callable(alive_share_curve) else (lambda n, c=alive_share_curve: c[n] if n < len(c) else None)
    N = int(particles_per_source) * int(n_sources)
    n = 0
    while n <= max_steps:
        share = get(n)
        if share is None:
            return None, 'run_too_short'
        kill_frac = 0.0
        if computation_method == ENERGETIC_METHOD and trans_epsilon is not None and killed_particles_rate > 0 and N > 0:
            kill_frac = (killed_particles_rate * n) * (10.0 ** (-float(trans_epsilon))) / N
        if share + kill_frac <= tolerance_fraction:
            return n, 'ok'
        n += 1
    return None, 'run_too_short'


# ------------------------------------------------------------------------------------------------
# Exact continuous-time parameters of an explicit arrangement (the ground truth of every test)
# ------------------------------------------------------------------------------------------------
def exact_params(pos, mass, te_list=(0.05, 0.08)):
    """The continuous-time EDT, Ts, D_te and C_te of an arrangement of point masses.

    pos  : reading times u >= 0 (s since the direct arrival). u == 0 is the direct sound (and
           energy lumped there): it counts in S(0) only.
    mass : true (continuous-air) energies >= 0.
    A continuous density is passed as fine atoms; the tests do that at 1 us.
    EDT is the ISO 3382-1 least-squares slope of 10 lg S(u)/S(0) over the time the curve spends in
    [-10, 0] dB, read continuously (the functional of decay.rs `Curve::line`), -60/slope.
    """
    pos = np.asarray(pos, float)
    mass = np.asarray(mass, float)
    keep = mass > 0
    pos, mass = pos[keep], mass[keep]
    S0 = mass.sum()
    out = {}
    if S0 <= 0:
        return dict(edt=float('nan'), ts=float('nan'), U=float('nan'))
    out['ts'] = float((pos * mass).sum() / S0)
    for te in te_list:
        e = mass[pos < te].sum()
        late = S0 - e
        out['d%g' % (te * 1e3)] = float(e / S0)
        if e > 0 and late > 0:
            out['c%g' % (te * 1e3)] = float(DB * math.log(e / late))
        else:
            out['c%g' % (te * 1e3)] = float('inf') if late <= 0 else float('-inf')
    refl = pos > 0
    p = pos[refl]
    m = mass[refl]
    o = np.argsort(p, kind='stable')
    p, m = p[o], m[o]
    suf = np.concatenate([np.cumsum(m[::-1])[::-1], [0.0]]) / S0   # S on (p[i-1], p[i]] = suf[i]
    if len(p) == 0 or suf[0] < 0.1:
        out.update(edt=float('nan'), U=0.0)
        return out
    i_star = int(np.nonzero(suf[1:] < 0.1)[0][0])
    U = float(p[i_star])
    left = np.concatenate([[0.0], p[:i_star]])
    right = p[:i_star + 1]
    lev = np.log(suf[:i_star + 1])
    hU = 0.5 * U
    J = float((lev * 0.5 * ((right - hU) ** 2 - (left - hU) ** 2)).sum())
    slope_db = DB * 12.0 * J / U ** 3
    out['U'] = U
    out['slope_db'] = slope_db
    out['edt'] = -60.0 / slope_db if slope_db < 0 else float('inf')
    return out


def _curve_at(pos, mass, u):
    """S(u) of an arrangement (mass at positions >= u), for u > 0."""
    o = np.argsort(pos)
    p, m = pos[o], mass[o]
    suf = np.concatenate([np.cumsum(m[::-1])[::-1], [0.0]])
    return suf[np.searchsorted(p, u, side='left')]


def _chord_ratio_extreme(m0, md, Ua, d, want):
    """max (want='max') or min over x in [0, d] of (m0 + (md - m0) x / d) / (Ua + x)^3. Ua > 0."""
    if not Ua > 0:
        raise ValueError('window-end interval must start after the arrival (Ua > 0)')
    f = lambda x: (m0 + ((md - m0) / d) * x if d > 0 else m0) / (Ua + x) ** 3
    xs = [0.0, d]
    if d > 0:
        s = (md - m0) / d
        if s != 0:
            x = (s * Ua - 3.0 * m0) / (2.0 * s)     # the one stationary point of the ratio
            if 0.0 < x < d:
                xs.append(x)
    vals = [f(x) for x in xs]
    return max(vals) if want == 'max' else min(vals)


# ------------------------------------------------------------------------------------------------
# The admissible set
# ------------------------------------------------------------------------------------------------
class Setup:
    """One band of one receiver: the recorded histogram and what is known about it.

    B          : recorded bin energies (SPPS's output for this band; f32 values are fine).
    dt         : step, s. Bin n is [n dt, (n+1) dt) on SPPS's own time axis (emission at a step).
    t_arr      : direct-arrival time, s, on that axis: emission time + d/c (the ball centre).
    half_width : h = R/c, s. The direct sound reaches the receiver over [t_arr - h, t_arr + h].
    air_rate   : a, 1/s, such that SPPS's per-step factor is p = exp(-a dt). Pass a = -ln(p_f32)/dt
                 with p_f32 the f32 value SPPS uses (base_core_configuration.cpp:115); then SPPS's
                 product p^(n+kappa) and the band agree exactly (SPEC section 1).
    kappa      : the per-step air exponent offset; 1 for SPPS (CalculationCore.cpp: energie *= p at
                 the start of every step, before the step records).
    rel_eps    : relative recording tolerance, a scalar or one value per bin: the exact value X_n
                 lies in [B_n (1 - eps_n), B_n (1 + eps_n)]. spps_rel_eps() gives SPPS's.
    tail_max   : an upper bound on the TRUE energy that arrived but is not in B (0: none): energy
                 after the series end, or energy SPPS dropped, if the caller can bound it.
    tail_t_min : the earliest time (s, same axis) that energy can arrive; default the series end.
                 Must be >= t_arr.
    tail_t_max : the latest time (s, same axis) that energy can arrive; None = unbounded.
    t_refl_min : optional, the earliest time (s, same axis) any reflected energy can reach the ball,
                 e.g. emission + (d1 - R)/c with d1 the triangle-inequality bound over the face
                 planes. None: t_arr - half_width (nothing known beyond the direct window).
    misfit_tol : energy share allowed where the premises say none can be (before the direct window,
                 or between it and t_refl_min) before every quantity is refused 'arrival_misfit'.
    n_sources, homogeneous_celerity : the premises P2/P3 hold for one point source in a medium of
                 one celerity. Anything else refuses every quantity 'premise_unsupported'.
    """

    def __init__(self, B, dt, t_arr, half_width=0.0, air_rate=0.0, kappa=1.0, rel_eps=0.0,
                 tail_max=0.0, tail_t_max=None, t_refl_min=None, misfit_tol=1e-6, tail_t_min=None,
                 n_sources=1, homogeneous_celerity=True):
        B = np.asarray(B, dtype=np.float64).copy()
        if B.ndim != 1 or len(B) == 0:
            raise ValueError('B must be a non-empty 1-D series')
        if not np.all(np.isfinite(B)) or np.any(B < 0):
            raise ValueError('bins must be finite and >= 0')
        for name, v in (('dt', dt), ('t_arr', t_arr), ('half_width', half_width), ('air_rate', air_rate),
                        ('tail_max', tail_max)):
            if not (math.isfinite(v) and v >= 0):
                raise ValueError('%s must be finite and >= 0' % name)
        if not dt > 0:
            raise ValueError('dt must be > 0')
        eps = np.broadcast_to(np.asarray(rel_eps, np.float64), B.shape).copy()
        if not (np.all(np.isfinite(eps)) and np.all(eps >= 0) and np.all(eps < 1)):
            raise ValueError('rel_eps must be in [0, 1)')
        self.N = N = len(B)
        self.dt = dt = float(dt)
        self.ta = ta = float(t_arr)
        self.h = h = float(half_width)
        self.a = a = float(air_rate)
        self.kappa = kappa = float(kappa)
        self.eps = eps
        self.n_sources = int(n_sources)
        self.homogeneous_celerity = bool(homogeneous_celerity)
        trm = ta - h if t_refl_min is None else max(float(t_refl_min), ta - h)
        self.t_refl_min = trm
        scale = float(B.sum())
        if scale <= 0:
            raise ValueError('the series holds no energy')
        self.scale = scale                       # every energy below is in units of sum(B)
        self.B_rec = B
        b = B / scale
        self.Bp = b * (1.0 + eps)
        self.Bm = b * (1.0 - eps)
        self.Tmax = float(tail_max) / scale
        # unrecorded energy: reading range [tv0, tv1] (true energy, no air factor: it is a bound on
        # the true energy itself)
        t_min = N * dt if tail_t_min is None else float(tail_t_min)
        if t_min < ta:
            raise ValueError('tail_t_min must be >= t_arr')
        if tail_t_max is not None and float(tail_t_max) < t_min:
            raise ValueError('tail_t_max must be >= tail_t_min')
        self.tail_t_min = t_min
        self.tail_t_max = tail_t_max
        self.tv0 = max(t_min, trm) - ta
        self.tv1 = (float(tail_t_max) - ta) if tail_t_max is not None else float('inf')
        n = np.arange(N, dtype=np.float64)
        t0 = n * dt
        t1 = (n + 1.0) * dt
        self.t0, self.t1 = t0, t1
        has = b > 0
        self.has = has
        # theta: true / recorded energy for energy that arrived at physical time tau in bin n:
        # exp(a ((n+kappa) dt - tau)). In reading time v = tau - t_a it is exp(a (cn - v)).
        self.cn = (n + kappa) * dt - ta
        # u = 0: direct sound (tau in the direct window) or reflected energy before t_a lumped at it
        z_lo = np.maximum(t0, ta - h)
        z_hi = np.minimum(t1, ta + h)
        self.z0 = has & (z_lo <= z_hi)
        # reflected energy read at its own time u = tau - t_a > 0: tau in [max(t0, t_a, trm), t1]
        f_lo = np.maximum(np.maximum(t0, ta), trm)
        self.refl = has & (f_lo < t1)
        # energy the premises exclude: fall back to the bin's whole span, and report its share
        bad = has & ~self.z0 & ~self.refl
        self.misfit_share = float(b[bad].sum())
        self.misfit_tol = misfit_tol
        if bad.any():
            before = bad & (t1 <= ta + h)
            z_lo = np.where(before, t0, z_lo)
            z_hi = np.where(before, np.minimum(t1, ta), z_hi)
            self.z0 = self.z0 | before
            f_lo = np.where(bad & ~before, np.maximum(t0, ta), f_lo)
            self.refl = self.refl | (bad & ~before)
        self.th0_lo = np.where(self.z0, np.exp(a * ((n + kappa) * dt - z_hi)), 1.0)
        self.th0_hi = np.where(self.z0, np.exp(a * ((n + kappa) * dt - z_lo)), 1.0)
        self.r0 = np.where(self.refl, f_lo - ta, np.inf)     # reflected reading range [r0, r1]
        self.r1 = t1 - ta
        # the reading grid: bin n covers u in [g0, g1]
        self.g0 = np.maximum(t0, ta) - ta
        self.g1 = t1 - ta
        self.rb = t1 > ta
        self.th_r0 = np.where(self.refl, np.exp(a * (self.cn - np.where(self.refl, self.r0, 0.0))), 0.0)
        self.th_r1 = np.where(self.refl, np.exp(a * (self.cn - self.r1)), 0.0)
        thmax = np.maximum(np.where(self.z0, self.th0_hi, 0.0), self.th_r0)
        thmin = np.minimum(np.where(self.z0, self.th0_lo, np.inf), np.where(self.refl, self.th_r1, np.inf))
        thmin = np.where(has, thmin, 0.0)
        self.S0_hi = float((self.Bp * thmax).sum()) + self.Tmax
        self.S0_lo = float((self.Bm * thmin).sum())
        # tube (SPEC Lemma T): suffix sums over bins m > k
        self.own_hi = self.Bp * self.th_r0
        self.own_lo = np.where(self.refl & ~self.z0, self.Bm * self.th_r1, 0.0)
        self.Hc = _suffix_excl(self.own_hi)
        self.Lc = _suffix_excl(self.own_lo)
        self.r_end = float(self.g1[-1])
        # bins read wholly after a window end (and without a u = 0 option): their mass at its largest
        # (all at r0, B+) or least (all at r1, B-), summed from bin n on
        agg = self.refl & ~self.z0
        self.SP0 = np.concatenate([np.cumsum(np.where(agg, self.Bp * self.th_r0, 0.0)[::-1])[::-1], [0.0]])
        self.SM1 = np.concatenate([np.cumsum(np.where(agg, self.Bm * self.th_r1, 0.0)[::-1])[::-1], [0.0]])
        self.n_eval = 0
        self._budget = float('inf')

    # -------------------------------------------------------------------------------------------
    def S_hi(self, x):
        """Upper bound on S(x) = true energy read at u >= x, for x > 0 (left-continuous)."""
        x = np.atleast_1d(np.asarray(x, float))
        k = np.searchsorted(self.g1, x, side='left')
        out = np.full(x.shape, self.Tmax)
        ok = k < self.N
        kk, xx = k[ok], x[ok]
        own = np.where(self.refl[kk] & (xx <= self.r1[kk]),
                       self.Bp[kk] * np.exp(self.a * (self.cn[kk] - np.maximum(xx, np.where(self.refl[kk], self.r0[kk], 0.0)))),
                       0.0)
        out[ok] += self.Hc[kk] + own
        return out

    def S_hi_open(self, x):
        """Upper bound on S(x+) = rho((x, inf)), true energy read strictly after x, for x >= 0: the
        bins whose reflected range reaches past x (r1 > x), each at its largest theta. A bin whose
        range ends at x (its closure point r1 = x) cannot put energy in (x, inf)."""
        x = np.atleast_1d(np.asarray(x, float))
        k = np.searchsorted(self.g1, x, side='right')
        out = np.full(x.shape, self.Tmax)
        ok = k < self.N
        kk, xx = k[ok], x[ok]
        own = np.where(self.refl[kk],
                       self.Bp[kk] * np.exp(self.a * (self.cn[kk] - np.maximum(xx, np.where(self.refl[kk], self.r0[kk], 0.0)))),
                       0.0)
        out[ok] += self.Hc[kk] + own
        return out

    def S_lo(self, x):
        """Lower bound on S(x) for x > 0 (mass that must lie at u >= x, at its least)."""
        x = np.atleast_1d(np.asarray(x, float))
        k = np.searchsorted(self.g1, x, side='left')
        out = np.zeros(x.shape)
        ok = k < self.N
        kk, xx = k[ok], x[ok]
        own = np.where(self.refl[kk] & ~self.z0[kk] & (self.r0[kk] >= xx), self.own_lo[kk], 0.0)
        out[ok] = self.Lc[kk] + own
        return out

    def premise_refusal(self):
        if self.n_sources != 1 or not self.homogeneous_celerity:
            return dict(refused='premise_unsupported',
                        why='the direct window and the first-reflection bound (P2, P3) hold for one point '
                            'source in a medium of one celerity; this run has %d source(s)%s' % (
                                self.n_sources, '' if self.homogeneous_celerity else ' and a celerity gradient'))
        if self.misfit_share > self.misfit_tol:
            return dict(refused='arrival_misfit',
                        why='%.3g of the energy lies where neither the direct sound (the window around '
                            't_arr) nor a reflection (after t_refl_min) can arrive' % self.misfit_share)
        return None

    # -------------------------------------------------------------------------------------------
    # C and D (SPEC Theorem CD): exact, closed form
    # -------------------------------------------------------------------------------------------
    def cd_band(self, te):
        """([D_lo, D_hi], [C_lo, C_hi], info) for a window te (s) after the arrival."""
        info = {}
        a = self.a
        r0s = np.where(self.refl, self.r0, 0.0)
        e_refl = self.refl & (r0s < te)
        early_opt = self.z0 | e_refl
        late_opt = self.refl & (self.r1 >= te)
        th_e_max = np.maximum(np.where(self.z0, self.th0_hi, 0.0), np.where(e_refl, self.th_r0, 0.0))
        th_e_min = np.minimum(np.where(self.z0, self.th0_lo, np.inf),
                              np.where(e_refl, np.exp(a * (self.cn - np.minimum(self.r1, te))), np.inf))
        th_e_min = np.where(early_opt, th_e_min, 0.0)
        th_l_max = np.where(late_opt, np.exp(a * (self.cn - np.maximum(r0s, te))), 0.0)
        th_l_min = np.where(late_opt, self.th_r1, 0.0)
        T = self.Tmax
        tail_early = T > 0 and self.tv0 < te
        tail_late = T > 0 and self.tv1 >= te
        # D_hi: every bin that can be early is early at its largest early theta; the rest late at
        # their least; the unrecorded energy early (all of it) if it can be, else none
        E = float(np.where(early_opt, self.Bp * th_e_max, 0.0).sum()) + (T if tail_early else 0.0)
        Lt = float(np.where(~early_opt & late_opt, self.Bm * th_l_min, 0.0).sum())
        d_hi = E / (E + Lt) if E + Lt > 0 else float('nan')
        c_hi = DB * math.log(E / Lt) if (E > 0 and Lt > 0) else (float('inf') if Lt <= 0 else float('-inf'))
        # D_lo: every bin that can be late is late at its largest late theta; the rest early at
        # their least; the unrecorded energy late (all of it) if it can be, else none
        Lt2 = float(np.where(late_opt, self.Bp * th_l_max, 0.0).sum()) + (T if tail_late else 0.0)
        E2 = float(np.where(~late_opt & early_opt, self.Bm * th_e_min, 0.0).sum())
        d_lo = E2 / (E2 + Lt2) if E2 + Lt2 > 0 else float('nan')
        c_lo = DB * math.log(E2 / Lt2) if (E2 > 0 and Lt2 > 0) else (float('-inf') if E2 <= 0 else float('inf'))
        return (d_lo, d_hi), (c_lo, c_hi), info

    # -------------------------------------------------------------------------------------------
    # Ts (SPEC Theorem TS): linear-fractional programme, Dinkelbach, every iterate certified
    # -------------------------------------------------------------------------------------------
    def _ts_step(self, t, mode):
        """sup (mode 'max') or inf over arrangements of sum m (u - t); returns (F, pos, mass)."""
        a = self.a
        sgn = 1.0 if mode == 'max' else -1.0
        k = np.nonzero(self.refl)[0]
        bestv = np.full(self.N, -np.inf * sgn)
        posv = np.zeros(self.N)
        thv = np.ones(self.N)
        if len(k):
            cands = [self.r0[k], self.r1[k]]
            if a > 0 and mode == 'max':
                cands.append(np.clip(t + 1.0 / a, self.r0[k], self.r1[k]))
            V = np.stack(cands, 1)
            val = np.exp(a * (self.cn[k][:, None] - V)) * (V - t)
            j = np.argmax(sgn * val, 1)
            ar = np.arange(len(k))
            bestv[k] = val[ar, j]
            posv[k] = V[ar, j]
            thv[k] = np.exp(a * (self.cn[k] - posv[k]))
        z = self.z0
        th_z = np.where((t > 0) == (mode == 'max'), self.th0_lo, self.th0_hi)
        vz = th_z * (-t)
        use_z = z & (sgn * vz > sgn * bestv)
        bestv = np.where(use_z, vz, bestv)
        posv = np.where(use_z, 0.0, posv)
        thv = np.where(use_z, th_z, thv)
        has = self.has
        mrec = np.where(sgn * bestv > 0, self.Bp, self.Bm)
        F = float(np.where(has, mrec * np.where(has, bestv, 0.0), 0.0).sum())
        pos = posv[has]
        mass = (mrec * thv)[has]
        if self.Tmax > 0:
            vt = self.tv1 if mode == 'max' else self.tv0
            tv = vt - t
            if sgn * tv > 0:
                F += self.Tmax * tv
                pos = np.concatenate([pos, [vt]])
                mass = np.concatenate([mass, [self.Tmax]])
        return F, pos, mass

    def _ts_cert(self, t, F, mode):
        """Every arrangement has N - t D <= F (max) or >= F (min), with D in [S0_lo, S0_hi]; so
        Ts <= t + F/D <= t + F/S0_lo (F >= 0) or t + F/S0_hi (F < 0); the min mirrors it."""
        if mode == 'max':
            return t + F / (self.S0_lo if F >= 0 else self.S0_hi)
        return t + F / (self.S0_lo if F <= 0 else self.S0_hi)

    def ts_band(self, max_iter=200):
        info = {}
        r = self.premise_refusal()
        if r:
            return None, r
        if self.Tmax > 0 and not math.isfinite(self.tv1):
            info['refused'] = 'truncated'
            info['why'] = ('up to %.3g of the energy may arrive unrecorded, at no known latest time: '
                           'Ts has no upper bound' % (self.Tmax / self.S0_lo))
            return None, info
        out = []
        wit = []
        iters = []
        for mode in ('min', 'max'):
            w = np.where(self.refl, self.Bp, 0.0)
            t = float((0.5 * (np.where(self.refl, self.r0, 0.0) + self.r1) * w).sum() / max(w.sum() + self.Bp[self.z0].sum(), TINY))
            cert = None
            it = 0
            for it in range(max(int(max_iter), 1)):
                F, pos, mass = self._ts_step(t, mode)
                c = self._ts_cert(t, F, mode)           # certified with the t at which F was taken
                cert = c if cert is None else (min(cert, c) if mode == 'max' else max(cert, c))
                D = mass.sum()
                t_new = float((pos * mass).sum() / D) if D > 0 else t
                wit.append(t_new)
                if abs(F) <= 1e-15 * max(1.0, abs(t)) or t_new == t:
                    break
                t = t_new
            iters.append(it + 1)
            out.append(cert)
        info['witness'] = (min(wit), max(wit))
        info['iterations'] = tuple(iters)
        return (out[0], out[1]), info

    # -------------------------------------------------------------------------------------------
    # EDT (SPEC section 4)
    # -------------------------------------------------------------------------------------------
    def _support(self):
        """Closed reading ranges that can hold energy at u > 0 (SPEC Lemma X): the reflected range of
        every bin with energy, and the unrecorded energy's range."""
        k = np.nonzero(self.refl)[0]
        lo = list(self.r0[k])
        hi = list(self.r1[k])
        if self.Tmax > 0:
            lo.append(self.tv0)
            hi.append(self.tv1)
        return np.asarray(lo, float), np.asarray(hi, float)

    def crossing_range(self):
        """[U_min, U_max]: every end the EDT window can have (SPEC Lemma X), or a refusal."""
        phi_lo = 0.1 * self.S0_lo
        phi_hi = 0.1 * self.S0_hi
        if self.Tmax >= phi_lo:
            return None, dict(refused='range_not_reached',
                              why='up to %.3g of the energy may be unrecorded: the -10 dB point may lie '
                                  'where the histogram does not see it' % (self.Tmax / self.S0_lo))
        if float(self.S_lo(np.array([1e-300]))[0]) < phi_hi:
            return None, dict(refused='range_in_arrival',
                              why='the level just after the arrival may already be below -10 dB: the '
                                  '-10 dB point may fall at the arrival itself')
        k_rb = np.nonzero(self.rb)[0]
        start = self.Hc + self.own_hi + self.Tmax           # S_hi at the start of bin k's grid
        okk = k_rb[start[k_rb] >= phi_lo]
        if len(okk) == 0:
            return None, dict(refused='range_in_arrival',
                              why='the level just after the arrival may already be below -10 dB')
        k = int(okk[-1])
        R = phi_lo - self.Hc[k] - self.Tmax
        if (not self.refl[k]) or self.a == 0 or R <= self.Bp[k] * self.th_r1[k]:
            U_max_raw = float(self.g1[k])
        else:
            U_max_raw = float(np.clip(self.cn[k] - math.log(R / self.Bp[k]) / self.a, self.r0[k], self.r1[k]))
        # U_min: S_lo(U+) <= phi_hi. On bin k's grid it is Lc[k] + own_lo[k] for U < r0[k], Lc[k] after.
        U_min_raw = None
        for kk in k_rb:
            if self.Lc[kk] + self.own_lo[kk] <= phi_hi:
                U_min_raw = float(self.g0[kk])
                break
            if self.Lc[kk] <= phi_hi:
                U_min_raw = float(max(self.g0[kk], min(self.r0[kk], self.g1[kk])))
                break
        if U_min_raw is None:
            U_min_raw = U_max_raw
        # U is a point of the closed support of the reading measure on u > 0 (SPEC Lemma X): snap
        # both ends onto the union of the ranges that can hold energy
        slo, shi = self._support()
        up = shi >= U_min_raw
        dn = slo <= U_max_raw
        info = dict(U_min_raw=U_min_raw, U_max_raw=U_max_raw, phi_lo=phi_lo, phi_hi=phi_hi)
        if not up.any() or not dn.any():
            info.update(refused='range_in_arrival', why='no reading time can hold the -10 dB crossing')
            return None, info
        U_min = float(np.min(np.maximum(slo[up], U_min_raw)))
        U_max = float(np.max(np.minimum(shi[dn], U_max_raw)))
        info.update(U_min=U_min, U_max=U_max)
        if not U_min <= U_max:
            info.update(refused='range_in_arrival', why='no reading time can hold the -10 dB crossing')
            return None, info
        cand = np.nonzero(self.refl & (self.r1 >= U_min) & (self.r0 <= U_max))[0]
        info['cand'] = cand.tolist()
        if np.any(self.z0[cand]):
            info['refused'] = 'range_in_arrival'
            info['why'] = ('the -10 dB point may lie in a bin that holds the direct sound (bins %s)'
                           % cand[self.z0[cand]].tolist())
            return None, info
        if not U_min > 0:
            info['refused'] = 'range_in_arrival'
            info['why'] = 'the -10 dB point cannot be kept away from the arrival (U_min = 0)'
            return None, info
        return (U_min, U_max), info

    def _geometry(self, Ua, Uw, mode):
        """Pieces of [0, Ua] split where the weight u - Uw/2 changes sign, with their tubes."""
        ustar = 0.5 * Uw
        idx = np.nonzero(self.rb & (self.g0 < Ua))[0]
        pa = self.g0[idx]
        pb = np.minimum(self.g1[idx], Ua)
        split = (pa < ustar) & (pb > ustar)
        A = np.concatenate([pa[~split], pa[split], np.full(int(split.sum()), ustar)])
        Bx = np.concatenate([pb[~split], np.full(int(split.sum()), ustar), pb[split]])
        K = np.concatenate([idx[~split], idx[split], idx[split]])
        o = np.lexsort((A, K))
        A, Bx, K = A[o], Bx[o], K[o]
        keep = Bx > A
        A, Bx, K = A[keep], Bx[keep], K[keep]
        neg = Bx <= ustar
        wint = 0.5 * ((Bx - ustar) ** 2 - (A - ustar) ** 2)
        phi_lo = 0.1 * self.S0_lo
        lo = np.maximum(self.S_lo(Bx), phi_lo)
        # on the open piece (A, B], S(u) <= S(A+) = rho((A, inf)): the bin whose range closes at A is
        # excluded (SPEC Lemma L)
        hi = np.maximum(self.S_hi_open(A), lo)
        chord = neg if mode == 'max' else ~neg
        wide = hi > lo * (1 + 1e-12)
        with np.errstate(divide='ignore', invalid='ignore'):
            cc = np.where(wide, (np.log(hi) - np.log(lo)) / (hi - lo), 1.0 / lo)
        ca = np.where(wide, np.log(lo) - cc * lo, np.log(lo) - 1.0)
        s0l, s0h = self.S0_lo, self.S0_hi
        if s0h > s0l * (1 + 1e-12):
            c0 = (math.log(s0h) - math.log(s0l)) / (s0h - s0l)
            a0 = math.log(s0l) - c0 * s0l
        else:
            c0, a0 = 1.0 / s0l, math.log(s0l) - 1.0
        return dict(Ua=Ua, ustar=ustar, mode=mode, A=A, B=Bx, K=K, neg=neg, wint=wint, lo=lo, hi=hi,
                    chord=chord, cc=cc, ca=ca, c0=c0, a0=a0, tan_init=hi.copy(),
                    Sa_init=float(self.S_hi(np.array([Ua]))[0]))

    def _tail_best(self, g, c, om_start, om_Ua, k_0, k_a, k_b, Ub, sgn):
        """The unrecorded energy (true mass in [0, Tmax], theta = 1) on its reading range
        [tv0, tv1]: the best position for psi and its value. psi is piecewise quadratic up to Ua and
        constant after, with jumps at Ua and Ub; the candidates hold its extremes."""
        tv0, tv1 = self.tv0, self.tv1
        Ua, us = g['Ua'], g['ustar']
        A, Bx = g['A'], g['B']
        cand = [tv0, np.nextafter(Ub, np.inf), Ua, Ub, us]
        if math.isfinite(tv1):
            cand.append(tv1)
        cand += list(A) + list(Bx)
        v = np.asarray(cand, float)
        v = v[(v >= tv0) & (v <= tv1)]
        if len(v) == 0:
            return None, 0.0
        psi = np.empty(len(v))
        below = v < Ua
        if below.any() and len(A):
            j = np.clip(np.searchsorted(A, v[below], side='right') - 1, 0, len(A) - 1)
            psi[below] = om_start[j] + 0.5 * c[j] * ((v[below] - us) ** 2 - (A[j] - us) ** 2) + k_0
        elif below.any():
            psi[below] = k_0
        mid = ~below & (v <= Ub)
        psi[mid] = om_Ua + k_0 + k_a
        psi[v > Ub] = om_Ua + k_0 + k_a + k_b
        j = int(np.argmax(sgn * psi))
        return float(v[j]), float(psi[j])

    def _eval(self, g, tang, Sa, e_a, e_0, Ub, lam1, lam2):
        """One linear relaxation (SPEC Lemmas L, S, D): its value, a valid bound on
            int_0^Ua (u - Uw/2) ln S du + e_a ln S(Ua) + e_0 (ln S(0) + ln 0.1)
        over the arrangements with S(Ua) >= S(0)/10 >= S(Ub+), and its optimiser."""
        self.n_eval += 1
        mode = g['mode']
        Ua, us = g['Ua'], g['ustar']
        a = self.a
        c = np.where(g['chord'], g['cc'], 1.0 / tang)
        al = np.where(g['chord'], g['ca'], np.log(tang) - 1.0)
        cw = c * g['wint']
        om_start = np.concatenate([[0.0], np.cumsum(cw)[:-1]]) if len(cw) else np.zeros(0)
        om_Ua = float(cw.sum())
        const = float((al * g['wint']).sum())
        # e_a ln S(Ua): e_a >= 0; max mode takes the tangent (upper bound); min mode never has e_a
        k_a = 0.0
        if e_a != 0.0:
            k_a += e_a / Sa
            const += e_a * (math.log(Sa) - 1.0)
        # e_0 (ln S(0) + ln 0.1): min mode, e_0 >= 0, needs a lower bound of ln S(0) (chord)
        k_0 = e_0 * g['c0']
        const += e_0 * (g['a0'] + LN_TENTH)
        if mode == 'max':
            sgn = 1.0
            k_a += lam1
            k_b = -lam2
            k_0 += -0.1 * lam1 + 0.1 * lam2
        else:
            sgn = -1.0
            k_a += -lam1
            k_b = lam2
            k_0 += 0.1 * lam1 - 0.1 * lam2
        V_list, bin_list, psi_list, kb_list = [], [], [], []
        m = self.refl[g['K']]
        if m.any():
            A, Bx, K, cp, osp = g['A'][m], g['B'][m], g['K'][m], c[m], om_start[m]
            r0, r1 = self.r0[K], self.r1[K]
            lo_v = np.maximum(A, r0)
            hi_v = np.minimum(Bx, r1)
            okp = lo_v <= hi_v
            A, Bx, K, cp, osp, lo_v, hi_v = A[okp], Bx[okp], K[okp], cp[okp], osp[okp], lo_v[okp], hi_v[okp]
            base = osp - 0.5 * cp * (A - us) ** 2 + k_0      # psi(v) = base + cp/2 (v-us)^2 (+k_a at Ua)
            cand = [lo_v, hi_v]
            if a > 0:
                qa = 0.5 * a * cp
                qb = -cp
                qc = a * base
                disc = qb * qb - 4 * qa * qc
                with np.errstate(invalid='ignore', divide='ignore'):
                    sq = np.sqrt(np.maximum(disc, 0.0))
                    y1 = np.where(qa != 0, (-qb + sq) / (2 * qa), np.nan)
                    y2 = np.where(qa != 0, (-qb - sq) / (2 * qa), np.nan)
                for y in (y1, y2):
                    v = us + y
                    cand.append(np.where((disc >= 0) & np.isfinite(v) & (v > lo_v) & (v < hi_v), v, lo_v))
            else:
                cand.append(np.clip(np.full(A.shape, us), lo_v, hi_v))
            for v in cand:
                V_list.append(v)
                bin_list.append(K)
                psi_list.append(base + 0.5 * cp * (v - us) ** 2 + np.where(v >= Ua, k_a, 0.0))
                kb_list.append(np.zeros(v.shape, bool))
        n_b = int(np.searchsorted(self.g0, Ub, side='right'))    # bins n >= n_b are read after Ub
        # bins after Ub that also have a u = 0 option are optimised one by one (their reflected
        # option: psi is the after-Ub constant, extremes at r0 and r1); the rest are aggregated
        late_z = np.nonzero(self.z0[n_b:])[0] + n_b
        lzr = late_z[self.refl[late_z]]
        if len(lzr):
            for v in (self.r0[lzr], self.r1[lzr]):
                V_list.append(v)
                bin_list.append(lzr)
                psi_list.append(np.full(len(lzr), om_Ua + k_0 + k_a + k_b))
                kb_list.append(np.ones(len(lzr), bool))
        kf = np.nonzero(self.refl[:n_b] & (self.r1[:n_b] > Ua))[0]
        if len(kf):
            s1 = np.maximum(self.r0[kf], Ua)
            e1 = np.minimum(self.r1[kf], Ub)
            ok1 = s1 <= e1
            for v in (s1, e1):
                V_list.append(v[ok1])
                bin_list.append(kf[ok1])
                psi_list.append(np.full(int(ok1.sum()), om_Ua + k_0 + k_a))
                kb_list.append(np.zeros(int(ok1.sum()), bool))
            ok2 = self.r1[kf] > Ub
            for v in (np.maximum(self.r0[kf], Ub)[ok2], self.r1[kf][ok2]):
                V_list.append(v)
                bin_list.append(kf[ok2])
                psi_list.append(np.full(int(ok2.sum()), om_Ua + k_0 + k_a + k_b))
                kb_list.append(np.ones(int(ok2.sum()), bool))
        best = np.full(self.N, -np.inf if sgn > 0 else np.inf)
        bpos = np.zeros(self.N)
        bth = np.ones(self.N)
        bkb = np.zeros(self.N, bool)
        if V_list:
            V = np.concatenate(V_list)
            KK = np.concatenate(bin_list)
            PS = np.concatenate(psi_list)
            KB = np.concatenate(kb_list)
            TH = np.exp(a * (self.cn[KK] - V))
            val = TH * PS
            o = np.lexsort((-sgn * val, KK))
            first = np.ones(len(o), bool)
            first[1:] = KK[o][1:] != KK[o][:-1]
            sel = o[first]
            kk = KK[sel]
            best[kk] = val[sel]
            bpos[kk] = V[sel]
            bth[kk] = TH[sel]
            bkb[kk] = KB[sel]
        thz = np.where((k_0 > 0) == (sgn > 0), self.th0_hi, self.th0_lo)
        vz = thz * k_0
        use_z = self.z0 & (sgn * vz > sgn * best)
        best = np.where(use_z, vz, best)
        bpos = np.where(use_z, 0.0, bpos)
        bth = np.where(use_z, thz, bth)
        bkb = np.where(use_z, False, bkb)
        has = self.has.copy()
        has[n_b:] = False
        has[late_z] = True
        bsafe = np.where(has, best, 0.0)
        mrec = np.where(sgn * bsafe > 0, self.Bp, self.Bm)
        contrib = float((np.where(has, mrec, 0.0) * bsafe).sum())
        psi_t = om_Ua + k_0 + k_a + k_b
        # bins after Ub: constant psi, so all at r0 with B+ or all at r1 with B- (SPEC Lemma P)
        big = (sgn * psi_t > 0)
        M_after = float(self.SP0[n_b] if big else self.SM1[n_b]) if n_b < self.N else 0.0
        contrib += psi_t * M_after
        # the unrecorded energy
        T, t_pos = 0.0, None
        if self.Tmax > 0:
            t_pos, t_psi = self._tail_best(g, c, om_start, om_Ua, k_0, k_a, k_b, Ub, sgn)
            if t_pos is not None and sgn * t_psi > 0:
                T = self.Tmax
                contrib += T * t_psi
        total = const + contrib
        pos = bpos[has]
        mass = (mrec * bth)[has]
        kbf = bkb[has]
        if n_b < self.N:
            ra = np.nonzero(self.refl[n_b:] & ~self.z0[n_b:])[0] + n_b
            pos = np.concatenate([pos, self.r0[ra] if big else self.r1[ra]])
            mass = np.concatenate([mass, (self.Bp[ra] * self.th_r0[ra]) if big else (self.Bm[ra] * self.th_r1[ra])])
            kbf = np.concatenate([kbf, np.ones(len(ra), bool)])
        S0 = mass.sum() + T
        in_a = (T if (T > 0 and t_pos >= Ua) else 0.0)
        in_b = (T if (T > 0 and t_pos > Ub) else 0.0)
        g1 = mass[pos >= Ua].sum() + in_a - 0.1 * S0
        g2 = 0.1 * S0 - mass[kbf | (pos > Ub)].sum() - in_b
        return dict(val=total, pos=pos, mass=mass, kb=kbf, T=T, t_pos=t_pos, g=(g1, g2))

    def _wit_arr(self, r, Ub):
        pos = np.where(r['kb'] & (r['pos'] <= Ub), np.nextafter(Ub, np.inf), r['pos'])
        if r['T'] > 0:
            return np.concatenate([pos, [r['t_pos']]]), np.concatenate([r['mass'], [r['T']]])
        return pos, r['mass']

    def _over(self):
        return self.n_eval >= self._budget

    def _solve(self, Ua, Ub, Uw, e_a, e_0, mode, n_tan=2, n_bis=18, n_double=40, n_rounds=2):
        """A certified bound (max mode: upper; min mode: lower) on
            N = int_0^Ua (u - Uw/2) ln S du + e_a ln S(Ua) + e_0 (ln S(0) + ln 0.1)
        over every admissible arrangement with S(Ua) >= S(0)/10 >= S(Ub+), and witnesses.
        Every relaxation evaluated is a valid bound (SPEC Lemma D: any lambda >= 0), so stopping at
        any point, including on the work budget, returns a valid bound."""
        g = self._geometry(Ua, Uw, mode)
        tang = g['tan_init'].copy()
        Sa = g['Sa_init']
        sgn = 1.0 if mode == 'max' else -1.0
        better = (lambda x, y: min(x, y)) if sgn > 0 else (lambda x, y: max(x, y))
        best = None
        wits = []
        lam0 = max(Ua * Ua, 1e-12)
        for it in range(n_tan):
            if it > 0 and self._over():
                break
            lam = [0.0, 0.0]
            r = self._eval(g, tang, Sa, e_a, e_0, Ub, 0.0, 0.0)
            best = r['val'] if best is None else better(best, r['val'])
            pair = None
            for _rnd in range(n_rounds):
                moved = False
                for i in (0, 1):
                    if r['g'][i] >= 0 or self._over():
                        continue
                    lo_l, r_lo = lam[i], r
                    hi_l = max(4 * lam[i], lam0)
                    r_hi = None
                    for _k in range(n_double):
                        if self._over():
                            break
                        l2 = list(lam)
                        l2[i] = hi_l
                        rr = self._eval(g, tang, Sa, e_a, e_0, Ub, *l2)
                        best = better(best, rr['val'])
                        if rr['g'][i] >= 0:
                            r_hi = rr
                            break
                        lo_l, r_lo = hi_l, rr
                        hi_l *= 4.0
                    if r_hi is None:
                        break
                    for _k in range(n_bis):
                        if self._over():
                            break
                        mid = 0.5 * (lo_l + hi_l)
                        l2 = list(lam)
                        l2[i] = mid
                        rr = self._eval(g, tang, Sa, e_a, e_0, Ub, *l2)
                        best = better(best, rr['val'])
                        if rr['g'][i] >= 0:
                            hi_l, r_hi = mid, rr
                        else:
                            lo_l, r_lo = mid, rr
                    lam[i] = hi_l
                    r = r_hi
                    pair = (r_lo, r_hi, i)
                    moved = True
                if not moved:
                    break
            wits.append(self._wit_arr(r, Ub))
            wp, wm = wits[-1]
            if pair is not None:
                r_lo, r_hi, i = pair
                gl, gh = r_lo['g'][i], r_hi['g'][i]
                al = gh / (gh - gl) if gh - gl > 0 else 0.0
                pl, ml = self._wit_arr(r_lo, Ub)
                ph, mh = self._wit_arr(r_hi, Ub)
                wp = np.concatenate([pl, ph])
                wm = np.concatenate([al * ml, (1 - al) * mh])
                wits.append((pl, ml))
                wits.append((wp, wm))
            mid = 0.5 * (g['A'] + g['B'])
            sw = _curve_at(wp, wm, mid)
            tang = np.where(sw > 0, sw, tang)
            sa = float(_curve_at(wp, wm, np.array([Ua]))[0])
            if sa > 0:
                Sa = sa
        return best, wits

    def _sub_bound(self, Ua, Ub, mode, wit_sink):
        """SPEC Theorem E on one window-end interval [Ua, Ub], Ub <= 2 Ua: a bound on the slope (dB/s).
        Lemma W: for every admissible arrangement with U in [Ua, Ub], x = U - Ua,
            G-(x) <= J(U) <= G+(x), both affine in x, with
            G(0)   = int_0^Ua (u - Ua/2) ln S du                              (both)
            G+(d)  = int_0^Ua (u - Ub/2) ln S du + (d Ua/2) ln S(Ua)
            G-(d)  = int_0^Ua (u - Ub/2) ln S du + (d Ua/2)(ln S(0) + ln 0.1)."""
        d = Ub - Ua
        if d > Ua * (1 + 1e-12):
            raise AssertionError('window-end interval violates Ub <= 2 Ua')
        m0, w0 = self._solve(Ua, Ub, Ua, 0.0, 0.0, mode)
        wits = list(w0)
        if d > 0:
            if mode == 'max':
                md, wd = self._solve(Ua, Ub, Ub, d * Ua / 2.0, 0.0, mode)
            else:
                md, wd = self._solve(Ua, Ub, Ub, 0.0, Ua * d / 2.0, mode)
            wits += wd
        else:
            md = m0
        for p, mm in wits:
            e = exact_params(p, mm, te_list=())['edt']
            if e == float('inf'):
                self._wit_nondecaying = True
            elif np.isfinite(e):
                wit_sink.append(e)
        # outward rounding margin: the relaxation's value is a sum of terms of size up to
        # Ua^2 (1 + |ln S|); float rounding in it is far below 1e-10 of that. The proof is in exact
        # arithmetic; this margin keeps a slope of exactly 0 from rounding to a tiny negative.
        eta = 1e-10 * Ua * Ua * (1.0 + abs(math.log(0.1 * self.S0_lo)) + abs(math.log(self.S0_hi)))
        sg = 1.0 if mode == 'max' else -1.0
        return DB * 12.0 * _chord_ratio_extreme(m0 + sg * eta, md + sg * eta, Ua, d, mode)

    def edt_band(self, gap_rel=2e-4, max_split=40, min_width_bins=2.0 ** -10, max_relax=6000, max_work=2e6,
                 stall=4):
        """[EDT_lo, EDT_hi] (s), certified, with the witness band inside it.

        The work budget: at most min(max_relax, max(64, max_work / W)) relaxations, W the bins up to
        U_max (each relaxation is O(W)). The initial cover always completes (at least one relaxation
        per bound); refinement and the lambda searches stop when the budget is spent. Every bound
        computed is valid, so the band is valid whenever it stops (SPEC section 7)."""
        r = self.premise_refusal()
        if r:
            return None, r
        rng, info = self.crossing_range()
        if rng is None:
            return None, info
        U_min, U_max = rng
        # initial cover: geometric, so that every window-end interval has Ub <= 2 Ua (SPEC Lemma W)
        edges = [U_min]
        while edges[-1] * 2.0 < U_max:
            edges.append(edges[-1] * 2.0)
            if len(edges) > MAX_INITIAL_INTERVALS:
                info['refused'] = 'range_in_arrival'
                info['why'] = ('the window end can lie anywhere from %.3g s to %.3g s after the arrival '
                               '(ratio above 2^%d)' % (U_min, U_max, MAX_INITIAL_INTERVALS))
                return None, info
        edges.append(U_max)
        subs = [(float(edges[i]), float(edges[i + 1])) for i in range(len(edges) - 1)]
        self.n_eval = 0
        W = max(int(np.searchsorted(self.g0, U_max, side='right')), 1)
        self._budget = float(min(max_relax, max(64.0, max_work / W)))
        self._wit_nondecaying = False
        wit = []
        hi_list = [(self._sub_bound(s[0], s[1], 'max', wit), s) for s in subs]
        lo_list = [(self._sub_bound(s[0], s[1], 'min', wit), s) for s in subs]
        n_split = 0
        exhausted = False
        for side in ('hi', 'lo'):
            lst = hi_list if side == 'hi' else lo_list
            mode = 'max' if side == 'hi' else 'min'
            hist = []
            for _ in range(max_split):
                if self._over():
                    exhausted = True
                    break
                if side == 'hi':
                    sl, s = max(lst, key=lambda t: t[0])
                    cert = -60.0 / sl if sl < 0 else float('inf')
                    w = max(wit) if wit else float('nan')
                    gap = (cert - w) / w if np.isfinite(w) else float('inf')
                else:
                    sl, s = min(lst, key=lambda t: t[0])
                    cert = -60.0 / sl if sl < 0 else float('inf')
                    w = min(wit) if wit else float('nan')
                    gap = (w - cert) / w if np.isfinite(w) else float('inf')
                if gap <= gap_rel or (s[1] - s[0]) <= min_width_bins * self.dt:
                    break
                # stop when splitting has stopped paying: the rest of the gap is the linearisation's
                # (second order in a dt), not the window's
                hist.append(cert)
                if stall and len(hist) > stall and abs(hist[-stall - 1] - hist[-1]) < 0.1 * abs(cert - w):
                    break
                lst.remove((sl, s))
                mid = 0.5 * (s[0] + s[1])
                for part in ((s[0], mid), (mid, s[1])):
                    lst.append((self._sub_bound(part[0], part[1], mode, wit), part))
                n_split += 1
        s_hi = max(t[0] for t in hi_list)
        s_lo = min(t[0] for t in lo_list)
        # consistency guard: witnesses are admissible values, so the band contains them in exact
        # arithmetic; where rounding says otherwise, the band is widened to them
        if self._wit_nondecaying:
            s_hi = max(s_hi, 0.0)
        edt_hi = -60.0 / s_hi if s_hi < 0 else float('inf')
        edt_lo = -60.0 / s_lo if s_lo < 0 else float('inf')
        widened = False
        if wit:
            if min(wit) < edt_lo:
                edt_lo, widened = min(wit), True
            if max(wit) > edt_hi:
                edt_hi, widened = max(wit), True
        info.update(slope_hi_db=s_hi, slope_lo_db=s_lo, n_sub=len(hi_list) + len(lo_list), n_split=n_split,
                    n_initial=len(subs), n_eval=self.n_eval, budget=self._budget, window_bins=W,
                    budget_exhausted=exhausted, widened_to_witness=widened,
                    witness=(min(wit), max(wit)) if wit else None,
                    witness_nondecaying=self._wit_nondecaying)
        if not s_hi < 0:
            info['refused'] = 'not_decaying'
            if self._wit_nondecaying:
                info['why'] = ('an admissible arrangement (a witness) has a least-squares slope >= 0 over '
                               'EDT\'s range')
            else:
                info['why'] = ('the certified upper bound on the slope is >= 0: a non-decaying arrangement '
                               'is not ruled out')
        self._budget = float('inf')
        return (edt_lo, edt_hi), info

    # -------------------------------------------------------------------------------------------
    def all_bands(self, tau=None, upstream=True):
        """Every early parameter: band, point value, half-width, refusal (SPEC section 6).

        GATE 4b run-too-short refusal (decision-log row 14, comment above
        GATE2_TAIL_REFUSAL_TOLERANCE_SHARE): when this Setup carries a positive tail_max
        (self.Tmax > 0), every quantity's band is also solved with an identical Setup ("notail")
        whose tail_max is 0 (same B, eps, everything else). The half-width the tail adds --
        tail_widening, in that quantity's own units (relative for EDT, seconds for Ts, dB for C,
        dimensionless for D) -- is recorded on the quantity's output. When the tail-inclusive
        half-width exceeds tau (what would otherwise be reported as 'band_too_wide'), the more
        specific 'run_too_short' is reported instead if tail_widening alone already exceeds
        GATE2_TAIL_REFUSAL_TOLERANCE_SHARE of tau -- i.e. the tail estimate, not the recorded bins,
        is what pushed the band over -- so the run-too-short refusal takes priority over the generic
        band-too-wide one whenever the tail is the material cause; each quantity is judged
        independently.
        """
        user = dict(tau or {})
        tau = dict(LIMIT, **user)
        notail = None
        if self.Tmax > 0:
            notail = Setup(self.B_rec, self.dt, self.ta, half_width=self.h, air_rate=self.a,
                            kappa=self.kappa, rel_eps=self.eps, tail_max=0.0,
                            tail_t_max=self.tail_t_max, t_refl_min=self.t_refl_min,
                            misfit_tol=self.misfit_tol, tail_t_min=self.tail_t_min,
                            n_sources=self.n_sources, homogeneous_celerity=self.homogeneous_celerity)

        def widening(band, band_n, relative):
            hw, hwn = _halfwidth(band, relative), _halfwidth(band_n, relative)
            return None if (hw is None or hwn is None) else hw - hwn

        res = {}
        band, info = self.edt_band()
        w = widening(band, notail.edt_band()[0], True) if notail is not None else None
        res['edt'] = _package('edt', band, info, tau['edt'], relative=True, tail_widening=w)
        if upstream:
            res['edt']['isimpa'] = upstream_edt(self.B_rec, self.dt)
        band, info = self.ts_band()
        if 'ts' in user or band is None:
            tau_ts = tau['ts']
        else:
            tau_ts = min(LIMIT['ts'], TS_RELATIVE * 0.5 * (band[0] + band[1]))
        w = widening(band, notail.ts_band()[0], False) if notail is not None else None
        res['ts'] = _package('ts', band, info, tau_ts, relative=False, tail_widening=w)
        res['ts']['tau'] = tau_ts
        pr = self.premise_refusal()
        for te in (0.05, 0.08):
            if pr:
                dband, cband, info = None, None, dict(pr)
            else:
                dband, cband, info = self.cd_band(te)
            w_c = w_d = None
            if notail is not None and not pr:
                dband_n, cband_n, _ = notail.cd_band(te)
                w_c = widening(cband, cband_n, False)
                if te == 0.05:
                    w_d = widening(dband, dband_n, False)
            tag = '%g' % (te * 1e3)
            res['c' + tag] = _package('c', cband, dict(info), tau['c'], relative=False, tail_widening=w_c)
            if te == 0.05:
                res['d' + tag] = _package('d', dband, dict(info), tau['d'], relative=False, tail_widening=w_d)
        res['diagnostics'] = dict(misfit_share=self.misfit_share, S0_ratio=self.S0_hi / self.S0_lo)
        return res


def _halfwidth(band, relative):
    """The half-width all_bands()/_package() would report for band = (lo, hi), or None if the
    band is missing/unbounded -- used to measure tail_widening between a with-tail and a
    tail_max=0 run of the same Setup (GATE 4b)."""
    if band is None:
        return None
    lo, hi = band
    if not (np.isfinite(lo) and np.isfinite(hi)):
        return None
    return (hi - lo) / (hi + lo) if relative else 0.5 * (hi - lo)


def _package(q, band, info, tau, relative, tail_widening=None):
    out = dict(info)
    if band is None:
        out.setdefault('refused', 'refused')
        out['band'] = None
        out['value'] = None
        return out
    lo, hi = band
    out['band'] = (lo, hi)
    if not (np.isfinite(lo) and np.isfinite(hi)):
        out['value'] = None
        out.setdefault('refused', 'unbounded')
        return out
    if relative:
        # the value whose largest relative error to any point of the band is least
        value = 2 * lo * hi / (lo + hi)
        hw = (hi - lo) / (hi + lo)
        out['half_width_rel'] = hw
        out['half_width_pct'] = 100 * hw
    else:
        value = 0.5 * (lo + hi)
        hw = 0.5 * (hi - lo)
        out['half_width'] = hw
    out['half_width_L'] = hw / LIMIT[q]
    out['half_width_JND'] = hw / JND[q]
    out['value'] = value
    out['tau'] = tau
    if tail_widening is not None:
        out['tail_widening'] = tail_widening
    if 'refused' not in out and hw > tau:
        # GATE 4b: when the tail estimate alone already eats more than
        # GATE2_TAIL_REFUSAL_TOLERANCE_SHARE of tau, the tail is the material cause of the
        # band being too wide, so the more specific 'run_too_short' is reported instead of the
        # generic 'band_too_wide' (comment above GATE2_TAIL_REFUSAL_TOLERANCE_SHARE).
        if tail_widening is not None and tail_widening > GATE2_TAIL_REFUSAL_TOLERANCE_SHARE * tau:
            out['refused'] = 'run_too_short'
            out['why'] = ('tail widening %.4g exceeds %.4g of tau %.4g (GATE2_TAIL_REFUSAL_'
                          'TOLERANCE_SHARE, decision-log row 14)' %
                          (tail_widening, GATE2_TAIL_REFUSAL_TOLERANCE_SHARE, tau))
        else:
            out['refused'] = 'band_too_wide'
            out['why'] = 'half-width %.4g exceeds tau %.4g' % (hw, tau)
    return out


# ------------------------------------------------------------------------------------------------
# The I-Simpa-compatible value: upstream's EDT, through the validated port (read-only import)
# ------------------------------------------------------------------------------------------------
_UP = None


def _upstream_module():
    global _UP
    if _UP is None:
        sys.dont_write_bytecode = True
        path = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'upstream-edt',
                                             'uphunt_upstream_edt.py'))
        spec = importlib.util.spec_from_file_location('uphunt_upstream_edt_ro', path)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        _UP = mod
    return _UP


def upstream_edt(B, dt, kind='point'):
    """Upstream I-Simpa's EDT (s) of the recorded series, exactly as its GUI computes it
    (projet_calculation.cpp:68-219, float32), via target/agents/upstream-edt."""
    val, _det = _upstream_module().edt(np.asarray(B, np.float32), dt, kind=kind)
    return val
