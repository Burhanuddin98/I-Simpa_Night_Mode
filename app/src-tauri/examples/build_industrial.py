"""The industrial hall example: upstream I-Simpa's tutorial 3, imported by `simpa import-proj`, then three edits.

1. Receiver 1 sits on a wall in upstream's project ((0, 1, 1.6), on face 12 of `diff_wall`, 0 m; `simpa validate`
   refuses it as receiver_on_surface). Upstream's receivers lie on the line (k-1, k, 1.6); Receiver 1 moves along
   that line to (0.5, 1.5, 1.6), 0.5 m from the wall, beyond the 0.31 m receiver radius.
2. The six sources are named Source 1-3 twice (two milling machines): renamed "Machine 1, source 1" and so on, so
   the scene list tells them apart. Their groups stay as upstream has them.
3. SPPS takes the app's new-project defaults (150,000 particles per source, 10 s, 1 ms, extinction 7, echograms
   per source), as decision 49 did for the Elmia hall, over the tutorial's 50,000, 2 s, 2 ms, 5.
"""
import json
from pathlib import Path

HERE = Path(__file__).parent
p = json.loads((HERE / "industrial.simpa").read_text(encoding="utf-8"))

r1 = next(r for r in p["point_receivers"] if r["name"] == "Receiver 1")
assert r1["position"] == [0.0, 1.0, 1.6], r1["position"]
r1["position"] = [0.5, 1.5, 1.6]

machine = {"Milling Machine": 1, "Milling Machine 2": 2}
for s in p["sources"]:
    s["name"] = f"Machine {machine[s['group']]}, source {s['name'].split()[-1]}"

sp = p["solvers"]["spps"]
sp.update({"particles_per_source": 150_000, "duration_s": 10.0, "time_step_s": 0.001,
           "extinction_exponent": 7.0, "echogram_per_source": True})

p["name"] = "Industrial hall"
p["description"] = ("Upstream I-Simpa's tutorial 3 (Industrial_hall.ply, tutorial_3.proj; GPL-3), imported by "
                    "simpa import-proj: two milling machines (three sources each) among fitting zones. Edited: "
                    "Receiver 1 moved off the wall along its line to (0.5, 1.5, 1.6); sources renamed per machine; "
                    "SPPS at the app's new-project defaults.")
(HERE / "industrial_example.simpa").write_text(json.dumps(p, indent=2), encoding="utf-8")
print([s["name"] for s in p["sources"]], r1["position"])
