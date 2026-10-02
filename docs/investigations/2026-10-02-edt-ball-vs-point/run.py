"""EDT, ball receiver vs ISO point receiver (PREREG.md beside this file). Imports read-only:
harness2/m8b/_ism.py (images, ball echogram), harness2/m8b/truth.py (truth_ideal, the checked copy),
frozen2/method.py (v2.1). Usage: python run.py [out_dir] [workers]
"""
import csv
import importlib.util
import json
import math
import sys
import time
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
HELD = HERE.parent / '2026-09-27-edt-heldout'
sys.path.insert(0, str(HELD / 'harness2'))
from m8b import _ism as ism  # noqa: E402
from m8b import truth  # noqa: E402

_spec = importlib.util.spec_from_file_location('frozen2_method', HELD / 'frozen2' / 'method.py')
method = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(method)

C = 343.20001220703125          # as ism_fresh.py:205
IMAGE_FACTOR = 0.6               # PREREG: image set to 0.6 x Eyring T60 (argv[3] overrides, AMENDMENT-1)
DT_FINE = 2e-5                   # 0.02 ms, as round 2
DL = C * DT_FINE
RADII = (0.1, 0.31, 0.5, 0.75, 1.0, 1.5)
STEPS_MS = (1, 2, 5)
REC_FRACS = ((0.55, 0.5, 0.5), (0.75, 0.7, 0.5), (0.85, 0.85, 0.5))
SRC_FRAC = (0.25, 0.3, 0.45)
ROOMS = {
    'S-live': ((6, 5, 3), [0.10] * 6),
    'S-dead': ((6, 5, 3), [0.40] * 6),
    'M-live': ((15, 10, 6), [0.15] * 6),
    'M-dead': ((15, 10, 6), [0.35] * 6),
    'Hall': ((30, 20, 12), [0.20] * 6),
    'Mixed': ((10, 7, 3.5), [0.20, 0.20, 0.20, 0.20, 0.50, 0.10]),
}


def eyring_t60(L, a6):
    Lx, Ly, Lz = L
    areas = [Ly * Lz, Ly * Lz, Lx * Lz, Lx * Lz, Lx * Ly, Lx * Ly]
    S = sum(areas)
    abar = sum(a * s for a, s in zip(a6, areas)) / S
    return 0.161 * Lx * Ly * Lz / (-S * math.log(1 - abar))


def alpha_pairs(a6):
    return [(a6[0], a6[1]), (a6[2], a6[3]), (a6[4], a6[5])]


def point_echogram(L, s, r, R, alpha, lmax, dl):
    """(direct, refl) of a point receiver: each image's ball total W R^3/(3 D^2) at path length D."""
    P, W, O = ism.images(L, s, alpha, lmax)
    D = np.linalg.norm(P - np.asarray(r, float)[None, :], axis=1)
    keep = (D < lmax) & (W > 0)
    D, W, O = D[keep], W[keep], O[keep]
    nb = int(math.ceil(lmax / dl))
    i = np.floor(D / dl).astype(np.int64)
    e = W * R ** 3 / (3 * D * D)
    ok = i < nb
    direct = np.bincount(i[ok & (O == 0)], weights=e[ok & (O == 0)], minlength=nb)[:nb]
    refl = np.bincount(i[ok & (O != 0)], weights=e[ok & (O != 0)], minlength=nb)[:nb]
    return direct, refl


def instrument_i():
    """One image (alpha 1 on every wall leaves the direct only): point total vs ball total, R 0.31, d 5."""
    L, s, r, R = (40.0, 40.0, 40.0), (10.0, 10.0, 10.0), (15.0, 10.0, 10.0), 0.31
    alpha = [(1.0, 1.0)] * 3
    bd, br = ism.echogram(L, s, r, R, alpha, 6.0, DL)
    pd, pr = point_echogram(L, s, r, R, alpha, 6.0, DL)
    rel = abs(pd.sum() / bd.sum() - 1)
    return dict(ball=float(bd.sum()), point=float(pd.sum()), rel=rel, ok=bool(rel <= 1e-3 and br.sum() == 0 and pr.sum() == 0))


