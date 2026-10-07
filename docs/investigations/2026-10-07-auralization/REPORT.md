# C5 report: auralization in v1 (2026-10-07 22:40 to 2026-10-08 01:45)

Written by the main thread from the C5 builder's final report (the harness refused the builder's own write of this
file); the method and the numbers rewritten in place by the builder after the audit of 2026-10-08 (verdict
SOUND-WITH-CAVEATS). Branch `aural` (from `gpu` 2e6345a). Spec: `SPEC.md` beside this file (Burhan's answers of 22:39:
48 kHz; per source and summed; the solver's own bands; a few verified clips). Decision 75; scope row; backlog 96-100.

## Done-when

| item | holds | receipt |
|---|---|---|
| 1 core | **partly** | 17/17 unit tests. CR4 bed, each source-receiver pair's EDT and T30 by the app's own results code, 3 seeds: **1,405 of 1,428** values within 5 % through the IEC-like bank (the headline), **1,429 of 1,430** through the steeper bank. Every miss is listed (backlog 99) |
| 2 CLI | yes | `simpa auralize ... --source-audio <wav>` (`--anechoic` kept as an alias), cli_auralize 3/3 |
| 3 app | yes | Results step: synthesised IR Play/Stop/Save per source or summed; Auralize with the bundled clips or the user's WAV; level meter |
| 4 clips | yes, with a caveat | 3 clips, licences verified; **dry and close-miked, not anechoic**, and the app says so (backlog 100) |
| 5 e2e | yes | m13.aural 5/5 after the audit fixes (`C:\tmp\nm-target-aural\gates\m11\20261008-012009`); before them, dock + 12 m12 specs + m13.blank + m13.aural 59/59, focus PASS (`...\20261007-235541`) |
| 6 docs | yes | decision 75, scope row, backlog 96-100 (these follow `gpu` c72dfc1's highest row, 95: no renumbering was needed) |

## Method

**The synthesis** (`crates/simpa-core/src/auralize.rs`, module docs). The plan's method (white noise per band times
the square root of the band's energy envelope) missed CR4's EDT/T30 by 10-40 %, measured with the module's own
estimator. Built instead: per band, Gaussian noise drawn on the band with its envelope flattened (four alternating
projections), multiplied by the root of the echogram's energy-preserving envelope, band-limited again through a smooth
bank (otherwise the echogram's 1 ms Monte-Carlo modulation spreads energy into neighbouring bands: T30 +18 % at 500 Hz
on CR4), and the envelope corrected through that bank in 3 passes. **Fixed after the audit:** the second band-limit's
FFT was padded by only 0.92 s, so a low band's acausal ringing wrapped into the response's last seconds (EDT v2.1
refused 125 Hz pair-bands as `not_decaying_at_run_end`); it is now padded to twice the length (1bdfebc).

**The bed** (`crates/simpa-core/tests/auralize_cr4.rs`), rebuilt after the audit to be independent of the synthesis:

- **Estimator: the app's own.** For every receiver and every source, the synthesised response is analysed back
  through a measuring bank and its per-step band energies put in place of that source's echogram in a copy of the run;
  `results::report` (what `simpa results` prints: EDT v2.1, and T30 with its floor and tail checks) is run on the run
  and on the copy. Only source-receiver pairs are graded: the app refuses EDT and T30 for the sources summed (ISO
  3382-1 defines them per pair). The module's own Schroeder estimator (`auralize::decay_times`) agrees with the report
  on the echograms to **1.31 % (EDT) and 0.03 % (T30)** in octaves, 1.06 % and 0.03 % in thirds; it is kept for the
  synthetic unit test only, and the bed no longer uses it.
- **Two measuring banks**, both Butterworth band-pass magnitudes on the run's own edges, applied zero-phase,
  independent of the synthesis's filters: the **IEC 61260 class-1-like bank**, prototype order 3 (band-pass order 6),
  and a **steeper bank**, prototype order 6 (band-pass order 12). **The headline uses the IEC-like bank**, because it
  is the kind of filter ISO 3382-1 measurement uses; it is the less favourable of the two, so it is not chosen for its
  result. The steeper bank is reported beside it because its skirts let less of a neighbouring band into a band.
- **Three noise seeds** (0x…4c31, 0x…4c32, 0x…4c33).
- **A floor of −100 dB** below each band's largest step is zeroed on the response side: 30 dB under SPPS's own
  (`trans_epsilon` 7) and far above the FFT's round-off, which EDT v2.1 otherwise reads as a decay still going at the
  run's end.
- **The regression guard:** every value outside 5 % must be one listed in `KNOWN` (backlog 99) by bank, set, seed,
  receiver, source, band, quantity and sign; a new miss fails, and a listed one may not grow past 6 % (or past its
  recorded size, for those already beyond: the synthesis and the report are deterministic). The 5 % tolerance is not
  loosened.

## Numbers (REALISED)

**CR4 bed** (runs `.out\aural\cr4-oct\runs\20261007-224710-748-spps`, 6 octaves 125 Hz-4 kHz, and
`.out\aural\cr4-third\runs\20261007-224716-245-spps`, 18 thirds 100 Hz-5 kHz; 300 k particles, 10 s at 1 ms, maps off;
5 receivers × 2 sources; receipt `.out\aural\bed\receipt.json`, log `.out\aural\bed-run3.txt`):

| Bank | Set | Within 5 %, per seed | Worst \|ΔEDT\| | Worst \|ΔT30\| |
|---|---|---|---|---|
| IEC-like (band-pass order 6), headline | octaves | 113/120, 113/120, 116/119 | 3.79 % (4 kHz) | **8.86 %** (500 Hz) |
| | thirds | 354/357, 356/356, 353/356 | **6.20 %** (400 Hz) | **7.22 %** (400 Hz) |
| steeper (band-pass order 12) | octaves | 120/120, 120/120, 120/120 | 4.35 % (500 Hz) | 4.95 % (4 kHz) |
| | thirds | 357/357, 356/356, 356/357 | **5.08 %** (160 Hz) | 4.78 % (400 Hz) |

Not measured (a refusal on either side; the reason is in the receipt): at most 1 of 120 per seed in octaves, 3-4 of
360 in thirds. MP1/LS1 800 Hz and MP3/LS1 1250 Hz T30 are refused on the echogram itself (`range_below_zero`); MP5/LS2
800 Hz is `truncated` on both sides; MP2/LS2 800 Hz is `truncated` on the response in two seeds.

**Every value outside 5 %, as the receipt has it (24 of 2,858):**

- IEC-like, octaves, T30, every one longer: seed 0: MP1/LS1 500 Hz +5.26, MP1/LS1 4 kHz +5.79, MP1/LS2 500 Hz +6.14,
  MP2/LS1 500 Hz +6.94, MP3/LS1 500 Hz +8.86, MP3/LS2 4 kHz +5.63, MP5/LS2 2 kHz +6.41; seed 1: MP1/LS2 500 Hz +6.83,
  MP3/LS1 500 Hz +6.35, MP3/LS1 4 kHz +7.00, MP3/LS2 4 kHz +7.58, MP4/LS2 500 Hz +7.01, MP4/LS2 4 kHz +5.19, MP5/LS2
  500 Hz +6.55; seed 2: MP3/LS1 500 Hz +5.73, MP3/LS2 4 kHz +7.99, MP5/LS2 2 kHz +6.62 %.
- IEC-like, thirds, all at 400 Hz: seed 0: MP1/LS2 T30 +5.09, MP2/LS2 T30 +6.37, MP4/LS2 T30 +6.20; seed 2: MP1/LS2 T30
  +5.73, MP3/LS2 **EDT** +6.20, MP4/LS2 T30 +7.22 %.
- Steeper, thirds: seed 2: MP4/LS1 160 Hz **EDT −5.08 %**.

What they say: 22 of the 23 IEC-like misses are T30, every one longer, at bands whose lower neighbour decays more
slowly (250 Hz T30 ≈ 2.1 s against 500 Hz ≈ 1.7 s), and the steeper bank shows none of them. The likely cause is the
6th-order skirts letting the slower neighbour into the measurement. No test shows it yet (backlog 99). It is a
property of the measurement as much as of the response: a measured IR through the same filters would carry it too.

**MP4's early part** (the audit's question). Before the audit the bed reported four misses, all EDT, all negative, all
at MP4 (summed 500 Hz −5.37, LS1 100 Hz −5.74, 125 Hz −5.81, 200 Hz −5.64 %). They were measured with the module's own
estimator and included the sources summed. MP4 has the strongest direct sound of the five receivers: up to **35 %** of a
pair-band's energy lies in its onset and next two steps (MP1 18-20 %, MP2 21-24 %, MP3 17-23 %, MP5 15-18 %), and its
first 10 dB of decay take **0.135 s** (0.16-0.19 s elsewhere). An EDT read over so short a window, a third of it a 1 ms
burst, is the most sensitive to how the band filters spread that burst in time. Through the app's own estimator three
of the four are within 5 %, and the summed one is not graded. The one miss left at MP4 is LS1 160 Hz −5.08 % (steeper
bank, seed 2): the same receiver, source and sign. The synthesis was not tuned to it.

**Unit tests** (`cargo test -p simpa-core --lib auralize`, 17/17):

- Reconstruction ripple: 0.00 dB for both synthesis banks, octaves and thirds (by construction).
- A white echogram measures ±0.46 dB, against the estimator's 0.80 dB 3-sigma.
- Leakage: band totals ≤ 0.24 dB in octaves and ≤ 0.85 dB in thirds (bound 1 dB). Per band-resolution window
  ≤ 13 % and ≤ 16 % (bound 20 %). Per 1 ms step up to 94 %: the random fine structure, not bounded.
- The click: energy 1.000000 of the step's, centroid within 0.01 ms, 100.000 % in band.
- A synthetic decay through the IEC-like bank: EDT −2.28 to +1.27 %, T30 +0.04 to +2.60 %.
- Resampling: 44.1 → 48 kHz error −116 dB; 96 → 48 kHz alias −118 dB.

## WAVs to listen to

`B:\repos\I-Simpa_Night_Mode\.out\aural\listen\CR4-third-{MP1,MP4}-summed-{ir,speech-lv-hislastbow,tenorsax-vcsl-c3,harp-vcsl-c5}.wav`
(regenerated after the wrap fix); per receiver and per source: `.out\aural\bed\{octave,third}\*.wav`.

## Clips (`app/src-tauri/examples/clips`, 2.1 MB, provenance with sha256 beside each)

- `speech-lv-hislastbow`: LibriVox, public domain ("all our recordings are public domain").
- `tenorsax-vcsl-c3`, `harp-vcsl-c5`: VCSL, CC0 1.0 (its README quoted).
- Licence pages re-fetched and both quotes checked by the builder. Iowa MIS refused (no CC0/PD statement).
- **Dry recordings, close-miked: not anechoic.** The folder, the code and the window call them that ("so the room they
  were recorded in adds a little"); a user's own file is asked for as dry or anechoic (backlog 100). The word
  "anechoic" remains in the code only in four places: the negations ("not anechoic"), the user's file ("a dry or
  anechoic recording"), the CLI's `--anechoic` alias, and the provenance files' own "NOT anechoic" notes.

## Other checks

After the audit: `cargo fmt` clean; workspace `clippy -D warnings` clean; the touched tests pass (auralize unit
tests 17/17, cli_auralize 3/3, app's aural test); `npm run typecheck` clean; `npm test` 355/355. Before it:
`cargo test` simpa-core + simpa all ok; app 96/96. m11 command inventory re-pinned 49 → 50 (`run_auralize`), reason
in the script.

## Open

- Backlog 99: the 24 listed misses. The T30 excess through the IEC-like bank is not yet shown to come from its
  skirts, and MP4's short EDT is not yet traced.
- Backlog 100: truly anechoic clips.
- Not deleted (the no-delete guard, after 22:00): `C:\tmp\nm-target-aural` (cargo target and gate work folders) and the
  scratchpad clip copies, for after 07:00. Keep `.out\aural` (runs, bed, listen).
