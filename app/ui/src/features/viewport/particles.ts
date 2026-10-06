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

/** The step the sources emit at: the first step any saved particle is alive (0 when none is). */
export function emissionStep(p: Particles): number {
  let first = Infinity;
  for (let i = 0; i < p.particleCount; i++) if (p.offsets[i + 1] > p.offsets[i]) first = Math.min(first, p.firstStep[i]);
  return Number.isFinite(first) ? first : 0;
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
 * The arithmetic the playback's keep rules are written in: plain numbers (the tests, which run the
 * rules themselves) or TSL nodes (the GPU's draw and its count mode). One definition of what is kept,
 * so the count is of what the draw keeps and the tests hold the rule the GPU runs.
 */
export interface KeepOps<V, B> {
  c(x: number): V;
  sub(a: V, b: V): V;
  add(a: V, b: V): V;
  abs(a: V): V;
  le(a: V, b: V): B;
  ge(a: V, b: V): B;
  gt(a: V, b: V): B;
  and(a: B, b: B): B;
}

export const NUMBER_OPS: KeepOps<number, boolean> = {
  c: (x) => x,
  sub: (a, b) => a - b,
  add: (a, b) => a + b,
  abs: Math.abs,
  le: (a, b) => a <= b,
  ge: (a, b) => a >= b,
  gt: (a, b) => a > b,
  and: (a, b) => a && b,
};

/**
 * The records drawn now (gate (d)): a record is drawn exactly at its own step, the `.pbin`'s own
 * definition of alive (`aliveCounts`). Energy is colour, never a cull: it is not an argument here.
 */
export function keptRecord<V, B>(o: KeepOps<V, B>, recordStep: V, step: V): B {
  return o.le(o.abs(o.sub(recordStep, step)), o.c(0.5));
}

/**
 * W3: the trail segments drawn now: the particle is alive at the step and the head is one of its last
 * `length` steps. On the steps alone, never energy.
 */
export function keptTrail<V, B>(o: KeepOps<V, B>, last: V, head: V, step: V, length: V): B {
  return o.and(o.and(o.ge(last, o.sub(step, o.c(0.5))), o.le(head, o.add(step, o.c(0.5)))), o.gt(head, o.add(o.sub(step, length), o.c(0.5))));
}

/** Where on the map's hot ramp a record's energy is drawn: 0.55 to 1 over the 40 dB below the band's loudest record. */
export function rampPlace(energy: number, logMax: number): number {
  const db = energy > 0 ? 10 * Math.log10(energy) : -1e9;
  return 0.55 + 0.45 * Math.min(1, Math.max(0, (db - logMax + 40) / 40));
}

/** A trail segment's opacity: 1 at the head's own step, down to 0.15 at the far end of the trail. */
export function trailAlpha(step: number, head: number, length: number): number {
  return 1 - 0.85 * Math.min(1, Math.max(0, (step - head) / Math.max(length, 1)));
}

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

/** The timeline card's one line while trails are drawn (the hint is its tooltip). */
export const TRAIL_NOTE = 'Straight lines between saved positions; older is fainter.';

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

