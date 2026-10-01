"""P2, P3 and P33 of ../../HARNESS-PLAN.md: the corpus the held-out sets keep clear of, and the frozen
method's weak spots on it, each written from its sources with the hash of every file read.

corpus_rooms.json (P2, P3): every corpus room with its geometry and its kind, box or not, read from the
  source that defines it, and the kind checked against that source's own words. The corpus items with no
  geometry (t1, the critique's scans and their double-slope rate ratios) are listed beside the rooms.
weak_spots.json (P33): every corpus row on which the frozen method is wrong-silent (status ok and
  |edt/truth - 1| > 0.05) or a near miss (status ok and 0.025 < |edt/truth - 1| <= 0.05), with the
  value of every quantity the row carries in the vocabulary of the fresh sets' draws (P21-P24):
  - from the logged results, final/res_{t1,z3,real,ism}.pkl, each set recomputed here with the frozen
    method from its own inputs and compared with the log, row by row;
  - for the scans, recomputed on their corpus inputs (P4): the five constructions of final/run_scans.py
    (scan 1, scan 4 occluded, scan 2b, scan 3 noise, the real seeds), checked against the tallies it
    logged (final/scans_all.json); and every other construction of the critique (critique/*.py) that
    pairs the method's input with a truth or is the same construction as one that does, each as that
    file builds it: scan 1 as critique/scan_i12.py builds it (its ill-conditioned rows, which
    run_scans.py left out) and as the critique first ran it (target's scan_i12.json: no d cutoff, no
    dedup), both checked row by row against those outputs; scan 2 (scan_i4.py, rate ratio 4); scan 2b
    as scan_i4b.py calls the method (no meta) beside run_scans.py's half_width 0; scan 5
    (scan_misc.py (a) and (b)); noise_probe.py's rows (scan 3's settings, its own noise seed) and
    ablate.py's rows (scan 1's grid cut at three run lengths, and blocked receivers), with the truth
    their parent scan gives the same construction; the real seeds as real_seeds.py calls the method
    (no meta), checked to give run_scans.py's results;
  - z3grid (P33 names it; no result of the frozen method on it was logged): eval_run.py's z3_job
    construction, every robust z3 geometry at 2, 1, 0.5, 0.2 and 0.1 ms with no delay, through a text
    copy of the judge's generator (m8b/_z3echo.py, checked against its source on every run).
  Corpus items no frozen-method result exists for, and rows of the same rooms that were never part of
  the EDT method's corpus, are listed under not_scored with the reason (checked where it can be).
The frozen method is frozen/method.py, run only after its sha256 is checked (PREREG.md:8-10). No held-out
data is read or made; nothing is written but the two files.

Usage (from harness/, with the venv's python and PYTHONDONTWRITEBYTECODE=1):
    python -m m8b.corpus [--target-root <repo>/target] [--out <dir>] [--workers 8] [--only rooms|weak]
"""
import argparse
import ast
import base64
import collections
import glob
import hashlib
import itertools
import json
import math
import os
import pickle
import platform
import re
import sys
import time
import types
from pathlib import Path

sys.dont_write_bytecode = True
import numpy as np  # noqa: E402

HERE = Path(__file__).resolve().parent                  # harness/m8b
HARNESS = HERE.parent                                   # harness/
INVESTIGATION = HARNESS.parent                          # docs/investigations/2026-09-27-edt-heldout
REPO = INVESTIGATION.parents[2]                         # the worktree
SIMPLIFY = REPO / 'docs' / 'investigations' / '2026-09-27-edt-simplify'
FROZEN = INVESTIGATION / 'frozen' / 'method.py'
FROZEN_SHA256 = '462c37cf159d4d9bd8abadd8261f975fa47ace66f0d4cb4b1e6f4234c77fdf6e'   # PREREG.md:8
SYNTH = SIMPLIFY / 'critique' / 'synth.py'
# critique/synth.py with its line ends as git stores them (\n): the same bytes as target/'s original.
SYNTH_SHA256_LF = '3399262fc310059918465d3ed4b0c963da33e39bc9428721fbddb7907fe1bc11'
Z3ECHO = HERE / '_z3echo.py'
PROVENANCE = HERE / 'provenance.json'
Z3ECHO_MARKER = '# ---- copied from judge.py (do not edit below this line) ----'

JND = 0.05                  # PREREG.md:48
NEAR = 0.025                # P33: half the JND
C_SPPS = 343.2              # P5, frozen/method.py:20
TEXT_SUFFIXES = {'.py', '.md', '.rs', '.json', '.jsonl', '.txt', '.log', '.simpa', '.toml'}

SCHEMA_ROOMS = 'm8b.corpus_rooms/1'
SCHEMA_WEAK = 'm8b.weak_spots/1'


class VoidRun(RuntimeError):
    """A pinned file's hash differs: nothing is computed from it (PREREG.md:10)."""


# ================================================================================================
# Sources: every file read goes through one of these, so each output lists every source's hash.
# ================================================================================================
def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


class Sources:
    def __init__(self, repo, target):
        self.repo = Path(repo).resolve()
        self.target = Path(target).resolve()
        self.seen = {}

    def label(self, path):
        p = Path(path).resolve()
        for root, name in ((self.repo, 'worktree'), (self.target.parent, 'checkout')):
            try:
                return name, p.relative_to(root).as_posix()
            except ValueError:
                pass
        return 'absolute', p.as_posix()

    def read(self, path):
        data = Path(path).read_bytes()
        root, rel = self.label(path)
        rec = dict(root=root, path=rel, bytes=len(data), sha256=sha256_bytes(data))
        if Path(path).suffix.lower() in TEXT_SUFFIXES:
            rec['sha256_lf'] = sha256_bytes(data.replace(b'\r\n', b'\n'))
        self.seen[(root, rel)] = rec
        return data

    def text(self, path):
        return self.read(path).decode('utf-8')

    def json(self, path):
        return json.loads(self.read(path))

    def pickle(self, path):
        return pickle.loads(self.read(path))

    def records(self):
        return [self.seen[k] for k in sorted(self.seen)]


def find_target_root(explicit=None):
    """The gitignored target/ that holds the corpus: --target-root, else $M8B_TARGET_ROOT, else the
    nearest target/agents/edt-simplify above the worktree (the main checkout's)."""
    if explicit:
        return Path(explicit)
    if os.environ.get('M8B_TARGET_ROOT'):
        return Path(os.environ['M8B_TARGET_ROOT'])
    for a in (REPO, *REPO.parents):
        if (a / 'target' / 'agents' / 'edt-simplify' / 'final').is_dir():
            return a / 'target'
    raise SystemExit('no target/agents/edt-simplify/final above %s: pass --target-root' % REPO)


def exec_module(name, path, data):
    mod = types.ModuleType(name)
    mod.__file__ = str(path)
    exec(compile(data, str(path), 'exec'), mod.__dict__)
    return mod


def load_frozen(src=None, z=None):
    """frozen/method.py, executed from the very bytes whose sha256 was checked. VoidRun on a mismatch."""
    data = src.read(FROZEN) if src else FROZEN.read_bytes()
    h = sha256_bytes(data)
    if h != FROZEN_SHA256:
        raise VoidRun('frozen/method.py has sha256 %s, not %s: the run is void (PREREG.md:10). '
                      'A checkout with other line ends than this one changes the bytes.' % (h, FROZEN_SHA256))
    mod = exec_module('m8b_frozen_method', FROZEN, data)
    if z is not None:
        mod.Z = float(z)
    return mod


def load_synth(src=None):
    data = src.read(SYNTH) if src else SYNTH.read_bytes()
    h = sha256_bytes(data.replace(b'\r\n', b'\n'))
    if h != SYNTH_SHA256_LF:
        raise VoidRun('critique/synth.py has sha256 (LF) %s, not %s' % (h, SYNTH_SHA256_LF))
    return exec_module('m8b_critique_synth', SYNTH, data)


# ================================================================================================
# The text copy of the judge's generator: checked against its source before any z3 row is made.
# ================================================================================================
def z3echo_body(source_text):
    """What m8b/_z3echo.py holds below its marker, made from judge.py's text: the ranges in
    provenance.json, each range's lines joined by \\n and the ranges by two blank lines."""
    prov = json.loads(PROVENANCE.read_text(encoding='utf-8'))['m8b/_z3echo.py']
    lines = source_text.replace('\r\n', '\n').split('\n')
    parts = ['\n'.join(lines[a - 1:b]) for a, b in prov['line_ranges']]
    return '\n\n\n'.join(parts) + '\n'


def check_z3echo(src, judge_path):
    prov = json.loads(PROVENANCE.read_text(encoding='utf-8'))['m8b/_z3echo.py']
    data = src.read(judge_path)
    if sha256_bytes(data) != prov['source_sha256']:
        raise VoidRun('%s has sha256 %s, not the copied %s' % (judge_path, sha256_bytes(data), prov['source_sha256']))
    copy = src.text(Z3ECHO).replace('\r\n', '\n')
    body = copy.split(Z3ECHO_MARKER + '\n', 1)[1]
    if body != z3echo_body(data.decode('utf-8')):
        raise VoidRun('m8b/_z3echo.py is not the text of its source lines %s' % prov['line_ranges'])
    if sha256_bytes(body.encode('utf-8')) != prov['copy_body_sha256_lf']:
        raise VoidRun('m8b/_z3echo.py body hash differs from provenance.json')
    return dict(source=prov['source'], source_sha256=prov['source_sha256'], line_ranges=prov['line_ranges'],
                verified=True)


def check_provenance(src):
    """Every entry of m8b/provenance.json against its source as it is now: the code copied or re-typed
    from a file must still match that file, or nothing is computed (VoidRun). A source outside this
    worktree and the checkout's target/ (a session scratchpad) is listed, not read: it is not the
    repo's, and reading it would tie this file to a folder that is not kept."""
    prov = json.loads(src.text(PROVENANCE))
    out = {}
    for name, e in prov.items():
        if name.startswith('_'):
            continue
        s = e['source']
        if s.startswith('target/'):
            path = src.target.parent / s
        elif not Path(s).is_absolute() and ':' not in s:
            path = src.repo / s
        else:
            out[name] = dict(source=s, checked=False, why='outside the worktree and target/: recorded, not read')
            continue
        data = src.read(path)
        got = dict(sha256=sha256_bytes(data), sha256_lf=sha256_bytes(data.replace(b'\r\n', b'\n')))
        want = {k: e['source_' + k] for k in ('sha256', 'sha256_lf') if 'source_' + k in e}
        bad = {k: (v, got[k]) for k, v in want.items() if got[k] != v}
        if bad:
            raise VoidRun('provenance.json %s: its source %s has changed: %s' % (name, s, bad))
        out[name] = dict(source=s, checked=True, hashes=sorted(want))
    return out


# ================================================================================================
# P5: design T60. ISO 9613-1 air: text copy of target/agents/followup-design/skeptic-gf3/ism.py:25-44
# (sha256 a7c9d41e..., recorded in provenance.json), unchanged.
# ================================================================================================
def iso9613_db_per_m(F, H=50.0, P=101325.0, T_c=20.0):
    K = T_c + 273.15
    K01, Pref, Kref = 273.16, 101325.0, 293.15
    C = -6.8346 * (K01 / K) ** 1.261 + 4.6151
    Ps = Pref * 10 ** C
    hmol = H * Ps / Pref
    cson = 343.2 * math.sqrt(K / Kref)
    Acr = (Pref / P) * 1.60e-10 * math.sqrt(K / Kref) * F ** 2
    FmolO, KvibO, FmolN, KvibN = 0.209, 2239.1, 0.781, 3352.0
    Fr = (P / Pref) * (24. + 4.04e4 * hmol * (0.02 + hmol) / (0.391 + hmol))
    Am = 1.559 * FmolO * math.exp(-KvibO / K) * (KvibO / K) ** 2
    AvibO = Am * (F / cson) * 2. * (F / Fr) / (1 + (F / Fr) ** 2)
    Fr = (P / Pref) * math.sqrt(Kref / K) * (9. + 280. * hmol * math.exp(-4.170 * ((K / Kref) ** (-1. / 3.) - 1)))
    Am = 1.559 * FmolN * math.exp(-KvibN / K) * (KvibN / K) ** 2
    AvibN = Am * (F / cson) * 2. * (F / Fr) / (1 + (F / Fr) ** 2)
    return Acr + AvibO + AvibN


def m_energy(F):
    return iso9613_db_per_m(F) * math.log(10) / 10


def box_areas(dims):
    """Areas of the six faces in the order x0, x1, y0, y1, z0 (floor), z1 (ceiling)."""
    Lx, Ly, Lz = dims
    return [Ly * Lz, Ly * Lz, Lx * Lz, Lx * Lz, Lx * Ly, Lx * Ly]


def design_t60(dims, alpha6, F):
    """P5: Eyring with air, T = 24 ln10 V / (c(-S ln(1 - abar) + 4 m V)), m at 20 C and 50 % RH."""
    V = dims[0] * dims[1] * dims[2]
    areas = box_areas(dims)
    S = sum(areas)
    abar = sum(a * s for a, s in zip(alpha6, areas)) / S
    if abar >= 1.0:
        return 0.0
    return 24.0 * math.log(10) * V / (C_SPPS * (-S * math.log(1.0 - abar) + 4.0 * m_energy(F) * V))


def first_order_d1(src, rec, dims):
    """Shortest source-image-to-receiver distance over the six first-order images of a box."""
    best = math.inf
    for ax in range(3):
        for pl in (0.0, dims[ax]):
            img = list(src)
            img[ax] = 2 * pl - src[ax]
            best = min(best, math.dist(img, rec))
    return best


