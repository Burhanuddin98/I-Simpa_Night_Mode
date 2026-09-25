# M8 evidence from the M7 follow-ups (2026-09-24): transcripts

The M7 follow-ups' critic found the receipts behind `docs/results.md`, "What M8 needs", and one
source `docs/params.md` cites only in gitignored `target/agents/*-scratch` folders that the reports
listed as deletable, and under `target/`, which `cargo clean` wipes. Copied here on 2026-09-25,
byte for byte, from the main checkout's `target/agents/`:

| Here | From | What |
|---|---|---|
| `critical/m8-cells-1.log`, `m8-cells-2.log` | `m7fu-critical-scratch/` | the first `m8_cells` runs (`crates/simpa/tests/m8_evidence.rs`, run on purpose) |
| `critical/energetic-seeds*.log`, `energetic-floor.log`, `energetic-lost.log` | `m7fu-critical-scratch/` | energetic mode over seeds, the solver's floor, and the lost particles from saved trajectories (tutorial 1) |
| `fix-critical/batch1a*.{txt,log}`, `batch1b*.{txt,log}` | `m7fu-fix-critical-scratch/` | the cells of the "What M8 needs" tables (`batch1a-cells.txt` lists them in `$SIMPA_M8_CELLS`' syntax), their runs, and their reading again with the commit's `simpa results` |
| `fix-critical/old-cells-*.txt`, `old*-reread.log` | `m7fu-fix-critical-scratch/` | the earlier cells, read again |
| `fix-critical/energetic-lost-m8cell*.log`, `energetic-lost-reread.log`, `energetic-floor-reread.log`, `energetic-seeds-reread.log` | `m7fu-fix-critical-scratch/` | lost particles in an M8 cell (5×4×3 m, α 0.4; `ρ`), and the tutorial-1 readings again |
| `fix-critical/lambert-400k.log`, `lambert-4M.log` | `m7fu-fix-critical-scratch/` | `lambert_box.rs`, the independent transport, at 400,000 and 4,000,000 rays a cell |
| `fix-critical/tcr-cells.log` | `m7fu-fix-critical-scratch/` | `m8_tcr_cells`: TCR's Eyring against the analytic value in every cell |
| `gui-2019/projet_calculation_e9da8b3f12.cpp` | `m7fu-coverage-scratch/` | upstream's `projet_calculation.cpp` at `e9da8b3f12`, the last change before `f50c36febd` (GPL-3, as upstream), sha256 `0f2529aaa38c252f9e7a74d54e2b2b57c03115ee345fbcd9ed1bc8eedeb73155`; `docs/params.md`, "Upstream's GUI reproduced on tutorial 1" |

**Not copied**, and still only in scratch: the run folders themselves
(`m7fu-critical-scratch/m8-cells-*`, `m7fu-fix-critical-scratch/m8-cells-*`, thousands of files
each) and the 2.5 GB of saved trajectories behind `ρ`
(`m7fu-fix-critical-scratch/energetic-lost-1790269638`). Keeping or moving them is Burhan's
decision; without them the numbers here can be re-run with `m8_evidence.rs`, not re-read.
