# Round 2 harness: RED record (T23-T43 written first, each run and seen failing)

Per `HARNESS-PLAN-2.md` section 9 M5: no separate RED subagent. The builder writes each test, runs it against the
copy of round 1's harness (`harness2/`, committed unchanged at `b7aff7d` apart from LF endings), and records the
failure here before implementing. Command, from `harness2/` with the venv
(`PYTHONDONTWRITEBYTECODE=1 C:\tmp\m8b-edt\venv\Scripts\python.exe -m pytest <file> -q --no-header --tb=line`).
Each block below is the tool's own output for that batch; a test that passes before the code exists is named as a
control. Nothing in this file was edited after the run it records.

## Batch 1, 2026-10-02 00:55: T23-T31 and T38 (tests/test_r2_scorer.py)

Result: 9 failed, 1 passed. The pass is T29, the planned control (no radius filter exists in round 1 either; it must still pass after).

```
FFFFFF.FFF                                                               [100%]
================================== FAILURES ===================================
E   AssertionError: assert None == '029d90ac5e8f6a634a51a7ffce136cbd4c0b7ea3d3ad8a18b78fd8240ff224a0'
     +  where None = getattr(corpus, 'FROZEN2_SHA256', None)
tests\test_r2_scorer.py:34: AssertionError: assert None == '029d90ac5e8f6a634a51a7ffce136cbd4c0b7ea3d3ad8a18b78fd8240ff224a0'
E   TypeError: load() got an unexpected keyword argument 'pin'
tests\test_r2_scorer.py:59: TypeError: load() got an unexpected keyword argument 'pin'
E   AssertionError: round 2 loads no Z = 3 instance
    assert not True
     +  where True = hasattr(method, 'load_z3')
tests\test_r2_scorer.py:100: AssertionError: round 2 loads no Z = 3 instance
E   AssertionError: assert True is False
     +  where True = <function _in_h4 at 0x000002045D802660>({'set': 'spps', 'id': 'r', 'status': 'ok', 'edt': 1.0, ...})
     +    where <function _in_h4 at 0x000002045D802660> = score._in_h4
     +    and   {'set': 'spps', 'id': 'r', 'status': 'ok', 'edt': 1.0, ...} = row(R_m=0.5000001)
tests\test_r2_scorer.py:122: AssertionError: assert True is False
E   KeyError: 'n_before'
tests\test_r2_scorer.py:149: KeyError: 'n_before'
E   AttributeError: module 'm8b.score' has no attribute 'rtl_by_r_class'
tests\test_r2_scorer.py:173: AttributeError: module 'm8b.score' has no attribute 'rtl_by_r_class'
E   KeyError: 'R_m'
tests\test_r2_scorer.py:231: KeyError: 'R_m'
E   KeyError: 'R_m'
tests\test_r2_scorer.py:248: KeyError: 'R_m'
E   AssertionError: assert ('ok' == 'refused'
      
      - refused
      + ok)
tests\test_r2_scorer.py:284: AssertionError: assert ('ok' == 'refused'
=========================== short test summary info ===========================
FAILED tests/test_r2_scorer.py::test_t23_frozen2_hash_z_and_line_ends - Asser...
FAILED tests/test_r2_scorer.py::test_t24_the_loader_is_pinned_per_round - Typ...
FAILED tests/test_r2_scorer.py::test_t25_evaluate_has_two_instances_and_no_z3
FAILED tests/test_r2_scorer.py::test_t26_h4_filter_step_t60_and_radius - Asse...
FAILED tests/test_r2_scorer.py::test_t27_h4_denominator_drops_receiver_too_large_and_counts_it
FAILED tests/test_r2_scorer.py::test_t28_refusals_by_reason_set_and_radius_class
FAILED tests/test_r2_scorer.py::test_t30_upstream_is_scored_on_the_same_ids_and_modes_stay_apart
FAILED tests/test_r2_scorer.py::test_t31_rows_carry_r_m_and_pass_check_inputs
FAILED tests/test_r2_scorer.py::test_t38_dead_room_is_refused_by_round_2_and_wrong_under_round_1
9 failed, 1 passed in 23.08s
```

