"""Shared paths, references and helpers for harness2: T1-T22 as adapted, T23-T43 (../../HARNESS-PLAN-2.md section 6).

Every test writes only under pytest's basetemp, C:\\tmp\\m8b-edt\\pytest-tmp (../pyproject.toml),
never on B:. Corpus inputs come from the main checkout's gitignored target/ (corpus.find_target_root) and
from this worktree, each hash-checked where a test depends on its exact bytes. No held-out data is
read or made, and no solver is run.

The references here are the tests' own: the corpus's generator and truth from target/, P3's rule,
P5's formula and the geometry of a union of boxes. A module under test is never its own reference.
"""
import ast
import hashlib
import json
import math
import os
import pickle
import shutil
import sys
import types
from pathlib import Path

sys.dont_write_bytecode = True
import numpy as np  # noqa: E402
import pytest  # noqa: E402

from m8b import corpus  # noqa: E402

HARNESS = corpus.HARNESS
INVESTIGATION = corpus.INVESTIGATION
REPO = corpus.REPO
FROZEN = corpus.FROZEN                                   # round 1's file: a control here, never the method under test
FROZEN_SHA256 = corpus.FROZEN_SHA256                     # PREREG.md:8
FROZEN2 = INVESTIGATION / 'frozen2' / 'method.py'        # round 2's method (PREREG-2.md, amendment 1: Z = 2.5)
FROZEN2_SHA256 = '029d90ac5e8f6a634a51a7ffce136cbd4c0b7ea3d3ad8a18b78fd8240ff224a0'   # HARNESS-PLAN-2.md section 4, LF bytes
SCRATCH = Path(r'C:\tmp\m8b-edt')
MIN_FREE_C = 8 * 1024 ** 3                               # the task's floor for C:

# The corpus sources the tests read as references (raw bytes as they are in target/, CRLF).
ISM_PY_SHA256 = 'a7c9d41ec61dd119a14583faaa98d4a88c98e897a0cf4a8d69c6d151d2cef747'        # P20
MIRROR_PY_SHA256 = 'c5603d5ba863e9f61086c3897106982161de2e431e65010731c6317f80791f85'     # P15
ATTACK_ISM_PY_SHA256 = '3fb8a84b9eb42569f9aa3dfb4693e76e098b70e0f1e2c14664b5d35da495652a'  # P15
ISM_ROWS_PKL_SHA256 = 'b966ab42e75d338e2e6550f214bfb4a4ca73ad04a6f6965d042f0a1a0b95f0b9'   # final/dev
OCTAVES = (125, 250, 500, 1000, 2000, 4000)


def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


def sha256_file(path):
    return sha256_bytes(Path(path).read_bytes())


def pytest_sessionstart(session):
    free = shutil.disk_usage('C:\\').free
    if free < MIN_FREE_C:
        pytest.exit('C: has %.1f GB free, under the 8 GB floor: stopping' % (free / 1024 ** 3), returncode=3)


def exec_checked(path, want_sha256, name):
    """A source file executed as a module from the very bytes whose sha256 was checked."""
    data = Path(path).read_bytes()
    got = sha256_bytes(data)
    assert got == want_sha256, '%s has sha256 %s, not %s' % (path, got, want_sha256)
    mod = types.ModuleType(name)
    mod.__file__ = str(path)
    exec(compile(data, str(path), 'exec'), mod.__dict__)
    return mod


@pytest.fixture(scope='session')
def target_root():
    return corpus.find_target_root()


@pytest.fixture(scope='session')
def simpa_exe():
    p = Path(os.environ.get('M8B_SIMPA_EXE', r'C:\tmp\nm-target\release\simpa.exe'))
    assert p.is_file(), 'no simpa.exe at %s (harness/SETUP.md; set M8B_SIMPA_EXE)' % p
    return p


@pytest.fixture(scope='session')
def solvers_dir():
    p = Path(os.environ.get('SIMPA_SOLVERS_DIR', r'C:\tmp\nm-m8a-solvers'))
    assert (p / 'spps.exe').is_file(), 'no verified solver folder at %s (harness/SETUP.md)' % p
    return p


