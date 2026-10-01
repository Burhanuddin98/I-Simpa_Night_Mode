# GREEN.md

Receipts for section 6 step 7a of `HARNESS-PLAN.md` (the reproductions only; D1-D4 dry runs are
separate). Run 2026-10-01 from `harness/`, venv `C:\tmp\m8b-edt\venv` (Python 3.13.13).

## Reproductions

### T4, T6, T14 held at their tolerances

Command:
```
cd docs/investigations/2026-09-27-edt-heldout/harness
set PYTHONDONTWRITEBYTECODE=1
C:\tmp\m8b-edt\venv\Scripts\python.exe -m pytest tests/test_upstream.py::test_t04_upstream_copy_hash_and_oracle_bit_for_bit tests/test_truth.py::test_t06_ism_rows_reproduce_bins_and_truth tests/test_driver.py::test_t14_reader_returns_the_old_loaders_fields -v
```

Result: `3 passed in 27.74s`.

| Test | Tolerance | Actual |
|---|---|---|
| T4 (`tests/test_upstream.py:24`) | Port's `edt_table` reproduces `oracle.json`'s EDT per band bit-for-bit (float32 bit pattern); against upstream's own `Acoustic parameters.gabe`, bit-identical on exactly the oracle's 24/29 rows, and the relative diff on the rest equals the oracle's own `max_rel_diff` | Held: 24/29 bit-identical, `max_rel_diff = 5.9832067225151775e-06` matched the oracle's recorded value exactly (`oracle.json`'s `params["EDT (s)"]["max_rel_diff"]`); port sha256 `57f391e6cb09ded8a408462fbabc7aacd93743a3fa46cc28deeef3f7a319f282` confirmed |
| T6 (`tests/test_truth.py:23`) | 12 picked rows (3 per corpus room: t1, corridor, dead, deader), `ism_fresh.make_row` vs `ism_rows.pkl`: bins and `truth_edt` within 1e-9 relative, `dt`/`t_arrival`/`half_width` within 1e-12 relative | Held on all 12 rows (assertions use `<=`, all passed) |
| T14 (`tests/test_driver.py:199`) | `driver.series_from_report` on 3 corpus noise-cal `report.json` files returns exactly the old loader's `energy_pa2` (`np.array_equal`), `arrival_s`, `R/c` and `dt` (exact `==`) | Held exactly on all 3 picked reports (first, middle, last of the noise-cal set) |

### Scan 1 regenerated through the harness

Critique's scan 1 source: `docs/investigations/2026-09-27-edt-simplify/final/run_scans.py:25-58`
("SCAN 1 (I1/I2/I3, noise-free, run 2*T60, R=0.31)"), originally run against
`target/agents/edt-simplify/critique/synth.py` and `final/method.py` directly.

Regenerated here not by calling that script, but through the harness's own reimplementation of the
same grid, `m8b.corpus.scan1(ctx)` (`harness/m8b/corpus.py:983-1074`), which computes every row fresh
from:
- `ctx.F`: `frozen/method.py`, loaded by `corpus.load_frozen` (`corpus.py:149-156`) only after its
  sha256 is checked against `PREREG.md`'s pinned hash;
- `ctx.S`: `critique/synth.py`, loaded by `corpus.load_synth` (`corpus.py:161-165`) only after its
  sha256 (LF-normalised) is checked against `corpus.SYNTH_SHA256_LF`.

Both loaders `exec()` the checked bytes into a fresh module every call (`corpus.py:140-144`,
`exec_module`), so nothing cached or previously imported can diverge from what the hash gate saw.

New file written for this: `harness/repro_scan1.py` (no `m8b/` code changed; it only imports and
calls `m8b.corpus.scan1`).

Command:
```
cd docs/investigations/2026-09-27-edt-heldout/harness
set PYTHONDONTWRITEBYTECODE=1
C:\tmp\m8b-edt\venv\Scripts\python.exe repro_scan1.py
```

Result (full JSON at `C:\tmp\m8b-edt\repro_scan1_output.json`):

| | Expected (`FINAL.md:55`) | Actual |
|---|---|---|
| Rows | 3,648 | 3,648 |
| Wrong-silent (`ok_WRONG`) at 1/2/5/10 ms | 0/0/1/0 | 0/0/1/0 |
| Refusals (`refused`) at 1/2/5/10 ms | 0/2/40/160 | 0/2/40/160 |

`frozen/method.py` sha256 seen at run time: `462c37cf159d4d9bd8abadd8261f975fa47ace66f0d4cb4b1e6f4234c77fdf6e`
(matches `corpus.FROZEN_SHA256`, PREREG.md:8's pin; the run did not raise `VoidRun`, so the gate
passed). `critique/synth.py` sha256 (LF) pin checked against: `3399262fc310059918465d3ed4b0c963da33e39bc9428721fbddb7907fe1bc11`.

Independently reviewed by a fresh-context sentinel pass (2026-10-01): confirmed `scan1(ctx)` is a
genuine row-by-row reimplementation of `run_scans.py:25-58`'s grid (same itertools.product ranges,
dedup key, well-conditioned filter, same `score()` kind semantics as `critique/common.py:18-26`),
computed fresh via `F.analyse(...)` inside the loop (`corpus.py:1012`), not copied from a logged
`scans_all.json`; `frozen/method.py` is byte-identical to `edt-simplify/final/method.py`, the file
`run_scans.py` itself imported. One noted gap, not disqualifying: the sentinel did not independently
re-diff `docs/investigations/.../critique/synth.py` against the original scratch-tree copy at
`target/agents/edt-simplify/critique/synth.py` that `run_scans.py` ran against; the match rests on
`SYNTH_SHA256_LF` being the correct pin, which was not re-derived from a second source.

