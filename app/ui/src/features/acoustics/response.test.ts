// The response window's model (response.ts) held to hand-made echograms: the colour map's stops,
// the 60 dB clip, the dB matrix, the bands summed, the pixels' layout, the time ticks, and the
// view's numbers as paths into the report. Each rule with a case it must say NO to.
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { Report } from '../../bindings/ipc';
import { at } from './model.ts';
import {
  broadband,
  clipDb,
  COLOUR_STOPS,
  CROP_MARGIN,
  cropCols,
  colourAt,
  colourBarCss,
  dbMatrix,
  decayDb,
  freqLabel,
  lastAbove,
  levelT,
  mapPixels,
  niceInterval,
  RESPONSE_NOTE,
  RESPONSE_TITLE,
  responseView,
  SPAN_DB,
  SPAN_LABELS,
  timeTicks,
} from './response.ts';

// SPPS's time step as the report carries it: 0.01 s through f32, widened.
const DT = Math.fround(0.01);

function report(opts: { solver?: string; emission?: number[]; zero?: boolean; pad?: number } = {}): Report {
  const tail = Array.from({ length: opts.pad ?? 0 }, () => 0);
  const series = (scale: number) => (opts.zero ? [0, 0, 0, 0, 0, 0] : [0, 0, 1e-2 * scale, 1e-4 * scale, 1e-6 * scale, 1e-9 * scale, ...tail]);
  const bands = (scale: number) => [
    { freq_hz: 500, energy_pa2: series(scale) },
    { freq_hz: 1000, energy_pa2: series(scale / 10) },
    { freq_hz: 1250, energy_pa2: series(scale / 100) },
  ];
  const emission = opts.emission ?? [0];
  return {
    solver: opts.solver ?? 'spps',
    bands_hz: [500, 1000, 1250],
    spps: {
      time_step_s: DT,
      sources: emission.map((e, i) => ({ name: `S${i + 1}`, emission_s: e })),
      point_receivers: [
        {
          label: 'R1',
          bands: bands(1),
          per_source: emission.length > 1 ? emission.map((_, i) => ({ source: `S${i + 1}`, bands: bands(1 + i) })) : [],
        },
      ],
    },
  } as unknown as Report;
}

test('the words: what the map is, and that it is not a pressure impulse response', () => {
  assert.equal(RESPONSE_TITLE, 'Energy response per band (SPPS echogram)');
  assert.match(RESPONSE_NOTE, /not a pressure impulse response/);
  assert.doesNotMatch(RESPONSE_TITLE + RESPONSE_NOTE, /\d|validated/i, 'no digit and no "validated" in the words');
  assert.deepEqual(
    SPAN_LABELS.map((l) => l.db),
    [0, -20, -40, -SPAN_DB],
  );
});

test('colourAt: black, deep red, red, white at the stops, linear between, clamped', () => {
  assert.deepEqual(colourAt(0), [0, 0, 0]);
  assert.deepEqual(colourAt(1 / 3), [0x3a, 0x0b, 0x10]);
  assert.deepEqual(colourAt(2 / 3), [0xe0, 0x20, 0x2e]);
  assert.deepEqual(colourAt(1), [255, 255, 255]);
  assert.deepEqual(colourAt(1 / 6), [29, 6, 8], 'halfway from black to deep red');
  assert.deepEqual(colourAt(5 / 6), [240, 144, 151], 'halfway from red to white');
  assert.deepEqual(colourAt(-1), [0, 0, 0]);
  assert.deepEqual(colourAt(2), [255, 255, 255]);
  assert.equal(COLOUR_STOPS.length, 4);
  // Monotone in lightness: a louder bin is never darker.
  let prev = -1;
  for (let i = 0; i <= 60; i++) {
    const [r, g, b] = colourAt(i / 60);
    const l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    assert.ok(l >= prev, `lightness at ${i}/60`);
    prev = l;
  }
  // say NO: a bin with no level has no colour.
  assert.throws(() => colourAt(Number.NaN), RangeError);
});

