import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { assignMaterial, batch, moveReceiver, nextName, opText, setMaterialBand } from './ops.ts';

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
