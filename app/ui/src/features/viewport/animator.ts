// The one Animator timeline the surface map and the particles share (M12 P3: R39, R53, R55).
// Both draw at `animatorStore`'s step; nothing else moves them. Every transition is a pure
// function here (tested by animator.test.ts); `Animator` applies them and drives `advance` from
// animation frames while playing.
//
// Playback speed (Burhan 2026-10-04 15:08: "it doesn't have any time steps so we go from a sphere to
// a chaotic field of beads in milliseconds"). The old rate was 30 steps a second whatever the run's
// step: at CR4's 1 ms that is 30 ms of sound a second, 10 m of travel, so the sphere had met the
// floor, the walls and the ceiling within about 2 s of Play, and the tour then dragged the slider
// at 170 steps a second. The speed is now a fraction of real time, said as milliseconds of sound
// per second of playback, so it means the same thing at any time step; the default is slow
// (0.01x: 10 ms of sound a second, one 1 ms step every 100 ms). At any speed of up to one step a
// frame, a tick moves at most one step, so every step is drawn; a long frame never leaps.
import { Store } from '../../store.ts';
import { stepTime } from './mapView.ts';

export interface AnimatorState {
  /** The step both layers draw, 0-based. */
  step: number;
  /** The run's step count (SPPS's `steps`). */
  steps: number;
  playing: boolean;
  /** Playback speed as a fraction of real time: 0.01 is 10 ms of sound per second. */
  speed: number;
  /** The run's time step, ms (null: unknown, then a step counts as 1 ms). */
  dtMs: number | null;
  /** Fractional steps not yet taken. */
  carry: number;
  /** The step the sources emit at: the first step any saved particle is alive (0 without particles). */
  start: number;
}

/** The speeds offered, fractions of real time, slowest first. */
export const SPEEDS: readonly number[] = [0.005, 0.01, 0.025, 0.1, 1];
/** The default: 10 ms of sound per second of playback. */
export const DEFAULT_SPEED = 0.01;
/** Up to this many steps a second, a tick takes at most one step: every step is drawn. */
export const STEP_BY_STEP_MAX = 60;
/** The longest frame a tick counts, ms: a stalled frame does not leap ahead. */
export const MAX_TICK_MS = 100;

/** Steps a second at the state's speed and time step. */
export function stepsPerSecond(s: Pick<AnimatorState, 'speed' | 'dtMs'>): number {
  return (s.speed * 1000) / (s.dtMs && s.dtMs > 0 ? s.dtMs : 1);
}

const last = (s: AnimatorState) => Math.max(0, s.steps - 1);
const clampStep = (s: AnimatorState, k: number) => Math.max(0, Math.min(last(s), Math.round(k)));

/** The state `dtMs` later: whole steps at the rate, the remainder carried; stops at the last step. */
export function advance(s: AnimatorState, dtMs: number): AnimatorState {
  if (!s.playing || s.steps <= 0) return s;
  const rate = stepsPerSecond(s);
  const total = s.carry + (Math.min(Math.max(0, dtMs), MAX_TICK_MS) / 1000) * rate;
  let whole = Math.floor(total);
  let carry = total - whole;
  if (rate <= STEP_BY_STEP_MAX && whole > 1) {
    whole = 1;
    carry = 0;
  }
  const step = s.step + whole;
  if (step >= last(s)) return { ...s, step: last(s), playing: false, carry: 0 };
  if (whole === 0 && carry === s.carry) return s;
  return { ...s, step, carry };
}

/** Play: from where it stands, from the emission when before it or at the end. */
export function played(s: AnimatorState): AnimatorState {
  if (s.playing || s.steps <= 1) return s;
  const from = s.step >= last(s) || s.step < s.start ? Math.min(s.start, last(s)) : s.step;
  return { ...s, playing: true, carry: 0, step: from };
}

/** Paused where it stands. */
export function paused(s: AnimatorState): AnimatorState {
  return s.playing || s.carry !== 0 ? { ...s, playing: false, carry: 0 } : s;
}

/** Paused at step `k`, kept in the run's steps. */
export function atStep(s: AnimatorState, k: number): AnimatorState {
  const step = clampStep(s, k);
  return step === s.step && !s.playing && s.carry === 0 ? s : { ...s, step, playing: false, carry: 0 };
}

/** One step on (`d` = 1) or back (`d` = -1), paused. */
export function stepped(s: AnimatorState, d: number): AnimatorState {
  return atStep(s, s.step + Math.sign(d));
}

