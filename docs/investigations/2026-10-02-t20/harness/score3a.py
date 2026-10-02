"""T20 part 3a: the shown range per receiver on the M8a runs (../PREREG-3A.md, frozen). Scores a bed report.json.

  python -B score3a.py REPORT_JSON [--out OUT_DIR]

Rows: every seed, receiver and band of every M8a cell (gated and reported, both modes), from
cells[].t20.seeds[].t20[r][b] = {t, mc_sd, source}. Value: `t` when source is `value` or a noise refusal
(`monte_carlo_noise`, `noise_uncalibrated`; SPEC section 4); any other source, or a seed whose run could not be
read, leaves the row unanswered, counted by code. Truth: the transport's per-receiver T20,
transports[].receivers_t20[r] = [mean, se], the transport matched on (room, alpha, air) with the air the cell's
reference band's bed_air_m_per_metre (None with the air off), as the bed's own T20 twin does. A row whose truth
se / mean exceeds 0.5 %, or whose truth is refused, leaves the denominators and is counted.

Covered: |truth - value| <= Z*mc_sd + 0.005*value (Z = 2.5; an answered value with no mc_sd shows no range, so
its half-width is the 0.5 % term alone, and it is counted). Wrong-silent: answered, not covered, and
|value/truth - 1| > 5 %. J2 per mode: coverage >= 90 % and wrong-silent <= 3 % of answered rows. J3: no
cell x receiver with >= 20 answered rows has wrong-silent > 10 %. Part 3a passes when J2 and J3 hold.

Writes OUT_DIR/summary.json and OUT_DIR/rows.csv when --out is given; prints the summary either way.
"""
import sys

sys.dont_write_bytecode = True

import argparse  # noqa: E402
import csv  # noqa: E402
import json  # noqa: E402
import statistics  # noqa: E402
from pathlib import Path  # noqa: E402

Z = 2.5
Z_SENSITIVITY = (2.0, 3.0)
JND = 0.05
TOL = 0.005  # 1/10 of the JND: the covered allowance and the truth-uncertainty limit
J2_COVERAGE = 0.90
J2_WRONG_SILENT = 0.03
J3_MIN_ROWS = 20
J3_WRONG_SILENT = 0.10
ANSWERED_SOURCES = ("value", "monte_carlo_noise", "noise_uncalibrated")
FAR_ROOM = "20x8x4"
FAR_MODE = "energetic"


class ScoreError(Exception):
    pass


def _air_of(cell, freq):
    if not cell["air"]:
        return None
    for b in cell["reference"]["bands"]:
        if b["freq_hz"] == freq:
            return b["bed_air_m_per_metre"]
    raise ScoreError(f"{cell['id']}: no reference band at {freq} Hz")


def _transport_index(report):
    idx = {}
    for t in report["transports"]:
        idx[(t["room"], t["alpha"], t["air_m_per_metre"])] = t
    return idx


def covered(value, mc_sd, truth, z=Z):
    half = z * (mc_sd or 0.0) + TOL * value
    return abs(truth - value) <= half


