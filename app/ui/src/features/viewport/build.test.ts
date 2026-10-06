import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { buildHeight, NO_CUT, swingAngle } from './build.ts';

test('the cut rises from the floor past the ceiling, then lifts away', () => {
  assert.equal(buildHeight(0, 2, 18), 2);
  let last = -Infinity;
  for (let f = 0; f < 1; f += 0.05) {
    const h = buildHeight(f, 2, 18);
    assert.ok(h >= last, `${f}`);
    last = h;
  }
  assert.ok(buildHeight(0.999, 2, 18) > 18 - 0.01);
  assert.equal(buildHeight(1, 2, 18), NO_CUT);
});

test('the camera starts swung back and ends on its framing', () => {
  assert.ok(Math.abs(swingAngle(0) + (25 * Math.PI) / 180) < 1e-12);
  assert.equal(swingAngle(1), -0);
});

