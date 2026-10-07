import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { PART_MAGIC } from '../../resultsData.ts';
import { decodeLiveBatch, LIVE_MAGIC, LIVE_SPEED, liveCaption, LiveRun, LiveSet } from './live.ts';

/** A LIVE v1 batch as live.rs encodes it: per particle its first step and record count; record k of particle i at (i, k, 0), energy 1/(k+1). */
function batch(spec: [number, number][], o: { soFar?: number; total?: number; bandHz?: number; bandIndex?: number; bands?: number; dt?: number } = {}): ArrayBuffer {
  const np = spec.length;
  const nr = spec.reduce((a, [, n]) => a + n, 0);
  const buf = new ArrayBuffer(32 + 32 + 4 * np + 4 * (np + 1) + 16 * nr);
  const v = new DataView(buf);
  [LIVE_MAGIC, 1, o.soFar ?? np, o.total ?? 800, o.bandIndex ?? 0, o.bands ?? 2, 0, 0].forEach((x, i) => v.setUint32(4 * i, x, true));
  const p = 32;
  [PART_MAGIC, 1, np, nr, 10_000].forEach((x, i) => v.setUint32(p + 4 * i, x, true));
  v.setFloat32(p + 20, o.dt ?? 0.001, true);
  v.setInt32(p + 24, o.bandHz ?? 1000, true);
  let at = p + 32;
  for (const [first] of spec) (v.setUint32(at, first, true), (at += 4));
  let off = 0;
  v.setUint32(at, 0, true);
  at += 4;
  for (const [, n] of spec) (v.setUint32(at, (off += n), true), (at += 4));
  const pos = at;
  const en = pos + 12 * nr;
  let r = 0;
  spec.forEach(([, n], i) => {
    for (let k = 0; k < n; k++, r++) {
      v.setFloat32(pos + 12 * r, i, true);
      v.setFloat32(pos + 12 * r + 4, k, true);
      v.setFloat32(pos + 12 * r + 8, 0, true);
      v.setFloat32(en + 4 * r, 1 / (k + 1), true);
    }
  });
  return buf;
}

test('a batch decodes: the envelope, then the PART the Results step reads', () => {
  const b = decodeLiveBatch(batch([[3, 4], [0, 2]], { soFar: 2, total: 800, bandHz: 500, bandIndex: 1, bands: 2 }));
  assert.deepEqual([b.soFar, b.total, b.bandIndex, b.bands], [2, 800, 1, 2]);
  assert.equal(b.particles.bandHz, 500);
  assert.equal(b.particles.particleCount, 2);
  assert.equal(b.particles.recordCount, 6);
  assert.deepEqual([...b.particles.firstStep], [3, 0]);
  assert.deepEqual([...b.particles.offsets], [0, 4, 6]);
  assert.equal(b.particles.energies[3], 0.25);
  const bad = batch([[0, 1]]);
  new DataView(bad).setUint32(0, 0, true);
  assert.throws(() => decodeLiveBatch(bad), /not "LIVE"/);
  assert.throws(() => decodeLiveBatch(new ArrayBuffer(8)), /shorter/);
});

test('the clock starts at the first batch, at its earliest first step, and runs at the live speed', () => {
  const s = new LiveSet();
  assert.equal(s.clock(0), null);
  s.add(decodeLiveBatch(batch([[5, 3], [2, 3]], { dt: 0.001 })), 1000);
  assert.equal(s.clock(1000), 2);
  // 1 s later at 0.05 of real time and 1 ms steps: 50 steps on
  assert.equal(s.clock(2000), 2 + LIVE_SPEED * 1000);
  assert.equal(s.clock(500), 2, 'never before its start');
  // a 2 ms step halves the rate
  const t = new LiveSet();
  t.add(decodeLiveBatch(batch([[0, 1]], { dt: 0.002 })), 0);
  assert.equal(t.clock(1000), (LIVE_SPEED * 1000) / 2);
});

