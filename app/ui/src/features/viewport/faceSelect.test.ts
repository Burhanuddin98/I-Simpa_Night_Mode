// faceSelect.ts under `node --test` (parity G47: Ctrl/Shift multi-select and the drag box).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { BOX_MAX_SAMPLES, boxSamples, combineFaces, inRect, pickMode, rectOf } from './faceSelect.ts';

test('Ctrl (or Cmd) toggles, Shift adds, a plain click replaces', () => {
  assert.equal(pickMode({ ctrlKey: false, shiftKey: false }), 'replace');
  assert.equal(pickMode({ ctrlKey: true, shiftKey: false }), 'toggle');
  assert.equal(pickMode({ ctrlKey: false, shiftKey: false, metaKey: true }), 'toggle');
  assert.equal(pickMode({ ctrlKey: false, shiftKey: true }), 'add');
  assert.equal(pickMode({ ctrlKey: true, shiftKey: true }), 'toggle');
});

test('replace keeps only the pick; add keeps both, sorted and each once', () => {
  assert.deepEqual(combineFaces([4, 2], [9, 9, 1], 'replace'), [1, 9]);
  assert.deepEqual(combineFaces([4, 2], [9, 2], 'add'), [2, 4, 9]);
  assert.deepEqual(combineFaces([], [], 'add'), []);
});

test('toggle takes a picked face out, puts an unpicked one in', () => {
  assert.deepEqual(combineFaces([1, 2, 3], [2], 'toggle'), [1, 3]);
  assert.deepEqual(combineFaces([1, 3], [2], 'toggle'), [1, 2, 3]);
  assert.deepEqual(combineFaces([5], [5], 'toggle'), []);
});

test('a surface toggles as one: out when all of it is in, else all of it in', () => {
  assert.deepEqual(combineFaces([1, 2, 3, 7], [1, 2, 3], 'toggle'), [7]);
  assert.deepEqual(combineFaces([1, 7], [1, 2, 3], 'toggle'), [1, 2, 3, 7]);
});

test('a box spans its two corners whichever way it is dragged', () => {
  const r = rectOf({ x: 50, y: 10 }, { x: 20, y: 40 });
  assert.deepEqual(r, { left: 20, top: 10, right: 50, bottom: 40 });
  assert.ok(inRect(r, 20, 40) && inRect(r, 35, 25));
  assert.ok(!inRect(r, 19.9, 25) && !inRect(r, 35, 40.1));
});

test('the samples cover the box, never leave it, and stay under the cap', () => {
  const small = rectOf({ x: 0, y: 0 }, { x: 30, y: 12 });
  const s = boxSamples(small);
  assert.equal(s.length, 10 * 4, '3 px apart');
  assert.ok(s.every((p) => inRect(small, p.x, p.y)));
  const big = rectOf({ x: 0, y: 0 }, { x: 1900, y: 1000 });
  const b = boxSamples(big);
  assert.ok(b.length <= BOX_MAX_SAMPLES && b.length > BOX_MAX_SAMPLES / 2, `${b.length} samples`);
  assert.ok(b.every((p) => inRect(big, p.x, p.y)));
  assert.deepEqual(boxSamples(rectOf({ x: 5, y: 5 }, { x: 6, y: 5 })), [{ x: 5.5, y: 5 }]);
});
