"""T32 and T33 (../../HARNESS-PLAN-2.md sections 2 and 6, with section 9 M1-M3): the widened corpus list and the
seven SPPS-fresh-2 rooms.

References are the tests' own: P3 and P5 as conftest types them, the union-of-boxes geometry in conftest.Union,
the plan's tables typed below (G2's absorption is section 9 M1's only permitted edit: 0.10, design T60 at 1 kHz
2.71 s, inside [2.6, 2.9]). The run-1 record the list must equal is `B:\\data\\m8b-edt\\results\\run1\\counts.json`.
"""
import collections
import json
import math
from pathlib import Path

import pytest
from conftest import HARNESS, Union, eyring, sha256_file

from m8b import corpus2, driver, rooms, rooms2

RUN1_CORPUS_SHA256 = 'dc3d0b2c0b8be0f69c7927f0c749a88dad76cc791edae2474f09cb1fd44da882'      # HARNESS-PLAN-2.md section 2
COUNTS_JSON = Path(r'B:\data\m8b-edt\results\run1\counts.json')
G = ('G1', 'G2', 'G3', 'G4', 'G5', 'G6', 'G7')


def test_t32_corpus_rooms_2_is_the_widened_list_and_rebuilds_byte_for_byte():
    assert sha256_file(HARNESS / 'corpus_rooms.json') == RUN1_CORPUS_SHA256
    assert (HARNESS / 'corpus_rooms.json').read_bytes() == (HARNESS.parent / 'harness' / 'corpus_rooms.json').read_bytes()
    path = HARNESS / 'corpus_rooms_2.json'
    c2 = json.loads(path.read_text(encoding='utf-8'))
    c1 = json.loads((HARNESS / 'corpus_rooms.json').read_text(encoding='utf-8'))
    R = c2['rooms']
    assert len(R) == 147
    assert collections.Counter(r['kind'] for r in R) == {'box': 143, 'non-box': 4}
    assert collections.Counter(r['set'] for r in R) == {'z3': 105, 'noise_cal': 6, 'ism': 4, 'm8a': 4, 'run1_spps': 7,
                                                        'run1_probe': 2, 'run1_ism': 12, 'round2_spps': 7}
    assert [r['id'] for r in R[:119]] == [r['id'] for r in c1['rooms']], 'round 1\'s 119 rooms come first, unchanged'
    assert R[:119] == c1['rooms']
    assert [r['set'] for r in R[-7:]] == ['round2_spps'] * 7, 'the G rooms are added last'
    by = lambda s: [r for r in R if r['set'] == s]
    assert [r['id'] for r in by('run1_spps')] == ['run1_spps:F%d' % i for i in range(1, 8)]
    assert [r['id'] for r in by('run1_probe')] == ['run1_probe:P0', 'run1_probe:P0b']
    assert [r['id'] for r in by('round2_spps')] == ['round2_spps:%s' % g for g in G]
    kinds = {r['id'].split(':')[1]: r['kind'] for r in by('run1_spps') + by('round2_spps')}
    assert {k for k, v in kinds.items() if v == 'non-box'} == {'F3', 'F4', 'G3', 'G4'}
    F = rooms.rooms()
    for r in by('run1_spps') + by('run1_probe'):
        f = F[r['id'].split(':')[1]]
        assert r['kind'] == f['kind'] and r['volume_m3'] == f['volume_m3'] and r['bbox_m'] == f['bbox_m']
    Gm = rooms2.rooms()
    for r in by('round2_spps'):
        g = Gm[r['id'].split(':')[1]]
        assert r['kind'] == g['kind'] and r['volume_m3'] == g['volume_m3'] and r['surface_m2'] == g['surface_m2']
    # the 12 run-1 ISM rooms are the ones run 1 scored (counts.json), draw for draw
    counts = json.loads(COUNTS_JSON.read_text(encoding='utf-8'))
    assert [r['id'] for r in by('run1_ism')] == ['run1_ism:' + x['id'] for x in counts['ism_rooms']]
    for r, x in zip(by('run1_ism'), counts['ism_rooms']):
        assert r['dims_m'] == x['dims_m'] and r['alpha_walls'] == x['alpha_walls'] and r['design_t60_s'] == x['design_t60_s']
        assert r['kind'] == 'box' and r['dims_sorted_m'] == sorted(x['dims_m'], reverse=True)
    # the seeds and the lists round 1 made, as lists
    su = c2['seeds_used']
    assert su['spps_tested'] == [1101, 1102, 1103, 1201, 1202, 1203, 1501, 1502, 1503, 2101, 2102, 2103, 2201, 2202, 2203,
                                 2501, 2502, 2503]
    assert su['spps_truth'] == [9001, 9002, 9003, 9004] and su['probes'] == [9998, 9999]
    assert su['ism_fresh'] == 2026100102 and su['synth_fresh'] == 2026100101
    assert len(c2['synth_specs_sha256']) == 64 and c2['synth_specs_n'] == 4000
    assert c2['corpus_rooms_sha256'] == RUN1_CORPUS_SHA256
    assert c2['run1_ism_rejections'] == counts['ism_rejections']
    # a rebuild is byte-identical (no clock, no path in the text)
    assert corpus2.build_text().encode('utf-8') == path.read_bytes()


