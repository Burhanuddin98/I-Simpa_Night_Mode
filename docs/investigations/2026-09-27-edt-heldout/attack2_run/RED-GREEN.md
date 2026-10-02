# RED-GREEN: run_h6.py

## RED, 2026-10-02, tests written before run_h6.py existed

`python -m pytest test_run_h6.py` (run_h6.py absent), 8 tests, none could run:

```
test_run_h6.py:18: in <module>
    import run_h6                                   # noqa: E402  (imports harness2 read-only)
    ^^^^^^^^^^^^^
E   ModuleNotFoundError: No module named 'run_h6'
=========================== short test summary info ===========================
ERROR test_run_h6.py
!!!!!!!!!!!!!!!!!!! Interrupted: 1 error during collection !!!!!!!!!!!!!!!!!!!!
============================== 1 error in 0.11s ===============================
```

## GREEN, 2026-10-02, run_h6.py written

One test defect fixed first (the noise test divided 0 by 0 on the blocks before the emission, where there are no counts; now it asserts those blocks are exactly zero and skips the z-score). Then:

```
collecting ... collected 8 items

test_run_h6.py::test_equivalence_with_synth_fresh_bit_for_bit PASSED     [ 12%]
test_run_h6.py::test_draws_reproducible_and_seeded PASSED                [ 25%]
test_run_h6.py::test_delay_rounded_up_to_whole_step_as_arrival_s PASSED  [ 37%]
test_run_h6.py::test_noise_reproducible_and_mean_converges PASSED        [ 50%]
test_run_h6.py::test_ism_class_not_implemented PASSED                    [ 62%]
test_run_h6.py::test_h6_wiring_on_planted_classes PASSED                 [ 75%]
test_run_h6.py::test_vote_parsing_of_judge_files PASSED                  [ 87%]
test_run_h6.py::test_classes_load_and_validate PASSED                    [100%]

============================== 8 passed in 0.40s ==============================
```

Also: `python -m m8b.freeze2 --check ../ADDENDUM-B1.md` -> every hash reproduced; `python -m m8b.corpus2 --check` -> all reproduced; `run_h6.verify_frozen()` -> method sha256 029d90ac...24a0. main() was not run on any class.
