"""Adversarial check 1 of the M8b tamper fix: the M8a bed's fingerprint, and every sealed file
re-hashed here, independently of the Rust that checks the seal and of tools/bed/seal_bed.py.
Reads only. Usage: py -3 bed_fingerprint.py <bed> <seal.json> [--hash]"""
import hashlib, json, os, sys, time

bed, seal_path = sys.argv[1], sys.argv[2]
do_hash = "--hash" in sys.argv
t0 = time.time()
files = size = 0
newest = (0, "")
for root, dirs, names in os.walk(bed):
    for n in names:
        p = os.path.join(root, n)
        st = os.stat(p)
        files += 1
        size += st.st_size
        if st.st_mtime_ns > newest[0]:
            newest = (st.st_mtime_ns, os.path.relpath(p, bed))
def sha(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()
print(f"bed {bed}: {files} files, {size} bytes, newest mtime {time.strftime('%Y-%m-%dT%H:%M:%S', time.gmtime(newest[0] / 1e9))}.{newest[0] % 10**9 // 10**6:03d}Z ({newest[1]})")
print(f"report.json {sha(os.path.join(bed, 'report.json'))}, summary.json {sha(os.path.join(bed, 'summary.json'))}")
raw = open(seal_path, "rb").read()
seal = json.loads(raw)
print(f"seal {seal_path}: {len(raw)} bytes, sha256 (LF) {hashlib.sha256(raw.replace(b'\r\n', b'\n')).hexdigest()}, {len(seal['runs'])} runs, {seal['files']} files, {seal['bytes']} bytes; report {seal['report_json_sha256'][:12]}, summary {seal['summary_json_sha256'][:12]}")
if do_hash:
    bad = []
    n = nb = 0
    for key, run in seal["runs"].items():
        d = os.path.join(bed, *key.split("/"))
        on_disk = {}
        for root, dirs, names in os.walk(d):
            for nm in names:
                p = os.path.join(root, nm)
                on_disk[os.path.relpath(p, d).replace(os.sep, "/")] = p
        want = {f[0]: (f[1], f[2]) for f in run["files"]}
        if set(on_disk) != set(want):
            bad.append(f"{key}: files differ: only on disk {sorted(set(on_disk) - set(want))[:3]}, only in seal {sorted(set(want) - set(on_disk))[:3]}")
        for rel, p in on_disk.items():
            if rel in want:
                n += 1
                s = os.path.getsize(p)
                nb += s
                if (s, sha(p)) != want[rel]:
                    bad.append(f"{key}/{rel}: not the sealed bytes")
    under_runs = sum(len(ns) for _, _, ns in os.walk(os.path.join(bed, "runs")))
    print(f"re-hashed {n} sealed files, {nb} bytes, in {len(seal['runs'])} runs; files under runs/: {under_runs}; not as sealed: {len(bad)}")
    for b in bad[:20]:
        print("  " + b)
print(f"done in {time.time() - t0:.1f} s")
