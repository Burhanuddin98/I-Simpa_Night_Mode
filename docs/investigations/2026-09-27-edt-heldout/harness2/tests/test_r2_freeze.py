"""T42 (../../HARNESS-PLAN-2.md section 7, with section 9 M4): freeze2 writes ADDENDUM-B1.md's hash table, hashing every
file under harness2/ and frozen2/ by glob (no hand list) plus the named documents, and --check recomputes it.
Nothing here writes ADDENDUM-B1.md itself: the table goes to tmp_path.
"""
import hashlib

from conftest import HARNESS, INVESTIGATION, sha256_file

from m8b import freeze2

NAMED = ['PREREG.md', 'PREREG-2.md', 'HARNESS-PLAN.md', 'HARNESS-PLAN-2.md', 'ADDENDUM-A1.md', 'expected_dry_2.json',
         'preview_pin.json', 'frozen2.sha256', '.gitattributes', 'frozen/method.py']


def test_t42_inventory_is_complete_and_check_catches_a_change(tmp_path):
    inv = freeze2.inventory()
    assert inv and all(len(v) == 64 for v in inv.values())
    for p in (HARNESS / 'm8b').glob('*.py'):
        assert 'harness2/m8b/' + p.name in inv, p.name
    for p in (INVESTIGATION / 'frozen2').rglob('*'):
        if p.is_file():
            assert 'frozen2/' + p.relative_to(INVESTIGATION / 'frozen2').as_posix() in inv
    for p in HARNESS.rglob('*'):
        if p.is_file() and '__pycache__' not in p.parts and p.suffix != '.pyc':
            assert 'harness2/' + p.relative_to(HARNESS).as_posix() in inv, p
    for n in NAMED:
        assert n in inv, n
    assert not [k for k in inv if '__pycache__' in k or k.endswith('.pyc')]
    # text files by their LF bytes, the CRLF-checked-out round-1 method by its raw bytes (PREREG.md:8)
    lf = (HARNESS / 'm8b' / 'method.py').read_bytes().replace(b'\r\n', b'\n')
    assert inv['harness2/m8b/method.py'] == hashlib.sha256(lf).hexdigest()
    assert inv['frozen/method.py'] == sha256_file(INVESTIGATION / 'frozen' / 'method.py')
    assert inv['frozen2/method.py'] == '029d90ac5e8f6a634a51a7ffce136cbd4c0b7ea3d3ad8a18b78fd8240ff224a0'
    # the table, and --check against the real tree
    out = tmp_path / 'table.md'
    freeze2.write_table(out)
    text = out.read_text(encoding='utf-8')
    assert all(k in text and v in text for k, v in inv.items())
    assert freeze2.check(out) == []
    h = inv['frozen2/method.py']
    planted = text.replace(h, h[:-1] + ('0' if h[-1] != '0' else '1'))
    (tmp_path / 'planted.md').write_text(planted, encoding='utf-8')
    assert freeze2.check(tmp_path / 'planted.md') == ['frozen2/method.py']
    assert freeze2.main(['--check', str(out)]) == 0 and freeze2.main(['--check', str(tmp_path / 'planted.md')]) != 0
    # a one-byte change in a tree: a miniature of the investigation folder
    root = tmp_path / 'D'
    for n in NAMED:
        (root / n).parent.mkdir(parents=True, exist_ok=True)
        (root / n).write_bytes(b'x ' + n.encode() + b'\n')
    (root / 'harness2' / 'm8b').mkdir(parents=True)
    (root / 'harness2' / 'm8b' / 'a.py').write_bytes(b'print(1)\n')
    (root / 'frozen2').mkdir(exist_ok=True)
    (root / 'frozen2' / 'method.py').write_bytes(b'Z = 2.5\n')
    mini = tmp_path / 'mini.md'
    freeze2.write_table(mini, root=root)
    assert freeze2.check(mini, root=root) == []
    (root / 'harness2' / 'm8b' / 'a.py').write_bytes(b'print(2)\n')
    assert freeze2.check(mini, root=root) == ['harness2/m8b/a.py']
    (root / 'harness2' / 'm8b' / 'b.py').write_bytes(b'new\n')          # a file the table does not know is a difference too
    assert sorted(freeze2.check(mini, root=root)) == ['harness2/m8b/a.py', 'harness2/m8b/b.py']
    # the facts B1 records
    f = freeze2.facts()
    for k in ('python', 'numpy', 'git_commit', 'git_tree', 'solver_manifest_sha256', 'simpa_exe_sha256', 'round2_rows_exist'):
        assert k in f, k
    assert f['round2_rows_exist'] is False
