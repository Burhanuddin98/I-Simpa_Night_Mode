"""spps-gpu bed, B3: is the walk, and are its outputs, the same with the live stream on?

Three runs of one case at the config's seed, each staged fresh (stage.py): stream off (a), stream off
again (b), stream on. Each writes --dump-walk and --dump-sums. Prints one JSON object:
- `walk`: compare_builds.py's per-band walk mismatches (step count, fate, final energy bits, children)
  for on vs off-a, and off-b vs off-a: must be 0 for both.
- `sums`: the double sums and float cells, on vs off-a beside off-b vs off-a: the GPU's atomic adds land
  in a different order on every run, so a cell may differ by float rounding between two stream-off runs;
  the stream is clean when on vs off-a is no further apart than off-b vs off-a.
- `files`: every output file hashed; the files on vs off-a differ in, each with whether off-b differs too.

usage: python stream_identity.py <exe> <src-solve-dir> <out-dir> [attr=value ...]
"""
import json, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import stage  # noqa: E402
from stream_cost import hashes  # noqa: E402


def compare(a, b):
    p = subprocess.run([sys.executable, os.path.join(HERE, "compare_builds.py"), a + ".walk", b + ".walk", a + ".sums", b + ".sums"],
                       capture_output=True, text=True, check=True)
    return json.loads(p.stdout)


def main(exe, src, out, attrs):
    os.makedirs(out, exist_ok=True)
    dirs = {}
    for arm, mode in (("off-a", "off"), ("off-b", "off"), ("on", "on")):
        d = os.path.join(out, arm)
        stage.stage(src, d, dict(attrs), template=False)
        rc = stage.run(exe, d, ["--stream", mode, "--dump-walk", d + ".walk", "--dump-sums", d + ".sums"])
        err = open(os.path.join(d, "solver.stderr.txt"), encoding="utf-8", errors="replace").read()
        if rc != 0 or err:
            sys.exit(f"{arm}: exit {rc}, stderr {err[:500]!r}")
        dirs[arm] = d
    on, rep = compare(dirs["off-a"], dirs["on"]), compare(dirs["off-a"], dirs["off-b"])
    h = {k: hashes(d) for k, d in dirs.items()}
    files = sorted(set(h["off-a"]) | set(h["on"]))
    differ = [{"file": f, "off_b_differs_too": h["off-b"].get(f) != h["off-a"].get(f)} for f in files if h["on"].get(f) != h["off-a"].get(f)]
    bands = []
    for x, y in zip(on["bands"], rep["bands"]):
        bands.append({"band": x["band"], "particles": x["particles"], "steps_traced": x["steps_traced"],
                      "walk_mismatches_on": sum(x["walk_mismatches"].values()), "walk_mismatches_rep": sum(y["walk_mismatches"].values()),
                      "states_equal_on": x["states_equal"],
                      "double_rel_on": x["double_sums_max_rel_of_peak"], "double_rel_rep": y["double_sums_max_rel_of_peak"],
                      "surf_rel_on": x["surf"]["max_of_peak"], "surf_rel_rep": y["surf"]["max_of_peak"],
                      "cut_rel_on": x["cut"]["max_of_peak"], "cut_rel_rep": y["cut"]["max_of_peak"]})
    res = {"case": out, "walk_identical_on": on["walk_identical"], "walk_identical_rep": rep["walk_identical"],
           "files": len(files), "files_identical_on": len(files) - len(differ), "differ": differ,
           "differ_only_where_repeat_differs": all(d["off_b_differs_too"] for d in differ), "bands": bands}
    with open(os.path.join(out, "identity.json"), "w", encoding="utf-8") as fh:
        json.dump(res, fh, indent=1)
    print(json.dumps(res))


if __name__ == "__main__":
    a = sys.argv[1:]
    if len(a) < 3:
        sys.exit(__doc__)
    main(a[0], a[1], a[2], dict(x.split("=", 1) for x in a[3:]))
