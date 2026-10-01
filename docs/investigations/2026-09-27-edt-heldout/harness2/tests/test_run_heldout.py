"""m8b/run_heldout.py as round 2 reads it: the queue runner for the 172 held-out runs (../../HARNESS-PLAN-2.md P10-P17, P35).

No solver is run and no project is meshed anywhere here: every driver.launch, driver.check_solvers,
rooms.write_projects and rooms.mesh_projects call is monkeypatched to a recorder. Every folder a test
touches is under pytest's own tmp_path (C:\\tmp\\m8b-edt\\pytest-tmp, ../pyproject.toml), never under
B: and never under the real C:\\tmp\\m8b-edt\\heldout or progress.log. What is under test: the plan's
reordering, the resume/skip/partial rule, the STOP file, the workers cap, and that driver.launch (never
some shortcut around it) is what actually gets called.
"""
import io
import json
import threading
import time

from m8b import driver, rooms2, run_heldout as rh


DEV_RUN = dict(run_id='dev-run', room='G1', kind='tested', mode='random', time_step_s=0.001,
               particles_per_source=1000, random_seed=4242, duration_s=0.1)


def make_run(run_id, room='G1', kind='tested', mode='random'):
    return dict(DEV_RUN, run_id=run_id, room=room, kind=kind, mode=mode)


# ---- ordered_runs: T-equivalent for the queue runner's own order ---------------------------------------
def test_ordered_runs_is_truth_then_tested_random_then_tested_energetic():
    runs = rh.ordered_runs()
    assert len(runs) == 172
    kinds_modes = [(r['kind'], r.get('mode')) for r in runs]
    assert kinds_modes[:28] == [('truth', 'random')] * 28, 'the first 28 must all be truth runs'
    assert kinds_modes[28:100] == [('tested', 'random')] * 72, 'then all 72 tested Random runs'
    assert kinds_modes[100:172] == [('tested', 'energetic')] * 72, 'then all 72 tested Energetic runs'
    assert runs[0]['run_id'] == 'truth-G1-9101'
    assert runs[27]['kind'] == 'truth' and runs[28]['kind'] == 'tested' and runs[28]['mode'] == 'random'
    assert runs[99]['mode'] == 'random' and runs[100]['mode'] == 'energetic'
    assert runs[-1]['run_id'] == 'tested-G7-energetic-5.0ms-150k-3503'
    # no run dropped or duplicated by the filter-and-concatenate
    assert sorted(r['run_id'] for r in runs) == sorted(r['run_id'] for r in driver.plan())


def test_group_of_matches_ordered_runs_groups():
    runs = rh.ordered_runs()
    assert all(rh.group_of(r) == 'truth' for r in runs[:28])
    assert all(rh.group_of(r) == 'tested-random' for r in runs[28:100])
    assert all(rh.group_of(r) == 'tested-energetic' for r in runs[100:172])


def test_project_paths_are_under_rooms_root_never_under_data_root(tmp_path):
    rooms_root = tmp_path / 'rooms-root'
    paths = rh.project_paths(rooms_root)
    assert set(paths) == set(rooms2.NAMES)
    for name, p in paths.items():
        assert p == rooms_root / 'projects' / ('%s.simpa' % name)
        assert str(rooms_root) in str(p)


# ---- run_status / move_aside_partial -------------------------------------------------------------------
def write_complete_run(run_dir):
    run_dir.mkdir(parents=True)
    (run_dir / 'launch.json').write_text(json.dumps(dict(run_exit=0, results_exit=0)), encoding='utf-8')
    (run_dir / 'report.json').write_text(json.dumps(dict(spps={})), encoding='utf-8')


def test_run_status_missing_when_no_folder(tmp_path):
    assert rh.run_status(tmp_path / 'nope') == 'missing'


def test_run_status_partial_when_no_launch_json(tmp_path):
    d = tmp_path / 'half'
    d.mkdir()
    (d / 'project.simpa').write_text('{}', encoding='utf-8')
    assert rh.run_status(d) == 'partial'


