"""T18-T21 (HARNESS-PLAN.md section 4): definitions, exclusions, H3-H6 and the void on a changed method."""
import json
import math

from conftest import FROZEN, FROZEN_SHA256

from m8b import attack, score
from m8b.corpus import VoidRun


def row(status='ok', edt=1.0, lo=None, hi=None, truth=1.0, ts='ok', reason='', **k):
    lo = edt if lo is None and edt is not None else lo
    hi = edt if hi is None and edt is not None else hi
    r = dict(set='spps', id='r', status=status, edt=edt, edt_lo=lo, edt_hi=hi, reason=reason,
             truth=truth, truth_status=ts, room='F1', d_m=1.5, step_ms=1.0, band_hz=500, particles=150000,
             seed=1, family=None, design_t60_s=1.0)
    r.update(k)
    return r


def test_t18_definitions_on_hand_made_rows():
    """T18: ok, usable, wrong-silent (strictly above 5 %, judged exactly) and covered (P28's 1e-9
    tolerance), on hand-made rows, the edges included."""
    c = score.classify(row(edt=21.0, lo=20.0, hi=22.0, truth=20.0))
    assert c is not None, 'score.classify returned nothing'
    assert c['ok'] is True and c['usable'] is True and c['has_truth'] is True and c['covered'] is True
    assert 21.0 / 20.0 - 1 > 0.05                          # float division rounds it up; the definition is exact
    assert c['wrong_silent'] is False, '|err| exactly 0.05 is not wrong-silent'
    assert score.classify(row(edt=math.nextafter(21.0, math.inf), truth=20.0))['wrong_silent'] is True
    assert score.classify(row(edt=19.0, truth=20.0))['wrong_silent'] is False
    assert score.classify(row(edt=math.nextafter(19.0, -math.inf), truth=20.0))['wrong_silent'] is True

    lo, hi = 0.9, 1.1
    assert score.classify(row(edt=1.0, lo=lo, hi=hi, truth=hi * (1 + 1e-9)))['covered'] is True
    assert score.classify(row(edt=1.0, lo=lo, hi=hi, truth=hi * (1 + 3e-9)))['covered'] is False
    assert score.classify(row(edt=1.0, lo=lo, hi=hi, truth=lo * (1 - 1e-9)))['covered'] is True
    assert score.classify(row(edt=1.0, lo=lo, hi=hi, truth=lo * (1 - 3e-9)))['covered'] is False

    w = score.classify(row(status='wide', edt=1.0, lo=0.8, hi=1.25, truth=1.2))
    assert (w['ok'], w['usable'], w['wrong_silent'], w['covered']) == (False, True, False, True)
    w2 = score.classify(row(status='wide', edt=1.0, lo=0.95, hi=1.05, truth=1.2))
    assert (w2['wrong_silent'], w2['covered']) == (False, False)
    f = score.classify(row(status='refused', edt=None, lo=None, hi=None, truth=1.0, reason='run_too_short'))
    assert (f['ok'], f['usable'], f['wrong_silent'], f['covered']) == (False, False, False, None)
    up = score.classify(row(edt=1.3, lo=1.3, hi=1.3, truth=1.3))         # upstream's point range
    assert up['covered'] is True and up['wrong_silent'] is False
    for ts, truth in (('truth_nan', None), ('truth_uncertain', 1.0), ('truth_truncated', 1.0)):
        x = score.classify(row(edt=2.0, lo=1.9, hi=2.1, truth=truth, ts=ts))
        assert x['ok'] is True and x['has_truth'] is False and x['wrong_silent'] is None and x['covered'] is None, ts


ROWS = [
    row(edt=1.0, lo=0.98, hi=1.02, truth=1.0),                                   # ok, good, covered
    row(edt=1.02, lo=1.0, hi=1.04, truth=1.0),                                   # ok, good, covered at lo
    row(edt=0.97, lo=0.95, hi=0.99, truth=1.0),                                  # ok, good, not covered
    row(edt=1.2, lo=1.15, hi=1.25, truth=1.0),                                   # ok, wrong-silent
    row(edt=3.0, lo=2.9, hi=3.1, truth=1.0, ts='truth_uncertain'),               # excluded (P19)
    row(edt=3.0, lo=2.9, hi=3.1, truth=1.0, ts='truth_truncated'),               # excluded (P18)
    row(status='wide', edt=1.0, lo=0.7, hi=1.4, truth=None, ts='truth_nan'),     # excluded
    row(status='wide', edt=1.0, lo=0.8, hi=1.25, truth=1.1),                     # wide, covered
    row(status='refused', edt=None, truth=1.0, reason='run_too_short'),
    row(status='refused', edt=None, truth=1.0, ts='truth_truncated', reason='step_too_coarse'),
]


