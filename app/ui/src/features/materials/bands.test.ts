import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { bandColumns, bandLabel, compareF64, naturalCompare, parseBandLabel } from './bands.ts';

const THIRDS = [50, 63, 80, 100, 125, 160, 200, 250, 315, 400, 500, 630, 800, 1000, 1250, 1600, 2000, 2500, 3150, 4000, 5000, 6300, 8000, 10000, 12500, 16000, 20000];

test('octave labels read 125 · 250 · 500 · 1k · 2k · 4k', () => {
  assert.deepEqual(bandColumns([125, 250, 500, 1000, 2000, 4000]).map((c) => c.label), ['125', '250', '500', '1k', '2k', '4k']);
});

test('third-octave labels read 50 … 1.25k … 20k, in ascending order', () => {
  const cols = bandColumns(THIRDS);
  assert.deepEqual(cols.map((c) => c.label), [
    '50', '63', '80', '100', '125', '160', '200', '250', '315', '400', '500', '630', '800',
    '1k', '1.25k', '1.6k', '2k', '2.5k', '3.15k', '4k', '5k', '6.3k', '8k', '10k', '12.5k', '16k', '20k',
  ]);
  assert.deepEqual(cols.map((c) => c.index), THIRDS.map((_, i) => i));
});

test('columns sort by frequency, never by label text, and keep their band index', () => {
  const cols = bandColumns([1000, 125, 4000, 250]);
  assert.deepEqual(cols.map((c) => c.hz), [125, 250, 1000, 4000]);
  assert.deepEqual(cols.map((c) => c.index), [1, 3, 0, 2]);
});

test('band labels read with a unit or a k suffix; bare numbers only when allowed', () => {
  for (const [text, hz] of [
    ['125 Hz', 125],
    ['125Hz', 125],
    ['125 hz', 125],
    ['1k', 1000],
    ['1K', 1000],
    ['1 kHz', 1000],
    ['1kHz', 1000],
    ['1.25k', 1250],
    ['3.15k', 3150],
    ['6.3 kHz', 6300],
    ['12.5k', 12500],
    ['1250 Hz', 1250],
    [' 4 kHz ', 4000],
  ] as const) {
    assert.equal(parseBandLabel(text), hz, text);
  }
  for (const s of THIRDS) assert.equal(parseBandLabel(bandLabel(s) + (s < 1000 ? ' Hz' : '')), s, `label of ${s}`);
  assert.equal(parseBandLabel('125'), null);
  assert.equal(parseBandLabel('125', true), 125);
  for (const bad of ['', 'Hz', 'k', '0,5', 'abc', '1 MHz', '-125 Hz', '1e3 Hz', 'Material']) {
    assert.equal(parseBandLabel(bad, true), null, bad);
  }
});

test('natural sort puts Material 2 before Material 10', () => {
  const names = ['Material 10', 'Material 2', 'material 1', 'Material 02b', 'Glass', 'Material 2a', 'Material'];
  assert.deepEqual(names.slice().sort(naturalCompare), ['Glass', 'Material', 'material 1', 'Material 2', 'Material 2a', 'Material 02b', 'Material 10']);
  assert.ok(naturalCompare('Material 2', 'Material 10') < 0);
  assert.ok(naturalCompare('a', 'A') !== 0, 'the order is total');
  assert.equal(naturalCompare('same', 'same'), 0);
});

test('band values sort ascending, non-finite values last', () => {
  const v = [0.5, 'NaN', 0.02, 'inf', 1];
  assert.deepEqual(v.slice().sort(compareF64), [0.02, 0.5, 1, 'NaN', 'inf']);
});
