"""The queue runner for the M8b EDT held-out test's 172 runs (HARNESS-PLAN.md P10-P17, `driver.plan()`):
builds and meshes the seven F-room projects once, then runs every planned run through `driver.launch`
-- never around it, its guards are the point -- N at a time, each in its own folder. Resumable, STOP-file
aware, progress logged. It never reads or scores a result (`driver.read_run`, `score.py`): that is a
later step, and P14's backlog-54 gap means a CLI run's `solver_build` does not reliably read 'verified'
for every build yet, so resumability here is decided from `launch.json` and `report.json` alone, never
by calling the reader's guard.

Where things are written, and why they differ:
- The run folders (one per planned run, `launch()`'s own `project.simpa`, `launch.json`, `report.json`,
  `simpa-run.std*`) go under `--data-root`, `B:\\data\\m8b-edt\\heldout` by default: the task names this
  location because C: does not hold the 172 runs plus the 8 GB floor `launch()` itself still checks.
- The nine F-room projects and their meshes are NOT written under `--data-root`: `rooms.write_projects`
  (`m8b/rooms.py`, frozen under this addendum) refuses a B: output directory outright ("the harness
  writes only on C:"), and a room project is exactly the small-file workload exFAT's 128 KB clusters
  punish (the project's own "exFAT temp-file leak" lesson). They go under `--rooms-root`,
  `C:\\tmp\\m8b-edt\\heldout-rooms` by default, built once and reused by every run via `--rooms-root`'s
  recorded `rooms.json`.
- Progress lines go to `--progress-log`, `C:\\tmp\\m8b-edt\\progress.log` by default, one line per run
  start/finish/fail plus a summary every 5 minutes (section 6's 15-minute check-in reads the same file).

Concurrency: `--workers` runs are active at a time, each through its own call to `driver.launch`, which
itself spawns `simpa run` as its own OS process -- the process that does the actual solving. The runner's
own scheduling uses a thread pool, not a process pool: `driver.launch` blocks on `subprocess.run` for the
whole run, which releases the GIL, so a thread gets the same real parallelism a process would for this
workload, while keeping resumability, the STOP file and progress logging in one process with no IPC.

Resumable: before a run is launched, its folder (`<data_root>/<run_id>`) is read back. A folder whose
`launch.json` records `run_exit == 0` and `results_exit == 0`, with a `report.json` that parses, is
skipped. Any other non-empty folder -- a prior run that was killed mid-write -- is moved aside to
`<run_id>.partial-<timestamp>` (never deleted) and the run is relaunched fresh.

STOP file: `--data-root/STOP` (or `--stop-file`). Checked before every new submission; once it exists, no
new run starts, and the runner exits after the ones already running finish.

    python -m m8b.run_heldout [--workers 8] [--data-root <dir>] [--rooms-root <dir>] [--solvers <dir>]
                               [--simpa <exe>] [--launch-log <file>] [--progress-log <file>]
                               [--stop-file <file>] [--dry-run]
"""
import argparse
import concurrent.futures
import datetime
import json
import os
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

from . import driver, rooms

SCRATCH = driver.SCRATCH                                         # C:\tmp\m8b-edt
DEFAULT_DATA_ROOT = Path(r'B:\data\m8b-edt\heldout')
DEFAULT_ROOMS_ROOT = SCRATCH / 'heldout-rooms'
DEFAULT_SOLVERS = Path(r'C:\tmp\nm-target\target\solvers\bin')
DEFAULT_PROGRESS_LOG = SCRATCH / 'progress.log'

GROUPS = ('truth', 'tested-random', 'tested-energetic')          # P13: execution order, truth first
SUMMARY_EVERY_S = 300


# ---- the plan, reordered and the room projects ----------------------------------------------------------
def ordered_runs():
    """driver.plan()'s 172 runs (28 truth, 72 tested Random, 72 tested Energetic), called once,
    reordered so all 28 truth runs come first, then tested Random, then tested Energetic. Within each
    group the relative order is plan()'s own (F1-F7, P12's steps and seeds)."""
    all_runs = driver.plan()
    truth = [r for r in all_runs if r['kind'] == 'truth']
    random_runs = [r for r in all_runs if r['kind'] == 'tested' and r['mode'] == 'random']
    energetic_runs = [r for r in all_runs if r['kind'] == 'tested' and r['mode'] == 'energetic']
    ordered = truth + random_runs + energetic_runs
    assert len(ordered) == len(all_runs), 'a run was dropped reordering the plan'
    return ordered


def group_of(run):
    if run['kind'] == 'truth':
        return 'truth'
    return 'tested-random' if run['mode'] == 'random' else 'tested-energetic'


