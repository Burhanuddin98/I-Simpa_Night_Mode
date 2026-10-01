"""SPPS runs (HARNESS-PLAN.md P8-P17, sections 3 and 7): the matrix and its plan-only mode, the
guards, the solver check, the report reader and the probe mode.

The run path, its guards, the solver check, the reader and the probe mode were built in section 6,
step 4 (T13, T14). plan() (section 6, step 6; T12) lists the matrix, with P10 fixed at 10 s for now
(decision row 36) and P11's two modes; it opens no file and launches nothing. Nothing here may
launch a run of the test before ADDENDUM-A1.md is committed.

Contract:
- Refused(RuntimeError) carries `code`, one of: 'reserved_seed', 'heldout_before_a1',
  'outside_data_root', 'solver_unverified', 'solver_build_unverified', 'unknown_room',
  'low_disk_space'; and, for the probe mode only, 'not_a_probe' (a probe name, room or folder that
  is not section 7's).
- plan() -> [run], opening no file: 144 tested runs (P13: 7 rooms x 3 steps x 3 seeds at 150k
  plus F6 x 3 x 3 at 50k, in each of P11's two modes) and 28 truth runs (7 rooms x K = 4, Random,
  1,000,000 particles, 0.1 ms, P17's length). A run is a dict with 'run_id', 'room', 'kind'
  ('tested' or 'truth'), 'mode' ('random' or 'energetic'), 'time_step_s', 'particles_per_source',
  'random_seed' (P12) and 'duration_s' (P10 for tested runs: the room project's own; P17 for truth).
- a1_committed(repo=None) -> bool: ADDENDUM-A1.md is committed on the branch's HEAD.
- heldout_seed(seed) -> bool: P12's tested and truth seeds, Synth-fresh's 2026100101 and
  ISM-fresh's 2026100102.
- require_not_heldout(seed, a1=None): Refused('heldout_before_a1') for a held-out seed while A1 is
  not committed (a1 None: ask a1_committed()). Synth-fresh and ISM-fresh call it too.
- check_solvers(solvers_dir, manifest=None) -> {'verified': bool, 'checks': [{'name', 'path',
  'code_sha256', 'expected', 'matches'}]}: spps.exe, classicalTheory.exe, tetgen.exe and
  preprocess.exe by code sha256 (tools/fixture-gen/pe_fingerprint.py) against
  solvers/manifest.json, as M8a's E1 does (tools/gates/m8a.ps1:8-9).
- launch(run, *, project, run_dir, solvers_dir, data_root=DATA_ROOT, launch_log=LAUNCH_LOG,
  simpa_exe=None, a1=None): the guards first, in this order, each raising Refused before anything
  is written or launched: the seed ('reserved_seed' for RESERVED_SEEDS, then require_not_heldout),
  the room ('unknown_room' unless run['room'] is one of the explicit allowlist ROOM_ALLOWLIST,
  i.e. rooms.NAMES (the F-rooms) plus rooms.PROBES (P0, P0b) -- launch() never runs anything else),
  the folder ('outside_data_root' unless run_dir lies inside data_root), the solvers
  ('solver_unverified' unless check_solvers verifies them), the freeze (a run of a held-out room,
  F1-F7, is refused before A1 whatever its seed, as 'heldout_before_a1', so that no histogram of a
  held-out room exists before the freeze; P0 and P0b are not held out), and free disk space
  ('low_disk_space' unless both the drive holding run_dir and C: have at least MIN_FREE_BYTES free,
  section 5 item 5). Then <run_dir>/project.simpa is written, one line appended to launch_log,
  `simpa run <project> --solver spps --runs <run_dir> --json` run with its stderr kept and classed,
  and <run_dir>/report.json written from `simpa results <run folder> --json`.
- read_run(run_dir, *, data_root=DATA_ROOT) -> series_from_report(<run_dir>/report.json), after
  refusing a folder outside data_root ('outside_data_root'), a run whose <run_dir>/project.simpa
  has a reserved random_seed ('reserved_seed'), and a report whose solver_build status is not
  'verified' ('solver_build_unverified'; P14, backlog 38, results/report.rs:899-901).
- series_from_report(report) -> [{'label', 'band_hz', 'energy_pa2' (float64 array), 'arrival_s',
  'half_width' (receiver_radius_m / speed_of_sound_m_s), 'dt' (time_step_s)}], one per point
  receiver and band: the old real loader's field access (the scratchpad's edtsimp/loaders.py:159-181,
  sha256 ea4d4c09...).
- probe(...): section 7's probe mode. It records PROBE.md's numbers and opens no output file.

P14 and the CLI (HARNESS-PLAN.md 8.1): `simpa run` records no solver check in run.json
(crates/simpa/src/mesh_run.rs:520 sets `verify: None`), and results::solver_build reads such a run
'solver_build_unrecorded' (crates/simpa-core/src/results.rs:467-514). The guard stays as written:
the truth and tested runs wait for backlog 54's CLI half. The probes run no `simpa results`, so only
check_solvers before the run applies to them.

What launch() leaves in run_dir: project.simpa (the room's project with the run's method, step,
length, particles and seed), launch.json (the record: the solver check, the commands, exit codes,
wall time, stderr by class), simpa-run.stdout.json (the run manifest `simpa run --json` prints, the
run folder's run.json), simpa-run.stderr.txt, the run folder <stamp>-spps/ that `simpa run` makes,
and report.json with simpa-results.stderr.txt when `simpa results` exits 0. Each run is given the
checked folder's executables by path (--solver-exe, --tetgen, --preprocess), so the run uses the
files the check read.

The probe mode (section 7, 7.1): probe(name, project=<the room's project>, ...) for a name in
PROBE_RUNS. It refuses ('not_a_probe') a name it does not know, a project that is not that probe's
room (its name and its geometry's bounding box), and a probe folder that overlaps the data root;
it refuses an unverified solver folder ('solver_unverified'). It writes <probe_root>/projects/
<name>-probe.simpa (<name>-probe-2.simpa on a second attempt, and so on), appends one line to the
launch log, runs `simpa run <it> --solver spps --runs <probe_root> --json` with the checked
executables, and polls the process tree for spps.exe's (and tetgen.exe's) peak working set, read once
more through the open handle after the process ends. It records the wall time, outcome.elapsed_ms,
the verdict and the manifest's line counts (from simpa's stdout, which is run.json's text), the
output size by a stat of every file in the run folder (no file in it is opened), stderr's lines by
class (stderr is kept beside the run folder), and the exe hashes, in <probe_root>/probes.jsonl. It
never runs `simpa results`. Seeds 9999 (P0) and 9998 (P0b) are reserved: launch() and read_run()
refuse them.

    python -m m8b.driver probe P0 [P0b-random-2s ...] [--simpa <exe>] [--solvers <dir>]
                               [--probe-root <dir>] [--launch-log <file>]
    python -m m8b.driver check-solvers [--solvers <dir>]
The probe command writes P0's and P0b's room projects under <probe-root>/rooms first, then runs the
named probes one after another and prints one line for each.
"""
import ctypes
import datetime
import hashlib
import importlib.util
import json
import math
import operator
import os
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

