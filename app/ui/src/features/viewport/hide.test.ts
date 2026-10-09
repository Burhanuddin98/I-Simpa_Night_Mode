// hide.ts under `node --test` (Burhan's 10-05 UI list, items 7 Roof off and 8 Isolate).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { groupList, groupNamesOf, indexOf, isolateRefusal, leftOut, roofFaces, ROOF_MIN_UP, shownPerVertex } from './hide.ts';

// A toy model as columns: each face has an outward normal z and a height; a face is covered when a face
// not left out lies above it in the same column (the ray straight up meets it).
function model(faces: { up: number; col: number; z: number }[]) {
  return {
    upZ: (f: number) => faces[f].up,
    coveredAbove: (f: number, out: Uint8Array) => faces.some((g, j) => j !== f && !out[j] && g.col === faces[f].col && g.z > faces[f].z),
  };
}

test('the roof is what faces down inside with nothing above it; floors and walls never are', () => {
  const m = model([
    { up: -1, col: 0, z: 0 }, // floor
    { up: 1, col: 0, z: 10 }, // roof
    { up: 0, col: 1, z: 5 }, // wall
    { up: 1, col: 1, z: 10 }, // roof
  ]);
  assert.deepEqual(roofFaces(4, m.upZ, m.coveredAbove), [1, 3]);
});

test('a pitched roof counts, a wall leaning 80 degrees does not', () => {
  const pitched = Math.cos((40 * Math.PI) / 180);
  const leaning = Math.cos((80 * Math.PI) / 180);
  assert.ok(pitched >= ROOF_MIN_UP && leaning < ROOF_MIN_UP);
  const m = model([
    { up: pitched, col: 0, z: 9 },
    { up: leaning, col: 1, z: 9 },
  ]);
  assert.deepEqual(roofFaces(2, m.upZ, m.coveredAbove), [0]);
});

test('a ceiling under the roof is peeled once the roof is off; a balcony underside under its own floor stays', () => {
  const m = model([
    { up: 1, col: 0, z: 12 }, // roof
    { up: 1, col: 0, z: 10 }, // ceiling under it
    { up: 1, col: 1, z: 4 }, // balcony underside (inside faces down)
    { up: -1, col: 1, z: 4.3 }, // the balcony's floor above it (inside faces up): never a roof, always covers
    { up: 1, col: 1, z: 12 }, // the roof over the balcony
  ]);
  assert.deepEqual(roofFaces(5, m.upZ, m.coveredAbove), [0, 1, 4]);
});

test('nothing left out draws the whole model (null); the roof and isolate combine', () => {
  assert.equal(leftOut(5, null, null), null);
  assert.equal(leftOut(5, [], null), null);
  assert.deepEqual([...(leftOut(5, [1], null) as Uint8Array)], [0, 1, 0, 0, 0]);
  assert.deepEqual([...(leftOut(5, null, [2, 3]) as Uint8Array)], [1, 1, 0, 0, 1]);
  assert.deepEqual([...(leftOut(5, [3], [2, 3]) as Uint8Array)], [1, 1, 0, 1, 1], 'isolated, and the roof off within it');
  assert.deepEqual([...(leftOut(3, null, []) as Uint8Array)], [1, 1, 1], 'isolating nothing leaves nothing');
});

test('the drawn and the left-out triangles split the index between them, in face order', () => {
  const idx = new Uint32Array([0, 1, 2, 2, 1, 3, 3, 1, 4]);
  const out = new Uint8Array([0, 1, 0]);
  assert.deepEqual([...indexOf(idx, out, 0)], [0, 1, 2, 3, 1, 4]);
  assert.deepEqual([...indexOf(idx, out, 1)], [2, 1, 3]);
  assert.deepEqual([...shownPerVertex(3, out)], [1, 1, 1, 0, 0, 0, 1, 1, 1]);
  assert.deepEqual([...shownPerVertex(2, null)], [1, 1, 1, 1, 1, 1]);
});

test('Isolate takes picked faces or surface groups, and says why not otherwise', () => {
  assert.equal(isolateRefusal('faces', 3), null);
  assert.equal(isolateRefusal('groups', 120), null);
  assert.match(isolateRefusal('source', 0) ?? '', /not a source or receiver/);
  assert.match(isolateRefusal('none', 0) ?? '', /Pick faces/);
});

test('the chip names the groups hidden, in the scene order, and shortens a long list', () => {
  assert.deepEqual(groupNamesOf([5, 0, 1], [2, 2, 0, 1, 1, 0], ['Floor', 'Walls', 'Ceiling']), ['Floor', 'Ceiling']);
  assert.equal(groupList(['A', 'B']), 'A, B');
  assert.equal(groupList(['A', 'B', 'C', 'D', 'E']), 'A, B, C and 2 more');
});
