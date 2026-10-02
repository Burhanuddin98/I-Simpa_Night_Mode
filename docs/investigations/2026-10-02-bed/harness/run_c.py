"""Set C solver runs (../PREREG.md; ../ADDENDUM-1.md item 9): two specular boxes (scattering 0) from T20 part 2's S2,
S-live and Mixed, at the new-project defaults, seeds 4101-4103, 6 runs.

Each project is round 2's tested-G1-energetic-1.0ms-150k-3101/project.simpa (the schema) with its geometry, materials,
surface groups, source and receivers replaced: the box of ../../2026-10-02-edt-ball-vs-point/run.py's ROOMS, the
source at SRC_FRAC and three receivers at REC_FRACS of it, every material scattering 0 with its alpha in every band.
Solvers and simpa: run_b.py's. 4 runs at a time. With --wait the runs start only once set B's run.out.log has a line
starting `exit` (set B holds the machine's four solver slots until then).
Usage: python run_c.py [outdir] [--wait]   (default B:\\data\\m8b-bed\\C); a finished run (done.json) is skipped.
"""
import importlib.util
import json
import math
import subprocess
import sys
import time
import uuid
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
BVP = HERE.parents[1] / '2026-10-02-edt-ball-vs-point' / 'run.py'
SIMPA = r"C:\tmp\nm-target-f\release\simpa.exe"
SOLV = r"C:\tmp\nm-m8a-solvers"
TEMPLATE = Path(r"B:\data\m8b-edt\round2\heldout\tested-G1-energetic-1.0ms-150k-3101\project.simpa")
B_LOG = Path(r"B:\data\m8b-bed\B\run.out.log")
ROOMS = ('S-live', 'Mixed')
SEEDS = (4101, 4102, 4103)
TESTED = dict(particles_per_source=150_000, time_step_s=0.001, duration_s=10.0, extinction_exponent=7.0,
              method='energetic')
WORKERS = 4
NS = uuid.UUID('6f1c1d0e-2026-4a10-b2ed-000000000c00')
WALLS = ('x_lo', 'x_hi', 'y_lo', 'y_hi', 'z_lo', 'z_hi')
# the box's 12 triangles in the template's vertex order (vertex i: x = Lx if i >= 4, y = Ly if i in 1,2,5,6,
# z = Lz if i in 2,3,6,7), two per wall, as G1's project lists them
FACES = {'x_lo': ([3, 2, 1], [3, 1, 0]), 'x_hi': ([4, 5, 6], [4, 6, 7]), 'y_lo': ([4, 7, 3], [4, 3, 0]),
         'y_hi': ([1, 2, 6], [1, 6, 5]), 'z_lo': ([1, 5, 4], [1, 4, 0]), 'z_hi': ([3, 7, 6], [3, 6, 2])}


def bvp():
    spec = importlib.util.spec_from_file_location('bvp_run', BVP)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def uid(*parts):
    return str(uuid.uuid5(NS, '|'.join(str(p) for p in parts)))


def box_vertices(L):
    Lx, Ly, Lz = L
    return [[Lx if i >= 4 else 0.0, Ly if i in (1, 2, 5, 6) else 0.0, Lz if i in (2, 3, 6, 7) else 0.0]
            for i in range(8)]


def geometry(name):
    """(L, a6, src, [rec, ...]) of the S2 room, exactly as t20p2.s2_units places them."""
    run = bvp()
    L, a6 = run.ROOMS[name]
    L = tuple(float(x) for x in L)
    src = tuple(f * l for f, l in zip(run.SRC_FRAC, L))
    recs = [tuple(f * l for f, l in zip(fr, L)) for fr in run.REC_FRACS]
    return L, [float(a) for a in a6], src, recs


