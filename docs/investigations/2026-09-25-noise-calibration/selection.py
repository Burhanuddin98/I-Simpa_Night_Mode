"""Selection by the noise refusal (review R2-M3): refusing a seed's decay time for its own noise
leaves the passing values biased when a value's own model sd grows with the value. Reads the
receipt and judges each seed with the shipped round-4 numbers (factor * sqrt(1 + kappa*n) * sd
against 2.5 %, inside the domain; energetic T20 and T30 outside uniform Lambert bands with the
roughness structure's sd), then, in every receiver-band where some seeds pass and some are
refused, compares the passing seeds' mean with all seeds' mean."""
import json, math, sys

rec = json.load(open(sys.argv[1]))
LIMIT = 0.025
# (k, kappa, min_particles, max_n, sd key) per method, quantity and walls, as shipped (round 4).
R = {"edt_s": (1.3, 2.0, 50000, 2.1641858528999998), "t20_s": (1.5, 2.75, 50000, 2.1641858528999998),
     "t30_s": (1.6, 3.0, 50000, 2.1641858528999998)}
E = {"edt_s": (0.86, 0.0, 50000, 2.12938788744)}
EO = {"t20_s": (1.2, 5.25, 15000, 1.3673826359), "t30_s": (1.3, 4.75, 150000, 1.3673826359)}
EU = {"t20_s": (0.098, 0.0, 5000, 2.12938788744), "t30_s": (0.052, 0.0, 50000, 2.12938788744)}
UMAX = 0.20000000298023224  # F6: the uniform-Lambert bound falls to 0.2 (round 4)
CONSTANT, ROUGH = "seed_model_sd", "seed_model_sd_roughness"


def entry(method, q, band):
    if method == "random":
        return R[q] + (CONSTANT,)
    if q == "edt_s":
        return E[q] + (CONSTANT,)
    if band["lambert"] and band["uniform"] and band["mean_absorption"] <= UMAX:
        return EU[q] + (CONSTANT,)
    return EO[q] + (ROUGH,)


for method in ["random", "energetic"]:
    for q in ["edt_s", "t20_s", "t30_s"]:
        xs, ys, shifts = [], [], []
        for c in rec["cells"]:
            if c["cell"]["method"] != method:
                continue
            N = c["cell"]["particles_per_source"]
            for r in c["quantities"][q]:
                band = next(b for b in c["bands"] if b["receiver"] == r["receiver"] and b["freq_hz"] == r["freq_hz"])
                k, kappa, nmin, nmax, key = entry(method, q, band)
                vals, sds = r["seed_values"], r.get(key)
                if sds is None or any(x is None for x in sds):
                    continue
                ns = [a * b for a, b in zip(band["seed_n1"], band["seed_cv2"])]
                mean = sum(vals) / len(vals)
                for v, s in zip(vals, sds):
                    xs.append(v / mean - 1)
                    ys.append(s)
                if N < nmin:
                    continue
                passing = [v for v, s, n in zip(vals, sds, ns) if n <= nmax and k * s * math.sqrt(1 + kappa * n) <= LIMIT]
                if 0 < len(passing) < len(vals):
                    shifts.append(sum(passing) / len(passing) / mean - 1)
        mx, my = sum(xs) / len(xs), sum(ys) / len(ys)
        sxy = sum((a - mx) * (b - my) for a, b in zip(xs, ys))
        sxx = sum((a - mx) ** 2 for a in xs)
        syy = sum((b - my) ** 2 for b in ys)
        corr = sxy / math.sqrt(sxx * syy)
        line = f"{method:9s} {q:6s}: corr(value, its own model sd) {corr:+.2f} over {len(xs)} seed values"
        if len(shifts) > 1:
            ms = sum(shifts) / len(shifts)
            se = math.sqrt(sum((x - ms) ** 2 for x in shifts) / (len(shifts) - 1) / len(shifts))
            line += f"; {len(shifts)} receiver-bands partly refused, passing mean {100*ms:+.2f} % +- {100*se:.2f} % against all seeds'"
        else:
            line += f"; {len(shifts)} receiver-bands partly refused"
        print(line)
