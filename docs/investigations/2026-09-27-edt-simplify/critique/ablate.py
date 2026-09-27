"""Ablations: does removing a part change any outcome? Regenerates a sample of this critique's
synthetic rows (direct/origin grid at three run lengths, occluded receivers) plus the eval's t1
golden rows, and runs the original module against an ablated copy. Single process, numpy only."""
import sys, math, itertools, importlib.util, collections, json, base64, warnings
sys.dont_write_bytecode = True
warnings.filterwarnings('ignore')
import numpy as np
import synth as S
AG = 'B:/repos/I-Simpa_Night_Mode/target/agents'


def load(tag, n):
    s = importlib.util.spec_from_file_location(tag, AG + '/edt-simplify/cand_' + n + '/method.py')
    m = importlib.util.module_from_spec(s)
    s.loader.exec_module(m)
    return m


def gen_rows():
    R = 0.31
    h = R / S.C
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
                yield S.histogram(dt, t_arr + frac * T60, t_arr, h, Ed, gap_ms * 1e-3, [1.0], [k]), dt, t_arr
    for T60, delay_ms, dt_ms in itertools.product((0.4, 0.8, 1.5), (5.0, 20.0), (1.0, 10.0)):
        dt = dt_ms * 1e-3
        k = 6 * math.log(10) / T60
        t_arr = 0.03 + 0.41 * dt
        yield S.histogram(dt, t_arr + 2 * T60, t_arr, 0.0, 0.0, delay_ms * 1e-3, [1.0], [k]), dt, t_arr
    for line in open(AG + '/edt-band/golden/t1.jsonl'):
        c = json.loads(line)
        inp = c['inputs']
        B = inp['B']
        a = np.frombuffer(base64.b64decode(B['b64']), dtype=np.dtype(B['dtype'])).reshape(B['shape']).astype(float)
        yield a, float(inp['dt']), float(inp['t_arr'])


def key(r):
    f = lambda x: None if x is None else round(float(x), 12)
    return (r['status'], f(r['edt']), f(r['edt_lo']), f(r['edt_hi']))


rows = list(gen_rows())
print('rows', len(rows))
tests = []
lb0 = load('lb0', 'leanband')
lbA = load('lbA', 'leanband'); lbA.ARRIVAL_SEARCH = 0
lbP = load('lbP', 'leanband'); lbP.PERSIST_BINS = 1
lbG = load('lbG', 'leanband')
_orig_ols = lbG._ols


def _ols_r2_high(t, y):
    r = _orig_ols(t, y)
    return r if r is None else (r[0], r[1], 1.0, r[3])   # r2 forced to 1: removes the r2>=0.5 gate


lbG._ols = _ols_r2_high
tests += [('leanband: ARRIVAL_SEARCH=0 (no arrival nudge)', lb0, lbA),
          ('leanband: PERSIST_BINS=1 (no 2-bin persistence)', lb0, lbP),
          ('leanband: r2>=0.5 gate removed', lb0, lbG)]
pr0 = load('pr0', 'practice')
prT = load('prT', 'practice'); prT.TAIL_ITERATIONS = 1
tests += [('practice: TAIL_ITERATIONS=1 (tail refusal kept, correction not applied)', pr0, prT)]
mi0 = load('mi0', 'minimal')
miN = load('miN', 'minimal')
_orig_fit = miN._fit_decay


def _fit_nonear(*a, **k):
    f = _orig_fit(*a, **k)
    if 'near_end' in f:
        f['near_end'] = False
    return f


miN._fit_decay = _fit_nonear
tests += [('minimal: near_run_end flag off', mi0, miN)]
for name, A, B in tests:
    ch_status = ch_value = 0
    for b, dt, ta in rows:
        ra, rb = A.analyse(b, dt, ta, {}), B.analyse(b, dt, ta, {})
        if ra['status'] != rb['status']:
            ch_status += 1
        elif key(ra) != key(rb):
            ch_value += 1
    print(name + ': status changed on %d, value/range changed on %d of %d' % (ch_status, ch_value, len(rows)))
