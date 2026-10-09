import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { decodeParameterMap, decodeParticles, decodeSurfaceMap, PART_MAGIC, PMAP_MAGIC, SMAP_MAGIC, surfaceValue } from './resultsData.ts';

/** SMAP v1 as `results_data::encode_surface` writes it, built by hand. */
function smap(nodes: number[], faces: { v: number[]; r: number; rec: [number, number][] }[], dt = 0.01, steps = 7): ArrayBuffer {
  const nn = nodes.length / 3;
  const nf = faces.length;
  const nr = faces.reduce((n, f) => n + f.rec.length, 0);
  const buf = new ArrayBuffer(32 + 12 * nn + 12 * nf + 4 * nf + 4 * (nf + 1) + 8 * nr);
  const d = new DataView(buf);
  d.setUint32(0, SMAP_MAGIC, true);
  d.setUint32(4, 1, true);
  d.setUint32(8, nn, true);
  d.setUint32(12, nf, true);
  d.setUint32(16, nr, true);
  d.setUint32(20, steps, true);
  d.setFloat32(24, dt, true);
  d.setInt32(28, 1, true);
  let o = 32;
  for (const x of nodes) (d.setFloat32(o, x, true), (o += 4));
  for (const f of faces) for (const v of f.v) (d.setUint32(o, v, true), (o += 4));
  for (const f of faces) (d.setUint32(o, f.r, true), (o += 4));
  let at = 0;
  d.setUint32(o, 0, true);
  o += 4;
  for (const f of faces) ((at += f.rec.length), d.setUint32(o, at, true), (o += 4));
  for (const f of faces) for (const [s] of f.rec) (d.setUint32(o, s, true), (o += 4));
  for (const f of faces) for (const [, e] of f.rec) (d.setFloat32(o, e, true), (o += 4));
  return buf;
}

/** PART v1 as `results_data::encode_particles` writes it, built by hand. */
function part(particles: { first: number; steps: [number, number, number, number][] }[], band = 500): ArrayBuffer {
  const np = particles.length;
  const nr = particles.reduce((n, p) => n + p.steps.length, 0);
  const buf = new ArrayBuffer(32 + 4 * np + 4 * (np + 1) + 16 * nr);
  const d = new DataView(buf);
  d.setUint32(0, PART_MAGIC, true);
  d.setUint32(4, 1, true);
  d.setUint32(8, np, true);
  d.setUint32(12, nr, true);
  d.setUint32(16, 100, true);
  d.setFloat32(20, 0.002, true);
  d.setInt32(24, band, true);
  let o = 32;
  for (const p of particles) (d.setUint32(o, p.first, true), (o += 4));
  let at = 0;
  d.setUint32(o, 0, true);
  o += 4;
  for (const p of particles) ((at += p.steps.length), d.setUint32(o, at, true), (o += 4));
  for (const p of particles) for (const s of p.steps) for (const x of s.slice(0, 3)) (d.setFloat32(o, x, true), (o += 4));
  for (const p of particles) for (const s of p.steps) (d.setFloat32(o, s[3], true), (o += 4));
  return buf;
}

test('a surface map decodes to its float32 values bit for bit, by face and step', () => {
  const m = decodeSurfaceMap(
    smap([0, 0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 0], [
      { v: [0, 1, 2], r: 0, rec: [[0, 0.1], [3, 1e-30]] },
      { v: [1, 3, 2], r: 1, rec: [[2, 2.5]] },
    ]),
  );
  assert.equal(m.nodeCount, 4);
  assert.equal(m.faceCount, 2);
  assert.equal(m.recordCount, 3);
  assert.equal(m.timeStepCount, 7);
  assert.equal(m.timeStepS, Math.fround(0.01));
  assert.deepEqual([...m.indices], [0, 1, 2, 1, 3, 2]);
  assert.deepEqual([...m.receivers], [0, 1]);
  assert.deepEqual([...m.offsets], [0, 2, 3]);
  // 0.1 as float32, not as the double 0.1: the file's bits.
  assert.equal(surfaceValue(m, 0, 0), Math.fround(0.1));
  assert.notEqual(surfaceValue(m, 0, 0), 0.1);
  assert.equal(surfaceValue(m, 0, 3), Math.fround(1e-30));
  assert.equal(surfaceValue(m, 1, 2), 2.5);
  // A step the face has no record for is null, not 0.
  assert.equal(surfaceValue(m, 0, 1), null);
  assert.equal(surfaceValue(m, 1, 0), null);
});

