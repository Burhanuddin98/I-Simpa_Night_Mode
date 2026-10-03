// Particle playback's data, before the GPU (M12 P3: R53; MQ4). Pure, tested by particles.test.ts.
//
// A `.pbin` particle holds one record per step from its first step on (`docs/formats/pbin.md`):
// record k of particle i is at step firstStep[i] + k. It is alive at exactly those steps, so the
// particles alive at a step are the records at that step. The playback shader draws a record
// only at its own step; gate (d) counts what that draw lets through against `aliveCounts`.
import type { Particles } from '../../resultsData.ts';
import { pbinBytes, sizeText } from '../simulate/settings.ts';

/** Each record's step, as a float the vertex shader compares with the timeline's step. */
export function recordSteps(p: Particles): Float32Array {
  const out = new Float32Array(p.recordCount);
  for (let i = 0; i < p.particleCount; i++) {
    for (let k = p.offsets[i]; k < p.offsets[i + 1]; k++) out[k] = p.firstStep[i] + (k - p.offsets[i]);
  }
  return out;
}

/** The particles alive at each of the run's `steps` steps. */
export function aliveCounts(p: Particles, steps: number): Uint32Array {
  const out = new Uint32Array(steps);
  for (let i = 0; i < p.particleCount; i++) {
    for (let k = p.offsets[i]; k < p.offsets[i + 1]; k++) {
      const s = p.firstStep[i] + (k - p.offsets[i]);
      if (s < steps) out[s]++;
    }
  }
  return out;
}

/** The particle count the hint sizes the file for. */
export const HINT_PARTICLES = 1000;

/**
 * MQ4: what playback says for a run that saved no particles, and how to turn it on, with what it
 * costs on disk by upstream's formula (`settings.ts`, `pbinBytes`) at this run's steps and sources.
 */
export function noParticlesText(steps: number, sources: number): { title: string; how: string } {
  const size = sizeText(pbinBytes(HINT_PARTICLES, steps, Math.max(1, sources)));
  return {
    title: 'No particles saved for this run',
    how: `To play particles, set Simulate › Particles saved for playback above 0 and run again. ` +
      `1,000 saved particles write about ${size} a band for this run's steps and sources.`,
  };
}
