import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { reasonWords, WORDED } from './reasonWords.ts';

// Every code a run can carry, gathered from the core's code lists on 2026-10-06 (the union the Rust
// test the_ui_code_table_covers_every_code_a_run_can_carry builds), minus the solver's progress and
// banner lines, which never end a run.
const RUN_CODES = readFileSync(new URL('./reasonCodes.txt', import.meta.url), 'utf8')
  .split(/\r?\n/)
  .map((l) => l.trim())
  .filter((l) => l && !l.startsWith('#'));

test('every code a run can carry has its own sentence', () => {
  const missing = RUN_CODES.filter((c) => !WORDED.includes(c));
  assert.deepEqual(missing, []);
});

test('a sentence is words: no code, no path, no file name, no hash', () => {
  for (const c of WORDED) {
    const s = reasonWords(c);
    assert.ok(!/[A-Z]{2,}_[A-Z]/.test(s) && !/[a-z]+_[a-z]+/.test(s), `${c}: ${s}`);
    assert.ok(!/0x|[A-Za-z]:\\|sha256|run\.json|\.simpa|\.xml|\.mbin|\.cbin/.test(s), `${c}: ${s}`);
    assert.ok(/^[A-Z].*[.]$/.test(s), `${c}: ${s}`);
  }
});

test('a code with no sentence still reads as words', () => {
  assert.equal(reasonWords('some_new_code'), 'Some new code.');
});
