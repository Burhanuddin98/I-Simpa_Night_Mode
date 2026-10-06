# Sparse surface-receiver series: the maps' memory follows the data, not the grid (patch 0002, 2026-10-06 15:44-16:00)

Burhan, 15:27: "how do we make sure there is all that data but we store it in an insanely efficient fashion"; 15:44:
"its literally just data man, raw data ... why does it occupy so muchg space for a run". Read with `COMPACT.md`
(patch 0001, the time bin, which coarsens the maps' time axis and which he did not want as the answer) and
`BED-0002.md` (the receipts).

## What the solver did, and why it is zeros

SPPS kept, for every cutting-plane cell and every surface-receiver face, one `float` per time step per computed
band, allocated and zero-filled at start-up (`r_SurfCut::Init`, `coreTypes.h:280-305`; `r_Surf_Face::InitFreq`,
`coreTypes.cpp:91-100`). The energy a particle leaves on a cell goes into the cell's slot for the current step.
Measured on the 05:13 CR4-third run (300,000 particles a source, the Audience plane at 0.5 m, 1,794 cells, 10 s at
1 ms): 10 to 20 % of the slots ever receive energy (`density.py`, 15:28). On a plane at 0.1 m the same particles
spread over 24 to 60 times more cells, so under 1 % of the slots do. The dense cube grows with cells x steps x
bands; the data grows with particles x crossings and does not change when the plane gets finer. The `.csbin` file
already stores only the non-zero (cell, step, energy) records, so a solver that keeps only those in RAM loses
nothing.

## The change (`patches/0002-sparse-surface-receiver-series.patch`, on top of 0001)

`SparseTimeSeries` (`coreTypes.h`) replaces `std::vector<float>` as a cell's time series (`t_time_ar`) and
`decimal*` as a face's (`r_Surf_Face::energieRecu[band]` is now a pointer to one). It keeps the steps that received
energy, sorted, as a `uint16_t` step and a `float` energy (6 bytes a record, 9 with the vectors' growth), and
switches a series to a dense array once more than an eighth of its steps hold energy, where dense is the cheaper
form. So a series never takes more than the dense array did, and a sparse one takes a few per cent of it.

Every reader and writer goes through `add`, `set`, `get` and `scale`; there is no index operator, so the compiler
found every site: the two accumulation sites per receiver kind in `spps/reportmanager.cpp`, the post-process
scaling loops there, the `.csbin` writers in `baseReportManager.cpp`, and TCR's assignments and reads in
`ctr/TC_CalculationCore.cpp` and `ctr/input_output/reportmanager.cpp`. `r_SurfCut::Init` and `InitFreq` now set the
step count and allocate nothing.

**Bit-for-bit.** A particle's energy is a `double` (`CONF_PARTICULE::energie`, `l_decimal`, `sppsTypes.h:72`) and
the cell is a `float`, so upstream's `cell += energy` added in double and rounded once. The first sparse build
rounded the energy to float before adding, and 1.5 % of the map values came out one float ulp apart (6e-8
relative; same records, same counts). `add(step, double)` now does `cell = (float)((double)cell + energy)`, the same
operation, and the second build is compared in `BED-0002.md`.

## What it costs

Inserting into a sorted vector moves the records after the insertion point; a series stops being sparse at an
eighth of its steps, so the move is bounded by 1,250 records of 6 bytes on a 10,000-step run. The writers read
every step through `get`, a binary search on the sparse series. The seeded 3-band arms ran in the same time as the
dense solver (26-29 s). The production-size run at 0.1 m (43,930 cells, 18 bands, 300,000 particles a source) took
275 s against 168 s for the 0.5 m plane on the dense solver at 05:13; no dense run of the 0.1 m plane exists to
compare with, because it aborted.

## Memory, forecast against realised

| Run | Dense cube (what the old solver allocated) | Sparse, realised peak working set |
|---|---|---|
| Seeded bed, 3 bands, 0.5 m plane, 20,000 particles | 205 MB cube; 237 MB measured (old solver) | 62 MB (bin unset), 41 MB (10 ms) |
| CR4-third, 18 bands, Audience plane at 0.1 m, 300,000 particles | 30.2 GB (aborted at start-up on a 32 GB machine) | 3.06 GB, 275 s, exit 0 |

The realised 3.06 GB is 18 bands x about 19 million cell-steps a band x 9 bytes, which is what the records cost;
the dense cube was 10 times that and never ran. A forecast for the app: a band's records are bounded by
cells x bins (the grid) and by particles x sources x crossings (the data); the 0.1 m run gives about 32 crossings
a particle on CR4 over 10 s. `runSize.ts` now takes the smaller bound per band, at 9 bytes a record against 4 a
dense slot, labelled as a forecast calibrated on this one run.

## Still open

- The `Global` map file (all bands summed) holds 149 million records for the 0.1 m run, 1.1 GB on disk; the maps
  together 3.7 GB. That is the data itself. Whether the app wants the per-band files and the Global file both is a
  product question, not a solver one.
- A dense run of the 0.1 m plane for a realised old-vs-new number does not exist and cannot: it aborts.
- The scene-receiver (`r_Surf`) path is changed by the same patch but no bed arm has a scene receiver (backlog 91).
