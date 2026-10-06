import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { Vec } from './geometry.ts';
import { gridStep, groundLayout } from './ground.ts';

test('the grid spacing gives at most 25 cells across the longest side', () => {
  assert.equal(gridStep(5), 0.5);
  assert.equal(gridStep(33.35), 2);
  assert.equal(gridStep(41.45), 2);
  assert.equal(gridStep(120), 5);
  assert.equal(gridStep(10_000), 20);
});

test('the plane sits just under the lowest point, centred on the footprint', () => {
  const box = { min: [-10, 0, -1] as Vec, max: [30, 20, 14] as Vec };
  const g = groundLayout(box);
  assert.ok(g.z < -1 && g.z > -1.1, `${g.z}`);
  assert.deepEqual(g.centre, [10, 10]);
  assert.equal(g.half, 64);
  assert.equal(g.step, 2);
});
