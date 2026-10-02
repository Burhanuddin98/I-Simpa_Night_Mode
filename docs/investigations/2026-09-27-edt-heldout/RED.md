# M8b EDT harness: the red check (plan section 6, step 3)

2026-10-01, branch `m8b-edt`. Two checks, each by an independent subagent that wrote none of the stubs, tests or
fixes (HARNESS-PLAN.md section 4): the first 04:55-05:25 at `99269a3`, the second 05:35-05:55 at `6d4f39b`,
after the fixes. No solver ran, no held-out row was read or made, and every scratch file and record is on C:,
under `C:\tmp\m8b-edt\`.

**In plain words.** Step 3's red check is passed. Against the empty stubs, the suite fails exactly as it should:
21 tests are red, each on its own assertion, and the 2 controls pass. The first check found 4 tests that no
correct harness could pass, each because of one wrong line: T4 (upstream's EDT), T9 (truth uncertainty), T15
(Synth-fresh) and T17 (the attack sandbox). They were fixed at 05:27. The second check confirmed each fix three
ways: the old line fails correct code for the stated reason, the new line passes it, and the new line still
catches wrong code planted to test it. The method file hashes as PREREG pins it, now on every fresh checkout.
Nothing here is Burhan's to decide. One point in the plan stays open for before A1 (8.1, Synth-fresh just under
DRR +9.54 dB), and the second check adds that it depends on the reflection gap as well as the DRR (last section).

The sections below, up to the dated one, are the first check's record. The commands and the table also carry the
second check, and the table's rows for T4, T9, T15, T16 and T17 are brought up to date.

## How it was checked (first check)

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

From `harness/`, in PowerShell, with the venv.

First check:

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

Second check. The throwaway code is the first check's `rc_impl.py`, unchanged since 05:15:36 (sha256
`a9914154…`). `C:\tmp\m8b-edt\redcheck2\rc2_trace.py` (sha256 `ac373d76…`, never committed) is a plugin loaded
after it. It only wraps the functions the four fixed tests call and records what they saw in
`redcheck2\trace.jsonl`; with `RC2_MUT` set, it plants one fault. Records are in `C:\tmp\m8b-edt\redcheck2\`
unless named otherwise.

```
$env:PYTHONDONTWRITEBYTECODE='1'
# 1. the stubs (record: C:\tmp\m8b-edt\step3-redcheck2-run1.txt; 05:40:22-05:42:29, 21 failed, 2 passed, 126.8 s)
C:\tmp\m8b-edt\venv\Scripts\python.exe -m pytest -v --tb=short -rA
# 2. the four fixed tests against the throwaway code, traced (record: run-fixed4.txt; 05:44:56-05:44:57, 4 passed)
$env:PYTHONPATH='C:\tmp\m8b-edt\redcheck;C:\tmp\m8b-edt\redcheck2'
C:\tmp\m8b-edt\venv\Scripts\python.exe -m pytest -p rc_impl -p rc2_trace -v --tb=short -rA -k "t04 or t09 or t15 or t17"
# 3. ten planted faults, one run each (records: run-mut-<fault>.txt; 05:45:23-05:45:28), <test> its test (t04 ...)
#    faults: t4_byte t4_crlf t9_pstdev t9_tiny t15_allnan t15_off t15_finite_for_nan t15_none t17_method t17_interface
$env:RC2_MUT='<fault>'
C:\tmp\m8b-edt\venv\Scripts\python.exe -m pytest -p rc_impl -p rc2_trace --tb=short -rA -k <test>
# 4. bc34651's conftest and four test files (git show bc34651:<path>) against the same code
#    (record: run-prefix4.txt; 05:46:01-05:46:02, 4 failed)
$env:PYTHONPATH='C:\tmp\m8b-edt\redcheck;<this folder>\harness'
cd C:\tmp\m8b-edt\redcheck2\tests_old
C:\tmp\m8b-edt\venv\Scripts\python.exe -m pytest -p rc_impl -p no:cacheprovider --basetemp=C:/tmp/m8b-edt/redcheck2/pt-tmp -v --tb=short -rA -k "t04 or t09 or t15 or t17" test_upstream.py test_truth.py test_fresh_sets.py test_attack.py
# 5. the rest as committed against the same code (record: run-impl-all.txt; 05:46:18-05:47:19, 21 passed)
$env:PYTHONPATH='C:\tmp\m8b-edt\redcheck'
C:\tmp\m8b-edt\venv\Scripts\python.exe -m pytest -p rc_impl -v --tb=short -rA -k "not t22a and not t11"
# 6. the frozen file as each git setting checks it out (Git Bash, from the worktree)
git -c core.autocrlf=<false|input|true> archive --format=tar <HEAD|bc34651> -- <this folder>/frozen/method.py | tar -xO | sha256sum
```

## One row per test

"Red" is the stub run. The second check's run at `6d4f39b` gives the same line for every test, except T16's,
one line lower because T15's fix added a line above it. "Correct code" is the test as committed against the
throwaway implementations: for T4, T9, T15 and T17 the test as fixed in `3964e54` (second check), with the first
check's result before the fix.

| Test | Red, at | Message | Correct code | Verdict |
|---|---|---|---|---|
| T1 | passes | control: `frozen/method.py` hashes to `462c37cf…` | passes | control, passes |
| T2 | test_method.py:29 | `method.load accepted a copy with one byte changed (returned None)` | passes | right red |
| T3 | test_method.py:42 | `method.load / load_z3 returned nothing` | passes | right red |
| T4 | test_upstream.py:35 | `no copy of the port at …\harness\m8b\_upstream_edt.py` (fixture hashes at :28-32 passed first) | passes since `3964e54`, on the port's own LF bytes; before it, failed at :37 (defect 1) | right red; fixed, checked again |
| T5 | test_upstream.py:68 | `upstream.analyse returned nothing` | passes | right red |
| T6 | test_truth.py:33 | `ism\|t1\|(0.4, 0.4, 0.35)\|1000\|1ms: ism_fresh.make_row returned nothing` | passes | right red |
| T7 | test_truth.py:66 | `truth.truth_ideal returned nothing` | passes | right red |
| T8 | test_truth.py:91 | `truth.split returned nothing` | passes | right red |
| T9 | test_truth.py:104 | `assert None == 'ok'` (verdict at the kept edges) | passes since `3964e54`, with u = 0.0 exactly on the identical references; before it, ZeroDivisionError at :130, reached from :148, raised in `rel` at :20 (defect 2) | right red; fixed, checked again |
| T10 | test_rooms.py:112 | `rooms.rooms() returned nothing` (the corpus-kind receipts at :109 passed first) | passes | right red |
| T11 | test_rooms.py:236 | `rooms.rooms() returned nothing` | passes: all nine projects import, `simpa validate` exit 0, `simpa check` exit 0 | right red |
| T12 | test_driver.py:48 | `driver.plan() returned nothing` | passes | right red |
| T13 | test_driver.py:154 | `the verified build must pass the check` (the flipped spps.exe was built first) | passes | right red |
| T14 | test_driver.py:218 | `driver.series_from_report returned nothing` | passes | right red |
| T15 | test_fresh_sets.py:47 | `synth_fresh.draw returned nothing` | passes since `3964e54`, through the NaN row `synth-1.5-1-252`; before it, failed at :94 on that row, `rel(nan, nan)` (defect 3) | right red; fixed, checked again |
| T16 | test_fresh_sets.py:117 (:116 before `3964e54`) | `ism_fresh.draw returned nothing` | passes | right red |
| T17 | test_attack.py:29 | `a class inside PHYSICS.md's limits must pass, got None` | passes since `3964e54`; before it, failed at :69, `method.py names a tuning log` (defect 4) | right red; fixed, checked again |
| T18 | test_score.py:25 | `score.classify returned nothing` | passes | right red |
| T19 | test_score.py:74 | `score.tally returned nothing` | passes | right red |
| T20 | test_score.py:118 | `score.h3 returned nothing` | passes | right red |
| T21 | test_score.py:208 | `the evaluation ran with a method file whose hash differs` | passes | right red |
| T22a | passes | control: step 2's `corpus.py` rebuilt `weak_spots.json` byte for byte | not rerun | control, passes |
| T22b | test_weak_spots.py:55 | `synth_fresh.BOUNDS has no bound for ['t60_s', 'late_share_db', 'R_m', 'd_m', 'd_minus_R_m', 'gap_ms', 'delay_ms']` | passes, with the bounds widened as below | right red |

