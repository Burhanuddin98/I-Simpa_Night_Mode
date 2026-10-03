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

/**
 * The playback's vertex shader (after the ramp's GLSL), in draw mode and in count mode (gate (d),
 * `ResultsLayer.countParticles`). Both pass through `kept()` first, the one place a record is
 * dropped: it is the `.pbin`'s own definition of alive (a record at its step, `aliveCounts`),
 * so the count is of what the draw keeps. Energy is colour, never a cull: a record alive by the
 * file is drawn and counted whatever its energy (zero draws at the ramp's floor). What count
 * mode does not share with the draw is the camera (each kept record lands on the one pixel, so
 * the count does not depend on the view: a particle off-screen or behind a wall is still alive)
 * and, in the fragment shader, the round sprite's corners (every point keeps its centre pixels).
 * particles.test.ts holds this shape: the gate, the count branch right after it, no cull after.
 */
export const PARTICLE_VERTEX_GLSL = /* glsl */ `
uniform float uStep;
uniform float uCount;
uniform float uSize;
uniform float uLogMax;
attribute float aStep;
attribute float aEnergy;
varying vec3 vColor;
// The records drawn now: the particles alive at the timeline's step.
bool kept() {
  return abs(aStep - uStep) <= 0.5;
}
void main() {
  if (!kept()) { gl_Position = vec4(2.0, 2.0, 2.0, 1.0); gl_PointSize = 0.0; vColor = vec3(0.0); return; }
  if (uCount > 0.5) { gl_Position = vec4(0.0, 0.0, 0.0, 1.0); gl_PointSize = 1.0; vColor = vec3(1.0); return; }
  float db = aEnergy > 0.0 ? 10.0 * log2(aEnergy) * 0.30102999566398120 : -1e9;
  vColor = hot(0.55 + 0.45 * clamp((db - uLogMax + 40.0) / 40.0, 0.0, 1.0));
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
  gl_PointSize = uSize;
}
`;

/** The playback's fragment shader. */
export const PARTICLE_FRAGMENT_GLSL = /* glsl */ `
      uniform float uCount;
      varying vec3 vColor;
      void main() {
        if (uCount > 0.5) { gl_FragColor = vec4(1.0, 0.0, 0.0, 1.0); return; }
        vec2 d = gl_PointCoord - 0.5;
        if (dot(d, d) > 0.25) discard;
        gl_FragColor = vec4(vColor, 1.0);
      }
`;
