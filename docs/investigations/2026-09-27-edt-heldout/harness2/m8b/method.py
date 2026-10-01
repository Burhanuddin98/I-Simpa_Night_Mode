"""Round 2's method, loaded only through its sha256 gate (PREREG-2.md, amendment 1; HARNESS-PLAN-2.md section 4).

Every instance of the method the harness runs comes from load(). The file's bytes are read once, as they are on
disk (no line-end change: frozen2/** is held at LF by ../../.gitattributes, so the hash is of the committed text
on every checkout), and hashed. Only when the hash is the pin's are those same bytes compiled and executed, into
a fresh module that is never put in sys.modules, so nothing re-reads the file between the check and the
execution and no two instances share a namespace. The shape is critique/common.py:6-7's loader (one module per
load, by name and path) with the check in front; no line of it is copied, so provenance.json has no entry for it.

Contract:
- load(path=None, *, pin=None): a fresh module instance of frozen2/method.py (or of `path`), executed from the
  very bytes whose sha256 was checked against `pin` (default corpus.FROZEN2_SHA256). The instance carries
  `checked_sha256` and `checked_path`. On a mismatch it raises VoidRun with the same two attributes set, and
  nothing is executed. Z is whatever the file says (2.5); it is never set after import, and there is no Z = 3
  instance in round 2 (PREREG-2.md: "No secondary Z is reported").
- Round 1's method is loaded, as a control only, by load(corpus.FROZEN, pin=corpus.FROZEN_SHA256): its file is
  CRLF on a Windows checkout (frozen/** eol=crlf), and that pin is of those bytes. A pin that does not belong to
  the file refuses it: round 1's pin rejects frozen2/method.py and round 2's rejects frozen/method.py.

`checked_path` is the absolute path read, a pathlib.Path. A file that cannot be read raises the OS error
unchanged: there is no hash to record, and the run stops before anything is computed.
"""
from pathlib import Path

from .corpus import FROZEN2, FROZEN2_SHA256, VoidRun, exec_module, sha256_bytes  # one VoidRun for the harness


def load(path=None, *, pin=None):
    want = FROZEN2_SHA256 if pin is None else pin
    p = Path(FROZEN2 if path is None else path).resolve()
    data = p.read_bytes()
    h = sha256_bytes(data)
    if h != want:
        e = VoidRun('%s has sha256 %s, not %s: the run is void (PREREG-2.md) and nothing was executed. A checkout with '
                    'other line ends than the LF form ../.gitattributes pins changes the bytes.' % (p, h, want))
        e.checked_sha256 = h
        e.checked_path = p
        raise e
    mod = exec_module('m8b_method_%s' % h[:8], p, data)
    mod.checked_sha256 = h
    mod.checked_path = p
    return mod
