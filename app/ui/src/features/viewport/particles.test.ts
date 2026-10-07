import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { Particles } from '../../resultsData.ts';
import { advance, type AnimatorState } from './animator.ts';
import {
  aliveCounts,
  emissionStep,
  noParticlesText,
  keptRecord,
  keptTrail,
  NUMBER_OPS,
  rampPlace,
  recordSteps,
  TRAIL_BUDGET_BYTES,
  TRAIL_BYTES_PER_SEGMENT,
  TRAIL_HINT,
  TRAIL_NOTE,
  TRAIL_LENGTHS,
  trailAlpha,
  trailCount,
  trailRefusal,
  trailSegments,
} from './particles.ts';

/** A decoded PART: per particle its first step and its record count. */
function particles(spec: [number, number][], maxSteps = 10): Particles {
  const offsets = new Uint32Array(spec.length + 1);
  spec.forEach(([, n], i) => (offsets[i + 1] = offsets[i] + n));
  const nr = offsets[spec.length];
  return {
    particleCount: spec.length,
    recordCount: nr,
    maxSteps,
    timeStepS: Math.fround(0.01),
    bandHz: 1000,
    firstStep: new Uint32Array(spec.map(([f]) => f)),
    offsets,
    positions: new Float32Array(3 * nr),
    energies: new Float32Array(nr).fill(1),
  };
}

test('a particle is alive at the steps it has a record for: first step + k', () => {
  const p = particles([
    [0, 3],
    [2, 2],
    [2, 0],
    [7, 3],
  ]);
  assert.deepEqual([...recordSteps(p)], [0, 1, 2, 2, 3, 7, 8, 9]);
  assert.deepEqual([...aliveCounts(p, 10)], [1, 1, 2, 1, 0, 0, 0, 1, 1, 1]);
  // Records past the run's last step are not counted at a step that does not exist.
  assert.deepEqual([...aliveCounts(p, 8)], [1, 1, 2, 1, 0, 0, 0, 1]);
});

test('no particles saved: what it says, and how to turn it on with the file size', () => {
  const t = noParticlesText(10_000, 1);
  assert.equal(t.title, 'No particles saved for this run');
  assert.match(t.how, /Simulate/);
  assert.match(t.how, /Particles saved for playback/);
  // 1,000 particles x 10,000 steps x 16 bytes x 1 source = 160 MB a band (upstream's formula).
  assert.match(t.how, /1,000 .*160 MB/);
  assert.match(noParticlesText(100, 3).how, /4\.8 MB/);
});

test('the shared timeline: steps advance at its rate, stop at the end, and a step is set in range', () => {
  // 0.1x of real time at a 1 ms step: 100 steps a second (more than one a frame: whole steps taken).
  const s: AnimatorState = { step: 0, steps: 100, playing: true, speed: 0.1, dtMs: 1, carry: 0, start: 0 };
  const a = advance(s, 50);
  assert.equal(a.step, 5);
  const b = advance({ ...a }, 5);
  assert.equal(b.step, 5, 'half a step carried, not dropped');
  assert.equal(advance(b, 5).step, 6);
  const end = advance({ ...s, step: 98 }, 100);
  assert.equal(end.step, 99);
  assert.equal(end.playing, false, 'playback stops at the last step');
  assert.deepEqual(advance({ ...s, playing: false }, 1000), { ...s, playing: false }, 'paused: nothing moves');
});

test('the emission step is the first step any saved particle is alive', () => {
  assert.equal(emissionStep(particles([[3, 2], [5, 1]])), 3);
  // A particle with no record is not alive anywhere: it does not set the emission.
  assert.equal(emissionStep(particles([[0, 0], [4, 2]])), 4);
  assert.equal(emissionStep(particles([])), 0, 'no particles: step 0');
});

test('count mode (gate (d)) and the draw keep the same records: keptRecord(), run as JavaScript, keeps exactly the particles aliveCounts counts', () => {
  // Assay (M12, LOW): a cull the draw made outside the shared gate would not be counted. The draw and
  // the count mode both build their gate from `keptRecord` (resultsLayer.ts), whose only inputs are the
  // record's step and the timeline's: energy is colour, never a cull.
  assert.equal(keptRecord.length, 3, 'keptRecord takes the ops, the record step and the step: no energy');
  const p = particles([
    [0, 30],
    [3, 12],
    [20, 9],
    [7, 0],
  ]);
  const steps = recordSteps(p);
  const alive = aliveCounts(p, 40);
  for (let s = 0; s < 40; s++) {
    let c = 0;
    for (let k = 0; k < p.recordCount; k++) if (keptRecord(NUMBER_OPS, steps[k], s)) c++;
    assert.equal(c, alive[s], `step ${s}`);
  }
});

