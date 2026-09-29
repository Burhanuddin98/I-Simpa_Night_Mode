// A paste into the materials grid, planned before any op (PLAN.md 6.2): all or nothing. A pure
// module: no store, no backend, only erasable TypeScript, tested by paste.test.ts under
// `node --test`.
import { NOT_A_NUMBER, parseStrictDecimal } from '../../numbers.ts';
import { bandRange, parseBandLabel, type BandColumn, type F64 } from './bands.ts';
import { parseTsv } from './tsv.ts';

/** The block does not fit: ragged rows, nothing to paste, or more rows or bands than there are. */
export const PASTE_SHAPE = 'PASTE_SHAPE';
/** A header names no band of the project, or a name column names no material. */
export const PASTE_HEADER = 'PASTE_HEADER';

/** A cell of the grid as displayed: `row` in display order, `col` a band column (-1: names). */
export interface GridCell {
  row: number;
  col: number;
}

export interface PasteContext {
  /** The band columns in display order (ascending frequency). */
  columns: readonly BandColumn[];
  /** The rows in display order. */
  rows: readonly { id: string; name: string }[];
  /** The focused cell: the block's top-left corner when it has no header and no name column. */
  cursor: GridCell;
  /** The value shown at a cell now, so that cells the paste does not change are left out. */
  valueAt: (row: number, col: number) => F64;
}

export interface PasteIssue {
  code: string;
  message: string;
  /** The grid cells the issue is about, when they are known. */
  cells: GridCell[];
}

export type PastePlan =
  | {
      ok: true;
      /** The cells whose value changes, in the block's row-major order. */
      cells: (GridCell & { value: number })[];
      /** Cells the paste covers whose value is already the pasted one. */
      unchanged: number;
      /** Every cell the block covers (changed or not), for the selection afterwards. */
      covered: GridCell[];
      header: boolean;
      names: boolean;
    }
  | { ok: false; issues: PasteIssue[] };

const NOTHING = 'Nothing was pasted.';
const MAX_NAMED = 6;

/** `a, b, c and 4 more`. */
function listed(items: readonly string[]): string {
  if (items.length <= MAX_NAMED) return items.join(', ');
  return `${items.slice(0, MAX_NAMED).join(', ')} and ${items.length - MAX_NAMED} more`;
}

/** Text, as opposed to a number or a band label: it holds a letter and is not a label (`1k`). */
const isText = (s: string) => /\p{L}/u.test(s) && parseBandLabel(s) === null;

/**
 * Plans a paste of clipboard `text` at `ctx.cursor`.
 *
 * - **Header row** (optional). The first line is a header when every cell is a band label with
 *   a unit or a `k` suffix (`125 Hz`, `1k`, `1 kHz`), or when its first cell is text (a letter,
 *   or empty before band labels) that names no material. A header maps columns **by
 *   frequency**; when its first cell is text, bare numbers (`125`) count as frequencies too.
 * - **Name column** (optional). Under a header whose first cell is not a band label, or, without
 *   a header, when some first cell is exactly a material's name. Rows then map by exact name.
 * - Otherwise the block lands with its top-left cell on the focused cell.
 * - **All or nothing.** `PASTE_SHAPE` (ragged rows, an empty block, a block running off the
 *   grid), `PASTE_HEADER` (a band or name that is not there, or given twice) and `NOT_A_NUMBER`
 *   (a cell outside `parseStrictDecimal`'s grammar) each refuse the whole paste, with the bad
 *   cells named.
 */
