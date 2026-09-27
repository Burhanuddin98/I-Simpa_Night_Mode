"""GATE 4b real-run check (2026-09-27): re-run gate2_real_check.py's same 3,600 real rows
(load_report datasets, x1000-inflated tail_max via band_early.production_inputs/production_tail_max,
unchanged from GATE 2) but now also exercise the new per-quantity tail-widening 'run_too_short'
refusal (band_early.Setup.all_bands, GATE 4b fix) instead of the old dimensionally-broken
quantity_tolerance comparison. Read-only; no solver run. Single Python process (this one).

Difference from gate2_real_check.py:
  - band_check() uses the real LIMIT tau (band_early.LIMIT) instead of tau=1e9, so both
    'band_too_wide' and the new 'run_too_short' can actually fire, and reports refused/tail_widening
    per quantity.
  - containment (truth inside band) is reported only on rows where that quantity was NOT refused
    for any reason (a refused quantity has no band to check truth against in production use).
  - per-quantity refusal counts (run_too_short specifically, plus any other refusal seen) and the
    tail_widening distribution (median/max, in units of that quantity's own tau) are recorded per
    dataset and overall.
"""
import glob
import json
import math
import os
import sys

sys.dont_write_bytecode = True
os.environ['PYTHONDONTWRITEBYTECODE'] = '1'
import numpy as np  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import band_early as be  # noqa: E402

AG = 'B:/repos/I-Simpa_Night_Mode/target/agents'
CAL1 = f'{AG}/pm8-noise-scratch/runs/noise-cal-1790307822'
CAL2 = f'{AG}/pm8-noise-scratch/runs/noise-cal-1790310131'
SPEC_DIR = f'{AG}/followup-design/spec'


def load_cells(root):
    cs = json.load(open(f'{root}/cells.json'))
    out = []
    for c in cs:
        cc = c if 'id' in c else c['cell']
        if abs(cc['time_step_s'] - 0.001) > 1e-9:
            continue     # real_rows.pkl / datasets() only keeps the 1 ms cells
        out.append(cc)
    return out


def band_check(B, dt, t_arr, h, eps, tail_max, tail_t_max, trm):
    """Real tau (band_early.LIMIT), so band_too_wide and the new run_too_short refusal can both
    fire; returns per-quantity band/refused/tail_widening/tau."""
    s = be.Setup(np.asarray(B, float), dt, t_arr, half_width=h, air_rate=0.0, kappa=1.0,
                 rel_eps=eps, tail_max=tail_max, tail_t_max=tail_t_max, t_refl_min=trm)
    res = s.all_bands(upstream=False)
    out = {}
    for q in ('edt', 'ts'):
        r = res[q]
        bd = r.get('band')
        out[q] = dict(band=(bd[0], bd[1]) if bd else None, refused=r.get('refused'),
                      tail_widening=r.get('tail_widening'), tau=r.get('tau'))
    return out


def inside(band, truth):
    if band is None or truth is None or not math.isfinite(truth):
        return None
    lo, hi = band
    return (lo - 1e-9) <= truth <= (hi + 1e-9)


