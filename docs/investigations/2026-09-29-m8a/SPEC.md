# M8a: the T30 bed, pinned down before it is built

Written 2026-09-29 on branch `m8a` (from `6a2b411`), before any M8a code exists. It fixes the
matrix, the references, every tolerance, the outputs and the gate. **It is pre-registered:** once
this file is committed, nothing in sections 2 to 7 changes except by a new row in
`docs/decision-log.md`, committed before the bed runs. One cell was run to measure the cost
(section 8); nothing else was run, and no M8a result has been seen.

**What binds it** (`docs/decision-log.md`):
- **Row 1** (Burhan, 2026-09-24 23:14): "Kuttruff's corrected Eyring with γ² computed from the
  room's geometry (5 %), with the independent transport as the tight cross-check; plain Eyring
  reported only."
- **Row 10** (Jarvis): "M8a gates T30 first; M8b gates EDT once the band is in Rust", which
  "tests exactly the same things and drops nothing".
- **Row 11** (Jarvis): "Default step for acoustic-parameter runs: 1 ms".
- **Row 17** (Jarvis, 2026-09-29 08:10): the critic's M8 calls adopted as recommended: "(2) every
  seed's value is judged, refused or not, with the seed spread as its uncertainty; (3) the bed
  measures the noise model (10 seeds); (4) user-facing particle counts stay as they are, and the
  fix moves to v1.1; (5) the verified solver build is used, and its manifest is checked on every
  run; (6) rho = 10 for energetic lost particles is accepted; (7) the reference precision stays at
  32x, with 0.6 % accepted."
- Also still standing: row 2 (the follow-up spec, `docs/investigations/2026-09-25-followup-spec/followup-spec.md`,
  whose seed-batch rules SB-2 and SB-3 and decisions D6 and D9 this bed follows), and the M8 design
  decisions of 2026-09-25 00:20 and 2026-09-24 17:45 (`session-logs/HANDOFF-2026-09-23.md:778-791`
  and `:939-949`).
- The original gate: `docs/rebuild-plan-raw-2026-09-23.json`, `plan.milestones`, id `M8`
  (quoted where used below).

## 1. What M8a gates, reports and leaves out

| | What |
|---|---|
| **Gated** | SPPS's T30, random and energetic mode, in the two boxes: (A) against Kuttruff's reference, (B) the seed spread of the cell mean, (C) against the independent transport; (D) TCR's Eyring time against its analytic value; (E) the preconditions (section 5.2) |
| **Reported, not gated** | plain Eyring; Sabine; Kuttruff against the transport; per-receiver seed spread; the noise model against the ten seeds; refusal counts; random against energetic; the 20×8×4 m room; upstream's atmospheric-absorption validation |
| **Not in M8a** | EDT (M8b, row 10). SPL's diffuse field, C80 and D50 (open question 4). Tutorial 1's random-against-energetic gap, open physics question 1 of `docs/scope.md` (open question 5) |

## 2. The matrix

### 2.1 Rooms, source and receivers

Every room is a box built as `crates/simpa/tests/m8_evidence.rs` builds M8's cells (`m8_cell`):
tutorial 1's box (`tests/fixtures/rooms/tutorial1_box.simpa`) scaled to the size, one surface group,
one material, the fixture's omni source (80 dB, white) and its environment (20 °C, 50 % RH,
101.325 kPa), no surface receivers, no intersection logs.