/** Back to the emission, paused. */
export function toStart(s: AnimatorState): AnimatorState {
  return atStep(s, s.start);
}

/** A speed from `SPEEDS`; any other is refused (the state is returned unchanged). */
export function withSpeed(s: AnimatorState, speed: number): AnimatorState {
  return SPEEDS.includes(speed) && speed !== s.speed ? { ...s, speed, carry: 0 } : s;
}

/** The emission step, once the particles say it; a paused timeline before it moves to it. */
export function withStart(s: AnimatorState, start: number): AnimatorState {
  const k = clampStep(s, start);
  if (k === s.start && (s.playing || s.step >= k)) return s;
  return { ...s, start: k, step: !s.playing && s.step < k ? k : s.step };
}

/** A new run's timeline: at step 0, paused, its step count and time step; the speed is kept. */
export function reset(s: AnimatorState, steps: number, dtMs: number | null): AnimatorState {
  return { ...s, step: 0, steps, dtMs, start: 0, playing: false, carry: 0 };
}

/** `0.01×`. */
export const speedLabel = (x: number) => `${x}×`;

/** `10 ms of sound per second, one step every 100 ms`: the speed in the run's own time. */
export function rateText(s: Pick<AnimatorState, 'speed' | 'dtMs'>): string {
  const msPerS = s.speed * 1000;
  const sound = `${msPerS < 10 ? msPerS.toFixed(1) : Math.round(msPerS)} ms of sound per second`;
  if (!s.dtMs) return `${speedLabel(s.speed)} real time, ${Math.round(stepsPerSecond(s))} steps a second`;
  const r = stepsPerSecond(s);
  const pace = r <= 1000 / 16.7 ? `one step every ${Math.round(1000 / r)} ms` : `${Math.round(r)} steps a second, more than one a frame`;
  return `${speedLabel(s.speed)} real time: ${sound}, ${pace}`;
}

/**
 * The time and step readout, as gate (a) marks: the time is the run's own `spps.time_step_s` (the
 * report's path) times `scale` = 1000 x step, shown at `digits` decimals, so it reads in ms; the
 * step count is the report's `spps.steps`. The step index is the timeline's own number (the time's
 * scale over 1000), not a value of the report. Without a time step the readout is `step k`, and a
 * run that is not SPPS's carries no path.
 */
export function readout(step: number, steps: number, dtS: number | null | undefined, spps = true) {
  const time = stepTime(step, dtS);
  const ms = dtS ? step * dtS * 1000 : null;
  return {
    time,
    ms: ms === null ? null : time.replace(/ ms$/, ''),
    path: dtS && spps ? 'spps.time_step_s' : null,
    stepsPath: spps ? 'spps.steps' : null,
    scale: 1000 * step,
    digits: ms === null ? null : ms < 100 ? 1 : 0,
    stepText: String(step),
    stepsText: String(steps),
  };
}

export const animatorStore = new Store<AnimatorState>({ step: 0, steps: 0, playing: false, speed: DEFAULT_SPEED, dtMs: null, carry: 0, start: 0 });

let frame = 0;
let lastT = 0;

function loop(t: number): void {
  const s = animatorStore.get();
  if (!s.playing) {
    frame = 0;
    return;
  }
  const next = advance(s, lastT ? t - lastT : 0);
  lastT = t;
  if (next !== s) animatorStore.set(next);
  frame = requestAnimationFrame(loop);
}

const apply = (f: (s: AnimatorState) => AnimatorState) => {
  const s = animatorStore.get();
  const n = f(s);
  if (n !== s) animatorStore.set(n);
  return n;
};

export const Animator = {
  /** A new run's timeline: step 0, paused. */
  reset(steps: number, dtMs: number | null = null): void {
    this.pause();
    apply((s) => reset(s, steps, dtMs));
  },
  setStart(step: number): void {
    apply((s) => withStart(s, step));
  },
  setStep(step: number): void {
    this.pause();
    apply((s) => atStep(s, step));
  },
  stepBy(d: number): void {
    this.pause();
    apply((s) => stepped(s, d));
  },
  toStart(): void {
    this.pause();
    apply(toStart);
  },
  setSpeed(speed: number): void {
    apply((s) => withSpeed(s, speed));
  },
  play(): void {
    const n = apply(played);
    if (!n.playing) return;
    lastT = 0;
    if (!frame) frame = requestAnimationFrame(loop);
  },
  pause(): void {
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
    apply(paused);
  },
};
