// Parity M18 and M46: the A-weighting a user's dB(A) is typed and read with, in the source spectrum
// editor and the library entry editor. A pure module: no store, no backend, only erasable
// TypeScript, tested by aweight.test.ts under `node --test`.
//
// The values are IEC 61672-1:2013's, Table 3 (the A-weighting at the nominal third-octave
// frequencies, rounded to 0.1 dB), the same table the results' dB(A) uses
// (`crates/simpa-core/src/params/level.rs`, `A_WEIGHTS_DB`) and the one upstream's GUI applies band
// by band (`e_data_row_bandefreq.h:128-133`: dB(A) = dB + weighting). The test holds every entry
// to the standard's formula (Annex E, equation E.6) at the exact base-10 centre frequency.

/** IEC 61672-1:2013 Table 3, A-weighting in dB, by nominal frequency in Hz (10 Hz to 20 kHz). */
export const A_WEIGHTING_DB: Readonly<Record<number, number>> = {
  10: -70.4,
  12.5: -63.4,
  16: -56.7,
  20: -50.5,
  25: -44.7,
  31.5: -39.4,
  40: -34.6,
  50: -30.2,
  63: -26.2,
  80: -22.5,
  100: -19.1,
  125: -16.1,
  160: -13.4,
  200: -10.9,
  250: -8.6,
  315: -6.6,
  400: -4.8,
  500: -3.2,
  630: -1.9,
  800: -0.8,
  1000: 0.0,
  1250: 0.6,
  1600: 1.0,
  2000: 1.2,
  2500: 1.3,
  3150: 1.2,
  4000: 1.0,
  5000: 0.5,
  6300: -0.1,
  8000: -1.1,
  10000: -2.5,
  12500: -4.3,
  16000: -6.6,
  20000: -9.3,
};

/** The A-weighting of the band at nominal frequency `hz`, in dB; null for a frequency the table
 * does not hold (the project's bands, 50 Hz to 20 kHz, all are in it). */
export function aWeightDb(hz: number): number | null {
  return A_WEIGHTING_DB[hz] ?? null;
}

/** IEC 61672-1 Annex E, equation E.6, without its 1 kHz normalisation: the pole frequencies of
 * equations E.1 to E.4 to the precision Annex E gives them. */
const POLES_HZ = [20.598997, 107.65265, 737.86223, 12194.217] as const;
function aRaw(f: number): number {
  const [f1, f2, f3, f4] = POLES_HZ;
  const f2s = f * f;
  return 20 * Math.log10((f4 * f4 * f2s * f2s) / ((f2s + f1 * f1) * Math.sqrt(f2s + f2 * f2) * Math.sqrt(f2s + f3 * f3) * (f2s + f4 * f4)));
}

/** The A-weighting by the standard's formula at frequency `f` Hz, normalised to 0 dB at 1 kHz
 * (Annex E's A1000, -2.000 dB to the 0.001 dB it is stated to). For the test of the table. */
export function aWeightFormulaDb(f: number): number {
  return aRaw(f) - aRaw(1000);
}
