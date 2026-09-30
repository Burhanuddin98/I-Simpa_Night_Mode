"""Seal an earlier M8 bed's run files: one JSON file listing, for every run's folder under the
bed (runs/<cell>/s<seed>/), every file in it with its size and sha256.

A bed made before M8b has no output hashes in its runs' run.json, so `simpa bed --from` refuses
every run of it (bed_run_unbound) unless it is given a seal of that bed (`--seal`), against which
it then holds every file it reads (crates/simpa-core/src/bed/bind.rs). This script writes that
seal. It only reads the bed. It is deliberately not the Rust code that checks the seal: the two
walk and hash the files independently, so a re-read of the sealed bed that accepts every run is a
check of both.

    py -3 tools/bed/seal_bed.py <root>/<stamp> <out.json> --why "<why the files can be sealed>"

The seal records the bed's folder name (`simpa bed` refuses a seal of another bed), where and when
it was made, the sha256 of the bed's report.json and summary.json, and, as provenance, the newest
modification time of any file under runs/ against report.json's.
"""

import argparse
import datetime
import hashlib
import json
import os
import sys


def sha256_and_size(path):
    h = hashlib.sha256()
    n = 0
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
            n += len(block)
    return h.hexdigest(), n


def utc(ts):
    return datetime.datetime.fromtimestamp(ts, datetime.timezone.utc).strftime(
        "%Y-%m-%dT%H:%M:%S.%fZ"
    )


def run_folder(key_dir):
    """The one folder in key_dir holding a run.json; anything else is an error."""
    found = [
        d
        for d in sorted(os.listdir(key_dir))
        if os.path.isdir(os.path.join(key_dir, d))
        and os.path.isfile(os.path.join(key_dir, d, "run.json"))
    ]
    if len(found) != 1:
        sys.exit(f"{key_dir}: {len(found)} run folders, not 1: {found}")
    return found[0]


def seal_run(key_dir):
    entries = []
    newest = 0.0
    for dirpath, dirnames, filenames in os.walk(key_dir, followlinks=False):
        for d in dirnames:
            if os.path.islink(os.path.join(dirpath, d)):
                sys.exit(f"{os.path.join(dirpath, d)}: a link, not a folder")
        for name in filenames:
            path = os.path.join(dirpath, name)
            if os.path.islink(path) or not os.path.isfile(path):
                sys.exit(f"{path}: not a plain file")
            rel = os.path.relpath(path, key_dir).replace(os.sep, "/")
            sha, size = sha256_and_size(path)
            entries.append([rel, size, sha])
            newest = max(newest, os.stat(path).st_mtime)
    entries.sort(key=lambda e: e[0])
    return {"run_folder": run_folder(key_dir), "files": entries}, newest


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("bed", help="the bed's folder, <root>/<UTC stamp>, holding runs/")
    ap.add_argument("out", help="the seal file to write")
    ap.add_argument("--why", required=True, help="why the files can be sealed as they are")
    a = ap.parse_args()
    bed = os.path.abspath(a.bed)
    runs_dir = os.path.join(bed, "runs")
    if not os.path.isdir(runs_dir):
        sys.exit(f"{bed}: no runs/ folder")
    runs = {}
    files = size = 0
    newest = 0.0
    for cell in sorted(os.listdir(runs_dir)):
        cell_dir = os.path.join(runs_dir, cell)
        if not os.path.isdir(cell_dir):
            sys.exit(f"{cell_dir}: not a folder")
        for seed in sorted(os.listdir(cell_dir)):
            key_dir = os.path.join(cell_dir, seed)
            if not os.path.isdir(key_dir):
                sys.exit(f"{key_dir}: not a folder")
            run, n = seal_run(key_dir)
            runs[f"runs/{cell}/{seed}"] = run
            files += len(run["files"])
            size += sum(e[1] for e in run["files"])
            newest = max(newest, n)
    report = os.path.join(bed, "report.json")
    summary = os.path.join(bed, "summary.json")
    report_sha, _ = sha256_and_size(report)
    summary_sha, _ = sha256_and_size(summary)
    report_mtime = os.stat(report).st_mtime
    head = {
        "seal_version": 1,
        "bed": os.path.basename(bed),
        "sealed_from": bed,
        "sealed_utc": datetime.datetime.now(datetime.timezone.utc).strftime(
            "%Y-%m-%dT%H:%M:%SZ"
        ),
        "tool": "tools/bed/seal_bed.py",
        "why": a.why,
        "provenance": {
            "runs": str(len(runs)),
            "newest_run_file_mtime_utc": utc(newest),
            "report_json_mtime_utc": utc(report_mtime),
            "every_run_file_older_than_report_json": str(newest < report_mtime).lower(),
        },
        "report_json_sha256": report_sha,
        "summary_json_sha256": summary_sha,
        "files": files,
        "bytes": size,
    }
    # One line per file, so that a change to the seal reads in a diff as the files it touches.
    lines = ["{"]
    for k, v in head.items():
        lines.append(f"  {json.dumps(k)}: {json.dumps(v, ensure_ascii=False)},")
    lines.append('  "runs": {')
    keys = list(runs)
    for i, key in enumerate(keys):
        r = runs[key]
        lines.append(f"    {json.dumps(key)}: {{")
        lines.append(f'      "run_folder": {json.dumps(r["run_folder"])},')
        lines.append('      "files": [')
        for j, e in enumerate(r["files"]):
            comma = "," if j + 1 < len(r["files"]) else ""
            lines.append(f"        {json.dumps(e, ensure_ascii=False)}{comma}")
        lines.append("      ]")
        lines.append("    }" + ("," if i + 1 < len(keys) else ""))
    lines.append("  }")
    lines.append("}")
    text = "\n".join(lines) + "\n"
    json.loads(text)
    with open(a.out, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    print(
        f"sealed {len(runs)} runs, {files} files, {size} bytes of {bed} into {a.out}; newest run "
        f"file {utc(newest)}, report.json {utc(report_mtime)}"
    )


if __name__ == "__main__":
    main()
