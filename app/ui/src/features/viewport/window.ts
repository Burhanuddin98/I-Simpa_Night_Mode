// The map's time window (Burhan 2026-10-05 01:59): each face's energy averaged over the last few
// steps, so a sparse map (a step where most faces caught no particle) reads as the sound field
// it samples instead of a patchwork of half-squares. Pure, tested by window.test.ts under
// `node --test`; the GPU's twin is resultsLayer.ts's `faceValue`.
//
// The rule: at step t with a window of w steps, the face's value is the mean of the file's own
// per-step values at steps max(0, t - w + 1) .. t (the instantaneous map's texels: a step with no
// record is 0, a non-finite record is 0), each added in float32 in step order, then divided by the
// number of steps the window holds (clipped at step 0, not w). Nothing is invented: a window that
// holds no energy draws nothing, and w = 1 is the raw step bit for bit. The window trails the step
// (the last w steps up to it), so nothing shows before the file has it.
//
// With the other views: a difference averages both runs over the same window, then subtracts the
// levels; smooth colour averages the faces' window means at each node; the cumulative map already
// sums every step from 0 ms, so the window does not apply to it (refused with that reason).
// The legend keeps the map's own range from the file's records (mapData.ts): a window mean is never
// above the largest record it holds, so the top holds; a mean below the floor is drawn at the
// floor's colour, as a record below it already is.
import type { SurfaceMap } from '../../resultsData.ts';

/** The window's choices, ms of sound; 0 is one step (no averaging). */
export const WINDOW_CHOICES_MS: readonly number[] = [0, 5, 10, 20, 50];
/** Burhan's default (2026-10-05). */
export const DEFAULT_WINDOW_MS = 10;
/** The longest window the shader's loop takes, steps. */
export const MAX_WINDOW_STEPS = 64;

/** Why the cumulative map takes no window. */
export const WINDOW_CUMULATIVE_REFUSAL = 'The cumulative map already sums every step from 0 ms: the time window does not apply to it';

/** The window's hint, on its chips. */
export const WINDOW_HINT =
  "Each face's value is the mean of the file's own values over the last steps up to this one: a step where few particles crossed a face no longer blanks it. Off shows each step on its own.";

const msText = (ms: number) => String(Math.round(ms * 10) / 10);

/** The steps a window of `ms` takes in a run of time step `dtS`, or why it is one step. */
export function windowChoice(ms: number, dtS: number | null | undefined): { steps: number; refusal: string | null } {
  if (!(ms > 0)) return { steps: 1, refusal: null };
  if (!dtS || !(dtS > 0)) return { steps: 1, refusal: 'The run has no time step: no window' };
  const stepMs = dtS * 1000;
  const steps = Math.round(ms / stepMs);
  if (steps <= 1) return { steps: 1, refusal: `${ms} ms is not averaged: one step is already ${msText(stepMs)} ms` };
  if (steps > MAX_WINDOW_STEPS) return { steps: 1, refusal: `${ms} ms is ${steps} steps, more than the ${MAX_WINDOW_STEPS} a window averages` };
  return { steps, refusal: null };
}

/** `averaged over 10 ms`, or null for one step. */
export function windowLabel(steps: number, dtS: number | null | undefined): string | null {
  if (!(steps > 1)) return null;
  return dtS ? `averaged over ${msText(steps * dtS * 1000)} ms` : `averaged over ${steps} steps`;
}

/** The face's value at each step as the instantaneous texture holds it (the last record of a step; 0 for none). */
function perStep(m: SurfaceMap, face: number): Float32Array {
  const out = new Float32Array(m.timeStepCount);
  for (let k = m.offsets[face]; k < m.offsets[face + 1]; k++) {
    const s = m.steps[k];
    if (s < m.timeStepCount) out[s] = m.values[k];
  }
  return out;
}

export interface WindowMean {
  /** The mean, float32; 0 where the window holds no energy. */
  mean: number;
  /** The window's first and last step, the steps it holds, and how many of them carry energy. */
  from: number;
  to: number;
  count: number;
  withEnergy: number;
}

/** Face `face`'s window mean at `step` over `w` steps, by the rule above (the probe's number). */
export function windowedAt(m: SurfaceMap, face: number, step: number, w: number): WindowMean {
  const to = Math.max(0, Math.min(m.timeStepCount - 1, step));
  const v = perStep(m, face);
  if (!(w > 1)) {
    const e = v[to];
    return { mean: e, from: to, to, count: 1, withEnergy: e > 0 && Number.isFinite(e) ? 1 : 0 };
  }
  const from = Math.max(0, to - w + 1);
  let s = 0;
  let withEnergy = 0;
  for (let k = from; k <= to; k++) {
    const e = Number.isFinite(v[k]) ? v[k] : 0;
    if (e > 0) withEnergy++;
    s = Math.fround(s + e);
  }
  return { mean: Math.fround(s / (to - from + 1)), from, to, count: to - from + 1, withEnergy };
}
