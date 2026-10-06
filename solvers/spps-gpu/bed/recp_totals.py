"""spps-gpu bed: each point receiver's echogram (.recp) of two runs, compared value by value: total per
band, and how many of the per-step floats are bit-identical.

usage: python recp_totals.py <case> <bed-log.jsonl> [...]
"""
import json, os, struct, sys


def read_gabe(path):
    b = open(path, "rb").read()
    cols, = struct.unpack_from("<i", b, 12)
    o, out = 20, []
    for _ in range(cols):
        typ, = struct.unpack_from("<H", b, o)
        rows, = struct.unpack_from("<i", b, o + 4)
        label = b[o + 24:o + 279].split(b"\0")[0].decode("latin-1")
        o += 280
        if typ == 50:
            out.append((label, list(struct.unpack_from("<%df" % rows, b, o + 4))))
            o += 4 + 4 * rows
        elif typ == 51:
            o += 1 + 4 * rows
        else:
            o += 1 + 50 * rows
    return out


if __name__ == "__main__":
    case = sys.argv[1]
    runs = {}
    for log in sys.argv[2:]:
        for line in open(log, encoding="utf-8"):
            r = json.loads(line)
            if r.get("case") == case and r.get("run"):
                runs[(r["arm"], r["seed"])] = r["run"]
    keys = sorted(runs)
    base = os.path.join(runs[keys[0]], "solve", "Punctual receivers")
    for rx in sorted(os.listdir(base)):
        cols = {k: read_gabe(os.path.join(runs[k], "solve", "Punctual receivers", rx, "Sound level.recp")) for k in keys}
        for ci, (label, _) in enumerate(cols[keys[0]]):
            vals = {k: cols[k][ci][1] for k in keys}
            a, b = vals[keys[0]], vals[keys[1]]
            same = sum(1 for x, y in zip(a, b) if struct.pack("<f", x) == struct.pack("<f", y))
            ta, tb = sum(a), sum(b)
            print(json.dumps({"receiver": rx, "band": label, keys[0][0]: ta, keys[1][0]: tb, "rel_diff": (tb - ta) / ta if ta else None,
                              "nonzero_steps": [sum(1 for x in a if x), sum(1 for x in b if x)], "bit_identical_steps": same, "steps": len(a)}))
