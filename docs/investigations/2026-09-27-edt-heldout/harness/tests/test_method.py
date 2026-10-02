"""T1-T3 (HARNESS-PLAN.md section 4): the frozen method and its loader (PREREG.md:8-11, P32)."""
from conftest import FROZEN, FROZEN_SHA256, sha256_bytes, sha256_file

from m8b import method


def test_t01_frozen_method_hash():
    """T1, the control: frozen/method.py hashes to 462c37cf... (PREREG.md:8). Passes before and after."""
    assert FROZEN.is_file()
    assert sha256_file(FROZEN) == FROZEN_SHA256 == '462c37cf159d4d9bd8abadd8261f975fa47ace66f0d4cb4b1e6f4234c77fdf6e'


def test_t02_loader_refuses_a_copy_with_one_byte_changed(tmp_path):
    """T2: a copy with one byte changed is refused with VoidRun, which records the hash it checked;
    the frozen file itself loads and records its own."""
    data = FROZEN.read_bytes()
    i = data.index(b'Z = 2.0') + len(b'Z = ')
    bad = bytearray(data)
    bad[i] = ord('3')                               # Z = 2.0 -> Z = 3.0: one byte, still valid Python
    assert sum(a != b for a, b in zip(data, bad)) == 1
    copy = tmp_path / 'method.py'
    copy.write_bytes(bytes(bad))

    try:
        got = method.load(copy)
    except method.VoidRun as e:
        assert getattr(e, 'checked_sha256', None) == sha256_bytes(bytes(bad)), 'VoidRun must record the hash it checked'
    else:
        raise AssertionError('method.load accepted a copy with one byte changed (returned %r)' % (got,))

    good = method.load(FROZEN)
    assert good is not None, 'method.load returned nothing for the frozen file'
    assert good.checked_sha256 == FROZEN_SHA256
    assert good.Z == 2.0 and good.MIN_POINTS == 8 and good.TAIL_SHARE == 0.02 and good.HW_FLOOR == 0.005


def test_t03_z3_instance_leaves_the_file_and_the_primary_unchanged():
    """T3: the Z = 3 instance has Z = 3.0, the primary instance keeps 2.0, and the file hash is unchanged."""
    before = sha256_file(FROZEN)
    primary = method.load()
    z3 = method.load_z3()
    assert primary is not None and z3 is not None, 'method.load / load_z3 returned nothing'
    assert z3 is not primary
    assert z3.Z == 3.0 and primary.Z == 2.0
    # each instance's analyse reads its own module's Z
    assert z3.analyse.__globals__['Z'] == 3.0 and primary.analyse.__globals__['Z'] == 2.0
    assert z3.checked_sha256 == FROZEN_SHA256 and primary.checked_sha256 == FROZEN_SHA256
    assert method.load().Z == 2.0, 'a load after the Z = 3 instance must still have Z = 2.0'
    assert sha256_file(FROZEN) == before == FROZEN_SHA256
