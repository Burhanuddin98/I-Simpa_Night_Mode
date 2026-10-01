"""T34 and T35 (../../HARNESS-PLAN-2.md sections 3 and 6): round 2's matrix, its seeds, and the guards that keep
round 2's rooms and seeds shut until ADDENDUM-B1.md is committed and keep round 1's out for good.

No test here can launch a solver: every launch() that is meant to pass the guards has subprocess.run replaced by
a recorder, and every project and simpa.exe named here does not exist.
"""
import collections
import json
import math
import subprocess

import pytest
from conftest import SCRATCH

from m8b import driver, rooms, rooms2, run_heldout as rh

G = ('G1', 'G2', 'G3', 'G4', 'G5', 'G6', 'G7')
SEEDS2 = {(1.0, 150000): (3101, 3102, 3103), (2.0, 150000): (3201, 3202, 3203), (5.0, 150000): (3501, 3502, 3503),
          (1.0, 50000): (4101, 4102, 4103), (2.0, 50000): (4201, 4202, 4203), (5.0, 50000): (4501, 4502, 4503)}
TRUTH2 = (9101, 9102, 9103, 9104)
ROUND1 = ([1101, 1102, 1103, 1201, 1202, 1203, 1501, 1502, 1503, 2101, 2102, 2103, 2201, 2202, 2203, 2501, 2502, 2503,
           9001, 9002, 9003, 9004, 2026100101, 2026100102])        # round 1's HELDOUT_SEEDS, typed here
PROBES, DEV = (9998, 9999), (1, 20261001, 4242)
P17 = {'G1': 3.0, 'G2': 5.9, 'G3': 3.9, 'G4': 3.0, 'G5': 3.0, 'G6': 3.0, 'G7': 3.0}


def test_t34_plan_is_the_round_2_matrix_with_the_round_2_seeds():
    before = sorted(p.name for p in SCRATCH.iterdir())
    runs = driver.plan()
    assert sorted(p.name for p in SCRATCH.iterdir()) == before, 'plan() created something'
    tested = [r for r in runs if r['kind'] == 'tested']
    truth = [r for r in runs if r['kind'] == 'truth']
    assert len(runs) == 172 and len(tested) == 144 and len(truth) == 28
    assert collections.Counter(r['mode'] for r in tested) == {'random': 72, 'energetic': 72}
    assert {r['room'] for r in runs} == set(G) and len({r['run_id'] for r in runs}) == 172
    assert runs[0]['run_id'] == 'tested-G1-random-1.0ms-150k-3101'
    for mode in ('random', 'energetic'):
        for room in G:
            rs = [r for r in tested if r['mode'] == mode and r['room'] == room]
            want = {(step, n, s) for (step, n), seeds in SEEDS2.items() for s in seeds if n == 150000 or room == 'G6'}
            got = {(round(r['time_step_s'] * 1e3, 9), r['particles_per_source'], r['random_seed']) for r in rs}
            assert got == want and len(rs) == len(want) == (18 if room == 'G6' else 9), (mode, room)
    R = rooms2.rooms()
    for room in G:
        ts = [r for r in truth if r['room'] == room]
        assert sorted(r['random_seed'] for r in ts) == list(TRUTH2)
        assert {r['duration_s'] for r in tested if r['room'] == room} == {10.0}
        for r in ts:
            assert r['mode'] == 'random' and r['particles_per_source'] == 1_000_000 and abs(r['time_step_s'] - 1e-4) < 1e-15
            assert abs(r['duration_s'] - P17[room]) < 1e-9 and round(r['duration_s'] / r['time_step_s']) <= 65_536
    seeds = {r['random_seed'] for r in runs}
    assert not seeds & set(ROUND1) and not seeds & set(PROBES) and not seeds & set(DEV)
    assert driver.HELDOUT_SEEDS == frozenset([s for v in SEEDS2.values() for s in v] + list(TRUTH2)
                                             + [2026100201, 2026100202, 2026100203])
    assert not driver.HELDOUT_SEEDS & set(ROUND1) and not driver.HELDOUT_SEEDS & set(PROBES)
    assert driver.ISM_SEED == 2026100201 and driver.SYNTH_SEED == 2026100202 and driver.ATTACK_SEED == 2026100203
    assert driver.RESERVED_SEEDS == PROBES and set(driver.ROUND1_SEEDS) == set(ROUND1)
    assert driver.ROOM_ALLOWLIST == set(G) | set(rooms.PROBES)
    assert [r['run_id'] for r in rh.ordered_runs()[:1]] == ['truth-G1-9101']


# ---- T35 ------------------------------------------------------------------------------------------------------------
DEV_SEED = 4242


def dev_run(room='G1', seed=DEV_SEED, run_id='t35'):
    return dict(run_id=run_id, room=room, kind='dev', mode='random', time_step_s=0.001, particles_per_source=1000,
                random_seed=seed, duration_s=0.1)


def refused(code, fn, *a, **k):
    try:
        got = fn(*a, **k)
    except driver.Refused as e:
        assert e.code == code, 'refused %r, expected %r' % (e.code, code)
        return e
    raise AssertionError('%s was not refused (%s expected); it returned %r' % (fn.__name__, code, got))


