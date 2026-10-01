# M8b EDT harness: the red check (plan section 6, step 3)

2026-10-01, 04:55-05:25, branch `m8b-edt` at `99269a3`. Checked by an independent subagent that did not write
the stubs or the tests (HARNESS-PLAN.md section 4). No solver ran, no held-out row was read or made, and every
scratch file and record is on C:, under `C:\tmp\m8b-edt\`.

**In plain words.** Against the empty stubs, the suite fails exactly as it should: 21 tests are red, each on its
own assertion, and the 2 controls pass. But 4 of the tests are wrong: no correct harness could pass them, each
because of one line in the test. They are T4 (upstream's EDT), T9 (truth uncertainty), T15 (Synth-fresh) and
T17 (the attack sandbox). With that one line corrected, each passes. Throwaway code written from the plan passes
the other 17 red tests unchanged. **Step 3 is not done.** The four lines, and one wrong sentence in
`upstream.py`'s contract, need fixing, and those four tests need their red seen again. Nothing here is
Burhan's to decide.

## How it was checked

1. The suite was run against the stubs as committed.
2. The nine stubs were read in full (below).
3. **Can each test pass?** I wrote throwaway implementations of all nine modules from HARNESS-PLAN.md and the
   stubs' docstrings, without reading the writer's scratch. A pytest plugin replaces the stub functions at
   start-up, and the tests ran against them **unmodified**. The code lives only at
   `C:\tmp\m8b-edt\redcheck\rc_impl.py` and is never committed. Its `launch()` raises after its guards, so it
   cannot start a run.
4. For each of the four tests that failed in 3, I ran a copy with one line corrected against the same code, to
   see whether anything else in it is wrong (`C:\tmp\m8b-edt\redcheck\tests_mod\`, made by `make_mods.py`
   there, never committed).

## Commands

From `harness/`, in PowerShell, with the venv:

```
$env:PYTHONDONTWRITEBYTECODE='1'
# 1. the stubs (record: C:\tmp\m8b-edt\step3-redcheck-run1.txt; 05:02:18-05:04:24, 21 failed, 2 passed, 126 s)
C:\tmp\m8b-edt\venv\Scripts\python.exe -m pytest -v --tb=short -rA
# 3. the unmodified tests against the throwaway code (record: C:\tmp\m8b-edt\redcheck\run-impl1.txt;
#    05:15:44-05:16:45, 18 passed, 4 failed, T22a deselected because it does not use the stubs)
$env:PYTHONPATH='C:\tmp\m8b-edt\redcheck'
C:\tmp\m8b-edt\venv\Scripts\python.exe -m pytest -p rc_impl -v --tb=short -rA -k "not t22a"
# 4. the four test files with one line corrected (record: C:\tmp\m8b-edt\redcheck\run-mod1.txt; 9 passed, 55 s)
$env:PYTHONPATH='C:\tmp\m8b-edt\redcheck;<this folder>\harness'
cd C:\tmp\m8b-edt\redcheck\tests_mod
C:\tmp\m8b-edt\venv\Scripts\python.exe -m pytest -p rc_impl -p no:cacheprovider --basetemp=C:/tmp/m8b-edt/redcheck/pt-tmp -v --tb=short -rA test_upstream_mod.py test_attack_mod.py test_fresh_sets_mod.py test_truth_mod.py
```

## One row per test

"Red" is the stub run. "Correct code" is the unmodified test against the throwaway implementations.

| Test | Red, at | Message | Correct code | Verdict |
|---|---|---|---|---|
| T1 | passes | control: `frozen/method.py` hashes to `462c37cf…` | passes | control, passes |
| T2 | test_method.py:29 | `method.load accepted a copy with one byte changed (returned None)` | passes | right red |
| T3 | test_method.py:42 | `method.load / load_z3 returned nothing` | passes | right red |
| T4 | test_upstream.py:35 | `no copy of the port at …\harness\m8b\_upstream_edt.py` (fixture hashes at :28-32 passed first) | **fails at test_upstream.py:37** | **cannot pass: defect 1** |
| T5 | test_upstream.py:68 | `upstream.analyse returned nothing` | passes | right red |
| T6 | test_truth.py:33 | `ism\|t1\|(0.4, 0.4, 0.35)\|1000\|1ms: ism_fresh.make_row returned nothing` | passes | right red |
| T7 | test_truth.py:66 | `truth.truth_ideal returned nothing` | passes | right red |
| T8 | test_truth.py:91 | `truth.split returned nothing` | passes | right red |
| T9 | test_truth.py:104 | `assert None == 'ok'` (verdict at the kept edges) | **ZeroDivisionError at test_truth.py:130**, reached from :148, raised in `rel` at :20 | **cannot pass: defect 2** |
| T10 | test_rooms.py:112 | `rooms.rooms() returned nothing` (the corpus-kind receipts at :109 passed first) | passes | right red |
| T11 | test_rooms.py:236 | `rooms.rooms() returned nothing` | passes: all nine projects import, `simpa validate` exit 0, `simpa check` exit 0 | right red |
| T12 | test_driver.py:48 | `driver.plan() returned nothing` | passes | right red |
| T13 | test_driver.py:154 | `the verified build must pass the check` (the flipped spps.exe was built first) | passes | right red |
| T14 | test_driver.py:218 | `driver.series_from_report returned nothing` | passes | right red |
| T15 | test_fresh_sets.py:47 | `synth_fresh.draw returned nothing` | **fails at test_fresh_sets.py:94**, `rel(nan, nan)` | **can fail correct code: defect 3** |
| T16 | test_fresh_sets.py:116 | `ism_fresh.draw returned nothing` | passes | right red |
| T17 | test_attack.py:29 | `a class inside PHYSICS.md's limits must pass, got None` | **fails at test_attack.py:69**, `method.py names a tuning log` | **cannot pass: defect 4** |
| T18 | test_score.py:25 | `score.classify returned nothing` | passes | right red |
| T19 | test_score.py:74 | `score.tally returned nothing` | passes | right red |
| T20 | test_score.py:118 | `score.h3 returned nothing` | passes | right red |
| T21 | test_score.py:208 | `the evaluation ran with a method file whose hash differs` | passes | right red |
| T22a | passes | control: step 2's `corpus.py` rebuilt `weak_spots.json` byte for byte | not rerun | control, passes |
| T22b | test_weak_spots.py:55 | `synth_fresh.BOUNDS has no bound for ['t60_s', 'late_share_db', 'R_m', 'd_m', 'd_minus_R_m', 'gap_ms', 'delay_ms']` | passes, with the bounds widened as below | right red |

