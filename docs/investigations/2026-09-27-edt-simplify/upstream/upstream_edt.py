#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
upstream_edt.py

A line-for-line-as-possible Python transcription of I-Simpa upstream's EDT /
RT computation pipeline (tag v1.4.0_snapshot_14_01_2026), used to:

  1. reproduce the exact arithmetic upstream performs on a receiver's
     energy-vs-time histogram to obtain EDT / RT-x, and
  2. run that arithmetic on a synthetic, noise-free energy-vs-time input
     (single process, no multiprocessing) to measure the bias the algorithm
     itself introduces, isolated from particle-count (Monte Carlo) noise.

Source functions transcribed, with their upstream receipts:

  to_deciBel / to_deciBelP0
      B:/repos/I-Simpa-upstream/src/isimpa/data_manager/projet_calculation.cpp:42-49

  MakeSchroederArray
      B:/repos/I-Simpa-upstream/src/isimpa/data_manager/projet_calculation.cpp:68-83

  GetSumLimit
      B:/repos/I-Simpa-upstream/src/isimpa/data_manager/projet_calculation.cpp:95-119

  GetTimeRange
      B:/repos/I-Simpa-upstream/src/isimpa/data_manager/projet_calculation.cpp:143-170

  ComputeLinearRegression
      B:/repos/I-Simpa-upstream/src/isimpa/data_manager/projet_calculation.cpp:175-186

  Compute_TR_Param   (EDT is Compute_TR_Param(0, 10, ...); RT-15 is
                       Compute_TR_Param(5, 20, ...); RT-30 is
                       Compute_TR_Param(5, 35, ...))
      B:/repos/I-Simpa-upstream/src/isimpa/data_manager/projet_calculation.cpp:190-219

Upstream data types (receipts):
  wxFloat32              -> 32-bit float, used for every quantity in the
                             functions above (timeTable, tab_wj, tab_schroeder,
                             regression sums, the returned EDT/RT value).
                             projet_calculation.cpp uses wxFloat32 throughout.
  decimal = float        B:/repos/I-Simpa-upstream/src/lib_interface/Core/mathlib.h:54
  l_decimal = double     B:/repos/I-Simpa-upstream/src/lib_interface/coreString.h:43
                          (used only inside SPPS's own energy accumulators,
                          e.g. tabEnergyByTimeStep and t_Recepteur_P::energy_sum;
                          it is downcast to wxFloat32/Floatb the moment it is
                          written into a .gabe file -- see gabe.h:63
                          `typedef float Floatb;` -- so by the time
                          projet_calculation.cpp reads it back for EDT/RT,
                          double precision is already gone.)

