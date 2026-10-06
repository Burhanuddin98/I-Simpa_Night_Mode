// The GPU particle layer's data, before the GPU (decision 68 (b), PLAN.md B2). Pure, tested by
// raysData.test.ts under `node --test`; the GPU's twin is rays.ts, whose compute pass runs `particleAt`.
//
// A saved particle is a polyline: its record k is where it was at step firstStep + k (`docs/formats/pbin.md`).
// Between two records the layer moves it in a straight line at the fraction of the step the timeline is
// at, so playback is smooth at any speed and lands exactly on the file's record at every whole step. A
// reflection between two records is cut as a chord, as the trails already say (particles.ts TRAIL_HINT).
//
// The benchmark's million: the saved paths replayed, copy c of particle s offset by a fixed jitter
// (`copyJitter`, 0 for copy 0, so the file's own particles are drawn exactly where the file has them).
import type { Particles } from '../../resultsData.ts';

/** Per particle: its first record, its record count, its first step, 0 (one uvec4 on the GPU). */
export function particleTable(p: Particles): Uint32Array {
  const out = new Uint32Array(4 * p.particleCount);
  for (let i = 0; i < p.particleCount; i++) {
    out[4 * i] = p.offsets[i];
    out[4 * i + 1] = p.offsets[i + 1] - p.offsets[i];
    out[4 * i + 2] = p.firstStep[i];
  }
  return out;
}

/** Per record: x, y, z and energy (one vec4 on the GPU). */
export function recordTable(p: Particles): Float32Array {
  const out = new Float32Array(4 * p.recordCount);
  for (let k = 0; k < p.recordCount; k++) {
    out[4 * k] = p.positions[3 * k];
    out[4 * k + 1] = p.positions[3 * k + 1];
    out[4 * k + 2] = p.positions[3 * k + 2];
    out[4 * k + 3] = p.energies[k];
  }
  return out;
}

/** Per record: the particle it belongs to (the ray mode joins record k to k + 1 only within one particle). */
export function recordOwners(p: Particles): Uint32Array {
  const out = new Uint32Array(p.recordCount);
  for (let i = 0; i < p.particleCount; i++) out.fill(i, p.offsets[i], p.offsets[i + 1]);
  return out;
}

/**
 * Particle `i` at fractional step `t`: its position and energy interpolated between the records either
 * side of `t`, or null where it is not alive (before its first record, after its last). At a whole step
 * it is the record itself; a particle with one record shows only at that step.
 */
export function particleAt(p: Particles, i: number, t: number): [number, number, number, number] | null {
  const a = p.offsets[i];
  const n = p.offsets[i + 1] - a;
  const k = t - p.firstStep[i];
  if (n === 0 || k < 0 || k > n - 1) return null;
  const k0 = Math.min(Math.floor(k), n - 1);
  const k1 = Math.min(k0 + 1, n - 1);
  const f = k - k0;
  const at = (j: number, c: number) => (c < 3 ? p.positions[3 * (a + j) + c] : p.energies[a + j]);
  return [0, 1, 2, 3].map((c) => at(k0, c) + (at(k1, c) - at(k0, c)) * f) as [number, number, number, number];
}

/** Particles the layer's alive rule shows at `t` (a whole step: exactly `aliveCounts` of particles.ts). */
export function aliveAt(p: Particles, t: number): number {
  let c = 0;
  for (let i = 0; i < p.particleCount; i++) if (particleAt(p, i, t)) c++;
  return c;
}

/** The benchmark's offset of copy `c`, metres: none for copy 0, else within +-`scale`/2 on each axis, fixed per copy. */
export function copyJitter(c: number, scale: number): [number, number, number] {
  if (c === 0) return [0, 0, 0];
  const h = (x: number) => {
    // A small integer hash (mulberry32's mix), the same every run.
    let z = (x + 0x6d2b79f5) | 0;
    z = Math.imul(z ^ (z >>> 15), z | 1);
    z ^= z + Math.imul(z ^ (z >>> 7), z | 61);
    return ((z ^ (z >>> 14)) >>> 0) / 4294967296;
  };
  return [0, 1, 2].map((k) => (h(3 * c + k) - 0.5) * scale) as [number, number, number];
}

/**
 * The bounce points of decision 69's sparks: records where a particle's path turns by more than
 * `minDeg` degrees (the chord before and the chord after), in record order. A saved record is where
 * the particle was at a step, so the reflection happened within the step before or after; the spark
 * is drawn at the record, at its step.
 */
export function bounceRecords(p: Particles, minDeg = 25): Uint32Array {
  const out: number[] = [];
  const cosMax = Math.cos((minDeg * Math.PI) / 180);
  const P = p.positions;
  for (let i = 0; i < p.particleCount; i++) {
    for (let k = p.offsets[i] + 1; k < p.offsets[i + 1] - 1; k++) {
      const a = [P[3 * k] - P[3 * k - 3], P[3 * k + 1] - P[3 * k - 2], P[3 * k + 2] - P[3 * k - 1]];
      const b = [P[3 * k + 3] - P[3 * k], P[3 * k + 4] - P[3 * k + 1], P[3 * k + 5] - P[3 * k + 2]];
      const la = Math.hypot(a[0], a[1], a[2]);
      const lb = Math.hypot(b[0], b[1], b[2]);
      if (!(la > 0 && lb > 0)) continue;
      if ((a[0] * b[0] + a[1] * b[1] + a[2] * b[2]) / (la * lb) < cosMax) out.push(k);
    }
  }
  return Uint32Array.from(out);
}
