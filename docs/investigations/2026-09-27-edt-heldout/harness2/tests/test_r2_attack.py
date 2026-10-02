"""T40 (../../HARNESS-PLAN-2.md sections 5 and 6, with section 9 m5): the round-2 attack kit. Nothing here runs an
attacker or a judge: the sandbox is built in tmp_path, and the real one (C:\\tmp\\m8b-edt\\attack2) waits for SENTINEL.md.
"""
import hashlib
import re

import numpy as np
import pytest
from conftest import FROZEN2, FROZEN2_SHA256

from m8b import attack, driver
from m8b.corpus import VoidRun

CLASS = {'generator': 'synth', 'params': {'t60_s': [0.5, 2.0], 'drr_db': [-10.0, 0.0]}, 'noise': {'kind': 'none'},
         'step_ms': 1.0, 'run_over_t60': [1.0, 2.0], 'R_m': [0.2, 0.5]}


def refusal_reasons():
    out = set()
    for line in FROZEN2.read_text(encoding='utf-8').splitlines():
        if '_refuse(' in line and not line.lstrip().startswith('def '):
            out |= set(re.findall(r"'([a-z_]+)'", line))
    return out


def test_t40_attack_round_2(tmp_path, monkeypatch):
    reasons = refusal_reasons()
    assert reasons == {'no_energy', 'no_energy_after_arrival', 'run_too_short', 'direct_only', 'step_too_coarse',
                       'not_decaying', 'too_few_particles', 'not_decaying_at_run_end', 'receiver_too_large'}
    sentinel = tmp_path / 'SENTINEL.md'
    with pytest.raises(driver.Refused) as e:
        attack.build_sandbox(tmp_path / 'sb', sentinel=sentinel)
    assert e.value.code == 'attack_before_sentinel' and not (tmp_path / 'sb').exists()
    sentinel.write_text('recounted\n', encoding='utf-8')
    attack.build_sandbox(tmp_path / 'sb', sentinel=sentinel)
    sb = tmp_path / 'sb'
    assert sorted(p.name for p in sb.iterdir()) == sorted(attack.SANDBOX_FILES)
    assert sorted(attack.SANDBOX_FILES) == ['INTERFACE.md', 'PHYSICS.md', 'generators.py', 'method.py']
    assert (sb / 'method.py').read_bytes() == FROZEN2.read_bytes()
    assert hashlib.sha256((sb / 'method.py').read_bytes()).hexdigest() == FROZEN2_SHA256
    iface = (sb / 'INTERFACE.md').read_text(encoding='utf-8')
    section = iface.split('## Refusal reasons')[1]
    assert set(re.findall(r'^- `([a-z_]+)`', section, re.M)) == reasons, 'INTERFACE.md names every refusal, and only those'
    assert 'receiver_too_large' in iface and 'wrong-silent' in iface and 'never' in iface
    assert "'ok'" in iface and "'wide'" in iface and "'refused'" in iface
    # a changed file voids the build and writes nothing
    bad = tmp_path / 'bad_method.py'
    bad.write_bytes(FROZEN2.read_bytes().replace(b'Z = 2.5', b'Z = 2.6'))
    monkeypatch.setattr(attack, 'FROZEN2', bad)
    with pytest.raises(VoidRun):
        attack.build_sandbox(tmp_path / 'sb2', sentinel=sentinel)
    assert not (tmp_path / 'sb2').exists() or not any((tmp_path / 'sb2').iterdir())
    monkeypatch.undo()
    # the folder, the seed and the draws (m5: fresh, not round 1's)
    assert str(attack.SANDBOX).replace('/', '\\') == r'C:\tmp\m8b-edt\attack2'
    assert attack.ATTACK_SEED == driver.ATTACK_SEED == 2026100203
    first = attack.draws(CLASS)
    assert first == attack.draws(CLASS) and len(first) == attack.DRAWS
    h = int(attack.class_sha256(CLASS), 16)
    r1 = np.random.default_rng(np.random.SeedSequence(h))                 # round 1's seeding: by the class alone
    round1_first_drr = -10.0 + 10.0 * r1.random()                          # params by sorted name: drr_db first
    assert first[0]['drr_db'] != round1_first_drr, 'the draws still ignore the attack seed'
