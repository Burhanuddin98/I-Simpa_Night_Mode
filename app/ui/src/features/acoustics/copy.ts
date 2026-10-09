// R58 (parity, upstream IHM/PropGrid.cpp:170-205, "copy the selected cells"): a table of the
// Acoustics tab or the response window copied as tab-separated text, the cells as shown, so a
// spreadsheet pastes it into rows and columns. A pure module for the text (tested by copy.test.ts
// under `node --test`); `tableCells` reads a table on the page.

/** One cell's text for TSV: its whitespace runs (tabs and line breaks too) made one space, trimmed,
 * so a cell never splits a row or a column. */
export function tsvCell(text: string): string {
  return text.replace(/\s+/g, ' ').trim();
}

/** Rows of cells as TSV: tabs between cells, CRLF between rows (what Windows spreadsheets paste). */
export function tsv(rows: readonly (readonly string[])[]): string {
  return rows.map((r) => r.map(tsvCell).join('\t')).join('\r\n');
}

/** The pieces of a cell that read as separate words: a value's range, its status tag, a note, a
 * refusal's parts. A space goes before each, so `0.60` and `0.59 – 0.61` do not run together. */
const PIECES = '.ac-range, .ac-tag, .ac-note, .ac-refusal > *';

/** The text of every row of `table`, header rows first, each cell as it reads on screen. */
export function tableCells(table: HTMLTableElement): string[][] {
  return [...table.rows].map((row) =>
    [...row.cells].map((c) => {
      const copy = c.cloneNode(true) as Element;
      copy.querySelectorAll(PIECES).forEach((e) => e.before(' '));
      return tsvCell(copy.textContent ?? '');
    }),
  );
}

/** Puts `text` on the clipboard: the async clipboard, else the page's copy command. */
export async function copyText(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    return;
  } catch {
    // The webview may refuse the async clipboard; the copy command below needs no permission.
  }
  const area = document.createElement('textarea');
  area.value = text;
  area.setAttribute('readonly', '');
  area.style.position = 'fixed';
  area.style.opacity = '0';
  document.body.appendChild(area);
  area.select();
  const ok = document.execCommand('copy');
  area.remove();
  if (!ok) throw new Error('the clipboard refused the copy');
}
