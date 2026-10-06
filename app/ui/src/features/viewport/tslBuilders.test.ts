import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { Vector3 } from 'three';
import { aoFactor } from './ao.ts';
import { buildKeep, buildLine } from './build.ts';
import { fadeAmount } from './fade.ts';
import { glassOpacity } from './glass.ts';
import { GLOW_MAX, glowTerm } from './glow.ts';
import { groundColour } from './ground.ts';
import { T } from './tsl.ts';

// The surface effects are TSL node graphs (decision 68 (b)): built here without a GPU, so a three.js
// upgrade that renames or drops a node they use fails a test, not the first frame on screen. Whether
// they compile is the GPU's to say (the e2e's pixel hooks).
test('the surface effects build as TSL nodes from the uniforms the engine passes', () => {
  const u = (v: unknown) => T.uniform(v);
  const nodes = [
    aoFactor(u(1)),
    buildKeep(u(1e9)),
    buildLine(u(1e9), u(0.3)),
    fadeAmount({ near: u(0), far: u(1), max: u(0) }),
    glassOpacity(T.materialOpacity),
    glowTerm({ pos: T.uniformArray(Array.from({ length: GLOW_MAX }, () => new Vector3()), 'vec3'), count: T.uniform(0, 'int'), radius: u(1.5), level: u(0), color: u(new Vector3(1, 0.22, 0.08)) }),
    groundColour({ centre: u(new Vector3()), half: u(1), step: u(1), footMin: u(new Vector3()), footMax: u(new Vector3()), soft: u(1), line: u(new Vector3(1, 1, 1)), lineA: u(0.14), shadowA: u(0.55) }),
  ];
  for (const n of nodes) assert.equal(n?.isNode, true);
});
