"""The freeze's hash table for ADDENDUM-B1.md (HARNESS-PLAN-2.md section 7, with section 9 M4).

M4: B1 hashes every file under `harness2/` and `frozen2/` (a glob, sorted; no hand list, so a test, a fixture or a dry
input cannot be left out), plus the named documents below. Text files are hashed by their LF bytes as committed
(`harness2/**` and `frozen2/**` are `text eol=lf`, ../../.gitattributes), so a checkout with other line ends gives the
same hash; fixtures that are not text (.gabe, .recp) and round 1's `frozen/method.py` (CRLF on a Windows checkout,
pinned so: PREREG.md:8) are hashed by their raw bytes. __pycache__ and .pyc are never listed.

    python -m m8b.freeze2 --write <file>     # the table and the facts, as markdown (B1 embeds it)
    python -m m8b.freeze2 --check <file>     # recompute every hash; exit 1 and name each file that differs

Contract:
- inventory(root=None) -> {path relative to the investigation folder: sha256}, sorted. root defaults to the
  investigation folder; FileNotFoundError when a named document is missing.
- write_table(path, root=None, extra=None): the table as markdown, plus `facts()` (or `extra`) as a list.
- check(path, root=None) -> [relative paths whose hash differs, that the table lacks, or that the table lists and
  the tree no longer has]; [] when nothing differs.
- facts(round2_paths=None) -> dict: python, numpy, git_commit, git_tree, solver_manifest_sha256, spps_code_sha256,
  simpa_exe_sha256 (None when the exe is not there) and round2_rows_exist (True when any round-2 data, room or log
  folder holds anything: B1 is written before the first one does).
Nothing here writes ADDENDUM-B1.md; it is written after the independent review.
"""
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

import numpy as np

from . import corpus, round2

NAMED = ('PREREG.md', 'PREREG-2.md', 'HARNESS-PLAN.md', 'HARNESS-PLAN-2.md', 'ADDENDUM-A1.md', 'expected_dry_2.json',
         'preview_pin.json', 'frozen2.sha256', '.gitattributes', 'frozen/method.py')
RAW = ('frozen/method.py',)                         # PREREG.md:8 pins the CRLF checkout's bytes
TEXT_SUFFIXES = {'.py', '.md', '.json', '.jsonl', '.txt', '.toml', '.sha256'}
ROW = re.compile(r'^\| (.+) \| ([0-9a-f]{64}) \|$')


def _skip(parts, name):
    return '__pycache__' in parts or name.endswith('.pyc')


def _hash(path, rel):
    data = Path(path).read_bytes()
    if rel not in RAW and (Path(rel).suffix in TEXT_SUFFIXES or Path(rel).name == '.gitattributes'):
        data = data.replace(b'\r\n', b'\n')
    return hashlib.sha256(data).hexdigest()


def inventory(root=None):
    root = Path(root or corpus.INVESTIGATION)
    out = {}
    for sub in ('harness2', 'frozen2'):
        for p in sorted((root / sub).rglob('*')):
            if p.is_file() and not _skip(p.relative_to(root).parts, p.name):
                rel = p.relative_to(root).as_posix()
                out[rel] = _hash(p, rel)
    for n in NAMED:
        p = root / n
        if not p.is_file():
            raise FileNotFoundError('%s is named in the freeze but is not there' % p)
        out[n] = _hash(p, n)
    return dict(sorted(out.items()))


def _git(*args):
    try:
        r = subprocess.run(['git', '-C', str(corpus.REPO), *args], capture_output=True, text=True, timeout=120)
        return r.stdout.strip() if r.returncode == 0 else None
    except (OSError, subprocess.SubprocessError):
        return None


def _sha256_file(p):
    p = Path(p)
    return hashlib.sha256(p.read_bytes()).hexdigest() if p.is_file() else None


def facts(round2_paths=None):
    paths = [round2.DATA_ROOT, round2.ROOMS_ROOT, round2.RESULTS_ROOT, round2.LAUNCH_LOG, round2.PROGRESS_LOG] \
        if round2_paths is None else list(round2_paths)

    def holds_something(p):
        p = Path(p)
        if p.is_dir():
            return any(p.iterdir())
        return p.is_file() and p.stat().st_size > 0

    manifest = corpus.REPO / 'solvers' / 'manifest.json'
    spps = None
    if manifest.is_file():
        spps = json.loads(manifest.read_text(encoding='utf-8-sig')).get('code_sha256', {}).get('spps.exe')
    return dict(python=sys.version.split()[0], numpy=np.__version__, git_commit=_git('rev-parse', 'HEAD'),
                git_tree=_git('rev-parse', 'HEAD^{tree}'), solver_manifest_sha256=_sha256_file(manifest), spps_code_sha256=spps,
                simpa_exe_sha256=_sha256_file(Path(r'C:\tmp\nm-target\release\simpa.exe')),
                round2_rows_exist=any(holds_something(p) for p in paths))


def write_table(path, root=None, extra=None):
    inv = inventory(root)
    f = facts() if extra is None else extra
    L = ['# Hash table for ADDENDUM-B1.md (m8b.freeze2)', '',
         '%d files: every file under harness2/ and frozen2/, and the named documents. Text files by their LF bytes, '
         'frozen/method.py and non-text fixtures by their raw bytes.' % len(inv), '',
         '| file | sha256 |', '|---|---|']
    L += ['| %s | %s |' % (k, v) for k, v in inv.items()]
    L += ['', 'Facts at the freeze:', ''] + ['- %s: %s' % (k, v) for k, v in f.items()]
    Path(path).write_text('\n'.join(L) + '\n', encoding='utf-8', newline='\n')
    return inv


def check(path, root=None):
    listed = {}
    for line in Path(path).read_text(encoding='utf-8').splitlines():
        m = ROW.match(line)
        if m:
            listed[m.group(1)] = m.group(2)
    now = inventory(root)
    return sorted(k for k in set(listed) | set(now) if listed.get(k) != now.get(k))


def main(argv=None):
    import argparse
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    g = ap.add_mutually_exclusive_group(required=True)
    g.add_argument('--write', metavar='FILE')
    g.add_argument('--check', metavar='FILE')
    a = ap.parse_args(argv)
    if a.write:
        inv = write_table(a.write)
        print('wrote %s: %d files' % (a.write, len(inv)))
        return 0
    bad = check(a.check)
    print('freeze check:', 'every hash reproduced' if not bad else 'DIFFERS: %s' % bad)
    return 1 if bad else 0


if __name__ == '__main__':
    sys.dont_write_bytecode = True
    sys.exit(main())