def rows_of(report):
    """Every row, one dict per seed x receiver x band x cell, before any exclusion."""
    tr = _transport_index(report)
    out = []
    for cell in report["cells"]:
        t20 = cell.get("t20")
        if t20 is None:
            raise ScoreError(f"{cell['id']}: no t20 block")
        bands = cell["bands_hz"]
        truths = []
        for b, f in enumerate(bands):
            key = (cell["room"], cell["alpha"], _air_of(cell, f))
            t = tr.get(key)
            if t is None:
                truths.append(None)
                continue
            if not t.get("receivers_t20"):
                raise ScoreError(
                    f"transport {key} has no receivers_t20: the report predates the per-receiver T20 field"
                )
            truths.append(t["receivers_t20"])
        receivers = max((len(t) for t in truths if t is not None), default=0)
        for seed in t20["seeds"]:
            for r in range(receivers):
                for b, f in enumerate(bands):
                    row = {
                        "cell": cell["id"], "room": cell["room"], "alpha": cell["alpha"],
                        "mode": cell["method"], "air": cell["air"], "gated": cell["gated"],
                        "seed": seed["seed"], "receiver": r, "freq_hz": f,
                        "value": None, "mc_sd": None, "source": None,
                        "truth": None, "truth_se": None,
                    }
                    tb = truths[b]
                    if tb is not None and tb[r] is not None:
                        row["truth"], row["truth_se"] = tb[r][0], tb[r][1]
                    if seed.get("error") is not None or not seed["t20"]:
                        row["source"] = "run_error"
                    else:
                        x = seed["t20"][r][b]
                        row["source"] = x["source"]
                        if x["source"] in ANSWERED_SOURCES and x["t"] is not None:
                            row["value"], row["mc_sd"] = x["t"], x["mc_sd"]
                    out.append(row)
        ext = t20.get("extension")
        if ext is not None and ext.get("seeds"):
            raise ScoreError(f"{cell['id']}: has T20 extension seeds, which PREREG-3A does not define rows for")
    return out


def classify(row, z=Z):
    """Adds `status` (excluded_* / unanswered / answered) and, for answered rows, err, covered, wrong_silent."""
    if row["truth"] is None:
        row["status"] = "excluded_truth_missing"
    elif row["truth_se"] / row["truth"] > TOL:
        row["status"] = "excluded_truth_uncertain"
    elif row["value"] is None:
        row["status"] = "unanswered"
    else:
        row["status"] = "answered"
        row["err"] = row["value"] / row["truth"] - 1.0
        row["covered"] = covered(row["value"], row["mc_sd"], row["truth"], z)
        row["wrong_silent"] = (not row["covered"]) and abs(row["err"]) > JND
        row["wide"] = row["covered"] and abs(row["err"]) > JND
        for zz in Z_SENSITIVITY:
            row[f"covered_z{zz:g}"] = covered(row["value"], row["mc_sd"], row["truth"], zz)
    return row


def _tally(rows):
    a = [r for r in rows if r["status"] == "answered"]
    n = len(a)
    out = {"answered": n}
    if n:
        out["covered"] = sum(r["covered"] for r in a)
        out["wrong_silent"] = sum(r["wrong_silent"] for r in a)
        out["coverage"] = out["covered"] / n
        out["wrong_silent_rate"] = out["wrong_silent"] / n
        out["wide_covered_over_jnd"] = sum(r["wide"] for r in a)
        for zz in Z_SENSITIVITY:
            out[f"coverage_z{zz:g}"] = sum(r[f"covered_z{zz:g}"] for r in a) / n
        errs = [r["err"] for r in a]
        out["mean_err"] = statistics.fmean(errs)
        out["median_err"] = statistics.median(errs)
        out["min_err"] = min(errs)
        out["max_err"] = max(errs)
    return out


