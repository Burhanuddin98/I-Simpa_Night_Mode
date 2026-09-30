// node --test suite for plant-loss.ts (M11 review F1): the planted table is the plan, every band
// still sums to its total, nothing else in run.json moves, and a run the plan was not worked for
// is refused rather than planted.
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { PLANTED, PLANTED_BANDS, plantLoss } from './plant-loss.ts';
import { bandLosses, type Manifest, worstLoss } from './runs.ts';

/** The box run's run.json as the core wrote it at 23:04 today (gate work folder
 * 20260929-230358), trimmed to the fields plantLoss reads and a few it must leave alone. */
function boxRun(): Record<string, unknown> {
  const band = (freq_hz: number, atmosphere: number, materials: number, remaining: number) => ({
    freq_hz,
    absorbed_by_atmosphere: atmosphere,
    absorbed_by_materials: materials,
    absorbed_by_fittings: 0,
    lost_by_infinite_loops: 0,
    lost_by_meshing_problems: 0,
    remaining,
    total: 150_000,
  });
  return {
    manifest_version: 1,
    solver: 'spps',
    exe: { path: 'C:\\w\\solvers\\spps.exe', sha256: '1d9900db' },
    outcome: { exit_code: 0, cancelled: false, elapsed_ms: 1511.0962 },
    lines: { progress: 9999, info: 2, ok: 1, warn: 0, fail: 0, unclassified: 0 },
    particles: {
      bands: [
        band(125, 511, 149_489, 0),
        band(250, 1600, 148_399, 1),
        band(500, 3282, 146_718, 0),
        band(1000, 5650, 144_348, 2),
        band(2000, 11_273, 138_727, 0),
        band(4000, 29_311, 120_689, 0),
      ],
    },
    loss_limit: 0.01,
    verdict: { status: 'OK', reasons: [], warnings: [] },
  };
}

type B = { freq_hz: number; absorbed_by_atmosphere: number; absorbed_by_materials: number; absorbed_by_fittings: number; lost_by_infinite_loops: number; lost_by_meshing_problems: number; remaining: number; total: number };
const bandsOf = (m: unknown) => (m as { particles: { bands: B[] } }).particles.bands;

test('the planted table is the plan, and each band still sums to its total', () => {
  const before = boxRun();
  const after = JSON.parse(plantLoss(JSON.stringify(before))) as Manifest;
  const w = worstLoss(after);
  assert.equal(w?.pct, '0.82');
  assert.equal(w?.freq_hz, 500);
  assert.equal(w?.pct, PLANTED.worst_pct);
  // The worst band is not the first: a display that took the first band would show 0.10.
  assert.notEqual(bandLosses(after)[0].freq_hz, PLANTED.worst_band_hz);
  assert.deepEqual(
    bandLosses(after).map((b) => [b.freq_hz, String(b.lost), b.pct]),
    [
      [125, '150', '0.10'],
      [250, '0', '0.00'],
      [500, '1234', '0.82'],
      [1000, '7', '0.00'],
      [2000, '1000', '0.67'],
      [4000, '0', '0.00'],
    ],
  );
  for (const b of bandsOf(after)) {
    const sum = b.absorbed_by_atmosphere + b.absorbed_by_materials + b.absorbed_by_fittings + b.lost_by_infinite_loops + b.lost_by_meshing_problems + b.remaining;
    assert.equal(sum, b.total, `${b.freq_hz} Hz`);
  }
  // Only the moved particles changed.
  for (const [i, b] of bandsOf(after).entries()) {
    const o = bandsOf(before)[i];
    const p = PLANTED_BANDS.find((x) => x.freq_hz === b.freq_hz);
    const moved = p ? p.lost_by_infinite_loops + p.lost_by_meshing_problems : 0;
    assert.equal(b.absorbed_by_materials, o.absorbed_by_materials - moved);
    assert.equal(b.absorbed_by_atmosphere, o.absorbed_by_atmosphere);
  }
  const strip = (m: Record<string, unknown>) => ({ ...m, particles: null });
  assert.deepEqual(strip(JSON.parse(plantLoss(JSON.stringify(before)))), strip(before));
});

test('a run the plan was not worked for is refused, not planted', () => {
  const variants: [string, (m: Record<string, unknown>) => void][] = [
    ['a TCR run', (m) => (m.solver = 'tcr')],
    ['a failed run', (m) => (m.verdict = { status: 'FAIL', reasons: [], warnings: [] })],
    ['another limit', (m) => (m.loss_limit = 0.02)],
    ['no statistics', (m) => (m.particles = null)],
    ['a run that lost particles itself', (m) => (bandsOf(m)[3].lost_by_meshing_problems = 1)],
    ['another particle count', (m) => (bandsOf(m)[0].total = 100_000)],
    ['too few absorbed to move', (m) => (bandsOf(m)[2].absorbed_by_materials = 1000)],
  ];
  for (const [name, change] of variants) {
    const m = boxRun();
    change(m);
    assert.throws(() => plantLoss(JSON.stringify(m)), /plantLoss/, name);
  }
});