import numpy as np

from . import corpus, rooms

SCRATCH = Path(r'C:\tmp\m8b-edt')
DATA_ROOT = SCRATCH / 'heldout'
PROBE_ROOT = SCRATCH / 'probe'
LAUNCH_LOG = SCRATCH / 'launch.log'
RESERVED_SEEDS = (9998, 9999)

DEFAULT_SIMPA = Path(r'C:\tmp\nm-target\release\simpa.exe')
DEFAULT_SOLVERS = Path(r'C:\tmp\nm-m8a-solvers')
MANIFEST = corpus.REPO / 'solvers' / 'manifest.json'
PE_FINGERPRINT = corpus.REPO / 'tools' / 'fixture-gen' / 'pe_fingerprint.py'
ADDENDUM = 'docs/investigations/2026-09-27-edt-heldout/ADDENDUM-A1.md'     # relative to the repo
EXES = ('spps.exe', 'classicalTheory.exe', 'tetgen.exe', 'preprocess.exe')

# P12: the tested runs' seeds by (step ms, particles), the truth's, and the generators' (P21, P24)
TESTED_SEEDS = {(1.0, 150000): (1101, 1102, 1103), (2.0, 150000): (1201, 1202, 1203),
                (5.0, 150000): (1501, 1502, 1503), (1.0, 50000): (2101, 2102, 2103),
                (2.0, 50000): (2201, 2202, 2203), (5.0, 50000): (2501, 2502, 2503)}
TRUTH_SEEDS = (9001, 9002, 9003, 9004)
SYNTH_SEED, ISM_SEED = 2026100101, 2026100102
HELDOUT_SEEDS = frozenset([s for seeds in TESTED_SEEDS.values() for s in seeds] + list(TRUTH_SEEDS)
                          + [SYNTH_SEED, ISM_SEED])
MODES = ('random', 'energetic')
LINE_CLASSES = ('PROGRESS', 'INFO', 'OK', 'WARN', 'FAIL')

TESTED_DURATION_S = 10.0          # P10, decision row 36: fixed for now; the room-based rule is backlog 57
TRUTH_TIME_STEP_S = 1e-4          # P16
TRUTH_PARTICLES = 1_000_000       # P16

ROOM_ALLOWLIST = frozenset(rooms.NAMES) | frozenset(rooms.PROBES)   # the only rooms launch() ever runs
MIN_FREE_BYTES = 8 * 1024 ** 3    # section 5 item 5: the run folder's drive and C: both keep >= 8 GB free

