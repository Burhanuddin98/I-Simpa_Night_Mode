// Sound filling the hall (wow list W2, parity R40; docs/investigations/2026-10-04-wow-w2w3w9/PLAN.md):
// the cumulative map. Pure, tested by cumulative.test.ts under `node --test`.
//
// Upstream's rule (`isimpa/3dengine/Core/Recepteurs_surfacique.cpp:337-387`, the menu's "Cumulative
// instantaneous sound level"): per face, each record's energy is added into its step in float32, a
// non-finite record read as 0; then `energy[t] += energy[t - 1]` over the steps; then dB. Here the
// adds land in a Float32Array, whose store rounds each double sum to float32: a float sum taken in
// double and rounded once to float is the float sum (double has more than 2 x 24 + 2 bits), so this
// is C's `float +=` bit for bit. The probe's one face goes through the same arithmetic.
import type { SurfaceMap } from '../../resultsData.ts';
import { LEVEL_DEPTH_DB, levelDb, type MapLayout, type Range } from './mapData.ts';

/** The switch's hint: what the picture is, and what it is not. */
export const CUMULATIVE_HINT =
  "Each face's energy summed from the first step to this one (I-Simpa's cumulative sound level): the colour builds up and holds. The decay is the instantaneous map.";

/** The legend's line while the map is cumulative. */
export const CUMULATIVE_NOTE = 'Summed from the first step: it builds up and holds.';

/** Why a map kind has no cumulative view, or null. */
export function cumulativeRefusal(kind: 'level' | 'diff'): string | null {
  return kind === 'diff' ? 'A difference between two runs is shown instantaneous only' : null;
}

/** One face's per-step energies, each record added into its step (non-finite as 0), then summed over the steps. */
function faceRun(m: SurfaceMap, face: number, out: Float32Array, at: number): void {
  const n = m.timeStepCount;
  for (let k = m.offsets[face]; k < m.offsets[face + 1]; k++) {
    const s = m.steps[k];
    if (s >= n) continue;
    const v = m.values[k];
    out[at + s] += Number.isFinite(v) ? v : 0;
  }
  for (let t = 1; t < n; t++) out[at + t] += out[at + t - 1];
}

/** The cumulative map's texture: the layout of `denseValues`, each texel the face's running sum. */
export function cumulativeValues(m: SurfaceMap, l: MapLayout): Float32Array {
  const out = new Float32Array(l.width * l.height);
  for (let f = 0; f < m.faceCount; f++) faceRun(m, f, out, f * l.steps);
  return out;
}

/** Face `face`'s cumulative energy at `step`, by the texture's arithmetic (the probe's number). */
export function cumulativeAt(m: SurfaceMap, face: number, step: number): number {
  const one = new Float32Array(m.timeStepCount);
  faceRun(m, face, one, 0);
  return one[Math.max(0, Math.min(m.timeStepCount - 1, step))];
}

/** Whole dB around every cumulative level, at most `LEVEL_DEPTH_DB` deep (as `levelRange`); null when none. */
export function cumulativeRange(m: SurfaceMap): Range | null {
  const one = new Float32Array(m.timeStepCount);
  let min = Infinity;
  let max = -Infinity;
  for (let f = 0; f < m.faceCount; f++) {
    one.fill(0);
    faceRun(m, f, one, 0);
    for (let t = 0; t < one.length; t++) {
      const l = levelDb(one[t]);
      if (l === null) continue;
      if (l < min) min = l;
      if (l > max) max = l;
    }
  }
  if (max === -Infinity) return null;
  const slack = 1e-5;
  const hi = Math.ceil(max - slack);
  let lo = Math.max(Math.floor(min + slack), hi - LEVEL_DEPTH_DB);
  if (lo >= hi) lo = hi - 1;
  return { lo, hi };
}