# ---- T33 --------------------------------------------------------------------------------------------------------
PLAN_T60 = {          # HARNESS-PLAN-2.md section 2, 125 / 500 / 1 k / 4 k Hz; G2 by section 9 M1; G3: the late slope
    'G1': (0.33, 0.33, 0.33, 0.31), 'G2': (2.90, 2.79, 2.71, 1.95), 'G3': (1.90, 1.86, 1.82, 1.44),
    'G4': (0.64, 0.64, 0.63, 0.58), 'G5': (0.75, 0.74, 0.73, 0.66), 'G6': (0.32, 0.32, 0.32, 0.30),
    'G7': (0.18, 0.18, 0.179, 0.175)}
PLAN_V = {'G1': 23.1, 'G2': 2352.0, 'G3': 368.2, 'G4': 264.0, 'G5': 582.8, 'G6': 520.0, 'G7': 168.8}
BLOCKED = {'G3': ['R007'], 'G4': ['R007']}
P17 = {'G1': 3.0, 'G2': 5.9, 'G3': 3.9, 'G4': 3.0, 'G5': 3.0, 'G6': 3.0, 'G7': 3.0}      # P17 with 125 Hz T60 5.9 s for G2
SPEC = {              # the tests' own typing of the plan's geometry: boxes, then (alpha) by box
    'G1': ([((0, 0, 0), (4.2, 2.5, 2.2))], [0.20]),
    'G2': ([((0, 0, 0), (28, 12, 7))], [0.10]),
    'G3': ([((0, 0, 0), (7.5, 6, 3.5)), ((7.5, 2.4, 0), (7.7, 3.9, 2.2)), ((7.7, 0, 0), (17.7, 6, 3.5))], [0.35, 0.06, 0.06]),
    'G4': ([((0, 0, 0), (14, 4, 3.0)), ((10, 4, 0), (14, 12, 3.0))], [0.18, 0.18]),
    'G5': ([((0, 0, 0), (18.5, 10.5, 3.0))], [0.20]),
    'G6': ([((0, 0, 0), (32, 6.5, 2.5))], [0.35]),
    'G7': ([((0, 0, 0), (9.0, 7.5, 2.5))], [0.50]),
}


def gap(a, b):
    """P3's largest relative difference, typed here; None across kinds."""
    if a['kind'] != b['kind']:
        return None
    sd = lambda r: [float(x) for x in r['dims_sorted_m']] if 'dims_sorted_m' in r else sorted(
        (float(q) - float(p) for p, q in zip(*r['bbox_m'])), reverse=True)
    g = [abs(x / y - 1) for x, y in zip(sd(a), sd(b))]
    if a['kind'] != 'box':
        g += [abs(a['volume_m3'] / b['volume_m3'] - 1), abs(a['surface_m2'] / b['surface_m2'] - 1)]
    return max(g)


