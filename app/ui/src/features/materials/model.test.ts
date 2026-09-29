import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { Material, SurfaceGroup, Variant } from '../../bindings/schema.ts';
import {
  alphaBars,
  effectiveMaterial,
  formatArea,
  newMaterial,
  nextSort,
  selectedGroupIds,
  sortRows,
  transmissionText,
  usage,
  type SortState,
} from './model.ts';

const mat = (id: string, name: string, absorption: (number | string)[]): Material => ({
  ...newMaterial(id, name, absorption.length, 0),
  absorption,
});

const groups: SurfaceGroup[] = [
  { id: 'g0', name: 'Floor', material: 'a' },
  { id: 'g1', name: 'Ceiling', material: 'b' },
  { id: 'g2', name: 'Walls', material: 'b' },
];
const variant: Variant = { id: 'v', name: 'Treated', overrides: [{ group: 'g2', material: 'c' }] };

test('the effective material follows the active variant', () => {
  assert.equal(effectiveMaterial(groups[2], undefined), 'b');
  assert.equal(effectiveMaterial(groups[2], variant), 'c');
  assert.equal(effectiveMaterial(groups[0], variant), 'a');
  const base = usage({ surface_groups: groups, materials: [], variants: [variant], active_variant: null });
  assert.deepEqual([...base], [
    ['a', 1],
    ['b', 2],
  ]);
  const treated = usage({ surface_groups: groups, materials: [], variants: [variant], active_variant: 'v' });
  assert.deepEqual([...treated], [
    ['a', 1],
    ['b', 1],
    ['c', 1],
  ]);
});

test('a selection names groups: a group by id, faces by their group names', () => {
  assert.deepEqual(selectedGroupIds({ kind: 'group', id: 'g1' }, groups), ['g1']);
  assert.deepEqual(selectedGroupIds({ kind: 'group', id: 'gone' }, groups), []);
  assert.deepEqual(selectedGroupIds({ kind: 'faces', groups: ['Walls', 'Floor'] }, groups), ['g0', 'g2']);
  assert.deepEqual(selectedGroupIds({ kind: 'material', id: 'a' }, groups), []);
  assert.deepEqual(selectedGroupIds({ kind: 'none' }, groups), []);
});

test('rows: project order by default; natural name sort; band sort; ties keep project order', () => {
  const ms = [mat('1', 'Material 10', [0.3, 0.1]), mat('2', 'Material 2', [0.1, 0.1]), mat('3', 'Glass', [0.3, 'NaN']), mat('4', 'Material 1', [0.2, 0.1])];
  const ids = (s: SortState) => sortRows(ms, s, 'absorption').map((m) => m.id);
  assert.deepEqual(ids({ kind: 'project' }), ['1', '2', '3', '4']);
  assert.deepEqual(ids({ kind: 'name', dir: 1 }), ['3', '4', '2', '1']);
  assert.deepEqual(ids({ kind: 'name', dir: -1 }), ['1', '2', '4', '3']);
  assert.deepEqual(ids({ kind: 'band', band: 0, dir: 1 }), ['2', '4', '1', '3']);
  assert.deepEqual(ids({ kind: 'band', band: 0, dir: -1 }), ['1', '3', '4', '2']);
  assert.deepEqual(ids({ kind: 'band', band: 1, dir: 1 }), ['1', '2', '4', '3']);
  assert.deepEqual(sortRows(ms, { kind: 'band', band: 0, dir: 1 }, 'scattering').map((m) => m.id), ['1', '2', '3', '4']);
});

test('a header click cycles ascending, descending, project order', () => {
  let s: SortState = { kind: 'project' };
  s = nextSort(s, 'name');
  assert.deepEqual(s, { kind: 'name', dir: 1 });
  s = nextSort(s, 'name');
  assert.deepEqual(s, { kind: 'name', dir: -1 });
  s = nextSort(s, 'name');
  assert.deepEqual(s, { kind: 'project' });
  s = nextSort(nextSort(s, 3), 2);
  assert.deepEqual(s, { kind: 'band', band: 2, dir: 1 });
});

test('transmission reads off, or on in some or every band', () => {
  const m = newMaterial('x', 'Door', 3, 0);
  assert.equal(transmissionText(m), 'off');
  assert.equal(transmissionText({ ...m, transmission_loss_db: [null, null, null] }), 'off');
  assert.equal(transmissionText({ ...m, transmission_loss_db: [null, 20, 25] }), 'on in 2 of 3 bands');
  assert.equal(transmissionText({ ...m, transmission_loss_db: [15, 20, 25] }), 'on in every band');
});

test('display helpers: the design mini-bars, areas as geometry facts, new rows', () => {
  assert.deepEqual(alphaBars([0, 0.5, 1, 1.5, 'NaN']), [2, 12, 24, 24, 2]);
  assert.equal(formatArea(18), '18');
  assert.equal(formatArea(4001.809), '4001.8');
  assert.equal(formatArea(null), '—');
  const m = newMaterial('id', 'Material 1', 6, 7);
  assert.equal(m.absorption.length, 6);
  assert.equal(m.scattering.length, 6);
  assert.equal(m.transmission_loss_db, null);
  assert.match(m.color, /^#[0-9a-f]{6}$/);
});
