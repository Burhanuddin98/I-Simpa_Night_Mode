// emission.ts under `node --test` (C1).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { bandLevels, bandNumber, energySum, shapeFor, spectrumKey, spectrumOptions, withBandLevel, withDirectivity } from './emission.ts';

const OCT = [125, 250, 500, 1000, 2000, 4000, 8000];
const near = (a: number, b: number, eps = 1e-9) => assert.ok(Math.abs(a - b) < eps, `${a} vs ${b}`);

test('band numbers are the exact third-octave steps', () => {
  assert.deepEqual([50, 63, 800, 1000, 1250, 3150, 20000].map(bandNumber), [-13, -12, -1, 0, 1, 5, 13]);
});

test('band levels sum to the global level; pink is flat, white rises 3 dB per octave', () => {
  const pink = bandLevels({ global_db: 85, shape: { kind: 'pink' } }, OCT)!;
  near(energySum(pink), 85);
  pink.forEach((l) => near(l, 85 - 10 * Math.log10(7)));
  const white = bandLevels({ global_db: 85, shape: { kind: 'white' } }, OCT)!;
  near(energySum(white), 85);
  near(white[1] - white[0], 3);
  assert.equal(bandLevels({ global_db: 85, shape: { kind: 'custom', relative_db: [0, 0] } }, OCT), null, 'a shape of another length');
});

test('a level typed in one band keeps the others and becomes the custom shape', () => {
  const before = bandLevels({ global_db: 85, shape: { kind: 'pink' } }, OCT)!;
  const p = withBandLevel({ global_db: 85, shape: { kind: 'pink' } }, OCT, 3, 90)!;
  assert.equal(p.shape.kind, 'custom');
  const after = bandLevels(p, OCT)!;
  near(after[3], 90, 1e-9);
  for (const i of [0, 1, 2, 4, 5, 6]) near(after[i], before[i], 1e-9);
  near(Number(p.global_db), energySum(after));
  assert.equal(withBandLevel({ global_db: 85, shape: { kind: 'pink' } }, OCT, 7, 90), null);
});

test('the spectrum select names a library shape, and falls back to typed', () => {
  const lib = [
    { reference_id: 1, name: 'Pink noise', shape: { kind: 'pink' as const } },
    { reference_id: 0, name: 'White noise', shape: { kind: 'white' as const } },
    { reference_id: 2, name: 'ES_VL', shape: { kind: 'custom' as const, relative_db: [1, 2, 3, 4, 5, 6, 7] } },
  ];
  assert.equal(spectrumKey({ kind: 'pink' }, lib), 'pink');
  assert.equal(spectrumKey({ kind: 'custom', relative_db: [1, 2, 3, 4, 5, 6, 7] }, lib), 'ref:2');
  assert.equal(spectrumKey({ kind: 'custom', relative_db: [1, 2, 3, 4, 5, 6, 8] }, lib), 'custom');
  assert.deepEqual(
    spectrumOptions(lib, 'pink').map((o) => o.value),
    ['pink', 'white', 'ref:2'],
  );
  assert.deepEqual(spectrumOptions(lib, 'custom').map((o) => o.value).at(-1), 'custom');
  assert.deepEqual(shapeFor('ref:2', lib), lib[2].shape);
  assert.deepEqual(shapeFor('white', lib), { kind: 'white' });
  assert.equal(shapeFor('custom', lib), null);
});

test('a unidirectional source keeps its direction; the planes and omni carry none', () => {
  assert.deepEqual(withDirectivity({ kind: 'omni' }, 'unidirectional'), { kind: 'unidirectional', direction: [1, 0, 0] });
  assert.deepEqual(withDirectivity({ kind: 'unidirectional', direction: [0, 1, 0] }, 'unidirectional'), { kind: 'unidirectional', direction: [0, 1, 0] });
  assert.deepEqual(withDirectivity({ kind: 'unidirectional', direction: [0, 1, 0] }, 'plane_xy'), { kind: 'plane_xy' });
  assert.equal(withDirectivity({ kind: 'omni' }, 'balloon'), null, 'a balloon is not offered');
});