def project(name, seed):
    p = json.loads(TEMPLATE.read_text(encoding='utf-8'))
    L, a6, src, recs = geometry(name)
    for r in recs:
        assert math.dist(src, r) >= 2.0, (name, r)
        assert 0.31 < min(min(x, l - x) for x, l in zip(r, L)), (name, r)
    nb = len(p['bands']['frequencies_hz'])
    alphas = sorted(set(a6))
    mats, groups = [], []
    for a in alphas:
        mid = uid(name, 'material', a)
        mats.append(dict(p['materials'][0], id=mid, name='%s: alpha %g, scattering 0' % (name, a),
                         absorption=[a] * nb, scattering=[0.0] * nb))
        groups.append(dict(p['surface_groups'][0], id=uid(name, 'group', a), name='alpha %g' % a, material=mid))
    gid = {a: g['id'] for a, g in zip(alphas, groups)}
    faces = [f + [gid[a]] for w, a in zip(WALLS, a6) for f in FACES[w]]
    p['id'] = uid(name, 'project')
    p['name'] = 'C-%s' % name
    p['description'] = ('The bed, set C (docs/investigations/2026-10-02-bed/PREREG.md, ADDENDUM-1.md item 9): T20 '
                        'part 2 S2 room %s, %g x %g x %g m, specular (scattering 0); written by harness/run_c.py.'
                        % (name, *L))
    p['geometry'] = dict(vertices=box_vertices(L), faces=faces)
    p['materials'], p['surface_groups'] = mats, groups
    p['sources'] = [dict(p['sources'][0], position=list(src))]
    tmpl = p['point_receivers'][0]
    p['point_receivers'] = [dict(tmpl, id=uid(name, 'receiver', i), name='R%03d' % i, position=list(r))
                            for i, r in enumerate(recs)]
    sp = p['solvers']['spps']
    sp.update(TESTED)
    sp['random_seed'] = seed
    assert sp['receiver_radius_m'] == 0.31
    return p


def one(out, name, seed):
    rid = 'tested-%s-%d' % (name, seed)
    d = out / rid
    if (d / 'done.json').exists():
        return json.loads((d / 'done.json').read_text())
    d.mkdir(parents=True, exist_ok=True)
    proj = d / 'project.simpa'
    proj.write_text(json.dumps(project(name, seed), indent=2), encoding='utf-8')
    cmd = [SIMPA, "run", str(proj), "--solver", "spps", "--runs", str(d), "--json",
           "--solver-exe", rf"{SOLV}\spps.exe", "--tetgen", rf"{SOLV}\tetgen.exe",
           "--preprocess", rf"{SOLV}\preprocess.exe"]
    t0 = time.time()
    r = subprocess.run(cmd, capture_output=True, text=True)
    (d / 'simpa-run.stdout.json').write_text(r.stdout)
    (d / 'simpa-run.stderr.txt').write_text(r.stderr)
    folders = sorted(x for x in d.iterdir() if x.is_dir() and x.name.endswith('-spps'))
    sp = json.loads(proj.read_text(encoding='utf-8'))['solvers']['spps']
    res = {'kind': 'tested', 'room': name, 'seed': seed, 'run_exit': r.returncode, 'wall_s': round(time.time() - t0, 1),
           'run_folder': str(folders[-1]) if folders else None, 'finished': time.strftime('%H:%M:%S'),
           'settings': {k: sp[k] for k in ('method', 'particles_per_source', 'time_step_s', 'duration_s',
                                           'extinction_exponent', 'random_seed', 'receiver_radius_m')}}
    (d / 'done.json').write_text(json.dumps(res, indent=1))
    print(json.dumps({k: res[k] for k in ('kind', 'room', 'seed', 'run_exit', 'wall_s', 'finished')}), flush=True)
    return res


def b_finished():
    try:
        return any(line.startswith('exit') for line in B_LOG.read_text(encoding='utf-8').splitlines())
    except OSError:
        return False


if __name__ == '__main__':
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    out = Path(args[0] if args else r'B:\data\m8b-bed\C')
    if '--wait' in sys.argv:
        while not b_finished():
            time.sleep(30)
        print('set B finished at %s; starting set C' % time.strftime('%H:%M:%S'), flush=True)
    out.mkdir(parents=True, exist_ok=True)
    jobs = [(n, s) for s in SEEDS for n in ROOMS]
    with ThreadPoolExecutor(WORKERS) as ex:
        res = list(ex.map(lambda j: one(out, *j), jobs))
    (out / 'runs.json').write_text(json.dumps(res, indent=1))
    bad = [o for o in res if o['run_exit'] != 0]
    print('exit %d' % len(bad), flush=True)