def job(spec):
    name, ri, R, factor = spec
    L, a6 = ROOMS[name]
    L = tuple(float(x) for x in L)
    alpha = alpha_pairs(a6)
    src = tuple(f * l for f, l in zip(SRC_FRAC, L))
    rec = tuple(f * l for f, l in zip(REC_FRACS[ri], L))
    d = math.dist(src, rec)
    t60 = eyring_t60(L, a6)
    base = dict(room=name, rec=ri, R=R, d=d, t60_eyring=t60, image_factor=factor)
    if d < 2.0:
        return [dict(base, excluded='near_source')]
    if R >= min(min(x, l - x) for x, l in zip(rec, L)):
        return [dict(base, excluded='ball_crosses_wall')]
    lmax = C * factor * t60
    t0 = time.time()
    bd, br = ism.echogram(L, src, rec, R, alpha, lmax, DL)
    pd, pr = point_echogram(L, src, rec, R, alpha, lmax, DL)
    t_arr = d / C
    edt_point = float(truth.truth_ideal(pd, pr, t_arr, DT_FINE)[0])
    edt_ball = float(truth.truth_ideal(bd, br, t_arr, DT_FINE)[0])
    v = bd + br
    out = []
    for step_ms in STEPS_MS:
        k = int(round(step_ms * 1e-3 / DT_FINE))
        n = len(v) // k
        vb = v[:n * k].reshape(n, k).sum(1)
        nz = np.nonzero(vb > 0)[0]
        vb = vb[:int(nz[-1]) + 1]
        o = method.analyse(vb, k * DT_FINE, t_arr, dict(half_width=R / C))
        e = (o['edt'] / edt_point - 1) if o['edt'] is not None else None
        out.append(dict(base, step_ms=step_ms, edt_point=edt_point, edt_ball=edt_ball,
                        ball_vs_point=edt_ball / edt_point - 1, edt_prod=o['edt'], status=o['status'],
                        edt_lo=o['edt_lo'], edt_hi=o['edt_hi'],
                        covered=(o['edt_lo'] <= edt_point <= o['edt_hi']) if o['edt'] is not None else None,
                        reason=o['reason'] if o['status'] == 'refused' else '', e=e,
                        images_s=time.time() - t0))
    return out


def verdict(rows):
    gated = [r for r in rows if 'excluded' not in r and r['R'] <= 0.5 and r['step_ms'] == 1 and r['e'] is not None]
    worst = max((abs(r['e']) for r in gated), default=float('nan'))
    if any(abs(r['e']) > 0.05 for r in gated):
        v = 'FAIL'
    elif any(abs(r['e']) > 0.005 for r in gated):
        v = 'PARTIAL'
    else:
        v = 'PASS'
    return dict(verdict=v if gated else 'INCONCLUSIVE', gated_rows=len(gated), worst_abs_e=worst,
                over_tenth=[(r['room'], r['rec'], r['R'], r['e']) for r in gated if abs(r['e']) > 0.005])


def main():
    out = Path(sys.argv[1] if len(sys.argv) > 1 else r'B:\data\m8b-edt\ball-vs-point')
    workers = int(sys.argv[2]) if len(sys.argv) > 2 else 4
    global IMAGE_FACTOR
    if len(sys.argv) > 3:
        IMAGE_FACTOR = float(sys.argv[3])
    only = sys.argv[4].split(',') if len(sys.argv) > 4 else list(ROOMS)
    out.mkdir(parents=True, exist_ok=True)
    log = open(out / 'run.log', 'a', encoding='utf-8')

    def say(msg):
        line = '%s %s' % (time.strftime('%H:%M:%S'), msg)
        print(line, flush=True)
        log.write(line + '\n')
        log.flush()

    inst = instrument_i()
    say('instrument (i) one image: %s' % json.dumps(inst))
    # (ii): a planted 1 % error must be flagged by the same rule.
    planted = verdict([dict(room='x', rec=0, R=0.31, step_ms=1, e=0.01)])
    inst2 = planted['verdict'] != 'PASS'
    say('instrument (ii) planted 1 %% -> %s' % planted['verdict'])
    if not (inst['ok'] and inst2):
        say('INCONCLUSIVE: an instrument check failed')
        return
    specs = [(name, ri, R, IMAGE_FACTOR) for name in only for ri in range(len(REC_FRACS)) for R in RADII]
    rows = []
    with ProcessPoolExecutor(workers) as ex:
        for i, res in enumerate(ex.map(job, specs), 1):
            rows.extend(res)
            r = res[0]
            say('%d/%d %s rec%d R=%.2f %s' % (i, len(specs), r['room'], r['rec'], r['R'],
                                              r.get('excluded') or 'e1=%s' % (('%+.4f' % res[0]['e']) if res[0]['e'] is not None else res[0]['reason'])))
    keys = sorted({k for r in rows for k in r})
    with open(out / 'rows.csv', 'w', newline='', encoding='utf-8') as f:
        w = csv.DictWriter(f, fieldnames=keys)
        w.writeheader()
        w.writerows(rows)
    v = verdict(rows)
    (out / 'rows.json').write_text(json.dumps(dict(instrument=[inst, planted], verdict=v, rows=rows), indent=1), encoding='utf-8')
    say('VERDICT %s' % json.dumps(v))


if __name__ == '__main__':
    main()
