import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { SurfaceMap } from '../../resultsData.ts';
import {
  CONTOUR_STEPS_DB,
  MAX_NODE_FACES,
  MAX_RANGE_DB,
  contourText,
  nodeEnergy,
  nodeFaces,
  parseRange,
  probeOf,
  probeOfParam,
  stepTime,
  valueBits,
} from './mapView.ts';

/**
 * Two triangles sharing the edge 1-2, four nodes; face 0 has records at steps 0 and 2, face 1 at
 * step 0 only; face 2 (a third triangle on nodes 2, 3, 4) has no energy at any step.
 */
function map(): SurfaceMap {
  const values = new Float32Array([1e-6, 4e-6, 3e-6, 0]);
  return {
    nodeCount: 5,
    faceCount: 3,
    recordCount: values.length,
    timeStepCount: 4,
    timeStepS: 0.01,
    recordType: 0,
    positions: new Float32Array(15),
    indices: new Uint32Array([0, 1, 2, 1, 3, 2, 2, 3, 4]),
    receivers: new Uint32Array([0, 0, 0]),
    offsets: new Uint32Array([0, 2, 3, 4]),
    steps: new Uint32Array([0, 2, 0, 1]),
    values,
  };
}

test('nodeFaces links each node to the faces that carry energy at some step (upstream\'s quantFacesLinked)', () => {
  const nf = nodeFaces(map());
  const of = (n: number) => [...nf.faces.subarray(nf.offsets[n], nf.offsets[n + 1])];
  assert.deepEqual(of(0), [0]);
  assert.deepEqual(of(1), [0, 1]);
  assert.deepEqual(of(2), [0, 1], 'say NO: face 2 never has energy, so it is not linked (Recepteurs_surfacique.cpp:358-363)');
  assert.deepEqual(of(3), [1]);
  assert.deepEqual(of(4), [], 'a node of only dead faces has no faces');
  assert.equal(nf.maxFaces, 2);
  assert.ok(MAX_NODE_FACES >= 32);
});

test('nodeEnergy is the sum of its faces\' energies at the step over their count; 0 where a face has no record', () => {
  const m = map();
  const nf = nodeFaces(m);
  const f = (x: number) => Math.fround(x);
  assert.equal(nodeEnergy(m, nf, 1, 0), (f(1e-6) + f(3e-6)) / 2);
  assert.equal(nodeEnergy(m, nf, 1, 2), (f(4e-6) + 0) / 2, 'face 1 has no record at step 2: it adds 0, and still counts');
  assert.equal(nodeEnergy(m, nf, 3, 2), 0);
  assert.equal(nodeEnergy(m, nf, 4, 0), 0, 'say NO: no linked face, no energy (not a division by 0)');
});

test('nodeEnergy reads a non-finite record as 0, as upstream does', () => {
  const m = map();
  m.values[2] = Number.POSITIVE_INFINITY;
  const nf = nodeFaces(m);
  // Face 1's only record is infinite: its sum is not finite, so it is not linked at all.
  assert.deepEqual([...nf.faces.subarray(nf.offsets[3], nf.offsets[4])], []);
  assert.equal(nodeEnergy(m, nf, 1, 0), Math.fround(1e-6));
  // Face 0 with one NaN record and one finite: still linked, the NaN read as 0.
  const m2 = map();
  m2.values[0] = Number.NaN;
  const nf2 = nodeFaces(m2);
  assert.deepEqual([...nf2.faces.subarray(nf2.offsets[0], nf2.offsets[1])], [0]);
  assert.equal(nodeEnergy(m2, nf2, 1, 0), (0 + Math.fround(3e-6)) / 2);
});

test('parseRange takes lo < hi in dB and says NO otherwise', () => {
  assert.deepEqual(parseRange('40', '70'), { ok: true, range: { lo: 40, hi: 70 } });
  assert.deepEqual(parseRange('-10.5', '0'), { ok: true, range: { lo: -10.5, hi: 0 } });
  const no = (lo: string, hi: string) => {
    const r = parseRange(lo, hi);
    assert.equal(r.ok, false, `${lo}..${hi}`);
    return r.ok ? '' : r.message;
  };
  assert.match(no('70', '40'), /low end must be below the high end/);
  assert.match(no('50', '50'), /low end must be below the high end/);
  assert.match(no('x', '50'), /not a number/);
  assert.match(no('40', ''), /not a number/);
  assert.match(no('-300', '0'), new RegExp(`at most ${MAX_RANGE_DB} dB`));
});