def test_t35_guards_for_round_2(tmp_path, solvers_dir, monkeypatch):
    root = tmp_path / 'data'
    root.mkdir()
    log = tmp_path / 'launch.log'
    nothing = dict(project=tmp_path / 'no-project.simpa', simpa_exe=tmp_path / 'no-simpa.exe', launch_log=log)
    go = lambda run, **k: driver.launch(run, run_dir=root / run['run_id'], solvers_dir=solvers_dir, data_root=root, **nothing, **k)
    # round 2's seeds and G rooms: shut before B1
    for s in [3101, 4503, 9101, 2026100201, 2026100202]:
        assert driver.heldout_seed(s) is True, s
        refused('heldout_before_b1', go, dev_run(seed=s), b1=False)
    for room in G:
        refused('heldout_before_b1', go, dev_run(room=room), b1=False)                  # a dev seed does not open a G room
    refused('heldout_before_b1', driver.require_not_heldout, 3101, b1=False)
    driver.require_not_heldout(3101, b1=True)
    # round 1's seeds and F rooms: never, B1 or not
    for s in ROUND1:
        refused('round1_seed', go, dev_run(seed=s), b1=True)
    refused('round1_seed', driver.require_not_heldout, 1101, b1=True)
    refused('round1_seed', driver.require_not_heldout, 2026100102, b1=False)
    for room in rooms.NAMES:
        refused('unknown_room', go, dev_run(room=room), b1=True)
    for s in PROBES:
        refused('reserved_seed', go, dev_run(seed=s), b1=True)
    # round 1's data roots
    for old in (r'B:\data\m8b-edt\heldout', r'C:\tmp\m8b-edt\heldout', r'B:\data\m8b-edt\results\run1'):
        refused('round1_root', driver.launch, dev_run(), project=nothing['project'], simpa_exe=nothing['simpa_exe'],
                    launch_log=log, run_dir=__import__('pathlib').Path(old) / 'x', solvers_dir=solvers_dir,
                    data_root=__import__('pathlib').Path(old), b1=True)
    assert not log.exists() or log.read_text(encoding='utf-8') == ''
    assert sorted(root.iterdir()) == [], 'a refused launch wrote under the data root'
    # after B1, with a real G room and room to run, launch gets to the solver call (recorded, never started)
    calls = []

    class Done:
        returncode = 0

    monkeypatch.setattr(driver.subprocess, 'run', lambda cmd, **k: calls.append(cmd) or Done())
    project = tmp_path / 'G1.simpa'
    project.write_text(json.dumps({'name': 'G1', 'solvers': {'spps': {}}}), encoding='utf-8')
    rec = driver.launch(dev_run(seed=3101), project=project, simpa_exe=tmp_path / 'simpa.exe', launch_log=log,
                        run_dir=root / 'ok', solvers_dir=solvers_dir, data_root=root, b1=True)
    assert len(calls) == 1 and rec['run']['room'] == 'G1' and (root / 'ok' / 'project.simpa').is_file()
    monkeypatch.undo()
    # B1 is "committed" only when HEAD holds it and the working copy is HEAD's (a throw-away repo)
    repo = tmp_path / 'repo'
    (repo / driver.ADDENDUM).parent.mkdir(parents=True)
    git = lambda *a: subprocess.run(['git', '-C', str(repo), '-c', 'user.name=t', '-c', 'user.email=t@t', *a],
                                    capture_output=True, text=True, check=True)
    git('init', '-q')
    assert driver.ADDENDUM.endswith('ADDENDUM-B1.md')
    assert driver.b1_committed(repo) is False
    (repo / driver.ADDENDUM).write_text('b1\n', encoding='utf-8')
    git('add', '-A')
    assert driver.b1_committed(repo) is False, 'staged is not committed'
    git('commit', '-q', '-m', 'b1')
    assert driver.b1_committed(repo) is True
    (repo / driver.ADDENDUM).write_text('b1 edited\n', encoding='utf-8')
    assert driver.b1_committed(repo) is False


def test_t35_runner_limits_and_paths(tmp_path, monkeypatch):
    # at most 4 workers: the runner refuses 5 at the command line and in the queue itself
    with pytest.raises(SystemExit):
        rh.main(['--workers', '5', '--dry-run'])
    assert rh.main(['--workers', '4', '--dry-run']) == 0
    with pytest.raises(ValueError):
        rh.run_queue([], projects={}, data_root=tmp_path, solvers_dir=tmp_path, launch_log=tmp_path / 'l', simpa_exe=tmp_path,
                     progress_log=tmp_path / 'p', workers=5, stop_file=tmp_path / 'STOP')
    # the defaults are round 2's and nobody else's
    assert str(rh.DEFAULT_DATA_ROOT).replace('/', '\\') == r'B:\data\m8b-edt\round2\heldout'
    assert str(rh.DEFAULT_ROOMS_ROOT).replace('/', '\\') == r'C:\tmp\m8b-edt\round2-rooms'
    assert 'round2' in str(rh.DEFAULT_PROGRESS_LOG) and 'round2' in str(driver.LAUNCH_LOG)
    # a room-project folder on B: is refused, before anything is written
    monkeypatch.setattr(driver, 'check_solvers', lambda d: dict(verified=True, checks=[]))
    b_root = __import__('pathlib').Path(r'B:\m8b-edt-t35-must-not-exist\rooms')
    with pytest.raises(ValueError):
        rh.ensure_rooms(b_root, tmp_path / 'simpa.exe', tmp_path / 'solvers')
    assert not b_root.exists() and not b_root.parent.exists()
    with pytest.raises(ValueError):
        rh.ensure_rooms(__import__('pathlib').Path(r'B:\data\m8b-edt\round2\rooms'), tmp_path / 'simpa.exe', tmp_path / 'solvers')
