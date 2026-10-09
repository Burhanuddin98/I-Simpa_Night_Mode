// A38: what a file dropped on the window opens (drop.ts).
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { dropChoice } from './drop.ts';

test('dropChoice: the first file, if Open takes it; the rest counted, never opened', () => {
  assert.deepEqual(dropChoice(['C:\\x\\room.simpa']), { path: 'C:\\x\\room.simpa', ignored: 0 });
  assert.deepEqual(dropChoice(['C:\\x\\hall.PLY', 'C:\\x\\b.simpa', 'C:\\x\\c.txt']), { path: 'C:\\x\\hall.PLY', ignored: 2 });
  for (const ext of ['proj', 'obj', 'stl']) assert.equal('path' in dropChoice([`D:\\m.${ext}`]), true, ext);
});

test('dropChoice: a file Open does not take is refused with a sentence naming it; nothing dropped is said', () => {
  const r = dropChoice(['C:\\x\\notes.txt', 'C:\\x\\room.simpa']);
  assert.ok('problem' in r);
  assert.match(r.problem, /^notes\.txt is not a file Night Mode opens/);
  assert.ok('problem' in dropChoice([]));
  assert.ok('problem' in dropChoice(['C:\\x\\folder']));
});
