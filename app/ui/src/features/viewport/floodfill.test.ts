import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { buildTopology, coplanarFaces, COPLANAR_EPS, faceNormalOf } from './floodfill.ts';

type V = [number, number, number];

/** A mesh from vertex and face lists. */
function mesh(vertices: V[], faces: [number, number, number][]) {
  const positions = new Float64Array(vertices.flat());
  const indices = new Uint32Array(faces.flat());
  return { positions, indices, topo: buildTopology(positions, indices) };
}

/** An nx x ny grid of unit quads at z = 0, two triangles each, normals +z. With `split`, every
 * quad gets its own four vertices, so neighbours share positions but no vertex index. */
function grid(nx: number, ny: number, split = false) {
  const vertices: V[] = [];
  const faces: [number, number, number][] = [];
  if (!split) {
    for (let j = 0; j <= ny; j++) for (let i = 0; i <= nx; i++) vertices.push([i, j, 0]);
    const at = (i: number, j: number) => j * (nx + 1) + i;
    for (let j = 0; j < ny; j++)
      for (let i = 0; i < nx; i++) {
        faces.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
        faces.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
      }
  } else {
    for (let j = 0; j < ny; j++)
      for (let i = 0; i < nx; i++) {
        const b = vertices.length;
        vertices.push([i, j, 0], [i + 1, j, 0], [i + 1, j + 1, 0], [i, j + 1, 0]);
        faces.push([b, b + 1, b + 2], [b, b + 2, b + 3]);
      }
  }
  return mesh(vertices, faces);
}

const all = (n: number) => Array.from({ length: n }, (_, i) => i);

test('a grid plane fills completely from any face', () => {
  const { topo } = grid(4, 3);
  for (const seed of [0, 7, 23]) assert.deepEqual(coplanarFaces(topo, seed), all(24));
});

test('neighbours are found by exact vertex position, not by vertex index', () => {
  const { topo } = grid(3, 3, true);
  assert.deepEqual(coplanarFaces(topo, 4), all(18));
});

test('a riser stops the fill: the parallel upper floor is not reached', () => {
  // Lower floor z = 0 on x 0..2, a riser at x = 2 up to z = 0.2, an upper floor z = 0.2 on x 2..4.
  const vertices: V[] = [
    [0, 0, 0], [2, 0, 0], [2, 1, 0], [0, 1, 0], // 0-3 lower floor
    [2, 0, 0.2], [2, 1, 0.2], // 4-5 riser top
    [4, 0, 0.2], [4, 1, 0.2], // 6-7 upper floor far edge
  ];
  const faces: [number, number, number][] = [
    [0, 1, 2], [0, 2, 3], // 0-1 lower floor (+z)
    [1, 4, 5], [1, 5, 2], // 2-3 riser (normal -x or +x: vertical)
    [4, 6, 7], [4, 7, 5], // 4-5 upper floor (+z)
  ];
  const { topo } = mesh(vertices, faces);
  assert.deepEqual(coplanarFaces(topo, 0), [0, 1]);
  assert.deepEqual(coplanarFaces(topo, 5), [4, 5]);
  assert.deepEqual(coplanarFaces(topo, 2), [2, 3]);
});

/** Two unit quads sharing the edge x = 1; the second is tilted by `deg` about that edge. */
function hinge(deg: number) {
  const t = (deg * Math.PI) / 180;
  const vertices: V[] = [
    [0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0],
    [1 + Math.cos(t), 0, Math.sin(t)], [1 + Math.cos(t), 1, Math.sin(t)],
  ];
  const faces: [number, number, number][] = [[0, 1, 2], [0, 2, 3], [1, 4, 5], [1, 5, 2]];
  return mesh(vertices, faces).topo;
}

test('a 1 degree tilt is taken, a 5 degree tilt is refused', () => {
  assert.ok(Math.cos((1 * Math.PI) / 180) > COPLANAR_EPS);
  assert.ok(Math.cos((5 * Math.PI) / 180) < COPLANAR_EPS);
  assert.deepEqual(coplanarFaces(hinge(1), 0), [0, 1, 2, 3]);
  assert.deepEqual(coplanarFaces(hinge(5), 0), [0, 1]);
});

