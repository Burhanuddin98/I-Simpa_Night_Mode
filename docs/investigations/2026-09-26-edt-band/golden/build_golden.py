"""GATE 4: freeze the golden parity corpus a future Rust port must reproduce bit for bit.

Builds golden/*.jsonl (one JSON object per line) plus MANIFEST.json. Python only, single process,
no solver run: every histogram here comes from this repo's own deterministic generators
(test_band_early.py's T1 generator, an archived adversary histogram, band_early's own Setup/
production_inputs calls, and the judge's synthetic Echo model used unchanged from eval_run.py) --
never from I-Simpa's SPPS/TCR solvers.

Usage: python -B golden/build_golden.py [--skip-z3]
"""
import base64
import hashlib
import json
import math
import os
import sys
import time

sys.dont_write_bytecode = True
os.environ['PYTHONDONTWRITEBYTECODE'] = '1'
import numpy as np  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
AG = os.path.dirname(ROOT)
sys.path.insert(0, ROOT)
import band_early as be  # noqa: E402
import test_band_early as TB  # noqa: E402

OUT = HERE


def arr(a):
    """Store a float64/float32 array exactly: base64 of its raw bytes plus dtype+shape, so a
    reload is bit-identical (JSON floats would round-trip fine too, but base64 is exact and
    compact for the larger histograms)."""
    a = np.asarray(a)
    return dict(dtype=str(a.dtype), shape=list(a.shape), b64=base64.b64encode(a.tobytes()).decode('ascii'))


def unarr(d):
    a = np.frombuffer(base64.b64decode(d['b64']), dtype=np.dtype(d['dtype']))
    return a.reshape(d['shape'])


def band_output(res, q):
    r = res[q]
    b = r.get('band')
    out = dict(lo=(None if not b else float(b[0])), hi=(None if not b else float(b[1])),
               value=(None if r.get('value') is None else float(r['value'])),
               refused=r.get('refused'))
    if q == 'edt':
        out['witness'] = [float(x) for x in r['witness']] if r.get('witness') else None
        out['n_eval'] = r.get('n_eval')
        out['budget_exhausted'] = r.get('budget_exhausted')
        u = (r.get('U_min'), r.get('U_max'))
        out['U'] = [None if u[0] is None else float(u[0]), None if u[1] is None else float(u[1])]
        out['n0'] = r.get('n_initial') if 'n_initial' in r else r.get('window_bins')
    return out


def full_outputs(res):
    return {q: band_output(res, q) for q in ('edt', 'ts', 'c50', 'c80', 'd50')}


def write_group(name, cases):
    path = os.path.join(OUT, f'{name}.jsonl')
    with open(path, 'w') as f:
        for c in cases:
            f.write(json.dumps(c, default=float) + '\n')
    return path, len(cases)


# --------------------------------------------------------------------------------------------
# Group T1: the full closed-form exponential-decay sweep (T1-T7 brief item), fully deterministic.
# --------------------------------------------------------------------------------------------
def build_t1():
    cases = []
    ta = 0.01234
    T60 = 0.5
    for dt in (2e-3, 1e-3, 5e-4, 2e-4, 1e-4):
        for air in ('none', '1k', '16k', '20k'):
            a = TB.AIR[air]
            k = 6 * math.log(10) / T60
            for dname, h in (('impulse', 0.0), ('ball R=0.31', 0.31 / TB.C)):
                for gap in (0.0, 2.5e-3):
                    for prec in ('f64', 'f32'):
                        tr = ta + gap
                        Ed = 0.5 * (1.0 / k) * math.exp(-k * tr)
                        A = 1.0
                        dtau, dE = TB.ball_direct_atoms(ta, h, Ed)
                        n_bins = int(math.ceil(3.0 / dt))
                        B, rem = TB.expo_bins(dt, ta, dtau, dE, A, k, tr, a, n_bins)
                        eps = 0.0
                        if prec == 'f32':
                            B = B.astype(np.float32).astype(np.float64)
                            eps = 2.0 ** -24
                        truth = TB.expo_truth(ta, Ed, A, k, tr)
                        t_refl_min = tr if gap > 0 else None
                        tail_max = rem * 1.001
                        tail_t_max = n_bins * dt + 100.0
                        s = be.Setup(B, dt, ta, half_width=h, air_rate=a, rel_eps=eps, tail_max=tail_max,
                                     tail_t_max=tail_t_max, t_refl_min=t_refl_min)
                        res = s.all_bands()
                        case = dict(
                            group='t1', id=f't1|{dt}|{air}|{dname}|{gap}|{prec}',
                            inputs=dict(B=arr(B), dt=dt, t_arr=ta, half_width=h, air_rate=a,
                                        rel_eps=eps, tail_max=tail_max, tail_t_max=tail_t_max,
                                        t_refl_min=t_refl_min, kappa=1.0),
                            setup=dict(air=air, direct=dname, gap=gap, prec=prec, n_bins=n_bins),
                            truth=truth, outputs=full_outputs(res))
                        cases.append(case)
    return cases


