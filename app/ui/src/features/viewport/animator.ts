// The one Animator timeline the surface map and the particles share (M12 P3: R39, R53, R55).
// Both draw at `animatorStore`'s step; nothing else moves them. `advance` is pure (tested by
// particles.test.ts); `Animator` drives it from animation frames while playing.
import { Store } from '../../store.ts';

export interface AnimatorState {
  /** The step both layers draw, 0-based. */
  step: number;
  /** The run's step count (SPPS's `steps`). */
  steps: number;
  playing: boolean;
  stepsPerSecond: number;
  /** Fractional steps not yet taken. */
  carry: number;
}

export const DEFAULT_RATE = 30;

/** The state `dtMs` later: whole steps at the rate, the remainder carried; stops at the last step. */
export function advance(s: AnimatorState, dtMs: number): AnimatorState {
  if (!s.playing || s.steps <= 0) return s;
  const total = s.carry + (dtMs / 1000) * s.stepsPerSecond;
  const whole = Math.floor(total);
  const step = s.step + whole;
  if (step >= s.steps - 1) return { ...s, step: Math.max(0, s.steps - 1), playing: false, carry: 0 };
  return { ...s, step, carry: total - whole };
}

export const animatorStore = new Store<AnimatorState>({ step: 0, steps: 0, playing: false, stepsPerSecond: DEFAULT_RATE, carry: 0 });

let frame = 0;
let last = 0;

function loop(t: number): void {
  const s = animatorStore.get();
  if (!s.playing) {
    frame = 0;
    return;
  }
  const next = advance(s, last ? t - last : 0);
  last = t;
  if (next !== s) animatorStore.set(next);
  frame = requestAnimationFrame(loop);
}

export const Animator = {
  /** A new run's timeline: step 0, paused. */
  reset(steps: number): void {
    this.pause();
    animatorStore.set({ ...animatorStore.get(), step: 0, steps, carry: 0 });
  },
  setStep(step: number): void {
    const s = animatorStore.get();
    const k = Math.max(0, Math.min(Math.max(0, s.steps - 1), Math.round(step)));
    if (k !== s.step || s.carry !== 0) animatorStore.set({ ...s, step: k, carry: 0 });
  },
  play(): void {
    const s = animatorStore.get();
    if (s.playing || s.steps <= 1) return;
    // From the start again when it stopped at the end.
    animatorStore.set({ ...s, playing: true, carry: 0, step: s.step >= s.steps - 1 ? 0 : s.step });
    last = 0;
    if (!frame) frame = requestAnimationFrame(loop);
  },
  pause(): void {
    const s = animatorStore.get();
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
    if (s.playing) animatorStore.set({ ...s, playing: false, carry: 0 });
  },
};
