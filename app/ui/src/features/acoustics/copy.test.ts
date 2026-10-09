// R58: the copied text's shape (copy.ts).
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { tsv, tsvCell } from './copy.ts';

test('R58: a table copies as tab-separated rows, CRLF between rows, each cell on one line', () => {
  assert.equal(
    tsv([
      ['Band', 'T30'],
      ['500 Hz', '0.60 0.59 – 0.61 ok'],
    ]),
    'Band\tT30\r\n500 Hz\t0.60 0.59 – 0.61 ok',
  );
  // A tab or a line break inside a cell would split the row or the column: made a space.
  assert.equal(tsvCell(' REFUSED\tparams_not_evaluable\n· range_not_reached  '), 'REFUSED params_not_evaluable · range_not_reached');
  assert.equal(tsv([]), '');
  assert.equal(tsv([['', 'x']]), '\tx');
});
