import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { LibraryMaterial } from '../../bindings/ipc.ts';
import { addedMaterial, libraryRows } from './library.ts';

// Three entries as the core gives them: the f32-widened absorption, not the decimal.
const lib: LibraryMaterial[] = [
  { reference_id: 24, name: '100% absorbing', absorption: 1, color: '#050505' },
  { reference_id: 21, name: '30% absorbing', absorption: Math.fround(0.3), color: '#b2b2b2' },
  { reference_id: 23, name: '0% absorbing', absorption: 0, color: '#ffffff' },
];

test("the rows keep the core's order and entries, untouched", () => {
  const rows = libraryRows(lib, []);
  assert.deepEqual(
    rows.map((r) => r.entry.reference_id),
    [24, 21, 23],
  );
  // The values are the core's objects, passed through (never retyped or rounded here).
  rows.forEach((r, i) => assert.equal(r.entry, lib[i]));
  assert.equal(rows[1].entry.absorption, 0.30000001192092896);
  assert.ok(rows.every((r) => r.inProject === null));
});

test('an entry whose name a project material has is marked with that material, the first of that name', () => {
  const rows = libraryRows(lib, [
    { id: 'a', name: 'Carpet' },
    { id: 'b', name: '30% absorbing' },
    { id: 'c', name: '30% absorbing' },
  ]);
  assert.deepEqual(
    rows.map((r) => r.inProject),
    [null, 'b', null],
  );
  // Names are compared exactly: another spelling is another material.
  assert.equal(libraryRows(lib, [{ id: 'x', name: '30% Absorbing' }])[1].inProject, null);
});

test('the material an add appended is found; anything but one new row at the end is not', () => {
  const before = [{ id: 'a' }, { id: 'b' }];
  assert.equal(addedMaterial(before, [...before, { id: 'n' }]), 'n');
  assert.equal(addedMaterial(before, before), null, 'a refused add appends nothing');
  assert.equal(addedMaterial(before, [{ id: 'a' }, { id: 'b' }, { id: 'n' }, { id: 'm' }]), null);
  assert.equal(addedMaterial(before, [{ id: 'b' }, { id: 'n' }, { id: 'a' }]), null, 'the last row was there before');
});
