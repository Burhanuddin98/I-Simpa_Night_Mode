// W9's actions (wow list; parity R56, R63; docs/investigations/2026-10-04-wow-w2w3w9/PLAN.md):
// File › Export view as PNG..., Export parameters as CSV... and as JSON.... Each asks the save
// dialog for a path (the e2e passes one, as its save-as hook does) and hands the bytes to the core's
// `export_write` through actions.ts, which checks the path's extension and the bytes' kind.
// R63: a chart of the Acoustics tab as a PNG ("Image…" on the chart's card), `exportChart`.
//
// The parameters are exportModel.ts's rows of the report the Acoustics tab shows; the image is the
// frame as drawn, with the shown map's legend in a strip below it (snapshot.ts).
import * as actions from '../../actions';
import { log, reportStore, runsStore, sceneStore, selectedRunStore, stepStore, Store, viewportStore } from '../../store';
import { animatorStore } from '../viewport/animator';
import { frameRgba } from '../viewport/engine';
import { stepTime } from '../viewport/mapView';
import { resultsViewStore } from '../viewport/resultsView';
import { chartFileName, exportName, exportRefusal, paramRows, paramsCsv, paramsJson } from './exportModel';
import { type ChartHeading, composite, encodeChartPng, encodeViewPng, hexRgb, over, stripLines, type StripLines } from '../viewport/snapshot';

export interface ExportDone {
  kind: 'csv' | 'json' | 'png';
  path: string;
  bytes: number;
  rows?: number;
  width?: number;
  height?: number;
  /** The PNG's strip, as drawn (null: no map shown, no strip). */
  strip?: StripLines | null;
}

/** The last export written, for the e2e. */
export const lastExportStore = new Store<ExportDone | null>(null);

/** Says why nothing was exported, in the Console, and refuses. */
function nothing(message: string): never {
  log('WARN', `Nothing exported: ${message}`);
  throw { code: 'EXPORT_NOTHING', message };
}

function runInfo(): { run: string | null; number: number | null; label: string | null } {
  const run = selectedRunStore.get();
  const row = run ? runsStore.get()?.rows.find((r) => r.run === run) : undefined;
  return { run, number: row?.number ?? null, label: row ? `Run ${row.number}` : null };
}

/** Why the parameters cannot be exported now, or null (the menu's title). A report not read yet is
 * read by the export itself, so it does not disable the item. */
export function paramsRefusal(): string | null {
  const run = selectedRunStore.get();
  const view = run ? (reportStore.get().get(run) ?? null) : null;
  const onResults = stepStore.get() === 'results';
  if (onResults && run && !view) return null;
  return exportRefusal({ onResults, run, view });
}

/** Writes the selected run's parameters as `kind` to `path`, or where the dialog says; null when cancelled. */
export async function exportParams(kind: 'csv' | 'json', path?: string): Promise<ExportDone | null> {
  const { run, number } = runInfo();
  // The report the Acoustics tab shows, read now if the tab has not read it.
  const view = run && stepStore.get() === 'results' ? await actions.reportFor(run) : null;
  const why = exportRefusal({ onResults: stepStore.get() === 'results', run, view });
  if (why) nothing(why);
  const report = view?.report;
  if (!run || !report) return nothing('No report to export');
  const project = sceneStore.get()?.info.name ?? null;
  const order = (sceneStore.get()?.view.sources ?? []).map((s) => s.name);
  const target = await actions.exportPath(kind, exportName(project, number, kind), path);
  if (target === null) return null;
  const rows = paramRows(report, order);
  const text = kind === 'csv' ? paramsCsv(rows) : paramsJson(report, { run, project, sourceOrder: order });
  const bytes = await actions.exportWrite(kind, target, new TextEncoder().encode(text), `the parameters of ${run}`, run);
  const done: ExportDone = { kind, path: target, bytes, rows: rows.length };
  lastExportStore.set(done);
  return done;
}

