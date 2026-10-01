"""T41 (../../HARNESS-PLAN-2.md section 7, P34): ISM scoring logs when a room STARTS, with its image count, and after
each receiver. Round 1 logged only when a room finished: first line at 20:05 for a job started at 19:13.
Fake workers only: nothing here builds an echogram.
"""
import numpy as np

from m8b import ism_fresh, score_heldout


def room(rid, images):
    return dict(id=rid, kind='drawn', parent=None, dims_m=[10.0, 8.0, 4.0], image_time_s=2.4, images=images,
                receivers=[dict(position_m=[1.0 + i, 2.0, 1.5], R_m=0.31, d_m=1.0 + i, **{'class': 'mid'}) for i in range(12)])


def test_t41_ism_rooms_log_start_before_done_and_one_line_per_receiver(tmp_path, monkeypatch):
    rooms = [room('rel:t1', 12345), room('drawn:1', 67890), room('drawn:2', 24680)]
    D = dict(seed=1, rooms=rooms, rejections={}, corpus_rooms_sha256='x')

    def fake_rows(Droom, on_receiver=None):
        n = len(Droom['rooms'][0]['receivers'])
        for k in range(1, n + 1):
            on_receiver(k, n, 0.25 * k)
        return [dict(id='%s-%d' % (Droom['rooms'][0]['id'], i)) for i in range(3)]

    monkeypatch.setattr(ism_fresh, 'rows_for_scoring', fake_rows)
    monkeypatch.setattr(score_heldout, '_memoise_echogram', lambda: None)
    log = score_heldout.Log(tmp_path / 'p.log')
    rows = score_heldout.build_ism(D, log, workers=1)
    assert len(rows) == 9
    lines = (tmp_path / 'p.log').read_text(encoding='utf-8').splitlines()
    starts = [i for i, l in enumerate(lines) if 'ism room' in l and ' start' in l]
    dones = [i for i, l in enumerate(lines) if 'ism room' in l and ' done' in l]
    assert len(starts) == 3 and len(dones) == 3 and max(starts) < min(dones), lines
    for r in rooms:
        s = [l for l in lines if ('ism room %s start' % r['id']) in l]
        assert len(s) == 1 and str(r['images']) in s[0] and 'pid' in s[0], (r['id'], s)
        recv = [l for l in lines if ('ism room %s receiver' % r['id']) in l]
        assert len(recv) == 12 and 'receiver 1/12' in recv[0] and 'receiver 12/12' in recv[-1]


def test_t41_rows_for_scoring_calls_back_once_per_receiver(monkeypatch):
    def fake(room_, rec, R, F, step_ms, image_time_s=None, run_s=None, retry_truncated=False):
        return dict(bins=np.array([1.0, 0.0]), dt=step_ms * 1e-3, t_arrival=0.01, half_width=R / 343.2, truth_edt=1.0,
                    truth_share=0.0, truth_status='ok')

    monkeypatch.setattr(ism_fresh, 'make_row', fake)
    monkeypatch.setattr(ism_fresh, 'BANDS_HZ', (125, 500))
    monkeypatch.setattr(ism_fresh, 'STEPS_MS', (1.0, 2.0))
    calls = []
    r = room('drawn:1', 1)
    r['design_t60_s'] = {125: 1.0, 500: 1.0}
    r['source_m'] = [0.5, 0.5, 0.5]
    r['receivers'] = r['receivers'][:3]
    rows = ism_fresh.rows_for_scoring(dict(seed=1, rooms=[r], rejections={}, corpus_rooms_sha256='x'),
                                      on_receiver=lambda k, n, secs: calls.append((k, n, secs >= 0)))
    assert len(rows) == 12 and calls == [(1, 3, True), (2, 3, True), (3, 3, True)]
