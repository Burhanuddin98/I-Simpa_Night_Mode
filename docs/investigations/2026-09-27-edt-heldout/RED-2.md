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
```