Every red in both stub runs is an `AssertionError` raised by the test's own `assert`, on a stub's `None` or on an
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

Line numbers here are those of `bc34651`. Each was fixed in `3964e54` with the line named here; the dated
section below records the second check of each fix.

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
9 of 9). Nothing else in them blocks a correct harness. The corrected copies differ from the tests as committed
at `bc34651` in those lines only.

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

## 2026-10-01, 05:35-05:55: the fixes, checked again

The second check ran at `6d4f39b`, by a subagent that wrote none of the tests, the fixes or the throwaway
code. It took each fix to be wrong until it saw otherwise.

**What changed.** `git diff bc34651..HEAD` touches 7 files. Under `harness/` it touches only the five files of
`3964e54`, and each change is the line that defects 1-4 name:

- T4: `test_upstream.py:37` hashes the copy's own LF bytes. The docstring (:25) and `m8b/upstream.py:9-11`
  now say the port is LF. The port in `target/` has 247 LF and 0 CR, and it hashes `57f391e6…` as it is and
  `5a44523c…` as CRLF. `harness/.gitattributes` keeps `*.py` LF, as the corrected contract says.
- T9: `test_truth.py:130-131` compares u with `math.isclose(…, rel_tol=1e-12, abs_tol=0.0)`.
- T15: `test_fresh_sets.py:94-95` accepts NaN against NaN. It now also fails a `None` truth with the row's id,
  where the old line would have raised a `TypeError`.
