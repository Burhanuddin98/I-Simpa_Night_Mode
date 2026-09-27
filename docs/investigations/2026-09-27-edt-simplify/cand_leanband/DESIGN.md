# LeanBand: EDT for SPPS histograms, simplified

## The algorithm, in one page

1. **Origin (I1/I2).** `t_arrival` is given (geometry: `distance/c`), not detected. Find
   `k0 = floor(t_arrival/dt)`, nudge forward up to 2 bins if that bin is exactly zero
   (bin-quantisation slack, I3). Compute the backward (Schroeder) tail sum
   `tail[i] = sum(bins[i:])` and convert to dB *relative to `tail[k0]`* — so 0 dB is, by
   construction, the direct-sound arrival, matching ISO 3382-1 and every surveyed tool
   (`upstream/READING.md` §1.1, `PRACTICE.md` §1.1/§5). The direct-sound bin gets no
   special weight beyond being first (I2) — causality already anchors it.
2. **Fit window.** Walk forward from `k0` for the first index where the dB curve drops
   to −10 dB *and stays there for 2 consecutive bins, both still inside the recorded
   array* (§"the cliff", below). Refuse (`too_few_points`) if that happens in fewer than
   4 bins — this is what catches a direct-only or truncated-to-nothing receiver (I6).
3. **Regression.** Ordinary least squares, bin-centre times vs. dB, over `[k0, i_end]`.
   `EDT = -60/slope`. Refuse `not_decaying` if the slope isn't negative.
4. **Run-length gate (I4), replacing the ×1000 factor.** Extrapolate the *fitted line* to
   where it would reach −20 dB and require the recording to actually be that long (CATT's
   own rule: fit range + 10 dB of margin, `PRACTICE.md` §3.2). If not, refuse
   `run_too_short`. See "the cliff" for why this has to use the fit, not the raw curve.