def project_paths(rooms_root):
    """{room name: project path} for the seven F-rooms, under rooms_root/projects -- the path
    ensure_rooms writes to, computed here without touching disk (dry-run's use)."""
    return {name: Path(rooms_root) / 'projects' / ('%s.simpa' % name) for name in rooms.NAMES}


def ensure_rooms(rooms_root, simpa_exe, solvers_dir):
    """Builds and meshes the seven F-room projects once (rooms.write_projects, rooms.mesh_projects),
    under rooms_root (never under data_root -- see the module docstring). Idempotent across restarts:
    if rooms_root/rooms.json already records mesh_exit 0 and verify_exit 0 for every room, nothing is
    rebuilt. Raises RuntimeError on any solver-check, mesh or verify failure. Returns {name: path}."""
    rooms_root = Path(rooms_root)
    record_path = rooms_root / 'rooms.json'
    if record_path.is_file():
        try:
            rec = json.loads(record_path.read_text(encoding='utf-8'))
            mesh = rec.get('mesh', {})
            if all(mesh.get(n, {}).get('mesh_exit') == 0 and mesh.get(n, {}).get('verify_exit') == 0
                   for n in rooms.NAMES):
                return {n: Path(rec['projects'][n]) for n in rooms.NAMES}
        except (ValueError, KeyError, OSError):
            pass                                            # a damaged record is rebuilt, not trusted
    check = driver.check_solvers(solvers_dir)
    if not check['verified']:
        raise RuntimeError('solvers at %s are not verified: %s'
                           % (solvers_dir, [c['name'] for c in check['checks'] if not c['matches']]))
    written = rooms.write_projects(rooms_root / 'projects', simpa_exe, names=list(rooms.NAMES))
    mesh = rooms.mesh_projects(written, rooms_root / 'mesh', simpa_exe, solvers_dir)
    bad = [n for n, m in mesh.items() if m['mesh_exit'] != 0 or m['verify_exit'] != 0]
    if bad:
        raise RuntimeError('mesh or mesh-verify failed for %s: %s' % (bad, {n: mesh[n] for n in bad}))
    rooms_root.mkdir(parents=True, exist_ok=True)
    record_path.write_text(json.dumps(dict(projects={n: str(p) for n, p in written.items()}, mesh=mesh),
                                      indent=1, sort_keys=True) + '\n', encoding='utf-8', newline='\n')
    return written


# ---- progress ---------------------------------------------------------------------------------------
def append_progress(path, text, lock):
    """One 'HH:mm <text>' line, appended under lock (several threads share this file)."""
    line = '%s %s\n' % (datetime.datetime.now().strftime('%H:%M'), text)
    with lock:
        Path(path).parent.mkdir(parents=True, exist_ok=True)
        with open(path, 'a', encoding='utf-8', newline='\n') as f:
            f.write(line)
    return line


def _free_gb(drive):
    try:
        return shutil.disk_usage(drive).free / 1024.0 ** 3
    except OSError:
        return None


def _cpu_performance_pct(timeout=5):
    """\\Processor Information(_Total)\\% Processor Performance: current CPU speed as a percentage of
    its base speed, the throttling proxy section 6's summary line asks for. None if it cannot be read
    (a non-Windows test box, PowerShell unavailable, a timeout): never fatal to the runner."""
    try:
        r = subprocess.run(['powershell', '-NoProfile', '-Command',
                           "(Get-Counter '\\Processor Information(_Total)\\% Processor Performance')"
                           ".CounterSamples.CookedValue"],
                           capture_output=True, text=True, timeout=timeout)
        return float(r.stdout.strip())
    except (OSError, subprocess.SubprocessError, ValueError):
        return None


def write_summary(progress_log, lock, counters, t_start, data_root):
    elapsed = time.monotonic() - t_start
    parts = ['%s %d/%d (failed %d)' % (g, counters[g]['done'] + counters[g]['skipped'], counters[g]['total'],
                                       counters[g]['failed']) for g in GROUPS]
    cpu = _cpu_performance_pct()
    b_free = _free_gb(driver._drive_root(data_root))
    c_free = _free_gb('C:\\')
    text = ('runner summary: %s | elapsed %ds | CPU perf %s%% of base | %s %s GB free | C: %s GB free'
           % (', '.join(parts), round(elapsed), ('%.0f' % cpu) if cpu is not None else 'n/a',
              driver._drive_root(data_root), ('%.1f' % b_free) if b_free is not None else 'n/a',
              ('%.1f' % c_free) if c_free is not None else 'n/a'))
    append_progress(progress_log, text, lock)


