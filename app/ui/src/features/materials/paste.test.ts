import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { bandColumns, type F64 } from './bands.ts';
import { PASTE_HEADER, PASTE_SHAPE, planPaste, type PasteContext, type PastePlan } from './paste.ts';

const FIXTURE = path.resolve(import.meta.dirname, '../../../../../tests/fixtures/ui/materials_6x6.tsv');
const NAMES = ['Linoleum on concrete', 'Mineral-fibre tile', 'Painted plaster', 'Slotted wood panel', 'Heavy velour curtain', 'Glass window'];

/** The teaching room's grid: six materials, octave bands, every value 0.1. */
function room(cursor = { row: 0, col: 0 }, values?: F64[][]): PasteContext {
  const grid = values ?? NAMES.map(() => [0.1, 0.1, 0.1, 0.1, 0.1, 0.1]);
  return {
    columns: bandColumns([125, 250, 500, 1000, 2000, 4000]),
    rows: NAMES.map((name, i) => ({ id: `m${i}`, name })),
    cursor,
    valueAt: (r, c) => grid[r][c],
  };
}

function ok(plan: PastePlan) {
  assert.ok(plan.ok, plan.ok ? '' : JSON.stringify(plan.issues));
  return plan;
}

function refused(plan: PastePlan, code: string) {
  assert.ok(!plan.ok, 'the paste must be refused');
  assert.equal(plan.issues[0].code, code, JSON.stringify(plan.issues));
  return plan.issues;
}

test('the 6x6 fixture lands at the focused cell, 36 values, row-major, exact', () => {
  const text = readFileSync(FIXTURE, 'utf8');
  const plan = ok(planPaste(text, room()));
  assert.equal(plan.cells.length, 36);
  assert.equal(plan.unchanged, 0);
  assert.equal(plan.header, false);
  assert.equal(plan.names, false);
  assert.deepEqual(plan.cells[0], { row: 0, col: 0, value: 0.02 });
  assert.deepEqual(plan.cells[23], { row: 3, col: 5, value: 0.39509836780726515 });
  assert.ok(Object.is(plan.cells[23].value, Number('0.39509836780726515')));
  assert.deepEqual(plan.cells[35], { row: 5, col: 5, value: 0.04 });
});

test('LF, a missing trailing newline and quoted cells paste alike', () => {
  const a = ok(planPaste('0.2\t0.3\r\n0.4\t0.5\r\n', room({ row: 1, col: 2 })));
  const b = ok(planPaste('"0.2"\t0.3\n0.4\t"0.5"', room({ row: 1, col: 2 })));
  assert.deepEqual(a.cells, b.cells);
  assert.deepEqual(a.cells.map((c) => [c.row, c.col, c.value]), [
    [1, 2, 0.2],
    [1, 3, 0.3],
    [2, 2, 0.4],
    [2, 3, 0.5],
  ]);
});

test('cells already holding the value are left out, and counted', () => {
  const plan = ok(planPaste('0.1\t0.5', room()));
  assert.deepEqual(plan.cells, [{ row: 0, col: 1, value: 0.5 }]);
  assert.equal(plan.unchanged, 1);
  assert.equal(plan.covered.length, 2);
  // -0 is not 0: the core keeps the sign, so it is a change.
  assert.equal(ok(planPaste('-0', room(undefined, NAMES.map(() => [0, 0, 0, 0, 0, 0])))).cells.length, 1);
});

test('a header row maps columns by frequency, in any order and spelling', () => {
  const plan = ok(planPaste('4 kHz\t125 Hz\t1k\r\n0.9\t0.05\t0.6\r\n', room({ row: 4, col: 3 })));
  assert.equal(plan.header, true);
  assert.deepEqual(plan.cells.map((c) => [c.row, c.col, c.value]), [
    [4, 5, 0.9],
    [4, 0, 0.05],
    [4, 3, 0.6],
  ]);
  const kHz = ok(planPaste('1 kHz\t500Hz\n0.3\t0.2', room({ row: 0, col: 5 })));
  assert.deepEqual(kHz.cells.map((c) => [c.row, c.col]), [
    [0, 3],
    [0, 2],
  ]);
});

