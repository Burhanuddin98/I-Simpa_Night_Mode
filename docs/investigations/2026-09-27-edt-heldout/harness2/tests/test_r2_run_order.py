"""T43 (../../HARNESS-PLAN-2.md section 7): the run order is enforced. The scorer refuses without B1 committed, with a
missing or partial run folder, with a pin that does not match, and on round 1's roots; the attack refuses without
SENTINEL.md (T40). Nothing is scored here and nothing is written outside tmp_path.
"""
import json
from pathlib import Path

import pytest

from m8b import corpus2, driver, score_heldout
from test_r2_plan_guards import refused


def complete(d):
    d.mkdir(parents=True)
    (d / 'launch.json').write_text(json.dumps(dict(run_exit=0, results_exit=0)), encoding='utf-8')
    (d / 'report.json').write_text('{}', encoding='utf-8')


def test_t43_scorer_run_order(tmp_path, monkeypatch):
    data, out = tmp_path / 'data', tmp_path / 'out'
    monkeypatch.setattr(driver, 'b1_committed', lambda repo=None: False)
    refused('heldout_before_b1', score_heldout.preflight, data_root=data, out=out, pin=False)
    monkeypatch.setattr(driver, 'b1_committed', lambda repo=None: True)
    # round 1's roots and results are never scored
    for old in (r'B:\data\m8b-edt\heldout', r'C:\tmp\m8b-edt\heldout'):
        refused('round1_root', score_heldout.preflight, data_root=Path(old), out=out, pin=False)
    refused('round1_root', score_heldout.preflight, data_root=data, out=Path(r'B:\data\m8b-edt\results\run1'), pin=False)
    # missing: nothing there
    refused('run_folders_incomplete', score_heldout.preflight, data_root=data, out=out, pin=False)
    runs = driver.plan()
    for r in runs[:-1]:
        complete(data / r['run_id'])
    e = refused('run_folders_incomplete', score_heldout.preflight, data_root=data, out=out, pin=False)
    assert runs[-1]['run_id'] in str(e)
    # partial: a folder with no report
    (data / runs[-1]['run_id']).mkdir()
    (data / runs[-1]['run_id'] / 'launch.json').write_text(json.dumps(dict(run_exit=0, results_exit=None)), encoding='utf-8')
    refused('run_folders_incomplete', score_heldout.preflight, data_root=data, out=out, pin=False)
    # all complete: the folder check passes
    (data / runs[-1]['run_id'] / 'launch.json').write_text(json.dumps(dict(run_exit=0, results_exit=0)), encoding='utf-8')
    (data / runs[-1]['run_id'] / 'report.json').write_text('{}', encoding='utf-8')
    assert score_heldout.preflight(data_root=data, out=out, pin=False) is None
    # the pin: a fresh draw that differs from preview_pin.json stops the run
    monkeypatch.setattr(corpus2, 'check_pin', lambda: ['ism_fresh_2'])
    refused('pin_mismatch', score_heldout.preflight, data_root=data, out=out, pin=True)
    monkeypatch.setattr(corpus2, 'check_pin', lambda: [])
    assert score_heldout.preflight(data_root=data, out=out, pin=True) is None
    # the command line goes through the same door
    monkeypatch.setattr(driver, 'b1_committed', lambda repo=None: False)
    with pytest.raises(SystemExit):
        score_heldout.main(['--out', str(tmp_path / 'o2'), '--data-root', str(data)])
    assert not (tmp_path / 'o2').exists(), 'a refused scorer wrote its output folder'