Every red in the stub run is an `AssertionError` raised by the test's own `assert`, on a stub's `None` or on an
empty `BOUNDS`. None is an import error, a `TypeError` or `KeyError` from a `None`, a missing fixture, or a
setup step. The session fixtures exist: `simpa.exe` at `C:\tmp\nm-target\release\simpa.exe`, and the solver
folder `C:\tmp\nm-m8a-solvers`, whose four executables match `solvers/manifest.json`'s code sha256. The plan's
T22 is red as a whole through T22b. T22a is the half that step 2 built, and it passes as a control.

## The stubs

All nine (`method`, `upstream`, `truth`, `rooms`, `driver`, `ism_fresh`, `synth_fresh`, `attack`, `score`)
were read in full. Every function body is `return None`, and both `BOUNDS` are `{}`. Beyond that they hold only
constants the tests read (paths, seeds, bands, steps, limits), `driver.Refused` (an exception that carries a
code), and `method.py`'s import of `FROZEN`, `FROZEN_SHA256` and `VoidRun` from `corpus.py`, which is step 2's
code and not a stub. They implement nothing.

## The four defects

1. **T4, test_upstream.py:36-37.** The test makes the copy LF, turns every LF into CRLF, and expects sha256
   `57f391e6…`. But the port, `target/agents/upstream-edt/uphunt_upstream_edt.py`, is LF throughout (247 LF,
   0 CRLF), and its own bytes hash to `57f391e6…`. In CRLF they hash to `5a44523c…`, so a byte-exact copy
   fails. `upstream.py:9-11` states the same wrong premise ("Its source is CRLF throughout"), and so does T4's
   docstring (test_upstream.py:25). Fix: `assert sha256_bytes(lf) == PORT_SHA256`, and correct both texts.
   `oracle.json` really is CRLF in `target/`, so the fixture handling at :28-32 is right.
2. **T9, test_truth.py:130, with :20 and :146-148.** The third case plants four identical references
   (`planted((0.8, 0.8, 0.8, 0.8), 0.25)`). Their EDTs are identical, so u is exactly 0 for any implementation;
   `statistics.stdev` and numpy's `std(ddof=1)` both give 0.0. `check_u` then computes `rel(0.0, 0.0)`, which
   divides by zero. Fix: compare u with
   `math.isclose(u, want, rel_tol=1e-12, abs_tol=0.0)`, which still holds the other cases to 1e-12.