test("the criterion is the seed's normal, not the neighbour's: a slow curve stops", () => {
  // A strip of quads, each turned 1 degree further than the last. Every neighbour pair differs
  // by 1 degree; the fill takes quads up to 3 degrees from the seed and stops before 4.
  const vertices: V[] = [
    [0, 0, 0],
    [0, 1, 0],
  ];
  const faces: [number, number, number][] = [];
  let x = 0;
  let z = 0;
  for (let k = 0; k < 8; k++) {
    const t = (k * Math.PI) / 180;
    x += Math.cos(t);
    z += Math.sin(t);
    const b = vertices.length;
    vertices.push([x, 0, z], [x, 1, z]);
    faces.push([b - 2, b, b + 1], [b - 2, b + 1, b - 1]);
  }
  const { topo } = mesh(vertices, faces);
  // Quads 0..3 are at 0, 1, 2 and 3 degrees: faces 0..7.
  assert.deepEqual(coplanarFaces(topo, 0), all(8));
});

test('an opposite-facing coplanar neighbour is taken (|dot|, as upstream)', () => {
  const vertices: V[] = [[0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0]];
  const { topo } = mesh(vertices, [[0, 1, 2], [0, 3, 2]]);
  assert.deepEqual(faceNormalOf(new Float64Array(vertices.flat()), new Uint32Array([0, 3, 2]), 0), [0, 0, -1]);
  assert.deepEqual(coplanarFaces(topo, 0), [0, 1]);
});

test('the tutorial 1 box: the ceiling gives [10, 11], a wall gives its 2 faces, not the 8 of Walls', () => {
  // tests/fixtures/rooms/tutorial1_box.simpa: groups Floor 0-1, Walls 2-9, Ceiling 10-11.
  const vertices: V[] = [
    [6, 0, 0], [0, 0, 0], [0, 10, 0], [6, 10, 0], [0, 10, 3], [6, 10, 3], [0, 0, 3], [6, 0, 3],
  ];
  const faces: [number, number, number][] = [
    [0, 1, 2], [0, 2, 3], [2, 4, 5], [2, 5, 3], [2, 6, 4], [2, 1, 6],
    [1, 0, 7], [6, 1, 7], [0, 3, 5], [7, 0, 5], [7, 5, 4], [6, 7, 4],
  ];
  const { topo } = mesh(vertices, faces);
  assert.deepEqual(coplanarFaces(topo, 10), [10, 11]);
  assert.deepEqual(coplanarFaces(topo, 11), [10, 11]);
  assert.deepEqual(coplanarFaces(topo, 0), [0, 1]);
  assert.deepEqual(coplanarFaces(topo, 2), [2, 3]);
  assert.deepEqual(coplanarFaces(topo, 4), [4, 5]);
  assert.deepEqual(coplanarFaces(topo, 6), [6, 7]);
  assert.deepEqual(coplanarFaces(topo, 9), [8, 9]);
  // Outward normals: the ceiling points up, the floor down.
  assert.deepEqual([...topo.normals.subarray(30, 33)], [0, 0, 1]);
  assert.deepEqual([...topo.normals.subarray(0, 3)], [0, 0, -1]);
});

test('the fill is not limited to a group: it has no group input and crosses group borders', () => {
  // The grid's two halves could be two groups; the fill takes both.
  const { topo } = grid(6, 1);
  assert.deepEqual(coplanarFaces(topo, 0), all(12));
});

test('a 200 x 200 floor (80,000 faces) fills without recursion', () => {
  const { topo } = grid(200, 200);
  const out = coplanarFaces(topo, 12345);
  assert.equal(out.length, 80000);
  assert.equal(out[0], 0);
  assert.equal(out[79999], 79999);
});

test('a degenerate seed takes only itself; a bad seed takes nothing', () => {
  const vertices: V[] = [[0, 0, 0], [1, 0, 0], [2, 0, 0], [0, 1, 0]];
  const { topo } = mesh(vertices, [[0, 1, 2], [0, 1, 3]]);
  assert.deepEqual([...topo.normals.subarray(0, 3)], [0, 0, 0]);
  assert.deepEqual(coplanarFaces(topo, 0), [0]);
  assert.deepEqual(coplanarFaces(topo, 2), []);
  assert.deepEqual(coplanarFaces(topo, -1), []);
  assert.deepEqual(coplanarFaces(topo, 0.5), []);
});