def wall_distance(p, dims):
    return min(min(p[i], dims[i] - p[i]) for i in range(3))


def room_quantities(dims, alpha6, src, rec, R, F, c):
    """Every quantity of a row in a box room that a fresh set draws (P21, P22, P24)."""
    srt = sorted((float(x) for x in dims), reverse=True)
    areas = box_areas(dims)
    d = math.dist(src, rec)
    d1 = first_order_d1(src, rec, dims)
    rw = wall_distance(rec, dims)
    q = dict(V_m3=dims[0] * dims[1] * dims[2], L1_m=srt[0], L2_m=srt[1], L3_m=srt[2],
             alpha_walls=[float(a) for a in alpha6], alpha_min=float(min(alpha6)), alpha_max=float(max(alpha6)),
             alpha_mean=sum(a * s for a, s in zip(alpha6, areas)) / sum(areas),
             t60_design_125_s=design_t60(dims, alpha6, 125.0), R_m=float(R), d_m=d, d_minus_R_m=d - R,
             gap_ms=1e3 * (d1 - d) / c, rec_wall_m=rw, rec_wall_minus_R_m=rw - R, src_wall_m=wall_distance(src, dims),
             delay_ms=0.0)
    if F is not None:
        q['band_hz'] = float(F)
        q['t60_s'] = design_t60(dims, alpha6, float(F))
    return q


# ================================================================================================
# Scoring: critique/common.py:18-26 (score), and P33's two classes.
# ================================================================================================
def score(r, truth, jnd=JND):
    """-> (kind, err), kind in ok_good, ok_WRONG, wide_in, wide_out, refused (critique/common.py:18-26)."""
    if r.get('edt') is None or r.get('status') == 'refused':
        return 'refused', None
    e = r['edt'] / truth - 1
    inside = r['edt_lo'] is not None and r['edt_hi'] is not None and r['edt_lo'] <= truth <= r['edt_hi']
    if r['status'] == 'ok':
        return ('ok_WRONG' if abs(e) > jnd else 'ok_good'), e
    return ('wide_in' if inside else 'wide_out'), e


def kind_of(r, truth):
    """score(), except that a shown row with no finite truth is 'no_truth'. score() is the critique's,
    copied verbatim, and counts such a row ok_good: it is kept for every tally checked against a log
    the critique wrote, and kind_of makes every other tally."""
    if truth is None or not math.isfinite(truth):
        return ('refused', None) if r.get('edt') is None or r.get('status') == 'refused' else ('no_truth', None)
    return score(r, truth)


def weak_class(status, edt, truth):
    """'wrong_silent', 'near_miss' or None (P33, PREREG.md:48)."""
    if status != 'ok' or edt is None or truth is None or not math.isfinite(truth) or truth == 0:
        return None
    e = abs(edt / truth - 1)
    if e > JND:
        return 'wrong_silent'
    if e > NEAR:
        return 'near_miss'
    return None


def histogram_holds_truth(q):
    """HARNESS-PLAN.md 8.2, second call: False when a synth row's source sits inside the receiver
    ball (d_m < R_m > 0). synth.ball_atoms floors rho there (rho = C tau, negative before t = 0), so
    the generator's own histogram does not hold the row's truth (what the method actually read): on
    the one corpus row this ever binds, scan1's d 0.17 m case, the method's edt is within 0.1 % of the
    truth of what its histogram holds, not of the row's own (formula) truth, 3.3244 s against 3.0003 s
    (HARNESS-PLAN.md 8.2's second call; synth_fresh.py's module docstring). A row this is False for is
    not counted as a weak spot of the method for d_m or d_minus_R_m: those two quantities are blanked
    to None in its weak_spots.json record (row_record, scan1), so T22b does not require a fresh set's
    bound to reach them, while the row itself stays in weak_spots.json (575 rows, T22a), unabridged in
    every other field, as the record of the generator's own defect that it is."""
    d, R = q.get('d_m'), q.get('R_m')
    return d is None or R is None or not (R > 0) or d >= R


def clean(x):
    """JSON-safe: numpy scalars to Python, non-finite floats to None."""
    if isinstance(x, dict):
        return {str(k): clean(v) for k, v in x.items()}
    if isinstance(x, (list, tuple)):
        return [clean(v) for v in x]
    if isinstance(x, (np.floating, float)):
        x = float(x)
        return x if math.isfinite(x) else None
    if isinstance(x, (np.integer,)):
        return int(x)
    if isinstance(x, np.bool_):
        return bool(x)
    return x


def row_record(set_, id_, res, truth, q, *, has_room, noise, truth_kind, extra=None):
    edt = res.get('edt')
    rec = dict(set=set_, id=id_, cls=weak_class(res.get('status'), edt, truth), status=res.get('status'),
               err=None if edt is None or truth is None or not math.isfinite(truth) else edt / truth - 1,
               edt=edt, edt_lo=res.get('edt_lo'), edt_hi=res.get('edt_hi'), truth=truth, truth_kind=truth_kind,
               reason=res.get('reason'), has_room=has_room, noise=noise, q=q)
    if extra:
        rec.update(extra)
    return rec


# ================================================================================================
# corpus_rooms.json (P2, P3)
# ================================================================================================
def kind_evidence(src, path, line, must_contain, last=None):
    """The source's own words at path:line (to :last), which must say what the kind is."""
    lines = src.text(path).replace('\r\n', '\n').split('\n')[line - 1:(last or line)]
    text = ' '.join(x.strip() for x in lines)
    if must_contain not in text:
        raise VoidRun('%s:%d does not say %r: %r' % (path, line, must_contain, text))
    root, rel = src.label(path)
    return dict(root=root, path=rel, line=line if not last else '%d-%d' % (line, last), text=text)


def room_entry(set_, id_, dims, kind, evidence, *, source=None, receivers=None, extra=None):
    dims = [float(x) for x in dims]
    e = dict(set=set_, id=id_, kind=kind, dims_m=dims, dims_sorted_m=sorted(dims, reverse=True),
             volume_m3=dims[0] * dims[1] * dims[2], surface_m2=sum(box_areas(dims)),
             bbox_m=[[0.0, 0.0, 0.0], dims], kind_evidence=evidence)
    if source is not None:
        e['source_m'] = [float(x) for x in source]
    if receivers is not None:
        e['receivers_m'] = [[float(x) for x in r] for r in receivers]
    if extra:
        e.update(extra)
    return e


def assign_value(tree, name):
    for node in ast.walk(tree):
        if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == name for t in node.targets):
            return ast.literal_eval(node.value), node.lineno, node.end_lineno
    raise KeyError(name)


def ism_rooms(src, target):
    """attack_ism.py ROOMS (the ISM corpus's four rooms) and attack_radius.py's radius variants."""
    gf3 = target / 'agents' / 'followup-design' / 'skeptic-gf3'
    ai = gf3 / 'attack_ism.py'
    rooms, a, b = assign_value(ast.parse(src.text(ai)), 'ROOMS')
    ev = [kind_evidence(src, gf3 / 'ism.py', 1, 'specular box room'),
          kind_evidence(src, ai, a + 1, 'name: (L, alpha')]
    rad = src.text(gf3 / 'attack_radius.py')
    radii = [float(x) for x in re.search(r'for R in \(([^)]*)\)', rad).group(1).split(',')]
    rad_rooms = re.findall(r"\('(\w+)', \[\(", rad)
    out = []
    for name, (L, alpha, s, T) in rooms.items():
        if len(L) != 3 or len(alpha) != 3 or not all(len(p) == 2 for p in alpha):
            raise VoidRun('ROOMS[%s] is not a box with a pair of absorptions per axis' % name)
        al6 = [alpha[0][0], alpha[0][1], alpha[1][0], alpha[1][1], alpha[2][0], alpha[2][1]]
        out.append(room_entry('ism', 'ism:' + name, L, 'box', ev, source=s,
                              extra=dict(alpha_walls=al6, image_time_s=T, defined_at='attack_ism.py:%d-%d' % (a, b),
                                         radius_variants_m=radii if name in rad_rooms else [])))
    return out, dict(rooms=len(out), radius_variant_rooms=rad_rooms, radius_variants_m=radii)


def z3_rooms(src, target):
    """z3's confirmed cases (judge_results.json), one entry per distinct box."""
    jr = src.json(target / 'agents' / 'z3-hunt' / 'judge' / 'judge_results.json')
    conf = jr['summary']['confirmed']
    ev = [kind_evidence(src, target / 'agents' / 'z3-hunt' / 'z3-verdict.md', 147, 'Specular boxes only'),
          kind_evidence(src, target / 'agents' / 'z3-hunt' / 'judge' / 'judge.py', 12, 'specular box')]
    boxes = collections.OrderedDict()
    for c in conf:
        if len(c['L']) != 3:
            raise VoidRun('z3 case %s is not a box' % c['id'])
        key = tuple(round(float(x), 9) for x in c['L'])
        boxes.setdefault(key, []).append(c)
    out = []
    for i, (key, cases) in enumerate(boxes.items()):
        c0 = cases[0]
        out.append(room_entry('z3', 'z3:box%03d' % i, c0['L'], 'box', ev,
                              extra=dict(cases=[c['id'] for c in cases], robust_cases=[c['id'] for c in cases if c.get('robust_both')],
                                         alpha_walls_by_case={c['id']: c['al6'] for c in cases})))
    robust = [c for c in conf if c.get('robust_both')]
    geoms = {(tuple(round(x, 12) for x in c['L']), tuple(round(x, 12) for x in c['al6']), tuple(round(x, 12) for x in c['S']),
              tuple(round(x, 12) for x in c['P']), round(float(c['R']), 12), round(float(c['c']), 9), round(float(c['m']), 15))
             for c in robust}
    return out, dict(confirmed_cases=len(conf), robust_cases=len(robust), distinct_boxes=len(out),
                     robust_geometries_z3grid=len(geoms))


RUST_NOISE = Path('crates') / 'simpa' / 'tests' / 'noise_calibration.rs'


def rust_fn_block(text, fn):
    i = text.index('fn %s(self)' % fn)
    j = text.index('\n    }\n', i)
    return text[i:j]


def noise_cal_rooms(src):
    """The noise calibration's six box rooms and its cell table (crates/simpa/tests/noise_calibration.rs),
    with the 32 cells the EDT corpus read (the pm8 runs' cells.json, P2)."""
    path = REPO / RUST_NOISE
    text = src.text(path).replace('\r\n', '\n')
    ev = kind_evidence(src, path, 61, 'A box room')
    num = r'(-?[\d.]+)'
    point = r'Room::(\w+) => \[%s, %s, %s\]' % (num, num, num)
    sizes = {m[0]: [float(m[1]), float(m[2]), float(m[3])] for m in re.findall(point, rust_fn_block(text, 'size'))}
    sources = {m[0]: [float(m[1]), float(m[2]), float(m[3])] for m in re.findall(point, rust_fn_block(text, 'source'))}
    names = dict(re.findall(r'Room::(\w+) => "([^"]+)"', rust_fn_block(text, 'name')))
    receivers = {}
    for m in re.finditer(r'Room::(\w+) => \[\n(.*?)\n\s*\],', rust_fn_block(text, 'receivers'), re.S):
        receivers[m.group(1)] = [[float(x) for x in p] for p in re.findall(r'\[%s, %s, %s\]' % (num, num, num), m.group(2))]
    if not sizes or set(sizes) != set(sources) or set(sizes) != set(names) or set(sizes) != set(receivers):
        raise VoidRun('noise_calibration.rs: the rooms of size(), source(), name() and receivers() differ')
    for key, dims in sizes.items():
        if names[key] != 'x'.join('%g' % x for x in dims):
            raise VoidRun('noise_calibration.rs: Room::%s is named %s but sized %s' % (key, names[key], dims))
    rooms = []
    for key, dims in sizes.items():
        rooms.append(room_entry('noise_cal', 'noise_cal:' + names[key], dims, 'box', [ev], source=sources[key],
                                receivers=receivers[key], extra=dict(rust_variant=key)))
    cell_re = re.compile(r'cell(_r)?\("([^"]+)", (\w+), Room::(\w+), (Walls::\w+(?:\([\d.]+\))?|[A-Z_]+), '
                         r'(Random|Energetic), ([\d_]+), ([\d.]+), ([\d.]+), ([\d.]+)(?:, ([\w.]+))?\),')
    roles = dict(C='calibration', V='validation', W='validation2', C3='calibration3', V3='validation3', V4='validation4')
    cells = []
    for m in cell_re.finditer(text):
        cells.append(dict(id=m.group(2), role=roles[m.group(3)], room=names[m.group(4)], walls_rust=m.group(5),
                          method=m.group(6).lower(), particles_per_source=int(m.group(7).replace('_', '')),
                          duration_s=float(m.group(8)), time_step_s=float(m.group(9)), trans_epsilon=float(m.group(10)),
                          receiver_radius_m=0.31 if m.group(11) in (None, 'R0') else float(m.group(11))))
    declared = int(re.search(r'const CELLS: \[Cell; (\d+)\]', text).group(1))
    if len(cells) != declared:
        raise VoidRun('parsed %d noise-cal cells, the table declares %d' % (len(cells), declared))
    return rooms, cells, names


def pm8_cells(src, target):
    """The noise-cal cells whose runs the EDT corpus read (pm8-noise-scratch/runs/noise-cal-*/cells.json)."""
    out = []
    for root in sorted((target / 'agents' / 'pm8-noise-scratch' / 'runs').glob('noise-cal-*')):
        for c in src.json(root / 'cells.json'):
            cc = c if 'id' in c else c['cell']
            out.append(dict(cc, run_root=src.label(root)[1]))
    return out


