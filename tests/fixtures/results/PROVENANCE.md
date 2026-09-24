# Provenance: results fixtures

Four run folders, exactly as `simpa run` wrote them, for `core::results` and `simpa results`
(milestone M7, gate (e), and the M7 review). Acoustic values in them are format evidence only,
never results to quote.

| Folder | What |
|---|---|
| `seats_spps/` | `simpa run tests/fixtures/rooms/seats_box.simpa --solver spps`: SPPS, seed 1, 2,000 particles, octave bands 500 Hz and 1 kHz, 1 s in 10 ms steps, point receivers `Seat` at (1, 1, 1.8) and `Seat2` at (3, 7, 1.8), the floor's surface receiver |
| `seats_tcr/` | the same project through TCR |
| `energetic_spps/` | `rooms/energetic_box.simpa` through SPPS: the same box in energetic mode, `trans_epsilon` 3, 50,000 particles: every particle dropped, absorbed or lost by the end, for the solver's floor and energetic mode's completeness |
| `sources2_spps/` | `rooms/sources2_box.simpa` through SPPS: the same box with a second source, `Source 2` at (5, 8.5, 1.2), 3 dB weaker and 20 ms late, and `output_recp_bysource` on: each receiver folder holds `Source 1/` and `Source 2/`, for the echograms per source and the refusal of onset-relative parameters on their sum |

Each holds `run.json`, `solver.stdout.txt`, `solver.stderr.txt`, `mesh/` (the run's own mesh) and
`solve/` (the solver's inputs and every file it wrote). `run.json`'s absolute paths (`exe`, `cwd`)
are where the runs were made; `core::results` reads the folder it is given and uses neither.

**Why `Seat` and `Seat2`:** one label is a prefix of the other, so a reader that matched receivers
by prefix would take one for the other. `crates/simpa/tests/cli_results.rs` checks that each is
read from its own folder (SPPS) and file (TCR), and that copies with the folders swapped or a third
`Seat3` folder added are read swapped or refused.

**Regenerate** on Grace, with the M1 solver build: remove the folders to regenerate, then
`cargo test -p simpa --test cli_results -- --ignored write_results_fixtures`. The writer deletes
nothing: it writes only the folders that do not exist and keeps the others. With seed 1 SPPS gives
the same files again apart from the
`.csbin` padding bytes (`docs/rebuild-plan.md`, "Found while building"); `run.json` differs in its
time, paths and hashes of the executables.

The rooms were rewritten after these runs (`rooms/PROVENANCE.md`, "M7 rooms"): they now pin the
seeded box's element ids, so a run made today writes `config.xml` with the source's id 1799,
the receivers 1473 and 1632 and the surface receiver 1792, where these carry no source id and
the ids export assigned then (0 and 1, and 0); each `run.json` records the room's sha256 before
the rewrite.

Written 2026-09-24 by the solvers of `.claude/worktrees/wf_b9ed1d0e-3d2-8/target/solvers/bin`,
the build `solvers/manifest.json` records at this branch's base (`spps.exe`, `classicalTheory.exe`
and TetGen 1.5.0; their sha256 values are in each `run.json` and `mesh/mesh.json`).
