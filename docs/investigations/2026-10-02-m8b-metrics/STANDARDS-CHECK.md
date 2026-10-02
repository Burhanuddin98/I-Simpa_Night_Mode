# M8b metrics plan, checked against the standards

2026-10-02. The plan's definitions were written from memory, marked "as commonly stated". Burhan supplied the texts
this morning, and each definition was checked against them by a fresh-context reader. The texts are copyrighted and
not in the repo. This file cites clause, equation and page (printed page numbers) and paraphrases. The numeric
tables below are data that upstream's GPL code already carries.

- **ISO 3382-1:2009**, first edition, confirmed 2015.
- **IEC 60268-16:2011**, edition 4, read as BS EN 60268-16:2011, which is identical to it. This is the edition
  upstream cites. Edition 5 (2020) has not been read, and the product's STI targets edition 4.
- **IEC 61672-1** is not in hand. Only the dB(A) table depends on it.

## Verdicts

| Item | Verdict | Receipt | Consequence |
|---|---|---|---|
| T20, T30 | MATCH | ISO cl. 6, p. 8; Eq. 1, p. 7 | 0 dB is the total of the Schroeder integral, not the direct peak. T = 60/slope (extrapolated) |
| EDT | MATCH with a named deviation | ISO A.2.2, p. 15; A.2.1 | The standard fits from 0 dB to -10 dB. v2.1 fits from the ball's back, `t_arrival + R/c`, and refuses `receiver_too_large` where that matters. This is a deliberate deviation from A.2.2, not the ISO fit. `frozen2/method.py`'s docstring (line 15) calls it "ISO 0/-10 dB OLS"; the file is frozen, so the deviation is recorded here and in `PORT.md` |
| C50, C80, D50 | MATCH | ISO A.2.3, Eqs. A.10-A.12, p. 15; A.3.4, pp. 18-19 | Time zero is the direct-sound start. C50 = 10 lg(D50/(1-D50)). The standard has no rule for an arrival inside a bin, a finite receiver or a finite run's tail. Those rules are the plan's own |
| Ts | MATCH | ISO A.2.3, Eq. A.13, p. 16 | Time zero is the direct sound, so upstream's absolute bin label is wrong by the standard |
| G | MATCH with a constant to watch | ISO A.2.1, Eqs. A.1-A.9, pp. 13-14 | The standard's `G = Lp - Lw + 31 dB` assumes ρc ≈ 400. The plan's exact form gives 30.85 dB at SPPS's ρc = 413.25, which is 0.15 dB off the "31" form. That is inside the 1 dB limen and outside the product's 0.1 dB limit. G is tested against the exact form, and the "31" form is reported with its ρc |
| SPL | Not defined by the standard | none | `SPL = 10 lg(Σ B_k / p0²)` stays the plan's own definition |
| Limens | MATCH where listed | ISO Table A.1, p. 12 | Listed: G 1 dB, EDT 5 %, C80 1 dB, D50 0.05, Ts 10 ms. **Not listed:** C50 (takes C80's 1 dB), T20 and T30 (take EDT's 5 %), SPL (takes G's 1 dB). Those carry-overs are the plan's choice |
| Uncertainty | Scatter formula only | ISO cl. 7.1, Eqs. 4-5, p. 9; cl. 7.2 | No clause requires a shown range. The T20/T30 scatter formula may be cited for a tolerance |
| STI method | MATCH | IEC cl. 6.1, p. 27; A.5.3-A.5.5, pp. 45-46 | Schroeder MTF, 14 modulation frequencies 0.63-12.5 Hz, SNR_eff ±15 dB, m > 1 truncated |
| STI duration | Missing from plan | IEC cl. 6.2b, 8.3a, pp. 27, 34 | The IR must be at least 1.6 s and at least T/2. Added to the run-length checks |
| STI masking | **Plan's recollection wrong** | IEC A.3.2, Table A.1, p. 40 | Band k is masked by band k-1's level. The 125 Hz band is not masked. Upstream's `:693-703` uses band k's own level, so that is a real upstream bug |
| STI threshold, weights, spectrum | Pinned | IEC Tables A.2-A.4, pp. 41-42 | See below |
| STI limen 0.03 | **Not a limen** | IEC Annex E, p. 52; Annex F, p. 53 | Edition 4 states no JND. 0.02 (full STI) and 0.03 (STIPA) are repeatability figures, and the Annex F rating bands are 0.04 wide. The plan's 0.03 becomes its own stated tolerance, citing these |
| STI gender | **Settled by the standard** | IEC A.3.4, A.3.5 note 1, Annex B | Male is the default for assessing channels. Both are computed and male is shown (plan decision 6) |
| STI speech level | **Settled** | IEC J.3 | 60 dB(A) at 1 m on axis; 70 dB(A) for raised effort |
| Classical vs Barron for C/D | Not in the standard | ISO, whole text | Stays Burhan's call |

## STI tables (IEC 60268-16:2011)

Octaves 125, 250, 500, 1k, 2k, 4k, 8k Hz.

| | 125 | 250 | 500 | 1k | 2k | 4k | 8k |
|---|---|---|---|---|---|---|---|
| ART, dB SPL (Table A.2) | 46 | 27 | 12 | 6.5 | 7.5 | 8 | 12 |
| Male α (Table A.3) | 0.085 | 0.127 | 0.230 | 0.233 | 0.309 | 0.224 | 0.173 |
| Male β, k to k+1 | 0.085 | 0.078 | 0.065 | 0.011 | 0.047 | 0.095 | — |
| Female α | — | 0.117 | 0.223 | 0.216 | 0.328 | 0.250 | 0.194 |
| Female β, k to k+1 | — | 0.099 | 0.066 | 0.062 | 0.025 | 0.076 | — |
| Male spectrum, dB re A-weighted level (Table A.4) | +2.9 | +2.9 | -0.8 | -6.8 | -12.8 | -18.8 | -24.8 |
| Female spectrum | — | +5.3 | -1.9 | -9.1 | -15.8 | -16.7 | -18.0 |

- Σα - Σβ = 1.000 for both sexes: male 1.381 - 0.381, female 1.328 - 0.328.
- Female has no 125 Hz band.
- Male STI can exceed 1 in a corner case and is truncated at 1.0 (note under Table A.3).
- Masking (Table A.1). Let L be band k-1's total level, including noise. Then
  `amdB = 0.5L - 65` for L < 63; `1.8L - 146.9` for 63 ≤ L < 67; `0.5L - 59.8` for 67 ≤ L < 100; `-10` for L ≥ 100.
  `I_am,k = I_(k-1) · 10^(amdB/10)`.
- Correction: `m' = m · I_k / (I_k + I_am,k + I_rt,k)`, with ambient noise added to the denominator.
- Cl. 8.3 sets the reporting requirement: a predicted-IR STI says so and names the weighting.

## Not read

- IEC 60268-16 Annexes M (noise), J.2, B and C (STIPA/STITEL details).
- IEC 61672-1.
