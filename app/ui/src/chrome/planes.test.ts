import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { RunData } from '../bindings/ipc.ts';
import type { SurfaceReceiver } from '../bindings/schema.ts';
import { addSurfaceReceiver, newCuttingPlane, opText, removeSurfaceReceiver, replaceSurfaceReceiver } from '../ops.ts';
import {
  EAR_HEIGHT_M,
  PLANE_RESOLUTION_M,
  earPlane,
  heightAboveFloor,
  levelHeight,
  parseHeightAboveFloor,
  parseResolution,
  planeCells,
  planesNotInRun,
  rerunText,
  roomBox,
  withLevelHeight,
  withCorner,
  planeTilt,
  parseCoordinate,
} from './planes.ts';

const CHECK = { bbox_min: [-1, 2, 0.5] as [number, number, number], extents_m: [10, 6, 3] as [number, number, number] };
const BOX = roomBox(CHECK)!;

test('roomBox reads the check: min and min + extents; none without a finite check', () => {
  assert.deepEqual(BOX, { min: [-1, 2, 0.5], max: [9, 8, 3.5] });
  assert.equal(roomBox(null), null);
  assert.equal(roomBox({ bbox_min: [0, 0, 0], extents_m: [Number.NaN, 1, 1] }), null);
});

test('earPlane is upstream\'s new plane: A (xmin, ymax), B (xmin, ymin), C (xmax, ymin), at the floor + 1.6 m', () => {
  const p = earPlane(BOX);
  // e_scene_recepteurss_recepteurcoupe.h:101-103, the model's box, z up.
  assert.deepEqual(p.a, [-1, 8, 2.1]);
  assert.deepEqual(p.b, [-1, 2, 2.1]);
  assert.deepEqual(p.c, [9, 2, 2.1]);
  assert.equal(EAR_HEIGHT_M, 1.6);
  assert.equal(PLANE_RESOLUTION_M, 1);
  assert.deepEqual(earPlane(BOX, 1.2).a, [-1, 8, 0.5 + 1.2]);
});

test('planeCells is the solver\'s ceil(|BC| / r) x ceil(|BA| / r), two map faces a cell', () => {
  const p = earPlane(BOX);
  assert.deepEqual(planeCells(p.a, p.b, p.c, 1), { u: 10, v: 6, cells: 60, faces: 120 });
  assert.deepEqual(planeCells(p.a, p.b, p.c, 0.5), { u: 20, v: 12, cells: 240, faces: 480 });
  assert.deepEqual(planeCells(p.a, p.b, p.c, 4), { u: 3, v: 2, cells: 6, faces: 12 }, 'a partial cell counts');
  // say NO: no cells for a resolution the core refuses or a corner that is not a number.
  assert.equal(planeCells(p.a, p.b, p.c, 0), null);
  assert.equal(planeCells(p.a, p.b, p.c, -1), null);
  assert.equal(planeCells(['NaN', 0, 0], p.b, p.c, 1), null);
});

test('levelHeight is the z of a level plane, null for a tilted one; withLevelHeight moves all three corners', () => {
  const p = earPlane(BOX);
  const shape = { kind: 'cutting_plane' as const, a: p.a, b: p.b, c: p.c, resolution_m: 1 };
  assert.equal(levelHeight(shape), 2.1);
  assert.equal(heightAboveFloor(shape, BOX), 2.1 - 0.5);
  const tilted = { ...shape, c: [9, 2, 2.4] as typeof shape.c };
  assert.equal(levelHeight(tilted), null, 'say NO: a tilted plane has no one height');
  assert.equal(heightAboveFloor(tilted, BOX), null);
  assert.equal(levelHeight({ kind: 'scene', groups: [] }), null);
  const moved = withLevelHeight(shape, 1.7);
  assert.deepEqual([moved.a[2], moved.b[2], moved.c[2]], [1.7, 1.7, 1.7]);
  assert.deepEqual([moved.a[0], moved.a[1], moved.c[0]], [-1, 8, 9], 'x and y kept');
});

test('parseHeightAboveFloor takes a height strictly between the floor and the ceiling', () => {
  assert.deepEqual(parseHeightAboveFloor('1.2', BOX), { ok: true, value: 1.2 });
  assert.deepEqual(parseHeightAboveFloor(' 2.95 ', BOX), { ok: true, value: 2.95 });
  // say NO
  const no = (t: string) => {
    const r = parseHeightAboveFloor(t, BOX);
    assert.equal(r.ok, false, t);
    return r.ok ? '' : r.message;
  };
  assert.match(no('abc'), /not a number/);
  assert.match(no('1,2'), /not a number/);
  assert.match(no('0'), /above the floor/);
  assert.match(no('-0.5'), /above the floor/);
  assert.match(no('3'), /below the ceiling, 3 m above the floor/);
  assert.match(no('7'), /below the ceiling/);
});

test('parseResolution takes a positive size and refuses others before the core sees them', () => {
  assert.deepEqual(parseResolution('0.5'), { ok: true, value: 0.5 });
  for (const t of ['0', '-1', 'x', '']) assert.equal(parseResolution(t).ok, false, t);
  const r = parseResolution('0');
  assert.ok(!r.ok && /above 0/.test(r.message));
});