export function planPaste(text: string, ctx: PasteContext): PastePlan {
  const table = parseTsv(text);
  if (table.length === 0 || (table.length === 1 && table[0].length === 1 && table[0][0].trim() === '')) {
    return fail(PASTE_SHAPE, `The clipboard holds no cells. ${NOTHING}`);
  }
  const width = table[0].length;
  const ragged = table.map((r, i) => [i, r.length] as const).filter(([, len]) => len !== width);
  if (ragged.length > 0) {
    const lines = ragged.map(([i, len]) => `line ${i + 1} has ${len}`);
    return fail(PASTE_SHAPE, `The lines are not all ${width} cells wide: ${listed(lines)}. ${NOTHING}`);
  }

  const namesOf = (s: string) => ctx.rows.filter((r) => r.name === s);
  const first = table[0];
  let header = false;
  let names = false;
  if (first.every((c) => parseBandLabel(c) !== null)) {
    header = true;
  } else if (
    width > 1 &&
    namesOf(first[0]).length === 0 &&
    (isText(first[0]) || (first[0].trim() === '' && first.slice(1).every((c) => parseBandLabel(c) !== null)))
  ) {
    header = true;
    names = true;
  } else {
    names = table.some((r) => namesOf(r[0]).length > 0);
  }

  const data = header ? table.slice(1) : table;
  const valueStart = names ? 1 : 0;
  const nValues = width - valueStart;
  const issues: PasteIssue[] = [];

  // Columns: by frequency under a header, else from the focused column on.
  const colOf: number[] = [];
  if (header) {
    const seen = new Map<number, string>();
    const unknown: string[] = [];
    const twice: string[] = [];
    for (let j = valueStart; j < width; j++) {
      const label = first[j];
      const hz = parseBandLabel(label, names);
      const col = hz === null ? -1 : ctx.columns.findIndex((c) => c.hz === hz);
      if (hz === null || col < 0) {
        unknown.push(`'${label}'`);
        continue;
      }
      if (seen.has(hz)) twice.push(`'${label}' (as '${seen.get(hz)}')`);
      seen.set(hz, label);
      colOf[j] = col;
    }
    if (unknown.length > 0) {
      const why = names && isText(first[0]) ? ` Line 1 reads as a header because its first cell '${first[0]}' is text and names no material.` : '';
      issues.push({
        code: PASTE_HEADER,
        message: `${listed(unknown)} in the header ${unknown.length === 1 ? 'is' : 'are'} not a band of this project (${bandRange(ctx.columns)}); a header cell reads like '125 Hz', '1k' or '1 kHz'.${why} ${NOTHING}`,
        cells: [],
      });
    }
    if (twice.length > 0) {
      issues.push({ code: PASTE_HEADER, message: `The header names a band twice: ${listed(twice)}. ${NOTHING}`, cells: [] });
    }
  } else {
    const start = Math.max(0, ctx.cursor.col);
    const room = ctx.columns.length - start;
    if (nValues > room) {
      issues.push({
        code: PASTE_SHAPE,
        message: `The block is ${nValues} bands wide; from ${ctx.columns[start]?.label ?? 'the focused cell'} there ${room === 1 ? 'is 1 band' : `are ${room} bands`}. ${NOTHING}`,
        cells: [],
      });
    }
    for (let j = valueStart; j < width; j++) colOf[j] = start + (j - valueStart);
  }

  // A header that reads wrong explains more than "no values" would.
  if (data.length === 0 || nValues === 0) {
    if (issues.length > 0) return { ok: false, issues };
    return fail(PASTE_SHAPE, `The block holds ${data.length === 0 ? 'a header and no values' : 'names and no values'}. ${NOTHING}`);
  }

  // Rows: by exact name, else from the focused row down.
  const rowOf: number[] = [];
  if (names) {
    const unknown: string[] = [];
    const ambiguous: string[] = [];
    const twice: string[] = [];
    const seen = new Set<number>();
    data.forEach((r, i) => {
      const name = r[0];
      const matches = ctx.rows.map((row, k) => (row.name === name ? k : -1)).filter((k) => k >= 0);
      if (matches.length === 0) unknown.push(`'${name}'`);
      else if (matches.length > 1) ambiguous.push(`'${name}'`);
      else {
        if (seen.has(matches[0])) twice.push(`'${name}'`);
        seen.add(matches[0]);
        rowOf[i] = matches[0];
      }
    });
    if (unknown.length > 0) {
      issues.push({
        code: PASTE_HEADER,
        message: `The name column holds ${listed(unknown)}, which ${unknown.length === 1 ? 'names' : 'name'} no material (names match exactly). ${NOTHING}`,
        cells: [],
      });
    }
    if (ambiguous.length > 0) {
      issues.push({ code: PASTE_HEADER, message: `${listed(ambiguous)} names more than one material. ${NOTHING}`, cells: [] });
    }
    if (twice.length > 0) {
      issues.push({ code: PASTE_HEADER, message: `The name column gives ${listed(twice)} twice. ${NOTHING}`, cells: [] });
    }
  } else {
    const room = ctx.rows.length - ctx.cursor.row;
    if (data.length > room) {
      issues.push({
        code: PASTE_SHAPE,
        message: `The block is ${data.length} rows tall; from ${ctx.rows[ctx.cursor.row]?.name ?? 'the focused cell'} down there ${room === 1 ? 'is 1 row' : `are ${room} rows`}. ${NOTHING}`,
        cells: [],
      });
    }
    data.forEach((_, i) => (rowOf[i] = ctx.cursor.row + i));
  }
  if (issues.length > 0) return { ok: false, issues };

  // Values: every cell by the strict grammar, before any op.
  const cells: (GridCell & { value: number })[] = [];
  const covered: GridCell[] = [];
  const bad: string[] = [];
  const badCells: GridCell[] = [];
  let unchanged = 0;
  data.forEach((r, i) => {
    for (let j = valueStart; j < width; j++) {
      const at = { row: rowOf[i], col: colOf[j] };
      const parsed = parseStrictDecimal(r[j]);
      if (!parsed.ok) {
        bad.push(`'${r[j]}' at ${ctx.rows[at.row].name} · ${ctx.columns[at.col].label}`);
        badCells.push(at);
        continue;
      }
      covered.push(at);
      if (Object.is(ctx.valueAt(at.row, at.col), parsed.value)) unchanged++;
      else cells.push({ ...at, value: parsed.value });
    }
  });
  if (bad.length > 0) {
    return {
      ok: false,
      issues: [
        {
          code: NOT_A_NUMBER,
          message: `Not a number: ${listed(bad)}. Write decimals with a point, as in 0.5; NaN and infinities are refused. ${NOTHING}`,
          cells: badCells,
        },
      ],
    };
  }
  return { ok: true, cells, unchanged, covered, header, names };
}

function fail(code: string, message: string): PastePlan {
  return { ok: false, issues: [{ code, message, cells: [] }] };
}