test('colourBarCss: the colour bar is the map stops at their fractions, bottom to top', () => {
  assert.equal(colourBarCss(), 'linear-gradient(to top, rgb(0, 0, 0) 0%, rgb(58, 11, 16) 33.3333%, rgb(224, 32, 46) 66.6667%, rgb(255, 255, 255) 100%)');
  // Say NO: the bar is not drawn top-down (white at the bottom would read the scale upside down).
  assert.ok(!colourBarCss().includes('to bottom'));
});

test('clipDb and levelT: 60 dB down and below read as the bottom; NaN and above the maximum refused', () => {
  assert.equal(clipDb(-12.5), -12.5);
  assert.equal(clipDb(0), 0);
  assert.equal(clipDb(-60), -60);
  assert.equal(clipDb(-60.01), -60);
  assert.equal(clipDb(-Infinity), -60);
  assert.equal(clipDb(-30, 20), -20);
  assert.equal(levelT(0), 1);
  assert.equal(levelT(-30), 0.5);
  assert.equal(levelT(-90), 0);
  // say NO
  assert.throws(() => clipDb(Number.NaN), RangeError);
  assert.throws(() => clipDb(3), RangeError, 'a level above the maximum');
  assert.throws(() => clipDb(-3, 0), RangeError, 'a span of 0');
});

test('dbMatrix: each bin re the largest; zeros are -Infinity; refuses ragged, negative, NaN, all-zero', () => {
  const m = dbMatrix([
    [0, 1, 0.1],
    [10, 0.01, 0],
  ]);
  assert.ok(m);
  assert.equal(m.max, 10);
  assert.deepEqual(m.at, { row: 1, col: 0 });
  assert.equal(m.db[1][0], 0);
  assert.ok(Math.abs(m.db[0][1] - -10) < 1e-12);
  assert.ok(Math.abs(m.db[0][2] - -20) < 1e-12);
  assert.ok(Math.abs(m.db[1][1] - -30) < 1e-12);
  assert.equal(m.db[0][0], -Infinity);
  // say NO
  assert.equal(dbMatrix([]), null);
  assert.equal(dbMatrix([[0, 0], [0, 0]]), null, 'no energy: no map');
  assert.equal(dbMatrix([[1, 2], [1]]), null, 'ragged');
  assert.equal(dbMatrix([[1, -2]]), null, 'negative energy');
  assert.equal(dbMatrix([[1, Number.NaN]]), null, 'NaN');
  assert.equal(dbMatrix([[1, Infinity]]), null, 'Infinity');
});

test('broadband and decayDb: the bands summed step by step, in dB re its maximum, clipped', () => {
  assert.deepEqual(broadband([[1, 2, 0], [3, 4, 0]]), [4, 6, 0]);
  const d = decayDb([10, 1, 1e-9, 0]);
  assert.ok(d);
  assert.equal(d[0], 0);
  assert.ok(Math.abs(d[1] - -10) < 1e-12);
  assert.equal(d[2], -60, '100 dB down reads as the bottom');
  assert.equal(d[3], -60);
  // say NO
  assert.equal(broadband([[1, 2], [1]]), null);
  assert.equal(broadband([]), null);
  assert.equal(decayDb([0, 0]), null);
});

test('mapPixels: one pixel per bin, the highest band on top, the maximum white, 60 dB down black', () => {
  const m = dbMatrix([
    [1, 1e-3, 1e-7],
    [1e-2, 1e-6, 0],
  ]);
  assert.ok(m);
  const px = mapPixels(m);
  assert.equal(px.length, 2 * 3 * 4);
  const pixel = (x: number, y: number) => [...px.slice((y * 3 + x) * 4, (y * 3 + x) * 4 + 4)];
  // Band 0 is the bottom image row (y = 1); band 1 the top (y = 0).
  assert.deepEqual(pixel(0, 1), [255, 255, 255, 255], 'the maximum is white');
  assert.deepEqual(pixel(2, 1), [0, 0, 0, 255], '70 dB down is black');
  assert.deepEqual(pixel(1, 0), [0, 0, 0, 255], '60 dB down is black');
  assert.deepEqual(pixel(2, 0), [0, 0, 0, 255], 'no energy is black');
  assert.deepEqual(pixel(0, 0), [...colourAt(levelT(-20)), 255], '20 dB down is red');
  assert.deepEqual(pixel(0, 0), [0xe0, 0x20, 0x2e, 255]);
  assert.deepEqual(pixel(1, 1), [...colourAt(levelT(-30)), 255]);
  // say NO: the band order is not the image order.
  assert.notDeepEqual(pixel(0, 0), [255, 255, 255, 255]);
});

