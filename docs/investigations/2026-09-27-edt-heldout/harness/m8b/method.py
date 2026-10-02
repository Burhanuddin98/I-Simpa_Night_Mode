"""The frozen method, loaded only through its sha256 gate (PREREG.md:8-11; HARNESS-PLAN.md P32).

Section 1's gate: every instance of the method the harness runs comes from load(). The file's
bytes are read once, as they are on disk (no line-end change: PREREG's hash is of the CRLF
checkout), and hashed. Only when the hash is PREREG's are those same bytes compiled and executed,
into a fresh module that is never put in sys.modules, so nothing re-reads the file between the
check and the execution and no two instances share a namespace. The shape is critique/common.py:6-7's
loader (one module per load, by name and path) with the check in front; no line of it is copied,
so provenance.json has no entry for this file.

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

`checked_path` is the absolute path read, a pathlib.Path. A `z` that is not a finite positive
number is refused with ValueError before the file is read. A file that cannot be read raises the
OS error unchanged: there is no hash to record, and the run stops before anything is computed.
"""
import math
from pathlib import Path

from .corpus import FROZEN, FROZEN_SHA256, VoidRun, exec_module, sha256_bytes  # one VoidRun for the harness

Z_SECONDARY = 3.0     # PREREG.md:11, P32: reported beside frozen/method.py:15's Z = 2.0; decides nothing


def load(path=None, z=None):
    if z is not None:
        z = float(z)
        if not (math.isfinite(z) and z > 0.0):
            raise ValueError('Z must be a finite positive number, not %r' % (z,))
    p = Path(FROZEN if path is None else path).resolve()
    data = p.read_bytes()
    h = sha256_bytes(data)
    if h != FROZEN_SHA256:
        e = VoidRun('%s has sha256 %s, not %s: the run is void (PREREG.md:10) and nothing was executed. '
                    'A checkout with other line ends than the CRLF form PREREG pins changes the bytes.'
                    % (p, h, FROZEN_SHA256))
        e.checked_sha256 = h
        e.checked_path = p
        raise e
    mod = exec_module('m8b_frozen_method' if z is None else 'm8b_frozen_method_z%g' % z, p, data)
    mod.checked_sha256 = h
    mod.checked_path = p
    if z is not None:
        mod.Z = z     # after import, on this instance's globals only: analyse() reads it from there
    return mod


def load_z3(path=None):
    return load(path, z=Z_SECONDARY)