def score(report, z=Z):
    rows = [classify(r, z) for r in rows_of(report)]
    counts = {}
    for r in rows:
        counts.setdefault(r["status"], 0)
        counts[r["status"]] += 1
    unanswered_by_code = {}
    for r in rows:
        if r["status"] == "unanswered":
            unanswered_by_code[r["source"]] = unanswered_by_code.get(r["source"], 0) + 1
    excluded_answered = sum(1 for r in rows if r["status"].startswith("excluded") and r["value"] is not None)
    no_mc_sd = sum(1 for r in rows if r["status"] == "answered" and r["mc_sd"] is None)

    j2 = {}
    for mode in sorted({r["mode"] for r in rows}):
        t = _tally([r for r in rows if r["mode"] == mode])
        t["coverage_ok"] = t["answered"] > 0 and t["coverage"] >= J2_COVERAGE
        t["wrong_silent_ok"] = t["answered"] > 0 and t["wrong_silent_rate"] <= J2_WRONG_SILENT
        t["holds"] = t["coverage_ok"] and t["wrong_silent_ok"]
        j2[mode] = t

    subgroups = []
    for cell in dict.fromkeys(r["cell"] for r in rows):
        for rec in sorted({r["receiver"] for r in rows if r["cell"] == cell}):
            sub = [r for r in rows if r["cell"] == cell and r["receiver"] == rec]
            t = _tally(sub)
            t["cell"], t["receiver"] = cell, rec
            t["mode"], t["gated"] = sub[0]["mode"], sub[0]["gated"]
            t["excluded"] = sum(1 for r in sub if r["status"].startswith("excluded"))
            t["unanswered"] = sum(1 for r in sub if r["status"] == "unanswered")
            t["j3_applies"] = t["answered"] >= J3_MIN_ROWS
            t["j3_breach"] = t["j3_applies"] and t["wrong_silent_rate"] > J3_WRONG_SILENT
            subgroups.append(t)
    j3_breaches = [f"{s['cell']} R{s['receiver']:03d}" for s in subgroups if s["j3_breach"]]
    j3 = {"holds": not j3_breaches, "breaches": j3_breaches,
          "subgroups_applying": sum(s["j3_applies"] for s in subgroups)}

    far = [s for s in subgroups
           if s["cell"].startswith(FAR_ROOM + "-") and s["mode"] == FAR_MODE]
    far_by_receiver = {}
    for rec in sorted({s["receiver"] for s in far}):
        sub = [r for r in rows if r["room"] == FAR_ROOM and r["mode"] == FAR_MODE and r["receiver"] == rec]
        far_by_receiver[f"R{rec:03d}"] = _tally(sub)

    overall = _tally(rows)
    return {
        "prereg": "docs/investigations/2026-10-02-t20/PREREG-3A.md",
        "z": z,
        "rows": len(rows),
        "status_counts": counts,
        "unanswered_by_code": unanswered_by_code,
        "excluded_answered_rows": excluded_answered,
        "answered_without_mc_sd": no_mc_sd,
        "overall": overall,
        "j2": j2,
        "j3": j3,
        "pass": all(t["holds"] for t in j2.values()) and bool(j2) and j3["holds"],
        "far_room_energetic": {
            "note": "part 1's -2.8 % estimate was R000 at alpha 0.4; error = value/truth - 1",
            "per_cell_receiver": [
                {k: s.get(k) for k in ("cell", "receiver", "answered", "mean_err", "median_err", "min_err",
                                       "max_err", "coverage", "wrong_silent_rate")}
                for s in far
            ],
            "per_receiver_all_alphas": far_by_receiver,
        },
        "subgroups": subgroups,
    }, rows


CSV_FIELDS = ["cell", "room", "alpha", "mode", "air", "gated", "seed", "receiver", "freq_hz", "source", "value",
              "mc_sd", "truth", "truth_se", "status", "err", "covered", "wrong_silent", "wide", "covered_z2",
              "covered_z3"]


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument("report")
    ap.add_argument("--out")
    a = ap.parse_args(argv)
    report = json.loads(Path(a.report).read_text(encoding="utf-8"))
    try:
        summary, rows = score(report)
    except ScoreError as e:
        print(f"refused: {e}", file=sys.stderr)
        return 2
    summary["report"] = str(Path(a.report).resolve())
    text = json.dumps(summary, indent=2)
    if a.out:
        out = Path(a.out)
        out.mkdir(parents=True, exist_ok=True)
        (out / "summary.json").write_text(text, encoding="utf-8")
        with open(out / "rows.csv", "w", newline="", encoding="utf-8") as f:
            w = csv.DictWriter(f, fieldnames=CSV_FIELDS, extrasaction="ignore")
            w.writeheader()
            w.writerows(rows)
    brief = {k: summary[k] for k in ("rows", "status_counts", "unanswered_by_code", "excluded_answered_rows",
                                      "answered_without_mc_sd", "j2", "j3", "pass")}
    brief["far_room_energetic"] = summary["far_room_energetic"]["per_receiver_all_alphas"]
    print(json.dumps(brief, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
