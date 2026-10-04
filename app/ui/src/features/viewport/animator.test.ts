import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import {
  advance,
  atStep,
  DEFAULT_SPEED,
  MAX_TICK_MS,
  paused,
  played,
  rateText,
  readout,
  reset,
  SPEEDS,
  STEP_BY_STEP_MAX,
  stepped,
  stepsPerSecond,
  toStart,
  withSpeed,
  withStart,
  type AnimatorState,
} from './animator.ts';

/** CR4's run: 10 s in 1 ms steps, at the default speed, playing from the emission. */
const cr4 = (patch: Partial<AnimatorState> = {}): AnimatorState => ({ step: 0, steps: 10_000, playing: true, speed: DEFAULT_SPEED, dtMs: 1, carry: 0, start: 0, ...patch });

/** `n` ticks of `ms` each; the step after each. */
function ticks(s: AnimatorState, n: number, ms: number): { end: AnimatorState; steps: number[] } {
  const steps: number[] = [];
  let x = s;
  for (let i = 0; i < n; i++) {
    x = advance(x, ms);
    steps.push(x.step);
  }
  return { end: x, steps };
}

test('the speed is a fraction of real time, in ms of sound per second, whatever the time step', () => {
  assert.equal(DEFAULT_SPEED, 0.01);
  assert.ok(SPEEDS.includes(DEFAULT_SPEED));
  assert.deepEqual([...SPEEDS].sort((a, b) => a - b), [...SPEEDS], 'slowest first');
  assert.equal(SPEEDS[SPEEDS.length - 1], 1, 'real time is offered');
  // 0.01x: 10 ms of sound a second, so 10 steps a second at 1 ms and 1 a second at 10 ms.
  assert.equal(stepsPerSecond({ speed: 0.01, dtMs: 1 }), 10);
  assert.equal(stepsPerSecond({ speed: 0.01, dtMs: 10 }), 1);
  assert.equal(stepsPerSecond({ speed: 1, dtMs: 1 }), 1000);
  assert.equal(rateText({ speed: 0.01, dtMs: 1 }), '0.01× real time: 10 ms of sound per second, one step every 100 ms');
  assert.equal(rateText({ speed: 0.005, dtMs: 1 }), '0.005× real time: 5.0 ms of sound per second, one step every 200 ms');
  assert.match(rateText({ speed: 1, dtMs: 1 }), /1000 ms of sound per second, 1000 steps a second, more than one a frame/);
});

test('at the default speed a 1 ms run expands one step at a time: 10 steps in a second of 40 Hz frames', () => {
  const { end, steps } = ticks(cr4(), 40, 25);
  assert.equal(end.step, 10);
  assert.equal(end.playing, true);
  for (let i = 1; i < steps.length; i++) assert.ok(steps[i] - steps[i - 1] <= 1, `tick ${i} moved ${steps[i] - steps[i - 1]} steps`);
  // Each step is held for several frames: the first step is reached on the 4th tick (100 ms).
  assert.equal(steps.indexOf(1), 3);
  assert.equal(steps.indexOf(2), 7);
});

test('say NO: a stalled frame at a slow speed takes one step, not the 20 its length is worth', () => {
  const s = advance(cr4({ step: 40 }), 2000);
  assert.equal(s.step, 41);
  assert.equal(s.carry, 0, 'nothing is banked to leap with on the next tick');
  assert.equal(advance(s, 1000 / 60).step, 41, 'the next frame does not catch up');
});

test('say NO: at real time a stalled frame counts at most MAX_TICK_MS', () => {
  const fast = cr4({ speed: 1 });
  assert.ok(stepsPerSecond(fast) > STEP_BY_STEP_MAX);
  assert.equal(advance(fast, 16).step, 16, 'real time: 16 steps a 16 ms frame');
  assert.equal(advance(fast, 2000).step, MAX_TICK_MS, 'a 2 s stall: 100 steps, not 2,000');
});

test('Play starts at the emission: from before it, and again from the end', () => {
  const s = cr4({ playing: false, start: 3 });
  assert.equal(played(s).step, 3, 'before the emission: from the emission');
  assert.equal(played({ ...s, step: 9_999 }).step, 3, 'at the end: from the emission, not step 0');
  assert.equal(played({ ...s, step: 500 }).step, 500, 'paused midway: resumes there');
  assert.equal(played(s).playing, true);
  assert.equal(played({ ...s, steps: 1 }).playing, false, 'say NO: one step has nothing to play');
  assert.equal(paused(played(s)).playing, false);
});

test('the emission comes from the particles; a paused timeline before it moves there', () => {
  const s = reset(cr4({ step: 700, speed: 0.1 }), 10_000, 1);
  assert.equal(s.step, 0);
  assert.equal(s.playing, false);
  assert.equal(s.speed, 0.1, 'a new run keeps the chosen speed');
  const e = withStart(s, 4);
  assert.equal(e.start, 4);
  assert.equal(e.step, 4);
  assert.equal(withStart({ ...s, step: 50 }, 4).step, 50, 'past the emission: stays');
  assert.equal(withStart(s, 20_000).start, 9_999, 'kept in the run');
});

test('step forward and back move one step, paused, and stop at the ends', () => {
  const s = cr4({ step: 10 });
  assert.equal(stepped(s, 1).step, 11);
  assert.equal(stepped(s, 1).playing, false, 'stepping pauses');
  assert.equal(stepped(s, -1).step, 9);
  assert.equal(stepped(s, 5).step, 11, 'one step whatever the sign is worth');
  assert.equal(stepped(cr4({ step: 0 }), -1).step, 0, 'say NO: no step before 0');
  assert.equal(stepped(cr4({ step: 9_999 }), 1).step, 9_999, 'say NO: no step past the last');
  assert.equal(toStart(cr4({ step: 300, start: 2 })).step, 2);
  assert.equal(atStep(s, 12.4).step, 12);
});

test('say NO: a speed that is not offered is refused', () => {
  const s = cr4();
  assert.equal(withSpeed(s, 0.025).speed, 0.025);
  assert.equal(withSpeed(s, 0.5), s);
  assert.equal(withSpeed(s, 0), s);
  assert.equal(withSpeed(s, -1), s);
});

test('the readout: the time is the report’s spps.time_step_s x 1000 x step at its decimals; the step count is steps', () => {
  const dt = Math.fround(0.001);
  const r = readout(150, 10_000, dt);
  assert.equal(r.time, '150 ms');
  assert.equal(r.path, 'spps.time_step_s');
  assert.equal(r.stepsPath, 'spps.steps');
  assert.equal(r.scale, 150_000);
  assert.equal(r.digits, 0);
  assert.equal((dt * r.scale).toFixed(r.digits ?? 0), r.ms, 'the gate (a) rule gives the text shown');
  assert.equal(r.stepText, '150');
  assert.equal(r.stepsText, '10000');
  const early = readout(42, 10_000, dt);
  assert.equal(early.time, '42.0 ms');
  assert.equal((dt * early.scale).toFixed(early.digits ?? 0), early.ms);
  // Say NO: without a time step there is no time to show, only the step.
  const none = readout(7, 100, null);
  assert.equal(none.time, 'step 7');
  assert.equal(none.path, null);
  assert.equal(none.ms, null);
  // Say NO: a TCR run's timeline is not SPPS's: no report path.
  const tcr = readout(0, 1, Math.fround(0.001), false);
  assert.equal(tcr.path, null);
  assert.equal(tcr.stepsPath, null);
});
