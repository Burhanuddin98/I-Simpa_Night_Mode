import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fadeRange, withFade } from './fade.ts';

const three = dirname(dirname(createRequire(import.meta.url).resolve('three')));
const shaderOf = (name: string): { vertex: string; fragment: string } => {
  const src = readFileSync(join(three, 'src/renderers/shaders/ShaderLib', `${name}.glsl.js`), 'utf8');
  const part = (k: string) => src.split(`export const ${k} = /* glsl */\``)[1].split('`;')[0];
  return { vertex: part('vertex'), fragment: part('fragment') };
};

test('the fade attaches to the surface shaders (matcap, Lambert) and the edges (basic)', () => {
  for (const [name, mode] of [['meshmatcap', 'mix'], ['meshlambert', 'mix'], ['meshbasic', 'alpha']] as const) {
    const s = shaderOf(name);
    const g = withFade(s.vertex, s.fragment, mode);
    // The depth is read after project_vertex has made mvPosition.
    assert.ok(g.vertex.indexOf('vNmDepth = -mvPosition.z;') > g.vertex.indexOf('#include <project_vertex>'), name);
    const at = g.fragment.indexOf(mode === 'mix' ? 'outgoingLight = mix(outgoingLight, nmFadeColor' : 'diffuseColor.a *= 1.0 - nmFadeMax');
    assert.ok(at > g.fragment.indexOf('vec3 outgoingLight') && at < g.fragment.indexOf('#include <opaque_fragment>'), name);
  }
  assert.throws(() => withFade('void main() {}', '#include <common>\n#include <opaque_fragment>', 'mix'), /vertex shader has no/);
});

test('the fade runs across the bounding sphere, from its near side to its far side', () => {
  assert.deepEqual(fadeRange(50, 20), { near: 30, far: 70 });
  // From inside the room it starts at the camera.
  assert.deepEqual(fadeRange(5, 20), { near: 0, far: 25 });
});
