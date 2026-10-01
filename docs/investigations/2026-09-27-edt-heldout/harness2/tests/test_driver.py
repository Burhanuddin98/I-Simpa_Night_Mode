"""T12-T14 as round 2 reads them (HARNESS-PLAN-2.md sections 3 and 6; round 1's version: ../../harness/tests/test_driver.py):
the driver's plan-only mode, its guards, and its reader. T34 and T35 (test_r2_plan_guards.py) hold round 2's new guards.

No test here can launch a solver: every launch() call names a project and a simpa.exe that do not
exist, so a guard that fails cannot start a run. Nothing is written under C:\\tmp\\m8b-edt\\heldout.

P14 against the CLI (HARNESS-PLAN.md 8.1): `simpa run` records no solver check in run.json
(crates/simpa/src/mesh_run.rs:520, `verify: None`), so results::solver_build reads every CLI run
'solver_build_unrecorded'. T13 holds the guard as P14 states it. The truth and tested runs wait for
backlog 54's CLI half (decision row 33); the probes run no `simpa results`, so only the check before
the run applies to them.
"""
import collections
import glob
import importlib.util
import json
import math
import shutil
import sys
from pathlib import Path

import numpy as np
from conftest import REPO, SCRATCH

from m8b import driver, ism_fresh, rooms2, synth_fresh

NAMES = ('G1', 'G2', 'G3', 'G4', 'G5', 'G6', 'G7')
SEEDS = {(1.0, 150000): (3101, 3102, 3103), (2.0, 150000): (3201, 3202, 3203), (5.0, 150000): (3501, 3502, 3503),
         (1.0, 50000): (4101, 4102, 4103), (2.0, 50000): (4201, 4202, 4203), (5.0, 50000): (4501, 4502, 4503)}   # section 3
TRUTH_SEEDS = (9101, 9102, 9103, 9104)
P17_LISTED = {'G1': 3.0, 'G2': 5.9, 'G3': 3.9, 'G4': 3.0, 'G5': 3.0, 'G6': 3.0, 'G7': 3.0}     # P17 on the G rooms
EXES = ('spps.exe', 'classicalTheory.exe', 'tetgen.exe', 'preprocess.exe')
DEV_SEED = 4242


def p17_duration(room):
    t_arr = max(r['d_m'] for r in room['receivers']) / 343.2
    t60 = max(float(v) for v in room['design_t60_s'].values())
    x = min(6.5, max(3.0, t_arr + 2.0 * t60))
    return math.ceil(x * 10 - 1e-9) / 10


def test_t12_plan_only_lists_the_matrix_with_p12_seeds():
    """T12: plan-only lists exactly 144 tested runs (72 in each mode) and 28 truth runs with P12's
    seeds; no seed repeats within a room and mode, truth seeds are apart from tested ones, and 9999
    and 9998 are absent. It creates nothing."""
    before = sorted(p.name for p in SCRATCH.iterdir())
    runs = driver.plan()
    assert runs is not None, 'driver.plan() returned nothing'
    assert sorted(p.name for p in SCRATCH.iterdir()) == before, 'plan-only created something under %s' % SCRATCH
    R = rooms2.rooms()
    assert R is not None
    tested = [r for r in runs if r['kind'] == 'tested']
    truth = [r for r in runs if r['kind'] == 'truth']
    assert len(runs) == 172 and len(tested) == 144 and len(truth) == 28
    assert collections.Counter(r['mode'] for r in tested) == {'random': 72, 'energetic': 72}
    assert {r['room'] for r in runs} == set(NAMES)
    assert len({r['run_id'] for r in runs}) == len(runs)
    all_seeds = {r['random_seed'] for r in runs}
    assert 9999 not in all_seeds and 9998 not in all_seeds
    tested_seeds = {r['random_seed'] for r in tested}
    for mode in ('random', 'energetic'):
        for room in NAMES:
            rs = [r for r in tested if r['mode'] == mode and r['room'] == room]
            want = {(step, n, s) for (step, n), seeds in SEEDS.items() for s in seeds if n == 150000 or room == 'G6'}
            got = {(round(r['time_step_s'] * 1e3, 9), r['particles_per_source'], r['random_seed']) for r in rs}
            assert got == want and len(rs) == len(want), (mode, room)
            assert len({r['random_seed'] for r in rs}) == len(rs), 'a seed repeats in %s %s' % (room, mode)
    for room in NAMES:
        durations = {r['duration_s'] for r in tested if r['room'] == room}
        assert len(durations) == 1 and min(durations) > 0, 'P10: one run length per room (%s)' % room
        ts = [r for r in truth if r['room'] == room]
        assert sorted(r['random_seed'] for r in ts) == list(TRUTH_SEEDS)
        assert not tested_seeds & set(TRUTH_SEEDS)
        want_s = p17_duration(R[room])
        if room in P17_LISTED:
            assert want_s == P17_LISTED[room], (room, want_s)
        for r in ts:
            assert r['mode'] == 'random' and r['particles_per_source'] == 1_000_000
            assert abs(r['time_step_s'] - 1e-4) <= 1e-15
            assert abs(r['duration_s'] - want_s) <= 1e-9, (room, r['duration_s'], want_s)
            assert round(r['duration_s'] / r['time_step_s']) < 65_536


