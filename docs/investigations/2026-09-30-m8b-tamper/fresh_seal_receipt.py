"""Receipt for M8b tamper round 2: a bed that `simpa bed` runs is sealed by it, from what it made, and
a re-read of that bed is held to the seal. Run on a small exploratory bed (it never passes, E7) cut
from beds/m8a.json: the 5x4x3 room, two gated cells at 31,000 particles per source (5x4x3 α 0.4
random, α 0.2 energetic), their TCR run, seeds 1 to 3, N5 and N6 at 31,000 particles; upstream's
atmospheric validation not run (--upstream names a folder with no upstream tree).

  F1  simpa bed runs it fresh: its seal is written and printed;
  F2  the seal, copied out of the bed folder, re-reads it: every run bound by run.json and the seal,
      and every verdict and number the fresh report's;
  F3  --seal the seal inside the bed folder: refused, exit 2, nothing read;
  F4  --from with no --seal: refused, exit 2;
  F5  a copy of the bed under the same folder name, U1's tamper in one run (seed 2 of the α 0.4 cell:
      byte 17 of one gabe changed, run.json's outputs rewritten to match) and U2's in another (seed 3
      carrying seed 1's solver outputs, byte 17 of each gabe changed, run.json forged): read against
      the same seal, both refused bed_run_files_changed, the cell not judged.

Writes the log given as argv[1] and the small bed file beside it (fresh-seal-bed.json). Everything
else it writes is under C:\\tmp\\nm-judge-tamper\\r2\\fresh; nothing is deleted."""

import hashlib
import json
import os
import shutil
import subprocess
import sys
import time

WT = r"B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b-tamper"
SIMPA = r"C:\tmp\nm-target\release\simpa.exe"
ROOT = r"C:\tmp\nm-judge-tamper\r2\fresh"
ENV = dict(os.environ, SIMPA_SOLVERS_DIR="C:/tmp/nm-m8a-solvers")
ENV.pop("SIMPA_UPSTREAM", None)


def small_bed(path):
    b = json.load(open(os.path.join(WT, "beds", "m8a.json"), encoding="utf-8"))
    b["rooms"] = [r for r in b["rooms"] if r["name"] == "5x4x3"]
    keep = ["5x4x3-a0.4-random-air-off", "5x4x3-a0.2-energetic-air-off"]
    b["cells"] = [dict(c, particles_per_source=31000) for c in b["cells"] if c["id"] in keep]
    b["tcr"] = [t for t in b["tcr"] if t["id"] == "5x4x3-a0.2-tcr-air-off"]
    b["seeds"] = [1, 2, 3]
    b["extension_seeds"] = [11, 12, 13]
    b["say_no"]["n6"]["particles_per_source"] = 31000
    open(path, "w", encoding="utf-8", newline="\n").write(json.dumps(b, indent=2) + "\n")


def simpa(args, log, name):
    t0 = time.time()
    p = subprocess.run([SIMPA] + args, env=ENV, capture_output=True, text=True, encoding="utf-8", errors="replace")
    secs = time.time() - t0
    keep = [l for l in p.stderr.splitlines() if l.startswith(("simpa:", "seal:", "[", "FAIL", "E1"))]
    log.write(f"==== {name}\nsimpa {' '.join(args)}\nexit {p.returncode} ({secs:.0f} s)\n" + "\n".join(keep[:60]) + "\n\n")
    log.flush()
    print(f"{name}: exit {p.returncode} ({secs:.0f} s)", flush=True)
    return p


def stamp_of(stderr):
    for line in reversed(stderr.splitlines()):
        if line.rstrip().endswith("report.json") and " " in line:
            return os.path.dirname(line.rstrip().split(" ")[-1])
    return None


def seal_line(stderr):
    for line in stderr.splitlines():
        if line.startswith("seal: "):
            parts = line.split(" ")
            return parts[1], parts[3]
    return None


def lf_sha(path):
    return hashlib.sha256(open(path, "rb").read().replace(b"\r\n", b"\n")).hexdigest()


