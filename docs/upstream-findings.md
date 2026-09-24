# Upstream findings (internal)

**Internal only.** This file records behaviour of upstream I-Simpa's own programs that our work
measured, each with its receipt. Every entry is a measurement or a reading of upstream's source,
labelled as such. None of it is a public claim. The project rule stands: no user-facing text (CLI
help, messages, README, UI) says that upstream is wrong until arc item 4 has run
(`docs/release-arc-plan.md`). Raising any of this upstream is Burhan's and Michael's call.

Upstream is `Universite-Gustave-Eiffel/I-Simpa` at `929a5c8` (v1.4.0 preview, 2026-01-14), the
tag our solvers are built from. "Ours" means that source built by `solvers/build.ps1`, whose
`spps.exe` has code sha256 `550485c695292501` (`solvers/manifest.json`). The shipped releases are
upstream's own installers, unpacked under `target/investigate/release-binary/` (`inst134`,
`inst140`).

## 1. `preprocess.exe` gives every user facet the first one's marker

**Source reading** (`src/lib_interface/input_output/poly/poly.cpp`, `929a5c8`). The `.poly`
reader counts facets in one counter, `parsedFaces`, which the scene facets (Part 2) have already
brought up to `sizeFaces`, their count (`:348-349`). The user facets (Part 5) test the same
counter against the same `sizeFaces` (`:418-423`), so after the first user facet the reader never
goes back to `USER_FACE_CONTENT_INFO`. Each following facet's info line (`1 0 <marker>`) is read
as a vertex line with one vertex and dropped, and its triangle is pushed with the first user
facet's marker, still in `newFace.faceIndex` (`:402-410`).

**Measured on tutorial 3**, whose box fitting zone upstream's GUI writes as 12 user facets with
markers 88 to 99:
- Every box facet that survives `preprocess.exe` carries marker 88: 19 facets after its splits,
  each lying in one of faces 90 to 99 (`parity_tutorials.rs::tutorial_3`, the accounting's 19
  `marker_changes`, each written 88).
- Our verification names what that does to the mesh: 280 tetrahedron faces whose marker is a face
  they do not lie on, and faces 90 to 99 carried by none (`tutorial_3`).
- The result is upstream's own: our parity mesh, which keeps `preprocess.exe`'s bytes, is the
  stored `temp/scene_mesh.poly` (4,889 bytes, sha256 `74b8f831…`) and each stored run's
  `tetramesh.mbin` (338,528 bytes, `bc2f0904…`) byte for byte (`tutorial_3`, parity gate (2)).
- As reported by the tutorial-3 investigation (`docs/investigations/2026-09-24-tutorial3/`,
  `synthesise.md` §2): every post-2017 build of `preprocess.exe` gives those bytes (ours, shipped
  1.3.4 and 1.4.0); the 2016 1.3.3 build writes markers 90 to 99, since it predates the reader.

**What it does to a run, measured.** SPPS, tutorial 3's stored run with transmission on
(`2019-06-18_14h28m18s`: 125 Hz, `trans_calc` 1, 50,000 particles per source), our `config.xml`
and `mesh.cbin`, `random_seed` 1:

