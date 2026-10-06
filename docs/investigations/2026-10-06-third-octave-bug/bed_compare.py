"""Three-arm bed for patch 0001 (surface-receiver time bin), 2026-10-06.

A-old: upstream 929a5c8 as shipped (C:/tmp/nm-m8a-solvers/spps.exe).
B-new-step: patched build, no recepteurs_surfaciques_pas_temps (must equal A in every file, decoded).
C-new-10ms: patched build, bin 10 ms (maps on 1,000 bins; per-face sums equal B's within float order;
            every non-map file identical to B).
"""
import hashlib
import os
import struct
import subprocess
import sys

BED = r"C:\tmp\nm-timebin-bed"
SIMPA = r"C:\tmp\nm-target\release\simpa.exe"
ARMS = ["A-old", "B-new-step", "C-new-10ms"]


def files(arm):
    out = {}
    root = os.path.join(BED, arm)
    for d, _, fs in os.walk(root):
        for f in fs:
            p = os.path.join(d, f)
            rel = os.path.relpath(p, root)
            if rel in ("config.xml", "mesh.cbin", "tetramesh.mbin", "stdout.txt", "stderr.txt"):
                continue
            out[rel] = p
    return out


def sha(p):
    return hashlib.sha256(open(p, "rb").read()).hexdigest()


def hexf(tok):
    tok = tok.lower().removeprefix("0x")
    return struct.unpack("<f", struct.pack("<I", int(tok, 16)))[0]


def dump(p):
    r = subprocess.run([SIMPA, "dump", "csbin", p], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f"simpa dump failed on {p}: {r.stderr}")
    return r.stdout


def parse(text):
    """Header fields and per-face sums (float32 accumulation in step order, as upstream sums)."""
    steps = None
    dt = None
    faces = []  # list of (sum, nrecords, maxstep)
    cur = None
    for line in text.splitlines():
        t = line.split()
        if not t:
            continue
        if t[0] == "timesteps":
            steps, dt = int(t[1]), hexf(t[2])
        elif t[0] == "face":
            cur = [0.0, 0, -1]
            faces.append(cur)
        elif t[0] == "record":
            s, e = int(t[1]), hexf(t[2])
            cur[0] = struct.unpack("<f", struct.pack("<f", cur[0] + e))[0]
            cur[1] += 1
            cur[2] = max(cur[2], s)
    return steps, dt, faces


def main():
    fa, fb, fc = (files(a) for a in ARMS)
    print(f"files: A {len(fa)}  B {len(fb)}  C {len(fc)}")
    bad = 0
    # A vs B: same file set, every non-csbin byte-identical, every csbin identical decoded.
    if set(fa) != set(fb):
        print("A/B file sets differ:", set(fa) ^ set(fb)); bad += 1
    for rel in sorted(fa):
        if rel not in fb:
            continue
        if rel.endswith(".csbin"):
            same = dump(fa[rel]) == dump(fb[rel])
        else:
            same = sha(fa[rel]) == sha(fb[rel])
        if not same:
            print(f"A/B DIFFER: {rel}"); bad += 1
    print(f"A vs B: {len(fa)} files compared, {bad} differ  -> {'IDENTICAL' if bad == 0 else 'FAIL'}")
    # B vs C: non-map files identical; maps on 1,000 bins with equal per-face sums.
    bad2 = 0
    worst = 0.0
    for rel in sorted(fb):
        if rel not in fc:
            print(f"C lacks {rel}"); bad2 += 1; continue
        if not rel.endswith(".csbin"):
            if sha(fb[rel]) != sha(fc[rel]):
                print(f"B/C DIFFER (non-map): {rel}"); bad2 += 1
            continue
        sb, dtb, Fb = parse(dump(fb[rel]))
        sc, dtc, Fc = parse(dump(fc[rel]))
        size_b, size_c = os.path.getsize(fb[rel]), os.path.getsize(fc[rel])
        ok = sb == 10000 and sc == 1000 and abs(dtb - 0.001) < 1e-9 and abs(dtc - 0.01) < 1e-8 and len(Fb) == len(Fc)
        maxstep_c = max((f[2] for f in Fc), default=-1)
        if maxstep_c >= 1000:
            ok = False
        tot_b = sum(f[0] for f in Fb); tot_c = sum(f[0] for f in Fc)
        rel_err = 0.0
        for (eb, _, _), (ec, _, _) in zip(Fb, Fc):
            if eb == 0.0 and ec == 0.0:
                continue
            d = abs(eb - ec) / max(abs(eb), abs(ec))
            rel_err = max(rel_err, d)
        worst = max(worst, rel_err)
        if rel_err > 1e-5:
            ok = False
        print(f"  {rel}: B {sb} steps dt {dtb:.4g} {size_b/1e6:.2f} MB | C {sc} bins dt {dtc:.4g} {size_c/1e6:.2f} MB "
              f"| faces {len(Fb)} | records B {sum(f[1] for f in Fb)} C {sum(f[1] for f in Fc)} | max step C {maxstep_c} "
              f"| total B {tot_b:.6e} C {tot_c:.6e} | worst per-face rel diff {rel_err:.2e} -> {'ok' if ok else 'FAIL'}")
        if not ok:
            bad2 += 1
    print(f"B vs C: {bad2} problems, worst per-face relative difference {worst:.2e}  -> {'PASS' if bad2 == 0 else 'FAIL'}")
    for a in ARMS:
        e = open(os.path.join(BED, a, "stderr.txt"), encoding="utf-8", errors="replace").read().strip()
        print(f"{a} stderr: {len(e)} chars" + (f": {e[:200]!r}" if e else ""))
    sys.exit(1 if bad or bad2 else 0)


if __name__ == "__main__":
    main()
