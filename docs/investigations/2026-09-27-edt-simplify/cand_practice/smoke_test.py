"""
Smoke test for cand_practice/method.py: 10 synthetic exponential decays with
a direct arrival, compared against closed-form truth. Single process, no
multiprocessing, run once.
"""
import sys
import time
import numpy as np

sys.path.insert(0, r"B:/repos/I-Simpa_Night_Mode/target/agents/edt-simplify/cand_practice")
from method import analyse

C_SOUND = 343.2  # m/s, 20 degC, matches upstream Celerite_du_son.cpp


def make_case(edt_true, r, dt, seed):
    """
    w(t) = 0 for t < tau, exp(-k(t-tau)) for t >= tau, k = 6*ln(10)/edt_true,
    so the exact Schroeder slope of this response is -60/edt_true dB/s and
    EDT == edt_true exactly for a perfect single-slope decay. Exact per-bin
    integrals (infinite-particle limit), same construction as
    upstream/READING.md section 7.1, plus small multiplicative shot noise to
    emulate a finite-particle SPPS run (this is what isolates I1/I3 there;
    here we add I5-like noise on top since that is what a real run has).
    """
    rng = np.random.default_rng(seed)
    tau = r / C_SOUND
    k = 6.0 * np.log(10.0) / edt_true
    duration = tau + edt_true * 1.3          # run long enough to clear -10dB with margin
    n = int(np.ceil(duration / dt))
    edges = np.arange(n + 1) * dt
    bins = np.zeros(n)
    for i in range(n):
        a, b = edges[i], edges[i + 1]
        if b <= tau:
            continue
        lo = max(a, tau)
        # exact integral of exp(-k(t-tau)) over [lo, b]
        bins[i] = (np.exp(-k * (lo - tau)) - np.exp(-k * (b - tau))) / k
    # multiplicative Monte-Carlo-like noise on the nonzero bins, ~ a few % relative
    noise = 1.0 + rng.normal(0.0, 0.03, size=n)
    bins = np.clip(bins * noise, 0.0, None)
    return bins, tau


def main():
    t0 = time.time()
    cases = [
        (0.5, 1, 0.001, 1), (0.5, 1, 0.010, 2),
        (1.0, 5, 0.001, 3), (1.0, 5, 0.010, 4),
        (1.5, 10, 0.001, 5), (1.5, 10, 0.010, 6),
        (2.0, 20, 0.001, 7), (2.0, 20, 0.010, 8),
        (0.8, 30, 0.001, 9), (0.8, 30, 0.010, 10),
    ]
    print(f"{'edt_true':>8} {'r(m)':>5} {'dt(ms)':>7} {'edt':>8} {'lo':>8} {'hi':>8} "
          f"{'status':>8} {'truth_in_range':>14} {'err%':>7}  reason")
    n_in = 0
    n_ok_or_wide = 0
    for edt_true, r, dt, seed in cases:
        bins, tau = make_case(edt_true, r, dt, seed)
        res = analyse(bins, dt, tau, {})
        edt, lo, hi, status, reason = (res["edt"], res["edt_lo"], res["edt_hi"],
                                        res["status"], res["reason"])
        if status == "refused":
            print(f"{edt_true:8.2f} {r:5.0f} {dt*1000:7.1f} {'--':>8} {'--':>8} {'--':>8} "
                  f"{status:>8} {'--':>14} {'--':>7}  {reason}")
            continue
        n_ok_or_wide += 1
        inside = (lo <= edt_true <= hi)
        n_in += int(inside)
        err_pct = 100.0 * (edt - edt_true) / edt_true
        print(f"{edt_true:8.2f} {r:5.0f} {dt*1000:7.1f} {edt:8.4f} {lo:8.4f} {hi:8.4f} "
              f"{status:>8} {str(inside):>14} {err_pct:7.2f}  {reason}")
    dt_wall = time.time() - t0
    print(f"\n{n_in}/{n_ok_or_wide} truth-inside-range among non-refused cases; "
          f"{len(cases)-n_ok_or_wide} refused; wall time {dt_wall:.2f}s")


if __name__ == "__main__":
    main()
