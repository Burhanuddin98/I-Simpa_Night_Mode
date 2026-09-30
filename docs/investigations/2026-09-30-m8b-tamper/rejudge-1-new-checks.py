"""Round 1 re-judge: what this round's checks put into report.json, read independently of the Rust.

Reads only. For the re-judged report (and the seal it was read through):
- meta: the seal and its sha256, the commit and dirt, the bed read, the phases;
- every run record (a dict with 'folder' and 'status'): how it was bound, its on-disk hashes, its
  mesh check, its outputs_sha256; each held to the seal's own entry for its run folder, independently;
- every refusal code this round added, and step 1's bed_run_not_planned, anywhere in the report;
- the seed rule: no two seeds of a cell (or of the atmospheric validation) with the same outputs_sha256.

usage: py -3 r1_extra_checks.py <report.json> <seal.json> <seal file on disk, for its sha256>
"""
import sys as _s; _s.stdout.reconfigure(encoding="utf-8")
import hashlib
import json
import sys
from collections import Counter, defaultdict

rep = json.load(open(sys.argv[1], encoding="utf-8"))
seal = json.load(open(sys.argv[2], encoding="utf-8"))
seal_sha = hashlib.sha256(open(sys.argv[3], "rb").read()).hexdigest()

m = rep["meta"]
print("meta:")
for k in ("git_commit", "git_dirty", "from", "root", "jobs", "seal", "seal_sha256", "started_utc", "finished_utc",
          "wall_s", "solver_phase_s", "transport_phase_s", "machine"):
    print(f"  {k}: {m.get(k)!r}")
print(f"  seal_sha256 equals the sha256 of the seal file on disk ({seal_sha}): {m.get('seal_sha256') == seal_sha}")
print(f"pass: {rep['pass']!r}; failures: {len(rep['failures'])}; exploratory: {rep['exploratory']!r}; "
      f"needs_extension: {rep['needs_extension']!r}")
print(f"transport_errors: {rep.get('transport_errors')!r}")

# Every run record in the report, with where it sits.
runs = []


def walk(x, path):
    if isinstance(x, dict):
        if "folder" in x and "status" in x and "exe_sha256" in x:
            runs.append((path, x))
        for k, v in x.items():
            walk(v, f"{path}.{k}" if path else k)
    elif isinstance(x, list):
        for i, v in enumerate(x):
            walk(v, f"{path}[{i}]")


walk(rep, "")
# Run records the report holds more than once (a TCR run sits in tcr[] and may be echoed) count once.
by_folder = defaultdict(list)
for p, r in runs:
    by_folder[r["folder"].replace("\\", "/")].append(p)
print(f"\nrun records: {len(runs)} in the report, {len(by_folder)} distinct run folders")
where = Counter(p.split("[")[0].split(".")[0] for p, _ in runs)
print(f"  by section: {dict(where)}")

status = Counter(r["status"] for _, r in runs)
bound = Counter(r.get("bound_by") for _, r in runs)
print(f"  status: {dict(status)}")
print(f"  bound_by: {dict(bound)}")

# Each run held to its seal entry, independently of the Rust.
bed_prefix = seal["sealed_from"].replace("\\", "/").rstrip("/") + "/"
seal_by_rundir = {}
for key, entry in seal["runs"].items():
    seal_by_rundir[key] = entry