def test_run_status_partial_when_launch_json_unreadable_or_run_failed(tmp_path):
    d = tmp_path / 'bad-json'
    d.mkdir()
    (d / 'launch.json').write_text('{not json', encoding='utf-8')
    assert rh.run_status(d) == 'partial'

    d2 = tmp_path / 'nonzero-exit'
    d2.mkdir()
    (d2 / 'launch.json').write_text(json.dumps(dict(run_exit=1, results_exit=None)), encoding='utf-8')
    assert rh.run_status(d2) == 'partial'

    d3 = tmp_path / 'no-report'
    d3.mkdir()
    (d3 / 'launch.json').write_text(json.dumps(dict(run_exit=0, results_exit=0)), encoding='utf-8')
    assert rh.run_status(d3) == 'partial'


def test_run_status_complete_when_launch_and_report_are_good(tmp_path):
    d = tmp_path / 'ok'
    write_complete_run(d)
    assert rh.run_status(d) == 'complete'


def test_move_aside_partial_renames_and_never_deletes(tmp_path):
    d = tmp_path / 'truth-F1-9001'
    d.mkdir()
    (d / 'marker.txt').write_text('half-written', encoding='utf-8')
    dest = rh.move_aside_partial(d)
    assert not d.exists(), 'the original path must be gone (renamed, not duplicated)'
    assert dest.exists() and dest.is_dir()
    assert dest.name.startswith('truth-F1-9001.partial-')
    assert (dest / 'marker.txt').read_text(encoding='utf-8') == 'half-written', 'contents must survive, nothing deleted'


# ---- do_run: skip / partial-then-relaunch / launch always used -----------------------------------------
def test_do_run_skips_a_complete_folder_without_calling_launch(tmp_path, monkeypatch):
    data_root = tmp_path / 'data'
    write_complete_run(data_root / DEV_RUN['run_id'])
    progress_log = tmp_path / 'progress.log'

    def must_not_be_called(*a, **k):
        raise AssertionError('driver.launch must not be called for a complete run folder')

    monkeypatch.setattr(driver, 'launch', must_not_be_called)
    res = rh.do_run(DEV_RUN, project=tmp_path / 'F1.simpa', data_root=data_root, solvers_dir=tmp_path,
                    launch_log=tmp_path / 'launch.log', simpa_exe=tmp_path / 'simpa.exe', progress_log=progress_log,
                    lock=threading.Lock())
    assert res == dict(run_id=DEV_RUN['run_id'], outcome='skip')
    assert 'skip' in progress_log.read_text(encoding='utf-8')


def test_do_run_moves_partial_aside_then_calls_launch(tmp_path, monkeypatch):
    data_root = tmp_path / 'data'
    run_dir = data_root / DEV_RUN['run_id']
    run_dir.mkdir(parents=True)
    (run_dir / 'simpa-run.stdout.json').write_text('{}', encoding='utf-8')          # no launch.json: partial
    progress_log = tmp_path / 'progress.log'
    calls = []

    def fake_launch(run, *, project, run_dir, solvers_dir, data_root, launch_log, simpa_exe, b1=None):
        calls.append(run_dir)
        return dict(run_exit=0, results_exit=0)

    monkeypatch.setattr(driver, 'launch', fake_launch)
    res = rh.do_run(DEV_RUN, project=tmp_path / 'F1.simpa', data_root=data_root, solvers_dir=tmp_path,
                    launch_log=tmp_path / 'launch.log', simpa_exe=tmp_path / 'simpa.exe', progress_log=progress_log,
                    lock=threading.Lock())
    assert res['outcome'] == 'done'
    assert len(calls) == 1 and calls[0] == run_dir, 'driver.launch must be called, with the run folder\'s own path'
    partials = [p for p in data_root.iterdir() if p.name.startswith(DEV_RUN['run_id'] + '.partial-')]
    assert len(partials) == 1, 'the half-written folder must be moved aside, not deleted'
    assert (partials[0] / 'simpa-run.stdout.json').is_file()
    text = progress_log.read_text(encoding='utf-8')
    assert 'partial moved to' in text and 'started' in text and 'done in' in text


