import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { SurfaceMap } from '../../resultsData.ts';
import { cumulativeAt, cumulativeRange, cumulativeValues, CUMULATIVE_HINT, CUMULATIVE_NOTE, cumulativeRefusal } from './cumulative.ts';
import { mapLayout, type MapLayout } from './mapData.ts';

/**
 * Face 0: records at steps 2, 0 (out of order), 2 again (added into its step, as upstream's `+=`);
 * face 1: a NaN at step 1 (read as 0) and 1e-6 at step 3; face 2: no records.
 */
function map(): SurfaceMap {
  const values = new Float32Array([4e-6, 1e-6, 3e-7, NaN, 1e-6]);
  return {
    nodeCount: 5,
    faceCount: 3,
    recordCount: values.length,
    timeStepCount: 5,
    timeStepS: 0.01,
    recordType: 0,
    positions: new Float32Array(15),
    indices: new Uint32Array([0, 1, 2, 1, 3, 2, 2, 3, 4]),
    receivers: new Uint32Array([0, 0, 0]),
    offsets: new Uint32Array([0, 3, 5, 5]),
    steps: new Uint32Array([2, 0, 2, 1, 3]),
    values,
  };
}

const f = Math.fround;

/** Upstream's rule written out step by step in float32 (Recepteurs_surfacique.cpp:337-387). */
function upstream(m: SurfaceMap): number[][] {
  const out: number[][] = [];
  for (let face = 0; face < m.faceCount; face++) {
    const e = new Array<number>(m.timeStepCount).fill(0);
    for (let k = m.offsets[face]; k < m.offsets[face + 1]; k++) {
      const v = Number.isFinite(m.values[k]) ? m.values[k] : 0;
      e[m.steps[k]] = f(e[m.steps[k]] + v);
    }
    for (let t = 1; t < e.length; t++) e[t] = f(e[t] + e[t - 1]);
    out.push(e);
  }
  return out;
}

test('the cumulative texture is upstream\'s float32 running sum, face-major like the map\'s', () => {
  const m = map();
  const l = mapLayout(m.faceCount, m.timeStepCount, 4) as MapLayout;
  const tex = cumulativeValues(m, l);
  const want = upstream(m);
  assert.equal(tex.length, l.width * l.height);
  for (let face = 0; face < m.faceCount; face++) {
    for (let s = 0; s < m.timeStepCount; s++) {
      assert.equal(tex[face * m.timeStepCount + s], want[face][s], `face ${face} step ${s}`);
      assert.equal(cumulativeAt(m, face, s), want[face][s], `probe of face ${face} step ${s}`);
    }
  }
  // Two records at one step add into it, the running sum holds after the last record.
  assert.equal(tex[2], f(f(4e-6 + 1e-6) + f(3e-7)));
  assert.equal(tex[4], tex[2]);
  // say NO: a NaN record is 0, not a NaN that would blank the face for every later step.
  assert.equal(tex[1 * 5 + 1], 0);
  assert.equal(tex[1 * 5 + 4], f(1e-6));
  // A face with no records stays 0: no colour, as the instantaneous map's empty cells.
  assert.deepEqual([...tex.subarray(10, 15)], [0, 0, 0, 0, 0]);
});

test('the range is the whole dB around every cumulative level, 60 dB deep at most', () => {
  const r = cumulativeRange(map());
  // Smallest positive running sum 1e-6 (60 dB), largest 5.3e-6 (67.24 dB).
  assert.deepEqual(r, { lo: 60, hi: 68 });
  // say NO: a map with no energy at all has no range.
  const empty = { ...map(), values: new Float32Array(5) };
  assert.equal(cumulativeRange(empty), null);
});

test('no cumulative map of a difference; the hint says it builds and holds', () => {
  assert.equal(cumulativeRefusal('level'), null);
  assert.match(cumulativeRefusal('diff') ?? '', /difference/);
  assert.match(CUMULATIVE_HINT, /builds up and holds/);
  assert.match(CUMULATIVE_HINT, /instantaneous/);
  assert.match(CUMULATIVE_NOTE, /builds up and holds/);
});

test('the probe of a cumulative map shows the running sum and says so; no energy yet, no number', async () => {
  const { probeOf, valueBits } = await import('./mapView.ts');
  const m = map();
  const p = probeOf(m, 0, 3, { what: 'Surface receivers', band: '1 kHz', dtS: 0.01, smooth: false, cumulative: true });
  const want = cumulativeAt(m, 0, 3);
  assert.equal(p.bits, valueBits(want));
  assert.equal(p.level, `${(10 * Math.log10(want) + 120).toFixed(1)} dB`);
  assert.match(p.value, /summed from the first step/);
  // Face 0 has no record at step 3: the instantaneous probe has no number there, the cumulative one has.
  assert.equal(probeOf(m, 0, 3, { what: 'S', band: 'b', dtS: 0.01, smooth: false }).level, null);
  // say NO: face 1 before its first energy (step 0) and face 2 ever: no number.
  const early = probeOf(m, 1, 0, { what: 'S', band: 'b', dtS: 0.01, smooth: false, cumulative: true });
  assert.equal(early.level, null);
  assert.equal(early.bits, null);
  assert.equal(probeOf(m, 2, 4, { what: 'S', band: 'b', dtS: 0.01, smooth: false, cumulative: true }).level, null);
});
