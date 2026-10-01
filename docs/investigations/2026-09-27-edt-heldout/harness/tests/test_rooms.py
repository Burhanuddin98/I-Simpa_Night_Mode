"""T10-T11 (HARNESS-PLAN.md section 4): SPPS-fresh's rooms, the probes' stand-ins, and the nine projects.

Expected values are HARNESS-PLAN.md 2.2's, checked in scratch on 2026-10-01 before this file was
written: every listed distance rounds to its value, every listed class holds, and every listed point
is at least 0.6 m from every surface (three of F1's at exactly 0.6 m). Three places where the plan
does not agree with itself are held to its rules, not its numbers, and flagged in the step-3 report:
- F3's design T60 (P5: its chamber, the doorway at alpha 1) is 2.11-1.56 s, not the table's
  2.16-1.59; T10 checks P5, not the table's F3 numbers.
- F7's points are F1's scaled per axis ("scaled to its box"); scaling puts F1's (2.8, 1.2, 1.9),
  0.6 m below the ceiling, 0.576 m below F7's, and moves F1's (2.4, 1.6, 0.9) from near (1.83 m) to
  mid (2.04 m). T10 checks the scaling and F1's clearance scaled with it.
- P0b's dimensions are not given; T10 checks what section 7 asks of them.
"""
import json
import math
import subprocess
from pathlib import Path

from conftest import OCTAVES, REPO, Union, eyring, p3_near_duplicate, sorted_dims

from m8b import rooms

NAMES = ('F1', 'F2', 'F3', 'F4', 'F5', 'F6', 'F7')
PROBES = ('P0', 'P0b')
# HARNESS-PLAN.md 2.2: source, then (receiver, listed distance, class, blocked)
PLAN_POINTS = {
    'F1': ((0.8, 0.8, 1.3), [((1.6, 1.2, 1.2), .90, 'near', False), ((2.0, 0.9, 1.5), 1.22, 'near', False),
                             ((1.3, 2.1, 1.1), 1.41, 'near', False), ((2.4, 1.6, 0.9), 1.83, 'near', False),
                             ((2.7, 2.2, 1.6), 2.38, 'mid', False), ((2.8, 1.9, 1.2), 2.28, 'mid', False),
                             ((2.6, 2.3, 0.7), 2.42, 'mid', False), ((2.8, 1.2, 1.9), 2.13, 'mid', False)]),
    'F2': ((3.0, 4.0, 1.6), [((4.2, 4.6, 1.3), 1.37, 'near', False), ((3.5, 5.6, 1.7), 1.68, 'near', False),
                             ((2.2, 5.0, 1.2), 1.34, 'near', False), ((7.5, 6.0, 1.4), 4.93, 'mid', False),
                             ((10.0, 3.0, 2.2), 7.10, 'mid', False), ((6.0, 10.5, 1.5), 7.16, 'mid', False),
                             ((14.5, 9.0, 1.4), 12.54, 'far', False), ((16.0, 11.5, 3.0), 15.07, 'far', False)]),
    'F3': ((2.5, 3.5, 1.5), [((3.8, 3.9, 1.2), 1.39, 'near', False), ((2.0, 5.0, 1.4), 1.58, 'near', False),
                             ((3.2, 2.0, 1.7), 1.67, 'near', False), ((6.5, 2.0, 1.2), 4.28, 'mid', False),
                             ((7.8, 5.5, 1.5), 5.66, 'mid', False), ((5.5, 6.0, 2.2), 3.97, 'mid', False),
                             ((14.0, 3.6, 1.2), 11.50, 'far', False), ((13.0, 5.8, 1.3), 10.75, 'far', True)]),
    'F4': ((2.0, 2.5, 1.5), [((3.2, 3.0, 1.2), 1.33, 'near', False), ((2.8, 1.2, 1.6), 1.53, 'near', False),
                             ((1.2, 3.8, 1.3), 1.54, 'near', False), ((6.0, 3.5, 1.4), 4.12, 'mid', False),
                             ((9.0, 1.5, 1.7), 7.07, 'mid', False), ((11.5, 4.0, 1.2), 9.62, 'mid', False),
                             ((14.5, 2.5, 1.3), 12.50, 'far', False), ((13.5, 12.0, 1.2), 14.92, 'far', True)]),
    'F5': ((3.0, 4.75, 1.6), [((4.3, 5.2, 1.3), 1.41, 'near', False), ((2.4, 3.2, 1.4), 1.67, 'near', False),
                              ((3.5, 6.4, 1.8), 1.74, 'near', False), ((7.5, 3.0, 1.2), 4.84, 'mid', False),
                              ((9.0, 7.5, 1.7), 6.60, 'mid', False), ((6.0, 8.5, 2.5), 4.89, 'mid', False),
                              ((13.0, 8.5, 1.3), 10.68, 'far', False), ((13.2, 1.2, 1.5), 10.80, 'far', False)]),
    'F6': ((2.0, 2.75, 1.5), [((3.3, 3.2, 1.2), 1.41, 'near', False), ((2.6, 1.3, 1.6), 1.57, 'near', False),
                              ((1.0, 3.9, 1.3), 1.54, 'near', False), ((6.0, 2.0, 1.4), 4.07, 'mid', False),
                              ((9.5, 4.5, 1.8), 7.71, 'mid', False), ((11.0, 1.5, 1.2), 9.09, 'mid', False),
                              ((16.0, 3.0, 1.3), 14.00, 'far', False), ((22.5, 2.2, 1.6), 20.51, 'far', False)]),
}
BOX_DIMS = {'F1': (3.4, 2.9, 2.5), 'F2': (17.0, 12.5, 8.0), 'F5': (14.0, 9.5, 6.0), 'F6': (24.0, 5.5, 3.2),
            'F7': (3.8, 3.3, 2.4), 'P0': (18.9, 9.5, 9.46)}
