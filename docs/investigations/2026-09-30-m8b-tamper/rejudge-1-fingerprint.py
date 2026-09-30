"""Fingerprint a bed folder, reading only: file count, bytes, newest mtime, report/summary sha256,
and the sha256 of every file (written as one manifest file OUTSIDE the bed), plus a digest of that
manifest so a before/after pair compares in one line.

usage: py -3 r1_fp_bed.py <bed> <manifest-out>
"""
import sys as _s; _s.stdout.reconfigure(encoding="utf-8")
import hashlib
import os
import sys
from datetime import datetime, timezone

bed, out = sys.argv[1], sys.argv[2]
assert not os.path.abspath(out).lower().startswith(os.path.abspath(bed).lower()), "manifest must be outside the bed"

rows = []
newest = (0, "")
total = 0
for root, dirs, files in os.walk(bed):
    dirs.sort()
    for f in sorted(files):
        p = os.path.join(root, f)
        st = os.stat(p)
        h = hashlib.sha256()
        with open(p, "rb") as fh:
            for chunk in iter(lambda: fh.read(1 << 20), b""):
                h.update(chunk)
        rel = os.path.relpath(p, bed).replace("\\", "/")
        rows.append(f"{rel}\t{st.st_size}\t{st.st_mtime_ns}\t{h.hexdigest()}")
        total += st.st_size
        if st.st_mtime_ns > newest[0]:
            newest = (st.st_mtime_ns, rel)

rows.sort()
text = "\n".join(rows) + "\n"
with open(out, "w", encoding="utf-8", newline="\n") as fh:
    fh.write(text)


def sha(path):
    with open(os.path.join(bed, path), "rb") as fh:
        return hashlib.sha256(fh.read()).hexdigest()


ts = datetime.fromtimestamp(newest[0] / 1e9, tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3] + "Z"
print(f"bed: {bed}")
print(f"at: {datetime.now().astimezone().isoformat(timespec='seconds')}")
print(f"files: {len(rows)}")
print(f"bytes: {total}")
print(f"newest mtime: {ts} ({newest[1]})")
print(f"report.json sha256: {sha('report.json')}")
print(f"summary.json sha256: {sha('summary.json')}")
print(f"manifest (path, size, mtime_ns, sha256 per file) sha256: {hashlib.sha256(text.encode()).hexdigest()}")
print(f"manifest: {out}")
