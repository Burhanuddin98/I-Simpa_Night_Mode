import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { arrowLength, arrowSegments } from './arrows.ts';
import type { Vec } from './geometry.ts';

test('M30: the arrow runs from the source along its direction, normalised, with four barbs at the tip', () => {
  const s = arrowSegments([1, 2, 3], [0, 0, -4], 2);
  assert.equal(s.length, 5 * 6);
  assert.deepEqual(s.slice(0, 6), [1, 2, 3, 1, 2, 1]);
  for (let k = 1; k < 5; k++) {
    const seg = s.slice(6 * k, 6 * k + 6);
    assert.deepEqual(seg.slice(0, 3), [1, 2, 1], 'each barb starts at the tip');
    assert.ok(Math.abs(seg[5] - 1.5) < 1e-12, 'a quarter of the length back');
    assert.ok(Math.abs(Math.hypot(seg[3] - 1, seg[4] - 2) - 0.2) < 1e-12, 'a tenth out');
  }
});

test('M30: no arrow for a zero or non-finite direction', () => {
  assert.deepEqual(arrowSegments([0, 0, 0], [0, 0, 0], 1), []);
  assert.deepEqual(arrowSegments([0, 0, 0], [Number.NaN, 0, 0], 1), []);
});

test('M30: the arrow is 8 % of the longest side, from 0.3 m to 3 m', () => {
  const box = (x: number) => ({ min: [0, 0, 0] as Vec, max: [x, 1, 1] as Vec });
  assert.equal(arrowLength(box(10)), 0.8);
  assert.equal(arrowLength(box(2)), 0.3);
  assert.equal(arrowLength(box(100)), 3);
  assert.equal(arrowLength(null), 0.5);
});