@pytest.fixture(scope='session')
def synth():
    """critique/synth.py, executed only after its hash is checked (corpus.load_synth)."""
    return corpus.load_synth()


@pytest.fixture(scope='session')
def corpus_rooms():
    return json.loads((HARNESS / 'corpus_rooms.json').read_text(encoding='utf-8'))


@pytest.fixture(scope='session')
def corpus_ism(target_root):
    """The corpus's ISM generator and truth, from target/ and hash-checked: ism.py (P20), mirror.py's
    line and integral, and attack_ism.py's truth_ideal, constants and ROOMS, taken by text from the
    file (importing attack_ism.py would import its rule modules)."""
    gf3 = target_root / 'agents' / 'followup-design' / 'skeptic-gf3'
    ism = exec_checked(gf3 / 'ism.py', ISM_PY_SHA256, 'ref_ism')
    mirror = exec_checked(gf3 / 'rerun' / 'mirror.py', MIRROR_PY_SHA256, 'ref_mirror')
    data = (gf3 / 'attack_ism.py').read_bytes()
    assert sha256_bytes(data) == ATTACK_ISM_PY_SHA256
    ns = dict(np=np, math=math, M=mirror)
    want_fn = {'truth_ideal', 'planes_box', 'first_reflection'}
    want_var = {'C', 'R_DEFAULT', 'DT_F', 'ROOMS'}
    for node in ast.parse(data).body:
        take = (isinstance(node, ast.FunctionDef) and node.name in want_fn) or (
            isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id in want_var for t in node.targets))
        if take:
            exec(compile(ast.Module([node], []), str(gf3 / 'attack_ism.py'), 'exec'), ns)
    return types.SimpleNamespace(ism=ism, mirror=mirror, truth_ideal=ns['truth_ideal'],
                                 first_reflection=ns['first_reflection'], C=ns['C'], R=ns['R_DEFAULT'],
                                 DT_F=ns['DT_F'], ROOMS=ns['ROOMS'])


@pytest.fixture(scope='session')
def ism_rows(target_root):
    """final/dev/ism_rows.pkl: the corpus's 1,920 ISM rows (loaders.load_ism(n_tasks=160, steps 1 and 2 ms))."""
    p = target_root / 'agents' / 'edt-simplify' / 'final' / 'dev' / 'ism_rows.pkl'
    data = p.read_bytes()
    assert sha256_bytes(data) == ISM_ROWS_PKL_SHA256
    return pickle.loads(data)