def m8a_rooms(src):
    """M8a's bed rooms (beds/m8a.json, which bed/file.rs holds byte for byte to BedFile::m8a) and the
    upstream atmospheric-absorption validation room the bed also runs (SPEC.md 2.5)."""
    bed = src.json(REPO / 'beds' / 'm8a.json')
    ev = kind_evidence(src, REPO / 'crates' / 'simpa-core' / 'src' / 'bed' / 'file.rs', 43, 'A box room')
    out = [room_entry('m8a', 'm8a:' + r['name'], r['size_m'], 'box', [ev], source=r['source_m'], receivers=r['receivers_m'],
                      extra=dict(gated=r['gated'], alphas=sorted({c['alpha'] for c in bed['cells'] if c['room'] == r['name']})))
           for r in bed['rooms']]
    spec = REPO / 'docs' / 'investigations' / '2026-09-29-m8a' / 'SPEC.md'
    ev_atm = [kind_evidence(src, spec, 178, '12 faces, 60 m³, 94 m²', last=179),
              kind_evidence(src, REPO / RUST_NOISE, 66, "upstream's atmospheric-absorption validation room")]
    out.append(room_entry('m8a', 'm8a:atmospheric-validation', [5.0, 4.0, 3.0], 'box', ev_atm,
                          source=[2.5, 2.0, 1.5], receivers=[[1.0, 1.0, 1.0]],
                          extra=dict(note='upstream Validation_atmospheric_absorption.proj as `simpa import-proj` reads it: '
                                          '12 faces, 60 m3, 94 m2 (SPEC.md:178-179), the 5 x 4 x 3 m room of '
                                          'noise_calibration.rs:66; dims checked against V and S below')))
    atm = out[-1]
    if abs(atm['volume_m3'] - 60.0) > 1e-9 or abs(atm['surface_m2'] - 94.0) > 1e-9:
        raise VoidRun('the atmospheric validation room is not 60 m3 and 94 m2')
    return out, len(bed['cells'])


def m8a_atmospheric_project_check(src, path):
    """When the bed's imported project is on disk: its 8 vertices are the corners of the 5 x 4 x 3 box."""
    if not path or not Path(path).is_file():
        return dict(checked=False, reason='not on disk: %s' % path)
    p = src.json(path)
    v = np.asarray(p['geometry']['vertices'], float)
    lo, hi = v.min(0), v.max(0)
    corners = all(all(abs(x - lo[i]) < 1e-9 or abs(x - hi[i]) < 1e-9 for i, x in enumerate(row)) for row in v)
    return dict(checked=True, vertices=len(v), faces=len(p['geometry']['faces']), dims_m=(hi - lo).tolist(),
                all_vertices_at_box_corners=bool(corners),
                box=bool(corners and len(v) == 8 and len(p['geometry']['faces']) == 12 and np.allclose(hi - lo, [5, 4, 3])))


def critique_families(src):
    """Every double-slope rate ratio in the critique's and the final method's constructions (k2 = k1 / r)."""
    fams = collections.defaultdict(list)
    files = sorted(list((SIMPLIFY / 'critique').glob('*.py')) + list((SIMPLIFY / 'final').rglob('*.py')))
    for f in files:
        for i, line in enumerate(src.text(f).replace('\r\n', '\n').split('\n'), 1):
            for m in re.finditer(r'k2 = k1 / ([\d.]+)', line):
                fams[float(m.group(1))].append('%s:%d' % (f.relative_to(REPO).as_posix(), i))
    return [dict(ratio=r, defined_at=fams[r]) for r in sorted(fams)]


def no_geometry_items(src, target):
    t1 = target / 'agents' / 'edt-band' / 'golden' / 't1.jsonl'
    n_t1 = sum(1 for line in src.text(t1).splitlines() if line.strip())
    rs = SIMPLIFY / 'final' / 'run_scans.py'
    src.read(rs)
    items = [dict(set='t1', id='t1:golden', kind='no geometry', n_cases=n_t1,
                  what='closed-form exponentials with a direct arrival (T1 generator), frozen in golden/t1.jsonl')]
    scans = [('scan1', 'final/run_scans.py:25-58; critique/scan_i12.py, fix_probe_dedup.py; target critique/scan_i12.json',
              'noise-free single slope, V in (200, 2000, 20000) m3 (a formula parameter), T60 0.3-3 s, d 0.7-30 m '
              '(grid) and the arrival it gives, 0.086-30.8 m in the first run (no 0.6 m cutoff), gap 0-10 ms, steps '
              '1/2/5/10 ms, arrival phase 0.05/0.5/0.95, R 0.31 m, run 2 T60; level after the direct above and below '
              '-8 dB'),
             ('scan2', 'critique/scan_i4.py', 'noise-free truncated runs, single and double slope with rate ratio 4, '
              'late share -15/-10/-5 dB, T60 0.5-8 s, steps 1/2/10 ms, runs 0.12-1.5 T60, DRR -12/-3/0 dB, point receiver'),
             ('scan2b', 'final/run_scans.py:85-118; critique/scan_i4b.py', 'as scan 2 with rate ratio 3, late share '
              '-20/-15/-10 dB, runs 0.15-1.5 T60, DRR -12/-6/-3 dB; the method told half_width 0 (run_scans) or '
              'nothing (scan_i4b)'),
             ('scan3', 'final/run_scans.py:120-157; critique/scan_i5.py', 'random-mode compound-Poisson noise, 12 '
              'configurations (V, T60, d, N, step), 200 seeds each, run 1.5 T60, R 0.31 m'),
             ('scan4', 'final/run_scans.py:60-83; critique/scan_i1_occluded.py', 'blocked or weak direct sound, T60 '
              '0.4-3 s, first arrival 2-40 ms late, steps 1/2/10 ms'),
             ('scan5', 'critique/scan_misc.py', '(a) energetic-mode noise V 20000 m3 T60 2.5 s d 10 m; (b) two sources; '
              '(c) degenerate inputs'),
             ('seeds', 'final/run_scans.py:159-213; critique/real_seeds.py', "the real noise-cal seeds' 10 cells at 1 ms "
              'rebinned to 2 ms, each seed against its 10-seed mean'),
             ('noise_probe', 'critique/noise_probe.py', "five of scan 3's configurations, 200 seeds each from generator "
              'seed 11'),
             ('ablate', 'critique/ablate.py:19-45', "scan 1's grid (d 0.7/1.5/3/8/20 m, gap 0/5 ms, steps 1/2/10 ms) cut "
              'at 0.3, 0.5 and 2.0 T60, and 12 blocked receivers (T60 0.4-1.5 s, first arrival 5 or 20 ms late)')]
    for name, where, what in scans:
        items.append(dict(set=name, id='scan:' + name, kind='no geometry', defined_at=where, what=what))
    return items


def build_corpus_rooms(target, m8a_project=None):
    src = Sources(REPO, target)
    ism, ism_info = ism_rooms(src, target)
    z3, z3_info = z3_rooms(src, target)
    nc_rooms, nc_cells, names = noise_cal_rooms(src)
    pm8 = pm8_cells(src, target)
    by_id = {c['id']: c for c in nc_cells}
    mism = [c['id'] for c in pm8 if c['id'] not in by_id or by_id[c['id']]['room'] != c['room']
            or abs(by_id[c['id']]['time_step_s'] - c['time_step_s']) > 1e-12
            or by_id[c['id']]['particles_per_source'] != c['particles_per_source']]
    if mism:
        raise VoidRun('pm8 cells disagree with the Rust cell table: %s' % mism)
    in_corpus = {c['id'] for c in pm8}
    for c in nc_cells:
        c['in_edt_corpus'] = c['id'] in in_corpus
    m8a, m8a_cells = m8a_rooms(src)
    atm = m8a_atmospheric_project_check(src, m8a_project)
    rooms = ism + z3 + nc_rooms + m8a
    kinds = collections.Counter(r['kind'] for r in rooms)
    out = dict(
        schema=SCHEMA_ROOMS,
        plan='HARNESS-PLAN.md P2, P3 and section 3 (corpus.py)',
        rule_p3='Two boxes are near-duplicates when each sorted dimension is within 10 %. Two non-box rooms are '
                'near-duplicates when their sorted bounding boxes, volumes and surface areas are each within 10 %. '
                'A box and a non-box room are not near-duplicates. A room is fresh when it is a near-duplicate of no '
                'corpus room.',
        rooms=rooms,
        no_geometry=no_geometry_items(src, target),
        families=critique_families(src),
        noise_cal_cells=nc_cells,
        summary=dict(rooms=len(rooms), kinds=dict(kinds), every_room_a_box=set(kinds) == {'box'},
                     ism=ism_info, z3=z3_info,
                     noise_cal=dict(rooms=len(nc_rooms), cells_in_table=len(nc_cells), cells_in_edt_corpus=len(in_corpus),
                                    edt_corpus_1ms_cells=sorted(c['id'] for c in pm8 if abs(c['time_step_s'] - 0.001) < 1e-12),
                                    edt_corpus_10ms_cells=sorted(c['id'] for c in pm8 if abs(c['time_step_s'] - 0.01) < 1e-12)),
                     m8a=dict(rooms=len(m8a), bed_cells=m8a_cells, atmospheric_project_check=atm),
                     distinct_boxes_sorted_dims=len({tuple(r['dims_sorted_m']) for r in rooms})),
        sources=src.records(),
    )
    return clean(out)


# ================================================================================================
# weak_spots.json (P33)
# ================================================================================================
ALLOWED = ['C-E3', 'C-E4', 'C-E6', 'V-E2', 'V-E5', 'C-R3', 'C-R4', 'C-R6', 'V-R2', 'V-R6']   # run_scans.py:162


def walls_alpha6(label):
    """The six absorptions (x0, x1, y0, y1, floor, ceiling) of a noise-cal cell's walls label
    (noise_calibration.rs Walls::label; tutorial 1's materials: ceiling 0.3, floor 0.1, walls 0.2, :174)."""
    m = re.match(r'^(Lambert|specular) a([\d.]+)$', label)
    if m:
        return [float(m.group(2))] * 6
    if label == "tutorial 1's materials, air on":
        return [0.2, 0.2, 0.2, 0.2, 0.1, 0.3]
    if label.startswith('dead floor (0.6; 0.05 elsewhere)'):
        return [0.05, 0.05, 0.05, 0.05, 0.6, 0.05]
    if label.startswith('dead ceiling (0.6; 0.05 elsewhere)'):
        return [0.05, 0.05, 0.05, 0.05, 0.05, 0.6]
    m = re.match(r'^[\w .]+: floor a([\d.]+), ceiling a([\d.]+), walls a([\d.]+)$', label)
    if m:
        f, c, w = (float(x) for x in m.groups())
        return [w, w, w, w, f, c]
    raise VoidRun('unknown walls label %r' % label)


def room_dims(name):
    a = [float(x) for x in name.split('x')]
    if len(a) != 3:
        raise VoidRun('room name %r is not AxBxC' % name)
    return a


class Ctx:
    """What every builder shares: the sources, the frozen method, synth, the target root."""

    def __init__(self, target, workers):
        self.target = Path(target)
        self.src = Sources(REPO, target)
        self.F = load_frozen(self.src)
        self.S = load_synth(self.src)
        self.workers = workers
        self.final = self.target / 'agents' / 'edt-simplify' / 'final'
        self.h = 0.31 / self.S.C
        self.timing = {}


def compare_logged(logged, recomputed):
    """Row by row: same status and the same edt (to 1e-12 relative) as the log."""
    same, differ = 0, []
    for rid, r in recomputed.items():
        lg = logged.get(rid)
        if lg is None:
            differ.append(dict(id=rid, why='not in the log'))
            continue
        ok = lg['status'] == r['status'] and (
            (lg['edt'] is None and r['edt'] is None) or
            (lg['edt'] is not None and r['edt'] is not None and abs(lg['edt'] - r['edt']) <= 1e-12 * abs(lg['edt'])))
        if ok:
            same += 1
        else:
            differ.append(dict(id=rid, logged=[lg['status'], lg['edt']], recomputed=[r['status'], r['edt']]))
    missing = [k for k in logged if k not in recomputed]
    return dict(logged=len(logged), recomputed=len(recomputed), same=same, differ=differ[:20], n_differ=len(differ),
                not_recomputed=missing[:20], n_not_recomputed=len(missing))


def status_counts(res):
    return dict(sorted(collections.Counter(r['status'] for r in res).items()))


def set_summary(rows_all, weak, verify=None, **more):
    out = dict(n_rows=len(rows_all), status=status_counts(rows_all),
               n_wrong_silent=sum(1 for r in weak if r['cls'] == 'wrong_silent'),
               n_near_miss=sum(1 for r in weak if r['cls'] == 'near_miss'))
    if verify is not None:
        out['verify_against_log'] = verify
    out.update(more)
    return out


# ---- t1 ----------------------------------------------------------------------------------------
def _unarr(d):
    return np.frombuffer(base64.b64decode(d['b64']), dtype=np.dtype(d['dtype'])).reshape(d['shape'])


