import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { GLOW_MAX, glowLevel, glowRadius, GLOW_PEAK, pulsePhase, PULSE_MS, spriteScale, STILL_PHASE, withGlow } from './glow.ts';

// The installed three.js's own shader sources, read as text: the anchors are checked against what ships.
const three = dirname(dirname(createRequire(import.meta.url).resolve('three'))); // .../three/build/three.cjs
const shaderOf = (name: string): { vertex: string; fragment: string } => {
  const src = readFileSync(join(three, 'src/renderers/shaders/ShaderLib', `${name}.glsl.js`), 'utf8');
  const part = (k: string) => src.split(`export const ${k} = /* glsl */\``)[1].split('`;')[0];
  return { vertex: part('vertex'), fragment: part('fragment') };
};

test('the glow attaches to the matcap and Lambert shaders three.js ships', () => {
  for (const name of ['meshmatcap', 'meshlambert']) {
    const s = shaderOf(name);
    const g = withGlow(s.vertex, s.fragment);
    assert.match(g.vertex, /vGlowWorld = \(modelMatrix \* vec4\(transformed, 1\.0\)\)\.xyz;/, name);
    assert.match(g.fragment, new RegExp(`uniform vec3 glowPos\\[${GLOW_MAX}\\];`), name);
    // The glow is added to the colour before it is written, not after.
    assert.ok(g.fragment.indexOf('outgoingLight += glowColor') < g.fragment.indexOf('#include <opaque_fragment>'), name);
    assert.ok(g.fragment.indexOf('vec3 outgoingLight') < g.fragment.indexOf('outgoingLight += glowColor'), name);
  }
});

test('a shader without the anchors is refused, not passed through', () => {
  assert.throws(() => withGlow('void main() {}', '#include <common>\n#include <opaque_fragment>'), /vertex shader has no/);
  assert.throws(() => withGlow('#include <common>\n#include <project_vertex>', 'void main() {}'), /fragment shader has no/);
});

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