test('timeTicks: a 1-2-5 interval, each tick a whole step, at the interval decimals', () => {
  const t = timeTicks(DT, 200);
  assert.deepEqual(t, [50, 100, 150, 200].map((j) => ({ j, digits: 1 })));
  // Every tick's text, the step times j, reads as the interval's multiple.
  assert.deepEqual(
    t.map((x) => (DT * x.j).toFixed(x.digits)),
    ['0.5', '1.0', '1.5', '2.0'],
  );
  assert.deepEqual(
    timeTicks(0.001, 120).map((x) => [x.j, x.digits]),
    [20, 40, 60, 80, 100, 120].map((j) => [j, 2]),
  );
  assert.deepEqual(timeTicks(Math.fround(0.003), 900).map((x) => x.j), [167, 333, 500, 667, 833]);
  // say NO: no step, no time axis.
  assert.deepEqual(timeTicks(0, 200), []);
  assert.deepEqual(timeTicks(DT, 0), []);
  assert.deepEqual(timeTicks(Number.NaN, 10), []);
});

test('mapPixels: drawn to a column count, the first columns of the full map', () => {
  const m = dbMatrix([[1, 1e-2, 1e-4, 1e-8]]);
  assert.ok(m);
  const full = mapPixels(m);
  const two = mapPixels(m, SPAN_DB, 2);
  assert.equal(two.length, 2 * 4);
  assert.deepEqual([...two], [...full.slice(0, 8)]);
  // say NO: no column count outside 1..cols.
  assert.throws(() => mapPixels(m, SPAN_DB, 0), RangeError);
  assert.throws(() => mapPixels(m, SPAN_DB, 5), RangeError);
});

test('niceInterval: the 1-2-5 interval for about `target` ticks over a span, and its decimals', () => {
  assert.deepEqual(niceInterval(2.73, 6), { interval: 0.5, digits: 1 });
  assert.deepEqual(niceInterval(10, 6), { interval: 2, digits: 0 });
  assert.deepEqual(niceInterval(0.063, 6), { interval: 0.02, digits: 2 });
});

/** One band, `cols` steps of `DT`, falling `perStep` dB a step from 0 dB at step 0. */
const falling = (cols: number, perStep: number) => [Array.from({ length: cols }, (_, c) => 10 ** ((-perStep * c) / 10))];
const fast = () => dbMatrix(falling(200, 10)) as NonNullable<ReturnType<typeof dbMatrix>>;

test('lastAbove: the last step with a bin above the floor in any band', () => {
  const m = dbMatrix([
    [1, 1e-3, 1e-7, 0],
    [1e-2, 1e-5, 1e-6, 0],
  ]);
  assert.ok(m);
  assert.equal(lastAbove(m), 1, 'step 2 is 70 dB and 60 dB down: not above');
  const end = dbMatrix([[1, 0, 1e-3]]);
  assert.ok(end);
  assert.equal(lastAbove(end), 2);
});

