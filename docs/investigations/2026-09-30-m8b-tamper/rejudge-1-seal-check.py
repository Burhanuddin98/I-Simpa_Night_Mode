"""Hold a bed manifest (r1_fp_bed.py's output) to the committed seal, independently of the Rust.

usage: py -3 r1_seal_vs_manifest.py <seal.json> <manifest.tsv>
"""
import sys as _s; _s.stdout.reconfigure(encoding="utf-8")
import json
import sys

seal = json.load(open(sys.argv[1], encoding="utf-8"))
disk = {}
for line in open(sys.argv[2], encoding="utf-8"):
    rel, size, _mtime, sha = line.rstrip("\n").split("\t")
    disk[rel] = (int(size), sha)

want = {}
for folder, entry in seal["runs"].items():
    for path, size, sha in entry["files"]:
        want[f"{folder}/{path}"] = (size, sha)

under_runs = {k: v for k, v in disk.items() if k.startswith("runs/")}
missing = sorted(set(want) - set(under_runs))
extra = sorted(set(under_runs) - set(want))
changed = sorted(k for k in set(want) & set(under_runs) if want[k] != under_runs[k])
print(f"seal: {len(seal['runs'])} runs, {len(want)} files, {sum(s for s, _ in want.values())} bytes "
      f"(header: {seal['files']} files, {seal['bytes']} bytes)")
print(f"disk under runs/: {len(under_runs)} files, {sum(s for s, _ in under_runs.values())} bytes")
print(f"in the seal, not on disk: {len(missing)}; on disk under runs/, not in the seal: {len(extra)}; "
      f"size or sha256 differ: {len(changed)}")
for k in (missing + extra + changed)[:20]:
    print("  ", k)
print(f"report.json: seal {seal['report_json_sha256']} disk {disk['report.json'][1]} "
      f"{'equal' if seal['report_json_sha256'] == disk['report.json'][1] else 'DIFFER'}")
print(f"summary.json: seal {seal['summary_json_sha256']} disk {disk['summary.json'][1]} "
      f"{'equal' if seal['summary_json_sha256'] == disk['summary.json'][1] else 'DIFFER'}")