def test_do_run_a_missing_folder_goes_straight_to_launch(tmp_path, monkeypatch):
    data_root = tmp_path / 'data'
    progress_log = tmp_path / 'progress.log'
    calls = []
    monkeypatch.setattr(driver, 'launch', lambda *a, **k: calls.append(1) or dict(run_exit=0, results_exit=0))
    res = rh.do_run(DEV_RUN, project=tmp_path / 'F1.simpa', data_root=data_root, solvers_dir=tmp_path,
                    launch_log=tmp_path / 'launch.log', simpa_exe=tmp_path / 'simpa.exe', progress_log=progress_log,
                    lock=threading.Lock())
    assert res['outcome'] == 'done' and len(calls) == 1
    assert not any(data_root.glob('*.partial-*')), 'nothing to move aside when the folder never existed'


def test_do_run_reports_failure_without_raising(tmp_path, monkeypatch):
    data_root = tmp_path / 'data'
    progress_log = tmp_path / 'progress.log'
    monkeypatch.setattr(driver, 'launch', lambda *a, **k: dict(run_exit=1, results_exit=None))
    res = rh.do_run(DEV_RUN, project=tmp_path / 'F1.simpa', data_root=data_root, solvers_dir=tmp_path,
                    launch_log=tmp_path / 'launch.log', simpa_exe=tmp_path / 'simpa.exe', progress_log=progress_log,
                    lock=threading.Lock())
    assert res['outcome'] == 'failed'
    assert 'FAILED' in progress_log.read_text(encoding='utf-8')

    def refuses(*a, **k):
        raise driver.Refused('low_disk_space', 'no room')

    monkeypatch.setattr(driver, 'launch', refuses)
    res2 = rh.do_run(DEV_RUN, project=tmp_path / 'F1.simpa', data_root=data_root, solvers_dir=tmp_path,
                     launch_log=tmp_path / 'launch.log', simpa_exe=tmp_path / 'simpa.exe', progress_log=progress_log,
                     lock=threading.Lock())
    assert res2['outcome'] == 'failed' and 'low_disk_space' in res2['reason']


# ---- run_queue: workers cap, STOP, order, launch always used -------------------------------------------
def test_run_queue_respects_the_workers_cap(tmp_path, monkeypatch):
    runs = [make_run('r%d' % i) for i in range(10)]
    data_root = tmp_path / 'data'
    progress_log = tmp_path / 'progress.log'
    stop_file = tmp_path / 'STOP'
    state = dict(active=0, peak=0)
    lock = threading.Lock()
    calls = []

    def fake_launch(run, *, project, run_dir, solvers_dir, data_root, launch_log, simpa_exe, b1=None):
        with lock:
            state['active'] += 1
            state['peak'] = max(state['peak'], state['active'])
            calls.append(run['run_id'])
        time.sleep(0.05)
        with lock:
            state['active'] -= 1
        return dict(run_exit=0, results_exit=0)

    monkeypatch.setattr(driver, 'launch', fake_launch)
    projects = {r['room']: tmp_path / ('%s.simpa' % r['room']) for r in runs}
    results = rh.run_queue(runs, projects=projects, data_root=data_root, solvers_dir=tmp_path,
                           launch_log=tmp_path / 'launch.log', simpa_exe=tmp_path / 'simpa.exe',
                           progress_log=progress_log, workers=3, stop_file=stop_file)
    assert state['peak'] <= 3, 'never more than --workers runs active at once'
    assert len(results) == 10 and all(r['outcome'] == 'done' for r in results)
    assert sorted(calls) == sorted(r['run_id'] for r in runs), 'driver.launch must be used for every run'


def test_run_queue_honours_the_stop_file(tmp_path, monkeypatch):
    runs = [make_run('s%d' % i) for i in range(5)]
    data_root = tmp_path / 'data'
    progress_log = tmp_path / 'progress.log'
    stop_file = tmp_path / 'STOP'
    calls = []

    def fake_launch(run, *, project, run_dir, solvers_dir, data_root, launch_log, simpa_exe, b1=None):
        calls.append(run['run_id'])
        if len(calls) == 2:
            stop_file.write_text('stop', encoding='utf-8')
        return dict(run_exit=0, results_exit=0)

    monkeypatch.setattr(driver, 'launch', fake_launch)
    projects = {r['room']: tmp_path / ('%s.simpa' % r['room']) for r in runs}
    results = rh.run_queue(runs, projects=projects, data_root=data_root, solvers_dir=tmp_path,
                           launch_log=tmp_path / 'launch.log', simpa_exe=tmp_path / 'simpa.exe',
                           progress_log=progress_log, workers=1, stop_file=stop_file)
    assert calls == ['s0', 's1'], 'no run may start once the STOP file exists'
    assert len(results) == 2 and all(r['outcome'] == 'done' for r in results)
    text = progress_log.read_text(encoding='utf-8')
    assert 'STOP file present, 3 run(s) not started' in text