def set_t1(ctx):
    logged = {r['id']: r for r in ctx.src.pickle(ctx.final / 'res_t1.pkl')}
    rec, weak, qs = {}, [], {}
    for line in ctx.src.text(ctx.target / 'agents' / 'edt-band' / 'golden' / 't1.jsonl').splitlines():
        if not line.strip():
            continue
        c = json.loads(line)
        inp = c['inputs']
        B = _unarr(inp['B']).astype(np.float64)
        dt, t_arr, hw = float(inp['dt']), float(inp['t_arr']), float(inp['half_width'])
        r = ctx.F.analyse(B, dt, t_arr, dict(half_width=hw))
        rec[c['id']] = r
        T60 = (c.get('truth') or {}).get('edt')         # a single exponential: its EDT is its T60
        qs[c['id']] = synth_q(T60=T60, dt=dt, t_arr=t_arr, R=hw * C_SPPS, run_s=len(B) * dt,
                              gap=float((c.get('setup') or {}).get('gap') or 0.0),
                              extra=dict(direct=(c.get('setup') or {}).get('direct'), air=(c.get('setup') or {}).get('air')))
    for rid, lg in logged.items():
        cls = weak_class(lg['status'], lg['edt'], lg['truth_edt'])
        if cls:
            weak.append(row_record('t1', rid, lg, lg['truth_edt'], qs.get(rid, {}), has_room=False, noise=False,
                                   truth_kind='exact'))
    return weak, set_summary(list(logged.values()), weak, compare_logged(logged, rec))


# ---- ISM -----------------------------------------------------------------------------------------
ISM_ID = re.compile(r'^ism\|([^|]+)\|\(([^)]*)\)\|(\d+)\|([\d.]+)ms$')


def set_ism(ctx, rooms):
    logged = {r['id']: r for r in ctx.src.pickle(ctx.final / 'res_ism.pkl')}
    inputs = ctx.src.pickle(ctx.final / 'dev' / 'ism_rows.pkl')
    rec, weak = {}, []
    c_ai = 343.20001220703125                                    # attack_ism.py:28
    for row in inputs:
        r = ctx.F.analyse(row['bins'], row['dt'], row['t_arrival'], dict(row['meta']))
        rec[row['id']] = r
        lg = logged[row['id']]
        cls = weak_class(lg['status'], lg['edt'], lg['truth_edt'])
        if not cls:
            continue
        name, rec_s, F, ms = ISM_ID.match(row['id']).groups()
        L, alpha, s, T = rooms[name]
        al6 = [alpha[0][0], alpha[0][1], alpha[1][0], alpha[1][1], alpha[2][0], alpha[2][1]]
        p = [float(x) for x in rec_s.split(',')]
        R = row['meta']['half_width'] * c_ai
        q = room_quantities(L, al6, s, p, R, float(F), c_ai)
        run_s = len(row['bins']) * row['dt']
        q.update(step_ms=row['dt'] * 1e3, run_s=run_s, run_over_t60=(run_s - row['t_arrival']) / q['t60_s'],
                 image_time_s=T)
        weak.append(row_record('ism', row['id'], lg, lg['truth_edt'], q, has_room=True, noise=False,
                               truth_kind='independent (image sources, Definition A)', extra=dict(room='ism:' + name)))
    return weak, set_summary(list(logged.values()), weak, compare_logged(logged, rec))


# ---- the real noise-cal seeds: res_real.pkl (truth: the shipped calculator) and the seeds scan ----
def real_cells(ctx):
    roots = [ctx.target / 'agents' / 'pm8-noise-scratch' / 'runs' / n for n in ('noise-cal-1790307822', 'noise-cal-1790310131')]
    out = []
    for root in roots:
        for c in ctx.src.json(root / 'cells.json'):
            cc = c if 'id' in c else c['cell']
            if abs(cc['time_step_s'] - 0.001) > 1e-9:
                continue
            out.append((root, cc))
    return out


def real_quantities(cc, ri, report, F, step):
    dims = room_dims(cc['room'])
    al6 = walls_alpha6(cc['walls'])
    c = report['speed_of_sound_m_s']
    R = report['receiver_radius_m']
    q = room_quantities(dims, al6, cc['source'], cc['receivers'][ri], R, F, c)
    q.update(step_ms=step * 1e3, particles=int(cc['particles_per_source']), method=cc['method'])
    return q


def same_result(a, b):
    return all(a.get(k) == b.get(k) for k in ('status', 'edt', 'edt_lo', 'edt_hi'))


def set_real_and_seeds(ctx):
    """res_real.pkl (each seed's 2 ms rebin against real_rows.pkl's truth, loaders.load_real) and the
    real-seeds scan (each seed against its 10-seed mean, run_scans.py:159-213), on the same inputs.
    critique/real_seeds.py built the seeds scan the same way but called the method with no meta (the
    default ball, 0.31 m / 343.2 m/s, for the report's R/c): every row is run both ways, and a row
    whose result differs is scored as real_seeds.py had it too."""
    logged = {r['id']: r for r in ctx.src.pickle(ctx.final / 'res_real.pkl')}
    rec, weak_real, weak_seeds = {}, [], []
    st = collections.Counter()
    bycell = collections.defaultdict(collections.Counter)
    seed_rows = []
    K = 2
    pos_checked, pos_worst = 0, 0.0
    meta_differ = collections.Counter()
    for root, cc in real_cells(ctx):
        if cc['id'] not in ALLOWED:
            continue
        paths = sorted(glob.glob(str(root / cc['id'] / 'seed*' / 'report.json')))
        runs = [ctx.src.json(p)['spps'] for p in paths]
        d0 = runs[0]
        hh = d0['receiver_radius_m'] / d0['speed_of_sound_m_s']
        for d in runs:                     # the receivers the quantities use are the ones each run had
            for ri, pr in enumerate(d['point_receivers']):
                gap = max(abs(a - b) for a, b in zip(pr['position_m'], cc['receivers'][ri]))
                pos_worst = max(pos_worst, gap)
                pos_checked += 1
                if gap > 1e-6:
                    raise VoidRun('%s receiver %d at %s in its report, %s in cells.json'
                                  % (cc['id'], ri, pr['position_m'], cc['receivers'][ri]))
        for ri, pr0 in enumerate(d0['point_receivers']):
            for bi, b0 in enumerate(pr0['bands']):
                ser = [np.asarray(d['point_receivers'][ri]['bands'][bi]['energy_pa2'], float) for d in runs]
                Lm = min(len(v) for v in ser)
                nb = Lm // K
                reb = [v[:nb * K].reshape(nb, K).sum(1) for v in ser]
                t_arr = pr0['arrival_s']
                dt = d0['time_step_s'] * K
                F = b0['freq_hz']
                per_seed, per_seed_c = [], []
                for si, v in enumerate(reb):
                    r = ctx.F.analyse(v, dt, t_arr, {'half_width': hh})
                    rc = ctx.F.analyse(v, dt, t_arr, {})          # real_seeds.py:46
                    meta_differ['seed_rows'] += not same_result(r, rc)
                    per_seed.append(r)
                    per_seed_c.append(rc)
                    rid = '|'.join(map(str, (cc['id'], si, pr0['label'], F)))
                    rec[rid] = r
                    lg = logged.get(rid)
                    if lg is not None and weak_class(lg['status'], lg['edt'], lg['truth_edt']):
                        q = real_quantities(cc, ri, d0, F, dt)
                        q.update(run_s=len(v) * dt, run_over_t60=(len(v) * dt - t_arr) / q['t60_s'])
                        weak_real.append(row_record('real', rid, lg, lg['truth_edt'], q, has_room=True, noise=True,
                                                    truth_kind="the shipped calculator's own reading at 1 ms (not independent, EVAL.md:18)",
                                                    extra=dict(room='noise_cal:' + cc['room'], cell=cc['id'], seed_folder=Path(paths[si]).parent.name)))
                mean = np.mean(reb, axis=0)
                rm = ctx.F.analyse(mean, dt, t_arr, {'half_width': hh})
                rmc = ctx.F.analyse(mean, dt, t_arr, {})              # real_seeds.py:41
                meta_differ['means'] += not same_result(rm, rmc)
                if rmc['edt'] is not None:
                    for si, (r, rc) in enumerate(zip(per_seed, per_seed_c)):
                        if same_result(r, rc) and same_result(rm, rmc):
                            continue                                  # scored below, as run_scans.py had it
                        cls = weak_class(rc['status'], rc['edt'], rmc['edt'])
                        if cls:
                            q = real_quantities(cc, ri, d0, F, dt)
                            q.update(run_s=len(reb[si]) * dt, run_over_t60=(len(reb[si]) * dt - t_arr) / q['t60_s'],
                                     method_half_width='default R 0.31 m (no meta)')
                            rid = '|'.join(map(str, ('seeds_nometa', cc['id'], si, pr0['label'], F)))
                            weak_seeds.append(row_record('seeds', rid, rc, rmc['edt'], q, has_room=True, noise=True,
                                                         truth_kind="the frozen method on the cell's 10-seed mean, "
                                                                    "no meta (real_seeds.py:41)",
                                                         extra=dict(room='noise_cal:' + cc['room'], cell=cc['id'],
                                                                    seed_folder=Path(paths[si]).parent.name)))
                if rm['edt'] is None:
                    st['mean_refused'] += 1
                    continue
                for si, r in enumerate(per_seed):
                    if r['edt'] is None:
                        st['refused'] += 1
                        continue
                    e = r['edt'] / rm['edt'] - 1
                    if r['status'] == 'ok':
                        kk = 'ok_WRONG' if abs(e) > JND else 'ok_good'
                        st[kk] += 1
                        bycell[cc['id']][kk] += 1
                        seed_rows.append(abs(e))
                        cls = weak_class('ok', r['edt'], rm['edt'])
                        if cls:
                            q = real_quantities(cc, ri, d0, F, dt)
                            q.update(run_s=len(reb[si]) * dt, run_over_t60=(len(reb[si]) * dt - t_arr) / q['t60_s'])
                            rid = '|'.join(map(str, ('seeds', cc['id'], si, pr0['label'], F)))
                            weak_seeds.append(row_record('seeds', rid, r, rm['edt'], q, has_room=True, noise=True,
                                                         truth_kind="the frozen method on the cell's 10-seed mean (run_scans.py:181-184)",
                                                         extra=dict(room='noise_cal:' + cc['room'], cell=cc['id'],
                                                                    seed_folder=Path(paths[si]).parent.name)))
                    else:
                        st['wide'] += 1
                        bycell[cc['id']]['wide'] += 1
    real_sum = set_summary(list(logged.values()), weak_real, compare_logged(logged, rec),
                           receivers_checked=dict(n=pos_checked, max_abs_diff_m=pos_worst,
                                                  what="each report's position_m against cells.json's receivers"))
    seeds_tally = dict(sorted(st.items()))
    seeds_sum = dict(n_rows=sum(st.values()), tally=seeds_tally,
                     by_cell={k: dict(sorted(v.items())) for k, v in sorted(bycell.items())},
                     n_wrong_silent=sum(1 for r in weak_seeds if r['cls'] == 'wrong_silent'),
                     n_near_miss=sum(1 for r in weak_seeds if r['cls'] == 'near_miss'),
                     real_seeds_py_no_meta=dict(seed_rows_differing=meta_differ['seed_rows'],
                                                means_differing=meta_differ['means'],
                                                what='results that change when the method is called as '
                                                     'critique/real_seeds.py calls it (no meta) instead of '
                                                     'with the report R/c; differing rows are scored both ways'))
    return weak_real, real_sum, weak_seeds, seeds_sum


# ---- the synthetic scans -------------------------------------------------------------------------
def synth_q(*, T60, dt, t_arr, R, run_s, gap=0.0, delay=0.0, drr_db=None, ratio=None, late_db=None, extra=None):
    q = dict(t60_s=T60, step_ms=dt * 1e3, d_m=t_arr * C_SPPS, R_m=R, d_minus_R_m=t_arr * C_SPPS - R,
             gap_ms=gap * 1e3, delay_ms=delay * 1e3, run_s=run_s,
             run_over_t60=(run_s - delay - t_arr) / T60 if T60 else None)
    if drr_db is not None:
        q['drr_db'] = drr_db
    if ratio is not None:
        q['ratio'] = ratio
        q['late_share_db'] = late_db
    if extra:
        q.update(extra)
    return q


def _same_truth(a, b, tol):
    if a is None or b is None:
        return a is b
    if not math.isfinite(a) or not math.isfinite(b):
        return (a != a and b != b) or a == b
    return abs(a / b - 1) <= tol


def check_against_output(name, got, ref, fields, tol=1e-9, exempt=None, exempt_why=None):
    """A construction's rows, in order, against the rows its source wrote: the grid values exactly and
    the truth to tol (relative). Raises VoidRun on any difference, so a wrong reconstruction never
    writes a weak-spot list. Rows for which exempt(row) is true have their truth compared and the
    differences counted and shown, not raised (exempt_why says why)."""
    if len(got) != len(ref):
        raise VoidRun('%s: %d rows here, %d in its output' % (name, len(got), len(ref)))
    worst = 0.0
    ex = collections.Counter()
    shown = []
    for i, (g, r) in enumerate(zip(got, ref)):
        if tuple(g[f] for f in fields) != tuple(r[f] for f in fields):
            raise VoidRun('%s row %d: %s here, %s in its output' % (name, i, [g[f] for f in fields], [r[f] for f in fields]))
        a, b = g['truth'], r['truth']
        same = _same_truth(a, b, tol)
        if exempt is not None and exempt(g):
            ex['same' if same else ('no truth here' if a is None or a != a else 'differs')] += 1
            if not same and len(shown) < 10:
                shown.append(dict(row=i, **{f: g[f] for f in fields}, truth_here=a, truth_in_output=b))
            continue
        if not same:
            raise VoidRun('%s row %d: truth %r here, %r in its output' % (name, i, a, b))
        if a is not None and b is not None and math.isfinite(a) and math.isfinite(b):
            worst = max(worst, abs(a / b - 1))
    out = dict(rows=len(got), fields_equal=list(fields), truth_tolerance=tol,
               truth_checked_rows=len(got) - sum(ex.values()), truth_max_rel_diff=worst)
    if exempt is not None:
        out['truth_not_held_to_output'] = dict(why=exempt_why, rows=sum(ex.values()), by_outcome=dict(sorted(ex.items())),
                                               first_differing=shown)
    return out


