"""Step 7d (HARNESS-PLAN.md section 5 item 3, D4; P25, P26; PREREG.md:60): the attack kit's setup
and mechanical half only. Two parts, neither of which runs the frozen method or touches heldout/:

Part 1 (STUB_CLASS_WRONG, STUB_CLASS_RIGHT): two hand-written m8b.attack classes, drawn through
m8b.attack.draws (the harness's own draw, seeded by the class's sha256 -- P26), each instance turned
into a m8b.synth_fresh spec and scored by a hand-written STUB, never frozen/method.py. The stub is
deliberately simple: it is keyed on the class's own 'ratio' and nothing else, so it reproduces
wrong-silent on every draw of one class and is exact on every draw of the other, by construction, not
by chance. This exercises the same path a real attack class takes (validate_class -> draws ->
per-instance truth) without the frozen file, which the hard rule keeps out of this step.

Part 2 (CONTROL_A, CONTROL_B): the two control classes the judge panel will be checked against before
any real class is judged (HARNESS-PLAN.md section 5 item 3's D4 bullet). CONTROL_A is F2's own
settings (HARNESS-PLAN.md 2.2's table and the room's receiver list), carried into the attack schema
through the same diffuse-field relation the implausible control's own rationale text uses (room
constant R_c = S*alpha/(1-alpha); DRR(dB) = 10*log10(R_c / (16*pi*d^2))), so both controls are
produced the same way: CONTROL_B is PREREG's own example of a configuration that passes PHYSICS.md's
bounds but is not a measurement any room could produce. Each is written through m8b.attack.judge_prompt
as its own, single-class prompt file -- no result field, no name that gives away which answer is
expected -- under C:\\tmp\\m8b-edt\\dry\\D4\\. No judge is run here; panel/parse_vote are not called.

Usage (from harness/, with the venv and no bytecode):
    set PYTHONDONTWRITEBYTECODE=1 & C:\\tmp\\m8b-edt\\venv\\Scripts\\python.exe dry\\run_d4.py
"""
import json
import math
import sys
from pathlib import Path

sys.path.insert(0, '.')
sys.dont_write_bytecode = True

from m8b import attack, synth_fresh  # noqa: E402

OUT = Path(r'C:\tmp\m8b-edt\dry\D4')
JND = 0.05                              # PREREG.md:48, "Wrong-silent"
RATIO_BUG_AT_OR_ABOVE = 3.0             # the stub's only "logic"

# ================================================================================================
# Part 1: two hand-written classes, drawn, scored by a stub that is wrong for exactly one of them.
# ================================================================================================
_COMMON_PARAMS = {
    't60_s': [0.8, 1.5],
    'drr_db': [-8.0, -2.0],
    'd_m': [3.0, 6.0],
    'gap_ms': [1.0, 5.0],
    'delay_ms': [0.0, 0.0],
    'late_share_db': [-12.0, -6.0],
}

STUB_CLASS_WRONG = {                    # every draw has ratio = 5.0: the stub's bug fires every time
    'generator': 'synth',
    'params': dict(_COMMON_PARAMS, ratio=[5.0, 5.0]),
    'noise': {'kind': 'none'},
    'step_ms': 1.0,
    'run_over_t60': [1.5, 2.5],
    'R_m': [0.31, 0.31],
}
STUB_CLASS_RIGHT = {                    # every draw has ratio = 1.5: the stub is exact every time
    'generator': 'synth',
    'params': dict(_COMMON_PARAMS, ratio=[1.5, 1.5]),
    'noise': {'kind': 'none'},
    'step_ms': 1.0,
    'run_over_t60': [1.5, 2.5],
    'R_m': [0.31, 0.31],
}


