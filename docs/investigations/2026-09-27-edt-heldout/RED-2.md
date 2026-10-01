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

## Batch 2, 2026-10-02 00:58: T32-T35 (tests/test_r2_corpus_rooms.py, tests/test_r2_plan_guards.py)

Result: 5 failed. rooms2, corpus2, freeze2 and round2 exist only as stubs (NotImplementedError / missing names); the run list lines the dry run of the unchanged runner printed (172 F-room runs) are dropped from the block below. Nothing was created on B: (T35 checks).

```
FFFFF                                                                    [100%]
================================== FAILURES ===================================
E   FileNotFoundError: [Errno 2] No such file or directory: 'B:\\repos\\I-Simpa_Night_Mode\\.claude\\worktrees\\m8b-edt\\docs\\investigations\\2026-09-27-edt-heldout\\\corpus_rooms_2.json'
C:\Program Files\Python313\Lib\pathlib\_local.py:537: FileNotFoundError: [Errno 2] No such file or directory: 'B:\\repos\\I-Simpa_Night_Mode\\.claude\\worktrees\\m8b-edt\\docs\\investigations\\2026-09-27-edt-heldout\\\corpus_rooms_2.json'
E   NotImplementedError: rooms2.rooms
m8b\rooms2.py:6: NotImplementedError: rooms2.rooms
E   AssertionError: assert ({'F1', 'F2', ...5', 'F6', ...} == {'G1', 'G2', ...5', 'G6', ...}
      
      Extra items in the left set:
      'F5'
      'F2'
      'F6'
      'F3'
      'F1'...
      
      ...Full output truncated (11 lines hidden), use '-vv' to show)
tests\test_r2_plan_guards.py:35: AssertionError: assert ({'F1', 'F2', ...5', 'F6', ...} == {'G1', 'G2', ...5', 'G6', ...}
E   AssertionError: 3101
    assert False is True
     +  where False = <function heldout_seed at 0x00000295C75D6B60>(3101)
     +    where <function heldout_seed at 0x00000295C75D6B60> = driver.heldout_seed
tests\test_r2_plan_guards.py:88: AssertionError: 3101
E   Failed: DID NOT RAISE SystemExit
---------------------------- Captured stdout call -----------------------------
M8b held-out queue runner -- dry run (nothing launched, nothing built, nothing read)
data_root: B:\data\m8b-edt\heldout
rooms_root: C:\tmp\m8b-edt\heldout-rooms

Projects (7), built once under rooms_root (never under data_root: rooms.py refuses B:):
  F1 -> C:\tmp\m8b-edt\heldout-rooms\projects\F1.simpa
  F2 -> C:\tmp\m8b-edt\heldout-rooms\projects\F2.simpa
  F3 -> C:\tmp\m8b-edt\heldout-rooms\projects\F3.simpa
  F4 -> C:\tmp\m8b-edt\heldout-rooms\projects\F4.simpa
  F5 -> C:\tmp\m8b-edt\heldout-rooms\projects\F5.simpa
  F6 -> C:\tmp\m8b-edt\heldout-rooms\projects\F6.simpa
  F7 -> C:\tmp\m8b-edt\heldout-rooms\projects\F7.simpa

Runs (172): 28 truth, 144 tested (72 random, 72 energetic)
Order (all truth runs, then tested Random, then tested Energetic):

first run: truth-F1-9001
last run:  tested-F7-energetic-5.0ms-150k-1503
tests\test_r2_plan_guards.py:142: Failed: DID NOT RAISE SystemExit
=========================== short test summary info ===========================
FAILED tests/test_r2_corpus_rooms.py::test_t32_corpus_rooms_2_is_the_widened_list_and_rebuilds_byte_for_byte
FAILED tests/test_r2_corpus_rooms.py::test_t33_g_rooms_meet_the_plan - NotImp...
FAILED tests/test_r2_plan_guards.py::test_t34_plan_is_the_round_2_matrix_with_the_round_2_seeds
FAILED tests/test_r2_plan_guards.py::test_t35_guards_for_round_2 - AssertionE...
FAILED tests/test_r2_plan_guards.py::test_t35_runner_limits_and_paths - Faile...
5 failed in 0.07s
```

## Batch 3, 2026-10-02 01:03: T36, T37, T39-T43 (tests/test_r2_fresh_sets.py, test_r2_dry.py, test_r2_attack.py, test_r2_logging.py, test_r2_freeze.py, test_r2_run_order.py)

Result: 8 failed (T41 has two tests). expected_dry_2.json was committed first (ddda75b). Stubs: dry/run_dry2.py, m8b/freeze2.py. Every failure is a missing function, keyword or name, or a stub, as the plan's 'each module starts as a stub' says.