SCAN1_RUNS = dict(
    run_scans='final/run_scans.py:25-58, the frozen method\'s own scan 1: deduplicated, d >= 0.6 m, well-conditioned '
              '(level after the direct >= -8 dB); its tally is checked against scans_all.json',
    scan_i12='the committed critique/scan_i12.py: deduplicated, d >= 0.6 m, ill-conditioned rows kept (reported '
             'apart there); checked row by row against its output, critique/scan_i12_R0.31.json',
    early_run='the critique\'s first run of the grid, critique/scan_i12.json in target/: no d cutoff and no dedup, '
              '6,240 rows; its script was not kept (the committed one adds both). Checked row by row against it: every '
              'grid value, and every truth at d >= 0.6 m; below 0.6 m its truth function, also not kept, differs, so '
              'those rows are scored against the committed synth.truth_edt and their first-run truth is shown beside')


def scan1(ctx):
    """Scan 1, critique/scan_i12.py's grid (R 0.31 m, run 2 T60, Sabine direct share), in the three runs
    of it the corpus holds (SCAN1_RUNS). Each distinct input is scored once; a weak row names the runs
    it is in. Only the run_scans rows make the tally that scans_all.json checks."""
    S, F, h = ctx.S, ctx.F, ctx.h
    crit = ctx.target / 'agents' / 'edt-simplify' / 'critique'
    cache, seen_i12 = {}, set()
    rows_i12, rows_early = [], []
    tal = collections.defaultdict(collections.Counter)                   # run_scans, by step
    tal_i12 = collections.defaultdict(collections.Counter)               # scan_i12, by regime and step
    tal_early = collections.Counter()                                    # early run only (d < 0.6 m)
    for V, T60, d, gap_ms, dt_ms in itertools.product((200.0, 2000.0, 20000.0), (0.3, 0.6, 1.0, 2.0, 3.0),
                                                       (0.7, 1.0, 1.5, 2.0, 3.0, 5.0, 8.0, 12.0, 20.0, 30.0),
                                                       (0.0, 2.0, 5.0, 10.0), (1.0, 2.0, 5.0, 10.0)):
        if d > 1.6 * V ** (1 / 3):
            continue
        dt = dt_ms * 1e-3
        k = 6 * math.log(10) / T60
        for phase in (0.05, 0.5, 0.95):
            t_arr = (math.floor((d / S.C) / dt) + phase) * dt
            dd = t_arr * S.C
            ikey = (V, T60, t_arr, gap_ms, dt_ms)                          # the method's exact inputs
            c = cache.get(ikey)
            if c is None:
                Ed = S.sabine_direct_energy(V, T60, dd, 1.0 / k)
                truth = S.truth_edt(t_arr, Ed, gap_ms * 1e-3, [1.0], [k])
                b = S.histogram(dt, 2.0 * T60 + t_arr + gap_ms * 1e-3, t_arr, h, Ed, gap_ms * 1e-3, [1.0], [k])
                c = cache[ikey] = dict(V=V, T60=T60, d=d, gap_ms=gap_ms, dt_ms=dt_ms, phase=phase, t_arr=t_arr, k=k,
                                       Ed=Ed, lvl=10 * math.log10((1 / k) / (Ed + 1 / k)), truth=truth, nb=len(b),
                                       r=F.analyse(b, dt, t_arr, {}), runs=set(), n_early=0, early_index=len(rows_early))
            kind, e = score(c['r'], c['truth'])                         # run_scans' tally, as logged
            kind2 = kind_of(c['r'], c['truth'])[0]
            c['runs'].add('early_run')
            c['n_early'] += 1
            rows_early.append(dict(V=V, T60=T60, d=round(dd, 3), gap_ms=gap_ms, dt_ms=dt_ms, phase=phase, truth=c['truth'],
                                   dd=dd))
            if dd < 0.6:
                tal_early[kind2] += 1
                continue
            key = (V, T60, round(dd, 4), gap_ms, dt_ms)
            if key in seen_i12:
                continue
            seen_i12.add(key)
            c['runs'].add('scan_i12')
            rows_i12.append(dict(V=V, T60=T60, d=round(dd, 3), gap_ms=gap_ms, dt_ms=dt_ms, phase=phase, truth=c['truth']))
            well = c['lvl'] >= -8
            tal_i12['%s %g ms' % ('well-conditioned' if well else 'ill-conditioned', dt_ms)][kind2] += 1
            if well:
                c['runs'].add('run_scans')
                tal[dt_ms][kind] += 1
    fields = ('V', 'T60', 'd', 'gap_ms', 'dt_ms', 'phase')
    check_i12 = check_against_output('scan_i12', rows_i12, ctx.src.json(crit / 'scan_i12_R0.31.json'), fields)
    ref_early = ctx.src.json(crit / 'scan_i12.json')
    check_early = check_against_output(
        'early_run', rows_early, ref_early, fields, exempt=lambda g: g['dd'] < 0.6,
        exempt_why='the first run\'s truth function was not kept: below d 0.6 m, where the direct sound dominates, '
                   'its truths differ from the committed synth.truth_edt (which scores these rows here), e.g. EDT 12.5 s '
                   'for T60 0.3 s; at d >= 0.6 m every truth matches')
    weak = []
    for c in cache.values():
        r = c['r']
        if not weak_class(r['status'], r['edt'], c['truth']):
            continue
        dt = c['dt_ms'] * 1e-3
        q = synth_q(T60=c['T60'], dt=dt, t_arr=c['t_arr'], R=0.31, run_s=c['nb'] * dt, gap=c['gap_ms'] * 1e-3,
                    drr_db=10 * math.log10(c['Ed'] * c['k']),
                    extra=dict(V_formula_m3=c['V'], phase=c['phase'], d_grid_m=c['d'], level_after_direct_db=c['lvl']))
        runs = [x for x in SCAN1_RUNS if x in c['runs']]
        extra = dict(runs=runs, rows_in_early_run=c['n_early'])
        if 'scan_i12' not in c['runs']:
            extra['early_run_truth'] = ref_early[c['early_index']]['truth']       # the first run's own, not kept
        if not histogram_holds_truth(q):
            # HARNESS-PLAN.md 8.2, second call: the generator's own defect (d < R), not the method's.
            # The row stays in weak_spots.json (575 rows, T22a) but does not bind d_m or d_minus_R_m.
            extra['generator_defect'] = ('d_m < R_m: synth.ball_atoms floors rho and the generator '
                                         'drops the direct sound; this row does not bind a bound (P33)')
            q = dict(q, d_m=None, d_minus_R_m=None)
        weak.append(row_record('scan1', 'scan1|V%g|T%g|d%g|gap%g|%gms|ph%g' % (c['V'], c['T60'], c['d'], c['gap_ms'],
                                                                              c['dt_ms'], c['phase']),
                               r, c['truth'], q, has_room=False, noise=False, truth_kind='independent (synth.truth_edt)',
                               extra=extra))
    tally = {str(k): dict(sorted(v.items())) for k, v in sorted(tal.items())}
    by_run = collections.Counter()
    for c in cache.values():
        for x in c['runs']:
            by_run[x] += 1
    return weak, dict(n_rows=sum(sum(v.values()) for v in tal.values()), tally=tally,
                      runs=SCAN1_RUNS, distinct_inputs=len(cache), distinct_inputs_by_run=dict(sorted(by_run.items())),
                      scan_i12=dict(n_rows=len(rows_i12), tally={k: dict(sorted(v.items())) for k, v in sorted(tal_i12.items())},
                                    check=check_i12),
                      early_run=dict(n_rows=len(rows_early), tally_d_below_0p6=dict(sorted(tal_early.items())),
                                     check=check_early))


def scan4_occluded(ctx):
    S, F, h = ctx.S, ctx.F, ctx.h
    weak = []
    tal = collections.defaultdict(collections.Counter)
    for T60, delay_ms, dt_ms, dir_db in itertools.product((0.4, 0.8, 1.5, 3.0), (2.0, 5.0, 10.0, 20.0, 40.0),
                                                          (1.0, 2.0, 10.0), (None, -20.0, -10.0)):
        dt = dt_ms * 1e-3
        k = 6 * math.log(10) / T60
        t_arr = 0.030 + 0.41 * dt
        gap = delay_ms * 1e-3
        Ed = 0.0 if dir_db is None else (1.0 / k) * 10 ** (dir_db / 10)
        truth = S.truth_edt(t_arr + gap, 0.0, 0.0, [1.0], [k]) if Ed == 0 else S.truth_edt(t_arr, Ed, gap, [1.0], [k])
        b = S.histogram(dt, t_arr + gap + 2.0 * T60, t_arr, h, Ed, gap, [1.0], [k])
        r = F.analyse(b, dt, t_arr, {})
        kind, e = score(r, truth)
        tal[str(dir_db)][kind] += 1
        if weak_class(r['status'], r['edt'], truth):
            q = synth_q(T60=T60, dt=dt, t_arr=t_arr, R=0.31, run_s=len(b) * dt, gap=gap, drr_db=dir_db,
                        extra=dict(blocked=dir_db is None))
            weak.append(row_record('scan4', 'scan4|T%g|late%gms|%gms|dir%s' % (T60, delay_ms, dt_ms, dir_db), r, truth, q,
                                   has_room=False, noise=False, truth_kind='independent (synth.truth_edt)'))
    tally = {k: dict(sorted(v.items())) for k, v in tal.items()}
    return weak, dict(n_rows=sum(sum(v.values()) for v in tal.values()), tally=tally)


def _double(shape, k1, ratio):
    if shape == 'single':
        return [1.0], [k1], None
    lvl = float(shape.split('-')[1])
    k2 = k1 / ratio
    E1 = 1.0 / k1
    E2 = E1 * 10 ** (-lvl / 10)
    return [1.0, E2 * k2], [k1, k2], -lvl


SCAN2B_FORMS = (
    ('run_scans', {'half_width': 0.0}, 'final/run_scans.py:81-121: the method told half_width 0 for the point-receiver '
     'histogram; a row whose own long run is not within 5 % of the truth leaves the tally, as there (its tally is '
     'checked against scans_all.json)'),
    ('scan_i4b', {}, 'the committed critique/scan_i4b.py: the method called as common.run_all calls it, with no meta, '
     'so it takes its default ball (0.31 m) for the point-receiver histogram; every row, the long-run filter counted'))


def scan2b(ctx):
    """Scan 2b, rate ratio 3, point receiver, in its two forms (SCAN2B_FORMS). Weak rows come from every
    row of both, whatever the long run gives: the filter is how the critique reported, not a row's input."""
    S, F = ctx.S, ctx.F
    weak = []
    tal = collections.defaultdict(collections.Counter)
    tal_i4b = collections.Counter()
    refused_long = collections.Counter()
    differ = 0
    for T60, dt_ms, frac, shape, drr_db in itertools.product(
            (0.5, 1.0, 2.0, 4.0, 8.0), (1.0, 2.0, 10.0),
            (0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.5, 0.6, 0.8, 1.0, 1.5),
            ('single', 'double-20', 'double-15', 'double-10'), (-12.0, -6.0, -3.0)):
        dt = dt_ms * 1e-3
        k1 = 6 * math.log(10) / T60
        A, k, late = _double(shape, k1, 3.0)
        t_arr = 0.02 + 0.37 * dt
        Srev = sum(a / kk for a, kk in zip(A, k))
        Ed = Srev * 10 ** (drr_db / 10)
        truth = S.truth_edt(t_arr, Ed, 0.0, A, k)
        T_long = t_arr + 150.0 / 60.0 * T60 * (3.0 if shape != 'single' else 1.0)
        b_long = S.histogram(dt, T_long, t_arr, 0.0, Ed, 0.0, A, k)
        b = S.histogram(dt, t_arr + frac * T60, t_arr, 0.0, Ed, 0.0, A, k)
        if len(b) < 4:
            continue
        res = {}
        for form, meta, _ in SCAN2B_FORMS:
            rl = F.analyse(b_long, dt, t_arr, meta)
            long_ok = not (rl['edt'] is None or abs(rl['edt'] / truth - 1) > 0.05)     # run_scans.py:105, as written
            r = res[form] = F.analyse(b, dt, t_arr, meta)
            kind, e = score(r, truth) if form == 'run_scans' else kind_of(r, truth)
            if not long_ok:
                refused_long[form] += 1
            if form == 'run_scans':
                if long_ok:
                    tal['final'][kind] += 1
                    tal['final_by_frac_%.2f' % frac][kind] += 1
            else:
                tal_i4b[kind] += 1
            if weak_class(r['status'], r['edt'], truth):
                q = synth_q(T60=T60, dt=dt, t_arr=t_arr, R=0.0, run_s=len(b) * dt, drr_db=drr_db,
                            ratio=None if late is None else 3.0, late_db=late,
                            extra=dict(frac=frac, method_half_width='0' if meta else 'default R 0.31 m (no meta)',
                                       long_run_within_5pct=long_ok))
                weak.append(row_record('scan2b', 'scan2b|T%g|%gms|f%g|%s|drr%g|%s' % (T60, dt_ms, frac, shape, drr_db, form),
                                       r, truth, q, has_room=False, noise=False, truth_kind='independent (synth.truth_edt)',
                                       extra=dict(form=form)))
        differ += not same_result(res['run_scans'], res['scan_i4b'])
    tally = {k: dict(sorted(v.items())) for k, v in sorted(tal.items())}
    return weak, dict(n_rows=sum(tal['final'].values()), long_run_not_within_5pct=refused_long['run_scans'], tally=tally,
                      forms={f: why for f, _, why in SCAN2B_FORMS},
                      scan_i4b=dict(n_rows=sum(tal_i4b.values()), tally=dict(sorted(tal_i4b.items())),
                                    long_run_not_within_5pct=refused_long['scan_i4b'],
                                    rows_whose_result_differs_from_run_scans=differ))