F4_BOXES = {((0.0, 0.0, 0.0), (16.0, 5.0, 3.5)), ((11.0, 5.0, 0.0), (16.0, 15.0, 3.5))}
ALPHA = {'F1': (0.15, 1.0), 'F2': (0.08, 1.0), 'F4': (0.12, 1.0), 'F5': (0.20, 0.2), 'F6': (0.30, 1.0),
         'F7': (0.45, 1.0), 'P0': (0.08, 1.0)}
T60_TABLE = {'F1': {125: 0.48, 4000: 0.44}, 'F2': {125: 3.63, 500: 3.46, 1000: 3.33, 4000: 2.26},
             'F4': {125: 1.20, 4000: 1.00}, 'F5': {125: 1.05, 4000: 0.89}, 'F6': {125: 0.42, 4000: 0.39},
             'F7': {500: 0.14}, 'P0': {500: 3.46}}


def box_set(boxes):
    return {tuple(tuple(round(float(x), 9) for x in corner) for corner in b) for b in boxes}


def klass(d):
    return 'near' if d < 2.0 else ('far' if d > 10.0 else 'mid')


def scaled(points, f):
    return [tuple(p[i] * f[i] for i in range(3)) for p in points]


def close(a, b, tol=1e-9):
    return all(abs(x - y) <= tol * max(1.0, abs(y)) for x, y in zip(a, b))


def check_corpus_kinds(corpus_rooms, target_root):
    """P3's receipts: every corpus room's recorded kind is what its source says, line for line."""
    roots = {'checkout': target_root.parent, 'worktree': REPO}
    texts = {}
    for room in corpus_rooms['rooms']:
        assert room['kind'] == 'box', room['id']
        says_box = False
        for e in room['kind_evidence']:
            key = (e['root'], e['path'], str(e['line']))
            if key not in texts:
                a, _, b = str(e['line']).partition('-')
                lines = (roots[e['root']] / e['path']).read_text(encoding='utf-8').replace('\r\n', '\n').split('\n')
                texts[key] = ' '.join(x.strip() for x in lines[int(a) - 1:int(b or a)])
            assert texts[key] == e['text'], '%s: %s:%s no longer says %r' % (room['id'], e['path'], e['line'], e['text'])
            says_box |= 'box' in e['text'].lower()
            if '12 faces' in e['text']:               # upstream's atmospheric room: 12 triangles, its V and S
                dims = room['dims_m']
                says_box |= (abs(room['volume_m3'] - dims[0] * dims[1] * dims[2]) < 1e-9
                             and abs(room['surface_m2'] - 2 * (dims[0] * dims[1] + dims[0] * dims[2] + dims[1] * dims[2])) < 1e-9)
        assert says_box, '%s: no source line names it a box' % room['id']