- T17: `test_attack.py:68-69` leaves `method.py` out of the token scan. Lines 55, 57 and 58 still hold the
  sandbox to exactly the four files, with `method.py` equal to `frozen/method.py` byte for byte.

Apart from wrapping, comments and T15's `None` guard, these are the first check's corrected lines
(`C:\tmp\m8b-edt\redcheck\tests_mod\`). The other two files are the `.gitattributes` beside this file
(`00e793e`) and section 8.1 of the plan (`6d4f39b`).

**Against the stubs.** 21 tests are red and the 2 controls pass, as in the first run. Each red is an
`AssertionError` from the test's own `assert`, at the line the table gives. No test errored in setup, and none
failed on an import, a `TypeError`, a `KeyError` or a missing fixture. The four fixed tests are red where they
were before (T4 at :35, T9 at :104, T15 at :47, T17 at :29), because each fixed line comes after the first
assertion a stub fails.

**Each fix, checked three ways on the first check's throwaway code.** I read `rc_impl.py` where these four
tests reach it: the upstream copy and `edt_table`; `truth`'s `read`, `uncertainty`, `verdict` and `assess`;
`synth_fresh`'s `draw` and `truth`; and `attack.build_sandbox`.

1. The old lines (`bc34651`) fail it, each for the reason defects 1-4 give. T4 fails at :37 (`5a44523c…`
   against `57f391e6…`). T9 raises a `ZeroDivisionError` in `rel` (:20), from :130, reached from :148. T15
   fails at :94 with `rel(nan, nan)` on `synth-1.5-1-252`. T17 fails at :69 with `method.py names a tuning log`.
2. The fixed lines pass it, 4 of 4. The trace shows which branch each line took.
   - T4 hashed the port's own bytes: `57f391e6…`, with no CRLF.
   - In T9's third case, four identical references gave four EDTs of 0.7597803003396266 s and u = 0.0
     exactly, so the comparison against an expected 0 ran. numpy's `std(ddof=1)` also gives exactly 0 here, and
     on 200,000 random sets of four equal values. An implementation that used it instead of
     `statistics.stdev` is not trapped either.
   - T15 compared `synth-1.5-1-0`, `-1`, `-2`, `-250`, `-251` and `-252`. The last has a DRR of 9.98 dB and a
     NaN truth on both sides, so the run reached a NaN row and passed it.
   - T17's sandbox `method.py` is 4,622 bytes, has sha256 `462c37cf…` and names `FINAL.md`. The other three
     files do not name it.
3. The fixed lines still fail wrong code. Ten planted faults were run, one per run:

| Planted fault | Result |
|---|---|
| T4: the copy with one byte changed | fails at test_upstream.py:37 |
| T4: the copy in CRLF | passes: "line ends aside" is the contract, and `harness/.gitattributes` keeps the real copy LF |
| T9: u from the population SD (ddof 0) | fails at test_truth.py:130, in the first case (:137) |
| T9: u = 1e-18 where the SD is 0 | fails at :130, in the third case (:149) |
| T15: every truth NaN | fails at test_fresh_sets.py:95, on `synth-1.5-1-0` |
| T15: every truth 1e-9 high | fails at :95, on `synth-1.5-1-0` |
| T15: a finite truth where the reference is NaN | fails at :95, on `synth-1.5-1-252` |
| T15: no truth (`None`) | fails at :95, on `synth-1.5-1-0` |
| T17: the sandbox's `method.py` with one bit flipped | fails at test_attack.py:58 |
| T17: `INTERFACE.md` naming `../FINAL.md` | fails at :71 |

Run as committed against the same code, the rest pass too: 21 of 21 selected, which are T1, the four fixed
tests and the other 16 red tests. T11 was left out because it runs `simpa import`, `validate` and `check`, which
this check does not name (the first check saw it pass). T22a was left out because it is the control and passed
in the stub run.

**The frozen method.** The working copy hashes `462c37cf…` and is CRLF throughout (91 lines). `git archive` of
`HEAD` gives `462c37cf…` under `core.autocrlf` false, input and true. At `bc34651` it gave `789053c9…`, the
committed LF blob, under false and input. `PREREG.md` and `frozen/method.py` have the same blobs at `e9c8208`
and `HEAD` (`b4dae808…`, `fcdb1e4a…`), and `e9c8208` is the only commit that touches them.

The fix has one limit, which I tested in a scratch repo. A checkout made under autocrlf false or input before
`00e793e` keeps its LF file after the pull, and `git status` shows it clean, until that file is checked out
again (delete it, then `git checkout -- <file>`). On such a checkout the hash gate raises VoidRun, so the
failure is loud. Grace runs autocrlf true.

**The plan's section 8.1, checked.**

- The pin is as above. One wording slip: the bullet names `../.gitattributes`, but the file is the
  `.gitattributes` beside the plan, in the parent folder of `frozen/`. No test reads that sentence.
- The DRR edge holds as stated. The throwaway dev draw has 61 NaN truths in 4,000, which are exactly the rows
  above 10·log10(9) = 9.5424 dB (the lowest NaN row is at 9.547 dB and the highest finite one at 9.539 dB). But
  "28 s at 9.50 dB for a 1 s decay", in 8.1 and in defect 3, holds only for a 5 ms reflection gap. For a 1 s
  single slope with no gap, the truth is 1.0 s at every DRR below the edge. With a 40 ms gap, the top of P24's
  range, it is already 13.3 s at 9.0 dB, and 1,380 s at 9.50 dB. How far below the edge the truth departs from
  the decay therefore depends on the gap, and whoever examines these rows for H1 needs to know that. It does
  not block step 4.
- P3's figures are wrong, as 8.1 says. The largest per-axis gap to the nearest corpus box is 30.4 % for F1
  (z3's box028), 22.0 % for F2 (box096), 17.1 % for F5 (box055), 31.2 % for F6 (M8a's 20x8x4) and 24.0 % for
  F7 (ISM's dead). That makes five boxes at 17.1-31.2 %, all clear of P3's 10 %. P3's "four fresh boxes …
  18-44 %" is wrong on both counts.

**Left as found.** No solver ran. Nothing exists under `C:\tmp\m8b-edt\heldout\`, and there is no launch log.
Before and after, the worktree was clean and held 1,246 files: the first check's 1,245 plus `00e793e`'s
`.gitattributes`. C: had 20.1 GB free at 05:51.

**Verdict: passed.** Against the stubs, every test except T1 and T22a is red on its own assertion, and both
controls pass. T4, T9, T15 and T17 pass against code written from the plan, and they still fail the wrong code
planted to test them.
