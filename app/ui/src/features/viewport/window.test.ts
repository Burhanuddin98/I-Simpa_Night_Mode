import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { SurfaceMap } from '../../resultsData.ts';
import {
  DEFAULT_WINDOW_MS,
  MAX_WINDOW_STEPS,
  WINDOW_CHOICES_MS,
  WINDOW_CUMULATIVE_REFUSAL,
  windowChoice,
  windowLabel,
  windowedAt,
} from './window.ts';

const f = Math.fround;

/**
 * One face's sparse series over 8 steps of 1 ms: energy at steps 1, 2 and 5 only (the patchy
 * map); face 1 holds a NaN at step 3 and 2e-6 at step 4; face 2 has no records.
 */
function map(): SurfaceMap {
  const values = new Float32Array([4e-6, 1e-6, 3e-7, NaN, 2e-6]);
  return {
    nodeCount: 5,
    faceCount: 3,
    recordCount: values.length,
    timeStepCount: 8,
    timeStepS: f(0.001),
    recordType: 0,
    positions: new Float32Array(15),
    indices: new Uint32Array([0, 1, 2, 1, 3, 2, 2, 3, 4]),
    receivers: new Uint32Array([0, 0, 0]),
    offsets: new Uint32Array([0, 3, 5, 5]),
    steps: new Uint32Array([1, 2, 5, 3, 4]),
    values,
  };
}

/** The rule written out: steps max(0, t - w + 1) .. t, each a float32 add (non-finite as 0), then over the count. */
function byHand(series: number[], t: number, w: number): number {
  const from = Math.max(0, t - w + 1);
  let s = 0;
  for (let k = from; k <= t; k++) s = f(s + (Number.isFinite(series[k]) ? series[k] : 0));
  return f(s / (t - from + 1));
}

test('the window mean is the mean of the face\'s own per-step values over the last w steps, gaps counted as 0', () => {
  const m = map();
  const face0 = [0, f(4e-6), f(1e-6), 0, 0, f(3e-7), 0, 0];
  for (let t = 0; t < 8; t++) {
    for (const w of [2, 3, 5]) {
      const a = windowedAt(m, 0, t, w);
      assert.equal(a.mean, byHand(face0, t, w), `face 0 step ${t} window ${w}`);
    }
  }
  // Step 4 has no record: alone it is blank, over 3 steps (2..4) it holds (1e-6 + 0 + 0) / 3.
  const at4 = windowedAt(m, 0, 4, 3);
  assert.equal(at4.mean, f(f(1e-6) / 3));
  assert.deepEqual([at4.from, at4.to, at4.count, at4.withEnergy], [2, 4, 3, 1]);
  // The window is clipped at step 0, and divides by the steps it holds, not by w.
  const at1 = windowedAt(m, 0, 1, 5);
  assert.deepEqual([at1.from, at1.to, at1.count, at1.withEnergy], [0, 1, 2, 1]);
  assert.equal(at1.mean, f(f(4e-6) / 2));
});

test('window 1 is the raw step: the file\'s own float32, bit for bit, and nothing where it has none', () => {
  const m = map();
  for (let k = 0; k < m.offsets[1]; k++) {
    const a = windowedAt(m, 0, m.steps[k], 1);
    assert.equal(a.mean, m.values[k]);
    assert.equal(a.count, 1);
  }
  assert.equal(windowedAt(m, 0, 3, 1).mean, 0);
  assert.equal(windowedAt(m, 0, 3, 1).withEnergy, 0);
});

test('say NO: a NaN record is 0 in the mean, a face with no records stays empty, a window past the last record empties', () => {
  const m = map();
  // Face 1: NaN at step 3 read as 0 (not a NaN that blanks the window), 2e-6 at step 4.
  assert.equal(windowedAt(m, 1, 4, 2).mean, f(f(2e-6) / 2));
  assert.equal(windowedAt(m, 1, 3, 2).mean, 0);
  assert.equal(windowedAt(m, 1, 3, 2).withEnergy, 0);
  assert.equal(windowedAt(m, 2, 7, 5).mean, 0, 'no records: no energy, whatever the window');
  // Face 0's last record is at step 5: a window of 2 ending at 7 holds steps 6..7, nothing.
  assert.equal(windowedAt(m, 0, 7, 2).mean, 0);
  // Out-of-range steps clamp to the map's steps.
  assert.equal(windowedAt(m, 0, 99, 3).to, 7);
});