def test_t10_rooms_meet_p3_p5_and_section_2_2(corpus_rooms, target_root):
    """T10: the seven rooms meet P3, P5 and 2.2 (features, V, design T60s, distance classes,
    clearance, blocked flags); F2 is the longest by design; P3 holds against every corpus room, whose
    recorded kinds match their sources; P0 and P0b stand to F2 and F7 as section 7 says."""
    check_corpus_kinds(corpus_rooms, target_root)

    R = rooms.rooms()
    assert R is not None, 'rooms.rooms() returned nothing'
    assert set(R) == set(NAMES + PROBES)
    unions = {}
    for name in NAMES + PROBES:
        room = R[name]
        U = unions[name] = Union(room['boxes'])
        assert abs(U.volume / room['volume_m3'] - 1) <= 1e-9 and abs(U.surface / room['surface_m2'] - 1) <= 1e-9, name
        assert room['kind'] == ('box' if len(room['boxes']) == 1 else 'non-box'), name
        assert set(room['design_t60_s']) == set(OCTAVES) or set(map(int, room['design_t60_s'])) == set(OCTAVES), name
        assert abs(sum(room['material_area_m2'].values()) - room['surface_m2']) <= 1e-6, name
        assert len(room['receivers']) == 8, name
        src = room['source_m']
        for r in room['receivers']:
            d = math.dist(src, r['position_m'])
            assert abs(r['d_m'] - d) <= 1e-9 and r['class'] == klass(d), (name, r['name'])
            assert r['blocked'] == (not U.segment_inside(src, r['position_m'])), (name, r['name'])

    def t60(name, band):
        dt = R[name]['design_t60_s']
        return float(dt.get(band, dt.get(str(band))))

    # -- geometry and materials, room by room
    for name, dims in BOX_DIMS.items():
        assert box_set(R[name]['boxes']) == box_set([((0, 0, 0), dims)]), name
    assert box_set(R['F4']['boxes']) == box_set(F4_BOXES)
    f3 = box_set(R['F3']['boxes'])
    main, chamber = ((0.0, 0.0, 0.0), (9.0, 7.0, 4.0)), ((9.2, 0.0, 0.0), (15.2, 7.0, 4.0))
    assert main in f3 and chamber in f3 and len(f3) == 3
    (lo, hi), = f3 - {main, chamber}
    assert (lo[0], hi[0]) == (9.0, 9.2) and abs(hi[1] - lo[1] - 2.0) < 1e-9 and abs(hi[2] - lo[2] - 2.5) < 1e-9
    assert 0.0 <= lo[1] and hi[1] <= 7.0 and 0.0 <= lo[2] and hi[2] <= 4.0, 'the doorway lies in the partition'
    for name, (a, s) in ALPHA.items():
        mats = list(R[name]['materials'].values())
        assert [(m['absorption'], m['scattering']) for m in mats] == [(a, s)], name
    f3m = {round(m['absorption'], 9): (m['scattering'], R['F3']['material_area_m2'][k]) for k, m in R['F3']['materials'].items()}
    assert set(f3m) == {0.40, 0.04} and f3m[0.40][0] == 1.0 and f3m[0.04][0] == 1.0
    assert 248.5 <= f3m[0.40][1] <= 249.9, 'main room and its partition face at 0.40: 249 m2 (+0.4 if the doorway floor)'
    assert 183.9 <= f3m[0.04][1] <= 185.3, 'chamber, its partition face and the doorway reveals at 0.04: 184.4-184.8 m2'

    # -- sources and receivers: 2.2's table; F7 is F1's scaled per axis (flagged in the docstring)
    f7 = [b / a for a, b in zip(BOX_DIMS['F1'], BOX_DIMS['F7'])]
    plan = dict(PLAN_POINTS)
    plan['F7'] = (scaled([PLAN_POINTS['F1'][0]], f7)[0],
                  [(p, None, klass(math.dist(p, scaled([PLAN_POINTS['F1'][0]], f7)[0])), False)
                   for p in scaled([r[0] for r in PLAN_POINTS['F1'][1]], f7)])
    for name in NAMES:
        src, recs = plan[name]
        room = R[name]
        assert close(room['source_m'], src), name
        for r, (p, listed, cls, blocked) in zip(room['receivers'], recs):
            assert close(r['position_m'], p), (name, r['name'])
            if listed is not None:
                assert round(r['d_m'], 2) == listed, (name, r['name'], r['d_m'])
            assert r['class'] == cls and r['blocked'] == blocked, (name, r['name'])
        floor = 0.6 if name != 'F7' else 0.6 * min(1.0, *f7)
        for p in [room['source_m']] + [r['position_m'] for r in room['receivers']]:
            assert unions[name].clearance(p) >= floor - 1e-9, (name, p, unions[name].clearance(p))
        classes = {r['class'] for r in room['receivers']}
        assert {'near', 'mid'} <= classes, name
        if name in ('F2', 'F3', 'F4', 'F5', 'F6'):
            assert 'far' in classes, name

    # -- P3 against every corpus room (all boxes; F3 and F4 are not)
    for name in NAMES:
        for c in corpus_rooms['rooms']:
            assert not p3_near_duplicate(R[name], c), '%s is a near-duplicate of %s' % (name, c['id'])

    # -- P5, the tests' own Eyring; the table's numbers where P5 gives them
    for name in NAMES + PROBES:
        room = R[name]
        for band in OCTAVES:
            if name == 'F3':
                want = eyring(6 * 7 * 4, [(183.0, 0.04), (5.0, 1.0)], band)       # chamber, doorway at alpha 1
                assert abs(t60(name, band) / want - 1) <= 0.005, (name, band, t60(name, band), want)
            else:
                (a, _), = [(m['absorption'], m['scattering']) for m in room['materials'].values()]
                want = eyring(room['volume_m3'], [(room['surface_m2'], a)], band)
                assert abs(t60(name, band) / want - 1) <= 1e-9, (name, band, t60(name, band), want)
    for name, cells in T60_TABLE.items():
        for band, listed in cells.items():
            assert round(t60(name, band), 2) == listed, (name, band, t60(name, band))

    # -- the features 2.2 assigns (PREREG.md:20-26)
    assert R['F1']['volume_m3'] <= 30.0
    assert round(R['F7']['volume_m3'], 1) == 30.1
    assert R['F3']['kind'] == 'non-box' and R['F4']['kind'] == 'non-box'
    assert any(r['blocked'] for r in R['F3']['receivers']) and any(r['blocked'] for r in R['F4']['receivers'])
    for band in OCTAVES:
        assert all(t60('F2', band) > t60(n, band) for n in NAMES if n != 'F2'), 'F2 is the longest at %d Hz' % band
    assert t60('F2', 500) >= 2.5 and t60('F2', 1000) >= 2.5

    # -- the probes' stand-ins (section 7): P0 to F2, P0b to F7
    for probe, ref in (('P0', 'F2'), ('P0b', 'F7')):
        P, F = R[probe], R[ref]
        assert P['kind'] == 'box'
        assert abs(P['volume_m3'] / F['volume_m3'] - 1) <= 0.01 and abs(P['surface_m2'] / F['surface_m2'] - 1) <= 0.01
        (pa, ps), = [(m['absorption'], m['scattering']) for m in P['materials'].values()]
        (fa, fs), = [(m['absorption'], m['scattering']) for m in F['materials'].values()]
        assert abs(pa / fa - 1) <= 0.01 and ps == fs
        pd = [hi - lo for lo, hi in zip(*P['bbox_m'])]
        fd = [hi - lo for lo, hi in zip(*F['bbox_m'])]
        assert all(abs(a / b - 1) > 0.10 for a, b in zip(pd, fd)), '%s must differ from %s by more than 10 %% on every axis' % (probe, ref)
        assert not p3_near_duplicate(P, F), '%s is a near-duplicate of %s' % (probe, ref)
        f = [a / b for a, b in zip(pd, fd)]
        assert close(P['source_m'], scaled([F['source_m']], f)[0])
        for rp, rf in zip(P['receivers'], F['receivers']):
            assert close(rp['position_m'], scaled([rf['position_m']], f)[0]), (probe, rp['name'])
        floor = 0.6 * min(1.0, *f) * (min(1.0, *f7) if ref == 'F7' else 1.0)
        for p in [P['source_m']] + [r['position_m'] for r in P['receivers']]:
            assert unions[probe].clearance(p) >= floor - 1e-9, (probe, p)
    assert sorted_dims(R['P0']) == sorted(BOX_DIMS['P0'], reverse=True)


