import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { EDGE_ON, FACE_ON, glassFactor, withGlass } from './glass.ts';

test('a wall seen face-on is clearer than the slider, one seen edge-on more solid', () => {
  assert.equal(glassFactor(1), FACE_ON);
  assert.equal(glassFactor(-1), FACE_ON);
  assert.equal(glassFactor(0), EDGE_ON);
  assert.ok(glassFactor(0.5) > FACE_ON && glassFactor(0.5) < EDGE_ON);
});

test('the scaling attaches to the matcap shader three.js ships, after the view direction and the normal exist', () => {
  const three = dirname(dirname(createRequire(import.meta.url).resolve('three')));
  const src = readFileSync(join(three, 'src/renderers/shaders/ShaderLib/meshmatcap.glsl.js'), 'utf8');
  const fragment = src.split('export const fragment = /* glsl */`')[1].split('`;')[0];
  const g = withGlass(fragment);
  const at = g.indexOf('diffuseColor.a = min(');
  assert.ok(at > g.indexOf('vec3 viewDir = normalize( vViewPosition );'));
  assert.ok(at > g.indexOf('#include <normal_fragment_begin>'));
  assert.ok(at < g.indexOf('#include <opaque_fragment>'));
  assert.throws(() => withGlass('void main() {}'), /matcap fragment shader has no/);
});
