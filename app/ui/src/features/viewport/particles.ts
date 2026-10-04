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

// ---- W3: particle trails (wow list; parity R54; docs/investigations/2026-10-04-wow-w2w3w9/PLAN.md) ----
//
// Upstream's "Rays" (`isimpa/3dengine/Core/Particules.cpp:366-386`) draw, for each live particle,
// one line from its position at the step before to its position at the step. A trail is the last
// N of those lines (N = 1 is upstream's ray). Each segment joins two saved records with a straight
// line, so a reflection between two saved steps is cut as a chord: the hint says so. A segment is
// kept only while its particle is alive at the step (as its dot is) and its head is one of the
// last N steps; both ends carry the same tags, so the shader keeps or drops a segment whole.

/** The trail lengths offered, steps; 1 is upstream's ray. */
export const TRAIL_LENGTHS: readonly number[] = [1, 5, 20, 60];
/** Two vertices a segment, each position (3), head step, last step and energy, float32. */
export const TRAIL_BYTES_PER_SEGMENT = 48;
/** The most GPU memory trails may take. */
export const TRAIL_BUDGET_BYTES = 256 * 1024 * 1024;

export const TRAIL_HINT =
  "Each live particle's path over its last steps: straight lines between its saved positions, one a step, so a bounce inside a step is cut short. Older is fainter.";

export interface Trails {
  segments: number;
  /** Two vertices a segment: the earlier record, then the head. */
  positions: Float32Array;
  /** Per vertex, the segment's head step. */
  head: Float32Array;
  /** Per vertex, the particle's last step. */
  last: Float32Array;
  /** Per vertex, the head's energy (the colour). */
  energy: Float32Array;
}

/** Every segment of the band's particles. */
export function trailSegments(p: Particles): Trails {
  let n = 0;
  for (let i = 0; i < p.particleCount; i++) n += Math.max(0, p.offsets[i + 1] - p.offsets[i] - 1);
  const positions = new Float32Array(6 * n);
  const head = new Float32Array(2 * n);
  const last = new Float32Array(2 * n);
  const energy = new Float32Array(2 * n);
  let j = 0;
  for (let i = 0; i < p.particleCount; i++) {
    const a = p.offsets[i];
    const b = p.offsets[i + 1];
    const end = p.firstStep[i] + (b - a) - 1;
    for (let k = a + 1; k < b; k++) {
      positions.set(p.positions.subarray(3 * (k - 1), 3 * k), 6 * j);
      positions.set(p.positions.subarray(3 * k, 3 * k + 3), 6 * j + 3);
      const h = p.firstStep[i] + (k - a);
      head[2 * j] = head[2 * j + 1] = h;
      last[2 * j] = last[2 * j + 1] = end;
      energy[2 * j] = energy[2 * j + 1] = p.energies[k];
      j++;
    }
  }
  return { segments: n, positions, head, last, energy };
}

/** The segments a step keeps with trails `n` steps long: per particle alive at `step`, min(n, step - first). */
export function trailCount(p: Particles, step: number, n: number): number {
  let c = 0;
  for (let i = 0; i < p.particleCount; i++) {
    const first = p.firstStep[i];
    const records = p.offsets[i + 1] - p.offsets[i];
    if (records === 0 || step < first || step > first + records - 1) continue;
    c += Math.min(n, step - first);
  }
  return c;
}

/** Why trails cannot be drawn, or null. */
export function trailRefusal(p: Particles | null): string | null {
  if (!p) return 'No particles saved for this run';
  const bytes = TRAIL_BYTES_PER_SEGMENT * Math.max(0, p.recordCount - p.particleCount);
  if (bytes > TRAIL_BUDGET_BYTES) {
    return `Trails of these particles would take ${Math.ceil(bytes / 2 ** 20)} MB of GPU memory, more than the ${TRAIL_BUDGET_BYTES / 2 ** 20} MB they may use`;
  }
  return null;
}

/**
 * The trails' vertex shader (after the ramp's GLSL), in draw mode and in count mode (the W3 hook,
 * `ResultsLayer.countTrails`, draws this material as points, two a kept segment). Both pass
 * through `keptTrail()` first, on the steps alone; energy is colour, never a cull.
 */
export const TRAIL_VERTEX_GLSL = /* glsl */ `
uniform float uStep;
uniform float uCount;
uniform float uLength;
uniform float uLogMax;
attribute float aHead;
attribute float aLast;
attribute float aEnergy;
varying vec3 vColor;
varying float vAlpha;
// The segments drawn now: the particle is alive at the step and the head is one of its last uLength steps.
bool keptTrail() {
  return aLast >= uStep - 0.5 && aHead <= uStep + 0.5 && aHead > uStep - uLength + 0.5;
}
void main() {
  if (!keptTrail()) { gl_Position = vec4(2.0, 2.0, 2.0, 1.0); gl_PointSize = 0.0; vColor = vec3(0.0); vAlpha = 0.0; return; }
  if (uCount > 0.5) { gl_Position = vec4(0.0, 0.0, 0.0, 1.0); gl_PointSize = 1.0; vColor = vec3(1.0); vAlpha = 1.0; return; }
  float db = aEnergy > 0.0 ? 10.0 * log2(aEnergy) * 0.30102999566398120 : -1e9;
  vColor = hot(0.55 + 0.45 * clamp((db - uLogMax + 40.0) / 40.0, 0.0, 1.0));
  vAlpha = 1.0 - 0.85 * clamp((uStep - aHead) / max(uLength, 1.0), 0.0, 1.0);
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
  gl_PointSize = 1.0;
}
`;

/** The trails' fragment shader. */
export const TRAIL_FRAGMENT_GLSL = /* glsl */ `
      uniform float uCount;
      varying vec3 vColor;
      varying float vAlpha;
      void main() {
        if (uCount > 0.5) { gl_FragColor = vec4(1.0, 0.0, 0.0, 1.0); return; }
        gl_FragColor = vec4(vColor, vAlpha);
      }
`;
