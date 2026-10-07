# A6: spps-gpu's child queue, fixed and bedded (2026-10-07 10:27-10:58, Grace)

**Done-when holds: yes.** Every row of the spec's bed has a receipt below, all PROVEN. Branch `a6`; not pushed.

**What it means for Burhan.** A project with a transmitting wall now gets the same answer on the GPU as
from SPPS. On the beam, the result is the same to the last bit. On a diffuse room, it agrees within
Monte-Carlo noise. A4's 41 % of SPPS's particles that were never run are now run: 14.43 M against SPPS's
14.46 M, where A4 ran 8.49 M. If the GPU ever runs out of room for those particles, the run stops with
exit 2. The app then shows FAIL with `child_pool_overflow`, never OK. An older spps-gpu that only counted
its dropped children now fails the run too. Runs with no transmitting wall (CR4) are unchanged, bit for
bit and in speed.

## Bed (every number REALISED on Grace, i7-14700KF + RTX 5070 12 GB, 2026-10-07)

Solvers `C:\tmp\nm-solvers-a6`: A5's four, plus spps-gpu 0.2.0. That is the fat build of `f3e3a6e`,
sha256 `31f1a0bb...`, pinned. `simpa.exe` is the release build of this tree (`C:\tmp\nm-target-a6`),
which embeds the pinned manifest. Every bed run's `run.json` reads "solver build verified" and its
verdict reads OK, with no warnings, unless stated otherwise. Every solver stderr was read: it is empty
on every OK run.