## Dry runs

Receipts for section 6 step 7c part 2 of `HARNESS-PLAN.md` (D1-D3; D4, the attack judge panel, is
separate). Run 2026-10-01 from `harness/`, same venv (`C:\tmp\m8b-edt\venv`, Python 3.13.13). The
expected table, `expected_dry.json`, was committed first (`73a0ca9`, amended `4c6f03a`/`5335919`)
and is unchanged by this step. Two new files, both under `harness/dry/`:

- `dry/build_d2.py`: builds D2's fake SPPS run folders (one mock room `mock_d2`, 8 receivers, 6
  bands, 3 steps, 3 seeds, K = 4 references at 0.1 ms) in the `simpa results` schema, under
  `C:\tmp\m8b-edt\dry\D2\` (not `heldout\`), from `critique/synth.py`'s analytic single-slope
  histogram (`m8b.corpus.load_synth`, the same sha256-gated loader `corpus.scan2b` uses), exactly as
  `expected_dry.json`'s `D2.plant` key fixes every row: the five faults (disagreeing references,
  one truncated reference, one all-zero/NaN-truth cell, one blocked receiver, one subgroup whose
  tested run is built at 1.08x the room's design T60) and the 297 baseline rows.
- `dry/run_dry.py`: runs D1 (the critique's ratio-3 family, rebuilt row by row from the same
  construction as `m8b.corpus.scan2b`'s `'run_scans'` form), D2 (`build_d2`'s fixture, read back
  through `m8b.driver.read_run` and `m8b.spps_rows.rows_from_runs`, with `m8b.rooms.rooms`
  monkeypatched to the mock room, the same technique `tests/test_wiring.py::_fake_room` uses — no
  edit to `m8b/rooms.py`) and D3 (`res_ism.pkl` + `dev/ism_rows.pkl`, read once, read-only, from the
  main repo's `target/` via `corpus.find_target_root()`, restricted to rooms t1 and corridor)
  through `m8b.score.evaluate` — which itself loads the frozen method via its sha256-checked loader
  (`m8b.method.load`), the Z = 3 instance, and the upstream comparator wrapper (`m8b.upstream`) — and
  prints/writes a per-value comparison table against `expected_dry.json` (`C:\tmp\m8b-edt\dry\comparison.json`),
  never editing that file.

Command:
```
cd docs/investigations/2026-09-27-edt-heldout/harness
set PYTHONDONTWRITEBYTECODE=1
C:\tmp\m8b-edt\venv\Scripts\python.exe dry\build_d2.py
C:\tmp\m8b-edt\venv\Scripts\python.exe dry\run_dry.py
```

`frozen/method.py` sha256 seen at run time, in all three of D1/D2/D3's own `score.evaluate` calls:
`462c37cf159d4d9bd8abadd8261f975fa47ace66f0d4cb4b1e6f4234c77fdf6e` (PREREG.md:8's pin; no `VoidRun`
raised). Upstream port sha256: `57f391e6cb09ded8a408462fbabc7aacd93743a3fa46cc28deeef3f7a319f282`.

### Result: 162 of 163 compared values match; 1 mismatch, not a harness bug

| | Values compared | Mismatches |
|---|---|---|
| D1 | 31 | 0 |
| D2 | 65 | 0 |
| D3 | 67 | 1 (`H3.n_subgroups`) |

**D1** (the critique's ratio-3 family, 1,980 rows, scored as `synth`): row count, the full status
tally (794 ok / 1,186 refused / 0 wide), both truth counts (1,980 with a truth, 0 excluded), 0
wrong-silent, H1 (794 ok-with-truth, 0 wrong-silent, pass), H3 (3 subgroups — family 3.0 x step in
{1, 2, 10} ms — every per-step `n_ok_truth` at or above the plant's lower bound of 180, 0 wrong-silent
per step, pass) and H5 (1,980 eligible, frozen 0 vs upstream 1,085 wrong-silent, pass) all match
exactly. H2, H4 and H6 are not asserted for D1, as `expected_dry.json`'s own `h_verdict_scope_note`
says (D1 carries no `spps`/`attack` rows).

**D2** (432 fabricated `spps` rows): the status tally (432 ok, 0 refused, 0 wide, 432 usable), every
truth exclusion count (9 `truth_uncertain`, 9 `truth_truncated`, 9 `truth_nan`, 27 total, 405
`n_ok_truth`), 54 wrong-silent, H2 (coverage 351/405 = 86.67 % fails the 90 % bar, wrong-silent
54/405 = 13.33 % fails the 3 % bar, both exactly as the plant derives them), all 9 H3 subgroups
(near/mid/far x 1/2/5 ms, including `mid|2.0`'s 54-of-54 wrong-silent failure and the other 8 passing
at 0 %) and H4 (144 rows at 1 ms, all usable, all ok, pass) all match exactly. H1 and H6 are not
applicable (D2 carries only `spps` rows, no `attack` class); H5 is intentionally not compared (see
below).

Each planted D2 fault landed in exactly the counter `expected_dry.json` names, checked by receiver
and band/subgroup, not just by total:

| Fault | Where it landed | Expected | Actual |
|---|---|---|---|
| disagreeing_references | `truth_status = truth_uncertain` at receiver R000 (r1), 125 Hz | 9 | 9 |
| truncated_reference | `truth_status = truth_truncated` at receiver R001 (r2), 250 Hz | 9 | 9 |
| nan_truth | `truth_status = truth_nan` at receiver R002 (r3), 500 Hz | 9 | 9 |
| blocked_receiver | rows at receiver R007 (r8), all 6 bands, `truth_status = ok` | 54 | 54 |
| method_multiplies_by_1.08 | rows at r4/r5/r6, step = 2 ms, wrong-silent AND not covered | 54 / 54 | 54 / 54 |

A first run (`C:\tmp\m8b-edt\dry\run1.log`, kept for the record) found `H2.n_covered` 319 against
the plant's 351 (`n_ok_truth` 405 and `n_wrong_silent` 54 both already matched): a bug in this
builder, not in `m8b/`. The first `build_d2.py` applied `critique/synth.py`'s `noisy()` (compound
Poisson, `w = 1e-6`) to every tested/reference histogram; that noise was small enough to leave every
row's wrong-silent verdict untouched (comfortably under the 5 % JND) but large enough, on 32 of 351
rows, to bias the method's point estimate outside its own narrow shown range (`frozen/method.py`'s
OLS standard error measures fit scatter around the fitted line, not distance from the true decay
rate) — breaking `covered`, which the plant fixes as true for every one of those 351 rows ("edt =
truth exactly (0 % error), comfortably covered under any positive half-width"). Fixed by building
every D2 histogram analytically, with no noise applied (`dry/build_d2.py`'s module docstring has the
full account); re-run, `H2.n_covered` matched at 351 exactly, and every other value was unaffected.
`D2.H5` is left uncompared on purpose: `expected_dry.json` records it as `null` because materialising
real bins would mean building the mock, which this step's builder now has done — but the task's hard
rule is that a mismatch is never fixed by editing the frozen file, and the converse holds too: a
newly-available number is not backfilled into it either. `run_dry.py` records this as informational,
not a mismatch.

**D3** (1,320 rows of the real ISM corpus, rooms t1 and corridor, re-run through `score.evaluate`
rather than trusted from the log): row counts (624 t1, 696 corridor, 1,320 total), method sha256,
the full status tally per room and combined (t1 482/132/10, corridor 183/281/232, combined
665/413/242), both `truth_nan` exclusion counts (4 t1, 8 corridor, 12 total), 0 wrong-silent, 0
near-miss (recomputed independently here as `corpus.NEAR < |err| <= JND` on this run's own frozen-
method results, not read from `weak_spots.json`), H1 (665 ok-with-truth, 0 wrong-silent, pass), all
9 named H3 subgroups' `n_ok_truth`/`judged`/`pass`, H4 (660 rows at 1 ms, 542 usable = 82.12 %,
fails the 90 % bar, both rooms' own n/n_usable) and H5 (1,308 eligible, frozen 0 vs upstream 609
wrong-silent, pass) all match exactly.

**The one mismatch**: `H3.n_subgroups` — `expected_dry.json` gives 9, this run gives 10. Not a bug
in `m8b/`, and not a bug in this step's builder/runner (D3 reads real, unmodified corpus data). The
extra subgroup is `(ism, corridor, near, 2.0)`: `m8b/score.py`'s `h3()` creates a subgroup entry for
every row whose `subgroup(row)` key matches, regardless of that row's status (`score.py:372-388`,
`groups.setdefault(key, ...)` runs before any ok/usable check) — and corridor does carry near-class
rows at the 2 ms step in the real data (6 receivers x 1 band each pass gives e.g.
`ism|corridor|(7.0, 2.0, 1.2)|4000|2ms`, status `wide`; checked directly against `ism_rows.pkl` and
`res_ism.pkl`), just none of them is ever `ok`, so the subgroup exists with `n_ok_truth = 0`,
`n_wrong_silent = 0`, `judged = False`, `pass = True` — correct per P29/score.py's own contract, and
harmless to H3's verdict (it passes vacuously, same as the listed `corridor|near|1.0`). All 9
subgroups `expected_dry.json` does name match it exactly, value for value; the mismatch is that its
subgroup table omitted the tenth, zero-`n_ok_truth` one, which is a count the table itself fixes and
this run computes differently — reported here, not corrected in either file.

**Ruling (Jarvis, 13:13).** The harness is right and the expectation was wrong, on a field that decides
nothing. PREREG's H3 (:57) defines the subgroups as room × distance class × step and judges only those with
≥ 20 ok rows, so a subgroup with rows but no ok row exists and is not judged. `n_subgroups` counts it; the
fields that decide, H3's pass, `n_judged` and `n_failing`, matched. `expected_dry.json` stays as committed,
because it is the before-the-fact record, and this erratum is the correction. Section 5 item 3 is met with
it: 162 of 163 values matched, and the one that did not is explained here with receipts.

### Suite

```
cd docs/investigations/2026-09-27-edt-heldout/harness
set PYTHONDONTWRITEBYTECODE=1
C:\tmp\m8b-edt\venv\Scripts\python.exe -m pytest tests/ -q
```
Result: `45 passed in 209.98s`. No file under `tests/`, `m8b/`, `PREREG.md`, `frozen/` or
`expected_dry.json` was touched by this step.
