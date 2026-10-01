"""The frozen method, loaded only through its sha256 gate (PREREG.md:8-11; HARNESS-PLAN.md P32).

STUB (HARNESS-PLAN.md section 6, step 3): every function returns None. Tests T2 and T3 hold the
contract below; T1 (the file's hash) is a control and needs no code here.

Contract:
- load(path=None, z=None): a fresh module instance of frozen/method.py (or of `path`), executed
  from the very bytes whose sha256 was checked against FROZEN_SHA256 (critique/common.py:6-7 is
  the loader's shape). The instance carries `checked_sha256` (the hash it checked) and
  `checked_path`. On a mismatch it raises VoidRun with the same two attributes set, and nothing is
  executed. `z`, when given, sets Z on this instance only, after import (P32,
  final/dev/z3_sensitivity.py:5-6): the file is never changed, and every other instance keeps
  Z = 2.0.
- load_z3(path=None): the Z = 3 instance, load(path, z=3.0). It informs Burhan's call and decides
  nothing (PREREG.md:11).
"""
from .corpus import FROZEN, FROZEN_SHA256, VoidRun  # noqa: F401  (one VoidRun for the harness)


def load(path=None, z=None):
    return None


def load_z3(path=None):
    return None
