// M12 P2's checks on the Acoustics tab (docs/investigations/2026-10-03-m12/PLAN.md, gate (a), (b),
// (e), (f)), as pure functions over what the page shows and what `simpa results <run> --json`
// prints. The tab's DOM contract (app/ui/src/features/acoustics/):
//   [data-num]   a number: its text is the value at `data-digits` decimals, of the report's JSON
//                at `data-json` (a dot path), times `data-scale` when present (1 kHz: 0.001)
//   [data-str]   a string: its text is the report's JSON at `data-json`, exactly
//   [data-label] a word that is not the report's: `wording` (MQ2), `param` (a parameter's
//                name, `data-param` says which), `group` (a surface group's name, from the open
//                project), `standard` (the words `DIN 18041`)
//   [data-param] every element of one parameter (gate (b)): a column head, a cell, a series
// Every digit the tab shows is inside one of these, or in a `<select>` (whose options are checked
// on their own) or the run label (`[data-run-label]`); `strayDigits` says where one is not.
// `acoustics.test.ts` holds each rule to hand-made cases, its say-NO among them.

/** The words on the Results screen (MQ2, Burhan 2026-10-03 08:31). */
export const MQ2_WORDING =
  'Computed to ISO 3382-1 / IEC 60268-16 and checked against exact solutions to within the just-noticeable difference. Simulated with I-Simpa’s solvers; not compared with measured rooms.';

/** Decision-log row 37 (1)-(2)'s two marks, shown with EDT wherever it appears (row 37 (5)). */
export const EDT_MARKS = ['EDT unchecked for receivers larger than one metre in radius', 'EDT unchecked in energetic mode'] as const;

/** Each parameter's name on screen, by its name in beds/summary.json. */
export const PARAM_LABELS: Record<string, string> = {
  spl_db: 'SPL',
  edt_s: 'EDT',
  t20_s: 'T20',
  t30_s: 'T30',
  c50_db: 'C50',
  c80_db: 'C80',
  d50: 'D50',
  ts_s: 'Ts',
  sti: 'STI',
  g_db: 'G',
  dba: 'dB(A)',
};

/** The value at a dot path (`spps.point_receivers.0.bands.3.parameters.spl_db.value`). */
export function at(root: unknown, path: string): unknown {
  let v: unknown = root;
  for (const k of path.split('.')) {
    if (v === null || typeof v !== 'object') return undefined;
    v = Array.isArray(v) ? v[Number(k)] : (v as Record<string, unknown>)[k];
  }
  return v;
}

export interface NumEl {
  path: string;
  text: string;
  digits: string | null;
  scale: string | null;
}

/**
 * Why a shown number is not its JSON value at the displayed precision, or null when it is. Both
 * ways must hold: the text is the value rounded by `toFixed` (the same IEEE value through two
 * paths: the app's IPC and the CLI's stdout), and it lies within half a unit of the last digit
 * shown of the exact value.
 */
export function numberMismatch(n: NumEl, json: unknown): string | null {
  const v = at(json, n.path);
  if (typeof v !== 'number') return `${n.path}: the JSON holds ${JSON.stringify(v)}, not a number`;
  const d = Number(n.digits);
  if (n.digits === null || !Number.isInteger(d) || d < 0 || d > 6) return `${n.path}: data-digits ${n.digits}`;
  const scale = n.scale === null ? 1 : Number(n.scale);
  if (!Number.isFinite(scale) || scale <= 0) return `${n.path}: data-scale ${n.scale}`;
  if (!/^-?\d+(\.\d+)?$/.test(n.text)) return `${n.path}: "${n.text}" is not a plain decimal`;
  const shownDigits = (n.text.split('.')[1] ?? '').length;
  if (shownDigits !== d) return `${n.path}: "${n.text}" shows ${shownDigits} decimals, data-digits ${d}`;
  const x = v * scale;
  const want = x.toFixed(d);
  if (Number(n.text) !== Number(want)) return `${n.path}: shown ${n.text}, JSON ${v} at ${d} decimals is ${want}`;
  if (Math.abs(Number(n.text) - x) > 0.5 * 10 ** -d * (1 + 1e-9)) return `${n.path}: shown ${n.text} is not within half a digit of ${x}`;
  return null;
}

/** Why a shown string is not its JSON string, or null. */
export function stringMismatch(s: { path: string; text: string }, json: unknown): string | null {
  const v = at(json, s.path);
  if (typeof v !== 'string') return `${s.path}: the JSON holds ${JSON.stringify(v)}, not a string`;
  return v === s.text ? null : `${s.path}: shown "${s.text}", JSON "${v}"`;
}

