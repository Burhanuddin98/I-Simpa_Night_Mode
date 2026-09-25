"""What the C and D truncation limits let through: every SPPS run of the noise calibration
(rounds 1 to 4, 940 runs) and tutorial 1 at upstream's default (20 runs), read with one
`simpa results --json` build. Per receiver-band, for C50, C80 and D50 (and the other five for
context): the value, or the refusal's code and why.

    python cd_limits.py <simpa.exe> <out.json> [jobs]
"""
import json
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

AGENTS = Path(r"B:\repos\I-Simpa_Night_Mode\target\agents")
ROOTS = [
    AGENTS / "pm8-noise-scratch" / "runs",
    AGENTS / "pm8-fix-noise-scratch" / "runs",
]
QS = ["c50_db", "c80_db", "d50", "ts_s", "edt_s", "t20_s", "t30_s", "spl_db"]


def runs():
    out = []
    for root in ROOTS:
        for rj in sorted(root.rglob("run.json")):
            d = rj.parent
            if (d / "solve").is_dir():
                out.append(d)
    return out


def label(d: Path) -> str:
    # .../runs/<batch>/<cell>/seedNN/runs/<run>  or  .../tutorial1-default-*/<Method>/seedNN/runs/<run>
    parts = d.parts
    i = max(k for k, p in enumerate(parts) if p.startswith("seed"))
    return "/".join(parts[i - 2 : i + 1])


def read(exe: str, d: Path):
    r = subprocess.run([exe, "results", str(d), "--json"], capture_output=True, text=True)
    if r.returncode not in (0, 6):
        return {"exit": r.returncode, "stderr": r.stderr[-400:]}
    rep = json.loads(r.stdout)
    rows = []
    for ri, rc in enumerate(rep["spps"]["point_receivers"]):
        for b in rc["bands"]:
            row = {"receiver": ri, "freq_hz": b.get("freq_hz")}
            for q in QS:
                p = b["parameters"][q]
                if isinstance(p.get("value"), (int, float)) and p.get("value") is not None:
                    row[q] = ["value", p["value"]]
                else:
                    ne = p.get("not_evaluable") or {}
                    why = (((ne.get("error") or {}).get("why") or {}).get("why")) or ne.get("code")
                    row[q] = ["refused", ne.get("code"), why]
            rows.append(row)
    return {"exit": r.returncode, "method": rep["spps"].get("method"), "rows": rows}


def main():
    exe, out = sys.argv[1], Path(sys.argv[2])
    jobs = int(sys.argv[3]) if len(sys.argv) > 3 else 12
    ds = runs()
    print(f"{len(ds)} runs", flush=True)
    res = {}
    with ThreadPoolExecutor(jobs) as ex:
        for d, r in zip(ds, ex.map(lambda d: read(exe, d), ds)):
            res[str(d)] = {"label": label(d), **r}
    out.write_text(json.dumps({"exe": exe, "runs": res}), encoding="utf-8")
    bad = [k for k, v in res.items() if v["exit"] not in (0, 6)]
    print(f"written {out}; {len(bad)} runs not read", flush=True)
    for k in bad[:10]:
        print("  ", k, res[k])


if __name__ == "__main__":
    main()
