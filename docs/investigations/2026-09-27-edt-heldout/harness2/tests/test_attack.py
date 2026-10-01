"""T17 (HARNESS-PLAN.md section 4): the attack kit (P25, P26). The panel rule itself is T20's."""
import copy
import json

from conftest import FROZEN

from m8b import attack

VALID = {"generator": "synth",
         "params": {"t60_s": [0.5, 2.0], "drr_db": [-10.0, 0.0], "d_m": [1.0, 5.0], "gap_ms": [0.0, 10.0],
                    "delay_ms": [0.0, 20.0], "ratio": [1.5, 3.0], "late_share_db": [-15.0, -5.0]},
         "noise": {"kind": "none"}, "step_ms": 1.0, "run_over_t60": [0.5, 2.0], "R_m": [0.31, 0.31]}
RESULT_TOKENS = ('edt_lo', 'edt_hi', 'wrong_silent', 'wrong-silent', 'n_wrong', '"status"', '"truth"', '"edt"',
                 'weak_spots')
TUNING_TOKENS = (b'weak_spots', b'res_z3', b'res_ism', b'scans_all', b'FINAL.md', b'corpus_rooms', b'edt-simplify')


def with_param(cls, key, box):
    c = copy.deepcopy(cls)
    c['params'][key] = box
    return c


def test_t17_attack_bounds_draws_sandbox_and_blind_judges(tmp_path):
    """T17: a class out of bounds is rejected with its reason; the draws reproduce from the class hash;
    the sandbox holds exactly the allowed files; the judges' prompt holds the class files and
    PHYSICS.md and no result field."""
    ok = attack.validate_class(VALID)
    assert ok == (True, None), 'a class inside PHYSICS.md\'s limits must pass, got %r' % (ok,)
    for key, box in (('t60_s', [5.0, 20.0]), ('t60_s', [0.05, 1.0]), ('drr_db', [-50.0, 0.0]), ('drr_db', [0.0, 35.0]),
                     ('d_m', [5.0, 1.0])):
        good, why = attack.validate_class(with_param(VALID, key, box))
        assert good is False and key in why, (key, box, why)
    bad_gen = dict(copy.deepcopy(VALID), generator='fdtd')
    good, why = attack.validate_class(bad_gen)
    assert good is False and 'generator' in why

    d1 = attack.draws(VALID)
    assert d1 is not None and len(d1) == 20
    for inst in d1:
        for k, (lo, hi) in VALID['params'].items():
            assert lo <= inst[k] <= hi, (k, inst[k])
        assert VALID['R_m'][0] <= inst['R_m'] <= VALID['R_m'][1]
        assert VALID['run_over_t60'][0] <= inst['run_over_t60'] <= VALID['run_over_t60'][1]
    reordered = json.loads(json.dumps(VALID, sort_keys=True))
    assert list(reordered) != list(VALID)
    assert attack.class_sha256(reordered) == attack.class_sha256(VALID)
    assert json.dumps(attack.draws(reordered), sort_keys=True) == json.dumps(d1, sort_keys=True)
    changed = with_param(VALID, 't60_s', [0.5, 2.1])
    assert attack.class_sha256(changed) != attack.class_sha256(VALID)
    assert json.dumps(attack.draws(changed), sort_keys=True) != json.dumps(d1, sort_keys=True)

    dest = tmp_path / 'attack'
    attack.build_sandbox(dest)
    assert dest.is_dir() and sorted(p.name for p in dest.iterdir()) == sorted(attack.SANDBOX_FILES)
    assert sorted(attack.SANDBOX_FILES) == ['INTERFACE.md', 'PHYSICS.md', 'generators.py', 'method.py']
    assert all(p.is_file() for p in dest.iterdir())
    assert (dest / 'method.py').read_bytes() == FROZEN.read_bytes()
    phys = (dest / 'PHYSICS.md').read_text(encoding='utf-8')
    assert phys == attack.physics_md()
    assert 'T60' in phys and '0.1' in phys and '10' in phys and 'DRR' in phys and '30' in phys
    assert '-40' in phys or '−40' in phys
    assert 'analyse(bins, dt, t_arrival, meta' in (dest / 'INTERFACE.md').read_text(encoding='utf-8')
    gen = (dest / 'generators.py').read_text(encoding='utf-8')
    for token in ('m8b', 'target/', 'target\\', 'agents/', ':\\', 'B:/', 'C:/'):
        assert token not in gen, 'generators.py reaches outside the sandbox (%r)' % token
    for p in dest.iterdir():
        if p.name == 'method.py':   # frozen, pinned byte for byte above; its line 1 names ../FINAL.md
            continue
        data = p.read_bytes()
        assert not any(t in data for t in TUNING_TOKENS), '%s names a tuning log' % p.name

    files = {'c01.json': json.dumps(VALID, indent=1), 'c02.json': json.dumps(changed, indent=1)}
    prompt = attack.judge_prompt(files)
    assert prompt is not None
    assert all(text in prompt for text in files.values()) and attack.physics_md() in prompt
    for token in RESULT_TOKENS:
        assert token not in prompt, 'the judges\' prompt holds %r' % token
    leaky = dict(files, **{'c03.json': json.dumps(dict(VALID, status='ok', wrong_silent=7), indent=1)})
    try:
        attack.judge_prompt(leaky)
    except ValueError:
        pass
    else:
        raise AssertionError('a class file with a result field reached the judges\' prompt')
