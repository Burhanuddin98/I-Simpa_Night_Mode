// node --test suite for stats.ts: gate (b)'s nearest-rank percentiles.
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { distribution, nearestRank } from './stats.ts';

test('p99 of 200 values is the 198th smallest; p95 of 60 the 57th', () => {
  const xs = Array.from({ length: 200 }, (_, i) => 200 - i); // 200 .. 1, unsorted
  assert.equal(nearestRank(xs, 0.99), 198);
  assert.equal(nearestRank(xs, 0.5), 100);
  assert.equal(nearestRank(xs, 1), 200);
  const ys = Array.from({ length: 60 }, (_, i) => i + 1);
  assert.equal(nearestRank(ys, 0.95), 57);
  assert.equal(nearestRank([7], 0.99), 7);
});

test('two slow values of 200 pass p99; three do not', () => {
  const two = [...Array(198).fill(5), 150, 150];
  assert.equal(nearestRank(two, 0.99), 5);
  const three = [...Array(197).fill(5), 150, 150, 150];
  assert.equal(nearestRank(three, 0.99), 150);
});

test('bad input is refused, and the summary prints every figure', () => {
  assert.throws(() => nearestRank([], 0.5));
  assert.throws(() => nearestRank([1], 0));
  assert.equal(distribution([1, 2, 3, 4]), 'n 4, min 1.00, median 2.00, p95 4.00, p99 4.00, max 4.00 ms');
});
