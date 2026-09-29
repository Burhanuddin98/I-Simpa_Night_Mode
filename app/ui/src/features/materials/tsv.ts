// Tab-separated text as spreadsheets put it on the clipboard, and the grid's number spelling
// (PLAN.md 6.2). A pure module: no store, no backend, only erasable TypeScript, tested by
// tsv.test.ts under `node --test`.
import type { F64 } from './bands.ts';

/**
 * Splits clipboard text into rows of cells.
 *
 * - Rows end with CRLF, LF or a lone CR. One trailing line end is dropped, as a spreadsheet
 *   writes one after the last row; a second one is an empty row and is kept.
 * - Cells are split on tabs.
 * - A cell that starts with `"` and has a closing `"` is Excel's quoted cell: `""` inside it is
 *   one `"`, and tabs and line ends inside it are text. A quote that is never closed is text.
 *
 * Empty text gives no rows.
 */
export function parseTsv(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = '';
  let atStart = true;
  let i = 0;
  const n = text.length;
  const endCell = () => {
    row.push(cell);
    cell = '';
    atStart = true;
  };
  const endRow = () => {
    endCell();
    rows.push(row);
    row = [];
  };
  while (i < n) {
    const ch = text[i];
    if (atStart && ch === '"') {
      const close = closingQuote(text, i + 1);
      if (close >= 0) {
        cell = text.slice(i + 1, close).replace(/""/g, '"');
        atStart = false;
        i = close + 1;
        continue;
      }
    }
    if (ch === '\t') {
      endCell();
      i++;
    } else if (ch === '\r' || ch === '\n') {
      endRow();
      i += ch === '\r' && text[i + 1] === '\n' ? 2 : 1;
    } else {
      cell += ch;
      atStart = false;
      i++;
    }
  }
  // Text after the last line end is a last row; nothing after it means the line end was the
  // trailing one.
  if (!atStart || cell !== '' || row.length > 0) endRow();
  return rows;
}

/** The index of the quote closing a quoted cell whose text starts at `from`, or -1. */
function closingQuote(text: string, from: number): number {
  for (let j = from; j < text.length; j++) {
    if (text[j] !== '"') continue;
    if (text[j + 1] === '"') {
      j++;
      continue;
    }
    return j;
  }
  return -1;
}

/**
 * Rows of cells as a spreadsheet writes them: tabs, CRLF after every row. A cell holding a tab,
 * a line end or a quote is quoted, its quotes doubled.
 */
export function toTsv(rows: readonly (readonly string[])[]): string {
  return rows.map((r) => r.map(quote).join('\t') + '\r\n').join('');
}

function quote(cell: string): string {
  return /[\t\r\n"]/.test(cell) ? `"${cell.replace(/"/g, '""')}"` : cell;
}

/**
 * A value's exact text: the shortest spelling that reads back to the same double (JavaScript's
 * `String(number)`, which is that by definition), -0 kept as `-0`. A non-finite value arrives
 * as a string and stays as it is.
 */
export function formatExact(v: F64): string {
  if (typeof v === 'string') return v;
  return Object.is(v, -0) ? '-0' : String(v);
}

/** The prefix of a display value that is rounded. */
export const ROUNDED_MARK = '≈';

/**
 * A value as a grid cell shows it: two decimals, or three when that is exact (`0.10`, `0.013`).
 * A value neither spells exactly is shown to two decimals behind `≈`, and `rounded` is set: the
 * display never rounds without marking it. Copy always takes `formatExact`.
 */
export function displayValue(v: F64): { text: string; rounded: boolean } {
  if (typeof v === 'string') return { text: v, rounded: false };
  if (Object.is(v, -0)) return { text: '-0.00', rounded: false };
  for (const digits of [2, 3]) {
    const s = v.toFixed(digits);
    if (Number(s) === v) return { text: s, rounded: false };
  }
  return { text: `${ROUNDED_MARK}${v.toFixed(2)}`, rounded: true };
}
