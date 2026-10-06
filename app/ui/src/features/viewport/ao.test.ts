import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { AO_STRENGTH, aoReach, bakeAo, hemisphere, type HitTest } from './ao.ts';
import type { Vec } from './geometry.ts';

/** A brute-force, two-sided hit test over `tris` (9 numbers a triangle), Moller-Trumbore. */
function hitsOf(tris: number[]): HitTest {
  return (o, d, far) => {
    for (let i = 0; i < tris.length; i += 9) {
      const a: Vec = [tris[i], tris[i + 1], tris[i + 2]];
      const e1: Vec = [tris[i + 3] - a[0], tris[i + 4] - a[1], tris[i + 5] - a[2]];
      const e2: Vec = [tris[i + 6] - a[0], tris[i + 7] - a[1], tris[i + 8] - a[2]];
      const p: Vec = [d[1] * e2[2] - d[2] * e2[1], d[2] * e2[0] - d[0] * e2[2], d[0] * e2[1] - d[1] * e2[0]];
      const det = e1[0] * p[0] + e1[1] * p[1] + e1[2] * p[2];
      if (Math.abs(det) < 1e-12) continue;
      const s: Vec = [o[0] - a[0], o[1] - a[1], o[2] - a[2]];
      const u = (s[0] * p[0] + s[1] * p[1] + s[2] * p[2]) / det;
      if (u < 0 || u > 1) continue;
      const q: Vec = [s[1] * e1[2] - s[2] * e1[1], s[2] * e1[0] - s[0] * e1[2], s[0] * e1[1] - s[1] * e1[0]];
      const v = (d[0] * q[0] + d[1] * q[1] + d[2] * q[2]) / det;
      if (v < 0 || u + v > 1) continue;
      const t = (e2[0] * q[0] + e2[1] * q[1] + e2[2] * q[2]) / det;
      if (t > 1e-9 && t <= far) return true;
    }
    return false;
  };
}

// A floor triangle whose stored normal points down, out of the room (the project's convention).
const FLOOR = [0, 0, 0, 0, 1, 0, 1, 0, 0];
const ceilingAt = (z: number) => [-5, -5, z, 5, -5, z, 5, 5, z, -5, -5, z, 5, 5, z, -5, 5, z];

test('a floor in the open stays at full brightness', () => {
  assert.deepEqual([...bakeAo(FLOOR, [0, 1, 2], hitsOf(FLOOR), 1)], [1, 1, 1]);
});

test('the rays go into the room: a ceiling just above darkens the floor, one beyond reach does not', () => {
  const low = bakeAo(FLOOR, [0, 1, 2], hitsOf([...FLOOR, ...ceilingAt(0.2)]), 1);
  for (const v of low) assert.ok(v < 1 - 0.8 * AO_STRENGTH, `${v}`);
  const high = bakeAo(FLOOR, [0, 1, 2], hitsOf([...FLOOR, ...ceilingAt(5)]), 1);
  assert.deepEqual([...high], [1, 1, 1]);
  // Below the floor (outside the room) nothing counts.
  assert.deepEqual([...bakeAo(FLOOR, [0, 1, 2], hitsOf([...FLOOR, ...ceilingAt(-0.2)]), 1)], [1, 1, 1]);
});

test('a corner where a wall meets the floor is darker than the open floor', () => {
  // The wall x = 0, its normal out of the room (-x).
  const wall = [0, -5, 0, 0, 5, 5, 0, 5, 0, 0, -5, 0, 0, -5, 5, 0, 5, 5];
  const ao = bakeAo(FLOOR, [0, 1, 2], hitsOf([...FLOOR, ...wall]), 1);
  // Vertices 0 and 1 sit on the wall's foot; vertex 2 is 1 m out, at the reach.
  assert.ok(ao[0] < 0.85 && ao[1] < 0.85, `${ao[0]} ${ao[1]}`);
  assert.ok(ao[2] > ao[0], `${ao[2]} vs ${ao[0]}`);
});

test('the fan is fixed, unit length and in the upper hemisphere', () => {
  assert.deepEqual(hemisphere(), hemisphere());
  for (const d of hemisphere()) {
    assert.ok(Math.abs(Math.hypot(...d) - 1) < 1e-12);
    assert.ok(d[2] > 0);
  }
});

test('the reach is a twelfth of the longest side, between 0.5 and 4 m', () => {
  const box = (x: number): { min: Vec; max: Vec } => ({ min: [0, 0, 0], max: [x, 1, 1] });
  assert.ok(Math.abs(aoReach(box(33.35)) - 33.35 / 12) < 1e-9);
  assert.equal(aoReach(box(4)), 0.5);
  assert.equal(aoReach(box(200)), 4);
});

