import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { faceNormalOf } from './floodfill.ts';
import {
  faceBounds,
  faceCentroid,
  fitOrtho,
  formatMetres,
  gizmoAxes,
  isFloorLike,
  placeOffFace,
  placementPoint,
  planDimensions,
  rayOnFacePlane,
  type Vec,
} from './geometry.ts';

// tests/fixtures/ui/teaching_room.simpa: a 10 x 6 x 3 m box, outward faces.
const ROOM = new Float64Array([0, 0, 0, 10, 0, 0, 10, 6, 0, 0, 6, 0, 0, 0, 3, 10, 0, 3, 10, 6, 3, 0, 6, 3]);
const ROOM_FACES = new Uint32Array([
  0, 2, 1, 0, 3, 2, // Floor
  4, 5, 6, 4, 6, 7, // Ceiling
  3, 7, 6, 3, 6, 2, // Left wall
  0, 1, 5, 0, 5, 4, // Right wall
  0, 4, 7, 0, 7, 3, // Front wall
  1, 2, 6, 1, 6, 5, // Rear wall
]);

test('only the floor of the room is floor-like; its outward normal points down', () => {
  const floorLike = Array.from({ length: 12 }, (_, f) => isFloorLike(faceNormalOf(ROOM, ROOM_FACES, f)));
  assert.deepEqual(floorLike, [true, true, false, false, false, false, false, false, false, false, false, false]);
});

test('a floor tilted 44 degrees is floor-like, 46 degrees is not', () => {
  const tilt = (deg: number): Vec => {
    const t = (deg * Math.PI) / 180;
    return [Math.sin(t), 0, -Math.cos(t)];
  };
  assert.equal(isFloorLike(tilt(44)), true);
  assert.equal(isFloorLike(tilt(46)), false);
  assert.equal(isFloorLike([0, 0, 1]), false, 'a ceiling seen from inside faces down');
});

test('a placement on the level floor sits exactly 1.2 m (receiver) or 1.5 m (source) above it', () => {
  // A ray from an oblique camera above the room onto the floor.
  const origin: Vec = [13.7, -4.1, 9.3];
  const target: Vec = [6.123456, 2.5, 0];
  const d = target.map((x, k) => x - origin[k]);
  const len = Math.hypot(...d);
  const dir = d.map((x) => x / len) as Vec;
  const hit = rayOnFacePlane(ROOM, ROOM_FACES, 0, origin, dir);
  assert.ok(hit);
  assert.equal(hit[2], 0, 'the level floor gives its own z exactly');
  assert.ok(Math.abs(hit[0] - 6.123456) < 1e-9 && Math.abs(hit[1] - 2.5) < 1e-9);
  assert.deepEqual(placementPoint(hit, 1.2), [6.123, 2.5, 1.2]);
  assert.deepEqual(placementPoint(hit, 1.5), [6.123, 2.5, 1.5]);
});

test('a raised floor keeps its height through the lift', () => {
  const raised = new Float64Array([0, 0, 0.1, 1, 0, 0.1, 1, 1, 0.1]);
  const hit = rayOnFacePlane(raised, new Uint32Array([0, 2, 1]), 0, [0.2, 0.3, 5], [0, 0, -1]);
  assert.deepEqual(hit, [0.2, 0.3, 0.1]);
  assert.deepEqual(placementPoint(hit as Vec, 1.2), [0.2, 0.3, 1.3]);
});

