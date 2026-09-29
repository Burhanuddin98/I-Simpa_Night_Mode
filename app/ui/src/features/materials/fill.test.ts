import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { F64 } from './bands.ts';
import { boundsOf, clampCell, inRect, planFill, rectOf } from './fill.ts';

const grid = (): F64[][] => [
  [0.1, 0.2, 0.3, 0.4],
  [0.5, 0.6, 0.7, 0.8],
  [0.9, 0.9, 0.1, 0.9],
];

test('one focused cell fills its whole row, both ways, leaving equal cells out', () => {
  const out = planFill(grid(), rectOf({ row: 0, col: 2 }, { row: 0, col: 2 }));
  assert.deepEqual(out, [
    { row: 0, col: 0, value: 0.3 },
    { row: 0, col: 1, value: 0.3 },
    { row: 0, col: 3, value: 0.3 },
  ]);
  assert.deepEqual(planFill(grid(), rectOf({ row: 2, col: 0 }, { row: 2, col: 0 })), [{ row: 2, col: 2, value: 0.9 }]);
});

test('a column of cells fills each of its rows across', () => {
  const out = planFill(grid(), rectOf({ row: 0, col: 1 }, { row: 1, col: 1 }));
  assert.equal(out.length, 6);
  assert.ok(out.every((c) => c.value === (c.row === 0 ? 0.2 : 0.6)));
});

test('a wider selection fills right from its leftmost column, inside the selection', () => {
  const out = planFill(grid(), rectOf({ row: 1, col: 1 }, { row: 0, col: 2 }));
  assert.deepEqual(out, [
    { row: 0, col: 2, value: 0.2 },
    { row: 1, col: 2, value: 0.6 },
  ]);
});

test('a fill that changes nothing plans nothing; the name column fills from the first band', () => {
  assert.deepEqual(planFill([[0.5, 0.5, 0.5]], rectOf({ row: 0, col: 1 }, { row: 0, col: 1 })), []);
  const named = planFill(grid(), rectOf({ row: 0, col: -1 }, { row: 0, col: -1 }));
  assert.deepEqual(named, [
    { row: 0, col: 1, value: 0.1 },
    { row: 0, col: 2, value: 0.1 },
    { row: 0, col: 3, value: 0.1 },
  ]);
  // -0 and 0 are different values to the core.
  assert.equal(planFill([[-0, 0]], rectOf({ row: 0, col: 0 }, { row: 0, col: 0 })).length, 1);
});

test('rectangles', () => {
  const r = rectOf({ row: 3, col: 2 }, { row: 1, col: -1 });
  assert.deepEqual(r, { r0: 1, r1: 3, c0: -1, c1: 2 });
  assert.ok(inRect(r, 2, 0));
  assert.ok(!inRect(r, 0, 0));
  assert.deepEqual(clampCell({ row: 9, col: 9 }, 3, 4), { row: 2, col: 3 });
  assert.deepEqual(clampCell({ row: -1, col: -5 }, 3, 4), { row: 0, col: -1 });
  assert.deepEqual(clampCell({ row: 2, col: 2 }, 0, 0), { row: 0, col: -1 });
  assert.deepEqual(boundsOf([{ row: 4, col: 0 }, { row: 2, col: 5 }]), { r0: 2, r1: 4, c0: 0, c1: 5 });
  assert.equal(boundsOf([]), null);
});
