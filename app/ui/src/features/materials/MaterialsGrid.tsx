// The materials grid (PLAN.md 2.3 and 6.2): one row per library material, in project order by
// default, one column per band in ascending frequency, for the quantity the switch shows.
//
// - A focus cell and a rectangular selection; arrows, Tab, Enter or F2 to edit, Esc to cancel.
// - Ctrl+C copies the selection as TSV of the exact shortest round-trip values; Ctrl+V pastes at
//   the focused cell, all or nothing, as one `batch` (one undo step); Ctrl+R fills the row.
// - The display never rounds without marking it (`≈`).
// - Messages: the validator's issues on the cells, the checked apply's refusals, and the UI's own
//   refusals, as "FAIL <CODE>: message" under the grid with the cells outlined.
//
// It writes only through `actions.apply` with `ops`.
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent } from 'react';
import * as actions from '../../actions';
import type { Material, Op } from '../../bindings/schema';
import { fieldKey } from '../../issues';
import { NOT_A_NUMBER, parseStrictDecimal } from '../../numbers';
import { addMaterial, batch, nextName, removeMaterial, rename, setMaterialBand } from '../../ops';
import { refusalStore, sceneStore, Store, useStore } from '../../store';
import { registerHook } from '../../testhooks';
import { bandColumns, type BandColumn, type F64 } from './bands';
import { boundsOf, clampCell, inRect, planFill, rectOf, type Cell } from './fill';
import { dismiss, errorOf, IssueLines, lineOf, visibleRefusals, type Line } from './inline';
import { newMaterial, nextSort, sortRows, transmissionText, usage, type Quantity, type SortState } from './model';
import { planPaste } from './paste';
import { displayValue, formatExact, ROUNDED_MARK, toTsv } from './tsv';

/** Refusals of a paste or a fill (one batch over many cells). A single cell's edit is filed
 * under its own field key, `material:<id>:<quantity>/<band>`. */
const VALUES_KEY = fieldKey('material', '*', 'values');
/** Refusals of adding or deleting a row. */
const ROWS_KEY = fieldKey('material', '*', 'rows');
const isMaterialKey = (key: string) => key.startsWith('material:');

/** The grid's view state, kept across step switches; reset when another project is loaded. */
interface GridUi {
  /** The load it belongs to (see `load` below). */
  load: string;
  quantity: Quantity;
  sort: SortState;
  cursor: Cell;
  extent: Cell;
}
const INITIAL: GridUi = { load: '', quantity: 'absorption', sort: { kind: 'project' }, cursor: { row: 0, col: 0 }, extent: { row: 0, col: 0 } };
const uiStore = new Store<GridUi>(INITIAL);
const setUi = (patch: Partial<GridUi>) => uiStore.set({ ...uiStore.get(), ...patch });

/** The UI's own refusals and failed commands, for the project load they were made in. */
interface LocalIssue {
  code: string;
  message: string;
  /** Outlined cells: `<material id>/<quantity>/<band>`, `<material id>/name` or `<material id>/`. */
  cells: string[];
}
const localStore = new Store<{ load: string; issues: LocalIssue[] }>({ load: '', issues: [] });

const cellKey = (id: string, q: Quantity, band: number) => `${id}/${q}/${band}`;
const QUANTITIES: readonly Quantity[] = ['absorption', 'scattering'];
const ARROWS: Record<string, [number, number]> = { ArrowUp: [-1, 0], ArrowDown: [1, 0], ArrowLeft: [0, -1], ArrowRight: [0, 1] };
const EMPTY: readonly Material[] = [];

interface Editing {
  row: number;
  col: number;
  token: number;
  initial: string;
  selectAll: boolean;
}
let tokens = 0;

/** What the page's copy and paste listeners and the test hooks reach, as of the last render. */
interface Api {
  copyText: () => string;
  paste: (text: string) => void;
  snapshot: () => unknown;
}