def run_cell(root, cc, rows_by_key, k=2):
    """k: the rebin factor to the registered coarse step (real_rows.pkl's smallest registered ms
    for these 1 ms-fine cells is 2 ms, i.e. k=2 of the fine dt)."""
    cell_id = cc['id']
    seeds = sorted(glob.glob(f"{root}/{cell_id}/seed*/report.json"))
    out_rows = []
    refusals = 0
    for si, rp in enumerate(seeds):
        d = json.load(open(rp))['spps']
        method = d['computation_method']
        trans_eps = d.get('trans_epsilon')
        pps = d['particles_per_source']
        n_src = len(d['sources'])
        c = d['speed_of_sound_m_s']
        R = d['receiver_radius_m']
        h = R / c
        fine_dt = d['time_step_s']
        for bi, freq in enumerate([b['freq_hz'] for b in d['point_receivers'][0]['bands']]):
            te = d['total_energy'][bi]['energy']
            pstat = d['particles']['bands'][bi]
            killed = pstat['absorbed_by_atmosphere'] + pstat['absorbed_by_materials'] + pstat['absorbed_by_fittings']
            for pr in d['point_receivers']:
                label = pr['label']
                key = (cell_id, si, label, freq)
                reg = rows_by_key.get(key)
                if not reg:
                    continue
                bnd = next(b for b in pr['bands'] if b['freq_hz'] == freq)
                v_fine = np.asarray(bnd['energy_pa2'], float)
                t_arr = pr['arrival_s']
                onset_step = bnd['onset']['index'] if bnd.get('onset') else 0
                try:
                    tail_prod, eps_fine, detail = be.production_inputs(
                        v_fine, fine_dt, onset_step, te, pps, n_sources=n_src,
                        trans_epsilon=trans_eps, computation_method=method, killed_particles=killed)
                except Exception as ex:
                    refusals += 1
                    out_rows.append(dict(key=key, error=repr(ex)[:200]))
                    continue
                # rebin to the registered coarse step; the rebin remainder is exact (not modelled)
                # and is added to the production tail, same convention as the existing harness.
                n = len(v_fine) // k
                v = v_fine[:n * k].reshape(n, k).sum(1)
                rest = float(v_fine[n * k:].sum())
                dt = fine_dt * k
                eps = eps_fine[:n * k].reshape(n, k).max(1)   # the coarser bin's eps is at least
                                                              # any fine bin folded into it
                tail_max = tail_prod + rest
                trm = max(0.0, t_arr - h)   # b0 variant: nothing beyond the direct window
                bandres = band_check(v, dt, t_arr, h, eps, tail_max, None, trm)
                truth = reg['truth']
                edt_r, ts_r = bandres['edt'], bandres['ts']
                # containment is checked against the band whenever one exists, REGARDLESS of
                # 'refused': _package still fills out['band'] = (lo, hi) for band_too_wide/
                # run_too_short (the band is a valid, sound interval either way -- 'refused' means
                # only "wider than this quantity's own tau", not "no band was computed"). Gating
                # containment on refused is None here would report edt_checked=0 across the board
                # the moment real LIMIT tau (rather than the old script's tau=1e9) makes refusals
                # common, which defeats the point of checking containment at all.
                row = dict(key=list(key), tail_max=tail_max, tail_fraction=detail['tail_fraction'],
                           kill_frac=detail.get('kill_frac', 0.0), f_end=detail['f_end'],
                           edt_refused=edt_r['refused'], ts_refused=ts_r['refused'],
                           edt_tail_widening=edt_r['tail_widening'], ts_tail_widening=ts_r['tail_widening'],
                           edt_tau=edt_r['tau'], ts_tau=ts_r['tau'],
                           edt_inside=inside(edt_r['band'], truth.get('edt')),
                           ts_inside=inside(ts_r['band'], truth.get('ts')),
                           edt_band=edt_r['band'], ts_band=ts_r['band'], truth=truth)
                out_rows.append(row)
    return out_rows, refusals


def widening_in_tau(rows, q):
    """tail_widening / tau for rows where both are known (tau > 0), q in ('edt', 'ts')."""
    out = []
    for r in rows:
        w, tau = r.get(f'{q}_tail_widening'), r.get(f'{q}_tau')
        if w is not None and tau:
            out.append(w / tau)
    return out


