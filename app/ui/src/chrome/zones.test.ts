import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { addFittingZone, opText, replaceFittingZone, setFittingBand } from '../ops.ts';
import { boxProblem, isBoxZone, newBoxZone, parseZoneValue, sameInEveryBand, withBound, withEveryBand, zoneEdges, ZONE_DEFAULTS } from './zones.ts';

const ROOM = { min: [0, 0, 0] as [number, number, number], max: [10, 6, 3] as [number, number, number] };

test('G29: a new box zone is 1 m, on the floor at the middle of the plan, with upstream values in every band', () => {
  const z = newBoxZone('z1', 'Fitting zone 1', ROOM, 3);
  assert.deepEqual(z.shape, { kind: 'box', min: [4.5, 2.5, 0], max: [5.5, 3.5, 1], destination: null });
  assert.deepEqual(z.absorption, [0, 0, 0]);
  assert.deepEqual(z.mean_free_path_m, [1, 1, 1]);
  assert.deepEqual(z.diffusion_law, ['uniform', 'uniform', 'uniform']);
  assert.equal(z.enabled, true);
  assert.equal(z.solver_id, null);
  assert.equal(ZONE_DEFAULTS.mean_free_path_m, 1);
  assert.equal(boxProblem(z.shape.min, z.shape.max, ROOM), null);
  assert.equal(opText(addFittingZone(0, z)).startsWith('{"op":"add_fitting_zone","index":0,"zone":{"id":"z1"'), true);
});

test('G28: a box with no volume, or reaching out of the room, is refused in words before it is sent', () => {
  assert.match(boxProblem([1, 1, 1], [1, 2, 2], ROOM) ?? '', /X from 1 m is not below its X to 1 m/);
  assert.match(boxProblem([1, 1, 1], [2, 7, 2], ROOM) ?? '', /outside the room's box on Y \(the room runs from 0 to 6 m\)/);
  assert.equal(boxProblem([0, 0, 0], [10, 6, 3], ROOM), null, 'the room box itself, walls included');
  assert.equal(boxProblem([0, 0, 0], [1, 1, 1], null), null, 'no room box: only the volume is checked');
  assert.match(boxProblem(['NaN', 0, 0], [1, 1, 1], ROOM) ?? '', /not a number/);
});

test('G28: a bound changes one corner on one axis, the destination flags kept', () => {
  const z = { ...newBoxZone('z1', 'Z', ROOM, 1), shape: { kind: 'box' as const, min: [1, 1, 0] as [number, number, number], max: [2, 2, 1] as [number, number, number], destination: ['max', 'min', 'max'] as ('min' | 'max')[] as ['min' | 'max', 'min' | 'max', 'min' | 'max'] } };
  const moved = withBound(z, 'max', 2, 1.5);
  assert.deepEqual(moved.shape, { kind: 'box', min: [1, 1, 0], max: [2, 2, 1.5], destination: ['max', 'min', 'max'] });
  assert.deepEqual(z.shape.max, [2, 2, 1], 'the original is not changed');
  assert.ok(isBoxZone(moved));
});

test('G28: every band at once, and the one value every band holds or none', () => {
  const z = newBoxZone('z1', 'Z', ROOM, 3);
  assert.deepEqual(withEveryBand(z, 'absorption', 0.4).absorption, [0.4, 0.4, 0.4]);
  assert.deepEqual(withEveryBand(z, 'diffusion_law', 'lambert_reflection').diffusion_law, ['lambert_reflection', 'lambert_reflection', 'lambert_reflection']);
  assert.equal(sameInEveryBand([0.4, 0.4]), 0.4);
  assert.equal(sameInEveryBand([0.4, 0.5]), null);
  assert.equal(sameInEveryBand([]), null);
  assert.equal(opText(setFittingBand('z1', 'mean_free_path', 2, 1.5)), '{"op":"set_fitting_band","zone":"z1","quantity":"mean_free_path","band":2,"value":1.5}');
  assert.ok(opText(replaceFittingZone(z)).startsWith('{"op":"replace_fitting_zone","zone":'));
});

test('G28: a typed absorption is 0 to 1, a mean free path above 0', () => {
  assert.deepEqual(parseZoneValue('0.35', 'absorption'), { ok: true, value: 0.35 });
  assert.equal(parseZoneValue('1.2', 'absorption').ok, false);
  assert.equal(parseZoneValue('0', 'mean_free_path_m').ok, false);
  assert.deepEqual(parseZoneValue('2', 'mean_free_path_m'), { ok: true, value: 2 });
  assert.equal(parseZoneValue('2,5', 'mean_free_path_m').ok, false);
});

test('G28: the view draws each enabled box zone as its 12 edges; a disabled zone or one of surfaces is not drawn', () => {
  const box = newBoxZone('z1', 'Stalls', ROOM, 1);
  const off = { ...newBoxZone('z2', 'Off', ROOM, 1), enabled: false };
  const surfaces = { ...newBoxZone('z3', 'Organ', ROOM, 1), shape: { kind: 'surfaces' as const, groups: ['g1'], inside_point: [1, 1, 1] as [number, number, number] } };
  const drawn = zoneEdges([box, off, surfaces]);
  assert.equal(drawn.length, 1);
  assert.equal(drawn[0].name, 'Stalls');
  assert.equal(drawn[0].segments.length, 12 * 6);
  // Every edge runs along one axis, the box's side long.
  for (let i = 0; i < drawn[0].segments.length; i += 6) {
    const d = [0, 1, 2].map((k) => Math.abs(drawn[0].segments[i + 3 + k] - drawn[0].segments[i + k]));
    assert.equal(d.filter((x) => x > 0).length, 1);
    assert.equal(Math.max(...d), 1);
  }
});