def test_run_queue_processes_pending_in_order_when_serial(tmp_path, monkeypatch):
    runs = [make_run('o%d' % i) for i in range(6)]
    data_root = tmp_path / 'data'
    progress_log = tmp_path / 'progress.log'
    stop_file = tmp_path / 'STOP'
    calls = []

    monkeypatch.setattr(driver, 'launch',
                        lambda run, **k: calls.append(run['run_id']) or dict(run_exit=0, results_exit=0))
    projects = {r['room']: tmp_path / ('%s.simpa' % r['room']) for r in runs}
    rh.run_queue(runs, projects=projects, data_root=data_root, solvers_dir=tmp_path, launch_log=tmp_path / 'launch.log',
                simpa_exe=tmp_path / 'simpa.exe', progress_log=progress_log, workers=1, stop_file=stop_file)
    assert calls == ['o%d' % i for i in range(6)], 'workers=1 must launch pending runs in the given order'


def test_run_queue_writes_a_summary_line(tmp_path, monkeypatch):
    """write_summary never blocks the queue on a bad counter read (CPU/disk helpers return None)."""
    runs = [make_run('sum0')]
    data_root = tmp_path / 'data'
    progress_log = tmp_path / 'progress.log'
    stop_file = tmp_path / 'STOP'
    monkeypatch.setattr(rh, '_cpu_performance_pct', lambda timeout=5: None)
    monkeypatch.setattr(rh, '_free_gb', lambda drive: None)
    monkeypatch.setattr(driver, 'launch', lambda run, **k: dict(run_exit=0, results_exit=0))
    projects = {r['room']: tmp_path / ('%s.simpa' % r['room']) for r in runs}
    rh.run_queue(runs, projects=projects, data_root=data_root, solvers_dir=tmp_path, launch_log=tmp_path / 'launch.log',
                simpa_exe=tmp_path / 'simpa.exe', progress_log=progress_log, workers=1, stop_file=stop_file)
    text = progress_log.read_text(encoding='utf-8')
    assert 'runner summary:' in text and 'n/a' in text


# ---- ensure_rooms: build-once, idempotent, never on B: --------------------------------------------------
def test_ensure_rooms_skips_rebuild_when_the_record_already_verifies(tmp_path, monkeypatch):
    rooms_root = tmp_path / 'rooms-root'
    rooms_root.mkdir()
    projects = {n: str(rooms_root / 'projects' / ('%s.simpa' % n)) for n in rooms2.NAMES}
    mesh = {n: dict(mesh_exit=0, verify_exit=0) for n in rooms2.NAMES}
    (rooms_root / 'rooms.json').write_text(json.dumps(dict(projects=projects, mesh=mesh)), encoding='utf-8')

    def must_not_be_called(*a, **k):
        raise AssertionError('a verified record must not trigger a rebuild')

    monkeypatch.setattr(driver, 'check_solvers', must_not_be_called)
    monkeypatch.setattr(rooms2, 'write_projects', must_not_be_called)
    monkeypatch.setattr(rooms2, 'mesh_projects', must_not_be_called)
    got = rh.ensure_rooms(rooms_root, tmp_path / 'simpa.exe', tmp_path / 'solvers')
    assert {n: str(p) for n, p in got.items()} == projects


