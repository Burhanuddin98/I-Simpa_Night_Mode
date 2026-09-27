"""Read-only probe: regenerate specific ISM eval rows (same construction as the evaluator's
loaders.load_ism), run the three candidates, and recompute the truth on (a) the eval's truncated
image set (lmax = C*T) and (b) an extended image set (lmax = C*T*ext) to see whether the eval
truth is the room's EDT or the EDT of a truncated series. Single process, numpy only."""
import math, sys, time, importlib.util
sys.dont_write_bytecode = True
import numpy as np
AG = 'B:/repos/I-Simpa_Night_Mode/target/agents'
sys.path.insert(0, AG + '/followup-design/skeptic-gf3'); sys.path.insert(0, AG + '/followup-design/skeptic-gf3/rerun')
import ism, attack_ism as AI

def load(name, path):
    s = importlib.util.spec_from_file_location(name, path); m = importlib.util.module_from_spec(s); s.loader.exec_module(m); return m
CANDS = {n: load(n, f'{AG}/edt-simplify/cand_{n}/method.py') for n in ('minimal', 'leanband', 'practice')}

def lvl_direct(direct, refl):
    tot = direct.sum() + refl.sum()
    # level of the Schroeder curve just after the direct sound (reflections before t lumped in)
    return 10 * math.log10(1 - direct.sum() / tot)

def probe(room, rec, F, ms, ext=1.0, R=AI.R_DEFAULT):
    L, alpha, src, T = AI.ROOMS[room]
    c = AI.C; DTF = AI.DT_F; dl = c * DTF
    d = math.dist(src, rec); t = d / c
    out = {}
    for tag, lmax in (('eval', c * T), ('ext', c * T * ext)):
        if tag == 'ext' and ext == 1.0:
            continue
        t0 = time.time()
        direct, refl = ism.echogram(L, src, rec, R, alpha, lmax, dl)
        m = ism.m_energy(F)
        ac = ism.air_factor(len(direct), dl, m, c, None)
        te, ts = AI.truth_ideal(direct * ac, refl * ac, t, DTF)
        k = int(round(ms * 1e-3 / DTF))
        vc = (direct + refl) * ism.air_factor(len(direct), dl, m, c, k * DTF)
        n = len(vc) // k
        vb = vc[:n * k].reshape(n, k).sum(1)
        nz = np.nonzero(vb > 0)[0]; vb = vb[:int(nz[-1]) + 1]
        res = {nm: C.analyse(vb, k * DTF, t, {}) for nm, C in CANDS.items()}
        out[tag] = dict(truth=te, lvl_direct=lvl_direct(direct * ac, refl * ac), n_bins=len(vb),
                        res={nm: (r['edt'], r['edt_lo'], r['edt_hi'], r['status'], r['reason']) for nm, r in res.items()},
                        secs=time.time() - t0)
    return out

if __name__ == '__main__':
    cases = [
        ('corridor', (5.45, 2.53, 1.53), 125, 1.0),
        ('corridor', (5.53, 1.97, 1.53), 125, 2.0),
        ('dead', (2.82, 2.37, 1.53), 8000, 2.0),
        ('dead', (2.52, 2.67, 1.53), 1000, 1.0),
        ('deader', (4.0, 0.4, 2.65), 16000, 2.0),
    ]
    for room, rec, F, ms in cases:
        ext = 2.0 if room == 'corridor' else 3.0
        o = probe(room, rec, F, ms, ext)
        print(f'--- {room} rec={rec} F={F} step={ms}ms  d={math.dist(AI.ROOMS[room][2], rec):.2f} m')
        for tag, v in o.items():
            print(f'  [{tag}] truth_ideal={v["truth"]:.4f}  lvl_after_direct={v["lvl_direct"]:.2f} dB  n_bins={v["n_bins"]}  ({v["secs"]:.1f}s)')
            for nm, r in v['res'].items():
                e = r[0]
                err = '' if e is None else '%+.1f%%' % (100 * (e / v['truth'] - 1))
                print(f'     {nm:9s} edt={e} lo={r[1]} hi={r[2]} {r[3]} {r[4]}  err={err}')
        sys.stdout.flush()
