"""F6 (round 4): the uniform-Lambert entries' bound fell from 0.4 to 0.2, so every uniform Lambert
band above 0.2 is judged by the roughness entries. The V4 cells check that in the suite; this also
reads the cells of rounds 1 to 3 that have such bands (in-sample for nothing: the roughness was fitted
on bands not uniform Lambert). Per cell and quantity, over the rows inside the domain: the pooled
ratio of the seeds' spread to k * sqrt(1 + kappa * n) * sd (roughness), with R3-3's effective degrees
of freedom and exact chi-square quantiles, and the one-sided 95 % lower bound."""
import json, math, sys
from scipy.stats import chi2

rec = json.load(open(sys.argv[1]))
# (quantity, k, kappa, fewest particles, largest n): T20 at its fitted 1.2 and at the 1.4 it ships.
ROUGH = [("t20_s", 1.2, 5.25, 15000, 1.3673826359), ("t20_s", 1.4, 5.25, 15000, 1.3673826359),
         ("t30_s", 1.3, 4.75, 150000, 1.3673826359)]


def band_of(c, r):
    return next(b for b in c["bands"] if b["receiver"] == r["receiver"] and b["freq_hz"] == r["freq_hz"])


def corr(a, b):
    n = len(a)
    ma, mb = sum(a) / n, sum(b) / n
    sab = sum((x - ma) * (y - mb) for x, y in zip(a, b))
    saa = sum((x - ma) ** 2 for x in a)
    sbb = sum((y - mb) ** 2 for y in b)
    return sab / math.sqrt(saa * sbb) if saa > 0 and sbb > 0 else 0.0


for c in rec["cells"]:
    if c["cell"]["method"] != "energetic":
        continue
    for q, k, kappa, nmin, nmax in ROUGH:
        if c["cell"]["particles_per_source"] < nmin:
            continue
        rows = []
        for r in c["quantities"][q]:
            b = band_of(c, r)
            if not (b["lambert"] and b["uniform"] and b["mean_absorption"] > 0.2000001):
                continue
            sd = r.get("seed_model_sd_roughness")
            if sd is None or any(x is None for x in sd):
                continue
            ns = [a * g for a, g in zip(b["seed_n1"], b["seed_cv2"])]
            if max(ns) > nmax:
                continue
            pv = sum(s * s * (1 + kappa * n) for s, n in zip(sd, ns)) / len(sd)
            rows.append((r["observed_sd"], pv, r["freq_hz"], r["seed_values"]))
        if not rows:
            continue
        seeds = len(rows[0][3])
        nu = seeds - 1
        x = [o * o / p for o, p, _, _ in rows]
        m = len(rows)
        mean = sum(x) / m
        var = sum((v - mean) ** 2 for v in x) / (m - 1) if m > 1 else 0.0
        phi = max(var / (2.0 / nu * mean * mean), 1.0) if m > 1 else 1.0
        bands = sorted(set(f for _, _, f, _ in rows))
        s2, pairs = 0.0, 0
        for f in bands:
            g = [v for _, _, ff, v in rows if ff == f]
            for i in range(len(g)):
                for j in range(i + 1, len(g)):
                    s2 += corr(g[i], g[j]) ** 2
                    pairs += 1
        deff = 1.0
        if pairs:
            bias = 1.0 / nu
            rho2 = max((s2 / pairs - bias) / (1 - bias), 0.0)
            deff = 1 + (m / len(bands) - 1) * rho2
        dof = m * nu / (phi * deff)
        ratio = math.sqrt(sum(o * o for o, _, _, _ in rows) / sum(k * k * p for _, p, _, _ in rows))
        lower = ratio * math.sqrt(dof / chi2.ppf(0.95, dof))
        print(f"{c['cell']['id']:7s} {c['cell']['role']:12s} {q:6s} k {k}: {m:2d} rows, {ratio:.3f} "
              f"[lower {lower:.3f}] dof {dof:.1f} {'ok' if lower <= 1 else 'FAIL'}")
