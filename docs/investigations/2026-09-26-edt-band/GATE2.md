# GATE 2 — production inputs (2026-09-27)

## The finding, restated

`critic:band`'s GATE 2 (`workflow-reports.json`): `Setup`'s defaults `tail_max = 0`, `rel_eps = 0`
(`band_early.py:184-185` before this change) are assumptions, not bounds, and the evaluation had
never exercised anything else -- every eval script fed the *generator's* exact dropped tail
(`eval_run.py:137-140, 233`; the real-set generator's `rest`, `eval_run.py:343-347) or a made-up
constant (`1e-10*sum` for Z3). Decision-log row 12 (Burhan): use a **run-length rule** -- widen or
refuse rather than assume 0, never a log-linear fill.

## What a finished run gives (SPPS, `docs/formats/results-json.md`, `docs/results.md`, `crates/simpa-core/src/run/stats.rs`)

Per band, per receiver: the recorded histogram (`energy_pa2`); `particles` (`absorbed_by_atmosphere`,
`_materials`, `_fittings`, `lost_by_infinite_loops`, `lost_by_meshing_problems`, `remaining`,
`total` -- `run/stats.rs::BandStats`, upstream's `partAbsAtmo` etc., `CalculationCore.cpp:88-107`);
`total_energy[band].energy[]`, the room's total energy per step; `trans_epsilon`,
`computation_method`, `particles_per_source`, `sources[].band_power_w`; `time_step_s`,
`duration_s`, `steps`; `receiver_radius_m`; and, from the pre-M8 `reference` block, `volume_m3`,
`area_m2`, `bands[].mean_absorption`, `free_paths.mean_free_path_m` (all computed from inputs
alone, not validated by M8 -- used here only as *known geometric facts*, not as a physics claim).
No output gives `m_n` (contributions per bin) or a per-particle trajectory, so neither of GATE 2's
two asks has an exact input; both need a "smallest safe substitute" (SPEC's own phrase).

## (b) `eps_n` without `m_n` -- P1, `safe_contributions_per_bin`

**Model-free: yes.** SPPS calls `ReportManager::RecordTimeStep` at most once per particle per
step (`CalculationCore.cpp:78`); an energetic-mode transmitted particle is a second, independent
particle already in the emitted total. So no bin's true `m_n` exceeds the number of particles ever
emitted:

```
m_n <= particles_per_source * n_sources   (constant, every bin)
```

This is loose (almost every bin's true `m_n` is far smaller: only particles alive and crossing the
ball in that step contribute) but never an underestimate, so `spps_rel_eps(n_bins,
safe_contributions_per_bin(...))` only widens `eps_n`, never narrows it -- the same "can only add
truths, never drop one" principle GATE 1's air-rate fix used. Implemented:
`band_early.safe_contributions_per_bin`, wired into `production_inputs`.

## (a)+(b) `tail_max` -- P4, `production_tail_max`

**Model-free: no**, and this is GATE 2's real finding, not a gap in the implementation.

Two components, both derivable from the run:
- **still alive at the series end.** `total_energy[]` and the source `.gap` both carry a factor
  `rho c` that cancels in a ratio (`reportmanager.cpp:155-166, 807-816`, cited already by
  `docs/results.md` "Lost particles" for the same reason). `f_end = total_energy[-1] /
  total_energy[onset_step]` is the share of the energy alive at the arrival still alive,
  unaccounted, at the series end -- exact, model-free, no fit.
- **killed at the `trans_epsilon` floor** (energetic mode; `CalculationCore.cpp:57-60, 141-146,
  305-310`; `sppsNantes.cpp:75`). A killed particle's energy at the kill is at most `E0 *
  10^-trans_epsilon`. The safe over-count of killed particles is `absorbed_by_atmosphere +
  absorbed_by_materials + absorbed_by_fittings` (only the energetic branches of those three states
  are floor kills; the rest are ordinary full absorption, already counted in the same field, so
  over-counting here only widens): `killed_particles * 10^-trans_epsilon / N` of the total emitted
  energy, `N = particles_per_source * n_sources`.

**What breaks the "fully model-free" requirement.** Both (a) and (b) are exact bounds on energy
*unaccounted in the room*. Neither is a bound on how much of that energy could still reach one
small receiver ball, and that conversion is where rigor runs out:
- a **hard geometric worst case** -- nothing rules out every remaining joule reaching the ball --
  gives a bound "of the order of the whole recorded energy" (SPEC 1.3's own words, written before
  this gate, about exactly this quantity): true, but useless, since it would make every band
  refuse or blow up to uselessness;
- the only tighter option is a **spatial assumption**: energy reaches the receiver in proportion to
  its share of a diffuse room. `docs/results.md` ("Lost particles", "the solver's floor") already
  makes this assumption and has tested it (`tests/params_floor.rs`: a closed-form model, tutorial
  1's room, alpha 0.05-0.9, trans_epsilon 1-7, r 2 and 8 m; 487 of 855 cells accepted, none of the
  accepted further from the model than its own limit) -- but it is explicitly not a proof:
  `docs/params.md` states the assumption in words ("that a dropped or lost particle's future is, on
  average, an alive particle's. In a diffuse room that holds; a particle lost where it would have
  crossed the receiver more than most is not covered").

**Conclusion, stated per the hard rule against faking a bound:** no fully rigorous, model-free
bound on either component of `tail_max` is possible from SPPS's outputs alone. This is the same
conclusion SPEC 1.3 already reached for the kill term, now shown to hold for the post-series-end
term too and confirmed by tracing (a) and (b) to the same geometric wall. `production_tail_max`
returns the diffuse-field (average-case, calibrated, not proven) bound, which is strictly better
than the production default of 0 (decision-log row 12: "never assume 0"), labelled in its own
docstring as an assumption, not a proof. This is a genuine, not cosmetic, gap: **Burhan, this is a
product decision** -- ship the calibrated (not worst-case) bound as GATE 2's production input, or
hold the band to a stricter, unprovable-in-general standard until a geometric receiver-capture
model is built and tested (a materially larger piece of work, out of GATE 2's scope). This report
recommends the former (it is real, tested, and non-zero, unlike the status quo) with the caveat
shipped alongside every number it produces.

## Implementation

`band_early.py`:
- `safe_contributions_per_bin(particles_per_source, n_sources)` -- the P1 `m_n` substitute.
- `production_tail_max(total_energy, onset_step, trans_epsilon, computation_method,
  killed_particles, particles_per_source, n_sources, receiver_recorded_from_onset)` -- (a)+(b)
  above, returns `(tail_fraction, detail)`.
- `production_inputs(B, dt, onset_step, total_energy, particles_per_source, n_sources,
  trans_epsilon, computation_method, killed_particles, ...)` -- GATE 2's entry point,
  `-> (tail_max, eps, detail)` in `Setup`'s own units.
- `min_run_length(dt, alive_share_curve, tolerance_fraction, trans_epsilon, computation_method,
  killed_particles_rate, particles_per_source, n_sources, max_steps)` -- the run-length rule: reads
  the alive-share curve one step at a time (the caller's own measurement, extended if it can be),
  returns the first step at which (a)+(b) fall to and stay at or below `tolerance_fraction`, or
  `(None, 'run_too_short')` if it never does within `max_steps`. No fit: it does not know the curve
  past what it is handed.
- SPEC.md: two new passages under section 1.3 (`T_max`'s derivation and its "what this is not")
  and after P1 (`m_n`'s substitute), both with file:line receipts.

## Run-length rule, a typical room

Not run against a real room end-to-end (out of scope for a python-only, no-solver gate): the rule
takes the alive-share curve as data, one step at a time, and is exercised in `gate2_real_check.py`
against ten real production runs' own `total_energy` series (below), where every run already given
is long enough (`f_end` and `kill_frac` both several orders of magnitude under any of the band's
tolerances -- see numbers below). No committed run in this repo's fixtures is short enough to show
a refusal; producing one would mean running the solver, against the hard rule. **This is a real
gap in this closure**, not a rounding of scope: the rule is implemented and unit-tested
(`band_early.py`'s own smoke check above returns `run_too_short` correctly on a synthetic curve
that never reaches its tolerance), but not demonstrated end-to-end on a real short run.

## Real-set re-run (`gate2_real_check.py`)

**Scope, stated plainly.** The full 26,808-row real set (`eval_real.json`) was generated from
`target/agents/followup-design/spec/real_rows.pkl`, built over 16 fine-step datasets
(`bracket/common.py::datasets`). Ten of those are genuine SPPS production runs read from
`report.json` (`load_report`: `C-E3`, `C-E4`, `C-E6`, `V-E2`, `V-E5` energetic; `C-R3`, `C-R4`,
`C-R6`, `V-R2`, `V-R6` random) -- these ARE what GATE 2 asks for, real production outputs with
`particles`, `total_energy`, `trans_epsilon`. The other six (`POS200`, `A02`, `A02e150k`,
`A02r150k`, `B02`, and `W-E6`, not found in this session's scratch) are `load_solve`-based: raw
`.recp` + `config.xml` + `mesh.cbin`, with **no `total_energy` or particle-statistics table
attached by the loader** -- `production_inputs` cannot be built for them without re-reading the
run's own stats GABE file, which this session did not do (time; the loader would need extending,
not just called). **This is itself a GATE 2 finding**: two of sixteen real-data loaders in the
existing harness do not carry what GATE 2 needs, so closing the gate for those would need a loader
change, not just a band change.

**What was actually run**: the ten `load_report` datasets, one seed's worth of receiver-bands
across all seeds and the coarsest registered step for each (`ms = 2`, `k = 2` of the 1 ms fine
step; the 3,600 rows this makes are 1/6 to 1/7 of the 26,808, not the whole set). Production
`tail_max` and `eps` (via `production_inputs`) replace the harness's exact fine-array rebin
remainder is *added* to the production tail (it is exact, not a model, so free to include) rather
than standing alone. No solver was run; only committed `report.json` files were read.

| | value |
|---|---|
| rows | 3,600 (360 per dataset x 10) |
| refusals (production_inputs raised) | 0 |
| EDT checked / outside | 3600 / **0** |
| Ts checked / outside | 1350 / **0** (the rest: no Ts truth registered at this step, not a miss) |
| max kill fraction seen | 9.9999e-8 (of S(onset)) |
| max still-alive fraction seen | 8.3627e-5 (of S(onset)) |

**Reading it**: on every one of these ten real cells (which include `trans_epsilon` 5 and 9,
random and energetic, five different rooms), the production tail is 4-6 orders of magnitude under
the EDT/Ts tolerances, so it changes nothing measurable in this sample: 0/3600 EDT, 0/1350 Ts
outside their band, same as the oracle-tail baseline reported in `eval_summary.txt` for the
corresponding rows. **This is a favourable result on real data**, not a proof the bound is always
cheap: these ten cells were all run long enough and at high enough `trans_epsilon` that the tail is
tiny by construction (`docs/results.md`, "The floor's bound is earned at upstream's default" shows
the opposite at `trans_epsilon` 5 with fewer particles). The full 26,808-row re-run, the remaining
six datasets, and a real short/starved run that exercises a refusal are open (see below).

## Tests T1-T7

Re-run with `--workers 2` (`gate2_test_stdout.txt`; `gate2_prev/` holds the pre-change
`results_t*.json`, copied before this re-run per the no-delete rule). All pass, unchanged by this
gate's additions, which are opt-in (`production_inputs`/`min_run_length` are new entry points;
`Setup`'s defaults are untouched):

- T1: 160 rows, 0 truths outside their band
- T2: 32 cases, 0 truths outside their band
- T3: 90 cases, 0 truths outside their band
- T4: 400 series, 80,000 arrangement-quantity checks, 0 outside their band (703 s)
- T5: 0 climbs escaped the band, 0 errors (42 s)
- T6: 240 series, 0 VIOLATIONS (17 s)
- T7: the adversary's edge cases (E1, E2, Ts/Dinkelbach capped iterations) all resolve OK under
  the method's stated preconditions

## What went wrong / needs Burhan

1. **Product decision, not an oversight**: the tail bound this gate ships is a calibrated,
   diffuse-room average-case bound (the same one `docs/results.md` already uses and has tested), not
   a worst-case proof -- because no worst-case proof is possible from SPPS's outputs alone (see
   above). Ship it as GATE 2's production input (this report's recommendation), or hold for a
   receiver-capture geometry model first (larger, separate piece of work).
2. Two of the sixteen real-eval datasets (`POS200`, `A02*`, `B02`) have no particle-statistics /
   total-energy loader wired up; `production_inputs` cannot cover them without extending
   `bracket/common.py::load_solve`.
3. The full 26,808-row real re-run, and a genuine `run_too_short` case on real data, are not done
   in this session (time; both would extend rather than repeat what is here).
4. `killed_particles = absorbed_by_atmosphere + absorbed_by_materials + absorbed_by_fittings` is a
   safe over-count, not the true floor-kill count (SPPS does not separate a floor kill from a
   deterministic full absorption within the same state); it can only widen the bound, never narrow
   it, but a materials table with any `alpha = 1` face would make it looser than described above
   (still safe, just not "tight in the common case" as this report characterises the Lambert-wall
   cells it tested).
