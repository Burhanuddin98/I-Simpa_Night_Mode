// groupsModel.ts under `node --test` (C1).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { deleteGroupOps, groupDeleteProblem, groupPicked, inUseSentence, mergePlan, moveTargets, renameProblem, renameTarget, toggleGroup } from './groupsModel.ts';

const groups = [
  { id: 'a', name: 'floor' },
  { id: 'b', name: 'Group 1' },
  { id: 'c', name: 'Group 2' },
];

test('Ctrl+click adds a group to the picked ones and takes a picked one out', () => {
  assert.deepEqual(toggleGroup({ kind: 'none' }, 'a'), { kind: 'group', id: 'a' });
  assert.deepEqual(toggleGroup({ kind: 'group', id: 'a' }, 'b'), { kind: 'groups', ids: ['a', 'b'] });
  assert.deepEqual(toggleGroup({ kind: 'groups', ids: ['a', 'b'] }, 'c'), { kind: 'groups', ids: ['a', 'b', 'c'] });
  assert.deepEqual(toggleGroup({ kind: 'groups', ids: ['a', 'b'] }, 'a'), { kind: 'group', id: 'b' });
  assert.deepEqual(toggleGroup({ kind: 'group', id: 'a' }, 'a'), { kind: 'none' });
  assert.deepEqual(toggleGroup({ kind: 'source', id: 's' }, 'c'), { kind: 'group', id: 'c' }, 'another selection starts over');
  assert.equal(groupPicked({ kind: 'groups', ids: ['a', 'b'] }, 'b'), true);
  assert.equal(groupPicked({ kind: 'group', id: 'a' }, 'b'), false);
});

test('Merge folds every picked group into the first picked; fewer than two is no merge', () => {
  assert.deepEqual(mergePlan({ kind: 'groups', ids: ['c', 'a'] }, groups), { into: groups[2], from: [groups[0]] });
  assert.equal(mergePlan({ kind: 'group', id: 'a' }, groups), null);
  assert.equal(mergePlan({ kind: 'groups', ids: ['a', 'zz'] }, groups), null, 'a group the project no longer has');
});

test('F2 renames one picked group; a blank name or another group\'s is refused', () => {
  assert.equal(renameTarget({ kind: 'group', id: 'b' }), 'b');
  assert.equal(renameTarget({ kind: 'groups', ids: ['a', 'b'] }), null);
  assert.equal(renameProblem('  ', 'b', groups)?.code, 'GROUP_NAME_EMPTY');
  assert.equal(renameProblem('floor', 'b', groups)?.code, 'GROUP_NAME_TAKEN');
  assert.equal(renameProblem('floor', 'a', groups), null, 'its own name again is no conflict');
  assert.equal(renameProblem(' FLOOR ', 'b', groups)?.code, 'GROUP_NAME_TAKEN', 'compared trimmed and without case, as the core does');
  assert.equal(renameProblem('Floor', 'a', groups), null, 'its own name in another case');
  assert.equal(renameProblem('wall north', 'b', groups), null);
});

test('Move to group offers every group but the one holding all the picked faces', () => {
  assert.deepEqual(moveTargets(groups, ['Group 1']).map((g) => g.id), ['a', 'c']);
  assert.deepEqual(moveTargets(groups, ['Group 1', 'floor']).map((g) => g.id), ['a', 'b', 'c']);
  assert.deepEqual(moveTargets(groups, []).map((g) => g.id), ['a', 'b', 'c']);
});

test('G18: a group that holds faces is refused with what to do; an empty one deletes', () => {
  const view = { surface_groups: groups, surface_receivers: [], variants: [] };
  const full = groupDeleteProblem('a', 12, view);
  assert.equal(full?.code, 'GROUP_NOT_EMPTY');
  assert.match(full?.message ?? '', /^floor still holds 12 faces, so it cannot be deleted\. Move them to another group first/);
  assert.match(groupDeleteProblem('a', 1, view)?.message ?? '', /holds 1 face,/);
  assert.equal(groupDeleteProblem('b', 0, view), null);
  assert.equal(groupDeleteProblem('zz', 3, view), null, 'a group the project no longer has');
});

