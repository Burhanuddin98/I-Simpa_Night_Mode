import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { bgraToRgba, composite, flipRows, hexRgb, over, STRIP_PX, stripLines, unpadRows } from './snapshot.ts';

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

test("WebGPU's padded copy rows packed tight, and its BGRA turned RGBA", () => {
  // 3 pixels a row (12 bytes) padded to 16, two rows.
  const src = new Uint8Array(16 + 12);
  for (let i = 0; i < 12; i++) {
    src[i] = i + 1;
    src[16 + i] = 101 + i;
  }
  src.fill(255, 12, 16);
  const out = unpadRows(src, 3, 2, 16);
  assert.deepEqual([...out], [...Array.from({ length: 12 }, (_, i) => i + 1), ...Array.from({ length: 12 }, (_, i) => 101 + i)]);
  assert.deepEqual([...unpadRows(new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8]), 1, 2, 4)], [1, 2, 3, 4, 5, 6, 7, 8], 'no padding: the bytes as they are');
  assert.throws(() => unpadRows(new Uint8Array(8), 3, 1, 8), RangeError);
  assert.throws(() => unpadRows(new Uint8Array(20), 3, 2, 16), RangeError);
  assert.deepEqual([...bgraToRgba(new Uint8Array([10, 20, 30, 40, 1, 2, 3, 4]))], [30, 20, 10, 40, 3, 2, 1, 4]);
});

test('R63: a chart canvas (straight alpha) over the panel background: c·a + bg·(1 − a), opaque', () => {
  const bg: [number, number, number] = [9, 9, 11];
  // Opaque red, nothing, white at half alpha (straight: the colour is not scaled by its alpha).
  const px = new Uint8ClampedArray([224, 32, 46, 255, 0, 0, 0, 0, 255, 255, 255, 128]);
  const out = over(px, bg);
  assert.deepEqual([...out.subarray(0, 4)], [224, 32, 46, 255]);
  assert.deepEqual([...out.subarray(4, 8)], [9, 9, 11, 255], 'an empty pixel is the background');
  const k = (v: number, b: number) => Math.round((v * 128 + b * 127) / 255);
  assert.deepEqual([...out.subarray(8, 12)], [k(255, 9), k(255, 9), k(255, 11), 255]);
  // say NO: composite (premultiplied) would add the background to a full white and clip it.
  assert.notDeepEqual([...composite(px, bg).subarray(8, 12)], [...out.subarray(8, 12)]);
});
