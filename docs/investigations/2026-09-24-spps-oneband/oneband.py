"""Item 6: spps_oneband run N times with one spps.exe and seed 1, each in a fresh folder, then N
times in one reused folder; the control is the same fixture with the source's spectrum complete
(a 500 Hz entry added). Per run: the 1000 Hz particle statistics and the 1000 Hz column of
`Total energy.recp` summed (the emitted power the solver used shows in it)."""
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
WT = Path(r"B:\repos\I-Simpa_Night_Mode\.claude\worktrees\t3-proj")
FIX = WT / "tests/fixtures/runs/spps_oneband"
SIMPA = WT / "target/debug/simpa.exe"
SPPS = Path(sys.argv[2]) if len(sys.argv) > 2 else WT / "target/solvers/bin/spps.exe"
N = int(sys.argv[1]) if len(sys.argv) > 1 else 30
OUT = HERE / "ob"


def dump(path: Path) -> str:
    return subprocess.run([str(SIMPA), "dump", "gabe", str(path)], capture_output=True, text=True, check=True).stdout


def columns(text: str):
    """GABE dump -> {column label: [values]}: float columns decoded from their f32 bits (hex)."""
    import struct
    cols, cur, kind = {}, None, None
    for line in text.splitlines():
        m = re.match(r"column (\S+) (.*)$", line)
        if m:
            kind, cur = m.group(1), m.group(2).replace("\\x20", " ")
            cols[cur] = []
            continue
        if line.startswith("rows ") or line.startswith("digits "):
            continue
        if cur is not None:
            cols[cur].append(struct.unpack(">f", bytes.fromhex(line))[0] if kind == "float" else line)
    return cols


def stage(d: Path, control: bool):
    d.mkdir(parents=True, exist_ok=True)
    cfg = (FIX / "config.xml").read_text(encoding="utf-8")
    cfg = cfg.replace("__RUNDIR__", str(d) + "\\")
    if control:
        cfg = cfg.replace('<bfreq freq="1000" db="90"/></source>', '<bfreq freq="1000" db="90"/><bfreq freq="500" db="90"/></source>')
        assert '<bfreq freq="500" db="90"/></source>' in cfg
    (d / "config.xml").write_text(cfg, encoding="utf-8")
    for f in ("mesh.cbin", "tetramesh.mbin"):
        shutil.copy(FIX / f, d / f)


def run(d: Path):
    r = subprocess.run([str(SPPS), "config.xml"], cwd=d, capture_output=True, text=True)
    st = columns(dump(d / "SPPS particle statistics.gabe"))
    te = columns(dump(d / "Total energy.recp"))
    k1000 = [k for k in te if k.startswith("1000")]
    energy = sum(float(v) for v in te[k1000[0]]) if k1000 else None
    stats1000 = st.get("1000 Hz")
    return {"exit": r.returncode, "stats_1000": stats1000, "stats_500": st.get("500 Hz"),
            "energy_1000_sum": energy, "energy_1000_first": te[k1000[0]][:3] if k1000 else None}


results = {"spps": str(SPPS), "fresh": [], "same": [], "control": []}
for i in range(N):
    d = OUT / f"f{i:02d}"
    stage(d, False)
    results["fresh"].append(run(d))
same = OUT / "same"
stage(same, False)
for i in range(N):
    results["same"].append(run(same))
for i in range(N):
    d = OUT / f"c{i:02d}"
    stage(d, True)
    results["control"].append(run(d))

for k in ("fresh", "same", "control"):
    outcomes = {}
    for r in results[k]:
        key = (r["exit"], tuple(r["stats_1000"]), r["energy_1000_sum"])
        outcomes[key] = outcomes.get(key, 0) + 1
    print(f"{k}: {len(results[k])} runs, {len(outcomes)} distinct outcomes")
    for key, n in sorted(outcomes.items(), key=lambda x: -x[1]):
        print(f"   {n:3d} x exit {key[0]}, 1000 Hz stats {key[1]}, Total energy 1000 Hz summed {key[2]!r}")
(HERE / f"oneband-{SPPS.parent.name}.json").write_text(json.dumps(results, indent=1), encoding="utf-8")