test('G18: an empty group a surface map is drawn on is refused, by name', () => {
  const view = {
    surface_groups: groups,
    surface_receivers: [{ name: 'Floor map', shape: { kind: 'scene', groups: ['b'] } }, { name: 'Plane 1', shape: { kind: 'cutting_plane' } }],
    variants: [],
  };
  assert.deepEqual(groupDeleteProblem('b', 0, view), { code: 'GROUP_IN_USE', message: 'Group 1 is part of the surface map Floor map, so it cannot be deleted.' });
  assert.equal(groupDeleteProblem('c', 0, view), null);
});

test('G18: deleting clears each variant override on the empty group in the same undo step', () => {
  const plain = { surface_groups: groups, surface_receivers: [], variants: [{ id: 'v1', overrides: [{ group: 'a' }] }] };
  assert.deepEqual(deleteGroupOps('b', plain), { op: 'remove_surface_group', id: 'b' });
  const over = { ...plain, variants: [{ id: 'v1', overrides: [{ group: 'b' }] }, { id: 'v2', overrides: [] }] };
  assert.deepEqual(deleteGroupOps('b', over), {
    op: 'batch',
    ops: [
      { op: 'set_variant_override', variant: 'v1', group: 'b', material: null },
      { op: 'remove_surface_group', id: 'b' },
    ],
  });
});

test('the in-use refusal of the core reads without the id', () => {
  assert.equal(
    inUseSentence('Group 1', "surface group 3f2a9c1e-0000-4000-8000-000000000000 is still used by fitting zone 'Stage'"),
    "Group 1 cannot be deleted: fitting zone 'Stage' still uses it.",
  );
  assert.equal(inUseSentence('Group 1', 'something else'), 'something else');
});

// M40: a surface map over the picked surfaces.
import { mapOfGroup, surfaceMapPlan } from './groupsModel.ts';

test('M40: a surface map takes the picked groups, from the list or from faces; nothing picked says so', () => {
  const view = {
    surface_groups: [
      { id: 'g1', name: 'Floor' },
      { id: 'g2', name: 'Walls' },
      { id: 'g3', name: 'Seats' },
    ],
    surface_receivers: [
      { id: 'r1', name: 'Map 1', enabled: true, shape: { kind: 'scene', groups: ['g3'] } },
      { id: 'r2', name: 'Off map', enabled: false, shape: { kind: 'scene', groups: ['g2'] } },
      { id: 'p1', name: 'Plane 1', enabled: true, shape: { kind: 'cutting_plane' } },
    ],
  };
  assert.deepEqual(surfaceMapPlan({ kind: 'group', id: 'g1' }, view), { groups: ['g1'] });
  assert.deepEqual(surfaceMapPlan({ kind: 'groups', ids: ['g1', 'g2'] }, view), { groups: ['g1', 'g2'] }, 'a switched-off map holds nothing');
  assert.deepEqual(surfaceMapPlan({ kind: 'faces', faces: [1, 2], groups: ['Walls', 'Floor', 'Walls'] }, view), { groups: ['g2', 'g1'] });
  const none = surfaceMapPlan({ kind: 'none' }, view);
  assert.ok('problem' in none && none.problem.code === 'MAP_NOTHING_PICKED');
  const taken = surfaceMapPlan({ kind: 'groups', ids: ['g1', 'g3'] }, view);
  assert.ok('problem' in taken && taken.problem.code === 'MAP_GROUP_TAKEN' && taken.problem.message.includes('Seats (in Map 1)'), JSON.stringify(taken));
  assert.deepEqual(mapOfGroup('g3', view), { id: 'r1', name: 'Map 1' });
  assert.equal(mapOfGroup('g3', view, 'r1'), null, 'its own map is not another');
});
