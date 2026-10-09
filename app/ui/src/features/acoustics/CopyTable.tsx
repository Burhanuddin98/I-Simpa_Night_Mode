// R58: the Copy button of a table (copy.ts makes the text).
import { log } from '../../store';
import { copyText, tableCells, tsv } from './copy';

/** R58: copies the table `part` of this card (or of the response window's table box) as
 * tab-separated text, its cells as they read, for a spreadsheet. */
export function CopyTable({ part, what }: { part: string; what: string }) {
  return (
    <button
      type="button"
      className="small-button ac-copy"
      data-action="copy-table"
      data-table={part}
      title={`Copy the ${what} table as tab-separated text, to paste into a spreadsheet`}
      onClick={(e) => {
        const table = e.currentTarget.closest('.ac-card, .rw-table-box, .rw')?.querySelector<HTMLTableElement>(`table[data-part="${part}"]`);
        if (!table) return;
        const rows = tableCells(table);
        copyText(tsv(rows)).then(
          () => log('OK', `Copied the ${what} table (${rows.length} rows) as tab-separated text`),
          (err: unknown) => log('FAIL', `Could not copy the ${what} table: ${err instanceof Error ? err.message : String(err)}`),
        );
      }}
    >
      Copy
    </button>
  );
}
