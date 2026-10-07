"""spps-gpu bed, B3: the live stream's cost and its bit-identity.

For one case, stages a fresh run folder per run (stage.py: the ROOT workingdirectory rewritten to the
copy, refused otherwise) and runs spps-gpu with the stream off and on, alternately, `n` times each,
then once with the stream kept. Per run it records the wall time measured here and spps-gpu.json's
own seconds. Every output file of every run (all but spps-gpu.json and the bed's own logs) is hashed
and compared with the first stream-off run: the walk and its outputs must be bit-identical with the
stream on. The kept stream is decoded and checked frame by frame against the .pbin files.

usage: python stream_cost.py <exe> <src-solve-dir> <out-dir> <n> [attr=value ...]
  SPPS_GPU_BED_ROOTS must include <out-dir>'s parent tree (stage.py refuses anything outside it).
  --prune: after hashing (and the stream check), delete each run's large outputs; hashes.json stays.
Writes <out-dir>/cost.jsonl (one line per run) and <out-dir>/summary.json.
"""
import hashlib, json, os, shutil, struct, subprocess, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import stage  # noqa: E402

SKIP = {"spps-gpu.json", "solver.stdout.txt", "solver.stderr.txt", "wall.txt", "config.xml", "spps-gpu.pstream"}


def hashes(d):
    out = {}
    for root, _, files in os.walk(d):
        for f in files:
            rel = os.path.relpath(os.path.join(root, f), d)
            if rel in SKIP:
                continue
            h = hashlib.sha256()
            with open(os.path.join(root, f), "rb") as fh:
                for chunk in iter(lambda: fh.read(1 << 22), b""):
                    h.update(chunk)
            out[rel.replace("\\", "/")] = h.hexdigest()
    return out


def read_pbin(path):
    with open(path, "rb") as fh:
        b = fh.read()
    n, _, _, _, _, steps, dt = struct.unpack_from("<6If", b, 0)
    at, parts = 28, []
    for _ in range(n):
        cnt, first = struct.unpack_from("<IH", b, at)
        at += 8
        parts.append((first, b[at:at + 16 * cnt]))
        at += 16 * cnt
    return steps, dt, parts


def check_stream(d):
    """The kept stream against the .pbin files: same particles, same records, bit for bit."""
    with open(os.path.join(d, "spps-gpu.pstream"), "rb") as fh:
        b = fh.read()
    magic, ver, dt, steps, per_band, nb = struct.unpack_from("<IIfIII", b, 0)
    assert magic == 0x4D545350 and ver == 1, (hex(magic), ver)
    bands = list(struct.unpack_from(f"<{nb}i", b, 24))
    at = 24 + 4 * nb
    frames = {}
    order = []
    while at + 4 <= len(b):
        (ln,) = struct.unpack_from("<I", b, at)
        assert at + 4 + ln <= len(b), "torn frame in a closed stream"
        hz, idx, first, n = struct.unpack_from("<iIII", b, at + 4)
        assert ln == 16 + 16 * n
        pos = b[at + 20:at + 20 + 12 * n]
        en = b[at + 20 + 12 * n:at + 20 + 16 * n]
        frames[(hz, idx)] = (first, pos, en)
        order.append((hz, idx))
        at += 4 + ln
    assert at == len(b)
    checked = records = 0
    for hz in bands:
        p = os.path.join(d, "Particles", str(hz), "particles.pbin")
        psteps, pdt, parts = read_pbin(p)
        assert psteps == steps and struct.pack("<f", pdt) == struct.pack("<f", dt)
        for i, (first, rec) in enumerate(parts):
            f = frames.pop((hz, i))
            assert f[0] == first, (hz, i, f[0], first)
            n = len(rec) // 16
            xyz = b"".join(rec[16 * k:16 * k + 12] for k in range(n))
            e = b"".join(rec[16 * k + 12:16 * k + 16] for k in range(n))
            assert f[1] == xyz and f[2] == e, (hz, i)
            checked += 1
            records += n
    assert not frames, f"{len(frames)} frames with no .pbin particle"
    in_order = order == sorted(order, key=lambda k: (bands.index(k[0]), k[1]))
    return {"bands": nb, "particles_per_band": per_band, "frames": checked, "records": records, "bytes": len(b), "in_order": in_order}


def main(exe, src, out, n, attrs, prune):
    os.makedirs(out, exist_ok=True)
    log = os.path.join(out, "cost.jsonl")
    ref = None
    runs = []
    plan = [(arm, i) for i in range(1, n + 1) for arm in ("off", "on")] + [("keep", 1)]
    for arm, i in plan:
        d = os.path.join(out, f"{arm}-{i}")
        stage.stage(src, d, dict(attrs), template=False)
        rc = stage.run(exe, d, ["--stream", arm])
        with open(os.path.join(d, "wall.txt")) as fh:
            wall = float(fh.readline())
        with open(os.path.join(d, "spps-gpu.json"), encoding="utf-8") as fh:
            j = json.load(fh)
        with open(os.path.join(d, "solver.stderr.txt"), encoding="utf-8", errors="replace") as fh:
            err = fh.read()
        h = hashes(d)
        if ref is None:
            ref = h
        same = h == ref
        diff = sorted(k for k in set(h) | set(ref) if h.get(k) != ref.get(k))
        row = {"arm": arm, "run": i, "exit": rc, "stderr": err, "wall_s": wall, "solver_wall_s": j["wall_seconds"],
               "kernel_s": j.get("kernel_seconds"), "trace_s": j["trace_seconds"], "retrace_s": j["retrace_seconds"],
               "band_output_s": j["band_output_seconds"], "stream": j.get("stream"), "stream_frames": j.get("stream_frames"),
               "stream_bytes": j.get("stream_bytes"), "stream_s": j.get("stream_seconds"),
               "stream_file_left": os.path.exists(os.path.join(d, "spps-gpu.pstream")),
               "files": len(h), "identical_to_off_1": same, "differing": diff[:10], "version": j.get("version")}
        if arm == "keep":
            row["stream_check"] = check_stream(d)
        runs.append((arm, i, d))
        with open(log, "a", encoding="utf-8") as fh:
            fh.write(json.dumps(row) + "\n")
        print(json.dumps(row), flush=True)
        with open(os.path.join(d, "hashes.json"), "w", encoding="utf-8") as fh:
            json.dump(h, fh, indent=0, sort_keys=True)
        if prune:
            # hashed and checked: the large outputs go (hashes.json stays as the receipt)
            for sub in ("Surface receiver", "Particles", "Punctual receivers", "Intensity animation", "spps-gpu.pstream"):
                q = os.path.join(d, sub)
                if os.path.isdir(q):
                    shutil.rmtree(q, ignore_errors=True)
                elif os.path.exists(q):
                    os.remove(q)


if __name__ == "__main__":
    a = [x for x in sys.argv[1:] if x != "--prune"]
    if len(a) < 4:
        sys.exit(__doc__)
    main(a[0], a[1], a[2], int(a[3]), dict(x.split("=", 1) for x in a[4:]), "--prune" in sys.argv)
