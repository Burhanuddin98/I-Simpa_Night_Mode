import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { Particles } from '../../resultsData.ts';
import { advance, type AnimatorState } from './animator.ts';
import { aliveCounts, noParticlesText, recordSteps } from './particles.ts';

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
  const s: AnimatorState = { step: 0, steps: 100, playing: true, stepsPerSecond: 50, carry: 0 };
  const a = advance(s, 100);
  assert.equal(a.step, 5);
  const b = advance({ ...a }, 10);
  assert.equal(b.step, 5, 'half a step carried, not dropped');
  assert.equal(advance(b, 10).step, 6);
  const end = advance({ ...s, step: 98 }, 1000);
  assert.equal(end.step, 99);
  assert.equal(end.playing, false, 'playback stops at the last step');
  assert.deepEqual(advance({ ...s, playing: false }, 1000), { ...s, playing: false }, 'paused: nothing moves');
});
