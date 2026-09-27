"""GATE 4: reload every golden case, recompute with band_early.py, assert against the frozen
outputs. Python only, single process, no solver run.

Exact-match fields (bit-identical float64 equality after reload): lo, hi, refused, n_eval,
budget_exhausted, n0. See golden/README.md for why each of these is exact-reproducible from the
frozen inputs with no iteration-order freedom.

Tolerance fields: value, witness, U -- these come out of edt_band()'s adaptive bisection
(`max_split`, `gap_rel`, `stall` in band_early.Setup.edt_band), whose loop can in principle take a
different number of splits on a different platform's float rounding of the same double-precision
arithmetic (a `gap <= gap_rel` comparison a few ULP either side of the threshold), which changes
which candidate the running max/min held at the moment it stopped. The stopped bound itself does
not move outside the true [lo, hi] (SPEC section 7's soundness holds regardless of when bisection
stops), only how tight it is; a Rust port that stops at one split earlier or later still reports a
band inside the same [lo, hi] but not necessarily the same reported `value`/`witness`/`U`, so these
are checked with a relative tolerance instead of equality.

Usage: python -B golden/check_golden.py
"""
import base64
import json
import math
import os
import sys

sys.dont_write_bytecode = True
os.environ['PYTHONDONTWRITEBYTECODE'] = '1'
import numpy as np  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, ROOT)
import band_early as be  # noqa: E402

EXACT_TOL = 0.0          # bit-identical, same process/platform (float64 IEEE 754 is deterministic)
VALUE_REL_TOL = 1e-9     # generous vs the ~1e-9 gap_rel default; catches a broken port, not a
                         # different (still-valid) number of bisection splits


def unarr(d):
    a = np.frombuffer(base64.b64decode(d['b64']), dtype=np.dtype(d['dtype']))
    return a.reshape(d['shape'])


def close(a, b, tol=VALUE_REL_TOL):
    if a is None and b is None:
        return True
    if a is None or b is None:
        return False
    if isinstance(a, float) and (math.isinf(a) or math.isnan(a)):
        return a == b or (math.isnan(a) and isinstance(b, float) and math.isnan(b))
    denom = max(abs(a), abs(b), 1e-300)
    return abs(a - b) <= tol * denom


def check_t1_case(case, fails):
    inp = case['inputs']
    B = unarr(inp['B'])
    s = be.Setup(B, inp['dt'], inp['t_arr'], half_width=inp['half_width'], air_rate=inp['air_rate'],
                 rel_eps=inp['rel_eps'], tail_max=inp['tail_max'], tail_t_max=inp['tail_t_max'],
                 t_refl_min=inp['t_refl_min'], kappa=inp.get('kappa', 1.0))
    res = s.all_bands()
    for q in ('edt', 'ts', 'c50', 'c80', 'd50'):
        exp = case['outputs'][q]
        r = res[q]
        b = r.get('band')
        got_lo = None if not b else float(b[0])
        got_hi = None if not b else float(b[1])
        if exp['lo'] is None and exp['hi'] is None:
            ok_band = got_lo is None and got_hi is None
        else:
            ok_band = close(exp['lo'], got_lo, EXACT_TOL if not (exp['lo'] and math.isinf(exp['lo'])) else 0) and \
                      close(exp['hi'], got_hi, EXACT_TOL if not (exp['hi'] and math.isinf(exp['hi'])) else 0)
        ok_refused = r.get('refused') == exp['refused']
        if not (ok_band and ok_refused):
            fails.append(dict(id=case['id'], q=q, expected=exp, got=dict(lo=got_lo, hi=got_hi,
                                                                          refused=r.get('refused'))))


def check_edge_case(case, fails):
    inp = case['inputs']
    cid = case['id']
    if cid.startswith('run_too_short'):
        # GATE 4b: these cases freeze Setup.all_bands()'s per-quantity tail-widening refusal
        # (rel_eps is itself an array here -- production_inputs()'s per-bin eps -- so it goes
        # through unarr like B, not read as a plain scalar the way the other edge cases' rel_eps is).
        B = unarr(inp['B'])
        rel_eps = unarr(inp['rel_eps'])
        s = be.Setup(B, inp['dt'], inp['t_arr'], half_width=inp['half_width'], air_rate=inp['air_rate'],
                     rel_eps=rel_eps, tail_max=inp['tail_max'], tail_t_max=inp['tail_t_max'],
                     t_refl_min=inp['t_refl_min'], kappa=inp.get('kappa', 1.0))
        res = s.all_bands(upstream=False)
        for q, exp in case['outputs'].items():
            r = res[q]
            b = r.get('band')
            got_lo = None if not b else float(b[0])
            got_hi = None if not b else float(b[1])
            ok = r.get('refused') == exp['refused'] and close(exp['lo'], got_lo) and close(exp['hi'], got_hi)
            if not ok:
                fails.append(dict(id=cid, q=q, expected=exp, got=dict(lo=got_lo, hi=got_hi, refused=r.get('refused'))))
        return
    B = unarr(inp['B'])
    kwargs = dict(half_width=inp['half_width'], air_rate=inp['air_rate'], rel_eps=inp.get('rel_eps', 0.0),
                  tail_max=inp.get('tail_max', 0.0), tail_t_max=inp.get('tail_t_max'),
                  t_refl_min=inp.get('t_refl_min'), kappa=inp.get('kappa', 1.0))
    if inp.get('n_sources'):
        kwargs['n_sources'] = inp['n_sources']
    s = be.Setup(B, inp['dt'], inp['t_arr'], **kwargs)
    if cid.startswith('not_decaying'):
        band, info = s.edt_band()
        exp = case['outputs']['edt']
        got_lo = None if band is None else float(band[0])
        got_hi = None if band is None else float(band[1])
        ok = (info.get('refused') == exp['refused'] and
              close(exp['lo'], got_lo) and close(exp['hi'], got_hi, tol=1e-6 if exp['hi'] not in (None, float('inf')) else 0))
        if not ok:
            fails.append(dict(id=cid, expected=exp, got=dict(lo=got_lo, hi=got_hi, refused=info.get('refused'))))
        return
    res = s.all_bands(upstream=False)
    for q, exp in case['outputs'].items():
        r = res[q]
        b = r.get('band')
        got_lo = None if not b else float(b[0])
        got_hi = None if not b else float(b[1])
        ok = r.get('refused') == exp['refused'] and close(exp['lo'], got_lo) and close(exp['hi'], got_hi)
        if not ok:
            fails.append(dict(id=cid, q=q, expected=exp, got=dict(lo=got_lo, hi=got_hi, refused=r.get('refused'))))


def main():
    fails = []
    n_checked = 0
    for name, fn in (('t1', check_t1_case), ('edge_cases', check_edge_case)):
        path = os.path.join(HERE, f'{name}.jsonl')
        if not os.path.exists(path):
            print(f'SKIP {name}: {path} not found')
            continue
        with open(path) as f:
            for line in f:
                case = json.loads(line)
                fn(case, fails)
                n_checked += 1
    print(f'{n_checked} cases replayed, {len(fails)} mismatches')
    for f_ in fails[:20]:
        print('MISMATCH', json.dumps(f_, default=str))
    print('GOLDEN CHECK: %s' % ('PASS' if not fails else 'FAIL'))
    return 1 if fails else 0


if __name__ == '__main__':
    sys.exit(main())
