// arc.ts under `node --test` (Burhan's 10-05 UI list, item 6: camera moves that arc).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { arcPose, easeInOut, slerpDir, type Pose, type V3 } from './arc.ts';

const len = (a: V3) => Math.hypot(a[0], a[1], a[2]);
const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const near = (a: V3, b: V3, tol = 1e-9) => assert.ok(len(sub(a, b)) <= tol, `${a} is not ${b}`);

test('the ease starts and ends still and exactly, and only rises', () => {
  assert.equal(easeInOut(0), 0);
  assert.equal(easeInOut(1), 1);
  assert.equal(easeInOut(0.5), 0.5);
  let last = 0;
  for (let i = 1; i <= 100; i++) {
    const e = easeInOut(i / 100);
    assert.ok(e >= last);
    last = e;
  }
});

test('both ends of a move are the poses given, exactly', () => {
  const from: Pose = { position: [10, -12, 8], target: [0, 0, 1.5] };
  const to: Pose = { position: [-3, 4, 20], target: [2, 1, 0] };
  assert.deepEqual(arcPose(from, to, 0), from);
  assert.deepEqual(arcPose(from, to, 1), to);
});

test('a turn about a fixed target keeps the distance: the camera orbits, it does not cut the chord', () => {
  const from: Pose = { position: [10, 0, 0], target: [0, 0, 0] };
  const to: Pose = { position: [0, 10, 0], target: [0, 0, 0] };
  for (const f of [0.1, 0.25, 0.5, 0.75, 0.9]) {
    const p = arcPose(from, to, f);
    assert.ok(Math.abs(len(p.position) - 10) < 1e-9, `at ${f} the camera is ${len(p.position)} m from the target, not 10`);
  }
  // The straight line's midpoint is 7.07 m from the target; the arc's is 10.
  near(arcPose(from, to, 0.5).position, [10 / Math.SQRT2, 10 / Math.SQRT2, 0]);
});

test('a half turn goes round the room about the vertical, never through the target', () => {
  const from: Pose = { position: [10, 0, 2], target: [0, 0, 2] };
  const to: Pose = { position: [-10, 0, 2], target: [0, 0, 2] };
  for (const f of [0.2, 0.5, 0.8]) {
    const p = arcPose(from, to, f);
    assert.ok(Math.abs(len(sub(p.position, p.target)) - 10) < 1e-9);
    assert.ok(Math.abs(p.position[2] - 2) < 1e-9, 'stays level: it turns about the vertical');
  }
  const d = slerpDir([0, 0, 1], [0, 0, -1], 0.5);
  assert.ok(Math.abs(len(d) - 1) < 1e-12 && Math.abs(d[2]) < 1e-9, 'a vertical half turn passes the horizon');
});

test('a zoom changes the distance evenly (geometrically) and a long move pulls back a little mid-flight', () => {
  const from: Pose = { position: [0, -4, 0], target: [0, 0, 0] };
  const to: Pose = { position: [0, -16, 0], target: [0, 0, 0] };
  assert.ok(Math.abs(len(arcPose(from, to, 0.5).position) - 8) < 1e-9, 'halfway between 4 m and 16 m is 8 m');
  const a: Pose = { position: [0, -5, 0], target: [0, 0, 0] };
  const b: Pose = { position: [20, -5, 0], target: [20, 0, 0] };
  const mid = arcPose(a, b, 0.5);
  const d = len(sub(mid.position, mid.target));
  assert.ok(d > 5 && d <= 5 * 1.3 + 1e-9, `pulled back to ${d} m, not past 30 %`);
  near(mid.target, [10, 0, 0]);
});