def load_pe_fingerprint():
    path = REPO / 'tools' / 'fixture-gen' / 'pe_fingerprint.py'
    spec = importlib.util.spec_from_file_location('m8b_test_pe_fingerprint', path)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = mod
    spec.loader.exec_module(mod)
    return mod


def flip_text_byte(src, dst):
    """A copy of a PE file with the middle byte of .text flipped, as solvers/pe-fingerprint.ps1's
    Copy-PeFlippedText makes M8a's N1."""
    pf = load_pe_fingerprint()
    data = bytearray(src.read_bytes())
    text = [s for s in pf.headers(bytes(data), str(src)).sections if s.name == '.text']
    assert len(text) == 1 and text[0].rsize > 0
    data[text[0].rptr + text[0].rsize // 2] ^= 0xFF
    dst.write_bytes(bytes(data))


def dev_run(seed=DEV_SEED, run_id='t13'):
    return dict(run_id=run_id, room='G1', kind='dev', mode='random', time_step_s=0.001,
                particles_per_source=1000, random_seed=seed, duration_s=0.1)


def write_run_folder(path, seed, report):
    path.mkdir(parents=True)
    (path / 'project.simpa').write_text(json.dumps({'solvers': {'spps': {'random_seed': seed}}}), encoding='utf-8')
    (path / 'report.json').write_text(json.dumps(report), encoding='utf-8')


def noise_cal_reports(target_root):
    """The corpus's real-run reports, as the old loader found them (loaders.py:121-134, 157)."""
    allowed = {'C-E3', 'C-E4', 'C-E6', 'V-E2', 'V-E5', 'C-R3', 'C-R4', 'C-R6', 'V-R2', 'V-R6'}
    out = []
    for name in ('noise-cal-1790307822', 'noise-cal-1790310131'):
        root = target_root / 'agents' / 'pm8-noise-scratch' / 'runs' / name
        for c in json.loads((root / 'cells.json').read_text(encoding='utf-8')):
            cc = c if 'id' in c else c['cell']
            if cc['id'] in allowed and abs(cc['time_step_s'] - 0.001) <= 1e-9:
                out += sorted(glob.glob(str(root / cc['id'] / 'seed*' / 'report.json')))
    return sorted(out)


def refused(code, fn, *a, **k):
    try:
        got = fn(*a, **k)
    except driver.Refused as e:
        assert e.code == code, 'refused %r, expected %r' % (e.code, code)
        return
    raise AssertionError('%s was not refused (%s expected); it returned %r' % (fn.__name__, code, got))


def test_t13_guards_refuse_before_anything_runs(tmp_path, solvers_dir, target_root):
    """T13: a flipped spps.exe is refused before any run; a report whose solver_build is unverified is
    refused; seed 9999, or a folder outside the data root, is refused; a held-out seed (SPPS, ISM or
    Synth) is refused while ADDENDUM-B1.md is not committed."""
    root = tmp_path / 'data'
    root.mkdir()
    log = tmp_path / 'launch.log'
    flipped = tmp_path / 'flipped'
    flipped.mkdir()
    for exe in EXES:
        if exe == 'spps.exe':
            flip_text_byte(solvers_dir / exe, flipped / exe)
        else:
            shutil.copyfile(solvers_dir / exe, flipped / exe)
    nothing = dict(project=tmp_path / 'no-project.simpa', simpa_exe=tmp_path / 'no-simpa.exe', launch_log=log)

    good = driver.check_solvers(solvers_dir)
    assert good is not None and good['verified'] is True, 'the verified build must pass the check'
    bad = driver.check_solvers(flipped)
    assert bad['verified'] is False
    assert [c['name'] for c in bad['checks'] if not c['matches']] == ['spps.exe']

    # the solvers, the folder, the seed: each refused before anything is written or launched
    refused('solver_unverified', driver.launch, dev_run(), run_dir=root / 'a', solvers_dir=flipped, data_root=root, **nothing)
    refused('outside_data_root', driver.launch, dev_run(), run_dir=tmp_path / 'elsewhere' / 'a', solvers_dir=solvers_dir,
            data_root=root, **nothing)
    refused('reserved_seed', driver.launch, dev_run(9999), run_dir=root / 'b', solvers_dir=solvers_dir, data_root=root, **nothing)
    refused('reserved_seed', driver.launch, dev_run(9998), run_dir=root / 'b', solvers_dir=solvers_dir, data_root=root, **nothing)

    # held-out seeds before B1 (section 3's, Synth-fresh-2's and ISM-fresh-2's)
    heldout = [s for seeds in SEEDS.values() for s in seeds] + list(TRUTH_SEEDS)
    for s in heldout + [2026100201, 2026100202, 2026100203]:
        assert driver.heldout_seed(s) is True, s
    for s in (DEV_SEED, 9999, 9998, 7, 20261001, 1101, 2026100101):         # round 1's seeds are refused by T35, not held out here
        assert driver.heldout_seed(s) is False, s
    for s in heldout:
        refused('heldout_before_b1', driver.launch, dev_run(s), run_dir=root / ('h%d' % s), solvers_dir=solvers_dir,
                data_root=root, b1=False, **nothing)
    refused('heldout_before_b1', driver.require_not_heldout, 3101, b1=False)
    driver.require_not_heldout(3101, b1=True)                  # after B1 the seed is allowed
    refused('heldout_before_b1', synth_fresh.draw, 2026100202, b1=False)
    refused('heldout_before_b1', ism_fresh.draw, 2026100201, b1=False)
    assert not log.exists() or log.read_text(encoding='utf-8') == '', 'a refused launch wrote the launch log'
    assert sorted(p.name for p in root.iterdir()) == [], 'a refused launch wrote under the data root'

    # the reader: an unverified build, a reserved seed, a folder outside the data root
    report = json.loads(Path(noise_cal_reports(target_root)[0]).read_text(encoding='utf-8'))
    verified = dict(report, solver_build={'status': 'verified'})
    unverified = dict(report, solver_build={'status': 'unverified', 'reason': {
        'code': 'solver_build_unrecorded', 'detail': 'run.json records no check of the executables'}})
    write_run_folder(root / 'r1', DEV_SEED, unverified)
    write_run_folder(root / 'r2', 9999, verified)
    write_run_folder(root / 'r3', DEV_SEED, verified)
    write_run_folder(tmp_path / 'outside' / 'r4', DEV_SEED, verified)
    refused('solver_build_unverified', driver.read_run, root / 'r1', data_root=root)
    refused('reserved_seed', driver.read_run, root / 'r2', data_root=root)
    refused('outside_data_root', driver.read_run, tmp_path / 'outside' / 'r4', data_root=root)
    got = driver.read_run(root / 'r3', data_root=root)               # the control: a good run is read
    assert got is not None and len(got) == len(driver.series_from_report(verified)) > 0


def test_t14_reader_returns_the_old_loaders_fields(target_root):
    """T14: for 3 corpus noise-cal report.json files, the reader returns exactly the old real loader's
    energy_pa2, arrival_s, R/c and dt (the scratchpad's edtsimp/loaders.py:159-181, re-typed here)."""
    paths = noise_cal_reports(target_root)
    assert len(paths) >= 3
    picked = [paths[0], paths[(len(paths) - 1) // 2], paths[-1]]
    for p in picked:
        report = json.loads(Path(p).read_text(encoding='utf-8'))
        d = report['spps']                                           # loaders.py:159
        c = d['speed_of_sound_m_s']
        R = d['receiver_radius_m']
        h = R / c
        fine_dt = d['time_step_s']
        old = {}
        for freq in (b['freq_hz'] for b in d['point_receivers'][0]['bands']):
            for pr in d['point_receivers']:
                bnd = next(b for b in pr['bands'] if b['freq_hz'] == freq)
                old[(pr['label'], freq)] = (np.asarray(bnd['energy_pa2'], float), pr['arrival_s'], h, fine_dt)
        got = driver.series_from_report(report)
        assert got is not None, 'driver.series_from_report returned nothing'
        assert len(got) == len(old), p
        for g in got:
            e, arr, hw, dt = old[(g['label'], g['band_hz'])]
            assert isinstance(g['energy_pa2'], np.ndarray) and g['energy_pa2'].dtype == np.float64
            assert np.array_equal(g['energy_pa2'], e), (p, g['label'], g['band_hz'])
            assert g['arrival_s'] == arr and g['half_width'] == hw and g['dt'] == dt, (p, g['label'], g['band_hz'])
