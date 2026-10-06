import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { buildHeight, NO_CUT, swingAngle, withBuild } from './build.ts';

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

test('the cut attaches to the surface and edge shaders three.js ships, and refuses others', () => {
  const three = dirname(dirname(createRequire(import.meta.url).resolve('three')));
  for (const [name, mode] of [['meshmatcap', 'surface'], ['meshlambert', 'surface'], ['meshbasic', 'line']] as const) {
    const src = readFileSync(join(three, 'src/renderers/shaders/ShaderLib', `${name}.glsl.js`), 'utf8');
    const part = (k: string) => src.split(`export const ${k} = /* glsl */\``)[1].split('`;')[0];
    const g = withBuild(part('vertex'), part('fragment'), mode);
    assert.ok(g.fragment.indexOf('if (vNmWorldZ > nmBuildZ) discard;') < g.fragment.indexOf('#include <opaque_fragment>'), name);
    assert.equal(g.fragment.includes('outgoingLight += vec3(1.0, 0.86, 0.78)'), mode === 'surface', name);
  }
  assert.throws(() => withBuild('void main() {}', '#include <common>\n#include <opaque_fragment>', 'line'), /vertex shader has no/);
});