def pick_rows(rows, room):
    """T6's and T7's rule, fixed before any harness code existed: among the room's ISM rows with a
    finite truth, sorted by id, the first, the middle and the last."""
    sel = sorted((r for r in rows if r['id'].split('|')[1] == room and r['truth_edt'] is not None
                  and math.isfinite(r['truth_edt'])), key=lambda r: r['id'])
    n = len(sel)
    return [sel[0], sel[(n - 1) // 2], sel[n - 1]]


def parse_ism_id(rid):
    """'ism|<room>|(x, y, z)|<band>|<step>ms' -> (room, rec, band_hz, step_ms)."""
    _, room, rec, band, step = rid.split('|')
    return room, tuple(float(x) for x in rec.strip('()').split(',')), int(band), float(step[:-2])


# ---- P3 and P5, the tests' own ---------------------------------------------------------------------
def sorted_dims(room):
    if 'dims_sorted_m' in room:
        return [float(x) for x in room['dims_sorted_m']]
    lo, hi = room['bbox_m']
    return sorted((float(b) - float(a) for a, b in zip(lo, hi)), reverse=True)


def within10(x, ref):
    return abs(x / ref - 1.0) <= 0.10


def p3_near_duplicate(room, ref):
    """P3: two boxes are near-duplicates when each sorted dimension is within 10 % (of the reference
    room's); two non-box rooms when their sorted bounding boxes, volumes and surface areas are each
    within 10 %; a box and a non-box room never are."""
    if room['kind'] != ref['kind']:
        return False
    if not all(within10(a, b) for a, b in zip(sorted_dims(room), sorted_dims(ref))):
        return False
    if room['kind'] == 'box':
        return True
    return within10(room['volume_m3'], ref['volume_m3']) and within10(room['surface_m2'], ref['surface_m2'])


def eyring(volume, parts, band_hz):
    """P5: T = 24 ln10 V / (c (-S ln(1 - abar) + 4 m V)), c = 343.2, m from ISO 9613-1 at 20 C and
    50 % RH (corpus.m_energy, the text of ism.py's). parts: [(area, alpha)]."""
    S = sum(a for a, _ in parts)
    abar = sum(a * al for a, al in parts) / S
    return 24.0 * math.log(10) * volume / (343.2 * (-S * math.log(1.0 - abar) + 4.0 * corpus.m_energy(band_hz) * volume))


# ---- a union of axis-aligned boxes (P7), the tests' own ----------------------------------------------
class Union:
    """The air volume of a room given as disjoint axis-aligned boxes: volume, boundary faces and
    area, a point's distance to the boundary, and whether a segment stays inside."""

    def __init__(self, boxes):
        self.boxes = [(np.asarray(lo, float), np.asarray(hi, float)) for lo, hi in boxes]
        cuts = [sorted({float(b[s][ax]) for b in self.boxes for s in (0, 1)}) for ax in range(3)]
        self.cuts = cuts
        shape = tuple(len(c) - 1 for c in cuts)
        inside = np.zeros(shape, bool)
        for i in range(shape[0]):
            for j in range(shape[1]):
                for k in range(shape[2]):
                    c = [0.5 * (cuts[0][i] + cuts[0][i + 1]), 0.5 * (cuts[1][j] + cuts[1][j + 1]),
                         0.5 * (cuts[2][k] + cuts[2][k + 1])]
                    inside[i, j, k] = self.contains(c, eps=0.0)
        self.inside = inside
        self.volume = 0.0
        self.faces = []                        # (axis, coord, (u0, u1), (v0, v1)), u, v the other axes in order
        for idx in np.ndindex(shape):
            if not inside[idx]:
                continue
            w = [cuts[a][idx[a] + 1] - cuts[a][idx[a]] for a in range(3)]
            self.volume += w[0] * w[1] * w[2]
            for ax in range(3):
                o = [a for a in range(3) if a != ax]
                for step in (-1, 1):
                    nb = list(idx)
                    nb[ax] += step
                    out = nb[ax] < 0 or nb[ax] >= shape[ax] or not inside[tuple(nb)]
                    if out:
                        coord = cuts[ax][idx[ax] + (1 if step > 0 else 0)]
                        self.faces.append((ax, coord, (cuts[o[0]][idx[o[0]]], cuts[o[0]][idx[o[0]] + 1]),
                                           (cuts[o[1]][idx[o[1]]], cuts[o[1]][idx[o[1]] + 1])))
        self.surface = sum((u1 - u0) * (v1 - v0) for _, _, (u0, u1), (v0, v1) in self.faces)

    def contains(self, p, eps=1e-9):
        return any(all(lo[a] - eps <= p[a] <= hi[a] + eps for a in range(3)) for lo, hi in self.boxes)

    def clearance(self, p):
        best = math.inf
        for ax, coord, (u0, u1), (v0, v1) in self.faces:
            o = [a for a in range(3) if a != ax]
            du = max(u0 - p[o[0]], 0.0, p[o[0]] - u1)
            dv = max(v0 - p[o[1]], 0.0, p[o[1]] - v1)
            best = min(best, math.sqrt((p[ax] - coord) ** 2 + du * du + dv * dv))
        return best

    def segment_inside(self, a, b, n=20000):
        a, b = np.asarray(a, float), np.asarray(b, float)
        return all(self.contains(a + (b - a) * (i / n)) for i in range(n + 1))
