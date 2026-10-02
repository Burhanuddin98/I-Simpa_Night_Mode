# Bed addendum 3: STI's open points, ruled before any STI number

2026-10-02 22:12. Technical calls on points `ADDENDUM-2-STI.md` leaves open, made by the builder before any STI value
(product or reference) was computed on any row of sets A, B7 or C7. Nothing in ADDENDUM-2 is changed. Seen before
this: only the projects' validation and the runs' progress lines, no report. Harness: `harness/stibed.py`, the shim's
`bed_shim_sti` test in `crates/simpa-core/tests/bed_shim.rs`, `harness/bands7.py`.

## Every set

1. **One row per receiver (or case) and sex.** Male (shown) and female are separate rows; the pass rules apply to
   both pooled and are also reported per sex. The reference is called once per sex with that sex's own Table A.4
   spectrum (female 125 Hz `NaN`: no speech, so it masks 250 Hz with its noise only, as the product does).
2. **The reference's series** is the band's series from the bin holding the direct sound's leading edge,
   `floor((t − R/c)/dt)`, to its end, as given: no relocation of the direct sound (ADDENDUM-1 item 4 is for windowed
   metrics; STI's MTF is defined on the response as received, and the reference takes bin 0 as the direct sound).
   Nothing arrives before that bin in any series of these sets, so the choice does not move energy.
3. **The reference refuses** when its own cl. 8.3 a flags fail: a band's series under 1.6 s, or under half the
   reverberation time it is given (set A: the row's truth T30 per band, else T20, the largest over the sex's bands;
   B7: the room's design T60 at the band, `rooms2`; C7: the band's Eyring T60). Rows where only one side answers are
   counted by side and reason and not scored; ADDENDUM-2's pass rule is on rows both answer.
4. **Answered** means the report's (or shim's) `sti.male` / `sti.female` carries a `value`.

## Set A

5. **Seven bands.** S1: the draw's one closed-form series in all seven bands (the generator has no frequency); S2:
   the unit's image-source ball echogram with each band's own continuous ISO 9613-1 air and its own fitted tail, as
   `seta.s2_unit` builds a band, at 125 Hz to 8 kHz. Steps 1 and 10 ms, every S1 slope, variant and radius as in
   ADDENDUM-1 items 1-2: 9,600 S1 cases and 636 S2 cases (318 units x 2 steps).
6. **The shim** treats each band's series as complete (noise-free, nothing after it) and builds each band as
   `results/report.rs` builds an SPPS receiver's: `decay::evaluate` with `Arrival::spread(t, R/c)` for SPL, T30, T20,
   `edt::analyse` for EDT, `from` the leading-edge bin, then `sti::receiver_sti(dt, bands, true)`.
7. **Levels, as the product computes them, re-derived in Python for the reference:** speech in band k =
   60 dB(A) + Table A.4_k + transfer_k, transfer_k = 10 lg(Σ bins_k / p0²) − 10 lg(Wρc / (4π·1 m²) / p0²),
   p0² = 4·10⁻¹⁰ Pa². Source power: S1, `Wρc = 4π d² · E_d` (the direct sound's energy is the free field at d, so
   bins are Pa²); S2, the echogram (energy x length per unit power) converted by `Wρc / (4/3 π R³)` with
   `Wρc = 413.25 · 10⁻⁴` (Lw 80 dB; the transfer does not depend on it). The shim's printed levels are compared with
   the Python ones and the largest difference is reported.
8. **Noise.** Each case is run twice: no noise, and 30 dB SPL in every band (both answer and are scored; the noise
   variant is the only place the noise term is exercised, as sets B7 and C7 have none). Rows: 2 x 2 sexes per case.

## Sets B7 and C7

9. **Levels:** the reference takes the tested run's own report levels per band (`sti.bands[].speech_male_db`,
   `speech_female_db`, `noise_db`), so the comparison isolates the energy response the runs test (the level path is
   set A's). As a sensitivity, not scored: the reference with the truth's own levels (B7: the pooled truth's SPL less
   the free field at 1 m of its `source_power_rho_c`; C7: the image-source level by ADDENDUM-1 item 6); its largest
   STI difference from the scored one is reported.
10. **B7 truth:** the reference on the two truth seeds' histograms averaged bin by bin (scorebc's `truth_b`
    pooling); the uncertainty is half the difference of the reference on each seed alone, same levels. Excluded
    above 0.01. The arrival is checked against `rooms2` geometry to 1 µs, as scorebc does.
11. **C7 reference:** `_ism.echogram` at R from the report, per band, the run's recorded air continuous, cut at 2.5
    x the band's Eyring T60 with t20p2's fitted tail, on the fine grid (2·10⁻⁵ s). One echogram per receiver at the
    longest band's length, cut per band at that band's own length (the image set to a length does not depend on the
    band: the materials are frequency-flat). Uncertainty: |STI(1.5 x cut + tail) − STI(2.5 x cut + tail)|,
    excluded above 0.01, or when a tail fit does not settle.
12. **Answered share** per room and sex is over the rows not excluded (scorebc's convention); the share over all rows
    is reported beside it.