# Sections 7 and 7.1: the probes, each in its stand-in room with its reserved seed
PROBE_RUNS = {
    'P0': dict(room='P0', mode='random', particles_per_source=1_000_000, time_step_s=1e-4, duration_s=6.5,
               random_seed=9999),
    'P0b-random-2s': dict(room='P0b', mode='random', particles_per_source=150_000, time_step_s=1e-3,
                          duration_s=2.0, random_seed=9998),
    'P0b-random-10s': dict(room='P0b', mode='random', particles_per_source=150_000, time_step_s=1e-3,
                           duration_s=10.0, random_seed=9998),
    'P0b-energetic-2s': dict(room='P0b', mode='energetic', particles_per_source=150_000, time_step_s=1e-3,
                             duration_s=2.0, random_seed=9998),
    'P0b-energetic-10s': dict(room='P0b', mode='energetic', particles_per_source=150_000, time_step_s=1e-3,
                              duration_s=10.0, random_seed=9998),
}


class Refused(RuntimeError):
    """A guard refused: nothing was written or launched."""

    def __init__(self, code, detail=''):
        super().__init__('%s: %s' % (code, detail))
        self.code = code


def truth_duration_s(room):
    """P17: min(6.5, max(3.0, t_arr,max + 2 x the design T60 maximum)), rounded up to 0.1 s (1e-9
    allowed). t_arr,max is the room's farthest receiver's straight-line distance / C."""
    t_arr = max(r['d_m'] for r in room['receivers']) / corpus.C_SPPS
    t60 = max(float(v) for v in room['design_t60_s'].values())
    x = min(6.5, max(3.0, t_arr + 2.0 * t60))
    return math.ceil(x * 10 - 1e-9) / 10


