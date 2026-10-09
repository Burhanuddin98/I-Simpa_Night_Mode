// spread.ts under `node --test` (Burhan's 10-05 UI list, item 10: results spreading from the source).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { NO_REVEAL, REVEAL_MS, revealRadius, revealReach, speedOfSound, wavefrontInRoom, wavefrontRadius } from './spread.ts';

test('the speed of sound is the solver’s, from the project’s temperature', () => {
  assert.equal(speedOfSound(20), 343.2);
  assert.ok(Math.abs((speedOfSound(0) as number) - 343.2 * Math.sqrt(273.15 / 293.15)) < 1e-12);
  assert.equal(speedOfSound('20'), 343.2, 'a stored text number');
  assert.equal(speedOfSound('warm'), null);
  assert.equal(speedOfSound(''), null);
  assert.equal(speedOfSound(-300), null);
});

test('the reveal runs from the sources to past the farthest face, then is off (the map as before)', () => {
  assert.equal(revealRadius(0, 30), 0);
  assert.ok(revealRadius(REVEAL_MS / 2, 30) > 0 && revealRadius(REVEAL_MS / 2, 30) < 30);
  assert.ok(revealRadius(REVEAL_MS - 1, 30) > 29.9);
  assert.equal(revealRadius(REVEAL_MS, 30), NO_REVEAL);
  assert.equal(revealRadius(10, 0), NO_REVEAL, 'nothing to reach: no reveal');
  let last = -1;
  for (let ms = 0; ms < REVEAL_MS; ms += 50) {
    const r = revealRadius(ms, 30);
    assert.ok(r >= last, 'the front only moves outward');
    last = r;
  }
});

test('the reach is the farthest point from its nearest source', () => {
  const pts = [0, 0, 0, 10, 0, 0, 20, 0, 0];
  assert.equal(revealReach(pts, [[0, 0, 0]]), 20);
  assert.equal(revealReach(pts, [[0, 0, 0], [20, 0, 0]]), 10);
  assert.equal(revealReach(pts, []), 0);
});

test('the wavefront is c times the time since the emission, a record being its step’s end', () => {
  // CR4, 1 ms steps: step 4's direct particles lie 1.716 m from the source (measured, v1q-view bed).
  assert.ok(Math.abs((wavefrontRadius(4, 0, 0.001, 343.2) as number) - 1.716) < 1e-12);
  assert.ok(Math.abs((wavefrontRadius(12.5, 2.5, 0.001, 340) as number) - 3.74) < 1e-12);
  assert.ok(Math.abs((wavefrontRadius(2, 2, 0.001, 343) as number) - 0.343) < 1e-12, 'the emission step: one step out');
  assert.equal(wavefrontRadius(1, 2, 0.001, 343), null, 'before the emission');
  assert.equal(wavefrontRadius(5, 0, null, 343), null);
  assert.equal(wavefrontRadius(5, 0, 0.001, null), null);
});

test('the wavefront is drawn until it encloses the whole room box', () => {
  const lo = [0, 0, 0];
  const hi = [10, 10, 10];
  assert.ok(wavefrontInRoom([0, 0, 0], 17, lo, hi));
  assert.ok(!wavefrontInRoom([0, 0, 0], 17.4, lo, hi), 'past the far corner, 17.32 m away');
});
