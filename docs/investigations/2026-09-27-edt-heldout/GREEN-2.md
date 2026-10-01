# Round 2 harness: GREEN record

Built 2026-10-02 00:44-01:35 on branch `m8b-edt`, from `HARNESS-PLAN-2.md` sections 1-9 (section 9 overriding), `PREREG-2.md` with
amendment 1 (Z = 2.5). **No solver ran, no round-2 truth or tested run exists, no round-2 histogram, echogram or method call was
made on held-out data, the attacker did not run, `ADDENDUM-B1.md` is not written.** Nothing exists under
`B:\data\m8b-edt\round2` or `C:\tmp\m8b-edt\round2-*`; `C:` holds 26 GB free.

**In plain words.** `harness2/` is round 1's harness, copied and edited in place for round 2. It has the seven G rooms, the new
seeds, a corpus list widened to 147 rooms, the v2.1 method behind its own hash, a scorer whose H4 asks only the question Burhan
ruled on, and a freeze tool that hashes every file. All 97 tests pass (74 carried or adapted from round 1, 23 new for T23-T43).
Two dry checks pass with no mismatch: the planted D2' run (97 values against numbers written before it ran) and the scorer on run 1's
inputs against `v21_zsweep.txt`'s Z = 2.5 block (80 numbers). One real defect was found by a test and fixed: concurrent log
writers lose lines on Windows (section "Found while building").

## The green run

From `harness2/`, venv Python 3.13.13, numpy 2.5.2, no bytecode:

```
set PYTHONDONTWRITEBYTECODE=1 & C:\tmp\m8b-edt\venv\Scripts\python.exe -m pytest -q --durations=8
=> 97 passed in 174.38 s          (final run 01:31, at commit 6406dee plus the .gitattributes line below)
```

- **Expected failures: none.** Round 1's one known failure (`test_driver.py::test_t13`, "ADDENDUM-A1.md is not committed yet": A1 has
  since been committed) lives on in `../harness/` as round 1's record and is left alone; harness2's T13 is adapted to B1 and passes.
- Counts: 97 = round 1's 74 tests (T1-T22 and the step-8 files; T3 replaced, T22a replaced, the rest adapted or unchanged) + 23 new
  (T23-T31 and T38 in `test_r2_scorer.py` = 10; T32-T33 = 2; T34-T35 = 3; T36-T37 = 2; T39, T40 = 2; T41 = 2; T42, T43 = 2).
- Slowest: T39 36 s (it builds the 13 fixture folders), T6 27 s, T7 25 s. A full `evaluate` of run 1's 14,800 inputs takes 9 s.
- `RED-2.md` holds each test of T23-T43 failing before its code; T29 is the one planned control (passes before and after, shown able
  to fail by a mutant).

### What each new test holds