# --------------------------------------------------------------------------------------------
# Group edge_cases: one per refusal code, plus the brief's named edge cases.
# --------------------------------------------------------------------------------------------
def build_edge_cases():
    cases = []

    # not_decaying -- the archived adversary histogram E1 (round1/attack1_edge.json), the exact
    # input that raised ZeroDivisionError before round 2's fix.
    E = json.load(open(os.path.join(ROOT, 'round1', 'attack1_edge.json')))['E1']
    c = [x for x in E if x.get('hit')][0]
    B = np.array(c['B'], dtype=np.float64)
    s = be.Setup(B, c['dt'], c['ta'], half_width=c['h'], air_rate=c['a'])
    band, info = s.edt_band()
    cases.append(dict(
        group='edge_cases', id='not_decaying|attack1_E1',
        inputs=dict(B=arr(B), dt=c['dt'], t_arr=c['ta'], half_width=c['h'], air_rate=c['a'],
                    rel_eps=0.0, tail_max=0.0, tail_t_max=None, t_refl_min=None, kappa=1.0),
        setup=dict(note='archived adversary histogram, round1/attack1_edge.json:E1'),
        truth=None,
        outputs=dict(edt=dict(lo=(None if band is None else float(band[0])),
                               hi=(None if band is None else float(band[1])),
                               value=None, refused=info.get('refused'),
                               witness=info.get('witness'), n_eval=info.get('n_eval'),
                               budget_exhausted=info.get('budget_exhausted'),
                               U=[info.get('U_min'), info.get('U_max')], n0=info.get('n_initial')))))

    # premise_unsupported -- two sources (t7_unsupported's own construction).
    S = TB.random_series(1234)
    s2 = be.Setup(S['B'], S['dt'], S['ta'], air_rate=S['a'], n_sources=2)
    r2 = s2.all_bands(upstream=False)
    cases.append(dict(
        group='edge_cases', id='premise_unsupported|two_sources',
        inputs=dict(B=arr(S['B']), dt=S['dt'], t_arr=S['ta'], half_width=0.0, air_rate=S['a'],
                    rel_eps=0.0, tail_max=0.0, tail_t_max=None, t_refl_min=None, kappa=1.0,
                    n_sources=2),
        setup=dict(note='test_band_early.random_series(1234), n_sources=2'),
        truth=None, outputs=full_outputs(r2)))

    # empty arrival bin -- B[onset] == 0, energy only later.
    dt = 1e-3
    ta = 0.01
    B = np.zeros(200)
    B[50:] = np.exp(-6.0 * np.arange(150) * dt)
    s3 = be.Setup(B, dt, ta, half_width=0.0, air_rate=0.0, tail_max=0.0)
    r3 = s3.all_bands()
    cases.append(dict(
        group='edge_cases', id='empty_arrival_bin',
        inputs=dict(B=arr(B), dt=dt, t_arr=ta, half_width=0.0, air_rate=0.0, rel_eps=0.0,
                    tail_max=0.0, tail_t_max=None, t_refl_min=None, kappa=1.0),
        setup=dict(note='B[0]=0, energy begins 50 bins after the onset step'),
        truth=None, outputs=full_outputs(r3)))

    # direct sound only -- all energy in the direct-window atom, nothing reflected.
    dt = 1e-3
    ta = 0.01
    B = np.zeros(50)
    B[0] = 1.0
    s4 = be.Setup(B, dt, ta, half_width=0.31 / TB.C, air_rate=0.0, tail_max=0.0)
    r4 = s4.all_bands()
    cases.append(dict(
        group='edge_cases', id='direct_sound_only',
        inputs=dict(B=arr(B), dt=dt, t_arr=ta, half_width=0.31 / TB.C, air_rate=0.0, rel_eps=0.0,
                    tail_max=0.0, tail_t_max=None, t_refl_min=None, kappa=1.0),
        setup=dict(note='all recorded energy in bin 0, inside the direct-sound half-width ball'),
        truth=None, outputs=full_outputs(r4)))

    # truncated series -- very short recorded series with an explicit unrecorded tail_max, so the
    # gate-2 style bound dominates.
    dt = 1e-3
    ta = 0.01
    n = 20
    k = 6 * math.log(10) / 0.5
    B = np.exp(-k * np.arange(n) * dt) * dt
    s_onset = float(B.sum())
    s5 = be.Setup(B, dt, ta, half_width=0.0, air_rate=0.0, tail_max=0.5 * s_onset,
                  tail_t_max=n * dt + 10.0)
    r5 = s5.all_bands()
    cases.append(dict(
        group='edge_cases', id='truncated_series|large_unrecorded_tail',
        inputs=dict(B=arr(B), dt=dt, t_arr=ta, half_width=0.0, air_rate=0.0, rel_eps=0.0,
                    tail_max=0.5 * s_onset, tail_t_max=n * dt + 10.0, t_refl_min=None, kappa=1.0),
        setup=dict(note=f'series cut at {n} bins with tail_max = 50% of S(onset)'),
        truth=None, outputs=full_outputs(r5)))

    # run_too_short -- GATE 4b's replacement refusal (decision-log row 14; the old
    # quantity_tolerance comparison mixed units and is removed, band_early.production_inputs()
    # raises TypeError if passed it now). Plausible construction from a real production run
    # (V-E5, noise-cal-1790307822, seed01, receiver 0, band 0), not a contrived array: the
    # recorded series is truncated to n_steps=100 of its 1500 fine (1 ms) bins, at which point the
    # room's alive share (total_energy[n_steps-1] / total_energy[onset]) is 0.0563 -- inside the
    # 1e-2..1e-1 band the brief asks for, i.e. a genuinely too-short run, not an edge worse than any
    # real one. production_inputs()'s x1000-inflated tail_max then makes D50's tail_widening (the
    # halfwidth D50 gains from the tail alone, band_early.Setup.all_bands) 0.4083 vs. D50's own tau
    # 0.005 -- 82x GATE2_TAIL_REFUSAL_TOLERANCE_SHARE * tau -- so D50 (and C50/C80) are refused
    # 'run_too_short'; EDT/Ts are refused for other, unrelated reasons at this truncation
    # ('range_not_reached'/'truncated') and are not part of this case's point.
    real_report = os.path.join(
        AG, 'pm8-noise-scratch', 'runs', 'noise-cal-1790307822', 'V-E5', 'seed01', 'report.json')
    rd = json.load(open(real_report))['spps']
    onset_step = 4
    n_steps = 100
    total_energy = np.asarray(rd['total_energy'][0]['energy'], dtype=np.float64)[:n_steps]
    bnd0 = rd['point_receivers'][0]['bands'][0]
    B = np.asarray(bnd0['energy_pa2'], dtype=np.float64)[:n_steps]
    t_arr = float(rd['point_receivers'][0]['arrival_s'])
    dt = float(rd['time_step_s'])
    R = float(rd['receiver_radius_m'])
    speed_c = float(rd['speed_of_sound_m_s'])
    half_width = R / speed_c
    pps = int(rd['particles_per_source'])
    n_sources = len(rd['sources'])
    method = int(rd['computation_method'])
    trans_epsilon = rd.get('trans_epsilon')
    pstat = rd['particles']['bands'][0]
    killed = int(pstat['absorbed_by_atmosphere'] + pstat['absorbed_by_materials'] + pstat['absorbed_by_fittings'])
    alive_share = float(total_energy[-1] / total_energy[onset_step])
    tail_max, eps, detail = be.production_inputs(
        B, dt, onset_step, total_energy, pps, n_sources=n_sources, trans_epsilon=trans_epsilon,
        computation_method=method, killed_particles=killed)
    s_refuse = be.Setup(B, dt, t_arr, half_width=half_width, air_rate=0.0, rel_eps=eps,
                         tail_max=tail_max, t_refl_min=max(0.0, t_arr - half_width))
    r_refuse = s_refuse.all_bands(upstream=False)
    cases.append(dict(
        group='edge_cases', id='run_too_short|gate4b_real_truncated',
        inputs=dict(B=arr(B), dt=dt, t_arr=t_arr, half_width=half_width, air_rate=0.0,
                    rel_eps=arr(eps), tail_max=tail_max, tail_t_max=None,
                    t_refl_min=max(0.0, t_arr - half_width), kappa=1.0),
        setup=dict(note=('V-E5/seed01 receiver 0 band 0, truncated to n_steps=%d of its fine bins; '
                          'alive_share=%.4g (target 1e-2..1e-1); production_inputs() detail: %s' %
                          (n_steps, alive_share, {k: (float(v) if isinstance(v, (int, float, np.floating)) else v)
                                                  for k, v in detail.items()}))),
        truth=None, outputs=full_outputs(r_refuse)))

    # run_too_short, boundary case just under the threshold -- same real receiver/band, FULL fine
    # series (n_steps=1500, no truncation): alive_share is then ~0 (the receiver has genuinely
    # decayed away), but production_tail_max's floor-kill term (b) is still present (this room used
    # the energetic method with a finite trans_epsilon), so tail_max is not exactly 0. D50's
    # tail_widening here is 3.897e-5 vs. tau 0.005 -- 0.78% of tau, just under the 1%
    # GATE2_TAIL_REFUSAL_TOLERANCE_SHARE -- so D50 is NOT refused: the same mechanism, same
    # receiver, on the side of the line that is trusted.
    total_energy_full = np.asarray(rd['total_energy'][0]['energy'], dtype=np.float64)
    B_full = np.asarray(bnd0['energy_pa2'], dtype=np.float64)
    tail_max_f, eps_f, detail_f = be.production_inputs(
        B_full, dt, onset_step, total_energy_full, pps, n_sources=n_sources, trans_epsilon=trans_epsilon,
        computation_method=method, killed_particles=killed)
    s_ok = be.Setup(B_full, dt, t_arr, half_width=half_width, air_rate=0.0, rel_eps=eps_f,
                     tail_max=tail_max_f, t_refl_min=max(0.0, t_arr - half_width))
    r_ok = s_ok.all_bands(upstream=False)
    cases.append(dict(
        group='edge_cases', id='run_too_short|gate4b_real_full_just_under',
        inputs=dict(B=arr(B_full), dt=dt, t_arr=t_arr, half_width=half_width, air_rate=0.0,
                    rel_eps=arr(eps_f), tail_max=tail_max_f, tail_t_max=None,
                    t_refl_min=max(0.0, t_arr - half_width), kappa=1.0),
        setup=dict(note=('same V-E5 receiver/band, full %d-bin series (no truncation); D50 tail_widening '
                          '/ tau = %.4g, just under the 1%% refusal share' %
                          (len(B_full), r_ok['d50'].get('tail_widening', float('nan')) / r_ok['d50']['tau']))),
        truth=None, outputs=full_outputs(r_ok)))

    return cases


def main():
    t0 = time.time()
    groups = {}
    print('building t1 ...')
    groups['t1'] = build_t1()
    print(f'  {len(groups["t1"])} cases')
    print('building edge_cases ...')
    groups['edge_cases'] = build_edge_cases()
    print(f'  {len(groups["edge_cases"])} cases')

    manifest = dict(built_utc=time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()), files={})
    for name, cases in groups.items():
        path, n = write_group(name, cases)
        h = hashlib.sha256(open(path, 'rb').read()).hexdigest()
        size = os.path.getsize(path)
        manifest['files'][os.path.basename(path)] = dict(cases=n, sha256=h, bytes=size)
        print(f'{name}: {n} cases, {size} bytes, sha256 {h[:12]}...')

    json.dump(manifest, open(os.path.join(OUT, 'MANIFEST.json'), 'w'), indent=1)
    print('MANIFEST.json written. Total time %.1fs' % (time.time() - t0))


if __name__ == '__main__':
    main()
