"""Step 7d (HARNESS-PLAN.md section 5 item 3's D4 bullet, last one; GREEN.md's "D4 judge panel, first
run"): PHYSICS.md's diffuse-field consistency check, added to m8b.attack.physics_md() so the text the
attacker and the judges both read carries it. This file only; it does not touch test_attack.py's T17.
"""
from dry import run_d4

from m8b import attack


def test_physics_md_has_diffuse_field_formula_and_bound():
    text = attack.physics_md()
    assert 'Diffuse-field consistency' in text
    assert '0.057' in text
    assert '20 log10' in text
    assert 'T60 (d / 0.057)^2 10^(DRR / 10)' in text
    assert '1e8' in text
    assert 'd^3' in text
    # the hard-limits section stays exactly as before
    assert '- T60 (the reverberant decay\'s own time constant): 0.1 to 10 seconds.\n' in text
    assert '- DRR (direct-to-reverberant energy ratio, dB): -40 to +30 dB.\n' in text
    # the wording stays product physics, never the harness's own test machinery
    lowered = text.lower()
    assert 'control' not in lowered
    assert 'verdict' not in lowered
    assert 'dry run' not in lowered and 'dry-run' not in lowered


def test_implied_volume_formula_matches_rc_and_drr():
    """r_c = 0.057 sqrt(V / T60), DRR(d) = 20 log10(r_c / d): implied_volume_m3 must invert exactly
    back to the (t60, drr, d) it was built from, for an arbitrary instance, not just the controls."""
    import math
    t60, d = 2.4, 7.5
    v = attack.implied_volume_m3(t60, drr_db=3.3, d_m=d)
    rc = 0.057 * math.sqrt(v / t60)
    drr_back = 20 * math.log10(rc / d)
    assert abs(drr_back - 3.3) < 1e-9


def test_implied_volume_on_the_controls():
    """CONTROL_B (T60 10 s, DRR +30 dB, d 30 m) implies about 2.8e9 m^3 on every one of its draws (its
    box is a single point). CONTROL_A (F2 @ 1 ms) implies at most 1.5e5 m^3 on its 20 draws."""
    b_vals = [attack.implied_volume_m3(i['t60_s'], i['drr_db'], i['d_m'])
              for i in attack.draws(run_d4.CONTROL_B, n=attack.DRAWS)]
    assert all(abs(v / 2.8e9 - 1.0) <= 0.05 for v in b_vals), b_vals

    a_vals = [attack.implied_volume_m3(i['t60_s'], i['drr_db'], i['d_m'])
              for i in attack.draws(run_d4.CONTROL_A, n=attack.DRAWS)]
    assert max(a_vals) <= 1.5e5, max(a_vals)


def test_regenerated_judge_prompts_carry_the_new_section():
    """Step 7d's second half: the two judge prompt files, rebuilt by attack.judge_prompt itself (as
    dry/run_d4.py's write_prompt does), must carry PHYSICS.md's new section verbatim."""
    for cls in (run_d4.CONTROL_A, run_d4.CONTROL_B):
        ok, why = attack.validate_class(cls)
        assert ok, why
    prompt = attack.judge_prompt({'x.json': __import__('json').dumps(run_d4.CONTROL_A)})
    assert 'Diffuse-field consistency' in prompt
    assert '1e8' in prompt
