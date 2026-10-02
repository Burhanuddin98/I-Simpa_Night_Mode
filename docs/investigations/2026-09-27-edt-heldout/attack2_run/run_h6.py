"""H6, round 2: turn each attack draw into a scored input row, run the frozen method, count wrong-silent, score H6.

Built after ADDENDUM-B1 and after H1-H5 were scored, before any attack class was run (ADDENDUM-B2.md). It imports
harness2/ read-only and changes nothing under harness2/, frozen2/, harness/ or frozen/.

What it adds (the gap: nothing in harness2 turned an attack draw into an input row):
- draw_to_row(cls, inst, index, class_id=None): the input-row dict Synth-fresh-2 builds (score_heldout.build_synth),
  with set 'attack', id, class_id and R_m. The spec comes from synth_fresh.make_spec, so the delay is rounded up to
  the next whole step (ceil(float32(delay)/float32(dt))) and t_arrival = emission + d/c is the product's arrival_s;
  the bins are synth_fresh.histogram(spec) and the truth is synth_fresh.truth(spec). Run length is
  run_over_t60 x T60 past the arrival (make_spec: run_s = t_arrival + f * t60), step is step_ms.
- Parameters a class does not name take single-slope defaults (stated, not hidden): ratio 1.0, late_share_db -300
  (a second slope of weight 1e-30), gap_ms 0, delay_ms 0. A class that names a parameter synth_fresh has no
  name for is refused with ValueError.
- Noise 'compound_poisson': the corpus's generator (synth.noisy, via synth_fresh.generator(), the hash-gated
  critique file) at the drawn particles_per_source N, with the corpus's weights (corpus.scan3_noise): the direct
  sound carries hits of energy w_dir = Ed / lam_d, lam_d = N R^2 / (4 d^2); the reverberant part w_rev = 1 / lam0,
  lam0 = N pi R^2 c / V, V from the Sabine direct-to-reverberant relation the corpus uses (synth.sabine_direct_energy):
  Ed / S_rev = 0.161 V / T60 / (16 pi d^2) = 10^(drr/10), so V = 10^(drr/10) 16 pi d^2 T60 / 0.161.
  Direct and reverberant parts are drawn separately (noisy(direct) + noisy(reverb)), as the corpus does.
  Seed: np.random.SeedSequence([int(class_sha256, 16), ATTACK_SEED, draw_index, NOISE_TAG]), NOISE_TAG = 0x6e6f697365
  ('noise'), so a draw's noise is a function of the class, the attack seed and the draw index only, and differs
  from the draw's parameters' stream (attack.draws seeds with [class_sha256, ATTACK_SEED]).
- An 'ism' class raises NotImplementedError (no ism class exists among c01-c11).
- main(): c01..c11 -> 20 draws each -> rows -> frozen2 analyse (method.load, hash pin) -> wrong-silent count
  (score.classify: status 'ok' and |edt/truth - 1| > 0.05) -> votes from the judge files -> score.h6 -> files
  under B:\\data\\m8b-edt\\round2\\results\\attack\\ (never overwritten).
"""
import csv
import datetime
import json
import math
import re
import sys
from pathlib import Path

sys.dont_write_bytecode = True
import numpy as np

HERE = Path(__file__).resolve().parent
D = HERE.parent
sys.path.insert(0, str(D / 'harness2'))

from m8b import attack, freeze2, method, round2, score, synth_fresh      # noqa: E402

C = synth_fresh.C
NOISE_TAG = 0x6e6f697365                    # b'noise'
DEFAULTS = {'ratio': 1.0, 'late_share_db': -300.0, 'gap_ms': 0.0, 'delay_ms': 0.0}
SPEC_PARAMS = ('t60_s', 'drr_db', 'd_m', 'ratio', 'late_share_db', 'gap_ms', 'delay_ms')
REQUIRED = ('t60_s', 'drr_db', 'd_m')
OUT_DIR = round2.RESULTS_ROOT / 'attack'
CLASSES_DIR = D / 'attack2' / 'classes'
JUDGE_FILES = [D / 'attack2' / ('judge%d.txt' % i) for i in (1, 2, 3)]
B1 = D / 'ADDENDUM-B1.md'
LINE = re.compile(r'^(c\d+)\.json: (.*)$')


def class_id_of(cls):
    return attack.class_sha256(cls)[:12]


