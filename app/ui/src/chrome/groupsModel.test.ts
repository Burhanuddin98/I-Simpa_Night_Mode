// groupsModel.ts under `node --test` (C1).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { groupPicked, mergePlan, moveTargets, renameProblem, renameTarget, toggleGroup } from './groupsModel.ts';

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
