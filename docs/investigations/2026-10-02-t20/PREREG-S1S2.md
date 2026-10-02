# T20, part 2: exact references (sets S1 and S2), pre-registered

2026-10-02. Committed before any part-2 number is computed. This is the independent half of T20's test (crucible B3):
the references share no model or code with SPPS or the Lambert transport. A crucible review of the first draft
(SOUND-WITH-FIXES, 1 blocker, 4 majors) is resolved in this text.

## What is tested

The product's own T20 code: `params::decay::evaluate` → `BandParameters.t20`, which is what `parameters.t20_s`
reports. It is fed noise-free histograms through a test-only shim. The shim takes bins, dt, an `Arrival` and the
half-width, and returns the value or the refusal code. It follows `tests/edt_port.rs`'s fixture pattern. Its source
hash is pinned in `RESULT-S1S2.md`.

**Arrival:** `Known` at the direct sound's centre time, including any emission delay rounded up to the next whole
step, with half-width R/c, exactly as `report.rs` builds it for an SPPS receiver.

**Not covered here:** the noise path (bootstrap `mc_sd`, `monte_carlo_noise`, `noise_*` refusals), the likeliest T20
failure on a short window. It is tested at user counts on S4 (part 1) and in part 3. T20 is not done without part 3.

## Truths

Both truths follow ISO 3382-1 cl. 6: least squares on the Schroeder curve, -5 to -25 dB, 0 dB is the **total
energy including the direct sound** (Eq. 1), and `T20 = -60/slope`. Both are computed by round 2's checked
exact-integral fit, `_mirror.line(top, pieces, dt, top_db=-5, bottom_db=-25)`, through a `truth_ideal`-shaped
wrapper with only the range changed.

- **Ball truth:** on the ball's fine-grid echogram, i.e. what the solver records. The product is scored against it.
- **Point truth:** on the point receiver's fine-grid echogram (S2 only), i.e. what ISO's microphone reads.

## Sets

**S1, closed-form decays.**
- `harness2/m8b/synth_fresh.py`'s generator, including its emission steps: direct step, gap, single or double slope.
- 400 draws from seed 20261002 over the generator's own ranges. The realised T60 range is reported.
- Steps 1, 2 and 5 ms; half-widths for R = 0.1, 0.31 and 0.5 m. Synth has no geometry, so the ball and point truths
  coincide.

**S2, image-source echograms of specular boxes.**
- `harness2/m8b/_ism.py`, at 1 kHz and 8 kHz. Air is continuous for the truth and per step for the product, as in
  round 2's `make_row`.
- The six rooms of `../2026-10-02-edt-ball-vs-point/PREREG.md`, plus 30 boxes drawn from seed 20261003:
  - each side uniform in 3 to 25 m;
  - wall α uniform in 0.03 to 0.5, independently per wall;
  - a draw whose Eyring T60 at 1 kHz is over 3 s is rejected and redrawn, and the rejections are counted;
  - source and receivers placed as in that PREREG.
- Radii 0.1, 0.31 and 0.5 m; steps 1, 2 and 5 ms.
- **One echogram per row, to 2.5 × Eyring T60.** Truth and product read the same series.
- **Convergence:** the truth on the series cut at 1.5 × and at 2.5 × Eyring T60 must agree to 0.05 %. Otherwise
  the row is `truth_truncated`: counted, not scored. **If more than 10 % of a set's rows are `truth_truncated`, the
  set is INCONCLUSIVE.** A product refusal `truncated` on a converged row counts against J4.

**Range not covered:** T60 above 3 s. The product accepts 0.1-10 s. That band is declared untested by part 2, and
is listed for part 3 and the backlog.

## Criteria, per set

The thresholds are carried over from EDT (H1/H4, PLAN.md J1/J4). They are not from a standard: ISO 3382-1 lists no
T20 limen (`STANDARDS-CHECK.md`).

- **Answered:** a value, not a refusal.
- **Wrong-silent:** answered and `|T20 / ball truth - 1| > 5 %`.
- **J1-a:** wrong-silent ≤ 0.5 % of answered rows in each set, at every radius and every step, 5 ms included. A
  product that cannot resolve a short window must refuse rather than answer wrong.
- **J1-b:** ≥ 99 % of answered rows with `|T20 / ball truth - 1| ≤ 0.5 %` at the 1 ms step. Reported at 2 and 5 ms.
- **J1-c (ball vs point, S2):** `|ball truth / point truth - 1| ≤ 0.5 %` in ≥ 99 % of rows with R ≤ 0.5 m. This is
  the metrics plan's M2 gate. The worst row is reported.
- **J4:** ≥ 90 % of converged rows answered at 1 ms, T60 ≤ 3 s, R ≤ 0.5 m. Refusals are reported by reason, set
  and step.

## Say-NO, run first

If any of these fails, the check is INCONCLUSIVE.

- (a) A fit planted over 0 to -20 dB on double-slope Synth rows fails J1-b on that subset.
- (b) A planted 2 % scale error on the product's value fails J1-b.
- (c) A truth anchored at the reflected energy only (direct sound left out of 0 dB) differs from the true truth by
  more than 0.5 % on S2 rows with d ≤ 3 m. This shows the anchor matters and is being checked.
- (d) An arrival planted one step late makes the product's value differ from its own unplanted value on at least
  one S1 row at 1 ms. This shows the shim passes the arrival through.
- (e) The truth wrapper on a pure exponential returns its known T to 1e-6.

## Pass

**Part 2 passes** when J1-a, J1-b and J4 hold on S1 and S2, and J1-c holds on S2.

**T20 is done** when part 1 (S4), part 2 and part 3 pass. Part 3 (fresh SPPS rooms, high-count truths, coverage
of the shown range at user counts, and T60 above 3 s) is pre-registered after part 1's result, which sizes it.