| Mesh | Lost to loops | Lost to meshing | Receipt |
|---|---|---|---|
| parity mesh (upstream's `.mbin`) | 533,967 of 2,653,740 (20.12 %) | 49 | `tutorial_3_loops_parity_against_default` |
| default mesh (markers restored) | 2 of 2,922,796 (0.00007 %) | 59 | the same |
| parity mesh, seeds 2 and 3 | 20.10 %, 20.08 % | 39, 44 | `tutorial_3_receiver_levels_default_against_parity` |
| the stored 2019 run (seed 0) | 536,546 of 2,670,907 (20.1 %) | | the investigation, `synthesise.md` §4 |

- SPPS itself warns on that run: above 5 % lost it prints `Warning 534016 particles has been in
  error on 2653740 particles.` on stderr (`sppsNantes.cpp:33, 425-439`), which our classifier
  reads as `particle_loss_reported`. The run verdict fails it with that code and
  `particle_loss_excess` (`tutorial_3_loops_parity_against_default`).
- Why the loops [inferred, the investigation's reading, `synthesise.md` §4]: SPPS takes a
  face's reflection from the scene face its marker names, so the box's vertical facets reflect off
  face 88's horizontal plane; and face 88's fitting is overwritten by the room tetrahedron that
  last touches it (`coreinitialisation.cpp:151-176`).
- Our default mesh restores the true markers (Burhan, 2026-09-24 14:11); parity mode keeps
  upstream's and is never run (`docs/m5-m6-design.md`, decision 12).

## 2. Upstream's shipped `spps.exe` loops the same way on that mesh

**Measured** (`parity_tutorials.rs::shipped_solvers_on_tutorial_3_parity_mesh`, parity gate (4)):
the same inputs and seed as above on the parity mesh, run by three builds.

| Build | Lost to loops | Lost to meshing | Output files against ours |
|---|---|---|---|
| ours (`929a5c8`) | 533,967 of 2,653,740 (20.12 %) | 49 | |
| shipped 1.4.0 (2026-01-14) | 533,967 of 2,653,740 (20.12 %) | 49 | all 24 identical |
| shipped 1.3.4 (2020-12-23, the last stable release) | 533,474 of 2,649,697 (20.13 %) | 50 | 24 of 24 differ |

- So the loss is not ours: upstream's last stable release loses the same fifth of its records to
  loops on upstream's own tutorial mesh.
- On our default mesh, 1.3.4 loses 1 of 2,924,063 to loops (a scratch run,
  `target/agents/fu-receipts-scratch/t3levels.txt`, not a committed test).
- 1.3.4 prints its warning with the counts followed by a stray `l` (`Warning 533524l particles
  has been in error on 2649697l particles.`, the same scratch run). Our classifier's pattern does
  not match that line; our runs use `929a5c8`, which prints the counts plainly.

## 3. What restoring the markers does to the receiver levels

The investigation left this UNKNOWN: at 10,000 particles per source it could not resolve an
effect above the seed noise (`synthesise.md`, "UNKNOWN"). **Measured** at the stored
configuration (50,000 particles per source), seeds 1, 2 and 3 on each mesh
(`parity_tutorials.rs::tutorial_3_receiver_levels_default_against_parity`, parity gate (5)). A
receiver's level is its `Sound level.recp` summed over time, in dB; the calibration cancels in a
difference.

| Receiver | Default minus parity (mean of 3 seeds) | Seed spread, default | Seed spread, parity | Resolved |
|---|---|---|---|---|
| Receiver 1 | +0.072 dB | 0.381 dB | 0.241 dB | no |
| Receiver 2 | −0.070 dB | 0.125 dB | 0.102 dB | no |
| Receiver 3 | +0.119 dB | 0.075 dB | 0.075 dB | yes: the seed ranges do not overlap |
| Receiver 4 | +0.016 dB | 0.143 dB | 0.252 dB | no |
| Receiver 5 | −0.006 dB | 0.314 dB | 0.102 dB | no |

- **Settled:** at every receiver the two meshes' mean levels agree within 0.12 dB, against a
  difference limen of about 1 dB for a level. The test holds the bound at 0.2 dB, and says no to
  the default mesh's levels raised 1 dB.
- Only Receiver 3's difference is resolved by three seeds. The other four differences lie inside
  their seed spreads (0.10 to 0.38 dB), so three seeds do not resolve them.
- So the 20 % of records lost to loops barely reach the receivers here. [inferred] The lost
  records are mostly transmission copies trapped at the box: the parity mesh's totals are 9 %
  lower (2.65 against 2.92 million), while total energy differs by 0.01 dB (−10.747 against
  −10.757 dB, scratch run `t3levels.txt`).
- The run verdict is a separate matter: with 20 % lost, the parity run fails
  `particle_loss_excess` against the proposed 1 % limit (open decision 7).

## 4. SPPS reads a short spectrum past its end, so such a run does not reproduce

**Source reading** (`929a5c8`). SPPS sizes a source's band array by the `<bfreq>` entries the
source lists (`base_core_configuration.cpp:141`), fills at most as many as there are bands
(`:143-152`), and reads a band's power at the band's index in the band list sorted by frequency
(`sppsNantes.cpp:73`; also `reportmanager.cpp:813`). With fewer entries than a computed band's
index, the power is read past the end of a heap array: whatever the heap holds there. Materials'
arrays are sized the same way (`base_core_configuration.cpp:202`; not measured).

**Measured** on the fixture `tests/fixtures/runs/spps_oneband` (the source lists 1000 Hz only;
500 and 1000 Hz computed; seed 1; our `spps.exe`), scratch
`target/agents/fu-receipts-scratch/oneband.py`, transcripts `oneband-30.txt` and
`oneband-same-200.txt`:

| Runs | 1000 Hz: 2,000 absorbed by the atmosphere at the first step | 1000 Hz: 41 by the atmosphere, 1,959 by materials |
|---|---|---|
| 30, each in a fresh folder | 29 | 1 |
| 200, in one folder | 194 | 6 |
| 30 with the spectrum complete (a 500 Hz entry added) | 0 | 30 |

- In the second outcome the band's energy summed over time (`Total energy.recp`) differs in every
  run, from 1.2e-34 to 4.8e34 (the complete spectrum gives 11.74 in every run). So the power read
  is garbage: zero or too small to stay above its own epsilon in most runs, a positive power in 7
  of 230, and then the particles have the fates a real power gives them.
- The folder does not matter (6 of 200 in one folder). [inferred] Windows' heap places an
  allocation in a slot it chooses at random, so the bytes past the array change from run to run.
- **Not the thread timing.** A seeded run has one thread: a nonzero `random_seed` switches SPPS's
  per-band threads off (`spps/data_manager/core_configuration.cpp:44-47`). [source reading, not
  measured] With `random_seed` 0, upstream's GUI default, the band threads draw from one global
  generator (`sppsTypes.cpp:14-27`) with no lock: such runs cannot repeat in any case.

**What it means for us.**
- No parity claim is affected. Every same-seed claim of the parity bed uses a nonzero seed and
  complete spectra. Checked with `fu-receipts-scratch/spectra.py` (transcript `spectra.txt`):
  every source, material, fitting zone and receiver noise spectrum in the tutorials' stored run
  configs, in our configs of a bed run (tutorial 1 SPPS and TCR, the say-no runs, tutorial 3's
  three runs) and in every run-folder fixture covers every computed band, except spps_oneband's
  source.
- Our validator refuses a short spectrum on any source, material or fitting zone
  (`band_set_mismatch`). `run-folder`'s pre-launch band check refuses a short source spectrum
  (decision 11), so `spps_oneband` never runs through it.
- `tools/fixture-gen/mkexpected.py` names the tolerance (`UNREPRODUCIBLE`): the fixture records
  the first outcome, the second is judged as it and said in a NOTE, and any other is a
  disagreement (`test_fixture_gen.py`, class `Unreproducible`).

## 5. SPPS's and TCR's air absorption lacks the pressure factor

**As reported by the M7 work, not re-derived here** (`docs/params.md`, "ISO 9613-1: air
attenuation", on branch `m7-params` at `66201eb` and `m7-followups` at `91c453f`; test
`params_air.rs::upstreams_form_agrees_at_the_reference_pressure_only`):
- Upstream computes the water-vapour concentration as `hmol = H·Ps/Pref`
  (`Coef_Att_Atmos.cpp:56`), without ISO 9613-1's division by the ambient pressure. At
  101.325 kPa this makes no difference. At 80 kPa, about 2,000 m up, it is 24.6 % off in the worst
  band, and the air term alone moves the 8 kHz band's value by 6.6 %.
- Upstream evaluates the attenuation at the nominal band frequency, the integer `freqValue`
  (`base_core_configuration.cpp:114`), where the standard's table uses the exact midband
  frequency: up to 1.6 % off the table (160 Hz; also 80, 125, 1600 and 8000 Hz over 1 %).
- M7 reports this as a finding for M8, not yet raised upstream.