def good_rows(n, set_='synth'):
    return [row(set=set_, id='%s%d' % (set_, i), edt=1.0, lo=0.99, hi=1.01, truth=1.0) for i in range(n)]


def test_t19_exclusions_and_empty_denominators():
    """T19: rows excluded for their truth leave every truth-based count and are counted by reason; they
    stay in the ok and usable shares; a ratio whose denominator is 0 fails its criterion (P27)."""
    t = score.tally(ROWS)
    assert t is not None, 'score.tally returned nothing'
    assert (t['n'], t['n_ok'], t['n_usable']) == (10, 6, 8)
    assert t['ok_share'] == 0.6 and t['usable_share'] == 0.8
    assert t['excluded'] == {'truth_uncertain': 1, 'truth_truncated': 2, 'truth_nan': 1}
    assert t['refused'] == {'run_too_short': 1, 'step_too_coarse': 1}
    assert (t['n_ok_truth'], t['n_wrong_silent'], t['wrong_silent_share']) == (4, 1, 0.25)
    assert (t['n_usable_truth'], t['n_covered'], t['coverage']) == (5, 3, 0.6)

    only_excluded = ROWS[4:7]
    t0 = score.tally(only_excluded)
    assert t0['n_ok_truth'] == 0 and t0['wrong_silent_share'] is None
    assert t0['n_usable_truth'] == 0 and t0['coverage'] is None
    assert t0['usable_share'] == 1.0 and abs(t0['ok_share'] - 2 / 3) < 1e-12

    assert score.h1({'ism': good_rows(200, 'ism'), 'synth': good_rows(200)})['pass'] is True
    h = score.h1({'ism': [dict(r, set='ism') for r in only_excluded], 'synth': good_rows(200)})
    assert h['pass'] is False and h['per_set']['ism']['pass'] is False and h['per_set']['synth']['pass'] is True
    assert score.h1({'ism': [], 'synth': good_rows(200)})['pass'] is False
    assert score.h2(good_rows(100, 'spps'))['pass'] is True
    assert score.h2(only_excluded)['pass'] is False


def sub(n, wrong, *, set_='spps', room='F1', d=1.5, step=1.0, family=None, no_truth=0, tag=''):
    """n ok rows of one subgroup, `wrong` of them wrong-silent, `no_truth` of them excluded for their truth;
    spread over two bands, ten seeds and two particle counts, which H3 pools."""
    out = []
    for i in range(n):
        bad = i < wrong
        excl = n - i <= no_truth
        out.append(row(set=set_, id='%s%s%d' % (tag, room, i), room=room, d_m=d, step_ms=step, family=family,
                       band_hz=(500, 1000)[i % 2], seed=1 + i % 10, particles=(150000, 50000)[i % 2],
                       edt=1.2 if bad else 1.0, lo=(1.15 if bad else 0.99), hi=(1.25 if bad else 1.01),
                       truth=1.0, ts='truth_uncertain' if excl else 'ok'))
    return out