test('a leading name column maps rows by exact name; a text corner allows bare frequencies', () => {
  const plan = ok(planPaste('Material\t500\t2k\r\nGlass window\t0.2\t0.3\r\nPainted plaster\t0.4\t0.5\r\n', room({ row: 0, col: 0 })));
  assert.equal(plan.header, true);
  assert.equal(plan.names, true);
  assert.deepEqual(plan.cells.map((c) => [c.row, c.col, c.value]), [
    [5, 2, 0.2],
    [5, 4, 0.3],
    [2, 2, 0.4],
    [2, 4, 0.5],
  ]);
  // An empty corner before band labels is a header with a name column too.
  const corner = ok(planPaste('\t125 Hz\nMineral-fibre tile\t0.7', room({ row: 5, col: 5 })));
  assert.deepEqual(corner.cells.map((c) => [c.row, c.col, c.value]), [[1, 0, 0.7]]);
  // Without a header: names map rows, values start at the focused column.
  const bare = ok(planPaste('Heavy velour curtain\t0.3\t0.4\nLinoleum on concrete\t0.2\t0.2', room({ row: 3, col: 4 })));
  assert.equal(bare.header, false);
  assert.deepEqual(bare.cells.map((c) => [c.row, c.col, c.value]), [
    [4, 4, 0.3],
    [4, 5, 0.4],
    [0, 4, 0.2],
    [0, 5, 0.2],
  ]);
});

test('PASTE_SHAPE: ragged rows, nothing, or a block running off the grid', () => {
  refused(planPaste('0.1\t0.2\r\n0.3\r\n', room()), PASTE_SHAPE);
  refused(planPaste('', room()), PASTE_SHAPE);
  refused(planPaste('\r\n', room()), PASTE_SHAPE);
  const wide = refused(planPaste('0.1\t0.2\t0.3', room({ row: 0, col: 4 })), PASTE_SHAPE);
  assert.match(wide[0].message, /3 bands wide; from 2k there are 2 bands/);
  const tall = refused(planPaste('0.1\n0.2\n0.3', room({ row: 4, col: 0 })), PASTE_SHAPE);
  assert.match(tall[0].message, /3 rows tall/);
  refused(planPaste('125 Hz\t250 Hz\r\n', room()), PASTE_SHAPE);
  // A second trailing line end is an empty row, so the shape is wrong.
  refused(planPaste('0.1\t0.2\r\n\r\n', room()), PASTE_SHAPE);
});

test('PASTE_HEADER: a band the project lacks, a band twice, a name that is not there', () => {
  const unknown = refused(planPaste('3 kHz\t125 Hz\n0.1\t0.2', room()), PASTE_HEADER);
  assert.match(unknown[0].message, /'3 kHz'/);
  assert.match(unknown[0].message, /125 … 4k/);
  refused(planPaste('1k\t1 kHz\n0.1\t0.2', room()), PASTE_HEADER);
  const name = refused(planPaste('Material\t125 Hz\nCarpet\t0.2', room()), PASTE_HEADER);
  assert.match(name[0].message, /'Carpet'/);
  refused(planPaste('Material\t125 Hz\nGlass window\t0.2\nGlass window\t0.3', room()), PASTE_HEADER);
  // Two materials with one name: a name column cannot tell them apart.
  const twin = room();
  const rows = twin.rows.map((r) => ({ ...r }));
  rows[1].name = 'Glass window';
  refused(planPaste('Glass window\t0.2', { ...twin, rows }), PASTE_HEADER);
  // A text first cell that names no material makes line 1 a header.
  const typo = refused(planPaste('Glas window\t0.2\t0.3', room()), PASTE_HEADER);
  assert.match(typo[0].message, /'Glas window' is text and names no material/);
});

test('NOT_A_NUMBER: the whole paste is refused, every bad cell named and located', () => {
  const issues = refused(planPaste('0.1\t0,5\r\n\tNaN\r\n', room({ row: 2, col: 1 })), 'NOT_A_NUMBER');
  assert.equal(issues.length, 1);
  assert.match(issues[0].message, /'0,5' at Painted plaster · 500/);
  assert.match(issues[0].message, /'' at Slotted wood panel · 250/);
  assert.match(issues[0].message, /'NaN' at Slotted wood panel · 500/);
  assert.deepEqual(issues[0].cells, [
    { row: 2, col: 2 },
    { row: 3, col: 1 },
    { row: 3, col: 2 },
  ]);
  for (const bad of ['Infinity', '1e999', '0x1', '1/2', '5 %']) refused(planPaste(bad, room()), 'NOT_A_NUMBER');
});