def stub_method(spec, truth_val):
    """NOT the frozen method. Its entire behaviour: wrong by 50% (status 'ok') when the row's ratio
    is >= RATIO_BUG_AT_OR_ABOVE, exact (status 'ok') otherwise. Built this way on purpose, so that
    running it against STUB_CLASS_WRONG (ratio 5.0 always) and STUB_CLASS_RIGHT (ratio 1.5 always)
    gives a clean, reproducible split without needing frozen/method.py or any real analysis."""
    if spec['ratio'] >= RATIO_BUG_AT_OR_ABOVE:
        return {'status': 'ok', 'edt': truth_val * 1.5}
    return {'status': 'ok', 'edt': truth_val}


def run_class(name, cls, n=10):
    ok, why = attack.validate_class(cls)
    assert ok, '%s failed validate_class: %s' % (name, why)
    instances = attack.draws(cls, n=n)
    assert len(instances) == n
    rows = []
    n_wrong = 0
    for i, inst in enumerate(instances):
        spec = synth_fresh.make_spec(id='%s|%03d' % (name, i), **inst)
        truth_val = synth_fresh.truth(spec)
        result = stub_method(spec, truth_val)
        wrong = (result['status'] == 'ok' and math.isfinite(truth_val) and truth_val != 0
                 and abs(result['edt'] / truth_val - 1) > JND)
        if wrong:
            n_wrong += 1
        rows.append({'id': spec['id'], 'ratio': inst['ratio'], 'truth': truth_val, 'edt': result['edt'],
                     'wrong_silent': wrong})
    reproducible = n_wrong >= attack.REPRODUCIBLE if n >= attack.REPRODUCIBLE else n_wrong == n
    return {'class': name, 'class_sha256': attack.class_sha256(cls), 'n': n, 'n_wrong_silent': n_wrong,
            'reproducible_wrong_silent': reproducible, 'rows': rows}


# ================================================================================================
# Part 2: the two control classes for the judge panel (HARNESS-PLAN.md section 5 item 3's D4 bullet).
# ================================================================================================
def _f2_drr_db(d_m, alpha=0.08, L=(17.0, 12.5, 8.0)):
    """The diffuse-field direct-to-reverberant ratio at distance d_m in F2 (HARNESS-PLAN.md 2.2):
    room constant R_c = S*alpha/(1-alpha), DRR(dB) = 10*log10(R_c / (16*pi*d_m^2)) for an omnidirectional
    source. The same relation CONTROL_B's own rationale text uses in reverse (DRR +30 dB at 30 m needs
    a room of about 3e9 m^3), so both controls are produced by one formula, not by separate guesses."""
    lx, ly, lz = L
    S = 2 * (lx * ly + lx * lz + ly * lz)
    Rc = S * alpha / (1 - alpha)
    return 10 * math.log10(Rc / (16 * math.pi * d_m * d_m))


# F2's own receivers (HARNESS-PLAN.md 2.2): near 1.34-1.68 m, mid 4.93-7.16 m, far 12.54-15.07 m.
_F2_D_NEAR, _F2_D_FAR = 1.34, 15.07
_F2_DRR_LO = round(_f2_drr_db(_F2_D_FAR), 1)     # the far receiver: least direct sound
_F2_DRR_HI = round(_f2_drr_db(_F2_D_NEAR), 1)    # the near receiver: most direct sound

CONTROL_A = {                           # "F2's settings at a 1 ms step" (plausible)
    'generator': 'synth',
    'params': {
        't60_s': [2.26, 3.63],          # F2's design T60, 4 kHz to 125 Hz (HARNESS-PLAN.md 2.2)
        'drr_db': [_F2_DRR_LO, _F2_DRR_HI],
        'd_m': [_F2_D_NEAR, _F2_D_FAR],
        'gap_ms': [0.0, 5.0],
        'delay_ms': [0.0, 0.0],
        'ratio': [1.0, 1.0],            # F2 is uniform absorption (0.08 on every surface): single slope
        'late_share_db': [-3.0, 0.0],
    },
    'noise': {'kind': 'compound_poisson', 'particles_per_source': [150000.0, 150000.0]},  # P8's default
    'step_ms': 1.0,
    'run_over_t60': [2.75, 4.43],       # P10's 10 s default run over F2's design T60 range
    'R_m': [0.31, 0.31],                # P8's default ball
}
CONTROL_B = {                           # DRR +30 dB at 30 m, T60 10 s (implausible)
    'generator': 'synth',
    'params': {
        't60_s': [10.0, 10.0],
        'drr_db': [30.0, 30.0],
        'd_m': [30.0, 30.0],
        'gap_ms': [0.0, 0.0],
        'delay_ms': [0.0, 0.0],
        'ratio': [1.0, 1.0],
        'late_share_db': [0.0, 0.0],
    },
    'noise': {'kind': 'none'},
    'step_ms': 1.0,
    'run_over_t60': [1.0, 1.0],
    'R_m': [0.31, 0.31],
}


