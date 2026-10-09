import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { Vec } from './geometry.ts';
import { gridStep, groundLayout, wallGridSegments } from './ground.ts';

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

test('G46: the XZ grid stands in y = the lowest y, the YZ grid in x = the lowest x, on the floor grid\'s spacing', () => {
  const box = { min: [1.2, -3, 0] as Vec, max: [7.9, 4.5, 3.1] as Vec };
  const step = groundLayout(box).step;
  assert.equal(step, 0.5);
  const xz = wallGridSegments(box, 'xz');
  const yz = wallGridSegments(box, 'yz');
  assert.equal(xz.length % 6, 0);
  for (let i = 1; i < xz.length; i += 3) assert.equal(xz[i], Math.fround(-3));
  for (let i = 0; i < yz.length; i += 3) assert.equal(yz[i], Math.fround(1.2));
  // Uprights at whole steps across the room's span: x from 1.0 to 8.0, 15 lines.
  const ups = [];
  for (let s = 0; s < xz.length; s += 6) if (xz[s] === xz[s + 3]) ups.push(xz[s]);
  assert.equal(ups.length, 15);
  assert.equal(ups[0], 1);
  assert.equal(ups[ups.length - 1], 8);
  // Everything between the ground's height and the first step at or above the top (3.5 m).
  const zs = [...xz].filter((_, i) => i % 3 === 2);
  assert.ok(Math.min(...zs) < 0 && Math.min(...zs) > -0.1);
  assert.equal(Math.max(...zs), 3.5);
});
