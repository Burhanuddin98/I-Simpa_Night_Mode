import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { BackSide, Ray, Vector3 } from 'three';
import { MeshBVH } from 'three-mesh-bvh';
import { faceNormalOf } from './floodfill.ts';
import { faceCentroid } from './geometry.ts';
import { firstFace, modelGeometry, pickingBvh } from './pick.ts';

// tests/fixtures/rooms/tutorial1_box.simpa: groups Floor 0-1, Walls 2-9, Ceiling 10-11.
const BOX = new Float64Array([6, 0, 0, 0, 0, 0, 0, 10, 0, 6, 10, 0, 0, 10, 3, 6, 10, 3, 0, 0, 3, 6, 0, 3]);
const BOX_FACES = new Uint32Array([0, 1, 2, 0, 2, 3, 2, 4, 5, 2, 5, 3, 2, 6, 4, 2, 1, 6, 1, 0, 7, 6, 1, 7, 0, 3, 5, 7, 0, 5, 7, 5, 4, 6, 7, 4]);

/** A ray from `distance` out on face f's interior side, through its centroid. */
function rayAtFace(positions: Float64Array, indices: Uint32Array, f: number, distance: number): Ray {
  const c = new Vector3(...faceCentroid(positions, indices, f));
  const n = new Vector3(...faceNormalOf(positions, indices, f));
  return new Ray(c.clone().addScaledVector(n, -distance), n);
}

test('every face of the box is picked by its own project index, from inside and from beyond the far wall', () => {
  const g = modelGeometry(BOX, BOX_FACES);
  const bvh = pickingBvh(g);
  for (let f = 0; f < 12; f++) {
    assert.equal(firstFace(bvh, rayAtFace(BOX, BOX_FACES, f, 0.5))?.face, f, `face ${f} from inside`);
    // From 20 m out the ray enters through the opposite wall's outer side, which BackSide skips.
    assert.equal(firstFace(bvh, rayAtFace(BOX, BOX_FACES, f, 20))?.face, f, `face ${f} through the far wall`);
  }
  assert.deepEqual([...(g.index?.array ?? [])], [...BOX_FACES], 'the index keeps project face order');
});

test('picks honour BackSide: from above, the ceiling is not pickable and the floor is', () => {
  const bvh = pickingBvh(modelGeometry(BOX, BOX_FACES));
  const hit = firstFace(bvh, new Ray(new Vector3(4, 6.5, 20), new Vector3(0, 0, -1)));
  assert.ok(hit && (hit.face === 0 || hit.face === 1), `hit ${JSON.stringify(hit)}`);
  assert.equal(hit.point.z, 0);
});

/** An n x n grid at z = 0 (normals +z), its faces in a shuffled order. */
function shuffledGrid(n: number, seed: number) {
  const positions: number[] = [];
  for (let j = 0; j <= n; j++) for (let i = 0; i <= n; i++) positions.push(i, j, 0);
  const at = (i: number, j: number) => j * (n + 1) + i;
  const faces: number[][] = [];
  for (let j = 0; j < n; j++)
    for (let i = 0; i < n; i++) {
      faces.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
      faces.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
    }
  let s = seed;
  const rand = () => ((s = (Math.imul(s, 1664525) + 1013904223) >>> 0) / 2 ** 32);
  for (let k = faces.length - 1; k > 0; k--) {
    const r = Math.floor(rand() * (k + 1));
    [faces[k], faces[r]] = [faces[r], faces[k]];
  }
  return { positions: new Float64Array(positions), indices: new Uint32Array(faces.flat()) };
}

test('on a large shuffled mesh, faceIndex is the project face index (indirect BVH)', () => {
  const { positions, indices } = shuffledGrid(40, 7);
  const g = modelGeometry(positions, indices);
  const bvh = pickingBvh(g);
  assert.equal(bvh.indirect, true);
  const faces = indices.length / 3;
  for (let f = 0; f < faces; f += 37) {
    // Normals are +z, so the room's side is below: shoot up.
    assert.equal(firstFace(bvh, rayAtFace(positions, indices, f, 5))?.face, f, `face ${f}`);
  }
  assert.deepEqual([...(g.index?.array ?? [])], [...indices]);
});

test('control: without indirect, three-mesh-bvh reorders the index and picks name other faces', () => {
  const { positions, indices } = shuffledGrid(40, 7);
  const g = modelGeometry(positions, indices);
  const bvh = new MeshBVH(g);
  assert.notDeepEqual([...(g.index?.array ?? [])], [...indices], 'the index was rearranged');
  let wrong = 0;
  for (let f = 0; f < indices.length / 3; f += 37) {
    const hit = bvh.raycastFirst(rayAtFace(positions, indices, f, 5), BackSide);
    if (hit?.faceIndex !== f) wrong++;
  }
  assert.ok(wrong > 0, 'a non-indirect BVH gives other face numbers');
});
