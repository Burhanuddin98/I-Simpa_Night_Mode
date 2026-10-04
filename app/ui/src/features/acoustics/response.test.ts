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
  colourAt,
  colourBarCss,
  dbMatrix,
  decayDb,
  freqLabel,
  levelT,
  mapPixels,
  RESPONSE_NOTE,
  RESPONSE_TITLE,
  responseView,
  SPAN_DB,
  SPAN_LABELS,
  timeTicks,
} from './response.ts';

// SPPS's time step as the report carries it: 0.01 s through f32, widened.
const DT = Math.fround(0.01);

function report(opts: { solver?: string; emission?: number[]; zero?: boolean } = {}): Report {
  const series = (scale: number) => (opts.zero ? [0, 0, 0, 0, 0, 0] : [0, 0, 1e-2 * scale, 1e-4 * scale, 1e-6 * scale, 1e-9 * scale]);
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
  // say NO: a TCR run, a receiver not in the report, a source with no echogram here, no energy.
  assert.equal(responseView(report({ solver: 'tcr' }), 0, null), null);
  assert.equal(responseView(r, 3, null), null);
  assert.equal(responseView(r, 0, 'S1'), null, 'one source: no echogram per source');
  assert.equal(responseView(report({ zero: true }), 0, null), null);
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