def scan2(ctx):
    """critique/scan_i4.py (scan 2): rate ratio 4, the candidates' default meta (none), point direct."""
    S, F = ctx.S, ctx.F
    weak = []
    tal = collections.defaultdict(collections.Counter)
    for T60, dt_ms, frac, shape, drr_db in itertools.product(
            (0.5, 1.0, 2.0, 4.0, 8.0), (1.0, 2.0, 10.0),
            (0.12, 0.15, 0.18, 0.2, 0.22, 0.25, 0.3, 0.35, 0.4, 0.5, 0.6, 0.8, 1.0, 1.5),
            ('single', 'double-15', 'double-10', 'double-5'), (-12.0, -3.0, 0.0)):
        dt = dt_ms * 1e-3
        k1 = 6 * math.log(10) / T60
        A, k, late = _double(shape, k1, 4.0)
        t_arr = 0.02 + 0.37 * dt
        gap = 0.0
        Srev = sum(a / kk for a, kk in zip(A, k))
        Ed = Srev * 10 ** (drr_db / 10)
        truth = S.truth_edt(t_arr, Ed, gap, A, k)
        T_run = t_arr + frac * T60
        b = S.histogram(dt, T_run, t_arr, 0.0, Ed, gap, A, k)
        if len(b) < 4:
            continue
        r = F.analyse(np.asarray(b, float), dt, t_arr, {})
        kind, e = kind_of(r, truth)
        tal[str(dt_ms)][kind] += 1
        if weak_class(r['status'], r['edt'], truth):
            q = synth_q(T60=T60, dt=dt, t_arr=t_arr, R=0.0, run_s=len(b) * dt, drr_db=drr_db,
                        ratio=None if late is None else 4.0, late_db=late, extra=dict(frac=frac, method_half_width='default R 0.31 m'))
            weak.append(row_record('scan2', 'scan2|T%g|%gms|f%g|%s|drr%g' % (T60, dt_ms, frac, shape, drr_db), r, truth, q,
                                   has_room=False, noise=False, truth_kind='independent (synth.truth_edt)'))
    tally = {k: dict(sorted(v.items())) for k, v in sorted(tal.items())}
    return weak, dict(n_rows=sum(sum(v.values()) for v in tal.values()), tally=tally)


I5_CONFIGS = [(200, 0.6, 2.0, 150e3, 1.0), (200, 0.6, 5.0, 150e3, 1.0), (200, 0.6, 5.0, 150e3, 10.0),
              (2000, 1.5, 5.0, 150e3, 1.0), (2000, 1.5, 15.0, 150e3, 1.0), (2000, 1.5, 15.0, 150e3, 10.0),
              (20000, 2.5, 10.0, 150e3, 1.0), (20000, 2.5, 30.0, 150e3, 1.0), (20000, 2.5, 30.0, 150e3, 10.0),
              (20000, 2.5, 30.0, 1.5e6, 1.0), (20000, 2.5, 30.0, 15e3, 1.0), (2000, 1.5, 15.0, 15e3, 1.0)]


def scan3_noise(ctx):
    """run_scans.py 'i5': random-mode compound-Poisson noise, 200 seeds per configuration."""
    S, F, h = ctx.S, ctx.F, ctx.h
    rng = np.random.default_rng(20260927)
    tot = collections.Counter()
    per_cfg, weak = [], []
    for V, T60, d, N, dt_ms in I5_CONFIGS:
        dt = dt_ms * 1e-3
        k = 6 * math.log(10) / T60
        t_arr = d / S.C
        Ed = S.sabine_direct_energy(V, T60, d, 1.0 / k)
        truth = S.truth_edt(t_arr, Ed, 0.0, [1.0], [k])
        T_run = t_arr + 1.5 * T60
        lam0 = N * math.pi * 0.31 ** 2 * S.C / V
        lam_d = N * 0.31 ** 2 / (4 * d * d)
        b_dir = S.histogram(dt, T_run, t_arr, h, Ed, 1e9, [0.0], [k])
        b_rev = S.histogram(dt, T_run, t_arr, h, 0.0, 0.0, [1.0], [k])
        per = collections.Counter()
        for s in range(200):
            b = S.noisy(b_dir, Ed / lam_d, rng) + S.noisy(b_rev, 1.0 / lam0, rng)
            r = F.analyse(b, dt, t_arr, {})
            kind, e = score(r, truth)
            per[kind] += 1
            tot[kind] += 1
            if weak_class(r['status'], r['edt'], truth):
                q = synth_q(T60=T60, dt=dt, t_arr=t_arr, R=0.31, run_s=len(b) * dt, drr_db=10 * math.log10(Ed * k),
                            extra=dict(V_formula_m3=V, particles=int(N), seed_index=s))
                weak.append(row_record('scan3', 'scan3|V%g|T%g|d%g|N%g|%gms|s%d' % (V, T60, d, N, dt_ms, s), r, truth, q,
                                       has_room=False, noise=True, truth_kind='independent (synth.truth_edt, noise-free)'))
        per_cfg.append(dict(V=V, T60=T60, d=d, N=N, dt_ms=dt_ms, tally=dict(sorted(per.items()))))
    return weak, dict(n_rows=sum(tot.values()), tally=dict(sorted(tot.items())), by_config=per_cfg)


def scan5(ctx):
    """critique/scan_misc.py (a) energetic-mode noise and (b) two sources; (c) has no truth."""
    S, F, h = ctx.S, ctx.F, ctx.h
    R = 0.31
    rng = np.random.default_rng(7)
    weak = []
    tal = collections.defaultdict(collections.Counter)
    for N in (150e3, 1.5e6):
        V, T60, d, dt = 20000.0, 2.5, 10.0, 1e-3
        k = 6 * math.log(10) / T60
        t_arr = d / S.C
        Ed = S.sabine_direct_energy(V, T60, d, 1.0 / k)
        truth = S.truth_edt(t_arr, Ed, 0.0, [1.0], [k])
        b_dir = S.histogram(dt, t_arr + 1.5 * T60, t_arr, h, Ed, 1e9, [0.0], [k])
        b_rev = S.histogram(dt, t_arr + 1.5 * T60, t_arr, h, 0.0, 0.0, [1.0], [k])
        lam_bin = N * math.pi * R * R * S.C / V * dt
        lam_d = N * R * R / (4 * d * d)
        for s in range(200):
            rev = np.where(b_rev > 0, b_rev * rng.poisson(lam_bin, size=b_rev.size) / lam_bin, 0.0)
            b = S.noisy(b_dir, Ed / lam_d, rng) + rev
            r = F.analyse(b, dt, t_arr, {})
            kind, e = kind_of(r, truth)
            tal['a N=%g' % N][kind] += 1
            if weak_class(r['status'], r['edt'], truth):
                q = synth_q(T60=T60, dt=dt, t_arr=t_arr, R=R, run_s=len(b) * dt, drr_db=10 * math.log10(Ed * k),
                            extra=dict(V_formula_m3=V, particles=int(N), seed_index=s, method='energetic'))
                weak.append(row_record('scan5a', 'scan5a|N%g|s%d' % (N, s), r, truth, q, has_room=False, noise=True,
                                       truth_kind='independent (synth.truth_edt, noise-free)'))
    for d1, d2, T60, V in ((3.0, 8.0, 1.0, 1000.0), (2.0, 12.0, 1.5, 3000.0), (1.0, 6.0, 0.6, 300.0), (4.0, 20.0, 2.0, 10000.0)):
        dt = 1e-3
        k = 6 * math.log(10) / T60
        t1, t2 = d1 / S.C, d2 / S.C
        E1 = S.sabine_direct_energy(V, T60, d1, 1.0 / k)
        E2 = S.sabine_direct_energy(V, T60, d2, 1.0 / k)
        atoms = S.ball_atoms(t2, h, E2)
        truth = S.truth_edt(t1, E1, 0.0, [2.0], [k], extra_atoms=atoms)
        b = S.histogram(dt, t1 + 2 * T60, t1, h, E1, 0.0, [2.0], [k], extra_atoms=atoms)
        r = F.analyse(np.asarray(b, float), dt, t1, {})
        kind, e = kind_of(r, truth)
        tal['b two sources'][kind] += 1
        if weak_class(r['status'], r['edt'], truth):
            q = synth_q(T60=T60, dt=dt, t_arr=t1, R=R, run_s=len(b) * dt, drr_db=10 * math.log10(E1 * k / 2.0),
                        extra=dict(V_formula_m3=V, d2_m=d2, sources=2))
            weak.append(row_record('scan5b', 'scan5b|d%g|d%g|T%g' % (d1, d2, T60), r, truth, q, has_room=False, noise=False,
                                   truth_kind='independent (synth.truth_edt with extra atoms)'))
    tally = {k: dict(sorted(v.items())) for k, v in tal.items()}
    return weak, dict(n_rows=sum(sum(v.values()) for v in tal.values()), tally=tally,
                      not_scored='(c) degenerate inputs: no truth')


NOISE_PROBE_CONFIGS = [(200, 0.6, 5.0, 150e3, 1.0), (2000, 1.5, 15.0, 150e3, 1.0), (20000, 2.5, 30.0, 150e3, 1.0),
                       (20000, 2.5, 30.0, 1.5e6, 1.0), (2000, 1.5, 15.0, 150e3, 10.0)]     # noise_probe.py:17-18


def noise_probe(ctx):
    """critique/noise_probe.py's rows: five of scan 3's configurations, 200 seeds each from its own
    generator seed (11), drawn in its order. noise_probe.py measured the seed spread and kept no truth;
    the truth is scan_i5.py's for the same configuration, the noise-free Definition-A EDT."""
    S, F, h = ctx.S, ctx.F, ctx.h
    R = 0.31
    rng = np.random.default_rng(11)
    tot = collections.Counter()
    per_cfg, weak = [], []
    for V, T60, d, N, dt_ms in NOISE_PROBE_CONFIGS:
        dt = dt_ms * 1e-3
        k = 6 * math.log(10) / T60
        t_arr = d / S.C
        Ed = S.sabine_direct_energy(V, T60, d, 1.0 / k)
        truth = S.truth_edt(t_arr, Ed, 0.0, [1.0], [k])                  # scan_i5.py:28-29
        T_run = t_arr + 1.5 * T60
        b_dir = S.histogram(dt, T_run, t_arr, h, Ed, 1e9, [0.0], [k])
        b_rev = S.histogram(dt, T_run, t_arr, h, 0.0, 0.0, [1.0], [k])
        lam0 = N * math.pi * R * R * S.C / V
        lam_d = N * R * R / (4 * d * d)
        per = collections.Counter()
        for s in range(200):
            b = S.noisy(b_dir, Ed / lam_d, rng) + S.noisy(b_rev, 1.0 / lam0, rng)
            r = F.analyse(b, dt, t_arr, {})
            kind, e = kind_of(r, truth)
            per[kind] += 1
            tot[kind] += 1
            if weak_class(r['status'], r['edt'], truth):
                q = synth_q(T60=T60, dt=dt, t_arr=t_arr, R=R, run_s=len(b) * dt, drr_db=10 * math.log10(Ed * k),
                            extra=dict(V_formula_m3=V, particles=int(N), seed_index=s, generator_seed=11))
                weak.append(row_record('noise_probe', 'noise_probe|V%g|T%g|d%g|N%g|%gms|s%d' % (V, T60, d, N, dt_ms, s), r,
                                       truth, q, has_room=False, noise=True,
                                       truth_kind='independent (synth.truth_edt, noise-free, as scan_i5.py)'))
        per_cfg.append(dict(V=V, T60=T60, d=d, N=N, dt_ms=dt_ms, tally=dict(sorted(per.items()))))
    return weak, dict(n_rows=sum(tot.values()), tally=dict(sorted(tot.items())), by_config=per_cfg)


