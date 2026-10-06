import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { Particles } from '../../resultsData.ts';
import { aliveCounts } from './particles.ts';
import { aliveAt, bounceRecords, copyJitter, particleAt, particleTable, recordOwners, recordTable } from './raysData.ts';

/** Two particles: one along x from step 2 (bouncing back at its third record), one still for one record at step 5. */
function sample(): Particles {
  const positions = new Float32Array([0, 0, 0, 1, 0, 0, 2, 0, 0, 1, 0, 0, 9, 9, 9]);
  return {
    particleCount: 2,
    recordCount: 5,
    maxSteps: 10,
    timeStepS: 0.001,
    bandHz: 1000,
    firstStep: new Uint32Array([2, 5]),
    offsets: new Uint32Array([0, 4, 5]),
    positions,
    energies: new Float32Array([1, 0.5, 0.25, 0.125, 2]),
  };
}

test('the tables the GPU reads: per particle first record, count and first step; per record x, y, z, energy and owner', () => {
  const p = sample();
  assert.deepEqual([...particleTable(p)], [0, 4, 2, 0, 4, 1, 5, 0]);
  assert.deepEqual([...recordTable(p).subarray(4, 8)], [1, 0, 0, 0.5]);
  assert.deepEqual([...recordOwners(p)], [0, 0, 0, 0, 1]);
});

test('a particle at a fractional step is its records interpolated, exactly the record at a whole step, absent outside its life', () => {
  const p = sample();
  assert.equal(particleAt(p, 0, 1.9), null);
  assert.deepEqual(particleAt(p, 0, 2), [0, 0, 0, 1]);
  assert.deepEqual(particleAt(p, 0, 3.5), [1.5, 0, 0, 0.375]);
  assert.deepEqual(particleAt(p, 0, 5), [1, 0, 0, 0.125]);
  assert.equal(particleAt(p, 0, 5.01), null);
  assert.deepEqual(particleAt(p, 1, 5), [9, 9, 9, 2]);
  assert.equal(particleAt(p, 1, 5.5), null);
  // At whole steps the alive rule is the file's (aliveCounts, the dots' count).
  const alive = aliveCounts(p, 10);
  for (let s = 0; s < 10; s++) assert.equal(aliveAt(p, s), alive[s], `step ${s}`);
});

test('the bounce points: where the path turns, not where it runs straight', () => {
  assert.deepEqual([...bounceRecords(sample())], [2]);
});

test("the benchmark's copies: copy 0 is the file's own particle, every other a fixed offset within the scale", () => {
  assert.deepEqual(copyJitter(0, 1), [0, 0, 0]);
  const a = copyJitter(7, 0.6);
  assert.deepEqual(copyJitter(7, 0.6), a);
  for (const v of a) assert.ok(Math.abs(v) <= 0.3);
  assert.notDeepEqual(copyJitter(8, 0.6), a);
});
