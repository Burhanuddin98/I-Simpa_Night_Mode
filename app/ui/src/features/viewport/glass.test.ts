import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { EDGE_ON, FACE_ON, glassFactor } from './glass.ts';

test('a wall seen face-on is clearer than the slider, one seen edge-on more solid', () => {
  assert.equal(glassFactor(1), FACE_ON);
  assert.equal(glassFactor(-1), FACE_ON);
  assert.equal(glassFactor(0), EDGE_ON);
  assert.ok(glassFactor(0.5) > FACE_ON && glassFactor(0.5) < EDGE_ON);
});