def plan():
    """P12, P13: the 144 tested runs (72 per mode: every room x 3 steps x 3 seeds at 150k, plus F6's
    extra 3 x 3 at 50k) and the 28 truth runs (every room x P12's 4 truth seeds), opening no file."""
    R = rooms.rooms()
    runs = []
    for mode in MODES:
        for room_name in rooms.NAMES:
            for (step_ms, particles), seeds in TESTED_SEEDS.items():
                if particles != 150000 and not (particles == 50000 and room_name == 'F6'):
                    continue
                for seed in seeds:
                    runs.append(dict(
                        run_id='tested-%s-%s-%sms-%dk-%d' % (room_name, mode, step_ms, particles // 1000, seed),
                        room=room_name, kind='tested', mode=mode, time_step_s=step_ms / 1.0e3,
                        particles_per_source=particles, random_seed=seed, duration_s=TESTED_DURATION_S))
    for room_name in rooms.NAMES:
        duration = truth_duration_s(R[room_name])
        for seed in TRUTH_SEEDS:
            runs.append(dict(
                run_id='truth-%s-%d' % (room_name, seed),
                room=room_name, kind='truth', mode='random', time_step_s=TRUTH_TIME_STEP_S,
                particles_per_source=TRUTH_PARTICLES, random_seed=seed, duration_s=duration))
    return runs


# ---- guards -------------------------------------------------------------------------------------------
def a1_committed(repo=None):
    """True only when HEAD holds ADDENDUM-A1.md and the working copy of it is HEAD's (git diff --quiet).
    Anything else, git failing included, is False: the held-out seeds stay refused."""
    repo = Path(repo) if repo else corpus.REPO
    try:
        r = subprocess.run(['git', '-C', str(repo), 'cat-file', '-e', 'HEAD:' + ADDENDUM], capture_output=True,
                           timeout=120)
        if r.returncode != 0:
            return False
        d = subprocess.run(['git', '-C', str(repo), 'diff', '--quiet', 'HEAD', '--', ADDENDUM], capture_output=True,
                           timeout=120)
        return d.returncode == 0 and (repo / ADDENDUM).is_file()
    except (OSError, subprocess.SubprocessError):
        return False


def heldout_seed(seed):
    return operator.index(seed) in HELDOUT_SEEDS


def require_not_heldout(seed, a1=None):
    if heldout_seed(seed):
        if a1 is None:
            a1 = a1_committed()
        if not a1:
            raise Refused('heldout_before_a1', 'seed %d is held out (P12, P21, P24) and ADDENDUM-A1.md is not '
                                               'committed' % operator.index(seed))


def _norm(p):
    return Path(os.path.normcase(os.path.abspath(Path(p).resolve())))


def inside(path, root):
    """Whether path lies strictly inside root, both resolved (junctions and symlinks followed)."""
    return _norm(root) in _norm(path).parents


def _load_pe_fingerprint():
    name = 'm8b_pe_fingerprint'
    if name not in sys.modules:
        spec = importlib.util.spec_from_file_location(name, PE_FINGERPRINT)
        mod = importlib.util.module_from_spec(spec)
        sys.modules[name] = mod
        spec.loader.exec_module(mod)
    return sys.modules[name]


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for chunk in iter(lambda: f.read(1 << 20), b''):
            h.update(chunk)
    return h.hexdigest()


def check_solvers(solvers_dir, manifest=None):
    """M8a's E1: each of EXES in solvers_dir has the code sha256 solvers/manifest.json lists."""
    pf = _load_pe_fingerprint()
    mpath = Path(manifest) if manifest else MANIFEST
    data = mpath.read_bytes()
    want = json.loads(data.decode('utf-8-sig'))['code_sha256']
    checks = []
    for name in EXES:
        path = Path(solvers_dir) / name
        c = dict(name=name, path=str(path), code_sha256=None, expected=want.get(name), matches=False)
        if path.is_file():
            c['sha256'] = sha256_file(path)
            try:
                c['code_sha256'] = pf.code_sha256(path)
            except pf.NotMeasured as e:
                c['error'] = str(e)
            c['matches'] = c['code_sha256'] is not None and c['code_sha256'] == c['expected']
        else:
            c['error'] = 'missing'
        checks.append(c)
    return {'verified': all(c['matches'] for c in checks), 'dir': str(solvers_dir), 'manifest': str(mpath),
            'manifest_sha256': hashlib.sha256(data).hexdigest(), 'checks': checks}


# ---- a run's project ----------------------------------------------------------------------------------
def run_project(room_project, run):
    """The room's project (its parsed JSON) with the run's settings: SPPS's method, step, length,
    particles per source and seed. Nothing else changes."""
    if run['mode'] not in MODES:
        raise ValueError('mode %r is not one of %s' % (run['mode'], MODES))
    P = json.loads(json.dumps(room_project))
    sp = P['solvers']['spps']
    sp.update(method=run['mode'], time_step_s=float(run['time_step_s']), duration_s=float(run['duration_s']),
              particles_per_source=int(run['particles_per_source']), random_seed=operator.index(run['random_seed']))
    return P


def _read_project(project):
    return json.loads(Path(project).read_text(encoding='utf-8'))


def _now():
    return datetime.datetime.now().astimezone().isoformat(timespec='milliseconds')


def _append_line(log, rec):
    log = Path(log)
    log.parent.mkdir(parents=True, exist_ok=True)
    with open(log, 'a', encoding='utf-8', newline='\n') as f:
        f.write(json.dumps(rec, sort_keys=True) + '\n')


def class_lines(text):
    """stderr's lines by class: simpa prints each solver line as 'CLASS  text' (mesh_run.rs:477-482);
    'simpa' for simpa's own lines, 'other' for the rest. The WARN and FAIL lines are kept whole."""
    counts = {k: 0 for k in LINE_CLASSES + ('simpa', 'other')}
    kept = []
    for line in text.splitlines():
        head = line.split(None, 1)[0] if line.strip() else ''
        if head in LINE_CLASSES:
            counts[head] += 1
            if head in ('WARN', 'FAIL'):
                kept.append(line)
        elif line.startswith('simpa:'):
            counts['simpa'] += 1
            kept.append(line)
        elif line.strip():
            counts['other'] += 1
    return dict(counts=counts, lines=kept[:200], lines_dropped=max(0, len(kept) - 200))


def _exe_args(solvers_dir):
    s = Path(solvers_dir)
    return ['--solver-exe', str(s / 'spps.exe'), '--tetgen', str(s / 'tetgen.exe'), '--preprocess', str(s / 'preprocess.exe')]


def _command(simpa_exe):
    if isinstance(simpa_exe, (list, tuple)):
        return [str(x) for x in simpa_exe]
    return [str(simpa_exe or os.environ.get('M8B_SIMPA_EXE', DEFAULT_SIMPA))]


def _env(solvers_dir):
    env = dict(os.environ)
    env['SIMPA_SOLVERS_DIR'] = str(solvers_dir)
    return env


def _fresh_dir(d):
    d = Path(d)
    if d.exists() and any(d.iterdir()):
        raise FileExistsError('%s exists and is not empty: a run folder is never reused' % d)
    d.mkdir(parents=True, exist_ok=True)
    return d


def _run_folder(manifest_text):
    try:
        m = json.loads(manifest_text)
    except ValueError:
        return None, None
    cwd = m.get('cwd')
    return m, (str(Path(cwd).parent) if cwd else None)


def _drive_root(path):
    """The drive letter of path (resolved, need not exist) as a root path shutil.disk_usage accepts,
    e.g. 'B:\\'. Falls back to 'C:\\' when path carries no drive (a relative path, or a non-Windows
    path in a test)."""
    drive = Path(path).resolve().drive
    return (drive or 'C:') + '\\'


def check_free_space(run_dir, min_free=MIN_FREE_BYTES):
    """Section 5 item 5: the drive that would hold run_dir, and C: (where the solver and Python both
    also write), each need at least min_free bytes free. Returns {'out_root', 'out_free_bytes',
    'c_free_bytes'}. Raises Refused('low_disk_space') and touches nothing on disk otherwise."""
    out_root = _drive_root(run_dir)
    out_free = shutil.disk_usage(out_root).free
    c_free = shutil.disk_usage('C:\\').free
    if out_free < min_free:
        raise Refused('low_disk_space', '%s has %d bytes free, below the %d byte floor' % (out_root, out_free, min_free))
    if c_free < min_free:
        raise Refused('low_disk_space', 'C:\\ has %d bytes free, below the %d byte floor' % (c_free, min_free))
    return dict(out_root=out_root, out_free_bytes=out_free, c_free_bytes=c_free)


def launch(run, *, project, run_dir, solvers_dir, data_root=DATA_ROOT, launch_log=LAUNCH_LOG,
           simpa_exe=None, a1=None):
    """One SPPS run, after the guards (see the module docstring). Returns launch.json's record."""
    seed = operator.index(run['random_seed'])
    if seed in RESERVED_SEEDS:
        raise Refused('reserved_seed', 'seed %d is reserved for the probes (P12, section 7)' % seed)
    if run['room'] not in ROOM_ALLOWLIST:
        raise Refused('unknown_room', '%r is not in the allowlist (%s)' % (run['room'], sorted(ROOM_ALLOWLIST)))
    require_not_heldout(seed, a1)
    if not inside(run_dir, data_root):
        raise Refused('outside_data_root', '%s is not inside %s' % (run_dir, data_root))
    check = check_solvers(solvers_dir)
    if not check['verified']:
        raise Refused('solver_unverified', '%s: %s' % (solvers_dir, [c['name'] for c in check['checks'] if not c['matches']]))
    if run['room'] in rooms.NAMES and not (a1_committed() if a1 is None else a1):
        raise Refused('heldout_before_a1', 'room %s is held out: no run of it, whatever its seed, before ADDENDUM-A1.md '
                                           'is committed (P1, P4)' % run['room'])
    check_free_space(run_dir)

    room_project = _read_project(project)
    if room_project.get('name') != run['room']:
        raise ValueError('%s is the project of %r, not of the run\'s room %r' % (project, room_project.get('name'), run['room']))
    run_dir = _fresh_dir(run_dir)
    proj = run_dir / 'project.simpa'
    proj.write_text(rooms.canonical_json(run_project(room_project, run)), encoding='utf-8', newline='\n')
    cmd = _command(simpa_exe) + ['run', str(proj), '--solver', 'spps', '--runs', str(run_dir), '--json'] + _exe_args(solvers_dir)
    rec = dict(run=run, project=str(proj), project_sha256=sha256_file(proj), room_project=str(project),
               solvers=check, command=cmd, started=_now())
    _append_line(launch_log, dict(at=rec['started'], what='launch', run_id=run['run_id'], room=run['room'],
                                  kind=run.get('kind'), mode=run['mode'], time_step_s=run['time_step_s'],
                                  particles_per_source=run['particles_per_source'], random_seed=seed,
                                  duration_s=run['duration_s'], run_dir=str(run_dir), solvers_verified=True))
    out, err = run_dir / 'simpa-run.stdout.json', run_dir / 'simpa-run.stderr.txt'
    t0 = time.perf_counter()
    with open(out, 'wb') as fo, open(err, 'wb') as fe:
        p = subprocess.run(cmd, stdout=fo, stderr=fe, env=_env(solvers_dir))
    rec['wall_s'] = time.perf_counter() - t0
    rec['run_exit'] = p.returncode
    rec['stderr'] = class_lines(err.read_text(encoding='utf-8', errors='replace'))
    manifest, folder = _run_folder(out.read_text(encoding='utf-8', errors='replace'))
    rec['run_folder'] = folder
    if manifest:
        rec['outcome'] = manifest.get('outcome')
        rec['verdict'] = manifest.get('verdict')
        rec['exit_class'] = manifest.get('exit_class')
    if folder:
        rcmd = _command(simpa_exe) + ['results', folder, '--json']
        r = subprocess.run(rcmd, capture_output=True, env=_env(solvers_dir))
        rec['results_command'] = rcmd
        rec['results_exit'] = r.returncode
        (run_dir / 'simpa-results.stderr.txt').write_bytes(r.stderr)
        if r.returncode == 0:
            (run_dir / 'report.json').write_bytes(r.stdout)
    rec['finished'] = _now()
    (run_dir / 'launch.json').write_text(json.dumps(rec, indent=1, sort_keys=True) + '\n', encoding='utf-8', newline='\n')
    return rec


# ---- the reader ---------------------------------------------------------------------------------------
def read_run(run_dir, *, data_root=DATA_ROOT):
    run_dir = Path(run_dir)
    if not inside(run_dir, data_root):
        raise Refused('outside_data_root', '%s is not inside %s' % (run_dir, data_root))
    seed = _read_project(run_dir / 'project.simpa')['solvers']['spps']['random_seed']
    if seed in RESERVED_SEEDS:
        raise Refused('reserved_seed', '%s ran with the reserved seed %d: a probe, never read' % (run_dir, seed))
    report = json.loads((run_dir / 'report.json').read_text(encoding='utf-8'))
    build = report.get('solver_build') or {}
    if build.get('status') != 'verified':
        raise Refused('solver_build_unverified', '%s: solver_build %s (P14, backlog 38)' % (run_dir, json.dumps(build)))
    return series_from_report(report)


def series_from_report(report):
    d = report['spps']
    h = d['receiver_radius_m'] / d['speed_of_sound_m_s']
    dt = d['time_step_s']
    out = []
    for freq in (b['freq_hz'] for b in d['point_receivers'][0]['bands']):
        for pr in d['point_receivers']:
            bnd = next(b for b in pr['bands'] if b['freq_hz'] == freq)
            out.append(dict(label=pr['label'], band_hz=freq, energy_pa2=np.asarray(bnd['energy_pa2'], dtype=np.float64),
                            arrival_s=pr['arrival_s'], half_width=h, dt=dt))
    return out


# ---- the probe mode (section 7) -----------------------------------------------------------------------
class _Watch:
    """Polls the process tree under one pid for the peak working set and peak private bytes of the
    processes with the watched names (Windows: a Toolhelp snapshot, then GetProcessMemoryInfo on a
    handle kept open, so the counters can be read once more after the process ends). It polls every
    poll_s until a process of the first name appears, then every slow_s: the peaks are the OS's own
    running maxima, so later polls only look for new processes."""

    def __init__(self, root_pid, names, poll_s, slow_s=2.0):
        self.root, self.names, self.poll_s, self.slow_s = root_pid, {n.lower() for n in names}, poll_s, slow_s
        self.primary = names[0].lower()
        self.seen = {}                 # pid -> dict(name, handle, peak_ws, peak_private, reads)
        self.polls = 0
        self.error = None
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._loop, daemon=True)
        self.ok = sys.platform == 'win32'
        if self.ok:
            from ctypes import wintypes
            self.w = wintypes
            self.k32 = ctypes.WinDLL('kernel32', use_last_error=True)

            class PE32(ctypes.Structure):
                _fields_ = [('dwSize', wintypes.DWORD), ('cntUsage', wintypes.DWORD), ('th32ProcessID', wintypes.DWORD),
                            ('th32DefaultHeapID', ctypes.c_size_t), ('th32ModuleID', wintypes.DWORD),
                            ('cntThreads', wintypes.DWORD), ('th32ParentProcessID', wintypes.DWORD),
                            ('pcPriClassBase', ctypes.c_long), ('dwFlags', wintypes.DWORD),
                            ('szExeFile', ctypes.c_wchar * 260)]

            class PMC(ctypes.Structure):
                _fields_ = [('cb', wintypes.DWORD), ('PageFaultCount', wintypes.DWORD),
                            ('PeakWorkingSetSize', ctypes.c_size_t), ('WorkingSetSize', ctypes.c_size_t),
                            ('QuotaPeakPagedPoolUsage', ctypes.c_size_t), ('QuotaPagedPoolUsage', ctypes.c_size_t),
                            ('QuotaPeakNonPagedPoolUsage', ctypes.c_size_t), ('QuotaNonPagedPoolUsage', ctypes.c_size_t),
                            ('PagefileUsage', ctypes.c_size_t), ('PeakPagefileUsage', ctypes.c_size_t)]
            self.PE32, self.PMC = PE32, PMC
            k = self.k32
            k.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
            k.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
            k.Process32FirstW.argtypes = [wintypes.HANDLE, ctypes.POINTER(PE32)]
            k.Process32NextW.argtypes = [wintypes.HANDLE, ctypes.POINTER(PE32)]
            k.OpenProcess.restype = wintypes.HANDLE
            k.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
            k.CloseHandle.argtypes = [wintypes.HANDLE]
            k.K32GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(PMC), wintypes.DWORD]

    def start(self):
        if self.ok:
            self._thread.start()
        return self

    def _tree(self):
        k, w = self.k32, self.w
        snap = k.CreateToolhelp32Snapshot(0x2, 0)
        if snap in (None, w.HANDLE(-1).value):
            return []
        try:
            entries = []
            e = self.PE32()
            e.dwSize = ctypes.sizeof(self.PE32)
            more = k.Process32FirstW(snap, ctypes.byref(e))
            while more:
                entries.append((e.th32ProcessID, e.th32ParentProcessID, e.szExeFile))
                more = k.Process32NextW(snap, ctypes.byref(e))
        finally:
            k.CloseHandle(snap)
        tree, grew = {self.root}, True
        while grew:
            grew = False
            for pid, ppid, _ in entries:
                if ppid in tree and pid not in tree and pid != ppid:
                    tree.add(pid)
                    grew = True
        return [(pid, name) for pid, _, name in entries if pid in tree and pid != self.root]

    def _read(self, info):
        pmc = self.PMC()
        pmc.cb = ctypes.sizeof(self.PMC)
        if self.k32.K32GetProcessMemoryInfo(info['handle'], ctypes.byref(pmc), pmc.cb):
            info['peak_ws'] = max(info['peak_ws'], pmc.PeakWorkingSetSize)
            info['peak_private'] = max(info['peak_private'], pmc.PeakPagefileUsage)
            info['reads'] += 1
            return True
        return False

    def poll(self):
        for pid, name in self._tree():
            if name.lower() not in self.names or pid in self.seen:
                continue
            h = self.k32.OpenProcess(0x1000 | 0x0010, False, pid)    # QUERY_LIMITED_INFORMATION | VM_READ
            if h:
                self.seen[pid] = dict(name=name, handle=h, peak_ws=0, peak_private=0, reads=0)
        for info in self.seen.values():
            if info['handle']:
                self._read(info)
        self.polls += 1

    def _loop(self):
        try:
            while not self._stop.is_set():
                self.poll()
                seen = any(i['name'].lower() == self.primary for i in self.seen.values())
                self._stop.wait(self.slow_s if seen else self.poll_s)
        except Exception as e:                       # recorded, never fatal to the run
            self.error = repr(e)

    def stop(self):
        """Stops polling, reads every handle once more (the process has ended) and closes it."""
        if not self.ok:
            return dict(supported=False)
        self._stop.set()
        self._thread.join()
        out = []
        for pid, info in sorted(self.seen.items()):
            after = self._read(info)
            self.k32.CloseHandle(info['handle'])
            info['handle'] = None
            out.append(dict(pid=pid, name=info['name'], peak_working_set_bytes=info['peak_ws'],
                            peak_private_bytes=info['peak_private'], reads=info['reads'], read_after_exit=after))
        return dict(supported=True, poll_s=self.poll_s, slow_s=self.slow_s, polls=self.polls, processes=out,
                    error=self.error)


