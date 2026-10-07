"""spps-gpu bed: the surface-receiver and cutting-plane maps of the bed runs, summed, SPPS against
spps-gpu (records written, total of every record, early tenth and late half of the bins).

usage: python maps.py <bed-log.jsonl> [...]
"""
import json, os, struct, sys

FILES = {
    "outputs": [("Global", "Sound level.csbin"), ("Global", "rs_cut.csbin"), ("1000 Hz", "rs_cut.csbin"), ("500 Hz", "Sound level.csbin")],
    "seats": [("Global", "Sound level.csbin")],
    "energetic": [("Global", "Sound level.csbin")],
    "cr4-1k": [("Global", "rs_cut.csbin")],
}


def summary(path):
    b = open(path, "rb").read()
    nodes, nrs, nsteps = struct.unpack_from("<3i", b, 24)
    o = 44 + 12 * nodes
    tot, recs, per_t = 0.0, 0, [0.0] * nsteps
    for _ in range(nrs):
        nf, = struct.unpack_from("<i", b, o + 4)
        o += 264
        for _ in range(nf):
            nr, = struct.unpack_from("<i", b, o + 12)
            o += 16
            for i in range(nr):
                t, = struct.unpack_from("<H", b, o)
                e, = struct.unpack_from("<f", b, o + 4)
                o += 8
                tot += e
                per_t[t] += e
            recs += nr
    return {"records": recs, "sum": tot, "early": sum(per_t[: max(1, nsteps // 10)]), "late": sum(per_t[nsteps // 2:])}


if __name__ == "__main__":
    runs = {}
    for log in sys.argv[1:]:
        for line in open(log, encoding="utf-8"):
            r = json.loads(line)
            if r.get("run"):
                runs[(r["case"], r["arm"], r["seed"])] = r["run"]
    for case, files in FILES.items():
        for folder, name in files:
            for arm in ("spps", "gpu"):
                for seed in (101, 202):
                    k = (case, arm, seed)
                    if k not in runs:
                        continue
                    p = os.path.join(runs[k], "solve", "Surface receiver", folder, name)
                    s = summary(p)
                    print(json.dumps({"case": case, "file": folder + "/" + name, "arm": arm, "seed": seed, **s}))