def ablate_rows(ctx):
    """critique/ablate.py's gen_rows (lines 19-45): scan 1's direct and origin grid, not deduplicated, cut
    at 0.3, 0.5 and 2.0 T60, and 12 blocked receivers (no direct, point receiver); its t1 golden rows are
    the t1 set. ablate.py compared two versions of each candidate and kept no truth; the truth is the one
    the parent scan gives the same construction, the infinite run's Definition-A EDT (scan_i12.py for the
    grid; scan_i1_occluded.py's 0 dB at the first arrival for the blocked rows). Each distinct input is
    scored once."""
    S, F = ctx.S, ctx.F
    R = 0.31
    h = R / S.C
    tal = collections.defaultdict(collections.Counter)
    cache = {}
    n = 0
    for V, T60, d, gap_ms, dt_ms in itertools.product((200.0, 2000.0, 20000.0), (0.3, 0.6, 1.0, 2.0, 3.0),
                                                       (0.7, 1.5, 3.0, 8.0, 20.0), (0.0, 5.0), (1.0, 2.0, 10.0)):
        if d > 1.6 * V ** (1 / 3):
            continue
        dt = dt_ms * 1e-3
        k = 6 * math.log(10) / T60
        for phase in (0.05, 0.5, 0.95):
            t_arr = (math.floor((d / S.C) / dt) + phase) * dt
            if t_arr * S.C < 0.6:
                continue
            Ed = S.sabine_direct_energy(V, T60, t_arr * S.C, 1.0 / k)
            for frac in (0.3, 0.5, 2.0):
                n += 1
                ikey = ('grid', V, T60, t_arr, gap_ms, dt_ms, frac)
                if ikey not in cache:
                    truth = S.truth_edt(t_arr, Ed, gap_ms * 1e-3, [1.0], [k])
                    b = S.histogram(dt, t_arr + frac * T60, t_arr, h, Ed, gap_ms * 1e-3, [1.0], [k])
                    cache[ikey] = dict(r=F.analyse(b, dt, t_arr, {}), truth=truth, nb=len(b), dt=dt, t_arr=t_arr, T60=T60,
                                       gap=gap_ms * 1e-3, R=R, drr_db=10 * math.log10(Ed * k),
                                       id='ablate|V%g|T%g|d%g|gap%g|%gms|ph%g|f%g' % (V, T60, d, gap_ms, dt_ms, phase, frac),
                                       extra=dict(V_formula_m3=V, phase=phase, d_grid_m=d, frac=frac))
                c = cache[ikey]
                tal['grid f%g' % frac][kind_of(c['r'], c['truth'])[0]] += 1
    for T60, delay_ms, dt_ms in itertools.product((0.4, 0.8, 1.5), (5.0, 20.0), (1.0, 10.0)):
        n += 1
        dt = dt_ms * 1e-3
        k = 6 * math.log(10) / T60
        t_arr = 0.03 + 0.41 * dt
        truth = S.truth_edt(t_arr + delay_ms * 1e-3, 0.0, 0.0, [1.0], [k])     # scan_i1_occluded.py:20
        b = S.histogram(dt, t_arr + 2 * T60, t_arr, 0.0, 0.0, delay_ms * 1e-3, [1.0], [k])
        c = cache[('blocked', T60, delay_ms, dt_ms)] = dict(
            r=F.analyse(b, dt, t_arr, {}), truth=truth, nb=len(b), dt=dt, t_arr=t_arr, T60=T60, gap=delay_ms * 1e-3, R=0.0,
            drr_db=None, id='ablate|blocked|T%g|late%gms|%gms' % (T60, delay_ms, dt_ms),
            extra=dict(blocked=True, method_half_width='default R 0.31 m (no meta)'))
        tal['blocked'][kind_of(c['r'], c['truth'])[0]] += 1
    weak = []
    for c in cache.values():
        r = c['r']
        if weak_class(r['status'], r['edt'], c['truth']):
            q = synth_q(T60=c['T60'], dt=c['dt'], t_arr=c['t_arr'], R=c['R'], run_s=c['nb'] * c['dt'], gap=c['gap'],
                        drr_db=c['drr_db'], extra=c['extra'])
            weak.append(row_record('ablate', c['id'], r, c['truth'], q, has_room=False, noise=False,
                                   truth_kind='independent (synth.truth_edt of the infinite run)'))
    return weak, dict(n_rows=n, distinct_inputs=len(cache), tally={k: dict(sorted(v.items())) for k, v in tal.items()},
                      not_scored='its t1 golden rows: the t1 set')


# ---- z3 and z3grid (the judge's generator, through the text copy) --------------------------------
Z3GRID_MS = (2.0, 1.0, 0.5, 0.2, 0.1)                    # eval_run.py:388


def z3_group_key(c):
    """eval_run.py:449-451."""
    return (tuple(round(x, 12) for x in c['L']), tuple(round(x, 12) for x in c['al6']),
            tuple(round(x, 12) for x in c['S']), tuple(round(x, 12) for x in c['P']),
            round(float(c['R']), 12), round(float(c['c']), 9), round(float(c['m']), 15))


_WORKER = {}


def _worker_init():
    sys.dont_write_bytecode = True
    _WORKER['F'] = load_frozen()
    from m8b import _z3echo
    _WORKER['E'] = _z3echo


def _z3_quantities(c, ec, F_hz, dt, delay_steps, nbins):
    q = room_quantities(c['L'], c['al6'], c['S'], c['P'], float(c['R']), F_hz, float(c['c']))
    run_s = nbins * dt
    e = delay_steps * dt
    q.update(step_ms=dt * 1e3, delay_ms=e * 1e3, run_s=run_s, run_over_t60=(run_s - e - ec.t) / q['t60_s'],
             drr_db=10 * math.log10(ec.E_dir / (ec.top - ec.E_dir)) if ec.top > ec.E_dir else None,
             c_case_m_s=float(c['c']), m_case_np_per_m=float(c['m']), image_time_s=float(ec.T), images=int(len(ec.D)),
             gap_case_ms=1e3 * (ec.d1 - ec.d) / ec.c)
    return q


def _z3_task(args):
    """One robust geometry: (1) each of its z3 cases as loaders.load_z3 builds it (the case's own Echo,
    its step and delay); (2) z3grid as eval_run.z3_job builds it (the first case's Echo, no delay)."""
    gid, cases = args
    F, E = _WORKER['F'], _WORKER['E']
    t0 = time.time()
    cache = {}

    def echo(c):
        T = (c.get('echo') or {}).get('T')
        key = (tuple(c['L']), tuple(c['al6']), tuple(c['S']), tuple(c['P']), float(c['R']), c['c'], c['m'], T)
        if key not in cache:
            cache[key] = E.Echo(c['L'], c['al6'], c['S'], c['P'], float(c['R']), c['c'], c['m'], T=T)
        return cache[key]

    z3_rows, grid_rows = [], []
    for cs in cases:
        ec = echo(cs)
        dt = float(cs['dt'])
        dl = int(cs['delay'])
        v = ec.run(dt, 'step', delay_steps=dl)
        t_arr = dl * dt + ec.t
        r = F.analyse(np.asarray(v, dtype=np.float64), dt, t_arr, {'half_width': ec.h})
        if cs['q'] == 'edt':
            truth = cs['truthA']['0.005ms']
        else:
            truth = ec.truth(dtfs=(1e-5,), bottoms=(-10.0,))['edt_0.01ms']['-10.0']
        z3_rows.append(dict(id=cs['id'], res=r, truth=truth, q=_z3_quantities(cs, ec, float(cs['F']), dt, dl, len(v)),
                            extra=dict(q_tag=cs['q'], mechanism=cs.get('mechanism'), mode=cs.get('mode'))))
    c0 = cases[0]
    ec = echo(c0)
    truth = None
    for cs in cases:
        if cs['q'] == 'edt':
            truth = cs['truthA']['0.005ms']
    truth_src = 'judge truthA 0.005 ms'
    if truth is None:
        truth = ec.truth(dtfs=(1e-5,), bottoms=(-10.0,))['edt_0.01ms']['-10.0']
        truth_src = 'judge ideal reading at 0.01 ms (no EDT case in this geometry)'
    for dms in Z3GRID_MS:
        dt = dms * 1e-3
        v = ec.run(dt, 'step', delay_steps=0)
        r = F.analyse(np.asarray(v, dtype=np.float64), dt, ec.t, {'half_width': ec.h})
        grid_rows.append(dict(id='z3grid|%s|%gms' % (gid, dms), res=r, truth=truth, truth_src=truth_src,
                              q=_z3_quantities(c0, ec, float(c0['F']), dt, 0, len(v)), extra=dict(geometry_of=gid, cases=[c['id'] for c in cases])))
    return dict(gid=gid, z3=z3_rows, grid=grid_rows, seconds=time.time() - t0)


def set_z3(ctx):
    from multiprocessing import Pool
    jr_path = ctx.target / 'agents' / 'z3-hunt' / 'judge' / 'judge_results.json'
    jr = ctx.src.json(jr_path)
    echo_check = check_z3echo(ctx.src, ctx.target / 'agents' / 'z3-hunt' / 'judge' / 'judge.py')
    box_of = {}                                      # case id -> its corpus_rooms.json id (z3_rooms' order)
    for c in jr['summary']['confirmed']:
        box_of.setdefault(tuple(round(float(x), 9) for x in c['L']), 'z3:box%03d' % len(box_of))
    room = {c['id']: box_of[tuple(round(float(x), 9) for x in c['L'])] for c in jr['summary']['confirmed']}
    robust = [c for c in jr['summary']['confirmed'] if c.get('robust_both')]
    groups = collections.OrderedDict()
    for c in robust:
        groups.setdefault(z3_group_key(c), []).append(c)
    tasks = [(cl[0]['id'], cl) for cl in groups.values()]
    logged = {r['id']: r for r in ctx.src.pickle(ctx.final / 'res_z3.pkl')}
    results = []
    t0 = time.time()
    with Pool(ctx.workers, initializer=_worker_init, maxtasksperchild=8) as pool:
        for i, res in enumerate(pool.imap_unordered(_z3_task, tasks)):
            results.append(res)
            if (i + 1) % 25 == 0:
                print('  z3: %d/%d geometries, %.0f s' % (i + 1, len(tasks), time.time() - t0), flush=True)
    order = {t[0]: i for i, t in enumerate(tasks)}
    results.sort(key=lambda r: order[r['gid']])
    rec, weak_z3, weak_grid, grid_all = {}, [], [], []
    truth_check = dict(same=0, differ=[])
    for res in results:
        for row in res['z3']:
            rec[row['id']] = row['res']
            lg = logged[row['id']]
            t_lg, t_new = lg['truth_edt'], row['truth']
            if (t_lg is None and t_new is None) or (t_lg is not None and t_new is not None and
                                                      ((not math.isfinite(t_lg) and not math.isfinite(t_new)) or t_lg == t_new)):
                truth_check['same'] += 1
            else:
                truth_check['differ'].append(dict(id=row['id'], logged=t_lg, recomputed=t_new))
            if weak_class(lg['status'], lg['edt'], t_lg):
                weak_z3.append(row_record('z3', row['id'], lg, t_lg, row['q'], has_room=True, noise=False,
                                          truth_kind='independent (judge truth A, or its ideal reading for Ts cases)',
                                          extra=dict(row['extra'], room=room[row['id']])))
        for row in res['grid']:
            grid_all.append(row)
            r = row['res']
            if weak_class(r['status'], r['edt'], row['truth']):
                weak_grid.append(row_record('z3grid', row['id'], r, row['truth'], row['q'], has_room=True, noise=False,
                                            truth_kind=row['truth_src'],
                                            extra=dict(row['extra'], room=room[row['extra']['geometry_of']])))
    verify = compare_logged(logged, rec)
    verify['truth_same'] = truth_check['same']
    verify['truth_differ'] = truth_check['differ'][:20]
    z3_sum = set_summary(list(logged.values()), weak_z3, verify, generator=echo_check)
    tal = collections.defaultdict(collections.Counter)
    for row in grid_all:
        kind, e = kind_of(row['res'], row['truth'])
        tal[row['id'].rsplit('|', 1)[1]][kind] += 1
    no_truth = collections.OrderedDict()
    for row in grid_all:
        if row['truth'] is None or not math.isfinite(row['truth']):
            no_truth.setdefault(row['extra']['geometry_of'], row['truth_src'])
    grid_sum = dict(n_rows=len(grid_all), status=status_counts([r['res'] for r in grid_all]),
                    tally={k: dict(sorted(v.items())) for k, v in tal.items()},
                    geometries_without_a_finite_truth=[dict(geometry_of=g, truth_src=s) for g, s in no_truth.items()],
                    n_wrong_silent=sum(1 for r in weak_grid if r['cls'] == 'wrong_silent'),
                    n_near_miss=sum(1 for r in weak_grid if r['cls'] == 'near_miss'),
                    geometries=len(tasks), generator=echo_check)
    ctx.timing['z3_worker_seconds'] = round(sum(r['seconds'] for r in results), 1)
    return weak_z3, z3_sum, weak_grid, grid_sum


# ---- the scans' logged tallies -------------------------------------------------------------------
def check_scans_all(ctx, got):
    """The recomputed tallies of run_scans.py's five scans against the frozen method's ('final') tallies
    it logged (final/scans_all.json)."""
    logged = ctx.src.json(ctx.final / 'scans_all.json')
    out = {}
    s1 = logged['scan1']['final']
    out['scan1'] = s1 == got['scan1']['tally']
    out['scan4'] = logged['occluded']['final'] == got['scan4']['tally']
    i4 = {k: v for k, v in logged['i4'].items() if k == 'final' or k.startswith('final_by')}
    out['scan2b'] = i4 == got['scan2b']['tally']
    out['scan3'] = logged['i5']['total']['final'] == got['scan3']['tally']
    seeds = logged['seeds']['stats']['final']
    out['seeds'] = seeds == got['seeds']['tally']
    return out, dict(scan1=s1, scan4=logged['occluded']['final'], scan2b=i4, scan3=logged['i5']['total']['final'], seeds=seeds)


