"""Smoke check of round 2's scorer on run 1's inputs (HARNESS-PLAN-2.md section 6, step 4 of the build brief): the scorer end to
end, frozen2/method.py at Z = 2.5 and upstream's port, on the 11,344 rows run 1 scored (B:\\data\\m8b-edt\\results\\run1\\inputs_*.pkl.gz),
compared with the Z = 2.5 block of target/agents/edt-v21/v21_zsweep.txt, which is DEV EVIDENCE: run 1's data shaped v2.1
(PREREG-2.md says so) and this check is not a test of the method. It checks that the harness reproduces the numbers the
method's author already had, so a scorer defect (a filter, a denominator, a pairing) cannot hide behind a method result.

Run 1's inputs carry no R_m (round 2 added it): it is taken from each row's own half_width, R = h x c, c 343.2 m/s
(343.20001220703125 for ISM, which that generator uses), as the v2.1 development script read it. Nothing is written but the
scorer's own output folder (the folder named on the command line; C:, never B:).

    python dry/smoke_run1.py <out folder> [--zsweep <v21_zsweep.txt>] [--run1 <results\\run1>]    (from harness2/)
Exit 0 when every compared number matches, 1 otherwise.
"""
import ast
import gzip
import json
import pickle
import re
import sys
from pathlib import Path

sys.path.insert(0, '.')
sys.dont_write_bytecode = True

from m8b import corpus, score  # noqa: E402

RUN1 = Path(r'B:\data\m8b-edt\results\run1')
ZSWEEP = Path(r'B:\repos\I-Simpa_Night_Mode\target\agents\edt-v21\v21_zsweep.txt')
C = {'spps': 343.2, 'synth': 343.2, 'ism': 343.20001220703125}
LABELS = ('spps-random', 'spps-energetic', 'ism', 'synth')
TABLE = re.compile(r'^(\S+)\s+(\d+)\s+(\d+)\s+(\d+) \|\s+(\d+)\s+(\d+)\s+([\d.]+)% \|\s+(\d+)\s+(\d+)\s+([\d.]+)%$')
H3 = re.compile(r'^\s+(\S+)\s+judged\s+(\d+) failing (\d+) ')
H4 = re.compile(r'^\s+(\S+)\s+all R: n=(\d+) usable ([\d.]+)% ok ([\d.]+)% \(rtl refused (\d+)\) \| R<=0.5 m: n=(\d+) usable ([\d.]+)% '
                r'ok ([\d.]+)% \(rtl refused (\d+)\)')
H5 = re.compile(r'^\s+(\S+)\s+Z25=(\d+) upstream=(\d+) ')
REF = re.compile(r'^\s+refusals (\S+)\s+(\{.*\})$')


def parse_z25(path):
    """The '===== Z25' block of v21_zsweep.txt as {'table': {label: (...)}, 'h3', 'h4', 'h5', 'refusals'}."""
    out = dict(table={}, h3={}, h4={}, h5={}, refusals={})
    on = False
    for line in Path(path).read_text(encoding='utf-8').splitlines():
        if line.startswith('====='):
            on = line.strip() == '===== Z25'
            continue
        if not on:
            continue
        for rx, key in ((TABLE, 'table'), (H3, 'h3'), (H4, 'h4'), (H5, 'h5'), (REF, 'refusals')):
            m = rx.match(line)
            if m:
                out[key][m.group(1)] = m.groups()[1:]
                break
    for k in out:
        want = set(LABELS) - ({'synth'} if k == 'h4' else set())          # H4 has no Synth-fresh line
        if set(out[k]) != want:
            raise ValueError('the Z25 block of %s has %s for %s, not %s' % (path, sorted(out[k]), k, sorted(want)))
    return out


def load_inputs(run1):
    inputs = []
    for name in ('spps', 'ism', 'synth'):
        with gzip.open(Path(run1) / ('inputs_%s.pkl.gz' % name), 'rb') as f:
            rows = pickle.load(f)
        for r in rows:
            r['R_m'] = round(r['meta']['half_width'] * C[name], 9)
        inputs.extend(rows)
    return inputs


