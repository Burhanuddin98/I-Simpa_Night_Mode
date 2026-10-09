import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { SurfaceMap } from '../../resultsData.ts';
import {
  COOL,
  denseValues,
  diffRange,
  HOT,
  legendLabels,
  levelDb,
  levelRange,
  mapQuantities,
  paramLabels,
  paramRange,
  mapLayout,
  rampColor,
  surfaceMismatch,
  texelOf,
} from './mapData.ts';

/** A decoded SMAP built by hand: faces with their records, every value a float32. */
function map(faces: [number, number][][], steps = 5, nodes = [0, 0, 0, 1, 0, 0, 0, 1, 0]): SurfaceMap {
  const offsets = new Uint32Array(faces.length + 1);
  const st: number[] = [];
  const vals: number[] = [];
  faces.forEach((recs, i) => {
    for (const [s, v] of recs) {
      st.push(s);
      vals.push(v);
    }
    offsets[i + 1] = st.length;
  });
  return {
    nodeCount: nodes.length / 3,
    faceCount: faces.length,
    recordCount: st.length,
    timeStepCount: steps,
    timeStepS: Math.fround(0.01),
    recordType: 0,
    positions: new Float32Array(nodes),
    indices: new Uint32Array(faces.flatMap(() => [0, 1, 2])),
    receivers: new Uint32Array(faces.length),
    offsets,
    steps: new Uint32Array(st),
    values: new Float32Array(vals),
  };
}

const bits = (x: number) => new Uint32Array(new Float32Array([x]).buffer)[0];

test('the layout is faces x steps, face-major, wrapped at the texture size limit', () => {
  const l = mapLayout(3, 4, 16);
  assert.ok(!('error' in l));
  assert.deepEqual([l.width, l.height], [12, 1]);
  assert.deepEqual(texelOf(l, 2, 3), { x: 11, y: 0 });
  const w = mapLayout(10, 7, 16);
  assert.ok(!('error' in w));
  // 70 texels at 16 a row: 5 rows, face 9 step 6 is texel 69.
  assert.deepEqual([w.width, w.height], [16, 5]);
  assert.deepEqual(texelOf(w, 9, 6), { x: 5, y: 4 });
  const big = mapLayout(1000, 1000, 16);
  assert.ok('error' in big, 'a map past the texture limit is refused, not cut');
});

test('dense values carry every record bit for bit, NaN payloads included, and 0 elsewhere', () => {
  const m = map([[[0, 0.1], [3, 1e-30]], [], [[4, 2.5e-7]]], 5);
  // A signalling NaN with a payload: written through the bits, it must come back the same.
  new Uint32Array(m.values.buffer, m.values.byteOffset, m.values.length)[1] = 0x7fa00001;
  const l = mapLayout(m.faceCount, m.timeStepCount, 4096);
  assert.ok(!('error' in l));
  const d = denseValues(m, l);
  assert.equal(d.length, l.width * l.height);
  const u = new Uint32Array(d.buffer);
  const at = (f: number, s: number) => {
    const t = texelOf(l, f, s);
    return u[t.y * l.width + t.x];
  };
  assert.equal(at(0, 0), bits(0.1));
  assert.equal(at(0, 3), 0x7fa00001);
  assert.equal(at(2, 4), bits(2.5e-7));
  assert.equal(at(1, 2), 0);
  assert.equal(at(0, 1), 0);
});

test('levels are 10 log10(E / 1e-12) as upstream draws them; no level for 0, negative or not finite', () => {
  assert.equal(levelDb(1e-12), 0);
  assert.ok(Math.abs((levelDb(1e-6) ?? NaN) - 60) < 1e-9);
  assert.equal(levelDb(0), null);
  assert.equal(levelDb(-1), null);
  assert.equal(levelDb(Number.NaN), null);
  assert.equal(levelDb(Infinity), null);
});

test('the level range is whole dB around every record, at most 60 dB deep, and none without energy', () => {
  // 1e-6 is 60 dB, 1e-9 is 30 dB, 3e-6 is 64.77 dB.
  const r = levelRange(map([[[0, 1e-6]], [[1, 1e-9]], [[2, 3e-6], [3, 0]]]));
  assert.deepEqual(r, { lo: 30, hi: 65 });
  const deep = levelRange(map([[[0, 1e-3]], [[1, 1e-15]]]));
  assert.deepEqual(deep, { lo: 30, hi: 90 }, '-30 dB is cut at 60 below the top');
  assert.equal(levelRange(map([[[0, 0]], []])), null);
});

test('the difference range is symmetric, at least 1 dB, over the cells both maps hold energy in', () => {
  const a = map([[[0, 1e-6], [1, 1e-6]], [[0, 1e-6]]]);
  const b = map([[[0, 2e-6]], [[0, 1e-7], [1, 1e-6]]]);
  // Face 1 step 0: 60 dB against 50 dB, 10 dB; face 0 step 0: -3.01 dB; one-sided cells are out.
  assert.deepEqual(diffRange(a, b), { lo: -10, hi: 10 });
  assert.deepEqual(diffRange(a, a), { lo: -1, hi: 1 });
});

test('a baseline must be the same surface: nodes, faces and steps', () => {
  const a = map([[[0, 1]], [[0, 1]]]);
  assert.equal(surfaceMismatch(a, map([[[1, 2]], [[2, 3]]])), null, 'other values, same surface');
  assert.match(surfaceMismatch(a, map([[[0, 1]]])) ?? '', /faces/);
  assert.match(surfaceMismatch(a, map([[[0, 1]], [[0, 1]]], 6)) ?? '', /steps/);
  assert.match(surfaceMismatch(a, map([[[0, 1]], [[0, 1]]], 5, [0, 0, 0, 1, 0, 0, 0, 2, 0])) ?? '', /nodes/);
  // R52: the same grid in time and the same kind of record, or no difference.
  assert.match(surfaceMismatch(a, { ...map([[[0, 1]], [[0, 1]]]), timeStepS: Math.fround(0.02) }) ?? '', /steps are 20 ms/);
  assert.match(surfaceMismatch(a, { ...map([[[0, 1]], [[0, 1]]]), recordType: 1 }) ?? '', /record type/);
});