test('cropCols: the map ends after the last bin above the floor, rounded up to a clean tick, with a margin', () => {
  assert.equal(CROP_MARGIN, 0.05);
  // 10 dB a step: steps 0..5 are above -60 dB, step 6 is at it. The last above ends at 0.06 s;
  // 5 % more is 0.063 s, whose 1-2-5 interval is 0.02 s: the map ends at 0.08 s, 8 steps.
  const fast = dbMatrix(falling(200, 10));
  assert.ok(fast);
  assert.equal(cropCols(fast, DT), 8);
  assert.deepEqual(
    timeTicks(DT, 8).map((t) => t.j),
    [2, 4, 6, 8],
    'the end is a tick',
  );
  // CR4's shape: 1 ms steps over 10 s, a band above the floor up to 2.6 s (step 2599), the others
  // lower: 2.6 s plus 5 % is 2.73 s, interval 0.5 s, so the map ends at 3.0 s, step 3000.
  const ms = Math.fround(0.001);
  const cr4 = Array.from({ length: 3 }, (_, b) => Array.from({ length: 10_000 }, (_, c) => (c < 2600 - b * 500 ? 10 ** (-5.9 * (c / 2599)) : 1e-9)));
  const m = dbMatrix(cr4);
  assert.ok(m);
  assert.equal(cropCols(m, ms), 3000);
  assert.equal(timeTicks(ms, 3000).at(-1)?.j, 3000, 'the end is a tick');
  // Every bin after the end, and after the last above, is at the floor or below in every band.
  m.db.forEach((row) => row.slice(2600).forEach((d) => assert.ok(d <= -SPAN_DB)));
  // A bin exactly 60 dB down is not above the floor.
  const edge = dbMatrix([[1, 1e-6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]]);
  assert.ok(edge);
  // The last above is step 0, 0.0105 s with the margin; its interval, 0.002 s, is under a step,
  // so the end rounds up to whole steps: 2.
  assert.equal(cropCols(edge, DT), 2);
});

test('cropCols says NO: a series still above the floor at its last bin shows the full run', () => {
  // Above -60 dB to the last step: no crop.
  const slow = dbMatrix(falling(200, 0.2));
  assert.ok(slow);
  assert.equal(cropCols(slow, DT), null);
  // One band dies early, another is still above at the end: the full run.
  const mixed = dbMatrix([falling(200, 10)[0], falling(200, 0.25)[0].map((e) => e * 0.5)]);
  assert.ok(mixed);
  assert.equal(cropCols(mixed, DT), null);
  // The rounded end would reach the run's end: the full run (last above at step 195, 1.96 s,
  // 2.058 s with the margin, rounded up to 2.5 s, past the 2.0 s run).
  const late = dbMatrix([Array.from({ length: 200 }, (_, c) => (c <= 195 ? 1 : 0))]);
  assert.ok(late);
  assert.equal(cropCols(late, DT), null);
  // No step, no crop.
  assert.equal(cropCols(fast(), 0), null);
  assert.equal(cropCols(fast(), Number.NaN), null);
});

test('freqLabel: Hz below 1 kHz, kHz from it, as a path with its scale', () => {
  const r = report();
  assert.deepEqual(freqLabel(r, 'spps.point_receivers.0.bands.0.freq_hz'), {
    num: { path: 'spps.point_receivers.0.bands.0.freq_hz', digits: 0, text: '500' },
    unit: 'Hz',
  });
  assert.equal(freqLabel(r, 'spps.point_receivers.0.bands.1.freq_hz')?.num.text, '1');
  assert.equal(freqLabel(r, 'spps.point_receivers.0.bands.2.freq_hz')?.num.text, '1.25');
  assert.equal(freqLabel(r, 'spps.point_receivers.0.bands.2.freq_hz')?.num.scale, 0.001);
  // say NO
  assert.equal(freqLabel(r, 'spps.point_receivers.0.bands.9.freq_hz'), null);
});

