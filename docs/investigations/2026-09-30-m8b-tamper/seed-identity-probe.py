# Read-only probe of the M8a bed: within each cell (and the atmospheric seeds), is any solver
# output file byte-identical between two seeds? Inputs (config.xml, mesh.cbin, tetramesh.mbin)
# are excluded. Writes nothing.
import hashlib, os, sys, collections
ROOT = r"C:\tmp\nm-m8a-bed\20260929T093134Z\runs"
INPUTS = {"config.xml", "mesh.cbin", "tetramesh.mbin"}
def sha(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()
dup_paths = collections.Counter(); groups = 0; files = 0; whole_equal = 0
for cell in sorted(os.listdir(ROOT)):
    seeds = sorted(os.listdir(os.path.join(ROOT, cell)))
    if len(seeds) < 2:
        continue
    groups += 1
    per_seed = {}
    for s in seeds:
        d = os.path.join(ROOT, cell, s)
        run = [x for x in os.listdir(d) if os.path.isdir(os.path.join(d, x))]
        assert len(run) == 1, (cell, s, run)
        solve = os.path.join(d, run[0], "solve")
        m = {}
        for dp, dn, fn in os.walk(solve):
            for f in fn:
                rel = os.path.relpath(os.path.join(dp, f), solve).replace("\\", "/")
                if rel in INPUTS:
                    continue
                m[rel] = sha(os.path.join(dp, f)); files += 1
        per_seed[s] = m
    paths = set().union(*[set(m) for m in per_seed.values()])
    for p in sorted(paths):
        hs = [m.get(p) for m in per_seed.values() if p in m]
        if len(hs) != len(set(hs)):
            dup_paths[(cell, p)] += 1
    sets = [tuple(sorted(m.items())) for m in per_seed.values()]
    if len(sets) != len(set(sets)):
        whole_equal += 1
print(f"groups {groups}, output files hashed {files}, cells with two equal whole sets {whole_equal}")
print(f"(cell, path) pairs with a byte-identical file in two seeds: {len(dup_paths)}")
kinds = collections.Counter(p for (_, p) in dup_paths)
for p, n in kinds.most_common(20):
    print(f"  {n:3d} cells  {p}")