```
FFFFFFFF                                                                 [100%]
================================== FAILURES ===================================
E   NotImplementedError: run_dry2.run_d2p
dry\run_dry2.py:5: NotImplementedError: run_dry2.run_d2p
E   TypeError: draw() got an unexpected keyword argument 'b1'. Did you mean 'a1'?
tests\test_r2_plan_guards.py:73: TypeError: draw() got an unexpected keyword argument 'b1'. Did you mean 'a1'?
E   TypeError: draw() got an unexpected keyword argument 'b1'. Did you mean 'a1'?
tests\test_r2_plan_guards.py:73: TypeError: draw() got an unexpected keyword argument 'b1'. Did you mean 'a1'?
E   TypeError: build_sandbox() got an unexpected keyword argument 'sentinel'
tests\test_r2_attack.py:32: TypeError: build_sandbox() got an unexpected keyword argument 'sentinel'
E   TypeError: 'NoneType' object is not callable
tests\test_r2_logging.py:22: TypeError: 'NoneType' object is not callable
E   TypeError: rows_for_scoring() got an unexpected keyword argument 'on_receiver'
tests\test_r2_logging.py:54: TypeError: rows_for_scoring() got an unexpected keyword argument 'on_receiver'
E   NotImplementedError: freeze2.inventory
m8b\freeze2.py:5: NotImplementedError: freeze2.inventory
E   AttributeError: <module 'm8b.driver' from 'B:\\repos\\I-Simpa_Night_Mode\\.claude\\worktrees\\m8b-edt\\docs\\investigations\\2026-09-27-edt-heldout\\\m8b\\driver.py'> has no attribute 'b1_committed'
tests\test_r2_run_order.py:22: AttributeError: <module 'm8b.driver' from 'B:\\repos\\I-Simpa_Night_Mode\\.claude\\worktrees\\m8b-edt\\docs\\investigations\\2026-09-27-edt-heldout\\\m8b\\driver.py'> has no attribute 'b1_committed'
=========================== short test summary info ===========================
FAILED tests/test_r2_dry.py::test_t39_dry_run_equals_expected_dry_2 - NotImpl...
FAILED tests/test_r2_fresh_sets.py::test_t36_ism_fresh_2 - TypeError: draw() ...
FAILED tests/test_r2_fresh_sets.py::test_t37_synth_fresh_2 - TypeError: draw(...
FAILED tests/test_r2_attack.py::test_t40_attack_round_2 - TypeError: build_sa...
FAILED tests/test_r2_logging.py::test_t41_ism_rooms_log_start_before_done_and_one_line_per_receiver
FAILED tests/test_r2_logging.py::test_t41_rows_for_scoring_calls_back_once_per_receiver
FAILED tests/test_r2_freeze.py::test_t42_inventory_is_complete_and_check_catches_a_change
FAILED tests/test_r2_run_order.py::test_t43_scorer_run_order - AttributeError...
8 failed in 0.13s
```

## Coverage

| Test | File | Recorded failing in | Control that passed before |
|---|---|---|---|
| T23 | test_r2_scorer.py | batch 1 | |
| T24 | test_r2_scorer.py | batch 1 | |
| T25 | test_r2_scorer.py | batch 1 | |
| T26 | test_r2_scorer.py | batch 1 | |
| T27 | test_r2_scorer.py | batch 1 | |
| T28 | test_r2_scorer.py | batch 1 | |
| T29 | test_r2_scorer.py | | yes: passed before and must after (planned control, plan section 6) |
| T30 | test_r2_scorer.py | batch 1 | |
| T31 | test_r2_scorer.py | batch 1 | |
| T32 | test_r2_corpus_rooms.py | batch 2 | |
| T33 | test_r2_corpus_rooms.py | batch 2 | |
| T34 | test_r2_plan_guards.py | batch 2 | |
| T35 | test_r2_plan_guards.py (two tests) | batch 2 | |
| T36 | test_r2_fresh_sets.py | batch 3 | |
| T37 | test_r2_fresh_sets.py | batch 3 | |
| T38 | test_r2_scorer.py | batch 1 | |
| T39 | test_r2_dry.py | batch 3 | |
| T40 | test_r2_attack.py | batch 3 | |
| T41 | test_r2_logging.py (two tests) | batch 3 | |
| T42 | test_r2_freeze.py | batch 3 | |
| T43 | test_r2_run_order.py | batch 3 | |

T29 is the only test of T23-T43 that did not fail first, and it is a control by design: its claim (no radius filter in
H1, H2, H3, H5) is true of round 1's scorer already and must stay true after H4 gains its filter. T29 was shown able to fail by a mutant (01:03): a pytest plugin that makes `score.tally` drop rows with R_m above 1.0 m
(a radius filter where there must be none) turns it into `assert (True is False)` at tests/test_r2_scorer.py:205
(H1 counting the R = 1.4 m wrong-silent row), `1 failed, 9 deselected`.

## Audit fixes (round 2 closing, 2026-10-02): tests written first

`tests/test_r2_features.py`, written before `m8b/features.py` and before the R_m change; both are recorded failing.

| Test | Covers | Recorded failing |
|---|---|---|
| T44 (seven tests) | audit fix 1: the P6 truth-feature report: ISO T30, G2 (>= 2.5 s), G7 (<= 0.25 s), G3 (T30/EDT >= 1.25 at half the receivers), each met on one planted truth and missed on another; 1 kHz only; unreadable truth is "missed"; RESULTS.md text; `from_runs` reads only G2, G3, G7 truth runs | `ImportError: cannot import name 'features' from 'm8b'`: the whole file fails at collection |
| T46 | audit fix 3: a missing R_m on a Synth or attack row (and through `evaluate`) is a ValueError | with the features import removed so the file loads: `Failed: DID NOT RAISE` at the first `_check_inputs` (tests/_t46_tmp.py:129, a temporary copy, deleted); the seven T44 tests fail with NameError in the same copy |
