"""Upstream's EDT on the same rows (PREREG.md:12; HARNESS-PLAN.md P32): the validated float32 port
target/agents/upstream-edt/uphunt_upstream_edt.py (sha256 57f391e6..., not the docs copy
1d12623f...), copied by text into COPY, with the old evaluator's wrapper.

COPY is the port byte for byte (10,777 bytes, 247 lines, LF). Nothing of it runs until port() has
read it, made any CRLF LF and found that those bytes hash to UPSTREAM_SHA256; the same bytes are
then compiled and executed into a module that is never put in sys.modules. The module is made once
per path and process and handed back on later calls, but only after each call has checked the file
again, so a copy changed on disk voids the next call. Its only state is the port's own time-table
cache (_TT), whose entries depend only on (dt, i), so sharing it changes no value. edt_table
re-types uphunt_oracle.py's 'EDT (s)' pass and analyse the old evaluator's analyse_upstream;
provenance.json lists the copy and both.

Contract:
- COPY: m8b/_upstream_edt.py, the port's text. Its source is LF throughout, as this folder
  keeps (../.gitattributes), so the copy's bytes, with any CRLF made LF, hash to
  UPSTREAM_SHA256. provenance.json records the source and its hashes.
- port(path=None): the copy (or `path`) as a module, executed only after that hash is checked
  (VoidRun otherwise, with checked_sha256 and checked_path set and nothing executed). The module
  carries the same two attributes.
- edt_table(recp_path): [(row label, numpy.float32 EDT in s)] for every band of a 'Sound level.recp',
  then 'Global' and 'Average', as target/agents/upstream-edt/uphunt_oracle.py:45-60 computes the
  'EDT (s)' column: compute_tr_param_table(0, 10, tt, tab), tt from the file's own time labels
  (time_table_from_labels), tab its float band columns. Row labels as the 'Acoustic
  parameters.gabe' first column names them: each band column's own label, then 'Global' and
  'Average' (upstream's projet_calculation.cpp:888, 901-902).
- analyse(bins, dt, t_arrival, meta=None): the old evaluator's wrapper (the scratchpad's
  edtsimp/wrappers.py:42-62, sha256 745910f2...; EVAL.md:9): the port's point-receiver EDT of
  float32(bins) at float(dt). status 'ok' with edt_lo == edt_hi == edt when that value is finite
  and positive; otherwise 'refused' with reason 'nan_or_nonpositive' and edt, edt_lo, edt_hi None
  ('empty_or_bad_dt' for an empty series or dt <= 0). t_arrival and meta are not used: upstream
  has no arrival and no range.

An 'ok' row's reason is 'point_value_no_range', as the old wrapper's was. The old wrapper's
defensive except (wrappers.py:55-58), which made an exception from the port a refusal, is left out:
it never fired on the corpus (0 of the 5,897 upstream rows in the scratchpad's results_*.pkl), and
here an input the port cannot read, a 2-D array say, stops the evaluation rather than counting as
one of upstream's refusals. analyse checks the copy before it looks at the input, so a bad copy
raises VoidRun on every call.
"""
import math
from pathlib import Path

import numpy as np

from .corpus import VoidRun, exec_module, sha256_bytes  # one VoidRun for the harness

HERE = Path(__file__).resolve().parent
COPY = HERE / '_upstream_edt.py'
UPSTREAM_SOURCE = 'target/agents/upstream-edt/uphunt_upstream_edt.py'
UPSTREAM_SHA256 = '57f391e6cb09ded8a408462fbabc7aacd93743a3fa46cc28deeef3f7a319f282'
# The rows upstream's GUI writes after the bands (projet_calculation.cpp:901-902, tag v1.4.0_snapshot_14_01_2026).
AFTER_BANDS = ('Global', 'Average')

_LOADED = {}        # resolved path -> the module executed from that path's checked bytes


def port(path=None):
    p = Path(COPY if path is None else path).resolve()
    data = p.read_bytes().replace(b'\r\n', b'\n')
    h = sha256_bytes(data)
    if h != UPSTREAM_SHA256:
        e = VoidRun('%s has sha256 %s with its line ends made LF, not %s: it is not the validated port '
                    '(P32), and nothing of it was executed' % (p, h, UPSTREAM_SHA256))
        e.checked_sha256 = h
        e.checked_path = p
        raise e
    mod = _LOADED.get(p)
    if mod is None:
        mod = exec_module('m8b_upstream_edt', p, data)
        mod.checked_sha256 = h
        mod.checked_path = p
        _LOADED[p] = mod
    return mod


def edt_table(recp_path):
    up = port()
    cols = up.read_gabe(str(recp_path))['cols']
    if not cols or cols[0]['type'] != 'str':
        raise ValueError('%s: column 0 is not the time labels' % recp_path)
    tt = up.time_table_from_labels(cols[0]['data'])
    bands = [c for c in cols[1:] if c['type'] == 'float']
    if not bands or any(len(c['data']) != len(tt) for c in bands):
        raise ValueError('%s: no float band column, or one whose length differs from the %d time labels'
                         % (recp_path, len(tt)))
    vals, _ = up.compute_tr_param_table(0, 10, tt, [c['data'] for c in bands])
    labels = [c['label'] for c in bands] + list(AFTER_BANDS)
    if len(vals) != len(labels):
        raise ValueError('%d values for %d rows' % (len(vals), len(labels)))
    return [(label, np.float32(v)) for label, v in zip(labels, vals)]


def _refuse(why):
    return dict(edt=None, edt_lo=None, edt_hi=None, status='refused', reason=why)


def analyse(bins, dt, t_arrival, meta=None):
    del t_arrival, meta                     # upstream has no arrival and no range
    up = port()
    v = np.asarray(bins, dtype=np.float64)
    if v.size == 0 or dt <= 0:
        return _refuse('empty_or_bad_dt')
    val, _ = up.edt(v.astype(np.float32), float(dt), kind='point')
    if val is None or not math.isfinite(val) or val <= 0:
        return _refuse('nan_or_nonpositive')
    return dict(edt=float(val), edt_lo=float(val), edt_hi=float(val), status='ok', reason='point_value_no_range')