test("the playback's colour and the trails' fade: the hot ramp's top 45 % over 40 dB, older segments fainter", () => {
  assert.equal(rampPlace(1, 0), 1);
  assert.ok(Math.abs(rampPlace(1e-2, 0) - (0.55 + 0.45 * 0.5)) < 1e-12, '20 dB down is half way');
  assert.equal(rampPlace(1e-5, 0), 0.55, '40 dB down and below: the floor');
  assert.equal(rampPlace(0, 0), 0.55, 'no energy: the floor, still drawn');
  assert.equal(trailAlpha(10, 10, 5), 1);
  assert.ok(Math.abs(trailAlpha(10, 5, 5) - 0.15) < 1e-12);
  assert.ok(trailAlpha(10, 8, 5) > trailAlpha(10, 6, 5));
});

test("W3 trails: one segment per pair of consecutive records, tagged with its head step and the particle's last step", () => {
  const p = particles([
    [0, 3],
    [2, 2],
    [2, 0],
    [7, 1],
  ]);
  for (let k = 0; k < p.recordCount; k++) p.positions.set([k, 10 * k, 100 * k], 3 * k);
  p.energies.set([1, 2, 3, 4, 5, 6]);
  const t = trailSegments(p);
  // Particle 0: (0,1), (1,2); particle 1: (3,4); particle 2 none; particle 3 one record, no segment.
  assert.equal(t.segments, 3);
  assert.deepEqual([...t.positions.subarray(0, 6)], [0, 0, 0, 1, 10, 100]);
  assert.deepEqual([...t.positions.subarray(12, 18)], [3, 30, 300, 4, 40, 400]);
  // Both ends of a segment carry the same tags, so the shader keeps or drops it whole.
  assert.deepEqual([...t.head], [1, 1, 2, 2, 3, 3]);
  assert.deepEqual([...t.last], [2, 2, 2, 2, 3, 3]);
  // The head's energy colours the segment, at both ends.
  assert.deepEqual([...t.energy], [2, 2, 3, 3, 5, 5]);
});

test("W3 trails: the count a step keeps is each live particle's last N steps; a dead particle leaves none", () => {
  const p = particles([
    [0, 3],
    [2, 2],
    [2, 0],
    [7, 3],
  ]);
  const brute = (step: number, n: number) => {
    const t = trailSegments(p);
    let c = 0;
    for (let i = 0; i < t.segments; i++) {
      const h = t.head[2 * i];
      if (t.last[2 * i] >= step && h <= step && h > step - n) c++;
    }
    return c;
  };
  for (const n of TRAIL_LENGTHS) for (let s = 0; s < 10; s++) assert.equal(trailCount(p, s, n), brute(s, n), `step ${s}, ${n} steps`);
  assert.equal(trailCount(p, 2, 1), 1, "upstream's ray: one segment for each live particle with a step before (particle 1 starts at 2)");
  assert.equal(trailCount(p, 2, 5), 2);
  assert.equal(trailCount(p, 3, 5), 1);
  // say NO: particle 0 died after step 2; at step 4 it draws nothing, though its segments are recent.
  assert.equal(trailCount(p, 4, 60), 0);
  assert.equal(trailCount(p, 9, 60), 2);
});

test('W3 trails: refused without particles and above the GPU budget, with the size', () => {
  assert.match(trailRefusal(null) ?? '', /No particles saved/);
  const small = particles([[0, 3]]);
  assert.equal(trailRefusal(small), null);
  const big = { ...small, recordCount: Math.ceil(TRAIL_BUDGET_BYTES / TRAIL_BYTES_PER_SEGMENT) + 2 };
  assert.match(trailRefusal(big) ?? '', /MB/);
  assert.match(TRAIL_HINT, /straight lines/);
  assert.match(TRAIL_NOTE, /Straight lines between saved positions/);
  assert.deepEqual(TRAIL_LENGTHS, [1, 5, 20, 60]);
});

test("W3 trails: keptTrail(), the rule the GPU runs, run as JavaScript, keeps exactly the segments trailCount counts", () => {
  // The e2e caught the window one step too long (6 heads for 5 steps): the rule itself is held here.
  assert.equal(keptTrail.length, 5, 'keptTrail takes the ops, last, head, step and length: no energy');
  const kept = (l: number, h: number, s: number, n: number) => keptTrail(NUMBER_OPS, l, h, s, n);
  const p = particles([
    [0, 30],
    [3, 12],
    [20, 9],
  ]);
  const t = trailSegments(p);
  for (const n of TRAIL_LENGTHS) {
    for (let s = 0; s < 40; s++) {
      let c = 0;
      for (let i = 0; i < t.segments; i++) if (kept(t.last[2 * i], t.head[2 * i], s, n)) c++;
      assert.equal(c, trailCount(p, s, n), `${n} steps at step ${s}`);
    }
  }
});