test('responseView: the receiver series at their paths, every number a path of the report', () => {
  const r = report();
  const v = responseView(r, 0, null);
  assert.ok(v);
  assert.equal(v.receiver.text, 'R1');
  assert.equal(v.source, null);
  assert.equal(v.k0, 0);
  assert.deepEqual(
    v.bands.map((b) => b.path),
    [0, 1, 2].map((b) => `spps.point_receivers.0.bands.${b}.energy_pa2`),
  );
  v.bands.forEach((b, i) => assert.deepEqual(v.energy[i], at(r, b.path)));
  assert.deepEqual(v.map.at, { row: 0, col: 2 });
  assert.equal(v.broadband?.[2], 0);
  assert.equal(v.broadband?.[0], -60);
  // The time ticks: the report's step times a whole number, as data-scale.
  assert.ok(v.ticks.length > 0);
  for (const t of v.ticks) {
    assert.equal(t.num.path, 'spps.time_step_s');
    assert.equal(t.num.scale, t.col);
    assert.equal(t.num.text, ((at(r, t.num.path) as number) * t.col).toFixed(t.num.digits));
  }
  assert.equal(v.emission?.source.text, 'S1');
  assert.equal(v.emission?.at.path, 'spps.sources.0.emission_s');
  // The run's length, and where the floor is reached: R1's last bin above -60 dB is step 4 (the
  // 1.25 kHz band is 60 dB down there, the others above); the rounded end, 0.06 s, is the run's
  // end, so there is no crop.
  assert.deepEqual(v.run, { path: 'spps.time_step_s', digits: 2, scale: 6, text: '0.06' });
  assert.deepEqual(v.floor, { path: 'spps.time_step_s', digits: 2, scale: 5, text: '0.05' });
  assert.equal(v.crop, null);
  // say NO: a TCR run, a receiver not in the report, a source with no echogram here, no energy.
  assert.equal(responseView(report({ solver: 'tcr' }), 0, null), null);
  assert.equal(responseView(r, 3, null), null);
  assert.equal(responseView(r, 0, 'S1'), null, 'one source: no echogram per source');
  assert.equal(responseView(report({ zero: true }), 0, null), null);
});

test('responseView: the crop, its end a whole number of steps, its own ticks; the full run kept', () => {
  const r = report({ pad: 20 });
  const v = responseView(r, 0, null);
  assert.ok(v);
  assert.equal(v.map.db[0].length, 26, 'the map holds the full run');
  assert.deepEqual(v.run, { path: 'spps.time_step_s', digits: 2, scale: 26, text: '0.26' });
  assert.deepEqual(v.floor, { path: 'spps.time_step_s', digits: 2, scale: 5, text: '0.05' }, "at the step's decimals");
  assert.ok(v.crop);
  assert.equal(v.crop.cols, 6);
  assert.deepEqual(v.crop.end, { path: 'spps.time_step_s', digits: 2, scale: 6, text: '0.06' });
  assert.equal(v.crop.ticks.at(-1)?.col, 6, 'the end is a tick');
  for (const t of [...v.crop.ticks, ...v.ticks]) {
    assert.equal(t.num.path, 'spps.time_step_s');
    assert.equal(t.num.scale, t.col);
  }
  assert.ok(v.ticks.at(-1)!.col > 6, 'the full run keeps its own ticks');
  // say NO: a series still above the floor at its last bin has no floor and no crop.
  const live = report({ pad: 0 }) as unknown as { spps: { point_receivers: { bands: { energy_pa2: number[] }[] }[] } };
  live.spps.point_receivers[0].bands.forEach((b) => (b.energy_pa2[5] = b.energy_pa2[2]));
  const lv = responseView(live as unknown as Report, 0, null);
  assert.ok(lv);
  assert.equal(lv.floor, null);
  assert.equal(lv.crop, null);
});

test("responseView: a source's own echogram, from its emission; the sources summed from the first", () => {
  const r = report({ emission: [Math.fround(0.02), 0] });
  const v = responseView(r, 0, 'S1');
  assert.ok(v);
  assert.equal(v.source?.text, 'S1');
  assert.equal(v.source?.path, 'spps.sources.0.name');
  assert.equal(v.k0, 2, 'S1 emits two steps in');
  assert.equal(v.bands[0].path, 'spps.point_receivers.0.per_source.0.bands.0.energy_pa2');
  assert.deepEqual(v.energy[0], (at(r, v.bands[0].path) as number[]).slice(2));
  assert.deepEqual(v.map.at, { row: 0, col: 0 });
  const s2 = responseView(r, 0, 'S2');
  assert.equal(s2?.k0, 0);
  assert.equal(s2?.bands[0].path, 'spps.point_receivers.0.per_source.1.bands.0.energy_pa2');
  const summed = responseView(r, 0, null);
  assert.equal(summed?.k0, 0);
  assert.equal(summed?.emission?.source.text, 'S2', 'the first to emit');
  // say NO: a source the run does not have.
  assert.equal(responseView(r, 0, 'S9'), null);
});
