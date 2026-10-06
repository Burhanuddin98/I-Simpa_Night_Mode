"""spps-gpu bed (a), every case: the fixtures, CR4's 1 kHz band and the rule variants, CPU build against
GPU build at one seed each. Writes <root>/<case>-compare.json per case and <root>/summary.jsonl.

usage: python cpu_gpu_all.py <root>
"""
import json, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from run_bed import CASES  # noqa: E402

SEEDS = {"cr4-1k": "7"}

if __name__ == "__main__":
    root = sys.argv[1]
    order = ["seats", "energetic", "outputs", "sources2"] + [c for c in CASES if c.startswith("v-")] + ["cr4-1k"]
    for case in order:
        src, attrs = CASES[case]
        if case in ("seats", "energetic", "outputs", "sources2"):
            attrs = dict(attrs)
        cmd = [sys.executable, os.path.join(HERE, "cpu_gpu.py"), root, case, src, SEEDS.get(case, "11")] + [f"{k}={v}" for k, v in attrs.items()]
        p = subprocess.run(cmd, capture_output=True, text=True)
        lines = [l for l in p.stdout.splitlines() if l.startswith("{")]
        with open(os.path.join(root, "summary.jsonl"), "a", encoding="utf-8") as fh:
            for l in lines:
                fh.write(l + "\n")
        print("\n".join(lines[-1:]) if lines else (case + " FAILED " + p.stdout[-500:] + p.stderr[-800:]), flush=True)
