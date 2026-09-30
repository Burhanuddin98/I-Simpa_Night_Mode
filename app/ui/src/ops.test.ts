import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { Material } from './bindings/schema.ts';
import {
  assignMaterial,
  batch,
  libraryMaterial,
  moveReceiver,
  nextName,
  opText,
  replaceMaterial,
  setMaterialBand,
  setSourceEnabled,
  withLaw,
} from './ops.ts';

test('opText keeps the sign of -0 and writes it as -0.0', () => {
  const text = opText(moveReceiver('r', [-0, 0, 1.5]));
  assert.equal(text, '{"op":"move_point_receiver","id":"r","position":[-0.0,0,1.5]}');
  assert.equal(JSON.stringify(-0), '0', 'the reason opText exists');
});

test('opText refuses a non-finite number and keeps the schema strings for them', () => {
  assert.throws(() => opText(setMaterialBand('m', 'absorption', 0, NaN)), /not a finite number/);
  assert.throws(() => opText(setMaterialBand('m', 'absorption', 0, Infinity)), /not a finite number/);
  assert.equal(
    opText(setMaterialBand('m', 'absorption', 2, 'NaN')),
    '{"op":"set_material_band","material":"m","quantity":"absorption","band":2,"value":"NaN"}',
  );
});

test('opText writes the shortest round-trip decimal, the bits intact', () => {
  const v = Number('0.39509836780726515');
  const text = opText(setMaterialBand('m', 'absorption', 5, v));
  assert.ok(text.includes('"value":0.39509836780726515'), text);
  assert.equal(JSON.parse(text).value, v);
});

test('opText nests batches and skips undefined keys', () => {
  const op = batch([moveReceiver('a', [1, 2, 3]), assignMaterial('g', 'm', null)]);
  assert.equal(
    opText(op),
    '{"op":"batch","ops":[{"op":"move_point_receiver","id":"a","position":[1,2,3]},' +
      '{"op":"set_group_material","group":"g","material":"m"}]}',
  );
  assert.equal(opText({ op: 'set_active_variant', variant: null }), '{"op":"set_active_variant","variant":null}');
});

test('assignMaterial writes the base or the active variant', () => {
  assert.deepEqual(assignMaterial('g', 'm', null), { op: 'set_group_material', group: 'g', material: 'm' });
  assert.deepEqual(assignMaterial('g', 'm', 'v'), { op: 'set_variant_override', variant: 'v', group: 'g', material: 'm' });
});

test('nextName takes the first free number', () => {
  assert.equal(nextName('R', ['R1', 'R2', 'R3']), 'R4');
  assert.equal(nextName('R', ['R1', 'R3']), 'R2');
  assert.equal(nextName('S', []), 'S1');
});

test('setSourceEnabled and replaceMaterial are the core ops', () => {
  assert.equal(opText(setSourceEnabled('s', false)), '{"op":"set_source_enabled","id":"s","enabled":false}');
  const m = libraryMaterial({ reference_id: 21, name: '30% absorbing', absorption: 0.3, color: '#b2b2b2' }, 'm1', 2);
  assert.equal(
    opText(replaceMaterial(m)),
    '{"op":"replace_material","material":{"id":"m1","name":"30% absorbing","color":"#b2b2b2",' +
      '"absorption":[0.3,0.3],"scattering":[0,0],"reflection_law":"specular",' +
      '"transmission_loss_db":null,"double_sided":true,"solver_id":null}}',
  );
});

test('a library material carries the core value in every band, nothing pinned', () => {
  const m = libraryMaterial({ reference_id: 22, name: '20% absorbing', absorption: 0.2, color: '#cccccc' }, 'x', 6);
  assert.deepEqual(m.absorption, [0.2, 0.2, 0.2, 0.2, 0.2, 0.2]);
  assert.deepEqual(m.scattering, [0, 0, 0, 0, 0, 0]);
  assert.equal(m.solver_id, null);
  assert.equal(m.reflection_law, 'specular');
});

test('withLaw sets one law for every band, a per-band law included', () => {
  const m = libraryMaterial({ reference_id: 22, name: 'x', absorption: 0.2, color: '#cccccc' }, 'x', 2);
  const perBand: Material = { ...m, reflection_law: ['specular', 'lambert'] as unknown as Material['reflection_law'] };
  assert.equal(withLaw(perBand, 'lambert').reflection_law, 'lambert');
  assert.equal(withLaw(m, 'w2').reflection_law, 'w2');
  assert.equal(m.reflection_law, 'specular', 'the input is not changed');
});
