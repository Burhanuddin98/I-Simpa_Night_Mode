// faces.ts under `node --test` (parity G43).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { FACE_SHOWS, facePlan, faceShowOf } from './faces.ts';

test('Inside, the default: the back faces, the near walls gone, picks on the drawn side', () => {
  assert.deepEqual(facePlan('inside', false, false), { drawn: true, bothSides: false, glass: false, pickSide: 'back', occludes: true });
  assert.equal(facePlan('inside', true, false).glass, true, 'See-through puts the glass back over the near walls');
});

test('Outside: every face drawn, both sides, no glass; a pick meets the nearest face', () => {
  assert.deepEqual(facePlan('outside', true, false), { drawn: true, bothSides: true, glass: false, pickSide: 'double', occludes: true });
});

test('None: no faces drawn, so nothing hides a marker; a pick still finds the nearest face', () => {
  assert.deepEqual(facePlan('none', true, false), { drawn: false, bothSides: false, glass: false, pickSide: 'double', occludes: false });
});

test('a plan always draws the inside: from outside it would show only the roof', () => {
  assert.deepEqual(facePlan('outside', false, true), facePlan('inside', false, false));
  assert.deepEqual(facePlan('none', true, true), facePlan('inside', true, false));
});

test('upstream’s three choices, Inside first; a stored value that is none of them reads Inside', () => {
  assert.deepEqual(
    FACE_SHOWS.map((f) => f.label),
    ['Inside', 'Outside', 'None'],
  );
  assert.equal(faceShowOf('outside'), 'outside');
  assert.equal(faceShowOf('none'), 'none');
  assert.equal(faceShowOf(undefined), 'inside');
  assert.equal(faceShowOf('glass'), 'inside');
});
