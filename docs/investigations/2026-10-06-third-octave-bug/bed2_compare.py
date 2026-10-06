"""Bed for patch 0002 (sparse surface-receiver series), 2026-10-06.

sparse-step  (patched 0001+0002, bin unset)  must equal A-old in every file, decoded.
sparse-10ms  (patched 0001+0002, bin 10 ms)  must equal C-new-10ms (0001 only, dense) in every file, decoded.
sparse-plane01: the production run with the Audience plane at 0.1 m (seed 0, 18 bands, 300k particles):
             reports its maps' size and record counts and the sampled memory (memory.txt).
"""
import hashlib
import os
import subprocess
import sys

BED = r"C:\tmp\nm-timebin-bed"
SIMPA = r"C:\tmp\nm-target\release\simpa.exe"
SKIP = ("config.xml", "mesh.cbin", "tetramesh.mbin", "stdout.txt", "stderr.txt", "memory.txt")


def files(arm):
    out = {}
    root = os.path.join(BED, arm)
    for d, _, fs in os.walk(root):
        for f in fs:
            p = os.path.join(d, f)
            rel = os.path.relpath(p, root)
            if rel in SKIP:
                continue
            out[rel] = p
    return out


def sha(p):
    return hashlib.sha256(open(p, "rb").read()).hexdigest()


def dump(p):
    r = subprocess.run([SIMPA, "dump", "csbin", p], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f"simpa dump failed on {p}: {r.stderr}")
    return r.stdout


def same_files(x, y):
    fx, fy = files(x), files(y)
    bad = 0
    if set(fx) != set(fy):
        print(f"  {x}/{y} file sets differ: {sorted(set(fx) ^ set(fy))[:5]}")
        bad += 1
    for rel in sorted(set(fx) & set(fy)):
        same = dump(fx[rel]) == dump(fy[rel]) if rel.endswith(".csbin") else sha(fx[rel]) == sha(fy[rel])
        if not same:
            print(f"  DIFFER: {rel}")
            bad += 1
    print(f"{x} vs {y}: {len(fx)} files, {bad} differ -> {'IDENTICAL' if bad == 0 else 'FAIL'}")
    return bad


def plane_report(arm):
    root = os.path.join(BED, arm)
    mem = os.path.join(root, "memory.txt")
    print(f"--- {arm}")
    if os.path.isfile(mem):
        print("  " + open(mem, encoding="utf-8-sig").read().strip())
    err = open(os.path.join(root, "stderr.txt"), encoding="utf-8", errors="replace").read().strip()
    print(f"  stderr: {len(err)} chars" + (f": {err[:200]!r}" if err else ""))
    total_recs = 0
    total_bytes = 0
    n = 0
    for rel, p in sorted(files(arm).items()):
        if not rel.endswith("rs_cut.csbin"):
            continue
        text = dump(p)
        steps = faces = recs = 0
        for line in text.splitlines():
            if line.startswith("timesteps "):
                steps = int(line.split()[1])
            elif line.startswith("face "):
                faces += 1
            elif line.startswith("records "):
                recs += int(line.split()[1])
        n += 1
        total_recs += recs
        total_bytes += os.path.getsize(p)
        if "Global" in rel or n <= 2:
            print(f"  {rel}: cells {faces//2:,} steps {steps:,} records {recs:,} file {os.path.getsize(p)/2**20:,.0f} MB "
                  f"(dense would be {faces//2*steps*4/2**20:,.0f} MB in RAM per band)")
    print(f"  {n} map files, {total_recs:,} records, {total_bytes/2**20:,.0f} MB on disk")


def main():
    # B-new-step was shown identical to A-old at 15:00 (BED-0001); A-old was re-run at 15:51 for its memory
    # number and SPPS then wrote duplicate receiver folders (MP10, MP20) beside the old ones, so B is the reference.
    arms = sys.argv[1:] or ["sparse2-step", "sparse2-10ms", "sparse2-plane01"]
    bad = same_files("B-new-step", arms[0])
    bad += same_files("C-new-10ms", arms[1])
    for arm in ("A-old",) + tuple(arms[:2]):
        mem = os.path.join(BED, arm, "memory.txt")
        if os.path.isfile(mem):
            print("  " + open(mem, encoding="utf-8-sig").read().strip())
    if len(arms) > 2 and os.path.isdir(os.path.join(BED, arms[2], "Surface receiver")):
        plane_report(arms[2])
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
