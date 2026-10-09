// aweight.ts under `node --test` (parity M18 and M46).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { A_WEIGHTING_DB, aWeightDb, aWeightFormulaDb } from './aweight.ts';

// IEC 61672-1:2013, Table 3: the A-weighting at every nominal third-octave frequency from 10 Hz to
// 20 kHz, in dB, as the standard prints it (to 0.1 dB). Typed here from the standard, apart from
// the module's table, so that a slip in either is caught.
const IEC_61672_TABLE_3: readonly [number, number][] = [
  [10, -70.4], [12.5, -63.4], [16, -56.7], [20, -50.5], [25, -44.7], [31.5, -39.4], [40, -34.6],
  [50, -30.2], [63, -26.2], [80, -22.5], [100, -19.1], [125, -16.1], [160, -13.4], [200, -10.9],
  [250, -8.6], [315, -6.6], [400, -4.8], [500, -3.2], [630, -1.9], [800, -0.8], [1000, 0.0],
  [1250, 0.6], [1600, 1.0], [2000, 1.2], [2500, 1.3], [3150, 1.2], [4000, 1.0], [5000, 0.5],
  [6300, -0.1], [8000, -1.1], [10000, -2.5], [12500, -4.3], [16000, -6.6], [20000, -9.3],
];

/** The project's bands (crates/simpa-core/src/schema/bands.rs): third octaves 50 Hz to 20 kHz and
 * octaves 63 Hz to 16 kHz. */
const THIRDS = [50, 63, 80, 100, 125, 160, 200, 250, 315, 400, 500, 630, 800, 1000, 1250, 1600, 2000, 2500, 3150, 4000, 5000, 6300, 8000, 10000, 12500, 16000, 20000];
const OCTAVES = [63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];

test("M18: every band centre's A-weighting is IEC 61672-1 Table 3's, to 0.1 dB", () => {
  const table = new Map(IEC_61672_TABLE_3);
  for (const hz of [...THIRDS, ...OCTAVES]) {
    const want = table.get(hz);
    assert.ok(want !== undefined, `${hz} Hz is in the standard's table`);
    const got = aWeightDb(hz);
    assert.ok(got !== null && Math.abs(got - want) < 0.05, `${hz} Hz: ${got} dB vs the standard's ${want} dB`);
  }
  // And the module's table is the standard's, every row.
  assert.deepEqual(
    Object.entries(A_WEIGHTING_DB)
      .map(([f, a]) => [Number(f), a])
      .sort((x, y) => x[0] - y[0]),
    [...IEC_61672_TABLE_3],
  );
});

test("M18: the table is the standard's formula (Annex E) at the exact base-10 centres, rounded to 0.1 dB", () => {
  // The exact centre of the band k third-octaves from 1 kHz is 1000 * 10^(k/10) Hz (IEC 61260-1).
  IEC_61672_TABLE_3.forEach(([hz, a], i) => {
    const k = i - 20;
    const exact = 1000 * 10 ** (k / 10);
    assert.ok(Math.abs(exact / hz - 1) < 0.03, `${hz} Hz is band ${k}`);
    const f = aWeightFormulaDb(exact);
    assert.ok(Math.abs(f - a) < 0.05, `${hz} Hz: the formula gives ${f.toFixed(4)} dB, the table ${a} dB`);
  });
  assert.ok(Math.abs(aWeightFormulaDb(1000)) < 1e-12, '0 dB at 1 kHz');
});

test('M18: a frequency the standard does not list has no A-weighting here', () => {
  assert.equal(aWeightDb(1001), null);
  assert.equal(aWeightDb(25000), null);
});