3. **T15, test_fresh_sets.py:92-94.** The PREREG's DRR range reaches +10 dB (PREREG.md:39). The synth truth,
   `critique/synth.py` `truth_edt` (P15's Definition A), is NaN for every DRR above 10·log10(9) = 9.54 dB,
   because the direct sound alone takes the level past −10 dB. My throwaway draw with the dev seed has 61 such
   rows in 4,000. `rel(nan, nan)` is NaN, so the test fails whenever one of the six rows it samples
   (`rows[:3] + delayed[:3]`) lies above 9.54 dB. That depends only on the implementation's draw order: about 1
   in 11 orders. Mine hit it at row `synth-1.5-1-252`, DRR 9.98 dB. Fix: accept NaN against NaN at :94.
   Near the edge the truth also grows very large (28 s at DRR 9.50 dB for a 1 s decay, 7,061 s at 9.54 dB).
   This is for the plan's owner, and was not examined further.
4. **T17, test_attack.py:67-69 against :58.** Line 58 requires the sandbox's `method.py` to be
   `frozen/method.py` byte for byte. Lines 67-69 forbid any sandbox file to contain `b'FINAL.md'`, and
   `frozen/method.py:1` reads `"""EDT for SPPS receiver histograms: the graft recommended in ../FINAL.md.`
   Both cannot hold, and `frozen/` may not change. Fix: leave `method.py` out of the token scan, since line 58
   already pins its bytes.

With each of these lines corrected, T4, T9, T15 and T17 pass against the throwaway implementations (run 4,
9 of 9). Nothing else in them blocks a correct harness. The corrected copies differ from the committed tests in
those lines only.

## What the throwaway code showed, for steps 4 and 6

- **T10.** A doorway at y 2.9-4.9 m, z 0-2.5 m gives F3's blocked flags as 2.2 lists them. P0b as a box of
  2.7 × 4.12 × 2.7 m (x, y, z) meets T10: V −0.2 %, S −0.1 %, every axis 12-29 % off F7, and not a P3
  near-duplicate. F3's materials come to 249.0 m² at 0.40 and 184.8 m² at 0.04, with the doorway floor
  counted as a reveal.
- **T11.** The following passes `simpa validate` and `simpa check` on all nine rooms: an OBJ on one global grid,
  then `simpa import --unit m --up z`, then JSON edits. The edits are Lambert materials, one source and 8
  receivers from `tutorial1_box.simpa`'s prototypes, seed 1, and intersection files and fittings off. A fresh
  import's `time_step_s` is 0.01 and T11 holds the room projects to it, so each run's step has to be set by the
  driver's per-run project.
- **T22b binds four bounds, not three.**
  - Synth `d_m` must cover 0.1716-30.80 m, and `d_minus_R_m` must go down to −0.138 m (scan 1's early run).
  - ISM `src_wall_m` must go down to 0.703 m (z3grid `dis-010`).
  - Synth `delay_ms` must go up to 60.0000028 ms: z3 `dis-012`, `dis-016` and `dis-029` are 60 ms as float32.
    That widening is only 3e-6 ms, but without it T22b fails.
- **T22a stops passing once step 6 adds its copies to `m8b/provenance.json`.** `weak_spots.json` carries
  `check_provenance`'s output (`corpus.py:1691`) and the hash of `provenance.json`. Regenerating
  `weak_spots.json` then keeps T22a a control, and its rows do not change.

## The writer's claims, checked

- 21 failed and 2 passed, at the lines given, each an `AssertionError` on the stub's result: confirmed.
- T6-T9, T10, T12 and T18-T20 shown satisfiable: confirmed except T9, which no implementation can pass
  (defect 2). T4, T5, T11, T13, T14, T15, T16, T17, T21 and T22b were unshown. T4, T15 and T17 have defects
  1, 3 and 4. The rest pass against the throwaway code.
- T22b forces three bounds wider: confirmed, but there is a fourth (`delay_ms`, above), so "no other bound
  binds" is wrong.
- `PREREG.md` and `frozen/` are unchanged since `e9c8208`. No solver ran (`simpa import`, `validate` and
  `check` only, inside T11). Nothing exists under `C:\tmp\m8b-edt\heldout\`, and there is no launch log. The
  worktree is clean, with no untracked or ignored file and 1,245 files before and after. C: had 20.0 GB free
  at 05:20.