def test_t33_g_rooms_meet_the_plan():
    Gm = rooms2.rooms()
    assert tuple(Gm) == G and rooms2.NAMES == G
    assert rooms2.check_rooms() is not None
    list2 = json.loads((HARNESS / 'corpus_rooms_2.json').read_text(encoding='utf-8'))['rooms']
    for name, room in Gm.items():
        boxes, alphas = SPEC[name]
        U = Union(boxes)
        assert abs(room['volume_m3'] - U.volume) < 1e-9 and abs(room['surface_m2'] - U.surface) < 1e-9, name
        assert abs(room['volume_m3'] - PLAN_V[name]) < 0.06, name
        assert len(room['receivers']) == 8
        pts = [room['source_m']] + [r['position_m'] for r in room['receivers']]
        assert min(U.clearance(p) for p in pts) >= 0.6 - 1e-9, name                       # 2.2: 0.6 m from every surface
        for r in room['receivers']:
            d = math.dist(room['source_m'], r['position_m'])
            assert abs(r['d_m'] - d) < 1e-12
            assert r['class'] == ('near' if d < 2 else 'far' if d > 10 else 'mid')
        assert [r['name'] for r in room['receivers'] if r['blocked']] == BLOCKED.get(name, []), name
        classes = {r['class'] for r in room['receivers']}
        assert {'near', 'mid'} <= classes and (('far' in classes) == (name not in ('G1', 'G7'))), name
        # P5, the tests' own: Eyring with air; G3 on the chamber with the doorway at alpha 1
        if name == 'G3':
            door = 1.5 * 2.2
            chamber = ((7.7, 0, 0), (17.7, 6, 3.5))
            cv = 10.0 * 6 * 3.5
            cs = 2 * (10 * 6 + 10 * 3.5 + 6 * 3.5)
            main_v, main_s = 7.5 * 6 * 3.5, 2 * (7.5 * 6 + 7.5 * 3.5 + 6 * 3.5)
            for f in (125, 1000):
                want = eyring(cv, [(cs - door, 0.06), (door, 1.0)], f)
                assert abs(room['design_t60_s'][f] / want - 1) < 1e-9
                early = eyring(main_v, [(main_s - door, 0.35), (door, 1.0)], f)
                assert abs(room['design_t60_early_s'][f] / early - 1) < 1e-9
                assert room['design_t60_s'][f] > 4 * room['design_t60_early_s'][f], 'double slope by design'
        else:
            for f in (125, 500, 1000, 4000):
                want = eyring(U.volume, [(U.surface, alphas[0])], f)
                assert abs(room['design_t60_s'][f] / want - 1) < 1e-9, (name, f)
        for f, v in zip((125, 500, 1000, 4000), PLAN_T60[name]):
            assert abs(room['design_t60_s'][f] - v) < 0.0051, (name, f, room['design_t60_s'][f], v)
        # P17 and the solver's step limit
        want_s = P17[name]
        assert abs(driver.truth_duration_s(room) - want_s) < 1e-9, (name, driver.truth_duration_s(room))
        assert round(driver.truth_duration_s(room) / 1e-4) <= 65_536
    # the features each room is for
    assert Gm['G7']['design_t60_s'][1000] <= 0.25
    assert 2.6 <= Gm['G2']['design_t60_s'][1000] <= 2.9 and max(Gm['G2']['design_t60_s'].values()) < 3.0     # M1: inside H4
    assert Gm['G1']['volume_m3'] <= 30 and Gm['G3']['kind'] == 'non-box' and len(Gm['G4']['boxes']) == 2
    assert Gm['G5']['materials']['walls']['scattering'] == 0.1
    assert all(m['scattering'] == 1.0 for n, room in Gm.items() if n != 'G5' for m in room['materials'].values())
    # P3: fresh against everything in the list that is not a G room (min 0.164), and the G rooms apart (>= 0.15)
    others = [r for r in list2 if r['set'] != 'round2_spps']
    worst = []
    for name, room in Gm.items():
        gaps = [g for g in (gap(room, c) for c in others) if g is not None]
        assert min(gaps) >= 0.15, (name, min(gaps))
        worst.append(min(gaps))
    assert abs(min(worst) - 0.164) < 0.001, min(worst)
    names = list(Gm)
    for i, a in enumerate(names):
        for b in names[i + 1:]:
            g = gap(Gm[a], Gm[b])
            assert g is None or g >= 0.15, (a, b, g)
    assert rooms2.geometry_sha256() == rooms2.geometry_sha256() and len(rooms2.geometry_sha256()) == 64
