"""spps-gpu bed: particle fates over many seeds, SPPS against spps-gpu, on one staged case.

Rare fates (lost, loop, alive at the end) come a few per hundred thousand particles; this runs each
arm over several seeds and sums the statistics file's rows, so the two rates can be compared.

usage: python fates.py <root> <case> <src-solve-dir> <arm: spps|gpu> <seeds a,b,..> [attr=value ...]
"""
import json, os, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import stage  # noqa: E402

EXE = {"spps": r"C:\tmp\nm-solvers-timebin\bin\spps.exe", "gpu": r"C:\tmp\nm-spps-gpu\bin\spps-gpu.exe"}
ROWS = ["atmosphere", "materials", "fittings", "loops", "meshing", "remaining", "total"]


def read_stats(path):
    b = open(path, "rb").read()
    cols, = struct.unpack_from("<i", b, 12)
    o, out = 20, []
    for c in range(cols):
        typ, = struct.unpack_from("<H", b, o)
        rows, = struct.unpack_from("<i", b, o + 4)
        label = b[o + 24:o + 279].split(b"\0")[0].decode("latin-1")
        o += 280
        if typ == 51:
            out.append((label, list(struct.unpack_from("<%di" % rows, b, o + 1))))
            o += 1 + 4 * rows
        elif typ == 52:
            o += 1 + 50 * rows
        else:
            o += 4 + 4 * rows
    return out


if __name__ == "__main__":
    if len(sys.argv) < 6:
        sys.exit(__doc__)
    root, case, src, arm, seeds = sys.argv[1:6]
    attrs = dict(x.split("=", 1) for x in sys.argv[6:])
    total = {}
    for seed in seeds.split(","):
        a = dict(attrs)
        a["random_seed"] = seed
        d = os.path.join(root, f"{case}-{arm}-s{seed}")
        stage.stage(src, d, a, template=False)
        rc = stage.run(EXE[arm], d, [])
        for label, vals in read_stats(os.path.join(d, "SPPS particle statistics.gabe")):
            t = total.setdefault(label, [0] * 7)
            for i, v in enumerate(vals):
                t[i] += v
            print(json.dumps({"seed": seed, "band": label, **dict(zip(ROWS, vals))}))
    for label, t in total.items():
        print(json.dumps({"arm": arm, "case": case, "seeds": seeds, "band": label, **dict(zip(ROWS, t))}))
