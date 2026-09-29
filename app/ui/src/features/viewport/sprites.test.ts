import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { BG, glowPixels, RED, ringPixels, WHITE } from './sprites.ts';

const at = (px: Uint8Array, size: number, i: number, j: number) => [...px.subarray(4 * (j * size + i), 4 * (j * size + i) + 4)];

test('the source glow: an opaque red centre that fades to nothing at the rim', () => {
  const n = 64;
  const px = glowPixels(n);
  assert.equal(px.length, n * n * 4);
  assert.deepEqual(at(px, n, 32, 32), [...RED, 255]);
  assert.equal(at(px, n, 0, 0)[3], 0, 'corner');
  assert.equal(at(px, n, 0, 32)[3], 0, 'rim');
  // Monotone fall-off along a radius, outside the dot.
  let last = 256;
  for (let i = 32; i < 64; i++) {
    const a = at(px, n, i, 32)[3];
    assert.ok(a <= last, `alpha rises at x = ${i}`);
    last = a;
  }
  // A glow, not a hard disc: something between the dot and the rim is partly transparent.
  assert.ok([...Array(20).keys()].some((k) => {
    const a = at(px, n, 44 + k / 2, 32)[3];
    return a > 10 && a < 200;
  }));
});

test('a receiver: a white ring around a dark fill, transparent outside', () => {
  const n = 32;
  const px = ringPixels(n, 0.55, 0.85, WHITE, BG);
  assert.deepEqual(at(px, n, 16, 16), [...BG, 255], 'fill');
  assert.equal(at(px, n, 0, 0)[3], 0, 'outside');
  // Across the ring band (radius 0.7 of the half width: about 11 px from the centre).
  assert.deepEqual(at(px, n, 16 + 11, 16), [...WHITE, 255]);
});

test('an unfilled ring is transparent in its hole', () => {
  const n = 32;
  const px = ringPixels(n, 0.7, 0.95, RED, null);
  assert.equal(at(px, n, 16, 16)[3], 0);
  assert.equal(at(px, n, 16 + 13, 16)[3], 255);
});
