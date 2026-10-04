import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { composite, flipRows, hexRgb, STRIP_PX, stripLines } from './snapshot.ts';

test('the frame over the window background: a premultiplied pixel plus the background behind what it leaves', () => {
  const bg: [number, number, number] = [9, 9, 11];
  // Opaque red, nothing, half-covered grey (premultiplied: 64 of 128 alpha).
  const px = new Uint8ClampedArray([255, 0, 0, 255, 0, 0, 0, 0, 64, 64, 64, 128]);
  const out = composite(px, bg);
  assert.deepEqual([...out.subarray(0, 4)], [255, 0, 0, 255]);
  assert.deepEqual([...out.subarray(4, 8)], [9, 9, 11, 255], 'an empty pixel is the background');
  const k = (v: number, b: number) => Math.min(255, v + Math.round((b * (255 - 128)) / 255));
  assert.deepEqual([...out.subarray(8, 12)], [k(64, 9), k(64, 9), k(64, 11), 255]);
  // say NO: the input is not changed in place.
  assert.deepEqual([...px.subarray(4, 8)], [0, 0, 0, 0]);
});

test('rows read from the GPU bottom first are turned top first', () => {
  // 1 x 2: bottom row blue, top row green.
  const gl = new Uint8ClampedArray([0, 0, 255, 255, 0, 255, 0, 255]);
  assert.deepEqual([...flipRows(gl, 1, 2)], [0, 255, 0, 255, 0, 0, 255, 255]);
  assert.deepEqual(hexRgb('#09090b'), [9, 9, 11]);
  assert.equal(hexRgb('red'), null, 'say NO: not a hex colour');
});

test('the strip holds the legend shown and nothing else: no map, no strip', () => {
  assert.equal(stripLines(null), null);
  const l = stripLines({
    project: 'Outputs box',
    run: 'Run 2',
    legend: { title: 'Surface receivers · level · 1 kHz', lo: '40', mid: '55', hi: '70 dB', gradient: 'x' },
    time: '120 ms',
    note: null,
  });
  assert.deepEqual(l, {
    heading: 'Outputs box · Run 2 · 120 ms',
    title: 'Surface receivers · level · 1 kHz',
    lo: '40',
    mid: '55',
    hi: '70 dB',
    note: null,
  });
  assert.ok(STRIP_PX >= 56);
});