| Test | File | Asserts (short) |
|---|---|---|
| T23 | test_r2_scorer | `frozen2/method.py` is `029d90ac...` (typed in conftest, and in `../frozen2.sha256`, and `corpus.FROZEN2_SHA256`), Z = 2.5 as imported, `.gitattributes` has `frozen2/**` and `harness2/**` eol=lf, `git ls-files --eol` shows `i/lf w/lf attr/text eol=lf` |
| T24 | test_r2_scorer | one flipped byte -> VoidRun (hash recorded); round 1's pin rejects frozen2 and round 2's rejects `frozen/`; round 1's file still loads under its own pin (Z = 2.0) |
| T25 | test_r2_scorer | `evaluate` has frozen + upstream, no `z3` key anywhere, `load_z3` does not exist, "Z = 3" absent and "Z = 2.5" present in REPORT.md and RESULTS.md |
| T26 | test_r2_scorer | H4 filter: R_m 0.5 in, 0.5000001 out; 2 ms out; design T60 3.0 in, 3.01 out; missing R_m raises (also in `_check_inputs`) |
| T27 | test_r2_scorer | H4 denominator: planted `receiver_too_large` rows leave it and are counted (n_before, count, share, per set); other refusals stay; ok share reported not gated; empty denominator fails |
| T28 | test_r2_scorer | refusals by reason over all radii, `rtl_by_r_class` per set and per class (0.5 in the first, 1.0 in the second), Energetic its own set, in REPORT.md |
| T29 | test_r2_scorer | H1, H2, H3, H5 each count a wrong-silent row at R = 1.4 m (control) |
| T30 | test_r2_scorer | upstream scored on the same ids in order, rows carry R_m, Random and Energetic never mix |
| T31 | test_r2_scorer | rows carry R_m: SPPS 0.31 (from the report), ISM the receiver's R, synth the spec's R; each passes `_check_inputs` |
| T32 | test_r2_corpus_rooms | 147 rooms (143 box, 4 non-box) by set as the plan, round 1's 119 first and unchanged, `corpus_rooms.json` sha unchanged, run 1's 12 ISM rooms equal `counts.json` (dims, alphas, T60s, rejections), seed lists, rebuild byte-identical |
| T33 | test_r2_corpus_rooms | G rooms: V, S, 8 receivers, 0.6 m clearance, classes, blocked only in G3 and G4, P5 T60s by an independent Eyring, the plan's table within 0.0051 s, P17 lengths and steps <= 65,536, features, P3 >= 0.15 against the list (min 0.164), G pairs apart |
| T34 | test_r2_plan_guards | `plan()`: 144 + 28 runs, seeds as section 3, G6 the 50k room, none in round 1's seeds, probes or dev seeds, `HELDOUT_SEEDS` exactly the new set |
| T35 | test_r2_plan_guards (2 tests) | round-2 seeds and G rooms refused before B1, allowed after; round-1 seeds (`round1_seed`) and F rooms (`unknown_room`) refused always; round-1 roots refused; `b1_committed` in a throw-away repo (staged is not committed); `--workers 5` refused in `main` and `run_queue`; defaults are round 2's; a B: room folder refused before anything is written |
| T36 | test_r2_fresh_sets | ISM-fresh-2: guards, 4 relatives (one per parent) + 8 drawn, each P3-fresh against the 147-room list and (relatives) against run 1's relative of the same parent, <= 1e8 images, rejections counted, reproducible (dev seed and the real seed, geometry only) |
| T37 | test_r2_fresh_sets | Synth-fresh-2: 4,000 specs, BOUNDS equal to round 1's literal, in bounds, 500 per cell with half delayed, reproducible, none equal to round 1's and none within 10 % (dB: 10 % of the range) of one on every parameter in its cell |
| T38 | test_r2_scorer | a dead room (ISM generator, 5 x 4 x 2.6 m, alpha 0.8, 0.31 m ball, 0.1 ms) is `receiver_too_large` under frozen2 and `ok` and 11 % wrong under round 1's file |
| T39 | test_r2_dry | D2' run against `expected_dry_2.json` (97 values) |
| T40 | test_r2_attack | sandbox = exactly `SANDBOX_FILES`; `method.py` byte-equal to frozen2; INTERFACE.md's refusal list equals the reasons found by regex in `frozen2/method.py` (9); VoidRun on a changed file; refuses before SENTINEL.md; folder `attack2`; draws take seed 2026100203 |
| T41 | test_r2_logging (2) | every room's `start` line (with its image count and pid) precedes the first `done` line with three concurrent workers; one line per receiver; `rows_for_scoring(on_receiver=)` calls once per receiver |
| T42 | test_r2_freeze | inventory = every file under `harness2/` and `frozen2/` plus the 10 named documents, text by LF bytes; the table round-trips; a planted hash and a one-byte file change are named; `facts()` |
| T43 | test_r2_run_order | scorer refuses without B1, on round-1 roots, with a missing or partial run folder (names it), with a pin mismatch; `main` exits before creating its output |

## G2 and the other design numbers (section 9 M1)

| Room | Design T60 125 / 500 / 1 k / 4 k Hz (s) | V m3 | P3 clearance, nearest | P17 length |
|---|---|---|---|---|
| G1 | 0.33 / 0.33 / 0.33 / 0.31 | 23.1 | 0.185 (P0b) | 3.0 s |
| **G2, alpha 0.10** | **2.90 / 2.79 / 2.71 / 1.95** | 2,352 | 0.246 (z3 box003), unchanged: geometry kept | **5.9 s** (59,000 steps) |
| G3 | late 1.90 / 1.86 / 1.82 / 1.44; early 0.31 (125), 0.30 (1 k) | 368.2 | 0.164 (F3) | 3.9 s |
| G4 | 0.64 / 0.64 / 0.63 / 0.58 | 264.0 | 0.420 (F4) | 3.0 s |
| G5 | 0.75 / 0.74 / 0.73 / 0.66 | 582.8 | 0.312 | 3.0 s |
| G6 | 0.32 / 0.32 / 0.32 / 0.30 | 520.0 | 0.333 (F6) | 3.0 s |
| G7 | 0.18 / 0.18 / 0.179 / 0.175 | 168.8 | 0.250 | 3.0 s |