| Room | V, S, 4V/S | Source (m) | Receivers (m), radius 0.31 m | Role |
|---|---|---|---|---|
| 6×10×3 m (tutorial 1) | 180 m³, 216 m², 3.333 m | (3.0, 5.0, 1.8) | (1.0, 1.0, 1.8), (3.0, 7.0, 1.8), (5.0, 8.5, 1.2) | gated |
| 5×4×3 m (upstream's validation room) | 60 m³, 94 m², 2.553 m | (2.52, 1.97, 1.53) | (1.0, 1.0, 1.0), (4.0, 3.0, 2.0), (1.0, 3.0, 1.9) | gated |
| 20×8×4 m | 640 m³, 544 m², 4.706 m | (4.02, 3.97, 1.53) | (2.0, 2.0, 1.2), (10.0, 6.0, 1.8), (18.0, 3.0, 2.5) | reported only |

- The first two rows are the source and receivers every M8 measurement so far used
  (`m8_evidence.rs` `Room::source`, `Room::receivers`; `params_kuttruff.rs` `ROOMS`), each receiver
  at least 1 m from the walls and the source, as the raw gate asks ("at least 3 receivers, each at
  least 1 m from walls and source").
- **The 20×8×4 m positions are this spec's choice** (no source gives them): the same rule, the
  source a little off every symmetry plane so that it lies on no internal facet, the receivers
  spread along the room's length, near, middle and far, since in a long room the field decays
  unevenly (`docs/params.md`, "An elongated room": 20×4×3 m, receivers against room energy up to
  0.8 % apart).

### 2.2 Walls, bands, air, step

- **Walls:** every face one material, uniform absorption α in every band, Lambert reflection with
  scattering 1, no transmission. α ∈ {0.05, 0.10, 0.20, 0.40} (the raw gate's set).
- **Bands:** octave bands 125 Hz to 4 kHz (6) with air absorption off; 125 Hz to 8 kHz (7) with air
  on (the raw gate: "for bands up to 8 kHz"; `m8_evidence.rs` `tutorial_octaves_to`).
- **Air:** off in the first table; on in the second, at 20 °C, 50 % RH and 101.325 kPa, the energy
  attenuation `m` the solvers apply (the solver's form at the nominal band frequency,
  `air::solver_air_absorption_per_m`; decision 4 of 00:20, "Air term: the solver's own (nominal
  band frequency), like for like"). The values, from ISO 9613-1 eq. (3)-(5) at the nominal
  frequency, which `docs/params.md` ("Upstream's version") puts within 0.035 % of the solver's form
  at this pressure; the 8 kHz value is `docs/params.md`'s 0.02425 /m, and every value agrees with
  what TCR's own times in `docs/investigations/2026-09-24-m8-evidence/fix-critical/tcr-cells.log`
  imply:

  | Band (Hz) | 125 | 250 | 500 | 1000 | 2000 | 4000 | 8000 |
  |---|---|---|---|---|---|---|---|
  | α_air (dB/m) | 0.00044 | 0.00131 | 0.00273 | 0.00467 | 0.00989 | 0.02967 | 0.10529 |
  | `m` (1/m) | 1.013e-4 | 3.016e-4 | 6.282e-4 | 1.074e-3 | 2.277e-3 | 6.831e-3 | 2.424e-2 |

  The bed takes `m` from `air::solver_air_absorption_per_m` at run time, never from this table.
- **Step:** `dt` = 1 ms in every SPPS cell (row 11; the rebuild plan's M8 row, "Physics test bed at
  `dt` 1 ms").

### 2.3 SPPS cells

Ten seeds, 1 to 10, in every cell: row 17 (3), "the bed measures the noise model (10 seeds)", and
SB-2, "10 distinct non-zero values (1 to 10 by default)". Seed 0 is never used (SB-2: an unseeded
run's draws are "neither reproducible nor shown to be independent").

| Room | α | Random: particles per source, duration | Energetic: particles, duration, `trans_epsilon` |
|---|---|---|---|
| 6×10×3 | 0.05 | 8,400,000, 4 s | 1,500,000, 4 s, 7 |
| 6×10×3 | 0.10 | 18,000,000, 3 s | 1,500,000, 2 s, 7 |
| 6×10×3 | 0.20 | 33,000,000, 2 s | 1,500,000, 1 s, 9 |
| 6×10×3 | 0.40 | 110,000,000, 1.5 s | 1,500,000, 0.5 s, 9 |
| 5×4×3 | 0.05 | 3,500,000, 3 s | 1,500,000, 3 s, 7 |
| 5×4×3 | 0.10 | 7,900,000, 2 s | 1,500,000, 2 s, 7 |
| 5×4×3 | 0.20 | 14,000,000, 1.5 s | 1,500,000, 0.8 s, 9 |
| 5×4×3 | 0.40 | 31,000,000, 1 s | 1,500,000, 0.4 s, 9 |
| 20×8×4 (reported) | 0.05 | 1,500,000, 5.7 s | 1,500,000, 5.7 s, 7 |
| 20×8×4 (reported) | 0.10 | 1,500,000, 4.3 s | 1,500,000, 2.9 s, 7 |
| 20×8×4 (reported) | 0.20 | 1,500,000, 2.9 s | 1,500,000, 1.5 s, 9 |
| 20×8×4 (reported) | 0.40 | 1,500,000, 2.2 s | 1,500,000, 0.8 s, 9 |

- **Both tables.** Every gated row runs twice, air off (6 bands) and air on (7 bands), with the same
  counts, durations and `trans_epsilon`. The 20×8×4 m room runs air off only (the raw gate lists it
  in the first table only). So: 2 rooms × 4 α × 2 methods × 2 air = **32 gated cells, 320 runs**;
  **8 reported cells, 80 runs**.
- **`trans_epsilon`.** Decision 6 of 00:20: "Bed files set trans_epsilon ≥ 7 (the default 5
  shortens T30 by about 0.34 %)" (`docs/results.md:888` narrows it to energetic cells; this spec
  takes the handoff's wider wording). Random cells set 7 too: it is inert there, since
  `energie_epsilon` is a fraction of the start energy (`sppsNantes.cpp:75`), and a random-mode
  particle keeps its start energy until it is destroyed, when it is set to 0 (upstream
  `spps/CalculationCore.cpp:62-67` for air, `:296-299` at a wall), and no cell has transmission. Energetic cells take `docs/results.md`'s measured values, 7 up to α 0.1 and 9
  from α 0.2.
- **Energetic counts: 1.5 million everywhere.** Decision 1 of 00:20 gates the cell mean "(met from
  1.5M)", and `docs/results.md` ("What M8 needs") asks the bed to "count again" because the larger
  energetic counts there were set by M7's noise model, since replaced. Measured at 1.5 M, T30's
  relative standard deviation per receiver-band is 0.03 to 0.18 % in these cells.
- **Random counts, by a rule fixed here before any M8a run.** The tightest gate (C, 0.5 %) needs the
  cell mean's standard error small. Rule: the smallest count whose predicted relative standard
  deviation of T30 per receiver-band is at most **0.8 %**, predicted from every row of that room and
  α in `docs/results.md`'s random table ("What M8 needs", air off), each row's pooled σ scaled as
  `1/√N`, taking the largest prediction, rounded up to two significant figures. Why 0.8 %: with 6
  bands as the only independent replicas (the three receivers of a band share its particles, taken
  as fully correlated, the conservative end), a seed's cell mean then has a standard deviation of
  at most 0.33 %, so the 95 % interval of ten seeds' mean is at most ±0.24 %, leaving room inside
  gate C's 0.5 % for the 0.01 to 0.19 % the evidence showed between random SPPS and the transport
  (`docs/results.md`, "T30 against Eyring"), and a ten-seed range beyond 2 % (gate B, 6.1 standard
  deviations) has a chance of 0.08 % per cell (2·10⁶ simulated ten-seed draws). The rows and what
  each predicts:

  | Cell | Rows (σ per receiver-band at N) | Largest prediction | Count |
  |---|---|---|---|
  | 6×10×3, α 0.05 | 1.89 % at 1.5 M; 0.62 % at 12 M | 8.37 M | 8.4 M |
  | 6×10×3, α 0.10 | 2.16 % at 1.5 M; 1.35 % at 6 M; 0.40 % at 40 M | 17.1 M | 18 M |
  | 6×10×3, α 0.20 | 3.74 % at 1.5 M; 1.60 % at 8 M (20 seeds); 0.45 % at 80 M | 32.8 M | 33 M |
  | 6×10×3, α 0.40 | 4.96 % at 1.5 M; 2.09 % at 16 M; 1.14 % at 48 M; 0.42 % at 260 M | 109 M | 110 M |
  | 5×4×3, α 0.05 | 1.16 % at 1.5 M; 0.47 % at 10 M | 3.45 M | 3.5 M |
  | 5×4×3, α 0.10 | 1.62 % at 1.5 M; 0.53 % at 18 M | 7.90 M | 7.9 M |
  | 5×4×3, α 0.20 | 2.25 % at 1.5 M; 1.05 % at 8 M; 0.42 % at 40 M | 13.8 M | 14 M |
  | 5×4×3, α 0.40 | 2.62 % at 1.5 M; 1.56 % at 8 M (20 seeds); 0.52 % at 60 M | 30.4 M | 31 M |

  Those rows were at `dt` 10 ms; T30's spread does not depend on the step at these counts
  (`docs/investigations/2026-09-27-step-cost/REPORT.md`, "T30"). The 20×8×4 m room is reported only
  and runs 1.5 M in both methods: no row exists to apply the rule to, and its noise is reported, not
  tuned.
- **Durations.** The gated rooms take the durations the evidence ran (`docs/results.md`, "What M8
  needs"; `docs/investigations/2026-09-24-m8-evidence/fix-critical/*-cells.txt`), in which T30 was
  given, or refused for noise only, in 18 of 18 receiver-bands. What they reach, on Kuttruff's time:
  random mode 89 to 309 dB, and a particle's chance of surviving to the end is at most
  `0.95^403 ≈ 10⁻⁹` (5×4×3 m at α 0.05), so every band should be complete (at most one particle in
  a million alive); energetic mode, Kuttruff's decay passes the floor `−10·ε` dB before the end in
  every cell (the tightest, 6×10×3 m at α 0.2 and ε 9, at 0.94 s of 1 s), and what particles still
  alive at the end could bring is bounded by the pipeline (`floor_db`, the alive share). The
  20×8×4 m room takes the 6×10×3 m durations times the ratio of mean free paths, 4.706/3.333 =
  1.412, rounded up to 0.1 s.
- **Everything else** as `m8_cell` sets it: the fixture's receiver radius 0.31 m, `method`,
  `particles_per_source`, `duration_s`, `time_step_s`, `extinction_exponent`, `air_absorption`
  (SPPS and TCR) and `random_seed` from the cell; meshed by `simpa run` with the default mesher
  (not `--parity`) and the verified TetGen 1.5.0.

### 2.4 TCR

TCR on every (room, α, air) of the SPPS table: **16 gated runs** (the raw gate: "TCR on every
cell") and 4 reported (20×8×4 m). TCR is deterministic; one run each, `random_seed` 1.

### 2.5 Upstream's atmospheric-absorption validation (reported, not gated)

The raw gate: "imports upstream's Validation_atmospheric_absorption.proj ... and compares it with
the .xlsx reference. This comparison is reported, not gated." Row 10 drops nothing, and it is a
T30 comparison (upstream's `Docs/validations/validation_atmospheric_absorption.rst`: "mainly in
terms of reverberation time"), so it belongs to M8a.
- `simpa import-proj` of
  `B:\repos\I-Simpa-upstream\src\isimpa\resources\doc\validation\atmospheric_absorption\Validation_atmospheric_absorption.proj`
  (read-only). Tried while writing this spec (this branch's build): it imports, exit 0, 12 faces,
  60 m³, 94 m²; walls Lambert with scattering 1 and α 0.1; source (2.5, 2.0, 1.5), receiver
  (1, 1, 1), radius 0.31 m; 20 °C, 50 %, 101.325 kPa; energetic, 500,000 particles, 2 s, 10 ms,
  `trans_epsilon` 6, seed 0; 27 third-octave bands, 50 Hz to 20 kHz.
- Run as imported, with three changes, each forced by a rule above: seeds 1 to 10 in place of its
  0 (SB-2), `trans_epsilon` 7 in place of 6 (decision 6), `dt` 1 ms in place of 10 ms (row 11).
  TCR once. Its source sits at the room's exact centre, which `m8_evidence.rs` avoided ("a little
  off it so that it lies on no internal facet of a symmetric mesh"); it is kept as upstream placed
  it, and a run that is not OK is itself the reported result.
- Compared per band with the `.xlsx` (sha256 `bb11404e…d1062`, read-only), transcribed into the bed
  file with that sha256: RT30 "SPPS 2018" and "SPPS 1.3.4" (sheet 2, rows 33-59), "RT Eyring (s)"
  (sheet 3, rows 33-59), RT30 "MD Octave" (sheet 4). Upstream's values were made with seed 0, 10 ms
  and ε 6, so the comparison is not like for like, and the report says so.

## 3. The references per cell

### 3.1 Kuttruff's corrected Eyring: the gated reference

```
T_K,b = K·V / (4·m_b·V + A_K),   A_K = −S·ln(1 − ᾱ)·[1 + (γ²/2)·ln(1 − ᾱ)],   K = 24·ln 10 / c
```

- `c` = 343.2 m/s, SPPS's speed of sound at 20 °C (`Celerite_du_son.cpp:46`), so `K` = 0.161020 s/m
  (decision 2 of 17:45: "K = 24·ln10/c with SPPS's own c (0.161) for SPPS").
- `V` is the run's `.mbin` volume, `S` the `.cbin` faces' area, `ᾱ = Σ Sᵢαᵢ/S` = α, `m_b` the
  solver's air term in band `b` (0 with air off, added outside the wall term: `docs/params.md`,
  "The formula, and where it comes from").
- **Read, not recomputed:** each seed's `simpa results --json`, `spps.reference.bands[b].kuttruff_s`
  (`value`, and `mc_sd`, the standard deviation it inherits from `γ²`; `results::reference`). The
  bed requires it to be bitwise the same in all ten seeds of a cell (same mesh, fixed transport
  settings) and every band's `lambert_walls` true.

**How `γ²` is computed** (`params::lambert::free_paths`, `docs/params.md`, "`γ²` from the geometry"):
a diffuse ray transport written apart from SPPS on the run's own `.cbin` triangles and `.mbin`
tetrahedra. Rays start at points drawn evenly from the tetrahedra, run straight to the nearest
face (a bounding-volume hierarchy, Möller-Trumbore), reflect by Lambert's law, and start again
10⁻¹⁰ of the room's size off the face. `FreePathSettings::STANDARD` is fixed: 16 × 4096 rays,
1 + 32 + 512 paths each (each ray's first 32 paths after its first are left out, so the field has
forgotten its start), 33,554,432 paths counted, a fixed seed. `γ² = (⟨ℓ²⟩ − ⟨ℓ⟩²)/⟨ℓ⟩²`, with its
standard error over rays (the delta method; each ray is the independent unit). It refuses a mean
free path more than 6 standard errors from `4V/S` and a `γ²` standard error above 0.002. `FreePaths`
has private fields, no other constructor and no `Deserialize`, so a `γ²` fitted to SPPS cannot
reach `kuttruff_rt`. Row 17 (7) keeps this precision ("32x").

The values the bed should see, with each box's exact `γ²` (integral geometry, `docs/params.md`,
"Known answers"); the shipped draw (`free_paths`: 0.38866 ± 0.00014 and 0.35269 ± 0.00012) moves
them by at most 0.008 %. For 20×8×4 m no exact value is committed; this spec's own Monte-Carlo of
Lambert chords (2·10⁷ chords, `refs.py` beside this file; the same code gives the two boxes'
exact values within 0.3 of its standard errors) gives 0.4104 ± 0.0002.

| Room | `γ²` | α 0.05 | α 0.10 | α 0.20 | α 0.40 |
|---|---|---|---|---|---|
| 6×10×3, air off: Kuttruff / plain Eyring / Sabine (s) | 0.388874 | 2.6424 / 2.6160 / 2.6837 | 1.3002 / 1.2736 / 1.3418 | 0.6286 / 0.6013 / 0.6709 | 0.2916 / 0.2627 / 0.3355 |
| 5×4×3, air off | 0.352401 | 2.0220 / 2.0037 / 2.0556 | 0.9939 / 0.9755 / 1.0278 | 0.4794 / 0.4606 / 0.5139 | 0.2211 / 0.2012 / 0.2569 |
| 20×8×4, air off | ≈ 0.4104 | 3.733 / 3.693 / 3.789 | 1.838 / 1.798 / 1.894 | 0.890 / 0.849 / 0.947 | 0.414 / 0.371 / 0.474 |
| 6×10×3, air on: Kuttruff at 125 Hz … 8 kHz | | 2.6249 … 1.0197 | 1.2960 … 0.7292 | 0.6276 … 0.4560 | 0.2914 … 0.2481 |
| 5×4×3, air on | | 2.0118 … 0.9117 | 0.9915 … 0.6218 | 0.4789 … 0.3720 | 0.2210 … 0.1951 |

Kuttruff is above plain Eyring by +0.9 to +11.7 %, which is the physics row 1 was decided on
("Plain Eyring is off by +1 to +11 % for Lambert walls").

### 3.2 The independent transport: the tight cross-check

`params::lambert::decay`: the same transport, now from the cell's source, with the three receivers'
balls (0.31 m) collecting energy times path length per 1 ms bin, as SPPS's receivers do, `(1 − α)`
per reflection, air as `e^(−m·c·t)` per bin; nothing of SPPS. **Its T30** is what
`crates/simpa-core/tests/params_kuttruff.rs` defines (lines 6-9): "the T30 of the energy the
transport's receiver balls collect ..., read through `params` from the arrival, at M8's source and
three receivers per room and `dt` 1 ms, averaged over the three receivers in each of 16 replicas;
the standard error is that of the 16 replica means. It is what M8 compares SPPS's T30 with."

- **Settings, fixed:** those of `kuttruff_against_the_transport_at_high_counts`, the source of the
  committed `HIGH` values: 16 replicas of `16·16384·α/0.05` rays (4.2 M rays a cell at α 0.05 to
  33.6 M at 0.4), duration 1.4 times the cell's plain Eyring time (with its air), seed
  `0x6d38_6365_6c6c_0000 + 1000·α`, `c` 343.2 m/s. With air on it is traced once per band with that
  band's `m`.
- **Self-check (gated, E4):** in the 8 air-off gated cells the bed's transport must reproduce `HIGH`
  (`params_kuttruff.rs:90-155`), computed as that file's `transport_t30` computes it, to 10⁻⁹ s:
  it is deterministic, so a difference is a changed transport, not noise.
- **Gated or reported?** The sources disagree; section 5.3. This spec gates it, at 0.5 %.
- The transport runs in its own phase, never beside the solver runs (it takes every core).

### 3.3 Plain Eyring and Sabine: reported only

- **Plain Eyring**: `T = K·V/(4mV − S·ln(1 − ᾱ))`, the same `K`, `V`, `S` and `m`, read from
  `spps.reference.bands[b].eyring_s`. Row 1: "plain Eyring reported only".
- **Sabine**: `T = K·V/(4mV + S·ᾱ)`, the same inputs; computed by the bed (`params::room`). The raw
  gate: "Sabine is reported, not gated."
- **TCR's own** Sabine and Eyring use TCR's constant 0.163 (`TC_CalculationCore.cpp:138`) and are
  held to their analytic values with that constant (gate D); against the physical `K` they sit
  +1.23 % high, the constant alone (`docs/params.md`, "Sabine and Eyring, as TCR computes them").

## 4. The value each seed gives

Per seed `s`, receiver `r` and band `b`, `T_{s,r,b}` is read from that seed's `simpa results --json`,
`spps.point_receivers[r].bands[b].parameters.t30_s`:
- a `value`: that value (and its `mc_sd`);
- refused `params_not_evaluable` whose `error.why.why` is `monte_carlo_noise` or
  `noise_uncalibrated`: the refusal's own `error.why.value`, "the value from the series" (and its
  `sd` when not null). Row 17 (2): "every seed's value is judged, refused or not"; D6: a seed's own
  noise refusal does not refuse the batch;
- **any other refusal** (`truncated`, `missing_moves`, `range_not_reached`, `not_decaying`,
  `range_too_short`, `unresolved`, `noise_unknown`, a refused series, ...): the cell is **not
  judged**, named with the seed, receiver, band and code, and the bed does not pass (E6). SB-3: "If
  any seed refuses the quantity, the batch refuses it ... The one exception is the seed's own
  single-run noise judgement."

A mean over a subset of seeds or receiver-bands is never formed (SB-3; `docs/results.md`,
"Constraints on M8 from the noise calibration": the passing mean sits low, "random T30 −0.82 % ±
0.13 %", "energetic T30 −1.23 %").

## 5. Tolerances

Every interval below is two-sided 95 %, Student's t with k − 1 degrees of freedom over the k = 10
per-seed values (`t₀.₉₇₅,₉` = 2.262): row 17 (2), "with the seed spread as its uncertainty".

### 5.1 The gated checks, per gated cell

| # | Check | Statistic | Limit | Source, verbatim |
|---|---|---|---|---|
| **A** | T30 against Kuttruff, per band | `x_s = mean_r T_{s,r,b} / T_K,b − 1`; the mean over s, and its interval | the interval inside **±5 %**, in every band | Row 1: "Kuttruff's corrected Eyring with γ² computed from the room's geometry (5 %)". Raw gate: "\|mean T30 - T_Eyring\| / T_Eyring <= 5% ..., for both computation methods"; "the air-absorption table meeting the same 5% criterion for bands up to 8 kHz" |
| **B** | Seed spread of the cell mean | `m_s = mean_{r,b} T_{s,r,b} / T_K,b`; `(max_s m_s − min_s m_s) / mean_s m_s` over the ten seeds | **≤ 2 %** | Raw gate: "seed spread (max - min) / mean <= 2%". Decision 1 of 00:20: "'Seed spread ≤ 2 %' is gated on the cell mean (met from 1.5M). Per-receiver σ is reported" |
| **C** | T30 against the transport | `y_s = mean_{r,b} T_{s,r,b} / T_tr,b − 1`; `d` = mean over s; `SE² = var_s(y)/10 + se_tr²`, `se_tr` the transport's relative standard error, averaged over the cell's bands as if fully correlated (the larger reading) | `\|d\| + 2.262·SE` **≤ 0.5 %** | Row 1: "the independent transport as the tight cross-check"; the number from row 1's revisit column, "gate the transport tightly (about 0.5 %)" (section 5.3) |
| **D** | TCR's Eyring time, per band, per TCR run | `\|T_TCR,Eyring / T_analytic,Eyring − 1\|`, both from `simpa results --json` (`tcr.bands[b].eyring.reverberation_time_s`, `tcr.analytic.bands[b].eyring_s.value`, as `m8_tcr_cells` reads them), TCR's 0.163 and the solver's `m` | **≤ 0.5 %** | Raw gate: "TCR Eyring equal to the analytic value within 0.5%". Decision 5 of 00:20: "TCR's 0.5 % check keeps 0.163 (4.4e-7 agreement); the physical K is for SPPS only" |

- **B** normalises each band by its Kuttruff time so that the air table's bands, whose times differ
  by up to 2.6 times, weigh alike; with air off it is exactly the raw `(max − min)/mean` of the cell
  mean. Over ten seeds rather than the raw gate's three it is stricter (the expected range of ten
  normal draws is 3.08 standard deviations, of three 1.69).
- **C, three outcomes**, the rule the follow-up spec gave R3 (`followup-spec.md`, section 4: "Pass:
  the 95 % interval of the gap lies within ±0.5 %. Fail: it excludes 0 and centres beyond 0.5 %.
  Otherwise add seeds."): PASS as in the table; FAIL when the interval excludes 0 and `|d|` > 0.5 %;
  otherwise INCONCLUSIVE, which does not pass. An INCONCLUSIVE cell may be extended **once**, by
  seeds 11 to 20, and judged again on all twenty (`t₀.₉₇₅,₁₉` = 2.093); gates A and B stay on seeds
  1 to 10. The report records the extension.
- **A** asks more than the raw gate's point estimate: its interval must lie inside ±5 %, as row 17
  (2) makes the seed spread the value's uncertainty.
- **Random and energetic are judged separately** (the raw gate: "for both computation methods").

### 5.2 Preconditions (gated, E)

| # | What | Source |
|---|---|---|
| E1 | Before any run, every solver executable's code sha256 (`solvers/pe-fingerprint.ps1`, `Get-CodeSha256`) equals `solvers/manifest.json`; after, every run's `run.json` executable sha256 equals the raw sha256 of the file checked | Row 17 (5): "the verified solver build is used, and its manifest is checked on every run"; `docs/scope.md`, "Engine work before M8", item 5 |
| E2 | Every run's verdict is OK (the run manager, `DEFAULT_LOSS_LIMIT` 1 %) and `simpa results` exits 0 | `docs/rebuild-plan.md`, M6 |
| E3 | In every gated band: `free_paths` computed, `kuttruff_s` a value, bitwise equal in the cell's ten seeds, `lambert_walls` true | Section 3.1 |
| E4 | The transport reproduces `HIGH` to 10⁻⁹ s in the 8 air-off gated cells | Section 3.2 |
| E5 | `cargo test --release -p simpa-core --test params_kuttruff` passes: Kuttruff's formula within the bare 0.6 % of the committed transport T30s in M8's 8 cells | Row 17 (7): "the reference precision stays at 32x, with 0.6 % accepted"; `docs/params.md`, "Kuttruff against the transport in M8's cells" |
| E6 | Every seed's T30 is a value or a noise refusal, in every receiver-band of every gated cell | Section 4; SB-3, D6 |
| E7 | The bed file is the matrix of section 2, byte for byte in its canonical form; any other bed file runs as `exploratory`, and an exploratory report never passes | This spec: a bed file must not be a way to tune the matrix after a result is seen |

`report.pass` is true exactly when A, B, C and D pass in all 32 gated cells (and D in the 16 gated
TCR runs), and E1 to E7 hold.

### 5.3 Where the sources conflict, and what this spec takes

| Question | Sources | Taken (the stricter) |
|---|---|---|
| **Is the transport cross-check gated, and at what?** | Gated, unnumbered: row 1 ("the tight cross-check"), `docs/rebuild-plan.md:97` and `:125`, `docs/params.md:895`, `docs/results.md:787` ("M8's tight cross-check"). Gated: `docs/v1.1-backlog.md` item 1, "M8's boxes are already covered by Kuttruff at 5 % and the transport (gated tight)". Not yet: row 1's revisit column, "Maybe: ... gate the transport tightly (about 0.5 %)", which puts a tight gate in v1.1. No document gives a v1 number | **Gated at 0.5 %** (the only number given), judged as C above. Open question 1 |
| Seed spread: per receiver-band, or the cell mean; 3 seeds or 10 | Raw gate: "seed spread (max - min) / mean <= 2%" with "3 seeds". Decision 1 of 00:20: "gated on the cell mean". Row 17 (3): 10 seeds | The cell mean (a later decision settles it; `docs/results.md` "Seed spread": per receiver-band would need 12 to 252 M random particles), over **10** seeds; per receiver-band σ reported |
| Kuttruff's 5 %: point estimate or interval | Raw gate: the mean. Row 17 (2): the seed spread is the uncertainty | The interval inside ±5 % |
| `trans_epsilon ≥ 7`: every cell or energetic only | `HANDOFF-2026-09-23.md:790` (every bed file) against `docs/results.md:888` (energetic cells) | Every cell (inert in random mode) |
| The bed's particle counts | Decision 3 of 17:45: "M8 then runs at user-realistic particle counts" (`HANDOFF-2026-09-23.md:948`). Decision 1 of 00:20: the cell mean "met from 1.5M". Gate C needs random counts of 3.5 to 110 M (section 2.3) | 1.5 M energetic; random by the rule of section 2.3. Not "stricter" in either direction: open question 2 |
| Kuttruff against the transport: 0.6 % | `docs/params.md`: the formula within 0.6 % in M8's cells, "not a held-out bound". Row 17 (7): "0.6 % accepted" | Gated through the suite (E5); reported per cell in the bed |

### 5.4 Reported, not gated

Per cell, in `report.json` and `summary.json`:
- **R1** plain Eyring and **R2** Sabine: the same statistic as A against them, with its interval.
- **R3** Kuttruff against the transport per band (the formula's own error, reported against the 0.6
  % of E5, which in the air cells and the 20×8×4 m room has no evidence behind it).
- **R4** per receiver-band: the ten seeds' mean, relative standard deviation and range (decision 1 of
  00:20: "Per-receiver σ is reported").
- **R5 The noise model against the ten seeds** (row 17 (3)): per receiver-band, the ten seeds'
  standard deviation `s₁₀` against the root mean square of the seeds' calibrated `mc_sd`, their
  ratio with its 90 % interval (χ², 9 degrees of freedom), and per cell the share of
  receiver-band-seeds whose `mc_sd` ≥ `s₁₀`, beside the 90 % that D7 of the follow-up spec asks of
  single-run coverage. Values refused `noise_uncalibrated` carry no `sd` and are counted apart.
  Reported because row 17 says "measures"; open question 3.
- **R6** refusal counts per code, and the share of `T_{s,r,b}` taken from a noise refusal.
- **R7** random against energetic: per (room, α, air), the difference of the two cell means with its
  interval. It is **not** open physics question 1, which is tutorial 1's specular materials.
- **R8** TCR: its Sabine against the analytic Sabine (0.163), and its Eyring against Kuttruff.
- **R9** the mean over only the values that came through against the mean over all, per cell: the
  selection bias the noise calibration measured, shown on this bed's data.
- **R10** the 20×8×4 m room: every statistic above, no pass or fail.
- **R11** upstream's atmospheric-absorption validation (section 2.5).
- **R12** per run: wall time, the solver's CPU time, files and bytes written.

## 6. Outputs

The bed is a new CLI command over a new core module, as the raw plan's architecture has it
("the physics bed with its analytic references" in `core`; `bed` among the CLI's commands):

```
simpa bed <bed.json> --out <root> [--jobs <n>] [--from <earlier-root>] [--json]
```
- `--jobs` (default 4): solver processes at once. `--from`: read an earlier root's runs again with
  this build, no solver run (as `$SIMPA_M8_FROM` does in `m8_evidence.rs`).
- Exit codes: **0 only when `report.pass` is true**; 8 the bed ran and did not pass; 2 usage or an
  invalid bed file; 5 a run was not OK.
- **The bed file** `beds/m8a.json` (committed): the matrix of section 2, the seeds, the transport
  settings, and the `.xlsx` values of section 2.5 with their sha256. JSON, not the raw plan's TOML:
  the core already depends on `serde_json`, and on no TOML crate. The limits of section 5 are
  constants in the code, not fields of the file.

Under `<root>/<UTC timestamp>/`:

| Path | What |
|---|---|
| `report.json` | Everything: the bed file's sha256, the git commit, each solver's code and raw sha256, the machine; per cell its settings, per seed its run folder, verdict, wall and CPU time, `T_{s,r,b}` with its source (value or which refusal) and `mc_sd`; the references (`γ²` and its SE, `kuttruff_s` and `mc_sd`, `eyring_s`, Sabine) and the transport's T30 and SE per band; checks A-E with statistic, interval, limit and verdict; R1-R12; `pass`, `exploratory`, and `failures[]` naming every failed or unjudged check with its numbers. Validated against a JSON Schema the bed also prints (`simpa bed --schema`) |
| `summary.json` | One row per cell and check: value, interval, limit, verdict; plus `pass`, the commit, the solvers' code sha256s, `report.json`'s sha256, wall time, file count and bytes. **Committed** as `beds/m8a/summary.json` after a run whose result is to be kept |
| `decays/<cell>.csv` | One file per cell, long form (never one file per row): `source` (`spps` or `transport`), `seed`, `receiver`, `freq_hz`, `u_s`, `level_db`: SPPS's `decay_curve.points` (thinned within 0.01 dB) and the transport's receiver Schroeder curves, with each T30 fit's end points |
| `decays/<cell>.npz`, `plots/<cell>.png`, `plots/summary.png` | Written by `tools/bed/m8a_plots.py <root>/<stamp>` (numpy and matplotlib; one Python process): per cell, the ten seeds' decays per receiver-band with the Kuttruff, plain Eyring and transport slopes; the summary, each check's deviation against α per room, method and air, with its limit band |
| `runs/<cell>/<seed>/...` | The run folders `simpa run` writes (run.json, mesh, solve) |

**Where `<root>` goes.** Off `B:` (exFAT with 128 KB clusters: the bed writes about 17,000 files,
section 8, close to the 20,000 a bed may leave there, and an extension adds more), and never under
any `target/` on `B:`. `simpa bed` counts its projected files before it starts and refuses a root
on an exFAT volume where they would pass 20,000. Nothing is deleted after the run (the user's
rule: always save outputs). Open question 6.

## 7. The gate: `tools/gates/m8a.ps1`

In the style of `tools/gates/m7.ps1` (`Check` blocks returning one bool, every solver call with a
hard timeout, a FAIL printed with its numbers), plus:
1. **Environment.** Honours an existing `$env:CARGO_TARGET_DIR` (this worktree builds into
   `C:\tmp\nm-target-m8a`: `B:`'s old target folder fails with os error 1392), else `<repo>\target`.
   Requires `$env:SIMPA_SOLVERS_DIR` and a bed root (`-BedRoot`, or `-From` an earlier one).
2. **E1 before anything runs:** the four executables' code sha256 against `solvers/manifest.json`.
3. **E5:** `cargo test --release -p simpa-core --test params_kuttruff`.
4. `cargo build --release -p simpa`, then `simpa bed beds/m8a.json --out <root> --jobs 4 --json`
   (or `--from`).
5. **Reads `report.json`**: it validates against its schema; `exploratory` is false; every run's
   `run.json` executable sha256 equals the checked file's raw sha256 (E1 again, from the files, not
   from the report's word); the bed's file count and bytes.
6. **Say-NO partners**, each of which must fail or refuse; all but N5 and N6 re-read the bed's own
   runs (`--from`) through test-only faults (`simpa_core::faults`, the `fault-injection` feature no
   normal build has), in an ignored test `crates/simpa/tests/bed_m8a.rs` run on purpose:
   - **N1** a copy of `spps.exe` with one text byte flipped (`Copy-PeFlippedText`): the bed refuses
     before any run (E1).
   - **N2** Kuttruff with its ½ dropped (`Fault::KuttruffFullVariance`: +11.0 % and +12.5 % at α 0.4
     in the two rooms, by the formula): A fails in every α 0.4 gated cell.
   - **N3** plain Eyring in place of Kuttruff as A's reference: A fails in every α 0.4 gated cell
     (SPPS sat +9.2 to +10.8 % above it in the evidence): the gate can tell the decided reference
     from the old one.
   - **N4** the transport reflecting evenly over the hemisphere (`Fault::LambertUniformReflection`;
     its mean free path 2.97 against 3.33 m, and in `params_kuttruff.rs` its T30 525 standard errors
     off in the worst cell): C fails in every gated cell.
   - **N5** one extra run: cell 5×4×3, α 0.4, random, seed 10 again at 31,000 particles, a
     thousandth of the cell's (below the calibration's 50,000, so its T30 is refused
     `noise_uncalibrated` and still read). With it in place of seed 10 the cell must not pass: by B,
     or by E6 if that seed's T30 is refused for more than noise; the report names which. This is
     the one say-NO that is statistical (at 31,000 particles T30's spread is about 25 % a
     receiver-band, so B misses 2 % by far unless the draw is extraordinary).
   - **N6** one extra run: 5×4×3, α 0.2, energetic, 150,000 particles, walls with scattering 0: the
     bed refuses the cell (`params_reference_not_applicable`, E3).
   - **N7** the transport traced with air off in every air-on gated cell: C fails in each.
   - **N8** gate D evaluated with the physical `K` in place of TCR's 0.163: D fails in every band
     (+1.23 %).
7. **Exit 0 only when** `report.pass` is the JSON `true`, every check of steps 2-5 passed and every
   say-NO partner failed as required. Anything else exits 1 and prints each failure.

A FAIL is a finding (the raw gate: "A FAIL goes to the user and Michael as a finding. The tolerance
is not loosened to pass it"): no limit, reference or cell of this spec changes because of a
result. The diagnostic arms the raw plan's risk list names ("computation_method 0 vs 1, particle
count, receiver radius, seeds") run only after a FAIL, as a separate exploratory bed.

## 8. Cost, from one measured cell

**The measured cell.** 6×10×3 m, α 0.05, energetic, 1,500,000 particles, 4 s, `dt` 1 ms,
`trans_epsilon` 7, air off, seed 1: a gated cell of section 2.3 exactly. One SPPS process, alone
(no other solver process on the machine; about 15 % CPU load from other work when it started),
Grace, 2026-09-29 08:22 to 08:43, through `m8_evidence.rs` `m8_cells` (`SIMPA_M8_CELLS=
"6x10x3,0.05,energetic,1500000,4,0.001,7"`, `SIMPA_M8_SEEDS=1`, `SIMPA_EVIDENCE_JOBS=1`), built from
this branch at `6a2b411` into `C:\tmp\nm-target-m8a`, with the verified solver build copied to
`C:\tmp\nm-m8a-solvers` (all four code sha256s equal `solvers/manifest.json`; the run's `run.json`
names that `spps.exe`, raw sha256 `1d9900db…`). The run folder is kept at
`C:\tmp\nm-m8a-cell\m8-cells-1790662923`; the test's output is `measured-cell.log` beside this file.

| Measured | |
|---|---|
| SPPS process (`run.json` `outcome.elapsed_ms`) | **1,248.7 s**, exit 0, verdict OK |
| `simpa run` (mesh, SPPS, verification) | 1,249.0 s; the whole test, with `simpa results`, 1,252 s |
| Files, bytes | 38 in the run folder plus the project file; 4.55 MB (185 bytes per 1 ms bin per band with three receivers, plus about 60 kB) |
| What it read (one seed; **not a bed result**; read at 08:43, after sections 2 to 7 were written, and nothing in them changed after it but section 6's file-count wording) | T30 given in 18 of 18 receiver-bands, mean 2.6474 s: +1.20 % on plain Eyring, +0.19 % on Kuttruff with the exact `γ²`, −0.01 % on the transport's committed 2.64768 s |

**The cost model.** SPPS's time is proportional to particle-reflections, summed over bands:
`N × Σ_b R_b`, with `R` the reflections a particle makes, `10·ε / (−10·lg(1 − α) + 4.343·m·ℓ)` in
energetic mode (`ℓ` = 4V/S; to the floor, capped at the duration) and `1/(1 − (1 − α)·e^(−m·ℓ))` in
random mode (to absorption). Anchored on one row, it reproduces all 36 wall times of
`docs/results.md`'s M8 tables (both modes, air off and on, 1.5 to 260 million particles) within
0.74 to 1.27 times, median 1.03 (`cost.py` beside this file). Those ran at 10 ms with 19 to 27
runs at once; the measured cell took 1,249 s at 1 ms alone against the evidence's 1,330 s at 10 ms
under load. The model is anchored on the measured cell; each cell below carries that 0.74 to 1.27
spread, and the 20×8×4 m room's cells more (at 1 ms a reflection there spans 13.7 steps against
9.7 in the anchor's room).

| Part | Runs | Single-process time |
|---|---|---|
| Gated SPPS cells (32 × 10 seeds) | 320 | 43.5 h |
| Random: 110 M at α 0.4 in 6×10×3 m is the largest, 729 s (air off) and 831 s (air on) a run | 160 | 16.6 h of the 43.5 |
| Energetic: 6×10×3 m and 5×4×3 m at α 0.05 are the largest, 1,212 to 1,249 s a run | 160 | 26.9 h of the 43.5 |
| Reported 20×8×4 m (8 × 10) | 80 | 7.0 h (random 0.4 h, energetic 6.6 h) |
| Upstream's atmospheric validation (10 seeds, 27 bands, 500,000 particles) | 10 | 2.2 h (782 s a run) |
| TCR | 21 | under 1 min |
| Say-NO runs N5, N6 | 2 | about 2 min |
| **SPPS in all** | **412** | **52.7 h** |
| The transport: 8 air-off cells, 56 air-on cell-bands, 4 in 20×8×4 m, at `HIGH`'s rays | | about 0.5 h on all cores (`HIGH`'s 12 cell-transports took "about five minutes" in a release build) |
| `simpa results` on every run (the reference's `free_paths`, about 1 s a run) | 432 | about 8 min |

**Wall time:** about **14 h at 4 runs at once** (13.2 h of SPPS if no run slows another, plus the
transport and reading), about 7 h at 8 at once, 53 h one at a time. Every longest run is about
21 min, so the tail of the queue adds at most that. Not included: gate C's one allowed extension
(seeds 11 to 20) of an INCONCLUSIVE cell, up to the cell's own time again.

**Disk and files:** about **17,000 files and 1.1 GB**: 15,760 files and 1.07 GB for the 400 matrix
runs, 520 files for the atmospheric validation's 10, TCR's 21 runs (not measured; a few hundred
files at most), about 130 bed outputs and the two say-NO runs. That is under the 20,000 a bed may
leave on `B:`, but not by much, and an extension adds 390 files a cell; hence `<root>` off `B:`
(section 6). The bed counts what it wrote
and records it in `report.json` and `summary.json`.

**This session's own footprint:** the one run, 39 files and 4.6 MB under `C:\tmp\nm-m8a-cell`;
the build folder `C:\tmp\nm-target-m8a`, 278 MB and 774 files (C: had 26 GB free after it).

## 9. Open questions

Each is a question for Burhan; the spec above takes the reading stated, and changes only by a
decision-log row.

1. **Is the transport cross-check gated in v1, at 0.5 %?** Row 1 calls it "the tight
   cross-check" and the backlog says "(gated tight)", but row 1's revisit column lists "gate the
   transport tightly (about 0.5 %)" as a v1.1 maybe, and no document gives a v1 number. This spec
   gates it at 0.5 % (gate C) with R3's three-way rule and one extension to 20 seeds. Is that the
   intended strength, or should C be reported only until v1.1?
2. **Do "user-realistic particle counts" (decision 3 of 17:45) bind the bed?** At 150,000 random
   particles T30's spread is 6.6 to 11.6 % a receiver-band at α 0.2 (`docs/investigations/2026-09-27-step-cost/REPORT.md`)
   and about 16 % at α 0.4 (1.5 M's 4.96 % scaled), so gate B fails and gate C cannot be resolved; this spec runs random cells at 3.5 to 110 million.
   Should the bed instead drop gate C for random mode, or add a 150,000-particle row as reported
   only?
3. **Is the noise-model measurement gated?** Row 17 (3) says the bed "measures the noise model"; this
   spec reports it (R5), with the follow-up spec's D7 coverage criterion (the calibrated `mc_sd` at
   least the ten seeds' spread in at least 90 % of receiver-band-seeds) shown beside it. Should a
   cell where the model under-states the observed spread fail M8a?
4. **Where do SPL's diffuse field, C80 and D50 go after the split?** The M8 row says they are
   "reported, not gated, until their references are chosen"; row 10 names only T30 (M8a) and EDT
   (M8b). Should M8a report them from its own runs, or are they M8b's?
5. **Does M8a have to settle open physics question 1?** `docs/scope.md`: random and energetic T30
   differ by +1.35 % on tutorial 1, and "this must be settled before either mode's T30 is shown".
   M8a's boxes have Lambert walls; tutorial 1's are specular, so R7 cannot settle it. Should the
   follow-up spec's R3 run be part of M8a, or stay a separate gate before M12?
6. **Where does the bed's root live?** It writes about 17,000 files and 1.1 GB (section 8); section 6
   keeps it off `B:`. Grace's `C:` had 26 GB free at 08:44; the raw plan put bed reports on `D:`
   (`D:\dev\<new-repo>\beds\<timestamp>`), which holds Michael's datasets and is the raw plan's
   open decision 5. Which volume?
7. **20×8×4 m only, or 20×4×3 m too?** The raw plan names 20×8×4 m (reported only). The one room
   where Kuttruff's formula is already known to miss 0.6 % is 20×4×3 m (−0.89 % against the room
   energy at α 0.4, `docs/params.md`). Should the reported room be 20×4×3 m, or both?
8. **Upstream's atmospheric validation at our settings or at theirs?** This spec runs it at 1 ms,
   ε 7 and seeds 1 to 10 (section 2.5), so it is not like for like with the `.xlsx`, which upstream
   made at 10 ms, ε 6 and seed 0. Is a second, exploratory run at upstream's own step and ε
   (seeded) wanted, so the comparison with upstream's numbers is direct?
