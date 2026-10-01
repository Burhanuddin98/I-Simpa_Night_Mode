"""Step-8 review (round 1's C:\\tmp\\m8b-edt\\step8-review.md, Findings), as round 2 reads it: two MINORs on driver.launch()'s
guards, fixed here. The allowlist is the G rooms plus the probes; an F room is an unknown room in round 2.

MINOR 1 -- the F-room guard was keyed to `room in rooms.NAMES`, so its safety rested on "nothing
calls launch() with a room outside both rooms.NAMES and the probe set" rather than on a check that
would catch a future caller doing so. The fix is driver.ROOM_ALLOWLIST (rooms.NAMES + rooms.PROBES)
and an explicit 'unknown_room' refusal for anything else.

MINOR 2 -- section 5 item 5's "C: keeps >= 8 GB free" was checked by hand (PROBE.md) but had no
automated guard inside launch(). The fix is driver.check_free_space / driver.MIN_FREE_BYTES, called
before anything is written.

No test here can launch a solver: the unknown-room and low-disk-space tests are refused before
check_solvers would even matter, and the "normal path" test monkeypatches subprocess.run itself, so
no process is started. Nothing is written under C:\\tmp\\m8b-edt\\heldout.
"""
import json

from m8b import driver, rooms, rooms2

DEV_SEED = 4242                   # not reserved, not held out (P12) -- test_driver.py's own control seed


def dev_run(room='G1', seed=DEV_SEED, run_id='guard-test'):
    return dict(run_id=run_id, room=room, kind='dev', mode='random', time_step_s=0.001,
                particles_per_source=1000, random_seed=seed, duration_s=0.1)


def refused(code, fn, *a, **k):
    try:
        got = fn(*a, **k)
    except driver.Refused as e:
        assert e.code == code, 'refused %r, expected %r' % (e.code, code)
        return e
    raise AssertionError('%s was not refused (%s expected); it returned %r' % (fn.__name__, code, got))


def test_room_allowlist_covers_exactly_the_g_rooms_and_probes():
    """driver.ROOM_ALLOWLIST is rooms2.NAMES (the G rooms) plus rooms.PROBES (P0, P0b), and nothing
    else -- the set the step-8 review's suggested fix names. No F room is in it."""
    assert driver.ROOM_ALLOWLIST == set(rooms2.NAMES) | set(rooms.PROBES)
    assert 'F99' not in driver.ROOM_ALLOWLIST and 'p0' not in driver.ROOM_ALLOWLIST
    assert not driver.ROOM_ALLOWLIST & set(rooms.NAMES)


def test_unknown_room_is_refused(tmp_path, solvers_dir):
    """MINOR 1: a room that is neither an F-room nor a probe room is refused before anything is
    written, whatever its seed -- independent of require_not_heldout and of A1."""
    root = tmp_path / 'data'
    root.mkdir()
    log = tmp_path / 'launch.log'
    nothing = dict(project=tmp_path / 'no-project.simpa', simpa_exe=tmp_path / 'no-simpa.exe', launch_log=log)
    for bad_room in ('F99', 'p0', 'nope', '', 'F1', 'F3'):
        refused('unknown_room', driver.launch, dev_run(room=bad_room),
                run_dir=root / ('r-%s' % (bad_room or 'empty')), solvers_dir=solvers_dir, data_root=root,
                b1=True, **nothing)
    assert not log.exists() or log.read_text(encoding='utf-8') == '', 'an unknown-room launch wrote the launch log'
    assert sorted(root.iterdir()) == [], 'an unknown-room launch wrote under the data root'


def test_low_free_space_is_refused(tmp_path, solvers_dir, monkeypatch):
    """MINOR 2: launch() refuses before writing anything when the drive holding the run folder, or
    C:, is under the MIN_FREE_BYTES floor. Both branches are exercised separately. Disk space is
    mocked throughout -- no real disk is touched or depended on."""
    root = tmp_path / 'data'
    root.mkdir()
    log = tmp_path / 'launch.log'
    nothing = dict(project=tmp_path / 'no-project.simpa', simpa_exe=tmp_path / 'no-simpa.exe', launch_log=log)
    run_dir = root / 'low-space'
    LOW, HIGH = 1 * 1024 ** 3, 100 * 1024 ** 3

    class FakeUsage:
        def __init__(self, free):
            self.total = self.used = free
            self.free = free

    # the run folder's own drive is short; C: is fine
    monkeypatch.setattr(driver, '_drive_root', lambda path: 'Z:\\')
    monkeypatch.setattr(driver.shutil, 'disk_usage', lambda path: FakeUsage(LOW if path == 'Z:\\' else HIGH))
    refused('low_disk_space', driver.launch, dev_run(), run_dir=run_dir, solvers_dir=solvers_dir, data_root=root,
            b1=True, **nothing)
    assert not run_dir.exists(), 'a low-disk-space launch created the run folder'
    monkeypatch.undo()

    # the run folder's drive is fine; C: is short
    monkeypatch.setattr(driver, '_drive_root', lambda path: 'Z:\\')
    monkeypatch.setattr(driver.shutil, 'disk_usage', lambda path: FakeUsage(LOW if path == 'C:\\' else HIGH))
    refused('low_disk_space', driver.launch, dev_run(), run_dir=run_dir, solvers_dir=solvers_dir, data_root=root,
            b1=True, **nothing)
    assert not run_dir.exists(), 'a low-disk-space launch created the run folder'
    monkeypatch.undo()

    assert not log.exists() or log.read_text(encoding='utf-8') == '', 'a low-disk-space launch wrote the launch log'


def test_normal_path_reaches_the_launch_call(tmp_path, solvers_dir, monkeypatch):
    """The guards do not block a legitimate run: with a real allowlisted room, a verified solver
    folder, B1 taken as committed and real (ample) disk space, launch() gets all the way to
    subprocess.run. subprocess.run itself is monkeypatched here, so no solver is actually started."""
    root = tmp_path / 'data'
    root.mkdir()
    run_dir = root / 'ok'
    project = tmp_path / 'G1.simpa'
    project.write_text(json.dumps({'name': 'G1', 'solvers': {'spps': {}}}), encoding='utf-8')
    log = tmp_path / 'launch.log'
    simpa_exe = tmp_path / 'simpa.exe'

    calls = []

    class FakeCompleted:
        returncode = 0

    def fake_run(cmd, **kwargs):
        calls.append((cmd, kwargs))
        return FakeCompleted()

    monkeypatch.setattr(driver.subprocess, 'run', fake_run)

    rec = driver.launch(dev_run(room='G1'), project=project, run_dir=run_dir, solvers_dir=solvers_dir,
                        data_root=root, launch_log=log, simpa_exe=simpa_exe, b1=True)

    assert len(calls) == 1, 'launch() did not reach subprocess.run exactly once'
    cmd, kwargs = calls[0]
    assert cmd[0] == str(simpa_exe)
    assert cmd[1] == 'run' and cmd[2] == str(run_dir / 'project.simpa')
    assert rec['run']['room'] == 'G1'
    assert run_dir.is_dir(), 'the guards passed but the run folder was never created'
    assert (run_dir / 'project.simpa').is_file()
    assert log.read_text(encoding='utf-8').strip() != '', 'a launch that reached the solver did not log'
