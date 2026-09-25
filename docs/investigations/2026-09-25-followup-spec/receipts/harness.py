"""One series at one step, read by the shipped check and by every variant of REGISTERED.txt."""
import math
import sys

sys.path.insert(0, 'B:/repos/I-Simpa_Night_Mode/target/agents/followup-design/spec')
import composite as K  # noqa: E402
sys.path.insert(0, K.BR)
import common as C  # noqa: E402

ALL = ('G1', 'G2', 'G3', 'G3b')
VARIANTS = {
    'w1d': dict(n_extra=1, guards=(), exact_split=False, fixed_air=0.95),
    'W1G': dict(n_extra=1, guards=ALL),
    'W2G': dict(n_extra=2, guards=ALL),
    'W1G-G3': dict(n_extra=1, guards=('G1', 'G2', 'G3b')),
    'W1G-G3b': dict(n_extra=1, guards=('G1', 'G2', 'G3')),
    'W1G-G12': dict(n_extra=1, guards=('G3', 'G3b')),
}


def read_all(v, dt, t, h, d1, c, clear, R, m):
    """dict variant -> dict(edt=(ok, value, why), ts=(ok, value, why)), plus 'feat'."""
    out = {}
    sv = C.shipped(v, dt, t, h)
    if sv is None:
        out['ship'] = dict(edt=(False, None, 'detected'), ts=(False, None, 'detected'))
    else:
        e, s = sv
        out['ship'] = dict(edt=(bool(e['ok']), e['mid'], None if e['ok'] else 'early_unresolved'),
                           ts=(bool(s['ok']), s['mid'], None if s['ok'] else 'early_unresolved'))
    feat = None
    for name, kw in VARIANTS.items():
        air = kw.get('fixed_air')
        if air is None:
            air = K.air_factor(m, R, c, dt) if m is not None else None
        x = K.evaluate(v, dt, t, h, d1, c, clear, R, air, n_extra=kw['n_extra'], guards=kw['guards'],
                       exact_split=kw.get('exact_split', True))
        out[name] = dict(edt=(bool(x['edt']['ok']), x['edt'].get('value'), x['edt'].get('why')),
                         ts=(bool(x['ts']['ok']), x['ts'].get('value'), x['ts'].get('why')))
        if name == 'W1G':
            feat = x.get('feat', {})
    out['feat'] = feat or {}
    return out


def err(q, value, truth):
    if value is None or truth is None or not (isinstance(truth, float) and math.isfinite(truth)):
        return None
    if q == 'edt':
        return abs(value / truth - 1) / K.EDT_LIMIT
    return abs(value - truth) / C.ts_limit(truth)