def test_ensure_rooms_builds_and_meshes_once_when_no_record(tmp_path, monkeypatch):
    rooms_root = tmp_path / 'rooms-root'
    write_calls, mesh_calls = [], []

    def fake_write_projects(out_dir, simpa_exe, names=None):
        write_calls.append((out_dir, simpa_exe, tuple(names)))
        return {n: out_dir / ('%s.simpa' % n) for n in names}

    def fake_mesh_projects(written, out_dir, simpa_exe, solvers_dir):
        mesh_calls.append((set(written), out_dir, simpa_exe, solvers_dir))
        return {n: dict(mesh_exit=0, verify_exit=0, tetrahedra=1) for n in written}

    monkeypatch.setattr(driver, 'check_solvers', lambda solvers_dir: dict(verified=True, checks=[]))
    monkeypatch.setattr(rooms2, 'write_projects', fake_write_projects)
    monkeypatch.setattr(rooms2, 'mesh_projects', fake_mesh_projects)
    got = rh.ensure_rooms(rooms_root, tmp_path / 'simpa.exe', tmp_path / 'solvers')
    assert set(got) == set(rooms2.NAMES)
    assert len(write_calls) == 1 and set(write_calls[0][2]) == set(rooms2.NAMES)
    assert len(mesh_calls) == 1
    record = json.loads((rooms_root / 'rooms.json').read_text(encoding='utf-8'))
    assert set(record['mesh']) == set(rooms2.NAMES)

    # a second call reads the record back and rebuilds nothing
    got2 = rh.ensure_rooms(rooms_root, tmp_path / 'simpa.exe', tmp_path / 'solvers')
    assert len(write_calls) == 1 and len(mesh_calls) == 1, 'a second call must not rebuild'
    assert {n: str(p) for n, p in got2.items()} == {n: str(p) for n, p in got.items()}


def test_ensure_rooms_raises_and_writes_no_record_when_mesh_fails(tmp_path, monkeypatch):
    rooms_root = tmp_path / 'rooms-root'
    monkeypatch.setattr(driver, 'check_solvers', lambda solvers_dir: dict(verified=True, checks=[]))
    monkeypatch.setattr(rooms2, 'write_projects',
                        lambda out_dir, simpa_exe, names=None: {n: out_dir / ('%s.simpa' % n) for n in names})
    monkeypatch.setattr(rooms2, 'mesh_projects', lambda written, out_dir, simpa_exe, solvers_dir:
                        {n: dict(mesh_exit=0, verify_exit=(1 if n == 'G3' else 0)) for n in written})
    try:
        rh.ensure_rooms(rooms_root, tmp_path / 'simpa.exe', tmp_path / 'solvers')
        raise AssertionError('G3 failing mesh-verify must raise')
    except RuntimeError as e:
        assert 'G3' in str(e)
    assert not (rooms_root / 'rooms.json').exists(), 'a failed build must not be recorded as done'


def test_ensure_rooms_raises_when_solvers_unverified(tmp_path, monkeypatch):
    rooms_root = tmp_path / 'rooms-root'
    monkeypatch.setattr(driver, 'check_solvers', lambda solvers_dir: dict(verified=False, checks=[]))

    def must_not_be_called(*a, **k):
        raise AssertionError('an unverified solver folder must stop before any project is written')

    monkeypatch.setattr(rooms2, 'write_projects', must_not_be_called)
    try:
        rh.ensure_rooms(rooms_root, tmp_path / 'simpa.exe', tmp_path / 'solvers')
        raise AssertionError('unverified solvers must raise')
    except RuntimeError:
        pass


# ---- print_dry_run: lists, touches nothing ---------------------------------------------------------------
def test_print_dry_run_lists_projects_and_runs_without_touching_disk(tmp_path):
    runs = rh.ordered_runs()
    data_root = tmp_path / 'data'           # deliberately not created
    rooms_root = tmp_path / 'rooms-root'    # deliberately not created
    projects = rh.project_paths(rooms_root)
    buf = io.StringIO()
    rh.print_dry_run(runs, projects, data_root, rooms_root, out=buf)
    text = buf.getvalue()
    assert not data_root.exists() and not rooms_root.exists(), 'a dry run must create nothing'
    assert 'Runs (172): 28 truth, 144 tested (72 random, 72 energetic)' in text
    assert 'first run: truth-G1-9101' in text
    assert 'last run:  tested-G7-energetic-5.0ms-150k-3503' in text
    for name in rooms2.NAMES:
        assert ('%s -> %s' % (name, projects[name])) in text