def spec_of(cls, inst):
    if cls.get('generator') != 'synth':
        raise NotImplementedError("generator %r: only 'synth' classes are built here; no ISM class exists among "
                                  "c01-c11 (no ism class here), so the ISM row builder is a known gap" % cls.get('generator'))
    unknown = sorted(set(cls['params']) - set(SPEC_PARAMS))
    if unknown:
        raise ValueError('class params %s have no name in synth_fresh' % unknown)
    p = dict(DEFAULTS)
    p.update({k: inst[k] for k in SPEC_PARAMS if k in inst})
    missing = [k for k in REQUIRED if k not in p]
    if missing:
        raise ValueError('class does not give %s' % missing)
    return synth_fresh.make_spec(id='attack-spec', ratio=p['ratio'], step_ms=inst['step_ms'], t60_s=p['t60_s'],
                                 late_share_db=p['late_share_db'], drr_db=p['drr_db'], R_m=inst['R_m'], d_m=p['d_m'],
                                 gap_ms=p['gap_ms'], delay_ms=p['delay_ms'], run_over_t60=inst['run_over_t60'])


def split_histogram(spec):
    """(direct part, reverberant part) of the spec's histogram, each placed after the emission as synth_fresh does."""
    return (synth_fresh.histogram(dict(spec, A=[0.0] * len(spec['A']))), synth_fresh.histogram(dict(spec, Ed=0.0)))


def noise_weights(spec, n_particles):
    """(w_dir, w_rev) of the corpus's compound-Poisson model at n_particles per source (module docstring)."""
    R, d = spec['R_m'], spec['d_m']
    V = 10 ** (spec['drr_db'] / 10) * 16 * math.pi * d * d * spec['t60_s'] / 0.161
    lam_d = n_particles * R * R / (4 * d * d)
    lam0 = n_particles * math.pi * R * R * C / V
    return spec['Ed'] / lam_d, 1.0 / lam0


def noise_rng(cls, index):
    return np.random.default_rng(np.random.SeedSequence([int(attack.class_sha256(cls), 16), attack.ATTACK_SEED,
                                                         int(index), NOISE_TAG]))


def draw_to_row(cls, inst, index, class_id=None):
    spec = spec_of(cls, inst)
    noise = cls.get('noise') or {'kind': 'none'}
    if noise.get('kind') == 'compound_poisson':
        S = synth_fresh.generator()
        w_dir, w_rev = noise_weights(spec, inst['particles_per_source'])
        d_part, r_part = split_histogram(spec)
        rng = noise_rng(cls, index)
        bins = S.noisy(d_part, w_dir, rng) + S.noisy(r_part, w_rev, rng)
    elif noise.get('kind') == 'none':
        bins = synth_fresh.histogram(spec)
    else:
        raise ValueError('noise kind %r' % noise.get('kind'))
    cid = class_id or class_id_of(cls)
    truth = synth_fresh.truth(spec)
    return dict(set='attack', id='attack|%s|%02d' % (cid, index), class_id=cid, bins=bins, dt=spec['dt'],
                t_arrival=spec['t_arrival'], meta=dict(half_width=spec['half_width']), truth=truth,
                truth_status='ok' if math.isfinite(truth) else 'truth_nan', step_ms=spec['step_ms'],
                R_m=spec['R_m'], d_m=spec['d_m'], emission_steps=spec['emission_steps'])


# ---- votes and classes -------------------------------------------------------------------------------------
def parse_vote_line(line):
    """'cNN.json: {json}' -> ('cNN', '{json}'); None for a blank line; ValueError for any other shape."""
    line = line.strip()
    if not line:
        return None
    m = LINE.match(line)
    if not m:
        raise ValueError('a vote line is "cNN.json: {json}", not %r' % line[:60])
    return m.group(1), m.group(2)


def read_votes(paths):
    """{class id: [raw vote of judge 1, 2, 3]} in the order of the files; the raw text goes to attack.parse_vote."""
    votes = {}
    for p in paths:
        for line in Path(p).read_text(encoding='utf-8').splitlines():
            got = parse_vote_line(line)
            if got:
                votes.setdefault(got[0], []).append(got[1])
    return votes


def load_classes(folder):
    return {p.stem: json.loads(p.read_text(encoding='utf-8')) for p in sorted(Path(folder).glob('c*.json'))}


def run_score_h6(rows):
    return score.h6([dict(r, n_draws=r.get('n_draws', attack.DRAWS)) for r in rows])


# ---- the run ------------------------------------------------------------------------------------------------
def verify_frozen():
    """harness2's own checks, before anything is built: the method's sha256 pin (method.load, VoidRun on a mismatch)
    and the B1 hash table (freeze2.check: [] when every hashed file is as B1 recorded it)."""
    m = method.load()
    differs = freeze2.check(B1)
    if differs:
        raise RuntimeError('B1 hash table differs: %s' % differs)
    return m


