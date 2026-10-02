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
