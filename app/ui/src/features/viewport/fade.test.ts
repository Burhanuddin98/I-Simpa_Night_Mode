import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { fadeRange } from './fade.ts';

test('the fade runs across the bounding sphere, from its near side to its far side', () => {
  assert.deepEqual(fadeRange(50, 20), { near: 30, far: 70 });
  // From inside the room it starts at the camera.
  assert.deepEqual(fadeRange(5, 20), { near: 0, far: 25 });
});
