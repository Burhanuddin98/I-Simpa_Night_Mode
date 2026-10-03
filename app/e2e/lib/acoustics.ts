// M12 P2's checks on the Acoustics tab (docs/investigations/2026-10-03-m12/PLAN.md, gate (a), (b),
// (e), (f)), as pure functions over what the page shows and what `simpa results <run> --json`
// prints. The tab's DOM contract (app/ui/src/features/acoustics/):
//   [data-num]   a number: its text is the value at `data-digits` decimals, of the report's JSON
//                at `data-json` (a dot path), times `data-scale` when present (1 kHz: 0.001)
//   [data-str]   a string: its text is the report's JSON at `data-json`, exactly
//   [data-label] a word that is not the report's: `wording` (MQ2), `param` (a parameter's
//                name, `data-param` says which), `group` (a surface group's name, from the open
//                project), `unit`
//   [data-param] every element of one parameter (gate (b)): a column head, a cell, a series
// Every digit the tab shows is inside one of these, or in a `<select>` (whose options are checked
// on their own) or the run label (`[data-run-label]`); `strayDigits` says where one is not.
// `acoustics.test.ts` holds each rule to hand-made cases, its say-NO among them.

/** The words on the Results screen (MQ2, Burhan 2026-10-03 08:31). */
export const MQ2_WORDING =
  'Computed to ISO 3382-1 / IEC 60268-16 and checked against exact solutions to within the just-noticeable difference. Simulated with I-Simpa’s solvers; not compared with measured rooms.';

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

/**
 * A receiver's RT series as the JSON gives it, the paths the tab must have drawn from: per band of
 * `bands_hz`, the parameter's value, or null where it is refused.
 */
export function rtSeries(json: unknown, receiver: number, param: string): { paths: string[]; values: (number | null)[] } {
  const bands = (at(json, 'bands_hz') as number[] | undefined) ?? [];
  const paths = bands.map((_, b) => `spps.point_receivers.${receiver}.bands.${b}.parameters.${param}.value`);
  return { paths, values: paths.map((p) => (typeof at(json, p) === 'number' ? (at(json, p) as number) : null)) };
}

/** Mismatches between a drawn series and the JSON's: its paths and its values, exactly. */
export function seriesMismatches(drawn: { paths: string[]; values: (number | null)[] }, json: unknown, receiver: number, param: string): string[] {
  const want = rtSeries(json, receiver, param);
  const out: string[] = [];
  if (drawn.values.length !== want.values.length) out.push(`${param}: ${drawn.values.length} points drawn, the JSON has ${want.values.length} bands`);
  want.values.forEach((v, i) => {
    if (drawn.paths[i] !== want.paths[i]) out.push(`${param}[${i}]: drawn from ${drawn.paths[i]}, not ${want.paths[i]}`);
    if (drawn.values[i] !== v) out.push(`${param}[${i}]: drawn ${drawn.values[i]}, JSON ${v}`);
  });
  return out;
}