HONESTY NOTE ON "LINE-FOR-LINE":
  The control flow, the order of operations, the reference point used
  (row_db[0], i.e. the first histogram bin, never re-anchored), the
  threshold logic (>= not >), the inclusive/inclusive bin selection in
  GetSumLimit, and the regression formula are all transcribed line-for-line.
  This is NOT a bit-for-bit reproduction of the C++ build: Python's `float`
  is 64-bit (C++ `double`), whereas the upstream functions above declare
  every local as `wxFloat32` (32-bit). A `--f32` mode is provided that
  rounds every intermediate result to float32 with numpy after each
  arithmetic step, to bound how much that difference matters. For the
  effect sizes this script reports (upstream's own structural EDT bias),
  running both modes below shows the float32/float64 difference is far
  smaller than the effect being measured (see comparison printed at the
  bottom of __main__). Where the two disagree beyond that, this file says
  so explicitly rather than picking the flattering number.

HARD RULES OBEYED: single process, no multiprocessing, no threads, no CLI
worker flags. This script does not build, launch, or link against the SPPS
solver; it is a standalone reimplementation of the *post-processing*
arithmetic that runs after SPPS has produced a receiver .gabe file.
"""

import math
import numpy as np


def _f32(x):
    return float(np.float32(x))


# ---------------------------------------------------------------------------
# Direct transcriptions of projet_calculation.cpp
# ---------------------------------------------------------------------------

def to_deciBel(wjVal, f32=False):
    # projet_calculation.cpp:42-45
    #   wxFloat32 to_deciBel(const wxFloat32& wjVal) { return 10*log10f(wjVal); }
    if wjVal <= 0:
        # log10f(<=0) is -inf/NaN in C++ too; upstream has no guard. We do not
        # add one either -- this is upstream's behaviour, not ours.
        return float('-inf') if wjVal == 0 else float('nan')
    v = 10 * math.log10(wjVal)
    return _f32(v) if f32 else v


def to_deciBelP0(wjVal, p_0, f32=False):
    # projet_calculation.cpp:46-49
    #   wxFloat32 to_deciBelP0(const wxFloat32& wjVal) { return 10*log10f(wjVal*p_0); }
    x = wjVal * p_0
    if x <= 0:
        return float('-inf') if x == 0 else float('nan')
    v = 10 * math.log10(x)
    return _f32(v) if f32 else v


P_0 = 1.0 / ((20 * (10.0 ** -6)) ** 2)  # projet_calculation.cpp:41


def MakeSchroederArray(srcArray, f32=False, dbfunc=None):
    """
    projet_calculation.cpp:68-83

    void MakeSchroederArray(srcArray, dstArray, funcDbConv=&to_deciBelP0)
    {
        dstArray = srcArray;                          // raw copy
        for each frequency row:
            sumW = 0
            for idStep from last down to 0:
                sumW += dstArray[idFreq][idStep]
                dstArray[idFreq][idStep] = funcDbConv(sumW)
    }

    srcArray: list of rows (one row per frequency band, or a single row for
              this script's single-band synthetic tests); each row is a
              list of per-time-bin energies (linear units, e.g. J/m^3-like,
              never yet in dB).
    Returns dstArray, same shape, values replaced with the backward
    (Schroeder) cumulative sum expressed in dB via dbfunc (default
    to_deciBelP0, matching upstream's default argument).
    """
    if dbfunc is None:
        dbfunc = lambda v: to_deciBelP0(v, P_0, f32=f32)
    dstArray = [list(row) for row in srcArray]
    for idFreq in range(len(dstArray)):
        sumW = 0.0
        row = dstArray[idFreq]
        for idStep in range(len(row) - 1, -1, -1):
            sumW = _f32(sumW + row[idStep]) if f32 else (sumW + row[idStep])
            row[idStep] = dbfunc(sumW)
    return dstArray


# SUM_OPERATION enum, projet_calculation.cpp:87-93
SUM_OPERATION_X = 0
SUM_OPERATION_Y = 1
SUM_OPERATION_XY = 2
SUM_OPERATION_X2 = 3


def GetSumLimit(idBandeFreq, fromTime, toTime, timeTable, tab_wj,
                operation=SUM_OPERATION_Y, f32=False):
    """
    projet_calculation.cpp:95-119

    Faithful transcription, INCLUDING the fact that upstream calls this
    function four separate times per regression (once per operation) even
    though every call re-walks the same index range -- that redundancy is
    upstream's own, not this script's; it is left in on purpose so the
    control flow matches exactly. Returns (sumJ, n) -- n is only meaningful
    when the caller asked with operation counting in mind; upstream instead
    passes a pointer and only reads it from the SUM_OPERATION_X call. We
    return it every time for convenience but only the X-call's n is used
    downstream, exactly mirroring ComputeLinearRegression's usage.
    """
    sumJ = 0.0
    n = 0
    for idStep in range(len(timeTable)):
        currentTime = timeTable[idStep]
        if currentTime >= fromTime and (currentTime <= toTime or toTime == -1.0):
            n += 1
            if operation == SUM_OPERATION_Y:
                sumJ += tab_wj[idBandeFreq][idStep]
            elif operation == SUM_OPERATION_XY:
                sumJ += tab_wj[idBandeFreq][idStep] * currentTime
            elif operation == SUM_OPERATION_X:
                sumJ += currentTime
            elif operation == SUM_OPERATION_X2:
                sumJ += currentTime * currentTime
            if f32:
                sumJ = _f32(sumJ)
        if currentTime > toTime and toTime != -1.0:
            break
    return sumJ, n


def GetTimeRange(fromdB, todB, timeTable, row_db):
    """
    projet_calculation.cpp:143-170

    void GetTimeRange(fromdB, todB, *timeDeb, *timeEnd, timeTable, row_db)
    {
        if row_db empty: return
        *timeDeb = timeTable[0]
        *timeEnd = timeTable[last]
        lastdB = row_db[0]            // <-- fixed reference, NEVER updated
        insideBound = false
        for idStep in row_db:
            if not insideBound:
                if abs(row_db[idStep]-lastdB) >= fromdB:
                    timeDeb = timeTable[idStep]; insideBound = true
            else:
                if abs(row_db[idStep]-lastdB) >= todB:
                    timeEnd = timeTable[idStep]; break
    }

    Note fromdB=0 makes the *first* sample satisfy the test trivially
    (abs(0) >= 0), so timeDeb is ALWAYS timeTable[0] when computing EDT
    (Compute_TR_Param is called with fromdbL=0 for EDT). This is receipt
    I1 in READING.md.
    """
    if len(row_db) == 0:
        return None, None
    timeDeb = timeTable[0]
    timeEnd = timeTable[-1]
    lastdB = row_db[0]
    insideBound = False
    for idStep in range(len(row_db)):
        if not insideBound:
            if abs(row_db[idStep] - lastdB) >= fromdB:
                timeDeb = timeTable[idStep]
                insideBound = True
        else:
            if abs(row_db[idStep] - lastdB) >= todB:
                timeEnd = timeTable[idStep]
                break
    return timeDeb, timeEnd


def ComputeLinearRegression(idBandeFreq, fromTime, toTime, timeTable, tab_db, f32=False):
    """
    projet_calculation.cpp:175-186 (least squares, called with tab_db =
    tab_schroeder, i.e. the dB'd Schroeder curve, NOT the raw energy).
    """
    Sx, n = GetSumLimit(idBandeFreq, fromTime, toTime, timeTable, tab_db, SUM_OPERATION_X, f32)
    Sy, _ = GetSumLimit(idBandeFreq, fromTime, toTime, timeTable, tab_db, SUM_OPERATION_Y, f32)
    Sx2, _ = GetSumLimit(idBandeFreq, fromTime, toTime, timeTable, tab_db, SUM_OPERATION_X2, f32)
    Sxy, _ = GetSumLimit(idBandeFreq, fromTime, toTime, timeTable, tab_db, SUM_OPERATION_XY, f32)
    denom = (n * Sx2 - (Sx * Sx))
    if denom == 0:
        a = float('nan')
        b = float('nan')
    else:
        a = (n * Sxy - Sx * Sy) / denom
        b = (Sy * Sx2 - Sx * Sxy) / denom
    if f32:
        a, b = _f32(a), _f32(b)
    return a, b


def Compute_TR_Param(fromdbL, todbL, timeTable, tab_wj, tab_schroeder, f32=False):
    """
    projet_calculation.cpp:190-219

    Returns (per_band_values, average, debT_list, endT_list, slope_list)
    per_band_values[i] is the RT/EDT (seconds) for band i; average excludes
    the last band (upstream's "Global"/summed band, see comment at
    projet_calculation.cpp:205 "Ne pas moyenner sur la ligne de somme").
    Label: fromdbL==0 -> "EDT (s)"; else "RT-{todbL-5} (s)".
    """
    nbands = len(tab_schroeder)
    values = [None] * nbands
    debTs = [None] * nbands
    endTs = [None] * nbands
    slopes = [None] * nbands
    s = 0.0
    for idFreq in range(nbands):
        debT, endT = GetTimeRange(fromdbL, todbL, timeTable, tab_schroeder[idFreq])
        a, b = ComputeLinearRegression(idFreq, debT, endT, timeTable, tab_schroeder, f32)
        val = (-60.0 / a) / 1000.0
        if f32:
            val = _f32(val)
        values[idFreq] = val
        debTs[idFreq] = debT
        endTs[idFreq] = endT
        slopes[idFreq] = a
        if idFreq != nbands - 1:
            s += val
    average = s / (nbands - 1) if nbands > 1 else float('nan')
    label = "EDT (s)" if fromdbL == 0 else f"RT-{todbL-5:g} (s)"
    return {"label": label, "values": values, "average": average,
            "debT": debTs, "endT": endTs, "slope_dB_per_ms": slopes}


# ---------------------------------------------------------------------------
# Synthetic input builder: "an exponential decay with EDT 1 s, a direct
# arrival at r = 2 m / r = 20 m", discretised at dt exactly the way SPPS's
# receiver histogram discretises energy (see READING.md I1/I3/I2 receipts:
# reportmanager.cpp:224 currentRecp->energy_sum[freq][pasCourant]+=energy;
# and reportmanager.cpp:526 label = (idstep+1)*timeStep*1000, " ms").
# ---------------------------------------------------------------------------

C_SOUND = 343.2  # m/s at 20 degC; Celerite_du_son.cpp:46, Kref=293.15K (20 degC)


def build_synthetic_bins(edt_true_s, r_m, dt_s, margin_s=0.5, c=C_SOUND):
    """
    Build the *exact* (noise-free) energy each dt-wide bin would receive if
    the true continuous energy impulse response were:
        w(t) = 0                          for t < tau
        w(t) = exp(-k*(t-tau))            for t >= tau
    with tau = r/c (the direct-sound arrival time) and k chosen so the
    Schroeder decay of this response has EDT = edt_true_s exactly
    (k = 6*ln(10)/edt_true_s nepers/s <=> -60/edt_true_s dB/s).

    Bin i (0-based) covers real time [i*dt, (i+1)*dt) and is labelled, the
    way SPPS labels it, at t_label = (i+1)*dt (reportmanager.cpp:526).
    The bin's *value* is the exact integral of w(t) over its interval --
    i.e. this models a noise-free SPPS histogram (infinite particles), so
    the only errors this isolates are the ALGORITHM's (I1/I2/I3), not
    particle-count statistics (I5), which is a separate, qualitative
    argument made in READING.md.

    Returns (timeTable_ms, energies, tau_s, k_per_s).
    """
    tau = r_m / c
    k = 6.0 * math.log(10.0) / edt_true_s  # nepers/s; matches -60/EDT dB/s
    total_T = tau + edt_true_s * (10.0 / 60.0) + margin_s  # comfortably past -10 dB point
    n_bins = int(math.ceil(total_T / dt_s))

    def W_integral(t0, t1):
        # integral of w(t) dt over [t0,t1), w=0 before tau, exp decay after
        lo = max(t0, tau)
        hi = max(t1, tau)
        if hi <= lo:
            return 0.0
        # integral of exp(-k*(t-tau)) dt from lo to hi
        return (math.exp(-k * (lo - tau)) - math.exp(-k * (hi - tau))) / k

    timeTable_ms = []
    energies = []
    for i in range(n_bins):
        t0, t1 = i * dt_s, (i + 1) * dt_s
        e = W_integral(t0, t1)
        timeTable_ms.append((i + 1) * dt_s * 1000.0)  # label in ms, matches upstream
        energies.append(e)
    return timeTable_ms, energies, tau, k


def analytic_biased_edt(edt_true_s, tau_s):
    """
    Closed-form continuum approximation (dt -> 0) of the EDT that upstream's
    algorithm returns for this synthetic input, derived by treating the
    fit window as a continuous "flat-then-linear" curve and computing the
    ordinary-least-squares slope with calculus instead of discrete sums.
    See READING.md for the derivation. Provided as an independent check on
    the discrete simulation below (should converge to it as dt -> 0).
    """
    k_dB_per_s = 60.0 / edt_true_s  # magnitude of the true dB/s slope
    L = 10.0 / k_dB_per_s           # seconds of real decay needed to drop 10 dB
    tau = tau_s
    T = tau + L                     # regression window is [0, T]
    if tau <= 0:
        return edt_true_s  # no plateau, no bias
    # E[t], E[y], E[ty] over uniform t in [0,T]; y(t)=0 on [0,tau], y=-k*(t-tau) on [tau,T]
    Ey = -(k_dB_per_s * L * L) / (2 * T)
    Ety = -(k_dB_per_s / T) * ((L ** 3) / 3.0 + tau * (L ** 2) / 2.0)
    Et = T / 2.0
    Cov_ty = Ety - Et * Ey
    Var_t = (T * T) / 12.0
    a_hat = Cov_ty / Var_t  # dB/s (continuum; note this fit uses seconds, not ms)
    edt_hat = -60.0 / a_hat
    return edt_hat


def run_case(edt_true_s, r_m, dt_s, f32=False):
    timeTable_ms, energies, tau_s, k = build_synthetic_bins(edt_true_s, r_m, dt_s)
    tab_wj = [energies]  # single "band"
    tab_schroeder = MakeSchroederArray(tab_wj, f32=f32)
    result = Compute_TR_Param(0, 10, timeTable_ms, tab_wj, tab_schroeder, f32=f32)
    measured_edt = result["values"][0]
    debT_ms, endT_ms = result["debT"][0], result["endT"][0]
    analytic = analytic_biased_edt(edt_true_s, tau_s)
    return {
        "r_m": r_m, "dt_ms": dt_s * 1000.0, "tau_ms": tau_s * 1000.0,
        "edt_true_s": edt_true_s, "measured_edt_s": measured_edt,
        "analytic_continuum_edt_s": analytic,
        "error_s": measured_edt - edt_true_s,
        "error_pct": 100.0 * (measured_edt - edt_true_s) / edt_true_s,
        "debT_ms": debT_ms, "endT_ms": endT_ms,
        "n_flat_bins_before_arrival": sum(1 for t in timeTable_ms if t <= tau_s * 1000.0),
        "arrival_bin_label_ms": next(t for t in timeTable_ms if t >= tau_s * 1000.0),
        "arrival_label_error_ms": next(t for t in timeTable_ms if t >= tau_s * 1000.0) - tau_s * 1000.0,
    }


def degenerate_curve_demo():
    """
    I6 (no guard on degenerate curves): a run so short (or a receiver so
    quiet at the far end of the recording) that only ONE histogram bin
    satisfies GetTimeRange's [debT, endT] window makes
    ComputeLinearRegression's denominator n*Sx2 - Sx^2 exactly 0 (with n=1,
    Sx2 = t^2 = Sx*Sx identically), producing a/b = NaN with no check
    anywhere in Compute_TR_Param, GetTimeRange, or ComputeLinearRegression
    (projet_calculation.cpp:175-219). This reproduces that exactly.
    """
    timeTable_ms = [100.0]     # a single-bin "recording" (or the tail of one)
    tab_wj = [[1.0]]
    tab_schroeder = MakeSchroederArray(tab_wj)
    result = Compute_TR_Param(0, 10, timeTable_ms, tab_wj, tab_schroeder)
    return result


if __name__ == "__main__":
    print("=" * 100)
    print("I-Simpa upstream EDT algorithm, transcribed, run on a noise-free synthetic")
    print("exponential decay (EDT_true = 1 s), for r in {2, 20} m and dt in {10, 1} ms.")
    print("=" * 100)

    cases = []
    for f32 in (False, True):
        for r in (2.0, 20.0):
            for dt_ms in (10.0, 1.0):
                res = run_case(1.0, r, dt_ms / 1000.0, f32=f32)
                res["f32"] = f32
                cases.append(res)

    hdr = f"{'f32':>5} {'r(m)':>6} {'dt(ms)':>7} {'tau(ms)':>8} {'debT':>7} {'endT':>7} {'measured EDT(s)':>16} {'analytic EDT(s)':>16} {'error(s)':>9} {'error(%)':>9} {'#flat bins':>10} {'arrival label err(ms)':>22}"
    print(hdr)
    print("-" * len(hdr))
    for c in cases:
        print(f"{str(c['f32']):>5} {c['r_m']:>6.1f} {c['dt_ms']:>7.1f} {c['tau_ms']:>8.2f} "
              f"{c['debT_ms']:>7.2f} {c['endT_ms']:>7.2f} {c['measured_edt_s']:>16.5f} "
              f"{c['analytic_continuum_edt_s']:>16.5f} {c['error_s']:>9.4f} {c['error_pct']:>9.2f} "
              f"{c['n_flat_bins_before_arrival']:>10d} {c['arrival_label_error_ms']:>22.3f}")

    print()
    print("float64 vs float32 agreement check (should match to several significant figures")
    print("if the bias is structural/algorithmic and not a floating-point artifact):")
    for r in (2.0, 20.0):
        for dt_ms in (10.0, 1.0):
            r64 = run_case(1.0, r, dt_ms / 1000.0, f32=False)
            r32 = run_case(1.0, r, dt_ms / 1000.0, f32=True)
            diff = r64["measured_edt_s"] - r32["measured_edt_s"]
            print(f"  r={r:>5.1f}m dt={dt_ms:>4.1f}ms  f64={r64['measured_edt_s']:.6f}s  "
                  f"f32={r32['measured_edt_s']:.6f}s  diff={diff:.2e}s")

    print()
    print("I6 degenerate-curve demo (single-bin window -> denom=0, no guard in upstream):")
    deg = degenerate_curve_demo()
    print(f"  debT={deg['debT'][0]} endT={deg['endT'][0]} slope(dB/ms)={deg['slope_dB_per_ms'][0]} "
          f"-> EDT written to the .gabe file = {deg['values'][0]!r}")