G2 was 0.09 (1 kHz T60 3.00 s, H4's edge). Absorption only, geometry kept, alpha 0.10: 1 kHz 2.707 s, inside [2.6, 2.9]; every band under
3.0 s; the T60 range [2.6, 2.9] allows alpha about 0.093-0.103 and 0.10 is the round value in it. Its run length falls from 6.5 s to 5.9 s
because P17 is 2 x the 125 Hz T60 (2.896 s) plus the arrival. All other rows equal the plan's table (the test compares to 0.0051 s). Minimum P3 clearance
over the G rooms and the widened list is 0.164 (G3 against F3), as the plan said.

## Corpus and pin

- `harness2/corpus_rooms_2.json`: 147 rooms, sha256 `5b9b0fba148f7ce3f013eb84b3755ac2b7acd78b657d63c935d3fcf2e8b7fdcc`; run 1's ISM draw
  reproduces `counts.json` exactly (dims, alphas, T60s, the nine rejection counts).
- `preview_pin.json` (sha256 `69a47adc92f094b7e1b3798d0eb2f288fcfa18d96e914ac44cf6505e1d343b39`, LF bytes): ISM-fresh-2 draw
  `e41d47e7...` (21 relative near-duplicates, 2 drawn near-duplicates, 1,073 position rejections, 0 over the image cap, 2.44e8 images,
  80 of 144 receivers at R <= 0.5 m, relatives clear their neighbours by 0.103-0.117: all as the plan's preview said), Synth-fresh-2
  `9b401469...` (4,000 specs), G rooms `57a4e0e2...`, corpus list `5b9b0fba...`. `corpus2.check_pin()` redraws and names any difference; the
  scorer's `preflight` and the runner's `main` both call it. `python -m m8b.corpus2 --check` says "all reproduced".
- `frozen2.sha256` committed: `029d90ac5e8f6a634a51a7ffce136cbd4c0b7ea3d3ad8a18b78fd8240ff224a0 *frozen2/method.py`.

## Dry runs (planted and dev inputs only)

1. **D2' (T39).** `dry/run_dry2.py`: round 1's D2 plant (432 SPPS rows of exact exponentials, five planted faults) plus one planted dead-room
   row (id `spps|mock_d2|R099|1000|1ms|seed999|random`, ISM generator, 6 x 5 x 3 m at alpha 0.65, 1 ms). `expected_dry_2.json` was committed
   first (`ddda75b`) and never edited. **97 values compared, 0 mismatches** on the first run: n 433, ok 432, one `receiver_too_large`
   refusal, H4 n_before 145 / rtl 1 / n 144 / usable 100 %, H2 and H3 as round 1's D2, H5 paired 433 and eligible 406 with the frozen method's
   54 wrong-silent rows, each fault in its own counter. (Upstream's own wrong-silent count, 109, is printed and not asserted: the plant does not fix it.)
2. **Smoke on run 1's inputs (step 4).** `dry/smoke_run1.py`: the scorer end to end (frozen2 and upstream) on run 1's 14,800 inputs
   (`B:\data\m8b-edt\results\run1\inputs_*.pkl.gz`, R_m taken from each row's own half_width) against the **Z = 2.5 block of
   `target/agents/edt-v21/v21_zsweep.txt`** (dev evidence). **Matched, 80 of 80 numbers, 0 mismatches**: per set n, ok, usable, ok with truth,
   wrong-silent (SPPS Random 11, Energetic 6, ISM 0, Synth 5), usable with truth, covered (96.38 %, 95.99 %, 99.91 %, 92.86 %), refusals by
   reason (ISM 534 `receiver_too_large`), H3 judged and failing per set (all 0 failing), H5 ours against upstream (11/1371, 6/1269, 0/2641, 5/2997),
   H4 both with the radius filter lifted ("all R", ISM n 1,011, 285 refused) and at R <= 0.5 m (ISM n 716, usable 95.7 %, 22 refused). That is
   dev evidence, not a test of the method (PREREG-2.md says run 1 shaped v2.1).

D1 and D3 of round 1 take their expected counts from round 1's method (`weak_spots.json` tallies of `frozen/method.py`), which do not hold for
frozen2, so they are not re-run; D4 needs a judge panel (no attacker before review). Stated in `expected_dry_2.json` scope.

## Found while building

- **Concurrent log writers lose lines on Windows.** T41 with three worker threads lost one `receiver` line: two handles opened in append mode
  on one file are not atomic there (the CRT seeks to the end, then writes). In the real run two worker processes and the parent write the same
  `round2-progress.log`. Fixed in `score_heldout.py`: one lock for every writer, a `multiprocessing` lock made by `_make_pool` and handed to each
  worker by an initializer. A real spawn-pool smoke with two workers on two small ISM rooms logged every line (start, receivers 1/2 and 2/2, done).
- `ism_fresh.py` had CRLF in its working copy in round 1 (index LF); the copy is LF now.
- `harness2` T22a (weak spots reproduce from sources) failed in the first copy (rebuild includes each source's path, and harness sources live
  under `harness2/`): it is replaced by a control that `weak_spots.json` is byte-equal to round 1's (`c75d5321...`), which PREREG-2 requires
  (not regenerated). Round 1's rebuild test stays in `../harness/`.

## Readings (ambiguities, resolved toward round 1's behaviour; none changed a number)

1. **Folders.** The brief names `round2-rooms` for projects and "round2 subfolders" on B:, the plan names `heldout2` and `results\run2`. Used:
   run folders `B:\data\m8b-edt\round2\heldout`, results `B:\data\m8b-edt\round2\results`, `SENTINEL.md` in that results folder, launch and
   progress logs `C:\tmp\m8b-edt\round2-launch.log` and `round2-progress.log`, attack sandbox `C:\tmp\m8b-edt\attack2`. Round 1's `B:\data\m8b-edt\results`
   (parent of run1) and `heldout` are refused as roots, and so is anything that holds or lies inside them.
2. **No dual path.** Under M5 the plan's "F rooms and round-1 seeds still ask A1" has no round-1 path to keep: round 1's seeds are refused outright
   (`round1_seed`, in `require_not_heldout`, whatever B1 says) and an F room is an `unknown_room`. The probes P0/P0b stay launchable with their reserved seeds.
3. **T38's "EDT 0.05 s".** An image-source room with EDT 0.05 s at a 0.31 m ball that round 1's method answers `ok` and wrongly was not found; the planted
   room has truth EDT 0.065 s (h / EDT = 0.014, above the 0.01 refusal) and round 1's method is `ok` and 11.2 % low. Read as "an EDT of order 0.05-0.07 s".
4. **R_m required where H4 reads it.** `_check_inputs` demands R_m of SPPS and ISM rows (the plan's words); Synth rows carry it too and the refusal table reads it.
5. **H4 columns.** `n` is the rows left after `receiver_too_large` leaves; `n_before` is the rows in the filter. The scorer prints both for each set and mode.
6. **`check_pin` and the runner/scorer door** use `preview_draw` (the same code as `draw`, with no B1 gate) because they are called after B1 is checked;
   `draw` itself still refuses before B1 and refuses round 1's seeds. `HELDOUT_SEEDS` includes the attack seed 2026100203 (T34), although no solver run uses it.
7. **Attack draws** are seeded by `SeedSequence([class sha256, 2026100203])`, so a class's instances are not round 1's.
8. **Runner default solver folder** stays round 1's `C:\tmp\nm-target\target\solvers\bin` (checked per run by `driver.check_solvers`); SETUP.md explains the M10 slot hazard.
9. **Tests carried.** T3 (Z = 3 instance) replaced by a test that loads are independent; T22a replaced (above); T13 adapted to B1; T12, the guards file, the runner
   file, the gap file (G rooms join F rooms: 12 box rooms x 8 receivers), wiring and step-8.2 adapted to `geometry=` / `b1=` / R_m / G rooms. Nothing else changed.

## Open for the reviewer

- **`simpa.exe` differs from SETUP.md's record.** `C:\tmp\nm-target\release\simpa.exe` is now sha256 `6a1656ed92efe2b4fa7229a77814a6e56f9b6e56b69166e01655a0c53e2c19f5`;
  `harness/SETUP.md` recorded `cfe8caff...` (a build at HEAD `610e691`). The solver executables are checked by code sha256 per run, and `simpa` is recorded in B1 as found, so
  this is a note, not a failure; someone should confirm the rebuild changed no run behaviour before B1.
- **The P6 truth-feature report is not built** (G2 truth T30 at 1 kHz >= 2.5 s, G3 T30/EDT at 1 kHz >= 1.25 at half its receivers, G7 truth T30 <= 0.25 s). It is
  computed after the truth runs; `results_md` carries the M2 no-swap paragraph and a `features` slot that reads "not checked in this run" until it is.
- `freeze2` writes only the hash table and facts; `ADDENDUM-B1.md`'s prose (restating sections 1-5 and 9) is not written, as instructed.
- Real runs of `run_heldout.main` and `score_heldout.main` (non-dry) are not exercised: every path that reaches a solver or round-2 data is refused or monkeypatched by the tests.