def write_prompt(out_dir, tag, internal_name, cls):
    """One judge_prompt file for exactly one class, under a neutral internal name (the dict key
    attack.judge_prompt turns into the prompt's section header) that gives no hint of the expected
    verdict."""
    ok, why = attack.validate_class(cls)
    assert ok, '%s failed validate_class: %s' % (tag, why)
    class_text = json.dumps(cls, indent=1)
    prompt = attack.judge_prompt({internal_name: class_text})
    path = out_dir / ('judge_prompt_%s.md' % tag)
    path.write_text(prompt, encoding='utf-8')
    return path, cls, attack.class_sha256(cls)


def main():
    OUT.mkdir(parents=True, exist_ok=True)

    print('=== Part 1: stub classes ===')
    result_wrong = run_class('wrong', STUB_CLASS_WRONG, n=10)
    result_right = run_class('right', STUB_CLASS_RIGHT, n=10)
    for r in (result_wrong, result_right):
        print('%-6s sha256=%s n=%d n_wrong_silent=%d reproducible=%s'
              % (r['class'], r['class_sha256'][:12], r['n'], r['n_wrong_silent'], r['reproducible_wrong_silent']))
    assert result_wrong['reproducible_wrong_silent'] is True, 'STUB_CLASS_WRONG did not reproduce wrong-silent'
    assert result_right['reproducible_wrong_silent'] is False, 'STUB_CLASS_RIGHT reproduced wrong-silent'
    assert result_right['n_wrong_silent'] == 0, 'STUB_CLASS_RIGHT was not exact on every draw'
    assert result_wrong['n_wrong_silent'] == result_wrong['n'], 'STUB_CLASS_WRONG was not wrong on every draw'
    (OUT / 'stub_results.json').write_text(
        json.dumps({'wrong': result_wrong, 'right': result_right}, indent=2), encoding='utf-8')
    print('exactly the class built to (ratio >= %g) reproduced wrong-silent: %s'
          % (RATIO_BUG_AT_OR_ABOVE,
             result_wrong['reproducible_wrong_silent'] and not result_right['reproducible_wrong_silent']))

    print('\n=== Part 2: control classes for the judge panel ===')
    path_a, cls_a, sha_a = write_prompt(OUT, 'f2_1ms', 'control_a.json', CONTROL_A)
    path_b, cls_b, sha_b = write_prompt(OUT, 'drr30_30m', 'control_b.json', CONTROL_B)
    print('CONTROL_A (F2 @ 1 ms, expected plausible):   sha256=%s -> %s' % (sha_a[:12], path_a))
    print('CONTROL_B (DRR+30 dB @ 30 m, T60 10 s, expected implausible): sha256=%s -> %s' % (sha_b[:12], path_b))
    (OUT / 'controls.json').write_text(
        json.dumps({'CONTROL_A': cls_a, 'CONTROL_A_sha256': sha_a, 'CONTROL_A_drr_range_db': [_F2_DRR_LO, _F2_DRR_HI],
                    'CONTROL_B': cls_b, 'CONTROL_B_sha256': sha_b}, indent=2), encoding='utf-8')

    print('\nNo judge was run. panel()/parse_vote() were not called. The coordinator dispatches three')
    print('independent judges per control next.')
    return path_a, path_b


if __name__ == '__main__':
    main()