problems = []
on_disk_null = 0
mesh_check_nonempty = []
for folder, paths in sorted(by_folder.items()):
    r = dict(runs)[paths[0]] if False else next(rr for pp, rr in runs if pp == paths[0])
    if not folder.lower().startswith(bed_prefix.lower()):
        problems.append(f"{folder}: not under the sealed bed {bed_prefix}")
        continue
    rel = folder[len(bed_prefix):]                      # runs/<cell>/s<seed>/<run folder>
    parts = rel.split("/")
    rundir, run_folder = "/".join(parts[:3]), parts[3]
    entry = seal_by_rundir.get(rundir)
    if entry is None:
        problems.append(f"{rundir}: no seal entry")
        continue
    if entry["run_folder"] != run_folder:
        problems.append(f"{rundir}: seal names {entry['run_folder']}, report {run_folder}")
    files = {p: (s, h) for p, s, h in entry["files"]}
    od = r.get("on_disk")
    if od is None:
        on_disk_null += 1
        continue
    for k in ("project_sha256", "config_sha256", "scene_sha256", "mbin_sha256", "run_mesh_input_hash",
              "run_mbin_sha256", "mesh_json_input_hash", "mesh_json_mbin_sha256", "mesh_check"):
        if od.get(k) is None:
            problems.append(f"{rundir}: on_disk.{k} is null")
    if od.get("mesh_check"):
        mesh_check_nonempty.append((rundir, od["mesh_check"]))
    # The seal's own hashes of the files on_disk hashed (config.xml aside: it is hashed without its
    # workingdirectory, so it cannot equal the file's sha256).
    want = {
        "project_sha256": files.get("project.simpa", (None, None))[1],
        "scene_sha256": files.get(f"{run_folder}/solve/mesh.cbin", (None, None))[1],
        "mbin_sha256": files.get(f"{run_folder}/solve/tetramesh.mbin", (None, None))[1],
    }
    for k, v in want.items():
        if od.get(k) != v:
            problems.append(f"{rundir}: on_disk.{k} {od.get(k)} but the seal's file {v}")
    if not (od.get("mbin_sha256") == od.get("run_mbin_sha256") == od.get("mesh_json_mbin_sha256")):
        problems.append(f"{rundir}: the .mbin is not one file in all three places")
    if od.get("run_mesh_input_hash") != od.get("mesh_json_input_hash"):
        problems.append(f"{rundir}: the mesh stamps differ")
    if r.get("project_sha256") != od.get("project_sha256"):
        problems.append(f"{rundir}: run.json's project sha256 {r.get('project_sha256')} is not project.simpa's "
                        f"{od.get('project_sha256')}")
    if not r.get("outputs_sha256"):
        problems.append(f"{rundir}: no outputs_sha256")
print(f"  on_disk null: {on_disk_null}; mesh checks with a reason: {len(mesh_check_nonempty)}")
for x in mesh_check_nonempty[:10]:
    print("   ", x)
print(f"  seal entries not in the report: {len(set(seal_by_rundir) - {'/'.join(f[len(bed_prefix):].split('/')[:3]) for f in by_folder})}")
print(f"  problems against the seal and within on_disk: {len(problems)}")
for p in problems[:30]:
    print("   ", p)

# The seed rule, from the report: no two seeds of a group with the same outputs_sha256.
groups = defaultdict(list)
for c in rep["cells"]:
    for s in c["seeds"]:
        if s.get("run"):
            groups[c["id"]].append((s["seed"], s["run"].get("outputs_sha256")))
    ext = c.get("extension")
    if isinstance(ext, dict):
        for s in ext.get("seeds", []) or []:
            if s.get("run"):
                groups[c["id"]].append((s["seed"], s["run"].get("outputs_sha256")))
atm = rep.get("atmospheric_validation") or {}
for s in atm.get("runs", []) or []:
    run = s.get("run") if isinstance(s, dict) else None
    if run:
        groups["atmospheric"].append((s.get("seed"), run.get("outputs_sha256")))
dupes = []
for g, lst in groups.items():
    cnt = Counter(h for _, h in lst)
    for h, n in cnt.items():
        if n > 1:
            dupes.append((g, h, n))
all_out = [r.get("outputs_sha256") for _, r in runs]
print(f"\nseed groups: {len(groups)}, seeds {sum(len(v) for v in groups.values())}; groups with a repeated "
      f"outputs_sha256: {len(dupes)}")
print(f"outputs_sha256 over all run records: {len(all_out)}, distinct {len(set(all_out))}")

# Refusal codes anywhere.
text = json.dumps(rep)
for code in ("bed_run_unbound", "bed_run_files_changed", "bed_seed_outputs_identical", "bed_run_not_planned"):
    print(f"'{code}' in the report: {text.count(code)}")