export function MaterialsGrid() {
  const scene = useStore(sceneStore);
  const refusals = useStore(refusalStore);
  const ui = useStore(uiStore);
  const local = useStore(localStore);
  const [editing, setEditing] = useState<Editing | null>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const editToken = useRef(0);
  const dragging = useRef(false);
  const pendingRow = useRef<string | null>(null);
  /** A Ctrl+C whose copy event has not come yet (see the key handler). */
  const copyPending = useRef(false);
  const latest = useRef<Api | null>(null);

  const view = scene?.view;
  // A new load (open, import, new) moves the geometry revision: the grid starts over.
  const load = scene ? `${scene.info.id}|${scene.info.geometry_rev}` : '';
  const materials = view?.materials ?? EMPTY;
  const frequencies = view?.bands.frequencies_hz;
  const columns = useMemo(() => bandColumns(frequencies ?? []), [frequencies]);
  const q = ui.quantity;
  const rows = useMemo(() => sortRows(materials, ui.sort, q), [materials, ui.sort, q]);
  const used = useMemo(() => (view ? usage(view) : new Map<string, number>()), [view]);
  const cur = clampCell(ui.cursor, rows.length, columns.length);
  const ext = clampCell(ui.extent, rows.length, columns.length);
  const rect = rectOf(cur, ext);

  const valueAt = (row: number, col: number): F64 => {
    const m = rows[row];
    const c = columns[col];
    return m && c ? (m[q][c.index] ?? '') : '';
  };

  useEffect(() => {
    if (uiStore.get().load === load) return;
    uiStore.set({ ...INITIAL, quantity: uiStore.get().quantity, load });
    localStore.set({ load, issues: [] });
    setEditing(null);
    editToken.current = 0;
  }, [load]);

  // A row just added: focus its name, wherever the sort put it.
  useEffect(() => {
    if (!pendingRow.current) return;
    const at = rows.findIndex((m) => m.id === pendingRow.current);
    if (at < 0) return;
    pendingRow.current = null;
    setUi({ cursor: { row: at, col: -1 }, extent: { row: at, col: -1 } });
  }, [rows]);

  // Keep the focused cell in view while the grid has focus.
  useEffect(() => {
    const grid = gridRef.current;
    if (!grid || !grid.contains(document.activeElement)) return;
    const sel = cur.col < 0 ? `[data-grid-name="${cur.row}"]` : `[data-grid-cell="${cur.row}:${cur.col}"]`;
    grid.querySelector(sel)?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }, [cur.row, cur.col]);

  // ---- edits ----------------------------------------------------------------------------------

  const setLocal = (issues: LocalIssue[]) => localStore.set({ load, issues });

  /** A new grid edit: the messages under the grid become this attempt's. */
  const attempt = () => {
    dismiss(refusalStore.get(), isMaterialKey);
    setLocal([]);
  };

  const run = (op: Op, key: string, cells: string[]) => {
    actions.apply(op, key).catch((e: unknown) => {
      const err = errorOf(e);
      setLocal([{ code: err.code, message: err.message, cells }]);
    });
  };

  const commitValue = (row: number, col: number, text: string) => {
    attempt();
    const m = rows[row];
    const c = columns[col];
    if (!m || !c) return;
    const cell = cellKey(m.id, q, c.index);
    const parsed = parseStrictDecimal(text);
    if (!parsed.ok) {
      setLocal([
        {
          code: NOT_A_NUMBER,
          message: `'${text}' at ${m.name} · ${c.label} is not a number. Write decimals with a point, as in 0.5; NaN and infinities are refused. The project is unchanged.`,
          cells: [cell],
        },
      ]);
      return;
    }
    if (Object.is(m[q][c.index], parsed.value)) return;
    run(setMaterialBand(m.id, q, c.index, parsed.value), fieldKey('material', m.id, `${q}/${c.index}`), [cell]);
  };

  const commitName = (row: number, text: string) => {
    attempt();
    const m = rows[row];
    if (!m || text === m.name) return;
    run(rename('material', m.id, text), fieldKey('material', m.id, 'name'), [`${m.id}/name`]);
  };

  const cellsOf = (cells: readonly Cell[]) =>
    cells.filter((c) => rows[c.row] && columns[c.col]).map((c) => cellKey(rows[c.row].id, q, columns[c.col].index));

  const paste = (text: string) => {
    attempt();
    const plan = planPaste(text, { columns, rows, cursor: cur, valueAt });
    if (!plan.ok) {
      setLocal(plan.issues.map((i) => ({ code: i.code, message: i.message, cells: cellsOf(i.cells) })));
      return;
    }
    const b = boundsOf(plan.covered);
    if (b) setUi({ cursor: { row: b.r0, col: b.c0 }, extent: { row: b.r1, col: b.c1 } });
    // Nothing changes: no op, so no empty undo step.
    if (plan.cells.length === 0) return;
    const ops = plan.cells.map((c) => setMaterialBand(rows[c.row].id, q, columns[c.col].index, c.value));
    run(batch(ops), VALUES_KEY, []);
  };

  const fill = () => {
    attempt();
    const values = rows.map((m) => columns.map((c) => m[q][c.index] ?? ''));
    const cells = planFill(values, rect);
    if (cells.length === 0) return;
    run(batch(cells.map((c) => setMaterialBand(rows[c.row].id, q, columns[c.col].index, c.value))), VALUES_KEY, []);
  };

  const addRow = () => {
    if (!view) return;
    attempt();
    const id = crypto.randomUUID();
    const name = nextName('Material ', materials.map((m) => m.name));
    pendingRow.current = id;
    run(addMaterial(materials.length, newMaterial(id, name, view.bands.frequencies_hz.length, materials.length)), ROWS_KEY, []);
  };

  const deleteRow = () => {
    attempt();
    const m = rows[cur.row];
    if (!m) return;
    run(removeMaterial(m.id), ROWS_KEY, [`${m.id}/`]);
  };

  const copyText = () => {
    const lines: string[][] = [];
    for (let row = rect.r0; row <= rect.r1; row++) {
      const m = rows[row];
      if (!m) continue;
      const line: string[] = [];
      for (let col = rect.c0; col <= rect.c1; col++) line.push(col < 0 ? m.name : formatExact(valueAt(row, col)));
      lines.push(line);
    }
    return toTsv(lines);
  };

  // ---- editing --------------------------------------------------------------------------------

  const exactText = (cell: Cell) => (cell.col < 0 ? (rows[cell.row]?.name ?? '') : formatExact(valueAt(cell.row, cell.col)));

  const beginEdit = (cell: Cell, initial?: string, selectAll = true) => {
    if (!rows[cell.row] || (cell.col >= 0 && !columns[cell.col])) return;
    const token = ++tokens;
    editToken.current = token;
    setEditing({ row: cell.row, col: cell.col, token, initial: initial ?? exactText(cell), selectAll });
  };

  const moveTo = (cell: Cell, extend = false) => {
    const next = clampCell(cell, rows.length, columns.length);
    if (extend) setUi({ extent: next });
    else setUi({ cursor: next, extent: next });
  };

  const finishEdit = (ed: Editing, text: string | null, move: [number, number] | null) => {
    if (editToken.current !== ed.token) return;
    editToken.current = 0;
    setEditing(null);
    if (move || text === null) gridRef.current?.focus({ preventScroll: true });
    if (move) moveTo({ row: ed.row + move[0], col: ed.col + move[1] });
    if (text === null) return;
    if (ed.col < 0) commitName(ed.row, text);
    else commitValue(ed.row, ed.col, text);
  };

  // ---- keys and mouse -------------------------------------------------------------------------

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (editing || e.target instanceof HTMLInputElement) return;
    const ctrl = e.ctrlKey || e.metaKey;
    const handled = () => {
      e.preventDefault();
      e.stopPropagation();
    };
    if (ctrl) {
      if (e.altKey) return;
      const k = e.key.toLowerCase();
      if (k === 'c' && !e.shiftKey) {
        // The page's copy event answers Ctrl+C. Should the webview send none (nothing is
        // selected on the page), the clipboard API writes the same text instead.
        copyPending.current = true;
        setTimeout(() => {
          if (!copyPending.current) return;
          copyPending.current = false;
          const text = latest.current?.copyText();
          if (text !== undefined) navigator.clipboard?.writeText(text).catch(() => {});
        }, 0);
      } else if (k === 'r' && !e.shiftKey) {
        handled();
        fill();
      } else if (k === 'a' && !e.shiftKey) {
        handled();
        setUi({ cursor: { row: 0, col: 0 }, extent: { row: Math.max(rows.length - 1, 0), col: Math.max(columns.length - 1, 0) } });
      }
      // Ctrl+C and Ctrl+V go on to the page's copy and paste events; undo, redo, save and open
      // are the app's global keys.
      return;
    }
    if (rows.length === 0) return;
    const from = e.shiftKey ? ext : cur;
    switch (e.key) {
      case 'ArrowUp':
      case 'ArrowDown':
      case 'ArrowLeft':
      case 'ArrowRight': {
        handled();
        const [dr, dc] = ARROWS[e.key];
        moveTo({ row: from.row + dr, col: from.col + dc }, e.shiftKey);
        return;
      }
      case 'Home':
      case 'End':
        handled();
        moveTo({ row: from.row, col: e.key === 'Home' ? -1 : columns.length - 1 }, e.shiftKey);
        return;
      case 'Tab': {
        const last = cur.row === rows.length - 1 && cur.col === columns.length - 1;
        const first = cur.row === 0 && cur.col === -1;
        if (e.shiftKey ? first : last) return; // let focus leave the grid
        handled();
        if (e.shiftKey) moveTo(cur.col > -1 ? { row: cur.row, col: cur.col - 1 } : { row: cur.row - 1, col: columns.length - 1 });
        else moveTo(cur.col < columns.length - 1 ? { row: cur.row, col: cur.col + 1 } : { row: cur.row + 1, col: 0 });
        return;
      }
      case 'Enter':
        handled();
        beginEdit(cur);
        return;
      case 'F2':
        handled();
        beginEdit(cur, undefined, false);
        return;
      case 'Backspace':
        handled();
        beginEdit(cur, '', false);
        return;
      case 'Escape':
        handled();
        moveTo(cur);
        return;
      case 'Delete':
        // A band value has no empty state, and rows go through the Delete button: the key
        // does nothing here, and stops so that no global handler deletes something else.
        handled();
        return;
    }
    if (e.key.length === 1 && !e.altKey) {
      // Typing starts an edit with that character, as a spreadsheet does.
      handled();
      beginEdit(cur, e.key, false);
    }
  };

  const cellAt = (target: EventTarget | null): Cell | null => {
    if (!(target instanceof Element)) return null;
    const el = target.closest<HTMLElement>('[data-grid-cell], [data-grid-name]');
    if (!el) return null;
    if (el.dataset.gridName !== undefined) return { row: Number(el.dataset.gridName), col: -1 };
    const [row, col] = (el.dataset.gridCell ?? '').split(':').map(Number);
    return { row, col };
  };

  const onMouseDown = (e: MouseEvent<HTMLDivElement>) => {
    if (e.button !== 0 || (e.target instanceof Element && e.target.closest('[data-grid-editor]'))) return;
    const cell = cellAt(e.target);
    if (!cell) return;
    e.preventDefault();
    // Focusing the grid first ends an open edit through its blur, committing it.
    gridRef.current?.focus({ preventScroll: true });
    moveTo(cell, e.shiftKey);
    dragging.current = true;
  };

  const onMouseOver = (e: MouseEvent<HTMLDivElement>) => {
    if (!dragging.current || (e.buttons & 1) === 0) return;
    const cell = cellAt(e.target);
    if (cell) moveTo(cell, true);
  };

  const onDoubleClick = (e: MouseEvent<HTMLDivElement>) => {
    const cell = cellAt(e.target);
    if (cell && !editing) beginEdit(cell);
  };

  useEffect(() => {
    const up = () => (dragging.current = false);
    window.addEventListener('mouseup', up);
    return () => window.removeEventListener('mouseup', up);
  }, []);

  // ---- the page's copy and paste, and the test hooks ------------------------------------------

  const api: Api = {
    copyText,
    paste,
    snapshot: () => ({
      quantity: q,
      sort: ui.sort,
      columns: columns.map((c) => ({ hz: c.hz, label: c.label, index: c.index })),
      rows: rows.map((m) => ({ id: m.id, name: m.name, values: columns.map((c) => formatExact(m[q][c.index] ?? '')) })),
      cursor: cur,
      extent: ext,
    }),
  };
  useLayoutEffect(() => {
    latest.current = api;
  });

  useEffect(() => {
    /** A copy or paste the grid answers: focus or target in the grid, and not in a text field
     * (the cell editor keeps the browser's own clipboard behaviour). */
    const mine = (e: Event) => {
      const grid = gridRef.current;
      const active = document.activeElement;
      if (!grid || active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement) return false;
      return grid.contains(active) || (e.target instanceof Node && grid.contains(e.target));
    };
    const onCopy = (e: ClipboardEvent) => {
      if (!mine(e) || !e.clipboardData || !latest.current) return;
      copyPending.current = false;
      e.clipboardData.setData('text/plain', latest.current.copyText());
      e.preventDefault();
    };
    const onPaste = (e: ClipboardEvent) => {
      if (!mine(e) || !latest.current) return;
      e.preventDefault();
      latest.current.paste(e.clipboardData?.getData('text/plain') ?? '');
    };
    document.addEventListener('copy', onCopy);
    document.addEventListener('paste', onPaste);
    const off = [
      registerHook('materialsGrid', () => latest.current?.snapshot()),
      registerHook('materialsCopy', () => latest.current?.copyText()),
    ];
    return () => {
      document.removeEventListener('copy', onCopy);
      document.removeEventListener('paste', onPaste);
      off.forEach((f) => f());
    };
  }, []);

  // ---- messages -------------------------------------------------------------------------------

  const lines: Line[] = [];
  const bad = new Map<string, string>();
  const outline = (key: string, code: string) => {
    if (!bad.has(key)) bad.set(key, code);
  };
  if (local.load === load) {
    for (const i of local.issues) {
      lines.push({ label: 'FAIL', code: i.code, message: i.message });
      i.cells.forEach((k) => outline(k, i.code));
    }
  }
  const located = (issue: { entity?: { kind: string; id: string } | null; field: string; code: string }) => {
    if (issue.entity?.kind === 'material') outline(`${issue.entity.id}/${issue.field}`, issue.code);
  };
  for (const r of visibleRefusals(refusals, isMaterialKey)) {
    lines.push(lineOf(r));
    located(r);
  }
  for (const i of scene?.issues ?? []) {
    if (i.entity?.kind !== 'material') continue;
    lines.push(lineOf(i));
    located(i);
  }

  if (!view) return null;
  const focused = rows[cur.row];
  const anyRounded = rows.some((m) => columns.some((c) => displayValue(m[q][c.index] ?? '').rounded));
  /** Toolbar buttons keep the grid's focus, unless an edit is open: then the click's blur
   * commits it first. */
  const keepFocus = (e: MouseEvent<HTMLButtonElement>) => {
    if (!editing) e.preventDefault();
  };
  const tool = (action: () => void) => () => {
    action();
    gridRef.current?.focus({ preventScroll: true });
  };

  const renderEditor = (ed: Editing) => (
    <CellEditor
      key={ed.token}
      initial={ed.initial}
      selectAll={ed.selectAll}
      onCommit={(text, move) => finishEdit(ed, text, move)}
      onCancel={() => finishEdit(ed, null, null)}
    />
  );

  const sortMark = (active: boolean, dir: 1 | -1) => (active ? <span className="mg-sort">{dir === 1 ? '▲' : '▼'}</span> : null);
  const ariaSort = (active: boolean, dir: 1 | -1) => (active ? (dir === 1 ? 'ascending' : 'descending') : undefined);
  const nameSorted = ui.sort.kind === 'name';

  return (
    <section className="mat-sec mat-lib" aria-label="Material library">
      <div className="mat-sec-head">
        <span className="label">Library</span>
        <div className="segmented" role="tablist" aria-label="Quantity">
          {QUANTITIES.map((k) => (
            <button
              key={k}
              role="tab"
              aria-selected={q === k}
              data-quantity-tab={k}
              onClick={() => setUi({ quantity: k })}
            >
              {k === 'absorption' ? 'Absorption' : 'Scattering'}
            </button>
          ))}
        </div>
      </div>
      <div
        ref={gridRef}
        className="mg-wrap"
        tabIndex={0}
        data-materials-grid=""
        data-quantity={q}
        onKeyDown={onKeyDown}
        onMouseDown={onMouseDown}
        onMouseOver={onMouseOver}
        onDoubleClick={onDoubleClick}
      >
        <div className="mg-scroll">
          <table className="mg" role="grid" aria-label={`Materials, ${q === 'absorption' ? 'absorption α' : 'scattering'} per band`} aria-multiselectable="true">
            <thead>
              <tr>
                <th
                  className="mg-name mg-head"
                  aria-sort={ariaSort(nameSorted, ui.sort.kind === 'name' ? ui.sort.dir : 1)}
                  onClick={() => setUi({ sort: nextSort(ui.sort, 'name') })}
                  title="Sort by name (natural order); a third click restores project order"
                >
                  Material{sortMark(nameSorted, ui.sort.kind === 'name' ? ui.sort.dir : 1)}
                </th>
                {columns.map((c: BandColumn, col) => {
                  const on = ui.sort.kind === 'band' && ui.sort.band === c.index;
                  const dir = ui.sort.kind === 'band' ? ui.sort.dir : 1;
                  return (
                    <th
                      key={c.index}
                      className="mg-head mg-band"
                      data-band-hz={c.hz}
                      data-grid-col={col}
                      aria-sort={ariaSort(on, dir)}
                      title={`${c.hz} Hz: sort by this band's value`}
                      onClick={() => setUi({ sort: nextSort(ui.sort, c.index) })}
                    >
                      <span data-part="band-label">{c.label}</span>
                      {sortMark(on, dir)}
                    </th>
                  );
                })}
              </tr>
            </thead>
            <tbody data-input="">
              {rows.map((m, row) => {
                const nameEditing = editing && editing.row === row && editing.col === -1;
                const nameBad = bad.get(`${m.id}/name`) ?? bad.get(`${m.id}/`);
                const nameCls = ['mg-name', inRect(rect, row, -1) ? 'sel' : '', cur.row === row && cur.col === -1 ? 'cur' : '', nameBad ? 'bad' : '']
                  .filter(Boolean)
                  .join(' ');
                return (
                  <tr key={m.id} data-material-id={m.id}>
                    <th scope="row" className={nameCls} data-grid-name={row} aria-selected={inRect(rect, row, -1)} data-cell-issue={nameBad}>
                      {nameEditing && editing ? (
                        renderEditor(editing)
                      ) : (
                        <span className="mg-namecell">
                          <span className="mat-swatch" style={{ background: m.color }} />
                          <span className="mg-label">
                            <span className="mg-text" title={m.name}>
                              {m.name}
                            </span>
                            <span className="mg-used">used by {used.get(m.id) ?? 0}</span>
                          </span>
                        </span>
                      )}
                    </th>
                    {columns.map((c, col) => {
                      const v = m[q][c.index] ?? '';
                      const shown = displayValue(v);
                      const issue = bad.get(cellKey(m.id, q, c.index));
                      const isEditing = editing && editing.row === row && editing.col === col;
                      const cls = [
                        'mg-cell',
                        inRect(rect, row, col) ? 'sel' : '',
                        cur.row === row && cur.col === col ? 'cur' : '',
                        issue ? 'bad' : '',
                        shown.rounded ? 'rounded' : '',
                      ]
                        .filter(Boolean)
                        .join(' ');
                      return (
                        <td
                          key={c.index}
                          className={cls}
                          data-grid-cell={`${row}:${col}`}
                          aria-selected={inRect(rect, row, col)}
                          data-cell-issue={issue}
                          title={`${m.name} · ${c.label}: ${formatExact(v)}${issue ? ` · FAIL ${issue}` : ''}`}
                        >
                          {isEditing && editing ? renderEditor(editing) : shown.text}
                        </td>
                      );
                    })}
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
        {rows.length === 0 && <div className="mg-empty empty">No materials yet: + Material adds one.</div>}
      </div>
      <div className="mg-tools">
        <button type="button" data-action="add-material" onMouseDown={keepFocus} onClick={tool(addRow)}>
          + Material
        </button>
        <button
          type="button"
          data-action="delete-material"
          disabled={!focused}
          title={focused ? `Delete ${focused.name}` : undefined}
          onMouseDown={keepFocus}
          onClick={tool(deleteRow)}
        >
          Delete
        </button>
        <button
          type="button"
          data-action="fill-row"
          disabled={!focused || columns.length === 0}
          title="Copy the focused cell across its row (one undo step)"
          onMouseDown={keepFocus}
          onClick={tool(fill)}
        >
          Fill row<span className="kbd">Ctrl R</span>
        </button>
      </div>
      {anyRounded && (
        <div className="mg-note">{ROUNDED_MARK} rounded for display. The stored value, and what Ctrl+C copies, are exact.</div>
      )}
      <IssueLines lines={lines} part="grid-issues" />
      {focused && (
        <div className="mat-row" title="Transmission is read-only in this version">
          <span>
            Transmission <span className="mg-of">· {focused.name}</span>
          </span>
          <span className="mono">{transmissionText(focused)}</span>
        </div>
      )}
    </section>
  );
}

/** The editor of one cell: commits on Enter (then down), Tab (then across) or blur; Esc cancels. */
function CellEditor({
  initial,
  selectAll,
  onCommit,
  onCancel,
}: {
  initial: string;
  selectAll: boolean;
  onCommit: (text: string, move: [number, number] | null) => void;
  onCancel: () => void;
}) {
  const [text, setText] = useState(initial);
  const ref = useRef<HTMLInputElement>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.focus({ preventScroll: true });
    if (selectAll) el.select();
    else el.setSelectionRange(el.value.length, el.value.length);
  }, [selectAll]);
  return (
    <input
      ref={ref}
      className="mg-editor"
      data-grid-editor=""
      value={text}
      spellCheck={false}
      autoComplete="off"
      onChange={(e) => setText(e.target.value)}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === 'Enter') {
          e.preventDefault();
          onCommit(text, [e.shiftKey ? -1 : 1, 0]);
        } else if (e.key === 'Tab') {
          e.preventDefault();
          onCommit(text, [0, e.shiftKey ? -1 : 1]);
        } else if (e.key === 'Escape') {
          e.preventDefault();
          onCancel();
        }
      }}
      onBlur={() => onCommit(text, null)}
    />
  );
}