def output_size(folder):
    """Every file under the run folder by the size its directory entry reports (os.scandir; on
    Windows the listing itself carries it, so no file is opened): total bytes and count, and per
    top-level entry."""
    parts, total, count = {}, 0, 0
    stack = [(Path(folder), None)]
    while stack:
        d, top = stack.pop()
        with os.scandir(d) as it:
            for e in it:
                key = top or e.name
                if e.is_dir(follow_symlinks=False):
                    stack.append((Path(e.path), key))
                elif e.is_file(follow_symlinks=False):
                    size = e.stat(follow_symlinks=False).st_size
                    n, b = parts.get(key, (0, 0))
                    parts[key] = (n + 1, b + size)
                    total += size
                    count += 1
    return dict(bytes=total, files=count, parts={k: dict(files=n, bytes=b) for k, (n, b) in sorted(parts.items())})


def probe(name, *, project, simpa_exe=None, solvers_dir=DEFAULT_SOLVERS, probe_root=PROBE_ROOT,
          launch_log=LAUNCH_LOG, data_root=DATA_ROOT, poll_s=0.1, watch=('spps.exe', 'tetgen.exe')):
    """One probe of sections 7 and 7.1 (see the module docstring). Returns its record."""
    if name not in PROBE_RUNS:
        raise Refused('not_a_probe', '%r is not one of %s' % (name, sorted(PROBE_RUNS)))
    spec = dict(PROBE_RUNS[name], run_id='probe-' + name, kind='probe')
    probe_root = Path(probe_root)
    if inside(probe_root, data_root) or inside(data_root, probe_root) or _norm(probe_root) == _norm(data_root):
        raise Refused('not_a_probe', '%s overlaps the data root %s' % (probe_root, data_root))
    if spec['random_seed'] not in RESERVED_SEEDS:
        raise Refused('not_a_probe', 'a probe runs with a reserved seed')
    room_project = _read_project(project)
    room = rooms.rooms()[spec['room']]
    verts = room_project['geometry']['vertices']
    box = [[min(v[a] for v in verts) for a in range(3)], [max(v[a] for v in verts) for a in range(3)]]
    if room_project.get('name') != spec['room'] or box != room['bbox_m']:
        raise Refused('not_a_probe', '%s is not the project of %s (name %r, bounding box %s)'
                      % (project, spec['room'], room_project.get('name'), box))
    check = check_solvers(solvers_dir)
    if not check['verified']:
        raise Refused('solver_unverified', '%s: %s' % (solvers_dir, [c['name'] for c in check['checks'] if not c['matches']]))

    pdir = probe_root / 'projects'
    pdir.mkdir(parents=True, exist_ok=True)
    stem, n = '%s-probe' % name, 1                   # a second attempt gets <name>-probe-2, and so on
    while (pdir / (stem + '.simpa')).exists():
        n += 1
        stem = '%s-probe-%d' % (name, n)
    proj = pdir / (stem + '.simpa')
    proj.write_text(rooms.canonical_json(run_project(room_project, spec)), encoding='utf-8', newline='\n')
    cmd = _command(simpa_exe) + ['run', str(proj), '--solver', 'spps', '--runs', str(probe_root), '--json'] + _exe_args(solvers_dir)
    exe = Path(cmd[0]) if not isinstance(simpa_exe, (list, tuple)) else None
    rec = dict(probe=name, settings=spec, room_project=str(project), project=str(proj), project_sha256=sha256_file(proj),
               command=cmd, simpa_sha256=sha256_file(exe) if exe and exe.is_file() else None, solvers=check,
               c_free_before_bytes=shutil.disk_usage('C:\\').free if sys.platform == 'win32' else None,
               started=_now())
    _append_line(launch_log, dict(at=rec['started'], what='probe', probe=name, room=spec['room'], mode=spec['mode'],
                                  time_step_s=spec['time_step_s'], particles_per_source=spec['particles_per_source'],
                                  random_seed=spec['random_seed'], duration_s=spec['duration_s'], runs=str(probe_root),
                                  solvers_verified=True))
    out = probe_root / ('%s.stdout.json' % stem)
    err = probe_root / ('%s.stderr.txt' % stem)
    t0 = time.perf_counter()
    with open(out, 'wb') as fo, open(err, 'wb') as fe:
        p = subprocess.Popen(cmd, stdout=fo, stderr=fe, env=_env(solvers_dir))
        w = _Watch(p.pid, watch, poll_s).start()
        code = p.wait()
        wall = time.perf_counter() - t0
        rec['memory'] = w.stop()
    rec['wall_s'] = wall
    rec['finished'] = _now()
    rec['exit'] = code
    manifest, folder = _run_folder(out.read_text(encoding='utf-8', errors='replace'))
    rec['run_folder'] = folder
    if manifest:
        rec['outcome'] = manifest.get('outcome')
        rec['verdict'] = manifest.get('verdict')
        rec['exit_class'] = manifest.get('exit_class')
        rec['manifest_lines'] = manifest.get('lines')
        rec['manifest_files'] = manifest.get('files')
        rec['exe'] = manifest.get('exe')
    rec['stderr'] = class_lines(err.read_text(encoding='utf-8', errors='replace'))
    rec['output'] = output_size(folder) if folder and Path(folder).is_dir() else None
    rec['c_free_after_bytes'] = shutil.disk_usage('C:\\').free if sys.platform == 'win32' else None
    _append_line(probe_root / 'probes.jsonl', rec)
    return rec