test('stepTime is the step\'s start time from the file\'s float32 step, as the transport prints it', () => {
  assert.equal(stepTime(12, 0.01), '120 ms');
  assert.equal(stepTime(3, 0.001), '3.0 ms');
  assert.equal(stepTime(5, null), 'step 5');
});

test('probeOf reads the face\'s own value from the file: its bits, its level to 0.1 dB, band and time', () => {
  const m = map();
  const p = probeOf(m, 1, 0, { what: 'Cutting planes', band: '1 kHz', dtS: 0.01, smooth: false });
  assert.equal(p.title, 'Cutting planes · 1 kHz · 0.0 ms');
  assert.equal(p.face, 1);
  assert.equal(p.bits, valueBits(Math.fround(3e-6)));
  assert.equal(p.level, '64.8 dB', '10 log10(3e-6 / 1e-12) = 64.77');
  assert.equal(p.value, 'file value 3.0000e-6');
  assert.equal(p.note, null);
  // say NO: a face with no record at the step shows no number.
  const none = probeOf(m, 1, 2, { what: 'Cutting planes', band: '1 kHz', dtS: 0.01, smooth: false });
  assert.equal(none.level, null);
  assert.equal(none.bits, null);
  assert.equal(none.value, 'No energy at this step');
  // A zero record is no energy too (no level for 0).
  assert.equal(probeOf(m, 2, 1, { what: 'w', band: 'b', dtS: 0.01, smooth: false }).level, null);
});

test('probeOf under smooth colour says the number is the face\'s own and the colours between are smoothed', () => {
  const p = probeOf(map(), 0, 0, { what: 'Surface receivers', band: 'all bands', dtS: 0.01, smooth: true });
  assert.match(p.note ?? '', /face's own value/);
  assert.match(p.note ?? '', /smoothed/);
});

test('probeOf on a difference: this run minus the baseline, with both levels; none where either has no energy', () => {
  const a = map();
  const b = map();
  b.values[2] = Math.fround(1.5e-6);
  const p = probeOf(a, 1, 0, { what: 'w', band: '1 kHz', dtS: 0.01, smooth: false, base: b });
  assert.equal(p.level, '+3.0 dB', '10 log10(3e-6 / 1.5e-6)');
  assert.equal(p.value, 'this run 64.8 dB, baseline 61.8 dB');
  const none = probeOf(a, 1, 2, { what: 'w', band: '1 kHz', dtS: 0.01, smooth: false, base: b });
  assert.equal(none.level, null);
  assert.equal(none.value, 'No energy in one of the runs at this step');
});

test('contourText names the spacing and what the lines follow; off is no text', () => {
  assert.deepEqual(CONTOUR_STEPS_DB, [1, 3, 6]);
  assert.equal(contourText(3), 'Contours every 3 dB, on the smoothed levels');
  assert.equal(contourText(0), null);
});

test('R42: the probe of a parameter map reads core’s value for the face, EDT with its range, or why it has none', () => {
  const m = { values: new Float32Array([0.95, Number.NaN]), offsets: new Uint32Array([0, 1, 2]) } as unknown as SurfaceMap;
  const p = { label: 'EDT', unit: 's', values: [0.712345, null], lo: [0.69, null], hi: [0.73, null], why: [null, 'edt_refused'], note: 'the range is its own' };
  const a = probeOfParam(m, 0, p, { what: 'Cutting planes', band: '1 kHz' });
  assert.equal(a.title, 'Cutting planes · EDT · 1 kHz');
  assert.equal(a.level, '0.71 s');
  assert.equal(a.value, "core's value 0.712345, range 0.69 s to 0.73 s");
  const b = probeOfParam(m, 1, p, { what: 'Cutting planes', band: '1 kHz' });
  assert.equal(b.level, null);
  assert.equal(b.value, 'Refused: edt_refused');
  const d = probeOfParam(m, 0, { ...p, unit: '', label: 'D50', values: [0.70321, null], lo: [], hi: [] }, { what: 'x', band: 'y' });
  assert.equal(d.level, '70.3 %');
});
