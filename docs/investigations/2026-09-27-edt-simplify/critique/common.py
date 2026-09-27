import sys, math, importlib.util
sys.dont_write_bytecode = True
import numpy as np
AG = 'B:/repos/I-Simpa_Night_Mode/target/agents'

def _load(name, path):
    s = importlib.util.spec_from_file_location(name, path); m = importlib.util.module_from_spec(s); s.loader.exec_module(m); return m

CANDS = {n: _load('c_' + n, f'{AG}/edt-simplify/cand_{n}/method.py') for n in ('minimal', 'leanband', 'practice')}

def run_all(bins, dt, t_arr, meta=None):
    out = {}
    for n, m in CANDS.items():
        r = m.analyse(np.asarray(bins, float), dt, t_arr, dict(meta or {}))
        out[n] = r
    return out

def score(r, truth, jnd=0.05):
    """-> (kind, err) kind in ok_good, ok_WRONG, wide, wide_rangebad, refused"""
    if r.get('edt') is None or r.get('status') == 'refused':
        return 'refused', None
    e = r['edt'] / truth - 1
    inside = r['edt_lo'] is not None and r['edt_hi'] is not None and r['edt_lo'] <= truth <= r['edt_hi']
    if r['status'] == 'ok':
        return ('ok_WRONG' if abs(e) > jnd else 'ok_good'), e
    return ('wide_in' if inside else 'wide_out'), e