test('G48: a placement on a wall or the ceiling goes the lift off it, into the room, along its normal', () => {
  // Left wall (faces 4, 5) lies in y = 6, its outward normal +y: the point is 1.2 m inside, at y = 4.8.
  const wall = rayOnFacePlane(ROOM, ROOM_FACES, 4, [3.3, 2, 1.7], [0, 1, 0]);
  assert.deepEqual(wall, [3.3, 6, 1.7]);
  assert.deepEqual(placeOffFace(wall as Vec, faceNormalOf(ROOM, ROOM_FACES, 4), 1.2), { point: [3.3, 4.8, 1.7], how: 'off' });
  // The ceiling at z = 3: 1.5 m below it.
  const ceiling = rayOnFacePlane(ROOM, ROOM_FACES, 2, [4, 4, 1], [0, 0, 1]);
  assert.deepEqual(placeOffFace(ceiling as Vec, faceNormalOf(ROOM, ROOM_FACES, 2), 1.5), { point: [4, 4, 1.5], how: 'off' });
  // The floor keeps its height rule: straight up.
  assert.deepEqual(placeOffFace([6.1234, 2.5, 0], faceNormalOf(ROOM, ROOM_FACES, 0), 1.2), { point: [6.123, 2.5, 1.2], how: 'above' });
});

test('G48: a raked floor is lifted straight up, a steep face along its normal, a degenerate one not at all', () => {
  const t = (30 * Math.PI) / 180;
  assert.deepEqual(placeOffFace([1, 1, 2], [Math.sin(t), 0, -Math.cos(t)], 1.2), { point: [1, 1, 3.2], how: 'above' });
  const s = (60 * Math.PI) / 180;
  const steep = placeOffFace([1, 1, 2], [Math.sin(s), 0, -Math.cos(s)], 1)?.point;
  assert.deepEqual(steep, [0.134, 1, 2.5]);
  assert.equal(placeOffFace([0, 0, 0], [0, 0, 0], 1.2), null);
});

test('a ray parallel to the plane, or pointing away from it, gives no point', () => {
  assert.equal(rayOnFacePlane(ROOM, ROOM_FACES, 0, [1, 1, 1], [1, 0, 0]), null);
  assert.equal(rayOnFacePlane(ROOM, ROOM_FACES, 0, [1, 1, 1], [0, 0, 1]), null);
});

test('centroid, bounds and the plan label of the teaching room', () => {
  const c = faceCentroid(ROOM, ROOM_FACES, 2);
  assert.ok(Math.abs(c[0] - 20 / 3) < 1e-12 && Math.abs(c[1] - 2) < 1e-12 && c[2] === 3, String(c));
  const box = faceBounds(ROOM, ROOM_FACES);
  assert.deepEqual(box, { min: [0, 0, 0], max: [10, 6, 3] });
  assert.equal(planDimensions(box!), '10 × 6 m');
  assert.equal(faceBounds(new Float64Array(0), new Uint32Array(0)), null);
});

test('metres print with at most two decimals and no trailing zeros', () => {
  assert.equal(formatMetres(10), '10');
  assert.equal(formatMetres(42.371), '42.37');
  assert.equal(formatMetres(25.1), '25.1');
  assert.equal(formatMetres(0.004), '0');
  assert.equal(formatMetres(-0.001), '0');
  assert.equal(formatMetres(Number.NaN), '—');
});

test('the plan frustum keeps the aspect and never crops the room', () => {
  const wide = fitOrtho(10, 6, 2, 1);
  assert.deepEqual(wide, { halfW: 6, halfH: 3 });
  const tall = fitOrtho(10, 6, 1, 1);
  assert.deepEqual(tall, { halfW: 5, halfH: 5 });
  const inset = fitOrtho(10, 6, 180 / 116);
  assert.ok(inset.halfW >= 5 && inset.halfH >= 3);
  assert.ok(Math.abs(inset.halfW / inset.halfH - 180 / 116) < 1e-12);
});

test('the gizmo: an identity view shows x right, y up, z towards the viewer', () => {
  const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
  const [x, y, z] = gizmoAxes(identity, 10);
  assert.deepEqual([x.dx, x.dy, x.depth], [10, 0, 0]);
  assert.deepEqual([y.dx, y.dy], [0, -10]);
  assert.equal(z.depth, 1);
  // A view turned a quarter about z: world x points up the screen, world y to the left.
  const turned = [0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
  const [tx, ty] = gizmoAxes(turned, 10);
  assert.deepEqual([tx.dx, tx.dy], [0, -10]);
  assert.deepEqual([ty.dx, ty.dy], [-10, 0]);
});