def analyse_class(m, cid, cls):
    out = []
    for i, inst in enumerate(attack.draws(cls)):
        row = draw_to_row(cls, inst, i, class_id=cid)
        res = m.analyse(row['bins'], float(row['dt']), row['t_arrival'], dict(row['meta']))
        shown = dict(status=res['status'], edt=res['edt'], edt_lo=res['edt_lo'], edt_hi=res['edt_hi'],
                     reason=res['reason'], truth=row['truth'], truth_status=row['truth_status'], id=row['id'])
        c = score.classify(dict(shown, set='attack'))
        out.append(dict(shown, class_id=cid, draw=i, n_bins=len(row['bins']), t_arrival=row['t_arrival'],
                        R_m=row['R_m'], d_m=row['d_m'], step_ms=row['step_ms'],
                        wrong_silent=bool(c['wrong_silent'])))
    return out


def report_md(res, per_class, classes):
    L = ['# H6 attack result, round 2', '']
    bad = [c['id'] for c in res['classes'] if not c['pass']]
    L.append('**H6 %s.** %d of %d classes are reproducible (5 or more of 20 draws wrong-silent); %d of them are judged '
             'physically plausible, and those fail H6.%s' % ('PASSES' if res['pass'] else 'FAILS', res['n_reproducible'],
                                                             res['n_classes'], res['n_failing'],
                                                             (' Failing: %s.' % ', '.join(bad)) if bad else ''))
    L += ['', 'Wrong-silent: status ok and the EDT more than 5 % from the truth. A refusal is never wrong-silent. '
          'A draw whose truth is NaN is not counted (truth_nan).', '',
          '| class | wrong-silent / 20 | reproducible | votes | panel | pass |', '|---|---|---|---|---|---|']
    for c in res['classes']:
        L.append('| %s | %d/20 | %s | %s | %s | %s |' % (c['id'], c['n_wrong_silent'], 'yes' if c['reproducible'] else 'no',
                                                         '/'.join(str(v) for v in c['votes']), c['panel'],
                                                         'pass' if c['pass'] else 'FAIL'))
    L += ['', '## Per class', '']
    for c in res['classes']:
        rs = per_class[c['id']]
        st = {}
        for r in rs:
            st[r['status']] = st.get(r['status'], 0) + 1
        L.append('- **%s**: %d/20 wrong-silent, reproducible %s, statuses %s, truth_nan %d. Votes %s, agreement %s, panel %s. %s'
                 % (c['id'], c['n_wrong_silent'], 'yes' if c['reproducible'] else 'no', st,
                    sum(r['truth_status'] != 'ok' for r in rs), c['votes'], c['agreement'], c['panel'],
                    'PASS' if c['pass'] else 'FAIL'))
        for k, why in enumerate(c['reasons']):
            L.append('  - judge %d: %s' % (k + 1, why))
    L.append('')
    return '\n'.join(L)


def main(argv=None):
    import argparse
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--out', default=str(OUT_DIR))
    a = ap.parse_args(argv)
    out = Path(a.out)
    if out.exists():
        raise FileExistsError('%s exists; this run never overwrites a result folder' % out)
    m = verify_frozen()
    classes = load_classes(CLASSES_DIR)
    votes = read_votes(JUDGE_FILES)
    per_class, tally = {}, []
    for cid, cls in classes.items():
        ok, why = attack.validate_class(cls)
        if not ok:
            raise ValueError('%s: %s' % (cid, why))
        per_class[cid] = analyse_class(m, cid, cls)
        tally.append(dict(id=cid, n_draws=attack.DRAWS, n_wrong_silent=sum(r['wrong_silent'] for r in per_class[cid]),
                          votes=votes.get(cid, [])))
    res = score.h6(tally)
    out.mkdir(parents=True, exist_ok=False)
    cols = ['class_id', 'draw', 'id', 'status', 'reason', 'edt', 'edt_lo', 'edt_hi', 'truth', 'truth_status',
            'wrong_silent', 'n_bins', 'step_ms', 'R_m', 'd_m', 't_arrival']
    with open(out / 'rows.csv', 'w', newline='', encoding='utf-8') as f:
        w = csv.DictWriter(f, cols, extrasaction='ignore')
        w.writeheader()
        for cid in per_class:
            w.writerows(per_class[cid])
    res['method_sha256'] = m.checked_sha256
    res['generated'] = datetime.datetime.now().astimezone().isoformat(timespec='seconds')
    (out / 'h6.json').write_text(json.dumps(res, indent=1, allow_nan=False) + '\n', encoding='utf-8', newline='\n')
    (out / 'H6.md').write_text(report_md(res, per_class, classes), encoding='utf-8', newline='\n')
    print('H6', 'PASS' if res['pass'] else 'FAIL', out)
    return 0 if res['pass'] else 1


if __name__ == '__main__':
    sys.exit(main())