def test_t20_h3_h4_h5_h6():
    """T20: H3's subgroups and its 20-row rule; H4's filter, per set; H5 strict, 0 against 0 failing;
    H6 failing on a reproducible class unless 3 of 3 judges call it implausible."""
    A = sub(20, 3, room='F1', d=1.5, step=1.0, tag='A')               # judged, 15 %: fails
    Bm = sub(19, 10, room='F2', d=5.0, step=1.0, tag='B')             # 19 ok rows: not judged
    Cf = sub(20, 5, room='F2', d=12.0, step=2.0, no_truth=1, tag='C') # 19 with a truth: not judged
    D = sub(20, 2, room='F3', d=2.0, step=5.0, tag='D')               # judged, exactly 10 %: passes
    h = score.h3(A + Bm + Cf + D)
    assert h is not None, 'score.h3 returned nothing'
    assert h['pass'] is False
    g = h['subgroups']
    assert g[('spps', 'F1', 'near', 1.0)]['judged'] is True and g[('spps', 'F1', 'near', 1.0)]['pass'] is False
    assert g[('spps', 'F1', 'near', 1.0)]['n_ok_truth'] == 20 and g[('spps', 'F1', 'near', 1.0)]['n_wrong_silent'] == 3
    assert g[('spps', 'F2', 'mid', 1.0)]['judged'] is False
    assert g[('spps', 'F2', 'far', 2.0)]['judged'] is False and g[('spps', 'F2', 'far', 2.0)]['n_ok_truth'] == 19
    assert g[('spps', 'F3', 'mid', 5.0)]['judged'] is True and g[('spps', 'F3', 'mid', 5.0)]['pass'] is True
    assert score.h3(Bm + Cf + D)['pass'] is True
    # distance classes on the straight line: 2.0 m is mid, 10.0 m is mid
    assert score.h3(sub(10, 0, room='F5', d=1.999, tag='n') + sub(20, 3, room='F5', d=2.0, tag='m'))['pass'] is False
    assert score.h3(sub(10, 0, room='F5', d=10.0, tag='m') + sub(20, 3, room='F5', d=10.001, tag='f'))['pass'] is False
    assert score.h3(sub(10, 0, room='F5', d=1.999, tag='n') + sub(10, 3, room='F5', d=2.0, tag='m'))['pass'] is True
    # Synth-fresh: family x step; ISM-fresh: room x class x step
    s = score.h3(sub(20, 3, set_='synth', room=None, d=None, family=1.5, tag='s') +
                 sub(20, 2, set_='synth', room=None, d=None, family=5.0, tag='t'))
    assert s['pass'] is False and s['subgroups'][('synth', 1.5, 1.0)]['pass'] is False
    assert s['subgroups'][('synth', 5.0, 1.0)]['pass'] is True
    assert score.h3(sub(20, 3, set_='ism', room='I03', d=0.8, step=2.0, tag='i'))['pass'] is False

    def h4rows(set_, n_usable, n_refused):
        out = [row(set=set_, status='ok' if i % 3 else 'wide', edt=1.0, lo=0.9, hi=1.1, design_t60_s=3.0 if i == 0 else 1.0,
                   ts='truth_uncertain' if i == 1 else 'ok') for i in range(n_usable)]
        out += [row(set=set_, status='refused', edt=None, reason='run_too_short') for _ in range(n_refused)]
        out += [row(set=set_, status='refused', edt=None, step_ms=2.0) for _ in range(5)]          # not at 1 ms
        out += [row(set=set_, status='refused', edt=None, design_t60_s=3.01) for _ in range(5)]    # T60 above 3 s
        return out
    h4 = score.h4({'spps': h4rows('spps', 9, 1), 'ism': h4rows('ism', 8, 2)})
    assert h4 is not None and h4['pass'] is False
    assert h4['per_set']['spps']['pass'] is True and h4['per_set']['ism']['pass'] is False
    assert abs(h4['per_set']['spps']['usable_share'] - 0.9) < 1e-12
    assert abs(h4['per_set']['spps']['ok_share'] - 0.6) < 1e-12
    assert score.h4({'spps': h4rows('spps', 9, 1), 'ism': h4rows('ism', 9, 1)})['pass'] is True
    assert score.h4({'spps': h4rows('spps', 9, 1), 'ism': []})['pass'] is False

    def h5rows(set_, n, wrong, extra_excluded_wrong=0):
        rs = [row(set=set_, id='%s%d' % (set_, i), edt=1.2 if i < wrong else 1.0, lo=1.19 if i < wrong else 0.99,
                  hi=1.21 if i < wrong else 1.01, truth=1.0) for i in range(n)]
        rs += [row(set=set_, id='%sx%d' % (set_, i), edt=1.3, lo=1.29, hi=1.31, truth=1.0, ts='truth_uncertain')
               for i in range(extra_excluded_wrong)]
        return rs
    sets = ('spps', 'ism', 'synth', 'attack')
    frozen = {s: h5rows(s, 30, w) for s, w in zip(sets, (1, 0, 0, 2))}
    up = {s: h5rows(s, 30, w) for s, w in zip(sets, (3, 2, 1, 5))}
    h5 = score.h5(frozen, up)
    assert h5 is not None and h5['pass'] is True
    assert score.h5(dict(frozen, synth=h5rows('synth', 30, 0)), dict(up, synth=h5rows('synth', 30, 0)))['pass'] is False
    assert score.h5(dict(frozen, spps=h5rows('spps', 30, 3)), up)['pass'] is False
    assert score.h5({s: frozen[s] for s in sets[:3]}, {s: up[s] for s in sets[:3]})['pass'] is False
    # the same eligible rows: an upstream wrong row whose truth is excluded does not count
    assert score.h5(dict(frozen, ism=h5rows('ism', 30, 1)), dict(up, ism=h5rows('ism', 30, 1, extra_excluded_wrong=3)))['pass'] is False

    def vote(v):
        return json.dumps({'verdict': v, 'reasons': 'because'})
    assert attack.parse_vote(vote('plausible')) == 'plausible' and attack.parse_vote(vote('implausible')) == 'implausible'
    for raw in (None, 'implausible?', vote('unsure'), '{"reasons": "x"}', '[1, 2]'):
        assert attack.parse_vote(raw) is None, raw
    assert attack.panel([vote('implausible')] * 3) == 'implausible'
    implausible3 = dict(id='a', n_draws=20, n_wrong_silent=5, votes=[vote('implausible')] * 3)
    not_reproducible = dict(id='f', n_draws=20, n_wrong_silent=4, votes=[vote('plausible')] * 3)
    h6 = score.h6([implausible3, not_reproducible])
    assert h6 is not None and h6['pass'] is True
    for votes in ([vote('implausible'), vote('implausible'), vote('plausible')],      # split
                  [vote('implausible'), vote('implausible'), None],                    # missing
                  [vote('implausible'), vote('implausible'), 'implausible?'],          # does not parse
                  [vote('implausible'), vote('implausible')]):                         # only two
        assert attack.panel(votes) == 'plausible', votes
        bad = dict(id='b', n_draws=20, n_wrong_silent=7, votes=votes)
        assert score.h6([implausible3, not_reproducible, bad])['pass'] is False, votes