def compare(summary, rows_jsonl, z):
    res = []

    def chk(what, want, got, tol=0.0):
        ok = (want == got) if not tol else abs(want - got) <= tol
        res.append((what, want, got, ok))

    crit = summary['criteria']['frozen']
    recs = [json.loads(x) for x in Path(rows_jsonl).read_text(encoding='utf-8').splitlines()]
    for lab in LABELS:
        t = summary['tables'][lab]['frozen']
        n, ok, usable, ok_t, ws, ws_pct, usable_t, cov, cov_pct = z['table'][lab]
        for what, want, got in (('n', n, t['n']), ('ok', ok, t['n_ok']), ('usable', usable, t['n_usable']),
                                ('ok_t', ok_t, t['n_ok_truth']), ('ws', ws, t['n_wrong_silent']),
                                ('usable_t', usable_t, t['n_usable_truth']), ('cov', cov, t['n_covered'])):
            chk('%s %s' % (lab, what), int(want), got)
        chk('%s ws%%' % lab, float(ws_pct), round(100.0 * t['n_wrong_silent'] / t['n_ok_truth'], 2), 0.0051)
        chk('%s cov%%' % lab, float(cov_pct), round(100.0 * t['n_covered'] / t['n_usable_truth'], 2), 0.0051)
        chk('%s refusals' % lab, ast.literal_eval(z['refusals'][lab][0]), t['refused'])
    # H3: judged subgroups and failing ones, per set (the dev script prints them per set)
    for lab in LABELS:
        mode = lab.split('-')[1] if lab.startswith('spps') else 'random'
        subs = [g for k, g in crit[mode]['H3']['subgroups'].items() if k.split('|')[0] == lab.split('-')[0]]
        judged, failing = int(z['h3'][lab][0]), int(z['h3'][lab][1])
        chk('%s H3 judged' % lab, judged, sum(g['judged'] for g in subs))
        chk('%s H3 failing' % lab, failing, sum(not g['pass'] for g in subs))
    # H5: wrong-silent against upstream's, per set
    for lab in LABELS:
        mode = lab.split('-')[1] if lab.startswith('spps') else 'random'
        p = crit[mode]['H5']['per_set'][lab.split('-')[0]]
        chk('%s H5 ours' % lab, int(z['h5'][lab][0]), p['n_wrong_silent'])
        chk('%s H5 upstream' % lab, int(z['h5'][lab][1]), p['n_wrong_silent_upstream'])
    # H4: the R <= 0.5 m population is round 2's H4; 'all R' is the same rows with the radius filter lifted
    def scored(r, R=None):
        f = r['frozen']
        return dict(set=r['set'], id=r['id'], mode=r['mode'], step_ms=r['step_ms'], design_t60_s=r['design_t60_s'],
                    R_m=r['R_m'] if R is None else R, status=f['status'], reason=f['reason'], edt=f['edt'], edt_lo=f['edt_lo'],
                    edt_hi=f['edt_hi'], truth=r['truth'], truth_status=r['truth_status'])

    for lab in ('spps-random', 'spps-energetic', 'ism'):
        st = lab.split('-')[0]
        mode = lab.split('-')[1] if lab.startswith('spps') else None
        mine = [r for r in recs if r['set'] == st and (mode is None or r['mode'] == mode)]
        for tag, R in (('all R', 0.31), ('R<=0.5', None)):
            h = score.h4({st: [scored(r, R) for r in mine]})['per_set'][st]
            n, u, o, rtl = [z['h4'][lab][i] for i in ((0, 1, 2, 3) if tag == 'all R' else (4, 5, 6, 7))]
            chk('%s H4 %s n' % (lab, tag), int(n), h['n'])
            chk('%s H4 %s usable%%' % (lab, tag), float(u), round(100.0 * h['usable_share'], 1), 0.051)
            chk('%s H4 %s ok%%' % (lab, tag), float(o), round(100.0 * h['ok_share'], 1), 0.051)
            chk('%s H4 %s rtl' % (lab, tag), int(rtl), h['n_receiver_too_large'])
    return res


def main(argv=None):
    import argparse
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('out')
    ap.add_argument('--zsweep', default=str(ZSWEEP))
    ap.add_argument('--run1', default=str(RUN1))
    a = ap.parse_args(argv)
    out = Path(a.out)
    z = parse_z25(a.zsweep)
    inputs = load_inputs(a.run1)
    print('%d inputs from %s; scoring with frozen2 (%s)' % (len(inputs), a.run1, corpus.FROZEN2_SHA256[:12]), flush=True)
    summary = score.evaluate(inputs, out_dir=out)
    res = compare(summary, out / 'rows.jsonl', z)
    for what, want, got, ok in res:
        print('[%s] %-34s v21_zsweep Z=2.5: %-28r scorer: %r' % ('OK ' if ok else 'MISMATCH', what, want, got))
    bad = [r for r in res if not r[3]]
    print('=== %d numbers compared, %d mismatches ===' % (len(res), len(bad)))
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(main())
