"""spps-gpu bed (a): the CPU build against the GPU build of the same walk, same config, same seed.

Reads the two runs' --dump-walk files (every primary particle: steps traced, fate, final energy,
children run and their steps) and --dump-sums files (each band's double accumulators and float
cells), and prints one JSON line of verdicts.

usage: python compare_builds.py <cpu.walk> <gpu.walk> <cpu.sums> <gpu.sums>
"""
import json, struct, sys
import numpy as np

WREC = np.dtype([("steps", "<u4"), ("state", "<i4"), ("E", "<f8"), ("children", "<u4"), ("pad", "<u4"), ("childSteps", "<u8")])


def read_walk(path):
    out = {}
    b = open(path, "rb").read()
    o = 0
    while o < len(b):
        band, n = struct.unpack_from("<ii", b, o)
        o += 8
        out[band] = np.frombuffer(b, WREC, n, o)
        o += n * WREC.itemsize
    return out


def read_sums(path):
    out = {}
    b = open(path, "rb").read()
    o = 0
    while o < len(b):
        band, S, R, srcCols, surfN, cutN, _, _ = struct.unpack_from("<8i", b, o)
        o += 32
        d = {}
        for name, n, t in (("total", S, "<f8"), ("rpE", R * S, "<f8"), ("rpLf", R * S, "<f8"), ("rpLfc", R * S, "<f8"),
                           ("rpI", 3 * R * S, "<f8"), ("rpSrc", R * srcCols, "<f8"), ("surf", surfN, "<f4"), ("cut", cutN, "<f4"),
                           ("states", 8, "<u8")):
            a = np.frombuffer(b, t, n, o)
            o += a.nbytes
            d[name] = a
        out[band] = d
    return out


def rel(a, b):
    a = np.asarray(a, np.float64)
    b = np.asarray(b, np.float64)
    if a.size == 0:
        return 0.0, 0.0
    scale = max(np.abs(a).max(), np.abs(b).max(), 1e-300)
    d = np.abs(a - b)
    per = d / np.maximum(np.abs(a), 1e-300)
    per[(a == 0) & (b == 0)] = 0
    return float(d.max() / scale), float(per[np.abs(a) > 1e-6 * scale].max() if (np.abs(a) > 1e-6 * scale).any() else 0)


def main(cw, gw, cs, gs):
    wc, wg, sc, sg = read_walk(cw), read_walk(gw), read_sums(cs), read_sums(gs)
    res = {"bands": []}
    for band in sorted(wc):
        a, b = wc[band], wg[band]
        same = {k: int((a[k] != b[k]).sum()) for k in ("steps", "state", "children", "childSteps")}
        same["E_bits"] = int((a["E"].view("<u8") != b["E"].view("<u8")).sum())
        r = {"band": band, "particles": int(a.size), "walk_mismatches": same,
             "steps_traced": int(a["steps"].sum() + a["childSteps"].sum()),
             "fates": {int(s): int((a["state"] == s).sum()) for s in np.unique(a["state"])}}
        x, y = sc[band], sg[band]
        r["states_equal"] = bool((x["states"] == y["states"]).all())
        worst = 0.0
        for k in ("total", "rpE", "rpLf", "rpLfc", "rpI", "rpSrc"):
            of_peak, per_cell = rel(x[k], y[k])
            r[k] = {"max_of_peak": of_peak, "max_rel_cell_above_1e-6_peak": per_cell}
            worst = max(worst, of_peak)
        r["double_sums_max_rel_of_peak"] = worst
        for k in ("surf", "cut"):
            of_peak, per_cell = rel(x[k], y[k])
            r[k] = {"max_of_peak": of_peak, "max_rel_cell_above_1e-6_peak": per_cell,
                    "nonzero_cpu": int((x[k] != 0).sum()), "nonzero_gpu": int((y[k] != 0).sum())}
        res["bands"].append(r)
    res["walk_identical"] = all(sum(b["walk_mismatches"].values()) == 0 for b in res["bands"])
    res["double_sums_within_1e-12"] = all(b["double_sums_max_rel_of_peak"] <= 1e-12 for b in res["bands"])
    print(json.dumps(res, indent=1))


if __name__ == "__main__":
    if len(sys.argv) != 5:
        sys.exit(__doc__)
    main(*sys.argv[1:])
