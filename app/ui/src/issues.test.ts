import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { UiIssue } from './bindings/ipc.ts';
import { fieldKey, issueKey, issuesByEntity, issuesForField, projectIssues } from './issues.ts';

const issue = (rule: string, path: string, entity: UiIssue['entity'], field: string): UiIssue => ({
  code: rule.toUpperCase(),
  rule,
  severity: 'error',
  path,
  entity,
  field,
  message: rule,
});

const r1 = { kind: 'point_receiver', id: 'r1' } as const;
const m1 = { kind: 'material', id: 'm1' } as const;
const all = [
  issue('receiver_outside_volume', '/point_receivers/0/position', r1, 'position'),
  issue('name_not_filename_safe', '/point_receivers/0/name', r1, 'name'),
  issue('material_value_out_of_range', '/materials/0/absorption/3', m1, 'absorption/3'),
  issue('source_none', '/sources', null, ''),
];

test('keys name the entity and the field', () => {
  assert.equal(fieldKey('point_receiver', 'r1', 'position'), 'point_receiver:r1:position');
  assert.equal(issueKey(all[2]), 'material:m1:absorption/3');
  assert.equal(issueKey(all[3]), null);
});

test('issues group by entity and by field', () => {
  const by = issuesByEntity(all);
  assert.equal(by.get('point_receiver:r1')?.length, 2);
  assert.equal(by.get('material:m1')?.length, 1);
  assert.deepEqual(
    issuesForField(all, 'point_receiver', 'r1', 'position').map((i) => i.rule),
    ['receiver_outside_volume'],
  );
  assert.equal(issuesForField(all, 'material', 'm1', 'absorption').length, 1, 'sub-fields count');
  assert.equal(issuesForField(all, 'material', 'm1', 'absorption/2').length, 0);
  assert.deepEqual(projectIssues(all).map((i) => i.rule), ['source_none']);
});
