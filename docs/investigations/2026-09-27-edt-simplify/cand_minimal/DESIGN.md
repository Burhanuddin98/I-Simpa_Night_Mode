# cand_minimal: the smallest fix to upstream that is still far better

## The algorithm, in one page

Upstream (`projet_calculation.cpp`) already does the right *shape* of computation for
EDT: backward-integrate the histogram into a Schroeder dB curve, fit a straight line
over a fixed dB window with ordinary least squares, extrapolate the slope to a 60 dB
drop. Its bug is not the shape, it's the origin: `GetTimeRange`'s reference is
`row_db[0]`, the recording's first bin (emission, t=0), not the direct-sound arrival
— so silence before the direct sound arrives gets folded into the regression as if it
were part of the decay. That's a measured 0.3%-20%+ overestimate that grows without
bound as the receiver gets farther from the source (`upstream/READING.md` Sec.7),
and it's the one thing every surveyed standard and tool (ISO 3382-1, ODEON, CATT,
Treble) does differently: they all anchor 0 dB at the direct-sound arrival. For SPPS
this is not even a detection problem the way it is for a measured impulse response —
`t_arrival = distance/c` is exact, known geometry, already an input to this function.

So: **keep upstream's algorithm, move its origin.**

1. `k0 = floor(t_arrival / dt)` — the first bin that can hold the direct sound.
   Everything before it is dropped, not fit (this is I1, and also drops any
   pre-arrival Monte-Carlo noise from the total, a small free win on I5).
2. Backward-cumulative-sum `bins[k0:]` into `S`, convert to dB relative to `S[0]`
   (so 0 dB is, by construction, the arrival-onward total — exactly ISO 3382-1's
   convention, no absolute noise-floor threshold needed, unlike upstream's own
   `GetTimeDecay` for C/D/Ts/ST, which *does* need one because it isn't handed an
   exact arrival time).
3. Walk forward for the -10 dB crossing (upstream's own `GetTimeRange` logic,
   unchanged, just re-anchored). If it's never reached before the recording ends:
   refuse, `run_too_short` — a real, honest "the run wasn't long enough," not a
   silent wrong number.
4. OLS on the points from arrival to the crossing (upstream's own
   `ComputeLinearRegression`, unchanged). Guard the two things upstream doesn't:
   slope must be negative (else `not_decaying`), and CATT's own shipped floor,
   r² ≥ 0.70 (else `not_linear`) — CATT's release notes call the 0/-10 dB fit
   "seldom linear... a fundamental problem with the measure" and ship exactly this
   floor (`practice/PRACTICE.md` Sec.3.2).
5. **The range (I7)** is the fit's own 95% confidence interval on the slope,
   propagated to EDT (`-60/(a∓1.96·SE(a))`). No worst-case model, no safety
   factor — it's the scatter of the actually-recorded points around the actually-
   fitted line, which is the cheapest, most standard, most explainable range there
   is. If that interval would blow up (slope CI includes ~0), the range is capped
   and the row is flagged `wide`/`slope_uncertain` rather than shown as infinite.
6. One more cheap, borrowed gate: CATT's own minimum-decay-range rule — the
   recording should keep decaying at least another 10 dB past the fit window's
   bottom. If it doesn't, the value is still given (the fit itself is complete) but
   flagged `wide`/`near_run_end`, not refused outright — the old design's 32.7%
   real-data refusal rate came from treating "less margin than I'd like" the same
   as "can't compute," and that's the mistake being corrected here.

Ts, D50, C50, C80 reuse the same exact `t_arrival` origin as sums, not fits (no I1
exposure), with D50/C50/C80's range coming from the same kind of honest, cheap
source as EDT's — I3's real ambiguity (which bin a 50/80 ms window edge falls in),
taken as a bin-floor/bin-ceil pair, not modelled or scaled.

## I1-I7, one line each

- **I1 fit origin** — fixed: origin is `t_arrival`, exact geometry, not detection.
- **I2 direct-sound weight** — unweighted, on purpose: causality already puts it
  first in the sum; no tool surveyed weights it separately (`PRACTICE.md` Sec.5).