def runs_of(report):
    out = {}
    for c in report["cells"]:
        for s in c["seeds"]:
            out[f"{c['id']} s{s['seed']}"] = (s.get("run") or {}, s.get("error"), s.get("t30"))
    for t in report["tcr"]:
        out[t["id"]] = (t.get("run") or {}, t.get("error"), None)
    for k in ("n5", "n6"):
        n = report["say_no"][k]
        out[k] = (n.get("run") or {}, n.get("error"), None)
    return out


def strip(cells):
    """The cells with what differs by design between a run and its re-read left out: each run's
    record (its folder path, and bound_by)."""
    out = json.loads(json.dumps(cells))
    for c in out:
        for s in c["seeds"]:
            s.pop("run", None)
    return out


def verdicts(report):
    return (
        [(c["id"], c["verdict"]) for c in report["cells"]],
        [(t["id"], t["verdict"]) for t in report["tcr"]],
        report["say_no"]["n5"]["as_required"],
        report["say_no"]["n6"]["as_required"],
    )


def tamper_gabe(path):
    b = bytearray(open(path, "rb").read())
    b[17] = (b[17] + 1) % 256
    open(path, "wb").write(bytes(b))


def rewrite_outputs(folder):
    """run.json's outputs rewritten to the files as they now are (the forged record)."""
    rj = os.path.join(folder, "run.json")
    m = json.load(open(rj, encoding="utf-8"))
    outs = []
    for d, _, files in os.walk(folder):
        for f in files:
            p = os.path.join(d, f)
            rel = os.path.relpath(p, folder).replace(os.sep, "/")
            if rel == "run.json":
                continue
            data = open(p, "rb").read()
            outs.append({"path": rel, "size": len(data), "sha256": hashlib.sha256(data).hexdigest()})
    m["outputs"] = sorted(outs, key=lambda o: o["path"])
    open(rj, "w", encoding="utf-8", newline="\n").write(json.dumps(m, indent=2) + "\n")


def run_folder(d):
    return [os.path.join(d, x) for x in os.listdir(d) if os.path.isfile(os.path.join(d, x, "run.json"))][0]


