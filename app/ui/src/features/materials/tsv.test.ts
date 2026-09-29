import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { displayValue, formatExact, parseTsv, toTsv } from './tsv.ts';

const FIXTURE = path.resolve(import.meta.dirname, '../../../../../tests/fixtures/ui/materials_6x6.tsv');

test('CRLF and LF rows, with one trailing line end dropped', () => {
  assert.deepEqual(parseTsv('1\t2\r\n3\t4\r\n'), [['1', '2'], ['3', '4']]);
  assert.deepEqual(parseTsv('1\t2\n3\t4\n'), [['1', '2'], ['3', '4']]);
  assert.deepEqual(parseTsv('1\t2\n3\t4'), [['1', '2'], ['3', '4']]);
  assert.deepEqual(parseTsv('1\t2\r3\t4\r'), [['1', '2'], ['3', '4']]);
  assert.deepEqual(parseTsv('0.5'), [['0.5']]);
  // Only one trailing line end is a spreadsheet's; a second is an empty row.
  assert.deepEqual(parseTsv('1\n\n'), [['1'], ['']]);
  assert.deepEqual(parseTsv('\r\n'), [['']]);
  assert.deepEqual(parseTsv(''), []);
  assert.deepEqual(parseTsv('1\t\r\n'), [['1', '']]);
});

test("Excel's quoted cells: doubled quotes, tabs and line ends inside", () => {
  assert.deepEqual(parseTsv('"0.5"\t"0.25"\r\n'), [['0.5', '0.25']]);
  assert.deepEqual(parseTsv('"Say ""hi"""\t1\r\n'), [['Say "hi"', '1']]);
  assert.deepEqual(parseTsv('"two\r\nlines"\t"a\tb"\r\n'), [['two\r\nlines', 'a\tb']]);
  assert.deepEqual(parseTsv('""\t1'), [['', '1']]);
  // A quote that is never closed is text; a quote inside a cell is text.
  assert.deepEqual(parseTsv('"open\t1\n'), [['"open', '1']]);
  assert.deepEqual(parseTsv('a"b\t1'), [['a"b', '1']]);
});

test('toTsv writes what parseTsv reads, quoting only where needed', () => {
  const rows = [['Painted plaster', '0.02', '-0'], ['Say "hi"', 'a\tb', 'two\nlines']];
  const text = toTsv(rows);
  assert.equal(text, 'Painted plaster\t0.02\t-0\r\n"Say ""hi"""\t"a\tb"\t"two\nlines"\r\n');
  assert.deepEqual(parseTsv(text), rows);
});

test('the committed 6x6 fixture parses to 36 cells, the 17-digit one intact', () => {
  const text = readFileSync(FIXTURE, 'utf8');
  assert.ok(text.endsWith('\r\n'), 'the fixture keeps its CRLF');
  const rows = parseTsv(text);
  assert.equal(rows.length, 6);
  for (const r of rows) assert.equal(r.length, 6);
  assert.equal(rows[3][5], '0.39509836780726515');
  assert.equal(rows[2][0], '0.013');
});

test('formatExact is the shortest round-trip spelling, -0 kept', () => {
  for (const v of [0.1, 0.02, 0.39509836780726515, 1 / 3, 0.30000000000000004, 1e-7, 1e21, 5e-324, 1]) {
    const s = formatExact(v);
    assert.ok(Object.is(Number(s), v), `${s} reads back`);
  }
  assert.equal(formatExact(0.39509836780726515), '0.39509836780726515');
  assert.equal(formatExact(0.5), '0.5');
  assert.equal(formatExact(-0), '-0');
  assert.ok(Object.is(Number(formatExact(-0)), -0));
  assert.equal(formatExact('NaN'), 'NaN');
});

test('the display never rounds without marking it', () => {
  assert.deepEqual(displayValue(0.1), { text: '0.10', rounded: false });
  assert.deepEqual(displayValue(0.5), { text: '0.50', rounded: false });
  assert.deepEqual(displayValue(0.013), { text: '0.013', rounded: false });
  assert.deepEqual(displayValue(1), { text: '1.00', rounded: false });
  assert.deepEqual(displayValue(-0), { text: '-0.00', rounded: false });
  assert.deepEqual(displayValue(0.39509836780726515), { text: '≈0.40', rounded: true });
  assert.deepEqual(displayValue(0.30000000000000004), { text: '≈0.30', rounded: true });
  assert.deepEqual(displayValue(0.0125), { text: '≈0.01', rounded: true });
  assert.deepEqual(displayValue('NaN'), { text: 'NaN', rounded: false });
  // Unmarked text always reads back to the value itself.
  for (const v of [0.02, 0.03, 0.7, 0.65, 0.015, 0.25]) {
    const d = displayValue(v);
    assert.ok(!d.rounded && Number(d.text) === v, String(v));
  }
});