def _peak(rec, name):
    ps = [p for p in (rec.get('memory') or {}).get('processes', []) if p['name'].lower() == name]
    return max((p['peak_working_set_bytes'] for p in ps), default=None)


def main(argv=None):
    import argparse
    ap = argparse.ArgumentParser(description='The M8b driver: the solver check and the probes (no run of the test).')
    sub = ap.add_subparsers(dest='cmd', required=True)
    c = sub.add_parser('check-solvers')
    c.add_argument('--solvers', type=Path, default=Path(os.environ.get('SIMPA_SOLVERS_DIR', DEFAULT_SOLVERS)))
    pr = sub.add_parser('probe')
    pr.add_argument('names', nargs='+', choices=sorted(PROBE_RUNS))
    pr.add_argument('--simpa', type=Path, default=Path(os.environ.get('M8B_SIMPA_EXE', DEFAULT_SIMPA)))
    pr.add_argument('--solvers', type=Path, default=Path(os.environ.get('SIMPA_SOLVERS_DIR', DEFAULT_SOLVERS)))
    pr.add_argument('--probe-root', type=Path, default=PROBE_ROOT)
    pr.add_argument('--launch-log', type=Path, default=LAUNCH_LOG)
    a = ap.parse_args(argv)
    if a.cmd == 'check-solvers':
        r = check_solvers(a.solvers)
        print(json.dumps(r, indent=1))
        return 0 if r['verified'] else 1
    written = rooms.write_projects(a.probe_root / 'rooms', a.simpa, names=sorted({PROBE_RUNS[n]['room'] for n in a.names}))
    for n in a.names:
        rec = probe(n, project=written[PROBE_RUNS[n]['room']], simpa_exe=a.simpa, solvers_dir=a.solvers,
                    probe_root=a.probe_root, launch_log=a.launch_log)
        o = rec.get('outcome') or {}
        print('%s: exit %s, verdict %s, wall %.1f s, elapsed_ms %s, spps peak working set %s bytes, output %s bytes in %s files'
              % (n, rec['exit'], (rec.get('verdict') or {}).get('status'), rec['wall_s'], o.get('elapsed_ms'),
                 _peak(rec, 'spps.exe'), (rec.get('output') or {}).get('bytes'), (rec.get('output') or {}).get('files')))
    return 0


if __name__ == '__main__':
    sys.exit(main())