| row | verdict | the numbers |
|---|---|---|
| **Beam, TL 10 dB (500 Hz) and TL 20 dB (1 kHz)**: spps-gpu against SPPS bit for bit, totals against the exact model | **PROVEN** | **Every `.recp` step is bit-identical to SPPS: 16 of 16 series** (4 receivers, sum and per source, 2 bands), at seeds 101 and 202. A4 had 0 of 16. Particles per band: 5,084,000 / 238,000 in both solvers, which is the exact model's 2,542 and 119 per family. Receiver totals against the exact truncated model: B1a +8.26e-6, B1b +3.04e-6, B2a +1.65e-5, B2b -3.56e-6 at 500 Hz, and +8.28e-6 / +2.94e-6 / +1.65e-5 / -3.71e-6 at 1 kHz. Those are SPPS's own deviations, under 1.7e-5. A4 had -2.45 to -3.26 %. The first transmitted arrival over the incident one is 0.1000021 (tau 0.1) and 0.0100002 (tau 0.01). |
| **Omni `trans`, 500 Hz, 200 k particles**: particles equal to SPPS's, T30 within noise with no sign bias | **PROVEN** | **Particles: spps-gpu 14,429,954 (s101) and 14,420,798 (s202); SPPS 14,459,245 and 14,427,504.** A4 had 8,487,515. The per-family spread from the seed-11 dump (sd 35.0 particles a family over 200,000 families) gives a standard deviation of 15,650 for a total. The pairs differ by -1.32 and -0.30 of their sigma; SPPS against itself differs by 1.43. At 1 kHz the totals are 3,252,557 and 3,252,520 against 3,249,369 and 3,251,173. **T30 in room 1 at 500 Hz:** spps-gpu R1a-d 0.5090, 0.5082, 0.5091, 0.5078 (s101) and 0.5089, 0.5081, 0.5059, 0.5099 (s202); SPPS 0.5085, 0.5101, 0.5073, 0.5107 and 0.5083, 0.5087, 0.5073, 0.5092. Means 0.50834 against 0.50874 (-0.08 %; A4 had -0.77 %). **4 of the 8 spps-gpu values lie below SPPS's mean, and none lies below every SPPS value** (A4: all 8 below all 8). Signed bias d/range for T30 at 500 Hz is -0.07 (rms 0.32), against controls -0.16 (SPPS against SPPS) and -0.12 (spps-gpu against spps-gpu). A4 had -0.66. Within the noise band: 150 of 152 values at s101 (worst 1.25: EDT at R1b at 1 kHz, counted twice through its per-source echogram) and 152 of 152 at s202, with controls at 152 of 152. L1 - L2: 11.421 / 11.419 dB against SPPS's 11.416 / 11.408 at 500 Hz. |
| **A4's one-sided, alpha = 1 and random-mode cases unchanged** | **PROVEN** | **Old build (A4's runs) against the new one, seed 11, dump against dump: 0 walk mismatches in every primary particle** of `trans-random`, `trans-a1`, `beam-a1`, `onesided`, `onesided-random` and `beam-onesided`, CPU build and GPU build alike. The step counts are the same as A4's (for example `onesided` 241,095,258 / 241,104,048), and the double sums agree within 1.3e-15. **Against SPPS (arm b):** `beam-a1` 16 of 16 series bit-identical and `beam-onesided` 24 of 24, at both seeds, as in A4. `onesided` 165 / 160 of 168; `onesided-random` 165 / 166 of 166; `trans-a1` 152 / 152 of 152; `trans-random` 124 / 124 of 126 and 124. Those are A4's counts, value for value. In `trans` and `beam`, every primary particle's steps, fate and final energy are bit-identical to the old build's. Only the children changed. |
| **The loud failure** | **PROVEN** | `simpa run beam.simpa --solver spps --device gpu` with `SPPS_GPU_POOL_CAP=1000` and the solvers verified (run `.out\a6\overflow\runs\20261007-104520-993-spps`): **exit 2**, run exit class 5, verdict **FAIL**, with reasons **`exit_nonzero`** ("exit 2 (0x00000002)") and **`child_pool_overflow`** ("spps-gpu: failed: child_pool_overflow: 45000 transmitted particles did not fit the child pool (1000 entries, 0 MiB) at 500 Hz, energy 3.40421e-07 J (0.0101965 of the band's source energy) dropped; the run is stopped and its results are not written; run SPPS on the CPU"). No warning. `solve\` holds only `config.xml`, `mesh.cbin` and `tetramesh.mbin`: no receiver file, no statistics, no "End of calculation.". `simpa results` refuses it with `results_run_failed`. The app reaches the same verdict through the same code: `app/src-tauri/src/runs.rs:1187` and `crates/simpa/src/mesh_run.rs:635` both call `run_project`. The app's UI table has words for the code (below). |
| **Cost**: CR4 1 kHz, 2 x 300 k, 10 s, before and after, three runs each | **PROVEN: no slowdown** | A5's `.out\a5\cr4-1k\CR4-1k.simpa` (copied), on the GPU, interleaved before/after (`bed/a6_cost.py`, `.out\a6\cost\cost.jsonl`), with the GPU otherwise idle. **Kernel seconds: before 0.6153, 0.6135, 0.6151; after 0.6149, 0.6150, 0.6135.** Trace seconds: before 0.7197, 0.7154, 0.7165; after 0.7148, 0.7137, 0.7153. Solver process: before 1.533 (the first, cold), 1.252, 1.245 s; after 1.214, 1.208, 1.189 s. All 6 runs: OK, 5 launches, 600,000 particles, stderr empty. CR4 has no transmitting surface, so no pool is allocated (`child_pool_capacity` 0). |

**Every A2 case, rerun at the end (spec item 4).**
- `cpu_gpu_all.py`, 17 cases, CPU build against GPU build (`C:\tmp\nm-a6\runs\a2-a\summary.jsonl`): **0 walk
  mismatches in every case.** CR4 traced 1,360,935,330 steps, the double sums agree within 4.6e-14, and
  every case is within 1e-12.
- The 17 GPU dumps against A2's own dumps (`C:\tmp\nm-spps-gpu\runs\a-final`): **walk identical in 17 of 17.**
- `vs_spps.py` through `run_bed.py`, both arms, seeds 101 and 202, every case (`.out\a6\a2bed\bed-a6.jsonl`,
  `analysis.md`): 72 of 72 runs OK. **spps-gpu against SPPS: 504 of 506 values within the noise;**
  SPPS against SPPS: 239 of 241. CR4: 54 of 55 (worst 1.14) and 55 of 55, which are A2's numbers.
  The other value outside is `v-random-diffuse` s202 (1.42); its SPPS control has one outside too (1.26).

## The pool as built

- **Where.** In `walk.h`, `pushChild` still fills the thread's 16-deep ring first: the fast path is
  unchanged. A child that does not fit goes to `acc.spill()`.
  - On the GPU, `spill` puts the child into a device ring of `Particle`, the global pool
    (`main.cu`, `GpuAcc::spill`). Any slot that needs work takes from the pool before it draws a fresh
    family from `dNextFam`, and runs the pooled child as a family of its own, its children in that
    slot's queue.
  - The ring has two 64-bit counters, taken and given. A launch takes only what was given before it
    (`poolHead0 .. poolAvail`), with a CAS loop on taken. It gives only into places already taken in an
    earlier launch (`index < poolHead0 + cap`), so the launch boundary orders every write before its read.
  - The host ends a band when no slot is busy and taken = given.
  - On the host (`--cpu` and the particle-file retrace), `spill` appends to an unbounded `std::deque`.
    `popChild` moves its oldest entry into the freed ring place, so the host order is SPPS's single FIFO
    (`sppsNantes.cpp:148-155`).
- **Capacity rule.** The pool is allocated at launch only when a child can be made: energetic, `trans_calc`,
  no direct-only run, and a material with transmission and tau ≠ 0 in a computed band. Its size:
  - **cap = min(max(2^22, 64 x the band's families), floor(free device memory / 4 / 136))**, where free is
    measured after every other allocation. `SPPS_GPU_POOL_CAP=<n>` overrides it, for the bed.
  - **136 bytes an entry** (`sizeof(Particle)`, `child_pool_entry_bytes`).
  - `spps-gpu.json` records the capacity, the free bytes it was sized from, the children given and the
    peak waiting.
- **The capacity at the spec's counts.**
  - **2 x 300 k = 600,000 families on 12 GB: REALISED** on `trans` at 600,000 particles, 1 source
    (`C:\tmp\nm-a6\runs\trans-600k`). 11,357,126,656 bytes were free, so the cap was **20,877,071 entries
    (2.84 GB)** against the 38.4 M that 64 x 600 k asks. The peak was 780,523 waiting children, 3.7 % of
    the cap, from 12,255,140 given.
  - **CR4-27** is 27 bands of the same 600,000 families. The pool is sized once, before the bands, so the
    rule gives the same: 38.4 M wanted, capped by memory. CR4 and CR4-27 have no transmitting surface, so
    neither allocates a pool.
  - **6 GB, computed, not run** (no 6 GB device here): with 4 to 5 GB free after the other allocations, the
    cap is 7.4 to 9.2 M entries (1.0 to 1.25 GB). That is about 10x the peaks measured here.
- **Peaks measured.**
  - `beam`, 2,000 families: 816,207 to 820,410. That is why the floor is 2^22, not the 2^20 of the first
    draft: at 2^20 the beam ran at 78 % of the cap.
  - `trans`, 200,000 families: 775,718 to 784,169.
  - `trans`, 600,000 families: 780,523.

  The peak stayed near 49,152 slots x 16 (786,432) whatever the family count. That suggests it is set by
  the frontier in flight, not by the total, but it is an observation on three cases, not a bound.
- **RNG derivation and its receipt.**
  - Before: a child's Philox counter was (family's particle index, `nextChild++`), a counter in the
    walking thread (`walk.h:577` at `30819ba`). A child that ran in another slot would have numbered its
    own children from that slot's counter.
  - Now each particle carries a 64-bit lineage and its own `kids` count. A source particle's lineage is 0,
    so its stream is unchanged. The k-th child of a particle of lineage L gets
    `childLineage(L, k) = fmix64(L ^ fmix64(k + golden))` (never 0, and siblings never collide, because
    fmix64 is a bijection).
  - The Philox counter is (particle, lineage low, draw/4, lineage high). The key is unchanged.
  - **Receipt:** the CPU build, which runs every child in one thread's FIFO, and the GPU build, which runs
    children in any slot through the pool, give **0 walk mismatches** on `trans` (14.44 M particles at
    500 Hz, children and child steps per family included) and on `beam` (5.08 M). All 8 A4 cases and all
    17 A2 cases give 0 too.
- **Failure.** If a spill is refused, it is counted (`childOverflow`, count and energy). The host checks
  after each launch, stops the band, prints the one `failed: child_pool_overflow` line and returns 2. That
  happens before the band's surface or cut files, the particle file, the receivers' files and "End of
  calculation.". `spps-gpu.json` is not written.
- **The run manager.** Classifier row 24, `child_pool_overflow`, FAIL, stderr:
  `^spps-gpu: (failed: child_pool_overflow|warning: child_queue_overflow): `. It catches the new line and
  an older build's warning. Verdict reasons come from FAIL rows by id, so `verdict.rs` names it with the
  line as its detail.
  - `docs/solver-contract.md` Part B has the row.
  - The UI gets a sentence for the code (`reasonWords.ts`, `reasonCodes.txt`).
  - `run_folder_fixtures` names the row as judged in `run_verdict.rs`, like `spps_gpu_refused`.

## Pinned

`solvers/manifest.json` on `a6` (commit `fd9d06b`):
- `spps-gpu.exe` sha256 `31f1a0bb4ed66d525139ffe576ff4b172fa858a087772a07074e5cb382042567`, code sha256
  `bc0c53d76bb4c0f7753e8f72dd8d26e829847a0dfdee7331379f5c0e834e9628` (`solvers/pe-fingerprint.ps1`
  `Get-CodeSha256`).
- The `spps_gpu` record:
  - source commit `f3e3a6e` (the last commit touching `src`, `build.cmd` and `third_party`), source tree
    `c284967` (`solvers/spps-gpu/src`, A5's convention);
  - build `solvers/spps-gpu/build.cmd C:\tmp\nm-a6\fat fat`, log `C:\tmp\nm-a6\build-fat.log`;
  - nvcc 13.2, sm_75 + compute_75 PTX + sm_120, built 2026-10-07T10:42.
- The exe is in `C:\tmp\nm-solvers-a6\` with A5's other four, whose hashes are unchanged.

The merge re-pin is the main thread's. B3 also changes `main.cu`, so the merged source needs a new build
and pin.

## Tests

`CARGO_TARGET_DIR=C:\tmp\nm-target-a6`; C: had 33-35 GB free.

| suite | result | notes |
|---|---|---|
| `simpa-core` lib, filter `run::` | 34 passed | classify, verdict, gpu units |
| `--test run_verdict` | 31 passed | new: `a6_lost_transmitted_children_fail_the_run_from_either_build`, on A4's real stderr lines (exit 0, every file) and on the new line (exit 2, no outputs); `each_fail_line_is_its_own_reason_code` now counts 13 FAIL rows |
| `--test run_contract_docs` | 4 passed | 24 rows; the old line also matches the row |
| `simpa --test run_folder_fixtures` | 3 passed | 29/29 fixtures, 23/23 rows; needs `SIMPA_SOLVERS_DIR` |
| UI `reasonWords.test.ts` | 3 / 3 | `node --test` |

Not run: the whole workspace, the app crate, the e2e gates (the spec's constraint).

## Untested

From A4's list:
- **How often real projects overflow**, or how close they come: no Elmia or tutorial-3 run on the GPU was
  counted.
- **A single-sided transmitting face.**
- **A surface receiver on a transmitting or one-sided face** (hazard 4).
- **Grazing incidence on a one-sided face.**
- **Fittings with transmission, and more than two bands.**
- **Random mode with alpha = 1.**

New:
- **Zeph** (6 GB, sm_75): the pool's sizing and the fat build's sm_75 code were not run there.
- **An overflow in a later band.** The bed's overflow came in the first band. In a later band, the
  earlier bands' surface and cut files would stay on disk under a FAIL verdict (exit 2, no receivers, no
  "End of calculation."). Nothing reads them as OK, but they are not removed.
- **The app's screen** for `child_pool_overflow`: the CLI and the shared `run_project` were run, the
  app's UI was not.
- **The host spill's memory.** It is unbounded, as SPPS's list is, so a pathological family could exhaust
  host RAM on `--cpu`.

## Found on the way

1. **The GPU is now slower than the CPU build on spill-heavy cases.**
   - Trace: `beam` 4.06 s against 2.91 s on 28 threads; `trans` 10.1 against 8.8 s (seed 11).
   - It still beats SPPS by 8-12x end to end: `trans` 8.8-14.3 s against 110-140 s; `beam` 2.9 s against
     22.8 s, under contention from the other bed runs.
   - Two causes. Pooled children become visible only at the next launch (~250 ms launches). And a pooled
     child's own subtree again runs serially in one slot.
   - Not in scope. A ready flag per entry, or shorter launches while the pool is non-empty, would be the
     next step if transmission-heavy projects matter.
2. **The pool is allocated for alpha = 1 transmitting materials**, which never make a child (`beam-a1`,
   `trans-a1`: 4.19 M and 12.8 M entries, never used). The predicate does not test 0 < alpha < 1. This
   is harmless, and costs one allocation.
3. **The overflow counts children per launch.** The 45,000 in the failure line are the children refused
   in the launch that overflowed, before the run stopped. They are not every child the run would have
   lost.
4. In this shell, **heredocs halve backslashes** (`\\n` became `\n`): two scripted edits failed their own
   asserts and were redone with Edit. This is the memory note "no backslash paths in heredocs", seen again.
5. `analyse.py` and `a4_bias.py` reproduced A4's one-sided numbers exactly (for example T20 at 500 Hz:
   +0.20, 0.83 against -0.08, 0.75), as bit-identical primaries and the same SPPS seeds predict.

## Files and commits (branch `a6`)

- `22103a3`: the pool, the lineage RNG and the overflow exit (`walk.h`, `main.cu`); the A4 drivers take
  A6's roots.
- `dd0bc3f`: the classifier row, tests, contract row, UI words; A2's drivers take env overrides.
- `f3e3a6e`: free bytes in `spps-gpu.json`; receipts at `main.cu:805`.
- `fd9d06b`: the pin.
- `9d1e08c`: decision 72; `bed/a6_trans.py`.
- This report, with `bed/a6_cost.py`.

Receipts are gitignored, under `.out\a6\`:
- `bed\bed-main.jsonl`, `analysis.md`, `bias.json`, `physics.json`, `trans.json`;
- `a2bed\bed-a6.jsonl`, `analysis.md`;
- `overflow\`;
- `cost\cost.jsonl`.

Receipts under `C:\tmp\nm-a6\`:
- `runs\a\` (A4 arm a), with `oldnew-*.json` against A4's dumps;
- `runs\a2-a\` (A2 arm a), with `oldnew-*-gpu.json` against A2's dumps;
- `runs\trans-600k\`;
- the build logs.

## Fixes after audit (2026-10-07 11:01-11:20)

The audit's verdict on `30819ba..3e6d5b2` was SHIP-WITH-FIXES, with six fixes. Each is its own commit on
`a6`, except fix 4, which was measured and reverted. Every number below is REALISED on Grace, with the GPU
otherwise idle (3 % utilisation before each timing).

| fix | commit | receipt |
|---|---|---|
| 1. Pool floor and override clamp | `7fe3fbd` | **A pool below two full queues a slot is refused at start.** On Grace that is 2 x 49,152 x 16 = 1,572,864 particles, 204 MiB. `simpa run-folder` on the band-3 template with `SPPS_GPU_FREE_BYTES=800000000` (`.out\a6\band3\runs-memlow\20261007-111422-095-spps`): exit 2, FAIL, reasons `exit_nonzero` and `spps_gpu_refused` with the detail "refused: gpu_memory_low: ... at least 1572864 particles (204 MiB) ... 762 MiB of its 12226 MiB are free (a quarter may be used: 190 MiB) ...". **`SPPS_GPU_POOL_CAP=100000000000` is clamped** to half the free memory, 41,754,142 entries, and the run is OK (`runs-bigcap\20261007-111422-460-spps`), not a `cuda_error`. A program holding 10 GB of device memory did **not** lower what spps-gpu read (11.36 GB free, `C:\tmp\nm-a6\hog`), because WDDM pages an idle allocation out. That is why the bed lowers the reading with `SPPS_GPU_FREE_BYTES`, which can only lower it. |
| 2. No files left by an overflowing run | `302b78a` | **The new bed row: an overflow forced in band 3 of 5 leaves no result file.** The case is `bed/a6_band3.py`, built from A4's `trans` with bands 125, 250, 500, 1000 and 2000 Hz. The panel transmits from 500 Hz only. It has a cutting plane and 10 saved particles. With `SPPS_GPU_POOL_CAP=1000` (`.out\a6\band3\runs-cap1000\20261007-111402-350-spps`): progress reached #40 (bands 1 and 2 done) and stopped in band 3 with exit 2, verdict FAIL, reasons `exit_nonzero` and `child_pool_overflow` ("2896 transmitted particles did not fit ... at 500 Hz ... the 4 result files its earlier bands wrote are removed"). **`solve\` holds only `config.xml`, `mesh.cbin` and `tetramesh.mbin`.** `simpa results` refuses the run (`results_run_failed`, exit 5). The same template without the cap (`runs-nocap\20261007-111402-903-spps`) is OK and writes 62 files, including the 4 removed above: `Surface receiver\125 Hz\rs_cut.csbin`, `Surface receiver\250 Hz\rs_cut.csbin`, `Particles\125\particles.pbin` and `Particles\250\particles.pbin`. Method: the working directory is listed before the first band. On the overflow exit, every file new or rewritten since is removed, and then every folder the run made that is left empty. |
| 3. One rule for making a child | `5dcea5d` | `makesChildren(MatBand, energetic, directCalc, transCalc)` in `walk.h` is the walk's own branch condition, and the pool sizing calls it on the same `MatBand` (`matBandAt`). `beam-a1` and `trans-a1` (alpha 1) no longer allocate a pool: `child_pool_capacity` 0 (A6's report had 4.19 M and 12.8 M entries). The walk is unchanged: see the identity rows below. |
| 4. Re-poll the pool within a launch | **none: reverted** | Built and measured. Entries carried a ready word, so a slot could take a child given earlier in the same launch, and a slot with no family left polled the pool until its budget was spent. The walk stayed identical (0 mismatches on `beam` and `trans`), but **it was slower.** Trace seconds, 3 runs each, same configs, alternating: `beam` 2.586, 2.574, 2.593 without it against 2.798, 2.846, 2.820 with it; `trans` 8.572, 8.556, 8.579 against 8.693, 8.718, 8.694 (`.out\a6\fix4-timings.txt`; the diff is kept in `.out\a6\fix4-repoll-reverted.diff`). **The audit's premise does not hold on a quiet machine.** The 4.06 and 10.1 s in A6's report were measured while other GPU work ran; quiet, `beam` is 2.59 s on the GPU against 2.84-2.90 s for the CPU build, and `trans` 8.57 against 8.42-8.49 s. `trans`'s rate per step, 1.70 G steps in 8.57 s (0.198 G/s), is the rate A4 measured before the pool existed (1.27 G in 6.43 s, 0.198 G/s). So the pool adds no cost. The slowness is this scene's own cost per step, against CR4's 1.36 G steps in 0.61 s. |
| 5. Stale cites | `b5b6e2a` | `classify.rs`, `run_verdict.rs` and `docs/solver-contract.md` now cite `main.cu:483, 494, 557, 626` (the refusals, `gpu_memory_low` among them) and `main.cu:837` (the overflow). B3's merge will move them again. |
| 6. One host spill | `a9b338a` | `HostSpill` (the deque, `spillWaiting`, `spill`, `unspill`) is the base of `CpuAcc` and `NullAcc`: 30 lines added, 51 removed. |

**Re-pinned** (`013423e`): the fat build of `302b78a` (tree `50bde32`), `spps-gpu.exe` sha256
`6589412c61aac3b6341b38558f0264ccd65d299e9669be2258257c77f9c23d53`, code sha256
`e274ca463b3ec6b34b240836739e5919425aa7e2122cb205ba3abc7a0619f7d9`, built 2026-10-07T11:11. The exe was
copied to `C:\tmp\nm-solvers-a6\`; the 10:42 build is kept there as `spps-gpu-1042-superseded.exe.bak`.
`simpa.exe` was rebuilt in `C:\tmp\nm-target-a6` with the new manifest embedded. Every run below reads
"solver build verified".

**Re-bedded with the re-pinned build:**
- **A2 `cpu_gpu_all.py`: 17 of 17 walk-identical**, double sums within 1e-12 (`C:\tmp\nm-a6\runs\a2-a-fix`).
- **A4 arm (a): 8 of 8 walk-identical**, CPU against GPU, with the same step counts as before
  (`C:\tmp\nm-a6\runs\a-fix`). The `beam` and `trans` GPU dumps are identical to the 10:42 build's.
- **A4 arm (b), seeds 101 and 202** (`.out\a6\fix\bed\`):
  - `beam` 16 of 16 and `beam-a1` 16 of 16 series bit-identical to SPPS at both seeds. The beam is within
    1.65e-5 of the exact model.
  - `trans`: particles 14,429,954 and 14,420,798 against SPPS's 14,459,245 and 14,427,504 (the same as
    before). Room-1 T30 at 500 Hz: mean -0.08 % against SPPS, 4 of 8 values below SPPS's mean, not every
    one below. Within the noise band: 150 of 152 and 152 of 152, against controls 152 of 152.
  - All 12 runs OK.
- **Cost, CR4 1 kHz, 2 x 300 k, 10 s, three interleaved runs each** (`.out\a6\cost-fix\cost.jsonl`):
  kernel 0.6296, 0.6302, 0.6317 s for A5's build against 0.6272, 0.6280, 0.6308 s after the fixes.
  Solver process 1.389, 1.230, 1.249 s against 1.313, 1.231, 1.207 s. No pool is allocated, and stderr is
  empty.
- Tests:
  - `simpa-core` `run_verdict` 31 passed and `run_contract_docs` 4 passed, after the cite change.
  - C: had 22-23 GB free.
  - The rest were not rerun: no Rust code changed beyond the cite strings.

**Left:**
- A real near-full device was not reproduced: WDDM pages other processes out, so `gpu_memory_low` was bedded
  through `SPPS_GPU_FREE_BYTES`.
- The cleanup removes by listing the working directory. A working directory shared with unrelated files that
  are written during the run would lose those files too. The run manager's solve folders are the run's own.
- Transmission-heavy scenes stay about as fast on the GPU as on the CPU build: the cost is per step, not the
  pool's.
