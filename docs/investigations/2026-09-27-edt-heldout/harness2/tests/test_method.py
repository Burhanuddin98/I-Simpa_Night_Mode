"""T1-T3 as round 2 reads them (HARNESS-PLAN-2.md section 6; round 1's version is ../../harness/tests/test_method.py):
the two method files and the loader. Round 2 has no Z = 3 instance, so round 1's T3 (the Z = 3 side instance) is not
carried; T23 and T24 (test_r2_scorer.py) hold the new pin and the per-round pins."""
from conftest import FROZEN, FROZEN2, FROZEN2_SHA256, FROZEN_SHA256, sha256_bytes, sha256_file

from m8b import method


def test_t01_frozen_methods_hash():
    """T1, the controls: round 1's frozen/method.py is untouched (462c37cf..., PREREG.md:8) and frozen2/method.py is the
    file PREREG-2 pins (029d90ac...). Both pass before and after."""
    assert FROZEN.is_file() and FROZEN2.is_file()
    assert sha256_file(FROZEN) == FROZEN_SHA256 == '462c37cf159d4d9bd8abadd8261f975fa47ace66f0d4cb4b1e6f4234c77fdf6e'
    assert sha256_file(FROZEN2) == FROZEN2_SHA256 == '029d90ac5e8f6a634a51a7ffce136cbd4c0b7ea3d3ad8a18b78fd8240ff224a0'


def test_t02_loader_refuses_a_copy_with_one_byte_changed(tmp_path):
    """T2: a copy of frozen2/method.py with one byte changed is refused with VoidRun, which records the hash it checked;
    the file itself loads and records its own."""
    data = FROZEN2.read_bytes()
    i = data.index(b'Z = 2.5') + len(b'Z = ')
    bad = bytearray(data)
    bad[i] = ord('3')                               # Z = 2.5 -> Z = 3.5: one byte, still valid Python
    assert sum(a != b for a, b in zip(data, bad)) == 1
    copy = tmp_path / 'method.py'
    copy.write_bytes(bytes(bad))

    try:
        got = method.load(copy)
    except method.VoidRun as e:
        assert getattr(e, 'checked_sha256', None) == sha256_bytes(bytes(bad)), 'VoidRun must record the hash it checked'
    else:
        raise AssertionError('method.load accepted a copy with one byte changed (returned %r)' % (got,))

    good = method.load(FROZEN2)
    assert good is not None, 'method.load returned nothing for the frozen file'
    assert good.checked_sha256 == FROZEN2_SHA256
    assert good.Z == 2.5 and good.MIN_POINTS == 8 and good.TAIL_SHARE == 0.02 and good.HW_FLOOR == 0.005 and good.K_BALL == 0.01


def test_t03_loads_are_independent_and_leave_the_file_unchanged():
    """T3 as round 2 reads it: two loads are two modules (no shared namespace), each keeps the file's Z = 2.5, and
    loading changes nothing on disk."""
    before = sha256_file(FROZEN2)
    a, b = method.load(), method.load()
    assert a is not b and a.analyse.__globals__ is not b.analyse.__globals__
    a.Z = 9.0                                        # one instance's global is not the other's
    assert b.Z == 2.5 and method.load().Z == 2.5
    assert sha256_file(FROZEN2) == before == FROZEN2_SHA256
