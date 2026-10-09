// A7: the recent projects list's rules (recentModel.ts).
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { projectAtStart, RECENT_MAX, readRecent, recentLabel, withoutRecent, withRecent } from './recentModel.ts';

test('projectAtStart (A33): the newest recent project, only when chosen, nothing else opened, not the self-test', () => {
  const recent = ['C:\\a.simpa', 'C:\\b.simpa'];
  const on = { reopen: true, opened: false, selftest: false, recent };
  assert.equal(projectAtStart(on), 'C:\\a.simpa');
  assert.equal(projectAtStart({ ...on, reopen: false }), null, 'off: the landing page');
  assert.equal(projectAtStart({ ...on, opened: true }), null, '--project wins');
  assert.equal(projectAtStart({ ...on, selftest: true }), null);
  assert.equal(projectAtStart({ ...on, recent: [] }), null);
});

test('withRecent: the newest first, once whatever its case or slashes, at most five', () => {
  let l: string[] = [];
  for (const p of ['C:\\a\\1.simpa', 'C:\\a\\2.simpa', 'C:\\a\\3.simpa']) l = withRecent(l, p);
  assert.deepEqual(l, ['C:\\a\\3.simpa', 'C:\\a\\2.simpa', 'C:\\a\\1.simpa']);
  l = withRecent(l, 'c:/A/1.SIMPA');
  assert.deepEqual(l, ['c:/A/1.SIMPA', 'C:\\a\\3.simpa', 'C:\\a\\2.simpa']);
  for (const p of ['C:\\b\\4.simpa', 'C:\\b\\5.simpa', 'C:\\b\\6.simpa']) l = withRecent(l, p);
  assert.equal(RECENT_MAX, 5);
  assert.deepEqual(l, ['C:\\b\\6.simpa', 'C:\\b\\5.simpa', 'C:\\b\\4.simpa', 'c:/A/1.SIMPA', 'C:\\a\\3.simpa']);
  assert.deepEqual(withoutRecent(l, 'C:\\B\\5.simpa'), ['C:\\b\\6.simpa', 'C:\\b\\4.simpa', 'c:/A/1.SIMPA', 'C:\\a\\3.simpa']);
});

test('readRecent: a stored list back as it was; anything else is no list', () => {
  assert.deepEqual(readRecent(['C:\\x.simpa', 'C:\\y.simpa']), ['C:\\x.simpa', 'C:\\y.simpa']);
  assert.deepEqual(readRecent(['C:\\x.simpa', 3, '', 'c:\\X.simpa', 'C:\\y.simpa']), ['C:\\x.simpa', 'C:\\y.simpa']);
  assert.deepEqual(readRecent(['1', '2', '3', '4', '5', '6', '7']), ['1', '2', '3', '4', '5']);
  for (const v of [null, 'C:\\x.simpa', { a: 1 }, 7]) assert.deepEqual(readRecent(v), []);
});

test('recentLabel: the file name without .simpa, and its folder', () => {
  assert.deepEqual(recentLabel('B:\\out\\cr4\\BRAS CR4.simpa'), { name: 'BRAS CR4', folder: 'B:\\out\\cr4' });
  assert.deepEqual(recentLabel('C:/x/room.SIMPA'), { name: 'room', folder: 'C:/x' });
  assert.deepEqual(recentLabel('room.simpa'), { name: 'room', folder: '' });
});