def test_t21_scorer_voids_the_evaluation_when_the_method_hash_differs(tmp_path, synth):
    """T21: the scorer checks the method file's hash before it computes or writes anything; a mismatch
    voids the evaluation (PREREG.md:10). The frozen file's run records the hash it checked."""
    data = FROZEN.read_bytes()
    i = data.index(b'Z = 2.0') + len(b'Z = ')
    bad = tmp_path / 'method.py'
    bad.write_bytes(data[:i] + b'3' + data[i + 1:])
    k = 6 * math.log(10) / 0.8
    h = 0.31 / synth.C
    Ed = (1 / k) * 10 ** (-0.3)
    bins = synth.histogram(1e-3, 2.0, 0.02, h, Ed, 0.002, [1.0], [k])
    inputs = [dict(set='synth', id='t21', bins=bins, dt=1e-3, t_arrival=0.02, meta={'half_width': h},
                   truth=synth.truth_edt(0.02, Ed, 0.002, [1.0], [k]), truth_status='ok', family=1.5, step_ms=1.0)]
    out = tmp_path / 'void'
    try:
        score.evaluate(inputs, method_path=bad, out_dir=out)
    except VoidRun:
        pass
    else:
        raise AssertionError('the evaluation ran with a method file whose hash differs')
    assert not out.exists() or not any(out.iterdir()), 'a void evaluation wrote something'
    ok = tmp_path / 'ok'
    s = score.evaluate(inputs, method_path=FROZEN, out_dir=ok)
    assert s['method']['sha256'] == FROZEN_SHA256 and s['method']['verified'] is True
    for name in ('REPORT.md', 'summary.json', 'rows.jsonl'):
        assert (ok / name).is_file(), name
    assert json.loads((ok / 'summary.json').read_text(encoding='utf-8'))['method']['sha256'] == FROZEN_SHA256
