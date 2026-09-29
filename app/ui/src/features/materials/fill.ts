// Row fill (Ctrl+R and the "Fill row" button) and the grid's rectangle arithmetic (PLAN.md 6.2).
// A pure module: no store, no backend, only erasable TypeScript, tested by fill.test.ts under
// `node --test`.
import type { F64 } from './bands.ts';

export interface Cell {
  row: number;
  /** A band column; -1 is the name column. */
  col: number;
}

/** A rectangle of cells, inclusive and ordered: r0 <= r1, c0 <= c1. */
export interface Rect {
  r0: number;
  r1: number;
  c0: number;
  c1: number;
}

/** The rectangle spanned by the focused cell and the selection's other corner. */
export function rectOf(a: Cell, b: Cell): Rect {
  return {
    r0: Math.min(a.row, b.row),
    r1: Math.max(a.row, b.row),
    c0: Math.min(a.col, b.col),
    c1: Math.max(a.col, b.col),
  };
}

export function inRect(r: Rect, row: number, col: number): boolean {
  return row >= r.r0 && row <= r.r1 && col >= r.c0 && col <= r.c1;
}

/** A cell clamped into a grid of `rows` by `cols` band columns (the name column is -1). */
export function clampCell(c: Cell, rows: number, cols: number): Cell {
  return {
    row: Math.min(Math.max(c.row, 0), Math.max(rows - 1, 0)),
    col: Math.min(Math.max(c.col, -1), Math.max(cols - 1, -1)),
  };
}

/** The bounding rectangle of some cells, or null for none. */
export function boundsOf(cells: readonly Cell[]): Rect | null {
  if (cells.length === 0) return null;
  let r = rectOf(cells[0], cells[0]);
  for (const c of cells) {
    r = { r0: Math.min(r.r0, c.row), r1: Math.max(r.r1, c.row), c0: Math.min(r.c0, c.col), c1: Math.max(r.c1, c.col) };
  }
  return r;
}

const same = (a: F64, b: F64) => Object.is(a, b);

/**
 * The cells a row fill changes. `values[row][col]` is the grid as displayed (band columns only).
 *
 * - A selection one band column wide (a single cell, or cells in one column): each selected
 *   row takes that column's value in every band, across its whole row.
 * - A wider selection: each selected row takes its leftmost selected value in the other selected
 *   columns (a spreadsheet's fill right).
 *
 * The name column is not a value: a selection that starts on it fills from the first band.
 * Cells that already hold the value are left out, so a fill that changes nothing issues no op.
 */
export function planFill(values: readonly (readonly F64[])[], rect: Rect): (Cell & { value: F64 })[] {
  const out: (Cell & { value: F64 })[] = [];
  const c0 = Math.max(rect.c0, 0);
  const c1 = Math.max(rect.c1, c0);
  for (let row = rect.r0; row <= rect.r1; row++) {
    const line = values[row];
    if (!line || c0 >= line.length) continue;
    const value = line[c0];
    const [from, to] = c1 === c0 ? [0, line.length - 1] : [c0 + 1, Math.min(c1, line.length - 1)];
    for (let col = from; col <= to; col++) {
      if (!same(line[col], value)) out.push({ row, col, value });
    }
  }
  return out;
}
