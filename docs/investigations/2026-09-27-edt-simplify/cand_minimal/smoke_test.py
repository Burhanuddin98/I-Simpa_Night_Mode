"""Smoke test for cand_minimal/method.py: single python process, no
multiprocessing, no --workers. 10 synthetic exponential decays with a direct
arrival, dt in {1,10} ms, compared against the closed-form truth
(EDT_true fixes the true Schroeder slope exactly, per
target/agents/edt-simplify/upstream/READING.md Sec.7.1:
k = 6*ln(10)/EDT_true)."""
import os
import time
import numpy as np
import method

C = 343.2

cases = [
    (0.5, 2, 0.010), (0.5, 2, 0.001),
    (1.0, 10, 0.010), (1.0, 10, 0.001),
    (1.5, 20, 0.010), (1.5, 20, 0.001),
    (2.0, 30, 0.010), (2.0, 30, 0.001),
    (0.8, 1, 0.010), (0.8, 1, 0.001),
]


def make_bins(edt_true, r, dt):
    tau = r / C
    k = 6.0 * np.log(10.0) / edt_true
    duration = tau + edt_true * 1.3  # comfortable margin past -10 dB and the CATT check
    n = int(np.ceil(duration / dt)) + 5
    bins = np.zeros(n, dtype=np.float64)
    for i in range(n):
        a, b = i * dt, (i + 1) * dt
        lo, hi = max(a, tau), max(b, tau)
        if hi > lo:
            bins[i] = (np.exp(-k * (lo - tau)) - np.exp(-k * (hi - tau))) / k
    return bins, tau


def main():
    t0 = time.time()
    rows = []
    for edt_true, r, dt in cases:
        bins, tau = make_bins(edt_true, r, dt)
        res = method.analyse(bins, dt, tau, {})
        edt = res['edt']
        err_pct = 100.0 * (edt - edt_true) / edt_true if edt is not None else None
        inside = (res['edt_lo'] <= edt_true <= res['edt_hi']) if edt is not None else None
        rows.append((edt_true, r, dt, tau, edt, res['edt_lo'], res['edt_hi'],
                     res['status'], res['reason'], err_pct, inside))
    elapsed = time.time() - t0

    def f(x):
        return f"{x:9.5f}" if x is not None else "     None"

    print(f"{'EDT_true':>8} {'r':>5} {'dt(ms)':>7} {'tau(ms)':>8} {'EDT_meas':>9} "
          f"{'lo':>9} {'hi':>9} {'status':>7} {'reason':>15} {'err%':>7} {'in_range':>8}")
    for edt_true, r, dt, tau, edt, lo, hi, status, reason, err, inside in rows:
        print(f"{edt_true:8.2f} {r:5.1f} {dt*1000:7.1f} {tau*1000:8.3f} {f(edt)} "
              f"{f(lo)} {f(hi)} {status:>7} {reason:>15} "
              f"{('%7.3f' % err) if err is not None else '   None'} {str(inside):>8}")

    n_accepted = sum(1 for row in rows if row[7] in ('ok', 'wide'))
    n_in = sum(1 for row in rows if row[10])
    errs = [abs(row[9]) for row in rows if row[9] is not None]
    print(f"\naccepted: {n_accepted}/{len(rows)}  truth-in-range: {n_in}/{n_accepted}  "
          f"max abs err%: {(max(errs) if errs else float('nan')):.4f}  "
          f"elapsed: {elapsed:.3f}s  pid: {os.getpid()}")


if __name__ == "__main__":
    main()