def run_simpa(exe, *args):
    return subprocess.run([str(exe), *map(str, args)], capture_output=True, text=True, timeout=600)


def test_t11_projects_validate_check_and_match_p8_to_p12(tmp_path, simpa_exe):
    """T11: each of the nine projects passes `simpa validate` (exit 0) and `simpa check`; its settings
    are a freshly imported project's except P9's (random_seed non-zero, intersection files off,
    fittings off); its duration equals a freshly imported project's (P10); its bands, materials,
    source and receivers are the room's."""
    R = rooms.rooms()
    assert R is not None, 'rooms.rooms() returned nothing'
    out = tmp_path / 'projects'
    written = rooms.write_projects(out, simpa_exe)
    assert written is not None, 'rooms.write_projects returned nothing'
    assert set(written) == set(NAMES + PROBES)
    p9 = {'random_seed', 'save_surface_intersections', 'save_receiver_intersections', 'fittings'}
    for name in NAMES + PROBES:
        path = Path(written[name])
        assert path.is_file() and out in path.parents, (name, path)
        v = run_simpa(simpa_exe, 'validate', path, '--json')
        assert v.returncode == 0, '%s: simpa validate exit %d: %s' % (name, v.returncode, v.stdout[-2000:] + v.stderr[-2000:])
        c = run_simpa(simpa_exe, 'check', path, '--json')
        assert c.returncode == 0, '%s: simpa check exit %d: %s' % (name, c.returncode, c.stdout[-2000:] + c.stderr[-2000:])

        obj = tmp_path / ('%s.obj' % name)
        obj.write_text(rooms.obj_text(R[name]), encoding='utf-8', newline='\n')
        fresh = tmp_path / ('%s-fresh.simpa' % name)
        i = run_simpa(simpa_exe, 'import', obj, fresh, '--unit', 'm', '--up', 'z', '--json')
        assert i.returncode == 0, '%s: simpa import exit %d: %s' % (name, i.returncode, i.stderr[-2000:])
        P = json.loads(path.read_text(encoding='utf-8'))
        F = json.loads(fresh.read_text(encoding='utf-8'))
        sp, fp = P['solvers']['spps'], F['solvers']['spps']
        assert set(sp) == set(fp), name
        for key in sorted(set(fp) - p9):
            assert sp[key] == fp[key], '%s: solvers.spps.%s is %r, a fresh import has %r' % (name, key, sp[key], fp[key])
        assert isinstance(sp['random_seed'], int) and sp['random_seed'] != 0, name
        assert sp['save_surface_intersections'] is False and sp['save_receiver_intersections'] is False, name
        assert sp['fittings'] is False and P['fitting_zones'] == [], name
        # P8 spelled out: what a fresh import gives today
        assert sp['method'] == 'random' and sp['air_absorption'] is True and sp['receiver_radius_m'] == 0.31
        assert sp['extinction_exponent'] == 5.0 and sp['particles_per_source'] == 150000 and sp['particles_saved'] == 0
        assert sp['echogram_per_source'] is False and all(sp['bands_computed'])
        assert sp['duration_s'] == fp['duration_s'], '%s: duration %r, a fresh import %r (P10)' % (name, sp['duration_s'], fp['duration_s'])
        assert P['bands'] == F['bands'] == {'kind': 'octave', 'frequencies_hz': list(OCTAVES)}
        assert P['environment'] == F['environment']
        # the room in the project
        room = R[name]
        enabled = [s for s in P['sources'] if s['enabled']]
        assert len(enabled) == 1 and close(enabled[0]['position'], room['source_m']), name
        assert len(P['point_receivers']) == 8
        for pr, r in zip(P['point_receivers'], room['receivers']):
            assert close(pr['position'], r['position_m']), (name, pr['name'])
        mats = {m['id']: m for m in P['materials']}
        used = set()
        for g in P['surface_groups']:
            m = mats[g['material']]
            assert m['reflection_law'] == 'lambert', (name, m['name'])
            assert len(set(m['absorption'])) == 1 and len(set(m['scattering'])) == 1, (name, m['name'])
            used.add((round(m['absorption'][0], 9), round(m['scattering'][0], 9)))
        want = {(round(m['absorption'], 9), round(m['scattering'], 9)) for m in room['materials'].values()}
        assert used == want, (name, used, want)
