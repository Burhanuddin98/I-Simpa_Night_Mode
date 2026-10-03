import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { Particles } from '../../resultsData.ts';
import { advance, type AnimatorState } from './animator.ts';
import { aliveCounts, noParticlesText, PARTICLE_VERTEX_GLSL, recordSteps } from './particles.ts';

/** A decoded PART: per particle its first step and its record count. */
function particles(spec: [number, number][], maxSteps = 10): Particles {
  const offsets = new Uint32Array(spec.length + 1);
  spec.forEach(([, n], i) => (offsets[i + 1] = offsets[i] + n));
  const nr = offsets[spec.length];
  return {
    particleCount: spec.length,
    recordCount: nr,
    maxSteps,
    timeStepS: Math.fround(0.01),
    bandHz: 1000,
    firstStep: new Uint32Array(spec.map(([f]) => f)),
    offsets,
    positions: new Float32Array(3 * nr),
    energies: new Float32Array(nr).fill(1),
  };
}

test('a particle is alive at the steps it has a record for: first step + k', () => {
  const p = particles([
    [0, 3],
    [2, 2],
    [2, 0],
    [7, 3],
  ]);
  assert.deepEqual([...recordSteps(p)], [0, 1, 2, 2, 3, 7, 8, 9]);
  assert.deepEqual([...aliveCounts(p, 10)], [1, 1, 2, 1, 0, 0, 0, 1, 1, 1]);
  // Records past the run's last step are not counted at a step that does not exist.
  assert.deepEqual([...aliveCounts(p, 8)], [1, 1, 2, 1, 0, 0, 0, 1]);
});

test('no particles saved: what it says, and how to turn it on with the file size', () => {
  const t = noParticlesText(10_000, 1);
  assert.equal(t.title, 'No particles saved for this run');
  assert.match(t.how, /Simulate/);
  assert.match(t.how, /Particles saved for playback/);
  // 1,000 particles x 10,000 steps x 16 bytes x 1 source = 160 MB a band (upstream's formula).
  assert.match(t.how, /1,000 .*160 MB/);
  assert.match(noParticlesText(100, 3).how, /4\.8 MB/);
});

test('the shared timeline: steps advance at its rate, stop at the end, and a step is set in range', () => {
  const s: AnimatorState = { step: 0, steps: 100, playing: true, stepsPerSecond: 50, carry: 0 };
  const a = advance(s, 100);
  assert.equal(a.step, 5);
  const b = advance({ ...a }, 10);
  assert.equal(b.step, 5, 'half a step carried, not dropped');
  assert.equal(advance(b, 10).step, 6);
  const end = advance({ ...s, step: 98 }, 1000);
  assert.equal(end.step, 99);
  assert.equal(end.playing, false, 'playback stops at the last step');
  assert.deepEqual(advance({ ...s, playing: false }, 1000), { ...s, playing: false }, 'paused: nothing moves');
});

test('count mode (gate (d)) and the draw keep the same records: one kept() gate, then the count branch, no cull after it', () => {
  // Assay (M12, LOW): count mode returned before the draw's own code, so a cull the draw made
  // after that point (on energy, say) would not be counted. Every condition on the data is
  // now one function both modes pass through first.
  const glsl = PARTICLE_VERTEX_GLSL.replace(/\/\/[^\n]*/g, '');
  const kept = /bool kept\(\)\s*\{([\s\S]*?)\n\s*\}/.exec(glsl);
  assert.ok(kept, 'a kept() function holds the records drawn');
  // Alive is the .pbin's own definition (particles.ts): a record at its step. Energy is colour,
  // never a cull: a record alive by the file is drawn and counted whatever its energy.
  assert.match(kept[1], /aStep/);
  assert.doesNotMatch(kept[1], /aEnergy/);
  const main = glsl.slice(glsl.indexOf('void main()'));
  const gate = /if \(!kept\(\)\) \{[^}]*\}/.exec(main);
  assert.ok(gate, 'main starts at the kept() gate');
  assert.match(main.slice(0, gate.index), /^void main\(\) \{\s*$/, 'nothing before the gate');
  const afterGate = main.slice(gate.index + gate[0].length);
  const count = /^\s*if \(uCount > 0\.5\) \{[^}]*\}/.exec(afterGate);
  assert.ok(count, 'the count branch follows the gate directly');
  const draw = afterGate.slice(count[0].length);
  for (const cull of [/return/, /discard/, /gl_PointSize\s*=\s*0\.0/, /gl_Position\s*=\s*vec4\(2\.0/, /if\s*\(/]) {
    assert.doesNotMatch(draw, cull, `the draw culls after the count branch: ${cull}`);
  }
});