def main():
    import pickle
    rows = pickle.load(open(f'{SPEC_DIR}/real_rows.pkl', 'rb'))
    rows_by_key = {}
    for r in rows:
        if r['ds'] not in ALLOWED:
            continue
        if abs(r['ms'] - 2.0) > 1e-9:
            continue
        rows_by_key[(r['ds'], r['run'], r['rec'], r['F'])] = r

    all_out = []
    n_refusals = 0
    per_dataset = {}
    for root in (CAL1, CAL2):
        for cc in load_cells(root):
            if cc['id'] not in ALLOWED:
                continue
            out_rows, ref = run_cell(root, cc, rows_by_key)
            all_out.extend(out_rows)
            n_refusals += ref
            ds = cc['id']
            edt_rts = sum(1 for r in out_rows if r.get('edt_refused') == 'run_too_short')
            ts_rts = sum(1 for r in out_rows if r.get('ts_refused') == 'run_too_short')
            edt_checked = [r for r in out_rows if r.get('edt_inside') is not None]
            ts_checked = [r for r in out_rows if r.get('ts_inside') is not None]
            w_edt = widening_in_tau(out_rows, 'edt')
            w_ts = widening_in_tau(out_rows, 'ts')
            edt_accepted = sum(1 for r in out_rows if r.get('edt_refused') is None)
            per_dataset[ds] = dict(
                rows=len(out_rows), production_input_refusals=ref,
                edt_run_too_short=edt_rts, ts_run_too_short=ts_rts, edt_accepted=edt_accepted,
                edt_checked=len(edt_checked), edt_outside=sum(1 for r in edt_checked if not r['edt_inside']),
                ts_checked=len(ts_checked), ts_outside=sum(1 for r in ts_checked if not r['ts_inside']),
                edt_widening_over_tau_median=(float(np.median(w_edt)) if w_edt else 0.0),
                edt_widening_over_tau_max=(float(np.max(w_edt)) if w_edt else 0.0),
                ts_widening_over_tau_median=(float(np.median(w_ts)) if w_ts else 0.0),
                ts_widening_over_tau_max=(float(np.max(w_ts)) if w_ts else 0.0),
            )
            print(f'{ds}: {len(out_rows)} rows, {ref} production_inputs refusals, '
                  f'{edt_rts} EDT run_too_short, {ts_rts} Ts run_too_short', flush=True)

    edt_checked = [r for r in all_out if r.get('edt_inside') is not None]
    ts_checked = [r for r in all_out if r.get('ts_inside') is not None]
    edt_out = [r for r in edt_checked if not r['edt_inside']]
    ts_out = [r for r in ts_checked if not r['ts_inside']]
    w_edt_all = widening_in_tau(all_out, 'edt')
    w_ts_all = widening_in_tau(all_out, 'ts')
    edt_rts_total = sum(1 for r in all_out if r.get('edt_refused') == 'run_too_short')
    ts_rts_total = sum(1 for r in all_out if r.get('ts_refused') == 'run_too_short')
    edt_accepted_total = sum(1 for r in all_out if r.get('edt_refused') is None)

    # compare against the pre-GATE-4b baseline (gate2_real_check.json: same rows, same x1000
    # tail_max, computed with tau=1e9 so nothing was ever refused there -- its edt_checked (3600)
    # is "accepted" under the old code path, since gate2_real_check.py never passed
    # quantity_tolerance either, so the old dimensionally-broken refusal never actually ran).
    # This run's edt_accepted (real LIMIT tau, new run_too_short refusal live) is the fair
    # comparison point, not this run's edt_checked (which counts containment regardless of refusal).
    old_path = os.path.join(HERE, 'gate2_real_check.json')
    old_edt_accepted = None
    if os.path.exists(old_path):
        old = json.load(open(old_path))
        old_edt_accepted = old['summary']['edt_checked']

    summary = dict(
        total_rows=len(all_out), production_input_refusals=n_refusals,
        edt_checked=len(edt_checked), edt_outside=len(edt_out),
        ts_checked=len(ts_checked), ts_outside=len(ts_out),
        edt_run_too_short=edt_rts_total, ts_run_too_short=ts_rts_total, edt_accepted=edt_accepted_total,
        edt_widening_over_tau_median=(float(np.median(w_edt_all)) if w_edt_all else 0.0),
        edt_widening_over_tau_max=(float(np.max(w_edt_all)) if w_edt_all else 0.0),
        ts_widening_over_tau_median=(float(np.median(w_ts_all)) if w_ts_all else 0.0),
        ts_widening_over_tau_max=(float(np.max(w_ts_all)) if w_ts_all else 0.0),
        max_kill_frac=max((r.get('kill_frac', 0.0) for r in all_out if 'kill_frac' in r), default=0.0),
        max_f_end=max((r.get('f_end', 0.0) for r in all_out if 'f_end' in r), default=0.0),
        old_gate2_edt_accepted=old_edt_accepted,
        edt_accepted_change=(None if old_edt_accepted is None else edt_accepted_total - old_edt_accepted),
        edt_outside_sample=edt_out[:5], ts_outside_sample=ts_out[:5],
        per_dataset=per_dataset,
    )
    json.dump(dict(summary=summary, rows=all_out), open(os.path.join(HERE, 'gate4b_real_check.json'), 'w'), indent=1)
    print(json.dumps(summary, indent=1, default=str))


ALLOWED = {'C-E3', 'C-E4', 'C-E6', 'V-E2', 'V-E5', 'C-R3', 'C-R4', 'C-R6', 'V-R2', 'V-R6'}

if __name__ == '__main__':
    main()