test('a trajectory that arrives late starts from its first record at the clock; an early one keeps its own step', () => {
  const s = new LiveSet(0.05);
  s.add(decodeLiveBatch(batch([[0, 2]])), 0);
  // 1 s later the clock is at step 50: a particle whose first step is 0 starts at 50, one at 80 keeps 80
  s.add(decodeLiveBatch(batch([[0, 3], [80, 2]], { soFar: 3 })), 1000);
  const p = s.particles();
  assert.ok(p);
  assert.deepEqual([...p.firstStep], [0, 50, 80]);
  assert.deepEqual([...p.offsets], [0, 2, 5, 7]);
  // the records themselves are untouched: particle 1's second record is the batch's record (0, 1, 0)
  assert.deepEqual([...p.positions.subarray(9, 12)], [0, 1, 0]);
  assert.equal(p.energies[3], 0.5);
  assert.ok(p.maxSteps >= 82);
});

test('past the record budget the oldest arrivals are let go, never the newest', () => {
  const s = new LiveSet(0.05, 10);
  s.add(decodeLiveBatch(batch([[0, 4], [0, 4]])), 0);
  s.add(decodeLiveBatch(batch([[0, 4]], { soFar: 3 })), 10);
  const sum = s.summary();
  assert.ok(sum);
  assert.equal(sum.held, 2);
  assert.equal(sum.records, 8);
  assert.equal(sum.dropped, 1);
  assert.equal(sum.soFar, 3);
  const p = s.particles();
  assert.ok(p);
  assert.equal(p.particleCount, 2);
  // a single trajectory over the budget is still drawn
  const t = new LiveSet(0.05, 2);
  t.add(decodeLiveBatch(batch([[0, 5]])), 0);
  assert.equal(t.particles()?.recordCount, 5);
});

test('the caption says it is the saved sample, how many of how many, and the band', () => {
  assert.equal(liveCaption(null), 'live: waiting for the first saved particles');
  assert.equal(liveCaption({ soFar: 1200, total: 10800, bandHz: 1000 }), 'live: the saved sample of particles, 1,200 of 10,800 so far, band 1000 Hz');
  const s = new LiveSet();
  s.add(decodeLiveBatch(batch([[0, 1]], { soFar: 400, total: 400, bandHz: 125 })), 0);
  assert.equal(liveCaption(s.summary()), 'live: the saved sample of particles, 400 of 400 so far, band 125 Hz');
  assert.notEqual(s.revision(), new LiveSet().revision());
});

test('a run is kept while the Simulate step is away, drawn when it is back, and its end clears it on any step', () => {
  const r = new LiveRun(7);
  // on the Simulate step: the first batch is drawn
  assert.equal(r.accept(7, decodeLiveBatch(batch([[0, 3]], { soFar: 1 })), 0), true);
  assert.equal(r.wantsBuild(true), true);
  r.built();
  assert.equal(r.wantsBuild(true), false, 'nothing new');
  // the user goes to Results: batches are still kept, nothing is rebuilt while hidden
  assert.equal(r.accept(7, decodeLiveBatch(batch([[0, 2], [0, 2]], { soFar: 3, bandHz: 500 })), 1000), true);
  assert.equal(r.accept(7, decodeLiveBatch(batch([[0, 4]], { soFar: 4, bandHz: 500 })), 2000), true);
  assert.equal(r.wantsBuild(false), false, 'no rebuild off the Simulate step');
  // back on Simulate: one build draws everything that arrived, at the clock's now
  assert.equal(r.wantsBuild(true), true);
  const p = r.set.particles();
  assert.equal(p?.particleCount, 4);
  assert.equal(r.set.summary()?.soFar, 4);
  assert.equal(r.set.summary()?.bandHz, 500);
  assert.equal(p?.firstStep[3], Math.floor(r.set.clock(2000) ?? -1), 'the late one starts at the clock');
  r.built();
  // another run's batch and another run's end are not this run's
  assert.equal(r.accept(8, decodeLiveBatch(batch([[0, 1]])), 2100), false);
  assert.equal(r.end(8), false);
  assert.equal(r.ended(), false);
  // the end clears it whatever is shown, once; nothing is kept or built after it
  assert.equal(r.end(7), true);
  assert.equal(r.end(7), false);
  assert.equal(r.ended(), true);
  assert.equal(r.accept(7, decodeLiveBatch(batch([[0, 1]], { soFar: 5 })), 2200), false);
  assert.equal(r.wantsBuild(true), false);
  assert.equal(r.set.summary()?.soFar, 4);
  // a look change wants a build even with nothing new (while live)
  const q = new LiveRun(1);
  q.accept(1, decodeLiveBatch(batch([[0, 1]])), 0);
  q.built();
  q.invalidate();
  assert.equal(q.wantsBuild(true), true);
});