def main():
    logpath = sys.argv[1]
    log = open(logpath, "w", encoding="utf-8", newline="\n")
    bed = os.path.join(os.path.dirname(logpath), "fresh-seal-bed.json")
    small_bed(bed)
    no_upstream = os.path.join(ROOT, "no-upstream")
    os.makedirs(no_upstream, exist_ok=True)
    head = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=WT, capture_output=True, text=True).stdout.strip()
    log.write(f"tree: HEAD {head}; simpa.exe {SIMPA}, sha256 {hashlib.sha256(open(SIMPA, 'rb').read()).hexdigest()}\n")
    log.write(f"bed file: {bed}, sha256 {hashlib.sha256(open(bed, 'rb').read()).hexdigest()}\n\n")

    p = simpa(["bed", bed, "--out", os.path.join(ROOT, "made"), "--jobs", "4", "--upstream", no_upstream], log, "F1 the bed run fresh")
    stamp = stamp_of(p.stderr)
    seal_path, printed = seal_line(p.stderr)
    fresh = json.load(open(os.path.join(stamp, "report.json"), encoding="utf-8"))
    seal = json.load(open(seal_path, encoding="utf-8"))
    log.write(f"stamp {stamp}\nseal {seal_path}: printed sha256 {printed}, file {lf_sha(seal_path)}; bed {seal['bed']}; "
              f"{len(seal['runs'])} runs, {seal['files']} files, {seal['bytes']} bytes; provenance {seal['provenance']}\n")
    log.write(f"report.json sha256 {hashlib.sha256(open(os.path.join(stamp, 'report.json'), 'rb').read()).hexdigest()}, the seal's {seal['report_json_sha256']}\n")
    fr = runs_of(fresh)
    log.write(f"fresh reads: {len(fr)}; bound_by {sorted(set(str(r[0].get('bound_by')) for r in fr.values()))}; errors {sum(1 for r in fr.values() if r[1])}\n")
    # The seal against the bed, re-hashed here.
    bad = 0
    for key, run in seal["runs"].items():
        for path, size, sha in run["files"]:
            data = open(os.path.join(stamp, key.replace("/", os.sep), path.replace("/", os.sep)), "rb").read()
            bad += (len(data) != size) or (hashlib.sha256(data).hexdigest() != sha)
        on_disk = sum(len(fs) for _, _, fs in os.walk(os.path.join(stamp, key.replace("/", os.sep))))
        bad += on_disk != len(run["files"])
    log.write(f"the seal against the bed folder, re-hashed by Python: {bad} differences\n\n")

    outside = os.path.join(ROOT, "seal-copy", "outputs-seal.json")
    os.makedirs(os.path.dirname(outside), exist_ok=True)
    shutil.copyfile(seal_path, outside)
    p = simpa(["bed", bed, "--out", os.path.join(ROOT, "reread"), "--jobs", "4", "--upstream", no_upstream, "--from", stamp, "--seal", outside], log, "F2 re-read against the copied seal")
    again = json.load(open(os.path.join(stamp_of(p.stderr), "report.json"), encoding="utf-8"))
    ar = runs_of(again)
    same_t30 = all(fr[k][2] == ar[k][2] for k in fr)
    log.write(f"re-read: {len(ar)} reads; bound_by {sorted(set(str(r[0].get('bound_by')) for r in ar.values()))}; errors {sum(1 for r in ar.values() if r[1])}; "
              f"meta.seal_sha256 {again['meta']['seal_sha256']} (the copy's {lf_sha(outside)}); "
              f"verdicts equal {verdicts(fresh) == verdicts(again)}; every seed's T30 equal {same_t30}; "
              f"cells equal as a whole {strip(fresh['cells']) == strip(again['cells'])}\n\n")

    simpa(["bed", bed, "--out", os.path.join(ROOT, "inside"), "--from", stamp, "--seal", seal_path], log, "F3 the seal inside the bed folder")
    log.write(f"F3 wrote: {os.listdir(os.path.join(ROOT, 'inside')) if os.path.isdir(os.path.join(ROOT, 'inside')) else 'nothing'}\n\n")
    simpa(["bed", bed, "--out", os.path.join(ROOT, "noseal"), "--from", stamp], log, "F4 --from with no --seal")

    copy = os.path.join(ROOT, "tampered", os.path.basename(stamp))
    shutil.copytree(os.path.join(stamp, "runs"), os.path.join(copy, "runs"))
    cell = os.path.join(copy, "runs", "5x4x3-a0.4-random-air-off")
    s2 = run_folder(os.path.join(cell, "s2"))
    g = [os.path.join(d, f) for d, _, fs in os.walk(os.path.join(s2, "solve")) for f in fs if f.endswith(".gabe")]
    tamper_gabe(sorted(g)[0])
    rewrite_outputs(s2)
    s1, s3 = run_folder(os.path.join(cell, "s1")), run_folder(os.path.join(cell, "s3"))
    inputs = {i["path"] for i in json.load(open(os.path.join(s3, "run.json"), encoding="utf-8"))["inputs"]}
    copied = 0
    for d, _, fs in os.walk(os.path.join(s1, "solve")):
        for f in fs:
            rel = os.path.relpath(os.path.join(d, f), os.path.join(s1, "solve"))
            if rel.replace(os.sep, "/") in inputs:
                continue
            dst = os.path.join(s3, "solve", rel)
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            shutil.copyfile(os.path.join(d, f), dst)
            if f.endswith(".gabe"):
                tamper_gabe(dst)
            copied += 1
    rewrite_outputs(s3)
    log.write(f"F5 tampered copy {copy}: seed 2's {os.path.basename(sorted(g)[0])} byte 17 changed, record forged; seed 3 carrying seed 1's {copied} solver outputs, gabes changed, record forged\n")
    p = simpa(["bed", bed, "--out", os.path.join(ROOT, "tampered-read"), "--jobs", "4", "--upstream", no_upstream, "--from", copy, "--seal", outside], log, "F5 the tampered copy against the seal")
    t = json.load(open(os.path.join(stamp_of(p.stderr), "report.json"), encoding="utf-8"))
    for c in t["cells"]:
        if c["id"] == "5x4x3-a0.4-random-air-off":
            log.write(f"F5 cell {c['id']}: verdict {c['verdict']}\n")
            for s in c["seeds"]:
                log.write(f"  seed {s['seed']}: {(s.get('error') or 'read')[:300]}\n")
    log.write(f"F5 pass {t['pass']}\n\nwritten under {ROOT}, left in place\n")
    log.close()


if __name__ == "__main__":
    main()