test("R52: the difference shown is this run's level minus the baseline's, cell by cell, as the probe reads it", () => {
  // Two maps of the same surface; the probe (mapView.probeOf) and the shader take levelDb(a) - levelDb(b).
  const a = map([[[0, 1e-6], [2, 4e-7]], [[1, 2e-8]]]);
  const b = map([[[0, 5e-7], [2, 4e-7]], [[1, 2e-9]]]);
  assert.equal(surfaceMismatch(a, b), null);
  const cells: [number, number, number][] = [[0, 0, 10 * Math.log10(2)], [0, 2, 0], [1, 1, 10]];
  for (const [f, st, want] of cells) {
    const rec = (m: SurfaceMap) => {
      for (let k = m.offsets[f]; k < m.offsets[f + 1]; k++) if (m.steps[k] === st) return m.values[k];
      return 0;
    };
    const d = (levelDb(rec(a)) as number) - (levelDb(rec(b)) as number);
    assert.ok(Math.abs(d - want) < 1e-5, `face ${f} step ${st}: ${d} against ${want}`);
  }
  assert.deepEqual(diffRange(a, b), { lo: -10, hi: 10 });
});

test('the ramps are the design’s: black through red to yellow, and its cool ramp for quieter', () => {
  assert.deepEqual(rampColor(HOT, 0), [0x0b, 0x0b, 0x0e]);
  assert.deepEqual(rampColor(HOT, 1), [0xff, 0xf7, 0xf2]);
  assert.deepEqual(rampColor(HOT, 0.5), [0xd8, 0x1f, 0x2d]);
  assert.deepEqual(rampColor(HOT, -3), rampColor(HOT, 0));
  assert.deepEqual(rampColor(COOL, 1), [0xa3, 0xcd, 0xe3]);
});

test('legend labels: whole dB, the middle to a tenth when it falls between', () => {
  assert.deepEqual(legendLabels({ lo: 30, hi: 65 }, 'level'), { lo: '30', mid: '47.5', hi: '65 dB' });
  assert.deepEqual(legendLabels({ lo: 40, hi: 80 }, 'level'), { lo: '40', mid: '60', hi: '80 dB' });
  assert.deepEqual(legendLabels({ lo: -10, hi: 10 }, 'diff'), { lo: '−10', mid: '0', hi: '+10 dB' });
});

test('decision 63: each level stop is lighter than the last by an even step (CIELAB L*)', () => {
  const lin = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  const lstar = (h: string) => {
    const [r, g, b] = [1, 3, 5].map((i) => lin(parseInt(h.slice(i, i + 2), 16) / 255));
    const y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    return 116 * (y > 0.008856 ? Math.cbrt(y) : 7.787 * y + 16 / 116) - 16;
  };
  const l = HOT.map(lstar);
  for (let i = 1; i < l.length; i++) {
    const step = l[i] - l[i - 1];
    assert.ok(step >= 8 && step <= 14, `stop ${i}: L* ${l[i - 1].toFixed(1)} -> ${l[i].toFixed(1)}`);
  }
  assert.ok(l[l.length - 1] > 95, 'the loudest is near white');
});

test('R42/R73: a parameter map is drawn over its own range in its unit, in whole steps, labelled in that unit', () => {
  assert.deepEqual(paramRange(0.912, 1.011, 's'), { lo: 0.9, hi: 1.05 });
  assert.deepEqual(paramRange(6.2, 7.22, 'dB'), { lo: 6, hi: 7.5 });
  assert.deepEqual(paramRange(0.663, 0.713, ''), { lo: 0.65, hi: 0.75 });
  assert.deepEqual(paramRange(1, 1, 's'), { lo: 1, hi: 1.05 }, 'at least one step wide');
  assert.equal(paramRange(null, null, 's'), null);
  assert.deepEqual(paramLabels({ lo: 0.9, hi: 1.05 }, 's'), { lo: '0.90', mid: '0.98', hi: '1.05 s' });
  assert.deepEqual(paramLabels({ lo: -2, hi: 7.5 }, 'dB'), { lo: '−2.0', mid: '2.8', hi: '7.5 dB' });
  assert.deepEqual(paramLabels({ lo: 0.65, hi: 0.75 }, ''), { lo: '65', mid: '70', hi: '75 %' });
});

test('R42: a FAIL planted on one map bed withholds that map only, with the reason; no bed withholds all', () => {
  const pass = { status: 'PASS', reasons: [] };
  for (const key of ['t30_s', 'edt_s', 'c80_db', 'd50']) {
    const bed: Record<string, { status: string; reasons: string[] }> = { map_t30_s: pass, map_edt_s: pass, map_c80_db: pass, map_d50: pass };
    bed[`map_${key}`] = { status: 'FAIL', reasons: [`map_${key}: a face did not hold`] };
    const q = mapQuantities(bed);
    assert.deepEqual(q.filter((x) => x.withheld).map((x) => [x.key, x.withheld]), [[key, `its test bed has not passed: map_${key}: a face did not hold`]]);
    assert.equal(q.filter((x) => !x.withheld).length, 3, key);
  }
  assert.ok(mapQuantities(undefined).every((x) => x.withheld === 'its test bed has not passed'));
  assert.ok(mapQuantities({ map_t30_s: { status: 'pass' } }).every((x) => x.withheld), 'only PASS shows');
});