test('particles decode to their positions and energies bit for bit', () => {
  const p = decodeParticles(
    part([
      { first: 3, steps: [[0.1, 0.2, 0.3, 1], [0.4, 0.5, 0.6, 0.5]] },
      { first: 0, steps: [[1, 2, 3, 0.25]] },
    ], 1000),
  );
  assert.equal(p.particleCount, 2);
  assert.equal(p.recordCount, 3);
  assert.equal(p.bandHz, 1000);
  assert.equal(p.maxSteps, 100);
  assert.equal(p.timeStepS, Math.fround(0.002));
  assert.deepEqual([...p.firstStep], [3, 0]);
  assert.deepEqual([...p.offsets], [0, 2, 3]);
  assert.deepEqual([...p.positions.slice(0, 3)], [Math.fround(0.1), Math.fround(0.2), Math.fround(0.3)]);
  assert.deepEqual([...p.energies], [1, 0.5, 0.25]);
});

test('a buffer that is not the layout is refused, never read', () => {
  const good = smap([0, 0, 0], [{ v: [0, 0, 0], r: 0, rec: [[0, 1]] }]);
  const bad = good.slice(0);
  new DataView(bad).setUint32(0, 0, true);
  assert.throws(() => decodeSurfaceMap(bad), /magic/);
  const v2 = good.slice(0);
  new DataView(v2).setUint32(4, 2, true);
  assert.throws(() => decodeSurfaceMap(v2), /version/);
  assert.throws(() => decodeSurfaceMap(good.slice(0, good.byteLength - 4)), /header says/);
  assert.throws(() => decodeSurfaceMap(new ArrayBuffer(8)), /shorter than the header/);
  const p = part([{ first: 0, steps: [[0, 0, 0, 1]] }]);
  assert.throws(() => decodeParticles(good), /magic/);
  assert.throws(() => decodeParticles(p.slice(0, p.byteLength - 4)), /header says/);
});

test('R42: a PMAP is the map JSON, padded to 4, then its SMAP; a refused map has no SMAP', () => {
  const s = smap([0, 0, 0, 1, 0, 0, 0, 1, 0], [{ v: [0, 1, 2], r: 0, rec: [[0, 0.95]] }, { v: [0, 1, 2], r: 0, rec: [[0, NaN]] }], 0.001, 1);
  const pmap = (view: unknown, tail: ArrayBuffer | null) => {
    const json = new TextEncoder().encode(JSON.stringify(view));
    const pad = (4 - (json.length % 4)) % 4;
    const b = new Uint8Array(12 + json.length + pad + (tail ? tail.byteLength : 0));
    const d = new DataView(b.buffer);
    d.setUint32(0, PMAP_MAGIC, true);
    d.setUint32(4, 1, true);
    d.setUint32(8, json.length, true);
    b.set(json, 12);
    if (tail) b.set(new Uint8Array(tail), 12 + json.length + pad);
    return b.buffer;
  };
  const view = { map: { parameter: 't30_s', values: [0.9512345678, null], why: [null, 'range_not_reached'], unit: 's' }, refusal: null };
  const got = decodeParameterMap(pmap(view, s));
  assert.equal(got.view.map?.values[0], 0.9512345678, "core's double, not the texture's float");
  assert.equal(got.map?.faceCount, 2);
  assert.ok(Number.isNaN(got.map?.values[1] as number));
  const refused = decodeParameterMap(pmap({ map: null, refusal: { code: 'map_global', message: 'per band' } }, null));
  assert.equal(refused.map, null);
  assert.equal(refused.view.refusal?.code, 'map_global');
  // Says no: a map whose faces are not its values' is refused.
  assert.throws(() => decodeParameterMap(pmap({ ...view, map: { ...view.map, values: [0.9] } }, s)), /2 faces, 1 values/);
});
