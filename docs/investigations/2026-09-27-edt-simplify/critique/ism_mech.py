"""Mechanism check on two ISM eval rows: pass t_arrival - R/c (the front of the receiver ball) instead
of the centre arrival d/c, and show which candidate's error disappears."""
import math, sys
sys.dont_write_bytecode = True
import numpy as np
from ism_probe import ism, AI, CANDS
for room, rec, F, ms in (('corridor', (5.45, 2.53, 1.53), 125, 1.0), ('corridor', (5.53, 1.97, 1.53), 125, 2.0),
                         ('dead', (2.52, 2.67, 1.53), 1000, 1.0)):
    L, alpha, src, T = AI.ROOMS[room]
    c = AI.C; DTF = AI.DT_F; dl = c * DTF; R = AI.R_DEFAULT; h = R / c
    d = math.dist(src, rec); t = d / c
    direct, refl = ism.echogram(L, src, rec, R, alpha, c * T, dl)
    m = ism.m_energy(F); ac = ism.air_factor(len(direct), dl, m, c, None)
    te, _ = AI.truth_ideal(direct * ac, refl * ac, t, DTF)
    k = int(round(ms * 1e-3 / DTF)); dt = k * DTF
    vc = (direct + refl) * ism.air_factor(len(direct), dl, m, c, dt)
    n = len(vc) // k; vb = vc[:n * k].reshape(n, k).sum(1); vb = vb[:int(np.nonzero(vb > 0)[0][-1]) + 1]
    dirb = (direct * ism.air_factor(len(direct), dl, m, c, dt))[:n * k].reshape(n, k).sum(1)
    k0 = int(t // dt)
    print(f'--- {room} {rec} {F}Hz {ms}ms  t_arr={t*1e3:.3f} ms  k0=floor={k0} round={int(round(t/dt))}  truth={te:.4f}')
    print('    direct energy per bin (share of total direct):', np.round(dirb[:k0 + 3] / dirb.sum(), 3))
    for label, ta in (('t_arr=d/c', t), ('t_arr=(d-R)/c', t - h)):
        for nm, Cm in CANDS.items():
            r = Cm.analyse(vb, dt, ta, {})
            e = r['edt']
            print(f'    {label:14s} {nm:9s} edt={e if e is None else round(e,4)} status={r["status"]} err={"" if e is None else "%+.1f%%" % (100*(e/te-1))} reason={str(r["reason"])[:50]}')