/** The first digit in `text` with what surrounds it, or null. */
export function strayDigits(text: string): string | null {
  const m = /\d/.exec(text);
  return m ? JSON.stringify(text.slice(Math.max(0, m.index - 30), m.index + 30)) : null;
}

/** Every parameter whose status in beds/summary.json is not PASS. */
export function notPassed(summary: { parameters: Record<string, { status: string }> }): string[] {
  return Object.entries(summary.parameters)
    .filter(([, p]) => p.status !== 'PASS')
    .map(([n]) => n);
}

/** A parameter's on-screen name as a word, for a text search (`dB(A)` has no word boundary). */
export function labelPattern(name: string): RegExp {
  const label = PARAM_LABELS[name];
  if (!label) throw new Error(`no label for ${name}`);
  const esc = label.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return new RegExp(`(^|[^A-Za-z0-9])${esc}(?![A-Za-z0-9])`);
}


/** A drawn or expected RT series: per band of `bands_hz`, the value and its range, with the
 * paths they are read from. */
export interface SeriesView {
  paths: string[];
  values: (number | null)[];
  loPaths: string[];
  lo: (number | null)[];
  hiPaths: string[];
  hi: (number | null)[];
}

/**
 * A receiver's RT series as the JSON gives it, the paths the tab must have drawn from: per band
 * of `bands_hz`, the parameter's value and range where the tables show it (status `ok` or `wide`
 * with `lo` and `hi`, row 37 (3)), or null: refused, or a value no table shows. The M12 assay
 * found the chart drawing `.value` wherever it was a number; the chart goes through the tables'
 * filter, and this is that filter, written from the JSON's side.
 */
export function rtSeries(json: unknown, receiver: number, param: string): SeriesView {
  const bands = (at(json, 'bands_hz') as number[] | undefined) ?? [];
  const base = bands.map((_, b) => `spps.point_receivers.${receiver}.bands.${b}.parameters.${param}`);
  const shown = base.map((p) => {
    const e = at(json, p) as { value?: unknown; status?: unknown; lo?: unknown; hi?: unknown } | undefined;
    const n = (v: unknown) => typeof v === 'number' && Number.isFinite(v);
    return !!e && typeof e === 'object' && n(e.value) && (e.status === 'ok' || e.status === 'wide') && n(e.lo) && n(e.hi);
  });
  const read = (k: string) => base.map((p, i) => (shown[i] ? (at(json, `${p}.${k}`) as number) : null));
  return {
    paths: base.map((p) => `${p}.value`),
    values: read('value'),
    loPaths: base.map((p) => `${p}.lo`),
    lo: read('lo'),
    hiPaths: base.map((p) => `${p}.hi`),
    hi: read('hi'),
  };
}

/** Mismatches between a drawn series and the JSON's: its paths, values and ranges, exactly. */
export function seriesMismatches(drawn: SeriesView, json: unknown, receiver: number, param: string): string[] {
  const want = rtSeries(json, receiver, param);
  const out: string[] = [];
  if (drawn.values.length !== want.values.length) out.push(`${param}: ${drawn.values.length} points drawn, the JSON has ${want.values.length} bands`);
  for (const k of ['lo', 'hi'] as const) {
    if (!Array.isArray(drawn[k]) || !Array.isArray(drawn[`${k}Paths`])) out.push(`${param}: no ${k} drawn (the range is not on the chart)`);
  }
  want.values.forEach((v, i) => {
    if (drawn.paths[i] !== want.paths[i]) out.push(`${param}[${i}]: drawn from ${drawn.paths[i]}, not ${want.paths[i]}`);
    if (drawn.values[i] !== v) out.push(`${param}[${i}]: drawn ${drawn.values[i]}, JSON ${v}`);
    for (const k of ['lo', 'hi'] as const) {
      const paths = drawn[`${k}Paths`];
      const vals = drawn[k];
      if (!Array.isArray(paths) || !Array.isArray(vals)) continue;
      if (paths[i] !== want[`${k}Paths`][i]) out.push(`${param}[${i}].${k}: drawn from ${paths[i]}, not ${want[`${k}Paths`][i]}`);
      if (vals[i] !== want[k][i]) out.push(`${param}[${i}].${k}: drawn ${vals[i]}, JSON ${want[k][i]}`);
    }
  });
  return out;
}

/** Decision-log row 46's mark, beside T30 wherever it appears. */
export const T30_MARK = 'Ranges on noise-limited T30 values may be slightly narrow: 1 of 833 checked values fell 1.5 ms outside its range.';

/** Every mark the tab may show beside its receivers table (`[data-label="mark"]`), word for word:
 * row 37's two EDT marks, row 46's T30 mark, MQ3's STI note. */
