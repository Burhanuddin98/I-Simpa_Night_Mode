# T20, part 1: the M8a bed re-read (set S4), pre-registered

2026-10-02. This is committed before any T20 number from the M8a runs is computed. Sources:
- `docs/investigations/2026-10-02-m8b-metrics/PLAN.md` (T20 section, J6), decision-log rows 1, 17, 18 and 37.
- M8a `SPEC.md` sections 4 and 5, which this copies for T20.

## What this part tests, and what it cannot

- **Tests:** whether the product's T20 on the 433 M8a runs (Lambert boxes, both modes, user counts) agrees with
  the independent Lambert transport's T20 on the same cells, to the tight 0.5 % that gate C held T30 to.
- **Cannot test:** anything the solver and the transport both assume, because they share the Lambert model
  (crucible B3). T20 is therefore not done on this part alone. Part 2, the closed-form decays (S1) and the
  image-source echograms (S2), is the independent reference. It is pre-registered separately, before it runs.

## Data and run

- **Runs:** `C:\tmp\nm-m8a-bed\20260929T093134Z`, read only. No new solver run.
- **Re-read:** `simpa bed --from` with the T20 twin (transport T20 from the same transport pass as T30). Output
  goes to `B:\data\m8b-t20\`.
- **Build:** pinned by commit in `RESULT-S4.md`.

## Precondition P0: the twin changed nothing in T30

The re-read's T30 gates A, B and C give, cell for cell, the same verdicts and statistics as M8a's
`report.json` (`REJUDGE.md`'s numbers). Statistics are equal to 1e-12 relative, or exactly where the transport is
seeded identically. If P0 fails, nothing below is read; the twin is fixed first.

## The value each seed gives

As M8a SPEC section 4, with `t20_s` in place of `t30_s`:
- A value is used as it stands.
- A `monte_carlo_noise` or `noise_uncalibrated` refusal gives its own value.
- Any other refusal leaves the cell's T20 not judged, named with its seed, receiver, band and code, and counted.

## Gate C-T20 (gated)

The statistic is SPEC 5.1's C, with T20:

- `y_s = mean_{r,b} T20_{s,r,b} / T20_tr,b − 1`
- `d` = the mean over the seeds
- `SE² = var_s(y)/k + se_tr²`, with `se_tr` the transport's relative standard error, averaged over the cell's bands
  as if fully correlated

Outcomes:

- **PASS:** `|d| + t·SE ≤ 0.5 %` (`t` = 2.262 for k = 10).
- **FAIL:** the interval excludes 0 and `|d| > 0.5 %`.
- **INCONCLUSIVE:** anything else.

An INCONCLUSIVE cell may be extended once, by seeds 11 to 20, but only where those runs already exist on disk.
New runs for an extension are put to Burhan first, with their cost. Random and energetic are judged separately.

**Part 1 passes** when every gated cell PASSes, or PASSes on its one extension, and no cell's T20 is left
unjudged by a non-noise refusal. A cell left unjudged is reported by code. It fails part 1 unless the same cell's
T30 was also unjudged in M8a (the bed's own exclusion, E6).

## Reported, not gated

- T20 against Kuttruff, statistic A. T20 and T30 differ legitimately by curvature, and Kuttruff is T60-like.
- The cell mean's seed spread, statistic B, for T20.
- **Range coverage (decision 37 (3)):** per seed, receiver and band, whether the transport's T20 lies in
  `T20 ± 2.5·mc_sd + 0.5 %`. Counted per cell and mode, with wrong-silent (outside that range and beyond the 5 %
  JND) counted separately. This is reported here and gated in part 2.
- The product's T20 refusal rate at the 1 ms step, by reason.
- Already measured, `../2026-10-02-edt-ball-vs-point/RESULT.md`: the -5 dB start is at least 14.0 ms after the
  onset on all 7,680 M8a decays, against a 1.8 ms ball crossing.