def ism_probe_cases(ctx):
    """The ISM rows critique/ism_probe.py and ism_mech.py re-made, each checked to be a row of the ISM
    sample (an id of res_ism.pkl)."""
    ids = {r['id'] for r in ctx.src.pickle(ctx.final / 'res_ism.pkl')}
    out = {}
    for f in ('ism_probe.py', 'ism_mech.py'):
        tree = ast.parse(ctx.src.text(SIMPLIFY / 'critique' / f))
        cases = []
        for node in ast.walk(tree):
            val = None
            if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'cases' for t in node.targets):
                val = node.value
            elif isinstance(node, ast.For) and isinstance(node.iter, (ast.Tuple, ast.List)):
                val = node.iter
            if val is None:
                continue
            try:
                lit = ast.literal_eval(val)
            except ValueError:
                continue
            cases += [c for c in lit if isinstance(c, tuple) and len(c) == 4 and isinstance(c[0], str)]
        rows = ['ism|%s|%s|%d|%gms' % (room, str(tuple(rec)), F, ms) for room, rec, F, ms in cases]
        out[f] = dict(rows=rows, in_ism_sample=sum(r in ids for r in rows))
        if not rows or out[f]['in_ism_sample'] != len(rows):
            raise VoidRun('%s re-makes ISM rows that are not in the sample: %s' % (f, [r for r in rows if r not in ids]))
    return out


def not_scored_items(ctx, z3_jr):
    probes = ism_probe_cases(ctx)
    conf = z3_jr['summary']['confirmed']
    not_robust = sorted(c['id'] for c in conf if not c.get('robust_both'))
    claims = set()

    def walk(x):
        if isinstance(x, dict):
            if 'id' in x and 'truthA' in x:
                claims.add(x['id'])
            for v in x.values():
                walk(v)
        elif isinstance(x, list):
            for v in x:
                walk(v)
    walk(z3_jr)
    unconfirmed = sorted(claims - {c['id'] for c in conf})
    return [
        dict(item='noise-cal cells W-E6 (1 ms) and the 21 cells at 10 ms', reason='no frozen-method result was logged and '
             'no construction gives them a truth: the real set and the seeds scan use the 10 cells of run_scans.py:162; '
             'final/dev/tail_blocks.py read W-E6 (and every other 1 ms cell) for refusal counts only. Their rooms are '
             'corpus rooms (corpus_rooms.json)'),
        dict(item="the ISM rooms' other receivers (attack_ism.py's grid outside the band evaluator's 160-receiver sample) "
                  "and their radius variants (attack_radius.py, R 0.1/0.2/0.5 m)",
             reason='rows of the W1G attack, a different rule; the EDT method was checked on the 1,920-row sample only '
                    '(res_ism.pkl; EVAL.md). Their rooms are corpus rooms (corpus_rooms.json)'),
        dict(item="critique/ism_probe.py's five rows on an image set 2-3x longer, and ism_mech.py's three rows with "
                  "t_arrival moved to the front of the ball",
             reason='each is a row of the ISM sample (checked: ids in res_ism.pkl), which is scored; the longer image set '
                    'and the moved arrival change no room, receiver, band or step, so they carry no quantity value a '
                    'sample row does not; remaking them needs the ISM generator, which the harness copies in ism_fresh.py',
             rows=probes),
        dict(item="z3's %d confirmed cases that are not robust under both truths, and the judge's %d unconfirmed claims"
                  % (len(not_robust), len(unconfirmed)),
             reason='rows of the W1G attack: the EDT evaluation took the 217 robust confirmed cases (loaders.load_z3, '
                    'EVAL.md) and no EDT candidate was run on these. The confirmed cases\' boxes are corpus rooms (P2)',
             not_robust=not_robust, unconfirmed=unconfirmed),
        dict(item="M8a's 433 runs", reason='P4: never read by the method'),
        dict(item='scan 5 (c), degenerate inputs', reason='no truth'),
        dict(item='critique/noise_probe_real.py', reason='LeanBand on the real seeds against the seed spread: no truth; '
             'its histograms are the seeds scan\'s'),
        dict(item='critique/check_truth.py, deletable.py, inspect_ws.py, same_value.py, fix_probe.py, fix_probe_dedup.py, '
                  'ism_fix_check.py; final/dev/*.py',
             reason='no rows of their own: they read the eval sets\' results, or re-run the rows of scan 1 '
                    '(run_scans), scan 2b (half_width 0) or the real seeds, all scored here'),
    ]


def build_weak_spots(target, workers=8, z3=True):
    ctx = Ctx(target, workers)
    src = ctx.src
    gf3 = ctx.target / 'agents' / 'followup-design' / 'skeptic-gf3'
    rooms, _, _ = assign_value(ast.parse(src.text(gf3 / 'attack_ism.py')), 'ROOMS')
    src.read(gf3 / 'ism.py')                                    # iso9613_db_per_m's source
    src.read(SIMPLIFY / 'critique' / 'common.py')               # score()'s source
    for f in ('run_scans.py',):
        src.read(SIMPLIFY / 'final' / f)
    for f in ('scan_i12.py', 'scan_i4.py', 'scan_i4b.py', 'scan_i5.py', 'scan_i1_occluded.py', 'scan_misc.py',
              'real_seeds.py', 'noise_probe.py', 'ablate.py'):
        src.read(SIMPLIFY / 'critique' / f)                     # the constructions, as they were built
    src.read(ctx.target / 'agents' / 'edt-band' / 'eval_run.py')   # z3grid's construction
    sets, rows = {}, []
    t = {}

    def run(name, fn, *a):
        t0 = time.time()
        out = fn(*a)
        t[name] = round(time.time() - t0, 1)
        print('  %s: %.1f s' % (name, t[name]), flush=True)
        return out

    w, s = run('t1', set_t1, ctx)
    rows += w
    sets['t1'] = s
    w, s = run('ism', set_ism, ctx, rooms)
    rows += w
    sets['ism'] = s
    wr, sr, ws, ss = run('real+seeds', set_real_and_seeds, ctx)
    rows += wr + ws
    sets['real'] = sr
    sets['seeds'] = ss
    for name, fn in (('scan1', scan1), ('scan4', scan4_occluded), ('scan2b', scan2b), ('scan3', scan3_noise),
                     ('scan2', scan2), ('scan5', scan5), ('noise_probe', noise_probe), ('ablate', ablate_rows)):
        w, s = run(name, fn, ctx)
        rows += w
        sets[name] = s
    if z3:
        wz, sz, wg, sg = run('z3+z3grid', set_z3, ctx)
        rows += wz + wg
        sets['z3'] = sz
        sets['z3grid'] = sg
    match, logged_tallies = check_scans_all(ctx, sets)
    for k, v in match.items():
        sets[k]['reproduces_scans_all_json'] = v
        sets[k]['scans_all_json_final'] = logged_tallies[k]
    if not all(match.values()):
        raise VoidRun('the recomputed scans do not reproduce final/scans_all.json: %s' % match)
    for k, v in sets.items():
        ver = v.get('verify_against_log')
        if ver and (ver['n_differ'] or ver['n_not_recomputed'] or ver.get('truth_differ')):
            raise VoidRun('%s: the frozen method on its inputs does not give its logged results: %s' % (k, ver))
    order = ['t1', 'z3', 'z3grid', 'real', 'ism', 'seeds', 'scan1', 'scan2', 'scan2b', 'scan3', 'scan4', 'scan5a', 'scan5b',
             'noise_probe', 'ablate']
    rows.sort(key=lambda r: (order.index(r['set']), r['id']))
    counts = collections.Counter((r['set'], r['cls']) for r in rows)
    out = dict(
        schema=SCHEMA_WEAK,
        plan='HARNESS-PLAN.md P33 and section 3 (corpus.py)',
        rule=dict(wrong_silent='status ok and |edt/truth - 1| > %g' % JND,
                  near_miss='status ok and %g < |edt/truth - 1| <= %g' % (NEAR, JND),
                  method='frozen/method.py, sha256 %s, Z = 2' % FROZEN_SHA256,
                  binding='P33: Synth-fresh is bound by every row; ISM-fresh by the rows with has_room true. A quantity '
                          'binds unless the PREREG fixes its range, the row\'s step lies outside the set\'s steps, or the '
                          'value is outside PHYSICS.md\'s limits. Scans carry V only as V_formula_m3, a formula parameter. '
                          'HARNESS-PLAN.md 8.2\'s second call: a row whose own histogram does not hold its truth '
                          '(d_m < R_m, a synth generator defect, not the method\'s) does not bind d_m or d_minus_R_m; '
                          'such a row keeps its place and every other field, with those two quantities null '
                          '(corpus.histogram_holds_truth).'),
        quantities=QUANTITY_DOC,
        sets=sets,
        counts={'%s/%s' % k: v for k, v in sorted(counts.items())},
        rows=rows,
        not_scored=not_scored_items(ctx, ctx.src.json(ctx.target / 'agents' / 'z3-hunt' / 'judge' / 'judge_results.json')),
        provenance=check_provenance(ctx.src),
        sources=src.records(),
        generated=dict(seconds=dict(t, **ctx.timing)),
    )
    return clean(out)


QUANTITY_DOC = dict(
    step_ms='time step of the histogram the method read', band_hz='octave band (rooms)', R_m='receiver ball radius '
    '(0 = the point receiver of scan 2 and 2b)', d_m='source to receiver centre (scans: t_arrival x 343.2)',
    d_minus_R_m='d_m - R_m', delay_ms='source delay (emission)', gap_ms='first reflection after the direct sound: '
    'the synth gap, or (first-order image distance - d) / c in a box', t60_s='synth: T60 of k1; rooms: P5 design T60 at '
    'the row\'s band', t60_design_125_s='rooms: P5 design T60 at 125 Hz', drr_db='synth: 10 log10(Ed / S_rev) (exact); '
    'z3: image-source direct over reflected energy; scan 4: the weak direct\'s level (null when blocked)',
    run_s='histogram length x step', run_over_t60='(run_s - delay - t_arrival) / t60_s', ratio='double-slope k1/k2',
    late_share_db='late part\'s energy over the early part\'s, dB (critique parametrisation, scan_i4.py:20-23)',
    V_m3='room volume (rooms only)', V_formula_m3='a scan\'s V: a parameter of the Sabine direct-energy formula, not a room',
    L1_m='largest room dimension', L2_m='middle', L3_m='smallest', alpha_walls='absorption per face x0 x1 y0 y1 floor ceiling',
    alpha_min='least face absorption', alpha_max='largest face absorption', alpha_mean='area-weighted mean absorption',
    rec_wall_m='receiver centre to the nearest face', rec_wall_minus_R_m='rec_wall_m - R_m', src_wall_m='source to the '
    'nearest face', particles='particles per source (SPPS cells, scan 3 and 5, noise_probe)',
    method='SPPS computation method (real rows; scan 5 (a) models energetic mode)',
    phase='scan 1 and ablate: where the arrival falls inside its step (fraction of a step)',
    d_grid_m='scan 1 and ablate: the grid distance the arrival came from (d_m is the arrival times 343.2)',
    level_after_direct_db='scan 1: 10 log10(S_rev / (Ed + S_rev)); below -8 dB the critique called a row ill-conditioned',
    frac='run length after the arrival over T60 (scan 2, 2b, ablate)', blocked='no direct sound reaches the receiver',
    seed_index='which noise draw (scan 3, 5 (a), noise_probe)', generator_seed='the noise generator\'s seed',
    method_half_width='the half_width the method was told, when it differs from the generator\'s R',
    long_run_within_5pct='scan 2b: the same response recorded to -150 dB gives an EDT within 5 % of the truth',
    d2_m='scan 5 (b): the second source\'s distance', sources='scan 5 (b): number of sources',
    c_case_m_s='z3: the case\'s speed of sound', m_case_np_per_m='z3: the case\'s air attenuation (energy, Np/m)',
    image_time_s='image-source set length (ISM, z3)', images='image sources in the z3 echo',
    gap_case_ms='z3: the judge\'s own first-reflection gap')


# ================================================================================================
def write_json(path, obj):
    text = json.dumps(obj, indent=1, sort_keys=False, ensure_ascii=False, allow_nan=False) + '\n'
    Path(path).write_text(text, encoding='utf-8', newline='\n')
    return sha256_bytes(text.encode('utf-8'))


# The one block of either file that changes from run to run; everything else reproduces exactly (T22).
VOLATILE = ('generated',)


def generated():
    return dict(python=platform.python_version(), numpy=np.__version__, platform=platform.platform(),
                at=time.strftime('%Y-%m-%dT%H:%M:%S%z'))


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--target-root')
    ap.add_argument('--out', default=str(HARNESS))
    ap.add_argument('--workers', type=int, default=8)
    ap.add_argument('--only', choices=('rooms', 'weak'))
    ap.add_argument('--m8a-project', default=r'C:\tmp\nm-m8a-bed\20260929T093134Z\runs\atmospheric-validation\s1\project.simpa')
    a = ap.parse_args(argv)
    target = find_target_root(a.target_root)
    out = Path(a.out)
    t0 = time.time()
    if a.only in (None, 'rooms'):
        rooms = build_corpus_rooms(target, a.m8a_project)
        rooms['generated'] = generated()
        h = write_json(out / 'corpus_rooms.json', rooms)
        print('corpus_rooms.json: %d rooms (%s), sha256 %s' % (rooms['summary']['rooms'], rooms['summary']['kinds'], h[:16]))
    if a.only in (None, 'weak'):
        weak = build_weak_spots(target, a.workers)
        weak['generated'].update(generated())
        h = write_json(out / 'weak_spots.json', weak)
        print('weak_spots.json: %d rows %s, sha256 %s' % (len(weak['rows']), weak['counts'], h[:16]))
    print('done in %.0f s' % (time.time() - t0))


if __name__ == '__main__':
    main()