test('the choices: Off is one step; a choice turns into whole steps of the run; one that is a single step or too long says why', () => {
  assert.deepEqual([...WINDOW_CHOICES_MS], [0, 5, 10, 20, 50]);
  assert.equal(DEFAULT_WINDOW_MS, 10);
  const ms1 = f(0.001);
  assert.deepEqual(windowChoice(0, ms1), { steps: 1, refusal: null });
  assert.deepEqual(windowChoice(10, ms1), { steps: 10, refusal: null });
  assert.deepEqual(windowChoice(50, ms1), { steps: 50, refusal: null });
  assert.equal(windowChoice(5, ms1).steps, 5);
  // say NO: a 10 ms run step makes 10 ms one step, 5 ms less than one: not averaged, and said.
  const ms10 = f(0.01);
  const one = windowChoice(10, ms10);
  assert.equal(one.steps, 1);
  assert.match(one.refusal ?? '', /one step is already 10 ms/);
  assert.match(windowChoice(5, ms10).refusal ?? '', /one step is already 10 ms/);
  assert.equal(windowChoice(20, ms10).steps, 2);
  // say NO: no time step, no window; a window past the shader's loop is refused.
  assert.match(windowChoice(10, null).refusal ?? '', /time step/);
  assert.equal(windowChoice(10, null).steps, 1);
  const tiny = windowChoice(50, f(0.0001));
  assert.equal(tiny.steps, 1);
  assert.match(tiny.refusal ?? '', new RegExp(`${MAX_WINDOW_STEPS}`));
});

test('the label says the window in ms of sound, and nothing for one step; the cumulative map refuses it with a reason', () => {
  assert.equal(windowLabel(10, f(0.001)), 'averaged over 10 ms');
  assert.equal(windowLabel(2, f(0.01)), 'averaged over 20 ms');
  assert.equal(windowLabel(5, f(0.001)), 'averaged over 5 ms');
  assert.equal(windowLabel(1, f(0.001)), null);
  assert.match(WINDOW_CUMULATIVE_REFUSAL, /cumulative/i);
  assert.match(WINDOW_CUMULATIVE_REFUSAL, /from 0 ms/);
});

test('the probe under a window shows the window mean and says so; the difference averages both runs alike', async () => {
  const { probeOf, valueBits } = await import('./mapView.ts');
  const m = map();
  const want = windowedAt(m, 0, 4, 3).mean;
  const p = probeOf(m, 0, 4, { what: 'S', band: '1 kHz', dtS: m.timeStepS, smooth: false, windowSteps: 3 });
  assert.equal(p.bits, valueBits(want));
  assert.equal(p.level, `${(10 * Math.log10(want) + 120).toFixed(1)} dB`);
  assert.match(p.value, /mean of the file's values over 3 ms/);
  assert.match(p.value, /steps 2–4, 1 of 3 with energy/);
  // The raw step there has no record: the same face with no window says so.
  assert.equal(probeOf(m, 0, 4, { what: 'S', band: 'b', dtS: m.timeStepS, smooth: false }).level, null);
  // say NO: a window with no energy has no number.
  const none = probeOf(m, 0, 7, { what: 'S', band: 'b', dtS: m.timeStepS, smooth: false, windowSteps: 2 });
  assert.equal(none.level, null);
  assert.equal(none.bits, null);
  assert.match(none.value, /No energy in the 2 ms up to this step/);
  // Smooth colour: the number is still the face's own mean, said so.
  const sm = probeOf(m, 0, 4, { what: 'S', band: 'b', dtS: m.timeStepS, smooth: true, windowSteps: 3 });
  assert.match(sm.note ?? '', /own mean/);
  // A difference: both runs' window means, this run minus the baseline.
  const base = { ...map(), values: new Float32Array([2e-6, 1e-6, 3e-7, NaN, 2e-6]) };
  const d = probeOf(m, 0, 2, { what: 'S', band: 'b', dtS: m.timeStepS, smooth: false, base, windowSteps: 2 });
  const la = 10 * Math.log10(windowedAt(m, 0, 2, 2).mean) + 120;
  const lb = 10 * Math.log10(windowedAt(base, 0, 2, 2).mean) + 120;
  assert.equal(d.level, `${la - lb > 0 ? '+' : '−'}${Math.abs(la - lb).toFixed(1)} dB`);
  assert.match(d.value, /each averaged over 2 ms/);
  // A window of 1 is the instantaneous probe exactly.
  assert.deepEqual(probeOf(m, 0, 2, { what: 'S', band: 'b', dtS: m.timeStepS, smooth: false, windowSteps: 1 }), probeOf(m, 0, 2, { what: 'S', band: 'b', dtS: m.timeStepS, smooth: false }));
});
