"""Upstream's EDT on the same rows (PREREG.md:12; HARNESS-PLAN.md P32): the validated float32 port
target/agents/upstream-edt/uphunt_upstream_edt.py (sha256 57f391e6..., not the docs copy
1d12623f...), copied by text into COPY, with the old evaluator's wrapper.

STUB (HARNESS-PLAN.md section 6, step 3): COPY does not exist yet and every function returns
None. Tests T4 and T5 hold the contract below.

Contract:
- COPY: m8b/_upstream_edt.py, the port's text. Its source is CRLF throughout and this folder
  keeps LF (../.gitattributes), so the copy's bytes with every LF made CRLF hash to
  UPSTREAM_SHA256. provenance.json records the source and its hashes.
- port(): the copy as a module, executed only after that hash is checked (VoidRun otherwise).
- edt_table(recp_path): [(row label, numpy.float32 EDT in s)] for every band of a 'Sound level.recp',
  then 'Global' and 'Average', as target/agents/upstream-edt/uphunt_oracle.py:45-60 computes the
  'EDT (s)' column: compute_tr_param_table(0, 10, tt, tab), tt from the file's own time labels
  (time_table_from_labels), tab its float band columns. Row labels as the 'Acoustic
  parameters.gabe' first column names them.
- analyse(bins, dt, t_arrival, meta=None): the old evaluator's wrapper (the scratchpad's
  edtsimp/wrappers.py:42-62, sha256 745910f2...; EVAL.md:9): the port's point-receiver EDT of
  float32(bins) at float(dt). status 'ok' with edt_lo == edt_hi == edt when that value is finite
  and positive; otherwise 'refused' with reason 'nan_or_nonpositive' and edt, edt_lo, edt_hi None
  ('empty_or_bad_dt' for an empty series or dt <= 0). t_arrival and meta are not used: upstream
  has no arrival and no range.
"""
from pathlib import Path

HERE = Path(__file__).resolve().parent
COPY = HERE / '_upstream_edt.py'
UPSTREAM_SOURCE = 'target/agents/upstream-edt/uphunt_upstream_edt.py'
UPSTREAM_SHA256 = '57f391e6cb09ded8a408462fbabc7aacd93743a3fa46cc28deeef3f7a319f282'


def port():
    return None


def edt_table(recp_path):
    return None


def analyse(bins, dt, t_arrival, meta=None):
    return None
