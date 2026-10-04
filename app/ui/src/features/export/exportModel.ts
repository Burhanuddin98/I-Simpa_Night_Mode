// What W9 exports (wow list; parity R56, R63; docs/investigations/2026-10-04-wow-w2w3w9/PLAN.md),
// before any file is written. Pure, tested by exportModel.test.ts under `node --test`.
//
// - The parameters are the Acoustics tab's cells (`cell()` in acoustics/model.ts): the bed's PASS
//   parameters only (gate (b)), a value only with its range and status, or its refusal (row 37
//   (3)). Each row carries the report path it was read from, and its numbers are the report's own
//   doubles, printed as JavaScript prints a double (the shortest text that reads back to it), so a
//   number in the file is the report's number, not the tab's rounding of it.
// - Nothing is written here. `actions.ts` asks the save dialog for the path and the core's
//   `export_write` writes the bytes, refusing a path without the kind's extension.
import type { Report, ReportView } from '../../bindings/ipc';
import { at, cell, MQ2_WORDING, paramMarks, paramPath, receivers, shownParams, sources, type BandSel, type SourceSel } from '../acoustics/model.ts';

/** The kinds the core writes, by extension, with the save dialog's filter. */
export const EXPORT_KINDS = {
  csv: { name: 'CSV table', what: 'parameters' },
  json: { name: 'JSON', what: 'parameters' },
  png: { name: 'PNG image', what: 'view' },
} as const;
export type ExportKind = keyof typeof EXPORT_KINDS;

export interface ParamRow {
  receiver: string;
  /** The source whose echogram the value is read from; `all` for the sources summed. */
  source: string;
  parameter: string;
  label: string;
  unit: string;
  /** A band in Hz, `sum` for the bands summed, `all` for a receiver-wide parameter. */
  band: string;
  status: 'ok' | 'wide' | 'value' | 'refused' | 'not shown';
  value: number | null;
  lo: number | null;
  hi: number | null;
  refusal: string | null;
  why: string | null;
  note: string | null;
  marks: string;
  /** The report path of the parameter: its value is at `<path>.value`, its range at `.lo` and `.hi`. */
  path: string;
}

const numAt = (r: Report, path: string | undefined): number | null => {
  if (!path) return null;
  const v = at(r, path);
  return typeof v === 'number' && Number.isFinite(v) ? v : null;
};

/** Every exported row: per source (the sources summed first), receiver, shown parameter and band. */
export function paramRows(report: Report, sourceOrder: readonly string[] = []): ParamRow[] {
  const specs = shownParams(report);
  const marks = paramMarks(report);
  const names = receivers(report);
  const srcs: SourceSel[] = [null, ...sources(report, sourceOrder)];
  const bands: BandSel[] = [...report.bands_hz.map((_, i) => i), 'sum'];
  const out: ParamRow[] = [];
  for (const src of srcs) {
    for (let r = 0; r < names.length; r++) {
      for (const spec of specs) {
        for (const b of spec.scope === 'receiver' ? [0 as BandSel] : bands) {
          const path = paramPath(report, spec, r, b, src);
          if (path === null) continue;
          const c = cell(report, spec, r, b, src);
          const band = spec.scope === 'receiver' ? 'all' : b === 'sum' ? 'sum' : String(report.bands_hz[b]);
          const row: ParamRow = {
            receiver: names[r],
            source: src ?? 'all',
            parameter: spec.name,
            label: spec.label,
            unit: spec.unit,
            band,
            status: 'not shown',
            value: null,
            lo: null,
            hi: null,
            refusal: null,
            why: null,
            note: null,
            marks: marks.filter((m) => m.param === spec.name).map((m) => m.text).join(' | '),
            path,
          };
          if (c) {
            row.status = c.status;
            row.value = numAt(report, c.value?.path);
            row.lo = numAt(report, c.lo?.path);
            row.hi = numAt(report, c.hi?.path);
            row.refusal = c.refusal?.code.text ?? null;
            row.why = c.refusal?.why?.text ?? null;
            row.note = c.note;
          }
          out.push(row);
        }
      }
    }
  }
  return out;
}

const COLUMNS: (keyof ParamRow)[] = ['receiver', 'source', 'parameter', 'label', 'unit', 'band', 'status', 'value', 'lo', 'hi', 'refusal', 'why', 'note', 'marks', 'path'];
const HEADER = COLUMNS.map((c) => (c === 'band' ? 'band_hz' : c));

/** One CSV field (RFC 4180): quoted when it holds a comma, a quote or a line break. */
export function csvField(s: string): string {
  return /[",\r\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

const text = (v: ParamRow[keyof ParamRow]) => (v === null ? '' : typeof v === 'number' ? String(v) : v);

/** The rows as CSV, CRLF line ends, a header first. */
export function paramsCsv(rows: ParamRow[]): string {
  const lines = [HEADER.join(','), ...rows.map((r) => COLUMNS.map((c) => csvField(text(r[c]))).join(','))];
  return lines.join('\r\n') + '\r\n';
}

/** Reads RFC 4180 CSV back (the tests' and the e2e's check of what was written). */
export function parseCsv(s: string): string[][] {
  const out: string[][] = [];
  let row: string[] = [];
  let f = '';
  let q = false;
  for (let i = 0; i < s.length; i++) {
    const ch = s[i];
    if (q) {
      if (ch === '"' && s[i + 1] === '"') {
        f += '"';
        i++;
      } else if (ch === '"') q = false;
      else f += ch;
    } else if (ch === '"') q = true;
    else if (ch === ',') {
      row.push(f);
      f = '';
    } else if (ch === '\r' && s[i + 1] === '\n') {
      row.push(f);
      out.push(row);
      row = [];
      f = '';
      i++;
    } else f += ch;
  }
  if (f !== '' || row.length) {
    row.push(f);
    out.push(row);
  }
  return out;
}

/** The parameters as JSON: the rows, the run, the bands, the words and marks shown with them. */
export function paramsJson(report: Report, o: { run: string; project: string | null; sourceOrder?: readonly string[] }): string {
  return JSON.stringify(
    {
      format: 'night-mode-parameters',
      version: 1,
      project: o.project,
      run: o.run,
      solver: report.solver,
      bands_hz: report.bands_hz,
      wording: MQ2_WORDING,
      marks: paramMarks(report),
      rows: paramRows(report, o.sourceOrder),
    },
    null,
    2,
  );
}

/** Why the parameters cannot be exported now, or null. */
export function exportRefusal(o: { onResults: boolean; run: string | null; view: ReportView | null }): string | null {
  if (!o.onResults) return 'Parameters are exported from the Results step';
  if (!o.run) return 'Select a run first';
  if (!o.view) return "The run's results are still being read";
  if (!o.view.report) return "This run's results are refused: there is nothing to export";
  if (paramRows(o.view.report).length === 0) return 'This run has no parameters to export';
  return null;
}

/** A file name: the project and run, then what it holds. */
export function exportName(project: string | null, runNumber: number | null, kind: ExportKind): string {
  const what = EXPORT_KINDS[kind].what;
  const safe = (s: string) => s.replace(/[\\/:*?"<>|]/g, '_');
  if (!project || runNumber === null) return `${what}.${kind}`;
  return `${safe(project)} - run ${runNumber} - ${what}.${kind}`;
}