# ---- resumability -------------------------------------------------------------------------------------
def run_status(run_dir):
    """'missing' (no folder), 'complete' (launch.json records run_exit 0 and results_exit 0, and
    report.json parses) or 'partial' (anything else a folder holds). Never calls driver.read_run: its
    solver_build_unverified guard is a later step's (P14, backlog 54), not this runner's to apply."""
    run_dir = Path(run_dir)
    if not run_dir.exists():
        return 'missing'
    launch_json = run_dir / 'launch.json'
    if not launch_json.is_file():
        return 'partial'
    try:
        rec = json.loads(launch_json.read_text(encoding='utf-8'))
    except (ValueError, OSError):
        return 'partial'
    report = run_dir / 'report.json'
    if rec.get('run_exit') == 0 and rec.get('results_exit') == 0 and report.is_file():
        try:
            json.loads(report.read_text(encoding='utf-8'))
        except (ValueError, OSError):
            return 'partial'
        return 'complete'
    return 'partial'


def move_aside_partial(run_dir):
    """Renames a half-written run folder to <name>.partial-<timestamp>, never deletes it."""
    run_dir = Path(run_dir)
    stamp = datetime.datetime.now().strftime('%Y%m%d-%H%M%S-%f')
    dest = run_dir.parent / ('%s.partial-%s' % (run_dir.name, stamp))
    shutil.move(str(run_dir), str(dest))
    return dest


# ---- one run ------------------------------------------------------------------------------------------
def do_run(run, *, project, data_root, solvers_dir, launch_log, simpa_exe, progress_log, lock):
    """Resume/skip, then driver.launch -- always, never around it. Returns {'run_id', 'outcome'
    ('skip', 'done' or 'failed'), ...}. Any exception from driver.launch is caught and reported as a
    failed run; it never kills the queue."""
    run_dir = Path(data_root) / run['run_id']
    status = run_status(run_dir)
    if status == 'complete':
        append_progress(progress_log, 'runs: %s skip (already complete)' % run['run_id'], lock)
        return dict(run_id=run['run_id'], outcome='skip')
    if status == 'partial':
        dest = move_aside_partial(run_dir)
        append_progress(progress_log, 'runs: %s partial moved to %s' % (run['run_id'], dest.name), lock)

    append_progress(progress_log, 'runs: %s started' % run['run_id'], lock)
    t0 = time.monotonic()
    try:
        rec = driver.launch(run, project=project, run_dir=run_dir, solvers_dir=solvers_dir, data_root=data_root,
                            launch_log=launch_log, simpa_exe=simpa_exe)
    except driver.Refused as e:
        append_progress(progress_log, 'runs: %s FAILED %s' % (run['run_id'], e), lock)
        return dict(run_id=run['run_id'], outcome='failed', reason=str(e))
    except Exception as e:                                        # a crash here must not kill the queue
        append_progress(progress_log, 'runs: %s FAILED unexpected %r' % (run['run_id'], e), lock)
        return dict(run_id=run['run_id'], outcome='failed', reason=repr(e))
    elapsed = time.monotonic() - t0
    if rec.get('run_exit') == 0 and rec.get('results_exit') == 0:
        append_progress(progress_log, 'runs: %s done in %ds' % (run['run_id'], round(elapsed)), lock)
        return dict(run_id=run['run_id'], outcome='done', elapsed_s=elapsed)
    reason = 'run_exit %r results_exit %r' % (rec.get('run_exit'), rec.get('results_exit'))
    append_progress(progress_log, 'runs: %s FAILED %s' % (run['run_id'], reason), lock)
    return dict(run_id=run['run_id'], outcome='failed', reason=reason)


# ---- the queue ----------------------------------------------------------------------------------------
def run_queue(runs, *, projects, data_root, solvers_dir, launch_log, simpa_exe, progress_log, workers, stop_file):
    """Runs every entry of `runs` through do_run, `workers` active at a time, each its own thread (each
    of which blocks in driver.launch's own subprocess -- the run's own process). Stops submitting once
    stop_file exists; waits out the runs already active, then returns every result."""
    lock = threading.Lock()
    counters = {g: dict(total=0, done=0, skipped=0, failed=0) for g in GROUPS}
    for r in runs:
        counters[group_of(r)]['total'] += 1
    t_start = time.monotonic()
    last_summary = t_start
    pending = list(runs)
    active = {}
    results = []
    workers = max(1, int(workers))
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as ex:
        def submit_next():
            while pending and len(active) < workers and not Path(stop_file).exists():
                run = pending.pop(0)
                fut = ex.submit(do_run, run, project=projects[run['room']], data_root=data_root,
                                solvers_dir=solvers_dir, launch_log=launch_log, simpa_exe=simpa_exe,
                                progress_log=progress_log, lock=lock)
                active[fut] = run

        submit_next()
        while active:
            done_now, _ = concurrent.futures.wait(list(active), timeout=5,
                                                   return_when=concurrent.futures.FIRST_COMPLETED)
            for fut in done_now:
                run = active.pop(fut)
                res = fut.result()
                results.append(res)
                g = group_of(run)
                counters[g]['done' if res['outcome'] == 'done' else
                           'skipped' if res['outcome'] == 'skip' else 'failed'] += 1
            submit_next()
            now = time.monotonic()
            if now - last_summary >= SUMMARY_EVERY_S:
                write_summary(progress_log, lock, counters, t_start, data_root)
                last_summary = now
        if pending and Path(stop_file).exists():
            append_progress(progress_log, 'runner: STOP file present, %d run(s) not started' % len(pending), lock)
    write_summary(progress_log, lock, counters, t_start, data_root)
    return results


