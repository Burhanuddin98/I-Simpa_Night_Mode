import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { decodeMesh, MESH_MAGIC } from './mesh.ts';

/** The layout `scene::mesh_bytes` writes, built by hand. */
function encode(positions: number[], faces: number[][], groups: number[], rev: bigint): ArrayBuffer {
  const nv = positions.length / 3;
  const nf = faces.length;
  const buf = new ArrayBuffer(24 + 24 * nv + 16 * nf);
  const v = new DataView(buf);
  v.setUint32(0, MESH_MAGIC, true);
  v.setUint32(4, 1, true);
  v.setUint32(8, nv, true);
  v.setUint32(12, nf, true);
  v.setBigUint64(16, rev, true);
  positions.forEach((x, i) => v.setFloat64(24 + 8 * i, x, true));
  const fo = 24 + 24 * nv;
  faces.forEach((f, i) => f.forEach((x, k) => v.setUint32(fo + 12 * i + 4 * k, x, true)));
  groups.forEach((g, i) => v.setUint32(fo + 12 * nf + 4 * i, g, true));
  return buf;
}

test('a mesh decodes bit for bit', () => {
  const pos = [0, 0, 0, 10, 0, 0, 0.1 + 0.2, -0, 3];
  const m = decodeMesh(encode(pos, [[0, 1, 2]], [4], 7n));
  assert.equal(m.geometryRev, 7);
  assert.equal(m.vertexCount, 3);
  assert.equal(m.faceCount, 1);
  assert.deepEqual([...m.indices], [0, 1, 2]);
  assert.deepEqual([...m.groups], [4]);
  assert.equal(m.positions[6], 0.1 + 0.2);
  assert.ok(Object.is(m.positions[7], -0));
});

test('a bad header or size is refused', () => {
  const good = encode([0, 0, 0], [], [], 1n);
  const bad = good.slice(0);
  new DataView(bad).setUint32(0, 0, true);
  assert.throws(() => decodeMesh(bad), /magic/);
  const v2 = good.slice(0);
  new DataView(v2).setUint32(4, 2, true);
  assert.throws(() => decodeMesh(v2), /version/);
  assert.throws(() => decodeMesh(good.slice(0, 30)), /header says/);
  assert.throws(() => decodeMesh(new ArrayBuffer(8)), /shorter than the header/);
});
