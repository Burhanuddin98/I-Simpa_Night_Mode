# The bed, Addendum 1 (before any scored row)

2026-10-02 19:00. Technical calls on points `PREREG.md` leaves open, made by the builder before any scored number
of sets A, B or C was computed (decision-log division of labour, 2026-09-27). Nothing in `PREREG.md` is changed.
Seen before this addendum: only the shim's own wiring test (a pure exponential, `bed_shim.rs`) and one set B truth
run's `simpa results --json` read for its schema (G1, seed 9201: field names, statuses; no value compared to anything).

## Set A

1. **S1 rows.** T20 part 2's 400 draws (seed 20261002, `t20p2.s1_draws`, d redrawn below 0.7 m), each run
   - as its **double slope** (the drawn ratio, 1.5 or 5) and as a **single slope** (the same draw with the slow
     slope removed: `A = [1]`, `k = [k1]`, `Ed` from the DRR on that reverberant total), the PREREG's "single and
     double slope";
   - at steps **1 and 10 ms** (the PREREG's "1 and 10 ms"; T20 part 2's 2 and 5 ms are not repeated);
   - at half-widths for R = 0.1, 0.31 and 0.5 m;
   - in both of T20 part 2's variants: **long** (to 2.5 × the slowest slope's T60) and **short** (the generator's own
     run, 0.3 to 3 × T60). Both count: a noise-free series too short for a metric must be refused, not answered
     wrong. Results are also reported per variant.

   9,600 rows per metric. Truths are closed forms of the generator's model, infinite length (item 4).
2. **S2 rows.** T20 part 2's 36 boxes and 318 units (room × receiver × R), 1 kHz and 8 kHz, steps 1 and 10 ms
   (1,272 rows per metric). Each row's series is the ball echogram to 2.5 × Eyring T60 with T20 part 2's fitted tail
   (`t20p2.extend`, its ADDENDUM-1 item 4). **Air is continuous in the series the product reads, as in the
   truth's.** T20 part 2 gave the product per-step air. That models SPPS, not the metric code. At 10 ms and 8 kHz the
   difference is near 0.1 dB in SPL, which is 1/10 of SPL's limen. The product reads the truth's fine echogram,
   summed into its steps.
3. **S2 truth uncertainty** is the truth from the 1.5 × cut plus its own tail, minus the truth from the 2.5 × cut
   plus its tail, in absolute value. A row is excluded and counted when this exceeds 1/10 of the limen, or when a
   tail fit does not settle (T20 part 2's `truth_truncated`). S1's truths are closed forms, with no uncertainty.

## Truths, every set

4. **Time zero** is the direct sound's centre, `t0 = r/c` (plus the emission delay in S1). The truths follow
   ISO 3382-1 A.2.3 and `docs/params.md` ("Clarity C50 and C80, definition D50, centre time Ts"; "Sound pressure
   level"). `u = t − t0`.
   - **The direct sound is at `u = 0`.** ISO's point microphone receives it there, and so does the product's
     model. Where the series separates it (S1, S2 and C, by image order 0), all of it is placed there.
   - Reflected energy is placed where the ball records it, spread evenly inside each fine bin. Any part of it that
     falls before `t0` is placed at `u = 0`, as T20 part 2's `truth20` does.
   - **Set B** cannot separate the direct sound in an SPPS histogram. There, every energy recorded before
     `t0 + R/c` (the end of the ball's direct sound) is direct, at `u = 0`. The bin that holds that time is split in
     proportion to the time on each side.
5. **The metrics.**
   - `C_te = 10 lg(E[0, te] / E(te, ∞))`.
   - `D50 = E[0, 50 ms] / E_total`.
   - `Ts = ∫ u E du / E_total`.
   - `SPL = 10 lg(E_total / (4·10⁻¹⁰))`.

   In each:
   - a window edge inside a bin splits that bin in proportion;
   - `E_total` is all the energy in the series, its tail extension included (S2, C), or the closed form's
     infinite total (S1).

   T20 and T30 use `truth20.t20` (T20 part 2's checked line: −5 to −25 dB and −5 to −35 dB). Set B passes it the
   item-4 split.
6. **Units for SPL.** In set A the bins are the generators' own units. SPL is then a check of the sum and of the
   tail, against the same 4·10⁻¹⁰. In set C the image-source echogram (energy × length per unit source power) is
   converted to Pa² by `source_power_rho_c / (4/3 π R³)`. It is read from the run's report, as SPPS's
   `energy_sum × ρc / V_receiver` gives (`docs/params.md`, "What a value is"). The power conversion itself is set D's
   to test, not set C's.

## Sets B and C

7. **Rows.** Set B: room × receiver × band × tested seed, 7 × 8 × 6 × 3 = 1,008 per metric. Set C: 2 × 3 × 6 × 3 =
   108 per metric.
   - **Answered** means the report's parameter carries a `value` (`ok` or `wide`).
   - **The shown range** is its `lo`, `hi`. Wrong-silent and covered are as `PREREG.md` defines them.
   - For T20 the limen is 5 % of the truth.
   - The report is `simpa results <run folder> --json`, from the build named in the scorer's output.
8. **Set B truth.** The metric on the two truth seeds' histograms summed bin by bin. The time zero is the truth
   report's `arrival_s`; the scorer checks it against the room's geometry (`rooms2`) to 1 µs. The **uncertainty** is
   half the difference between the metric on each seed alone (the standard error of a mean of two).
9. **Set C rooms:** `S-live` (6 × 5 × 3 m, α 0.10 everywhere) and `Mixed` (10 × 7 × 3.5 m, α 0.2 on the walls, 0.5
   on the floor and 0.1 on the ceiling). They are one uniform live room and one uneven room, both small enough for
   the image sources to 2.5 × T60.
   - Each project is a round-2 G-room project with its geometry, materials (scattering 0, α the same in every band),
     source and three receivers replaced.
   - The source and receivers are at S2's fractions of the box; every receiver is at least 2 m from the source.
   - Receiver radius 0.31 m. New-project SPPS defaults; seeds 4101-4103.
   - **Reference:** `_ism.echogram` at R from the report, per band, with continuous ISO 9613-1 air at the project's
     20 °C, 50 %, 101,325 Pa. It is cut and tail-extended as in S2, at 2.5 × the band's Eyring T60, air included.
     The reference uncertainty is that of item 3. The speed of sound is the report's.
10. **Answers often enough** (criterion 3) is reported per metric and room. Every G room and both C rooms have
    T60 ≤ 3 s at 1 kHz.
