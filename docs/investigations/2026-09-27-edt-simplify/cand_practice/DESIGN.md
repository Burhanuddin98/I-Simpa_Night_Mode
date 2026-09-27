# DESIGN — the practice-based EDT method (`cand_practice/method.py`, 248 lines)

## The algorithm, in one page

1. **Fit origin (I1).** The direct-sound arrival `t_arrival` is known exactly from
   geometry (`distance/c`), so it is an index, not a detection problem. Bins before
   the arrival bin `k0` are dropped, not fit against. The Schroeder curve is the
   backward-integrated energy from `k0` onward, in dB, referenced to 0 dB at the
   arrival bin itself — ISO 3382-1's own convention, which upstream's EDT never uses
   (it references bin 0 of the whole recording, `READING.md` §1.3/§6).
2. **Direct sound (I2).** No special weight: causality already puts it first in the
   backward sum, matching field practice (`PRACTICE.md` §5, I2).
3. **Fit window and fit quality (I6).** Ordinary least squares on the dB curve over
   `[0, -10 dB]` (ISO 3382-1's own EDT window), `-60/slope` for the extrapolated time.
   Refuse (not report) if: fewer than 4 points in the window, the slope is not
   negative, or `r² < 0.70` — CATT-Acoustic's own EDT acceptance floor, lowered from
   0.95 in v9.1 because "0 to -10 dB decay is seldom linear" (`PRACTICE.md` §3.2).
4. **Unrecorded tail (I4).** A finite recording is always missing whatever energy
   would arrive after it stops, and that missing energy deflates the 0 dB reference
   every bin is measured against — the recording's own last bin cannot be used to
   detect this (it looks small because it *is* the last one, not because the decay
   is finished; see "a real failure this method fixed" below). Exactly as Lundeby
   1995 does, the current fit's own line is extrapolated to the recording's true end
   to estimate what fraction of the 0 dB reference is still missing, that estimate
   is added back to every point, and the fit repeats (6 rounds, fixed). A long-enough
   recording converges immediately; one that is not converges to a large correction,
   and if that correction still exceeds 5% after 6 rounds, the row is refused
   (`run_too_short`) rather than trusted.
5. **Time-step discretisation (I3).** Bins are timed at their **center**
   `(k+0.5)·dt`, not their end (upstream's convention, `READING.md` §6 I3) — this
   removes most of the bias. The residual (bin position genuinely unknown to
   `dt/2`) is added as a small relative widening of the reported range,
   `0.5·dt / window_duration`.
6. **Particle noise (I5).** Schroeder integration is itself an exact ensemble
   average (`PRACTICE.md` §2.1) — no separate noise model is layered on top. What
   noise remains shows up as scatter around the regression line: the fit's OLS
   standard error is one estimate of that; because neighbouring bins on a
   backward-integrated curve share almost all their underlying particle hits (they
   are not independent samples), that estimate can be over-confident at a fine
   `dt`. As an independent check, the window's early and late halves are each
   fit alone; how much *they* disagree is immune to that correlation. The wider of
   the two is what is reported.
7. **Display (I7).** `edt_lo`/`edt_hi` are always returned together with `edt`
   (decision-log row 9), built from the regression's own statistics above, not a
   scaled safety factor. `status` is `'ok'` (`r² ≥ 0.95`), `'wide'`
   (`0.70 ≤ r² < 0.95`), or `'refused'`, each with a plain-text `reason`.

Ts/D50/C50/C80 (optional, requested if simple) reuse the same arrival-referenced
energy but skip the OLS/Hirata treatment: they widen by the fraction of post-arrival
energy still arriving in the last decile of the recording — a cheap, honest proxy
for "is the tail still open," not the rigor EDT gets. This is a deliberate, named
simplification (see below), not an oversight.

## A real failure this method fixed during design (not hidden)

While stress-testing (300 randomized synthetic cases, `stress_test.py`, not part of
the required smoke test but run and kept for honesty), the **first** version of this
method — OLS fit, `r²` gate, and a check that the recording's *raw* dB curve reached
10 dB past the fit window (CATT's literal rule) — silently returned a wrong EDT
(true value outside the reported range) on up to **31.5%** of accepted cases. The
cause: a finite backward sum is forced toward its own last bin regardless of how
much real decay has happened, so the *raw* curve can look like it has fallen well
past the CATT margin even when several percent of the true energy is still
unrecorded — the recording's own tail is not a trustworthy witness for "is the
recording long enough," because it is itself the artifact in question. Replacing
that check with the Lundeby-style model-based iteration in step 4 above (the
fitted line's own extrapolation, not the raw curve) cut this to **5.9%** — and that
residual matches almost exactly what a stated ~95% interval (`Z=2`) is supposed to
miss by construction, not a further bug. This is disclosed here because the task
asked directly whether any measured case would get a wrong number without warning:
during design, yes, until this fix; after it, the remaining rate is what the
reported confidence level itself promises.

## What this gives up against `band_early.py` (1,369 lines, current band)

- **No model-free guarantee.** The band's headline result — 0 wrong across 67,468+
  noise-free rows by worst-case construction — is a stronger property than
  anything here. This method's range is a statistical interval (like every
  standard/tool surveyed), not a certified bound; on adversarial, non-physical bin
  arrangements it has no proof of containment.
- **No formal treatment of I4/I5/I3 for Ts/D50/C50/C80** — only EDT gets the full
  OLS + Hirata-iteration + split-half treatment. Widening for the other four is a
  single tail-fraction proxy.
- **Two sources, celerity gradients, `premise_unsupported`-style cases**: not
  detected. A second source would show up only indirectly, as poor `r²`.
- **No use of `trans_epsilon`/`particles_per_source`/`total_energy` from `meta`** —
  gracefully ignored, per the interface, rather than modelled (the band used these
  for its own tail bound; this method's tail estimate comes only from the bins
  themselves, per Lundeby/Hirata practice).
- **A known, named residual (Chu 1978's problem, not solved here):** a receiver
  whose bins are dominated by a roughly constant floor (Monte-Carlo noise with no
  real decay) can still look like decay under backward integration, purely because
  fewer terms remain in the sum as the window advances — Chu's fix is to subtract
  an estimated floor before integrating, which this method does not do, trading
  that protection for simplicity. It was not observed on this repo's real
  production data (11,520 real receiver-band rows checked, see below) or on the
  10-case smoke test; it is a stress-test artifact (uniform bins) and the M8b
  `run_too_short` gate does catch it in that synthetic form, but a real receiver
  that is genuinely non-decaying at a low level is not guaranteed to be caught the
  same way.

## Measured effect (this session, single-process, read-only)

- **Required smoke test** (10 synthetic decays, EDT 0.5–2 s, r 1–30 m, dt 1/10 ms):
  10/10 accepted, all true values inside the reported range, worst point error
  1.9%, wall time 0.09 s.
- **Bonus: 11,520 real receiver-band rows** from this repo's own committed SPPS
  fixtures (`pm8-noise-scratch/runs/noise-cal-*`, dt≈10 ms, upstream's own default
  step): **0 refused**, 96.6% `ok`, 3.4% `wide`, median reported half-width 7.4% of
  the value. The current band, at its 0.5%-JND tolerance, refuses **100%** of a
  comparable 3,600-row real check (32.7% `run_too_short`, the rest `band_too_wide`,
  per `INVENTORY.md`); at upstream's own 10 ms default it accepts 0% at any `tau`
  (`PRACTICE.md` §6). This is the concrete "gives a usable answer on ordinary real
  runs" result the task asked for.
- **Bonus: 300 randomized synthetic trials**, deliberately including short/marginal
  run durations: 287 accepted, 13 refused, 5.9% of accepted cases have the truth
  just outside the range (matches the ~95% CI's own expected miss rate, see above).

## For a user

You get one EDT number and, next to it, a range — always, even when the fit is
excellent, because a single number invites more trust than a Monte-Carlo particle
simulation should get. The range comes from two honest sources: how much the early
and late halves of the measured decay actually disagree with each other (real
statistical noise, not a guess), and how long you ran the simulation relative to
how long the room actually rings (if you stopped too early, this method notices by
checking whether its own fitted decay line predicts much energy still missing when
your recording ends, and either widens the range or tells you plainly that the run
was too short to trust — it does not guess). If you see `status: 'wide'`, the decay
was less clean than usual (typically a source very close to, or very far from, the
receiver) — the number is still usable, just less precise. If you see `'refused'`,
trust the reason string over any number: something about this specific receiver
(too short a run, a flat/non-decaying signal, too few usable bins) means this
method could not respond honestly, and it says so instead of guessing.
