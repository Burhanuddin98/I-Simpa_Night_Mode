"""Item 6: spps_oneband run N more times in ob/same (one folder, reused: no new files)."""
import sys
from pathlib import Path

sys.argv = [sys.argv[0], "0"] + sys.argv[2:] if len(sys.argv) > 2 else [sys.argv[0], "0"]
import importlib.util

spec = importlib.util.spec_from_file_location("ob", Path(__file__).with_name("oneband.py"))
N = int(__import__("os").environ.get("OB_N", "200"))
ob = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ob)  # N=0: stages nothing, runs nothing
same = ob.OUT / "same"
ob.stage(same, False)
outcomes = {}
for i in range(N):
    r = ob.run(same)
    key = (r["exit"], tuple(r["stats_1000"]), r["energy_1000_sum"])
    outcomes[key] = outcomes.get(key, 0) + 1
print(f"same folder, {N} runs, {len(outcomes)} distinct outcomes")
for key, n in sorted(outcomes.items(), key=lambda x: -x[1]):
    print(f"   {n:3d} x exit {key[0]}, 1000 Hz stats {key[1]}, Total energy 1000 Hz summed {key[2]!r}")