const plane = (name: string, enabled = true): SurfaceReceiver => ({ ...newCuttingPlane(name, name, [0, 1, 1], [0, 0, 1], [1, 0, 1], 1), enabled });
const scene: SurfaceReceiver = { id: 's', name: 'Floor', enabled: true, shape: { kind: 'scene', groups: [] }, solver_id: null };
const run = (planes: string[], surface: string[] = []): RunData => ({
  bands_hz: [1000],
  echograms: [],
  particle_files: [],
  solver: 'spps',
  steps: 10,
  time_step_s: 0.01,
  surfaces: [
    { band_hz: 1000, cutting_plane: true, faces: 2, nodes: 4, path: 'Cutting plane/1000 Hz/Sound level.csbin', receivers: planes.map((name, i) => ({ name, faces: 2, xml_index: i })), records: 0, time_step_count: 10, time_step_s: 0.01 },
    { band_hz: 1000, cutting_plane: false, faces: 2, nodes: 4, path: 'Surface receiver/1000 Hz/Sound level.csbin', receivers: surface.map((name, i) => ({ name, faces: 2, xml_index: i })), records: 0, time_step_count: 10, time_step_s: 0.01 },
  ],
});

test('planesNotInRun names the enabled planes the run\'s cutting-plane maps do not hold', () => {
  assert.deepEqual(planesNotInRun([plane('Plane 1'), plane('Cut')], run(['Cut'])), ['Plane 1']);
  assert.deepEqual(planesNotInRun([plane('Cut')], run(['Cut'])), [], 'say NO: a plane the run holds is not named');
  assert.deepEqual(planesNotInRun([plane('Off', false)], run([])), [], 'a disabled plane is not run');
  assert.deepEqual(planesNotInRun([scene], run([])), [], 'a surface receiver is not a plane');
  assert.deepEqual(planesNotInRun([plane('Floor')], run([], ['Floor'])), ['Floor'], 'a surface receiver of the same name is not the plane');
  assert.deepEqual(planesNotInRun([plane('P')], null), []);
});

test('rerunText says SPPS must run again, or nothing', () => {
  assert.equal(rerunText([]), null);
  assert.equal(rerunText(['Plane 1']), 'Plane 1 is not in this run: run SPPS again to map it.');
  assert.equal(rerunText(['Plane 1', 'Plane 2']), 'Plane 1 and Plane 2 are not in this run: run SPPS again to map them.');
});

test('the surface-receiver ops are the core\'s, as text', () => {
  const p = newCuttingPlane('id1', 'Plane 1', [0, 1, 1.6], [0, 0, 1.6], [1, 0, 1.6], 1);
  assert.equal(
    opText(addSurfaceReceiver(2, p)),
    '{"op":"add_surface_receiver","index":2,"receiver":{"id":"id1","name":"Plane 1","enabled":true,"shape":{"kind":"cutting_plane","a":[0,1,1.6],"b":[0,0,1.6],"c":[1,0,1.6],"resolution_m":1},"solver_id":null}}',
  );
  assert.equal(opText(removeSurfaceReceiver('id1')), '{"op":"remove_surface_receiver","id":"id1"}');
  assert.match(opText(replaceSurfaceReceiver(p)), /^\{"op":"replace_surface_receiver","receiver":\{"id":"id1"/);
});

test('M41: a corner moved on one axis tilts the plane; its normal and tilt from BC x BA; a level plane reads 0 degrees', () => {
  const level = { kind: 'cutting_plane' as const, ...earPlane(BOX), resolution_m: 1 };
  assert.deepEqual(planeTilt(level)?.tiltDeg, 0);
  assert.equal(levelHeight(level), 2.1);
  // A raised 2 m above B and C: the plane leans about the BC edge, still a plane with the same cells along BC.
  const tilted = withCorner(level, 'a', 2, 4.1);
  assert.deepEqual(tilted.b, level.b);
  assert.deepEqual(tilted.c, level.c);
  assert.deepEqual(tilted.a, [-1, 8, 4.1]);
  assert.equal(levelHeight(tilted), null);
  const t = planeTilt(tilted)!;
  assert.ok(Math.abs(t.tiltDeg - (Math.atan2(2, 6) * 180) / Math.PI) < 1e-9, `${t.tiltDeg}`);
  assert.equal(planeCells(tilted.a, tilted.b, tilted.c, 1)?.u, 10);
  assert.equal(planeCells(tilted.a, tilted.b, tilted.c, 1)?.v, Math.ceil(Math.fround(Math.hypot(6, 2))));
  // Upright: A straight above B.
  const upright = withCorner(withCorner(level, 'a', 1, 2), 'a', 2, 3.1);
  assert.ok(Math.abs(planeTilt(upright)!.tiltDeg - 90) < 1e-9);
  // Collinear corners have no plane.
  assert.equal(planeTilt({ ...level, a: [0, 2, 2.1], b: [1, 2, 2.1], c: [2, 2, 2.1] }), null);
});

test('M41: a corner coordinate is any finite number; words are refused before the core', () => {
  assert.deepEqual(parseCoordinate('-3.25'), { ok: true, value: -3.25 });
  assert.equal(parseCoordinate('two').ok, false);
});