- **I3 discretisation** — accepted as a resolution choice (SPPS's own docs frame it
  this way), not modelled; the smoke test's own residual bias (0.66%-3.28% at 10 ms,
  <0.1% at 1 ms) is exactly this, small, and shrinks with `dt` as expected.
- **I4 unrecorded tail** — not separately modelled. EDT's window is shallow (10 dB);
  for the smoke test's r=1-30 m, EDT=0.5-2 s cases the window closes in 80-870 ms,
  long before `trans_epsilon` kills or run-end truncation plausibly matter (READING.md
  §4: the kill threshold bounds a particle's own pre-kill energy, already small). The
  `run_too_short`/`near_run_end` gates catch the case where the window doesn't close
  cleanly, which is where I4 would actually bite.
- **I5 particle noise** — not separately modelled; Schroeder integration is already
  an exact ensemble-average substitute (Schroeder 1965, `PRACTICE.md` Sec.2.1), and
  the OLS confidence interval (item 5 above) absorbs whatever scatter noise leaves
  behind in the fit window.
- **I6 degenerate curves** — `n<4` bins, arrival past run end, zero energy at arrival,
  <3 fit points, non-negative slope, and r²<0.70 all refuse by name, before a number
  is ever computed (upstream's single-bin case, which writes `NaN` with zero
  intervention, is one of these — verified refused, not crashed, in this candidate).
- **I7 display** — always a range (`edt_lo`/`edt_hi`), sourced per item 5, never a
  bare point value; `status`/`reason` say when to trust it less (`wide`) or not at
  all (`refused`).

## What this gives up against `band_early.py`

No certified, model-free worst-case bound — the range is a standard 95% regression
confidence interval, which is honest about being *statistical*, not a mathematical
guarantee. On genuinely adversarial inputs (Z3's worst-case constructions, §GATE
1-4b), it can in principle be wrong at roughly the stated rate, unlike the band's
measured 0-wrong. No `production_tail_max`/`min_run_length`/`premise_refusal`
machinery — dropped because, per the inventory, none of it ever fired on real data
(0 times in ~94,000 evaluated rows) or ever produced a value the current band
couldn't also reach some other way; if a genuinely pathological multi-source or
non-homogeneous-celerity input occurred, this candidate would likely show it as a
poor-r² `wide`/`not_linear` result rather than a specifically-named refusal — a
coarser diagnosis, not a wrong number. This candidate was not run against the
repo's own 26,808/217/3,600-row evaluation sets (out of scope here — one evaluator
runs every candidate through the shared interface); its own smoke test is 10
clean synthetic cases only.

## In one paragraph, for a user

EDT is computed the way ISO 3382-1 defines it — fit a line to the early part of the
decay curve and extrapolate to a 60 dB drop — starting the clock at the moment the
direct sound actually arrives at the receiver, not at the moment the source fired.
Upstream starts the clock at the source instead, which is why it can read up to 20%
too long for a far receiver; this fix alone removes that bias. Alongside the number
you'll see a range: it's not a worst-case guess, it's how well the recorded decay
actually fit a straight line — a tight range means a clean, confident decay; a wide
one, or a refusal, means the recording was noisy, too short, or didn't decay in a
straight line, and says so honestly instead of quietly handing you a wrong number.

## Smoke test result

`cand_minimal/smoke_test.py`, single process, 0.004 s wall time: 10 synthetic
exponential decays (EDT 0.5-2.0 s, r 1-30 m, dt 1 & 10 ms) against closed-form
truth. **10/10 accepted (`ok`), 10/10 truth inside `[edt_lo, edt_hi]`, worst error
3.28%** (EDT=0.5 s, r=2 m, dt=10 ms — a coarse step relative to a short window;
falls to 0.07% at dt=1 ms, confirming the residual is I3 discretisation, not I1
bias). Degenerate-input checks (flat/zero energy, single bin, arrival after the
run ends, a too-short run) all refuse by name with no crash and no silent number —
`run_too_short`, `no_energy_at_arrival`, `arrival_after_run_end`,
`insufficient_data`.