export const MARKS: readonly string[] = [...EDT_MARKS, T30_MARK, 'STI: noise range not computed'];

/**
 * One cell of the tab's tables as the page shows it (gate (a), the assay's MED finding): the
 * labels a reader takes it by, read from the rendered DOM, and the JSON paths of everything in it.
 *   receivers-table  rowHead the receiver, colHead the parameter, `band` the Band select's text
 *   rt-table         rowHead the band, colHead the parameter, `receiver` the Receiver select's text
 */
export interface LabelledCell {
  table: string;
  rowHead: string;
  colHead: string;
  band: string;
  receiver: string;
  paths: string[];
}

/** What a cell's JSON path is of: receiver index, parameter, band (an index, `sum`, or null for a
 * receiver-wide parameter); null when it is the path of no cell. */
export function pathCell(path: string): { receiver: number; param: string; band: number | 'sum' | null } | null {
  const m = /^(?:spps|tcr)\.point_receivers\.(\d+)\.(.+)$/.exec(path);
  if (!m) return null;
  const r = Number(m[1]);
  const rest = m[2];
  let k: RegExpExecArray | null;
  if ((k = /^bands\.(\d+)\.parameters\.([a-z0-9_]+)\./.exec(rest))) return { receiver: r, param: k[2], band: Number(k[1]) };
  if ((k = /^bands\.(\d+)\.g_db\./.exec(rest))) return { receiver: r, param: 'g_db', band: Number(k[1]) };
  if ((k = /^aggregate\.parameters\.([a-z0-9_]+)\./.exec(rest))) return { receiver: r, param: k[1], band: 'sum' };
  if (/^aggregate\.dba\./.test(rest)) return { receiver: r, param: 'dba', band: null };
  if (/^sti\./.test(rest)) return { receiver: r, param: 'sti', band: null };
  return null;
}

/** A band's name as the tab writes it: `500 Hz`, `1 kHz`, `bands summed`. */
export function bandName(json: unknown, band: number | 'sum'): string {
  if (band === 'sum') return 'bands summed';
  const hz = (at(json, 'bands_hz') as number[] | undefined)?.[band];
  if (typeof hz !== 'number') return `no band ${band}`;
  return hz >= 1000 ? `${hz / 1000} kHz` : `${hz} Hz`;
}

const squash = (s: string) => s.replace(/\s+/g, ' ').trim();

/**
 * Why a cell's visible labels do not name the receiver, parameter and band its JSON paths are
 * of, or null when they do. Every path in the cell must be of one cell, and that cell's receiver
 * label (the JSON's), parameter name (`PARAM_LABELS`) and band name must be the ones the page
 * shows for it.
 */
export function cellLabelMismatch(c: LabelledCell, json: unknown): string | null {
  if (c.paths.length === 0) return `${c.table} "${c.rowHead}" x "${c.colHead}": no path in the cell`;
  const of = c.paths.map((p) => ({ p, at: pathCell(p) }));
  const bad = of.find((x) => x.at === null);
  if (bad) return `${c.table} "${c.rowHead}" x "${c.colHead}": ${bad.p} names no receiver cell`;
  const key = (x: NonNullable<ReturnType<typeof pathCell>>) => `${x.receiver}|${x.param}|${x.band}`;
  const first = of[0].at as NonNullable<ReturnType<typeof pathCell>>;
  const other = of.find((x) => key(x.at as NonNullable<ReturnType<typeof pathCell>>) !== key(first));
  if (other) return `${c.table} "${c.rowHead}" x "${c.colHead}": its paths disagree (${of[0].p} and ${other.p})`;
  const shownReceiver = squash(c.table === 'rt-table' ? c.receiver : c.rowHead);
  const shownBand = squash(c.table === 'rt-table' ? c.rowHead : c.band);
  const shownParam = squash(c.colHead);
  const label = at(json, `spps.point_receivers.${first.receiver}.label`) ?? at(json, `tcr.point_receivers.${first.receiver}.label`);
  const where = `${c.table} "${c.rowHead}" x "${c.colHead}" (${of[0].p})`;
  if (shownReceiver !== label) return `${where}: the receiver shown is "${shownReceiver}", the path's is "${String(label)}"`;
  if (shownParam !== PARAM_LABELS[first.param]) return `${where}: the parameter shown is "${shownParam}", the path's is "${PARAM_LABELS[first.param] ?? first.param}"`;
  if (first.band !== null && shownBand !== bandName(json, first.band)) return `${where}: the band shown is "${shownBand}", the path's is "${bandName(json, first.band)}"`;
  return null;
}
