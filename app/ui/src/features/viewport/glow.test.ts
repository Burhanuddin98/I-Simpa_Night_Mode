import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { glowLevel, glowRadius, GLOW_PEAK, pulsePhase, PULSE_MS, spriteScale, STILL_PHASE } from './glow.ts';

test('the radius is a tenth of the longest side, between 1.5 and 8 m', () => {
  const box = (x: number, y: number, z: number) => ({ min: [0, 0, 0] as [number, number, number], max: [x, y, z] as [number, number, number] });
  assert.ok(Math.abs(glowRadius(box(33.35, 33.13, 14.27)) - 3.335) < 1e-9);
  assert.equal(glowRadius(box(5, 4, 3)), 1.5);
  assert.equal(glowRadius(box(200, 50, 20)), 8);
  assert.equal(glowRadius(null), 1.5);
});

test('the pulse runs dim to bright and back, never out, and holds the sprite size when still', () => {
  assert.equal(pulsePhase(0), 0);
  assert.ok(Math.abs(pulsePhase(PULSE_MS / 2) - 1) < 1e-12);
  assert.ok(Math.abs(pulsePhase(PULSE_MS)) < 1e-12);
  assert.equal(glowLevel(0), 0.75 * GLOW_PEAK);
  assert.equal(glowLevel(1), GLOW_PEAK);
  assert.equal(spriteScale(STILL_PHASE), 1);
});