const cssVar = (name: string, fallback: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;

/**
 * R63: writes a chart of the Acoustics tab as a PNG to `path`, or where the dialog says; null when
 * cancelled. The chart is its own canvas as drawn (its axes, labels and the series the legend
 * shows) over the window's background, under `heading`; it is noted beside the run it shows.
 */
export async function exportChart(canvas: HTMLCanvasElement, heading: ChartHeading, chart: string, path?: string): Promise<ExportDone | null> {
  const { run, number } = runInfo();
  if (!run || stepStore.get() !== 'results') return nothing('A chart is shown on the Results step only');
  const project = sceneStore.get()?.info.name ?? null;
  const target = await actions.exportPath('png', chartFileName(project, number, chart), path);
  if (target === null) return null;
  const g = canvas.getContext('2d');
  if (!g || canvas.width === 0 || canvas.height === 0) return nothing('The chart is not drawn');
  const px = g.getImageData(0, 0, canvas.width, canvas.height).data;
  const bgText = cssVar('--bg', '#09090b');
  const bg = hexRgb(bgText) ?? [9, 9, 11];
  const png = await encodeChartPng(
    { width: canvas.width, height: canvas.height, rgba: over(px, bg) },
    heading,
    canvas.clientWidth > 0 ? canvas.width / canvas.clientWidth : 1,
    { bg: bgText, text: cssVar('--text', '#ececee'), dim: cssVar('--text-2', '#a1a1aa'), font: cssVar('--sans', 'Inter Variable') },
  );
  const bytes = await actions.exportWrite('png', target, png, `the ${chart} chart`, run);
  const done: ExportDone = { kind: 'png', path: target, bytes, width: canvas.width, height: canvas.height };
  lastExportStore.set(done);
  return done;
}

/** Why the view cannot be exported, or null. */
export function viewRefusal(): string | null {
  return viewportStore.get().live ? null : 'The 3D view is not drawing';
}

/** Writes the 3D view as a PNG to `path`, or where the dialog says; null when cancelled. */
export async function exportView(path?: string): Promise<ExportDone | null> {
  const { number, label } = runInfo();
  const project = sceneStore.get()?.info.name ?? null;
  const target = await actions.exportPath('png', exportName(project, stepStore.get() === 'results' ? number : null, 'png'), path);
  if (target === null) return null;
  const frame = await frameRgba();
  if (!frame) return nothing('The 3D view is not drawing');
  const bgText = cssVar('--bg', '#09090b');
  const bg = hexRgb(bgText) ?? [9, 9, 11];
  const v = resultsViewStore.get();
  const onMap = stepStore.get() === 'results' && v.map !== null;
  const dt = v.data?.time_step_s ?? v.data?.surfaces[0]?.time_step_s;
  const strip = onMap
    ? stripLines({
        project,
        run: label,
        legend: v.map?.legend ?? null,
        time: stepTime(Math.min(animatorStore.get().step, Math.max(0, (v.data?.steps ?? 1) - 1)), dt),
        note: v.map?.cumulative ? 'Cumulative: each face summed from the first step.' : v.smooth && !v.smoothRefusal ? 'Smoothed between faces.' : null,
      })
    : null;
  const png = await encodeViewPng(
    { width: frame.width, height: frame.height, rgba: composite(frame.rgba, bg) },
    strip,
    v.map?.legend.gradient ?? null,
    { bg: bgText, text: cssVar('--text', '#ececee'), dim: cssVar('--text-2', '#a1a1aa'), font: cssVar('--sans', 'Inter Variable') },
  );
  // R3: on the Results step the image shows the selected run, so it is noted beside that run.
  const fromRun = stepStore.get() === 'results' ? selectedRunStore.get() : null;
  const bytes = await actions.exportWrite('png', target, png, 'the 3D view', fromRun);
  const done: ExportDone = { kind: 'png', path: target, bytes, width: frame.width, height: frame.height, strip };
  lastExportStore.set(done);
  return done;
}