5. **Range (I5/I7).** The regression's own standard error on the slope gives a ~95% CI
   (`slope ± 2·SE`), converted to `edt_lo`/`edt_hi` (capped at 10× the point estimate if
   the interval can't rule out zero slope). `status='ok'` if that half-width is under the
   ISO Annex A JND (5%) and r²≥0.5, else `'wide'`. This one mechanism replaces a separate
   noise model, a separate regression-quality gate, and a separate tail-safety-factor: a
   noisy run or a poor fit both show up as a bigger standard error, nothing else needed.
6. Ts/C50/C80/D50: direct energy-ratio point estimates from the same `k0`, no band
   attempted (`status='wide'`, honestly labelled `point_estimate_no_ci` — see "gives up").

## The cliff: why the run-length check can't just re-read the curve

Every finite histogram's *last* recorded bin is, by construction, the sum of just that
one bin — there is nothing after it to add. Near the end of any array, the backward tail
sum therefore drops faster than the room is actually decaying, purely because the array
ran out, not because of physics. A synthetic test that truncates an EDT=1 s decay exactly
at its own true −10 dB point still shows a "confirmed", 2-bin-persistent crossing to
−13 dB in the last two bins (measured in this session's scratchpad) — the cliff, not
decay. Searching the raw curve again for a deeper (−20 dB) crossing does not fix this: the
same cliff forges a "confirmed" −20 dB crossing a few bins later in the same way. The fix
that survives this (step 4 above) never re-reads the corrupted tail: it asks whether the
recording is long enough for the *already-fitted, uncorrupted* line to reach −20 dB, and
refuses otherwise. Measured effect: without step 4, that same truncated-at-its-own-point
case reports EDT 27% short of the truth with r²=0.99 and would show `status='ok'` — a
wrong number with no warning. With it, that case and everything shorter is refused; the
smoke test's 10 well-recorded cases (below) are unaffected (huge margin either way).

## I1–I7, one line each

- **I1** fixed structurally: fit starts at `t_arrival`, not emission. This is the
  documented 0.3–20% upstream bias (`upstream/READING.md` §7.2) at zero extra cost.
- **I2**: no special case needed (causality); documented, not built.
- **I3**: bin-centre times, not bin-end labels; a few-bin arrival search. Residual,
  small, unmeasured further — matches field practice of treating it as resolution, not
  an uncertainty term (`PRACTICE.md` §5 I3).
- **I4**: the run-length gate (step 4) for "did we record enough"; `trans_epsilon`-killed
  energy is not modelled — `PRACTICE.md` §4 argues it is trivially small by construction,
  and it was never measured to matter for EDT in the inventory either.
- **I5**: folded into the regression's own standard error, plus the 2-bin persistence
  requirement (a single noisy dip can't end the window). No per-bin eps model.
- **I6**: refusal reasons cover empty input, arrival past the recording, no energy after
  arrival, too few points, non-decaying, and (via step 4) too short a run. A two-source or
  non-single-exponential curve isn't special-cased — it shows up as low r² and a wide (or
  capped-open) band instead, same as field practice (`PRACTICE.md` §5 I6).
- **I7**: EDT always ships `edt_lo`/`edt_hi` and a status (Burhan's decision-log row 9).
  Ts/C50/C80/D50 ship a value too, but honestly flagged as a point estimate, not a band.

## What survives from the old band, what's dropped

**Survives:** a real range on EDT, always shown, never silently a bare number — the part
of decision-log row 9 that matters. The core insight that a *point value* is the wrong
shape of answer for a Monte-Carlo histogram survives completely; only its machinery is
replaced. The direct-arrival fit origin (I1) is a genuine, measured, zero-cost fix, kept
in full. The `run_too_short` refusal concept is kept, but re-derived from a standard
(CATT) rule instead of an uncalibrated ×1000 diffuse-field guess.

**Dropped:** the certified, model-free, worst-case-arrangement guarantee ("0 wrong under
any admissible energy arrangement inside a bin") — replaced by an ordinary OLS confidence
interval, which is a statistical statement, not a proof. `spps_rel_eps`/air-rate
correction, `safe_contributions_per_bin`, `production_tail_max`'s diffuse-field model,
`premise_refusal`'s two branches, and the GATE 4b twin-solve for Ts: all gone. Per the
inventory, none of these ever changed an accept/reject outcome on real production data
(air-rate: zero real rows exercised it at all; `premise_unsupported`/`arrival_misfit`:
zero occurrences in ~94,000 evaluated rows; Ts's twin-solve: zero non-`None` results in
the one real check that ran it) — dropping them costs nothing measured, and ~900 of
1,369 lines of `band_early.py`/`SPEC.md`'s own machinery inventory.

**Would any measured case now get a wrong number without warning?** Not among the cases
this session could check (10 synthetic decays; the run-length boundary case that exposed
the cliff and was then fixed; flat, empty, direct-only, rising, arrival-past-record,
and a two-slope bump — all refuse or flag `wide` correctly). The genuine trade against the
old design's headline claim: this is no longer a *proof*. An adversarial or highly unusual
histogram could in principle fool the OLS confidence interval the way any statistical
interval can be fooled — that risk was exactly what the dropped machinery existed to rule
out by construction. This design accepts that risk in exchange for actually answering on
ordinary runs, which per the inventory the old design did not (0/3,600 on the one
real-production check).

## For the user

EDT is read off the loudness decay curve at your receiver, starting from the moment the
direct sound actually arrives (not from when the source fired) down to 10 dB of decay,
then scaled up to the standard 60 dB definition. Because the simulation is built from a
finite number of sound particles, that curve is noisy, so instead of one number you get a
range: a tight range means the run gave a clean, confident reading; a wide range means the
receiver was noisy, poorly fit, or borderline, and you should treat the number with more
caution. If the room simulation didn't run long enough to actually see the decay happen,
you get a plain refusal instead of a guess — an "I don't know" is safer than a wrong "1.2 s".

## Smoke test

`analyse()` against 10 closed-form exponential decays (EDT 0.5–2.0 s, r 1–30 m, dt 1 and
10 ms), single process, elapsed 0.01–0.02 s. All 10 accepted (`status='ok'`), worst error
1.89% (dt=10ms, r=30m — the coarsest, farthest case), median error well under 1%, all CIs
comfortably inside the 5% JND line. Full output is in the session's scratchpad
(`smoke_test_leanband.py`).
