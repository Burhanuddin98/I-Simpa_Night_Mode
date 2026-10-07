"""spps-gpu bed (a): stage a case twice, run the CPU build and the GPU build with the same seed, and
compare them (compare_builds.py). Writes <root>/<case>-compare.json and prints a summary line.

usage: python cpu_gpu.py <root> <case> <src-solve-dir> <seed> [attr=value ...]
"""
import json, os, subprocess, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import stage  # noqa: E402

EXE = os.environ.get("SPPS_GPU_BED_EXE", r"C:\tmp\nm-spps-gpu\bin\spps-gpu.exe")   # B3: overridable

if __name__ == "__main__":
    if len(sys.argv) < 5:
        sys.exit(__doc__)
    root, case, src, seed = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
    attrs = dict(x.split("=", 1) for x in sys.argv[5:])
    attrs["random_seed"] = seed
    files = {}
    for arm in ("cpu", "gpu"):
        d = os.path.join(root, f"{case}-{arm}")
        stage.stage(src, d, dict(attrs), template=False)
        w, s = os.path.join(root, f"{case}-{arm}.walk"), os.path.join(root, f"{case}-{arm}.sums")
        extra = (["--cpu"] if arm == "cpu" else []) + ["--dump-walk", w, "--dump-sums", s]
        rc = stage.run(EXE, d, extra)
        if rc != 0:
            sys.exit(f"{arm} exit {rc}")
        files[arm] = (w, s, d)
    out = os.path.join(root, f"{case}-compare.json")
    p = subprocess.run([sys.executable, os.path.join(HERE, "compare_builds.py"), files["cpu"][0], files["gpu"][0], files["cpu"][1], files["gpu"][1]],
                       capture_output=True, text=True, check=True)
    with open(out, "w", encoding="utf-8") as fh:
        fh.write(p.stdout)
    d = json.loads(p.stdout)
    timing = {}
    for arm in ("cpu", "gpu"):
        with open(os.path.join(files[arm][2], "spps-gpu.json"), encoding="utf-8") as fh:
            j = json.load(fh)
        timing[arm] = {"trace_s": j["trace_seconds"], "wall_s": j["wall_seconds"]}
    for b in d["bands"]:
        print(json.dumps({"case": case, "band": b["band"], "particles": b["particles"], "steps": b["steps_traced"], "fates": b["fates"],
                          "walk_mismatches": sum(b["walk_mismatches"].values()), "states_equal": b["states_equal"],
                          "double_rel": b["double_sums_max_rel_of_peak"], "surf_rel": b["surf"]["max_rel_cell_above_1e-6_peak"],
                          "surf_cells": b["surf"]["nonzero_cpu"], "cut_rel": b["cut"]["max_rel_cell_above_1e-6_peak"],
                          "cut_cells": b["cut"]["nonzero_cpu"], "timing": timing}))
    print(json.dumps({"case": case, "walk_identical": d["walk_identical"], "double_sums_within_1e-12": d["double_sums_within_1e-12"]}))
