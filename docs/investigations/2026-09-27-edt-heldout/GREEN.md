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