# ---- dry run --------------------------------------------------------------------------------------------
def print_dry_run(runs, projects, data_root, rooms_root, out=sys.stdout):
    truth = [r for r in runs if r['kind'] == 'truth']
    tested = [r for r in runs if r['kind'] == 'tested']
    random_n = sum(1 for r in tested if r['mode'] == 'random')
    energetic_n = sum(1 for r in tested if r['mode'] == 'energetic')
    print('M8b held-out queue runner -- dry run (nothing launched, nothing built, nothing read)', file=out)
    print('data_root: %s' % data_root, file=out)
    print('rooms_root: %s' % rooms_root, file=out)
    print(file=out)
    print('Projects (%d), built once under rooms_root (never under data_root: rooms.py refuses B:):'
         % len(projects), file=out)
    for name in rooms.NAMES:
        print('  %s -> %s' % (name, projects[name]), file=out)
    print(file=out)
    print('Runs (%d): %d truth, %d tested (%d random, %d energetic)'
         % (len(runs), len(truth), len(tested), random_n, energetic_n), file=out)
    print('Order (all truth runs, then tested Random, then tested Energetic):', file=out)
    for i, r in enumerate(runs, 1):
        folder = Path(data_root) / r['run_id']
        print('  %4d %-6s %-5s %-9s %-2s %-45s -> %s'
             % (i, r['kind'], r['mode'], group_of(r), r['room'], r['run_id'], folder), file=out)
    print(file=out)
    print('first run: %s' % runs[0]['run_id'], file=out)
    print('last run:  %s' % runs[-1]['run_id'], file=out)


# ---- main -----------------------------------------------------------------------------------------------
def main(argv=None):
    ap = argparse.ArgumentParser(description='Queue runner for the M8b EDT held-out test\'s 172 runs: '
                                 'builds and meshes the F-rooms once, then runs the matrix through driver.launch.')
    ap.add_argument('--workers', type=int, default=8)
    ap.add_argument('--data-root', type=Path, default=DEFAULT_DATA_ROOT)
    ap.add_argument('--rooms-root', type=Path, default=DEFAULT_ROOMS_ROOT)
    ap.add_argument('--solvers', type=Path, default=DEFAULT_SOLVERS)
    ap.add_argument('--simpa', type=Path, default=Path(os.environ.get('M8B_SIMPA_EXE', driver.DEFAULT_SIMPA)))
    ap.add_argument('--launch-log', type=Path, default=driver.LAUNCH_LOG)
    ap.add_argument('--progress-log', type=Path, default=DEFAULT_PROGRESS_LOG)
    ap.add_argument('--stop-file', type=Path, default=None)
    ap.add_argument('--dry-run', action='store_true')
    a = ap.parse_args(argv)
    stop_file = a.stop_file or (a.data_root / 'STOP')
    projects = project_paths(a.rooms_root)

    runs = ordered_runs()
    if a.dry_run:
        print_dry_run(runs, projects, a.data_root, a.rooms_root)
        return 0

    lock = threading.Lock()
    check = driver.check_solvers(a.solvers)
    append_progress(a.progress_log, 'runner: solver check at %s: %s' % (a.solvers,
                    'verified' if check['verified'] else 'FAILED'), lock)
    if not check['verified']:
        print('solvers at %s are not verified: %s'
             % (a.solvers, [c['name'] for c in check['checks'] if not c['matches']]), file=sys.stderr)
        return 1

    append_progress(a.progress_log, 'runner: building and meshing the F-room projects under %s' % a.rooms_root, lock)
    projects = ensure_rooms(a.rooms_root, a.simpa, a.solvers)
    append_progress(a.progress_log, 'runner: rooms ready, starting %d runs with %d workers' % (len(runs), a.workers), lock)

    results = run_queue(runs, projects=projects, data_root=a.data_root, solvers_dir=a.solvers,
                        launch_log=a.launch_log, simpa_exe=a.simpa, progress_log=a.progress_log,
                        workers=a.workers, stop_file=stop_file)
    failed = [r for r in results if r['outcome'] == 'failed']
    append_progress(a.progress_log, 'runner: queue finished, %d of %d runs failed' % (len(failed), len(results)), lock)
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
