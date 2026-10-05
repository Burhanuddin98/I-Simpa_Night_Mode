// The Acoustics tab's model (M12 P2, docs/investigations/2026-10-03-m12/PLAN.md): what the tab
// shows of a run's report, as plain data the component maps one to one onto elements. A pure
// module, tested by model.test.ts under `node --test`.
//
// The rules it carries:
// - **Only a PASS parameter is shown (gate (b)).** Whether a parameter passed is read from the
//   report core built (`report.bed.parameters[name].status`, from `beds/summary.json`), never
//   from a list here: `PARAM_SPECS` says only how a parameter is shown, `shownParams` which are.
// - **Every number is a path into the report (gate (a)).** A `Num` carries the dot path of the
//   value it shows, the decimals it shows and a scale, and its text is `toFixed` of that value;
//   the page marks it `[data-num][data-json][data-digits]`, so the gate reads the same value
//   from `simpa results --json` by the same path.
// - **A value is never shown without its range and status, or its refusal** (decision-log row
//   37 (3)); STI, which has no range yet, carries "noise range not computed" (MQ3); EDT carries
//   row 37's two marks wherever it appears, and a per-value mark where `edt_validated` is false.
// - **No screen text says "validated"** (MQ2, decision 39): `MQ2_WORDING` is the tab's words.
import type { Advice, ApplyConflict, Report, Setting } from '../../bindings/ipc';

/** The words on the Results screen (MQ2, Burhan 2026-10-03 08:31). */
export const MQ2_WORDING =
  'Computed to ISO 3382-1 / IEC 60268-16 and checked against exact solutions to within the just-noticeable difference. Simulated with I-Simpa’s solvers; not compared with measured rooms.';

/** Row 37 (1)-(2)'s marks, shown with EDT wherever it appears (row 37 (5)), in words that do not
 * say "validated". */
export const EDT_MARKS = [
  'EDT unchecked for receivers larger than one metre in radius',
  'EDT unchecked in energetic mode',
] as const;

/** MQ3: STI's Monte-Carlo noise is not modelled (backlog 71). */
export const STI_NOTE = 'noise range not computed';

/** Decision 56's words after the band's lost share (a number of the report, `Cell.lost`), beside a
 * value whose band (or, for STI and dB(A), one of whose bands) lost from 0.3 % of its particles. */
export const LOST_WARNING = 'of particles lost: the late decay may hold too little energy';

/** Decision 56's words after the share (`Refusal.lost`), beside a refusal `lost_particles`: from
 * 1 % lost, where the model is broken. */
export const LOST_REFUSED = 'of particles lost (holes or a bad mesh?)';

/** Decision-log row 46's mark, shown with T30 wherever it appears: T30 is shown by Burhan's
 * decision although one of 833 checked noise-limited values fell outside its range (backlog 65).
 * Its digits are words, not the report's numbers: the page marks it `[data-label="mark"]`. */
export const T30_MARK =
  'Ranges on noise-limited T30 values may be slightly narrow: 1 of 833 checked values fell 1.5 ms outside its range.';

export interface ParamSpec {
  /** The parameter's name in `beds/summary.json` and `report.bed`. */
  name: string;
  /** On screen. */
  label: string;
  unit: string;
  /** Decimals shown, at about a tenth of the parameter's difference limen. */
  digits: number;
  /** `band`: per receiver and band (and the bands summed); `receiver`: one per receiver. */
  scope: 'band' | 'receiver';
  /** A reverberation time: drawn per band against the DIN target. */
  rt?: true;
}

/** How each parameter is shown, in the tab's column order. Whether it is shown is `shownParams`. */
export const PARAM_SPECS: readonly ParamSpec[] = [
  { name: 'spl_db', label: 'SPL', unit: 'dB', digits: 1, scope: 'band' },
  { name: 'g_db', label: 'G', unit: 'dB', digits: 1, scope: 'band' },
  { name: 'edt_s', label: 'EDT', unit: 's', digits: 2, scope: 'band', rt: true },
  { name: 't20_s', label: 'T20', unit: 's', digits: 2, scope: 'band', rt: true },
  { name: 't30_s', label: 'T30', unit: 's', digits: 2, scope: 'band', rt: true },
  { name: 'c50_db', label: 'C50', unit: 'dB', digits: 1, scope: 'band' },
  { name: 'c80_db', label: 'C80', unit: 'dB', digits: 1, scope: 'band' },
  { name: 'd50', label: 'D50', unit: '', digits: 2, scope: 'band' },
  { name: 'ts_s', label: 'Ts', unit: 's', digits: 3, scope: 'band' },
  { name: 'sti', label: 'STI', unit: '', digits: 2, scope: 'receiver' },
  { name: 'dba', label: 'dB(A)', unit: 'dB', digits: 1, scope: 'receiver' },
];

/** The parameters the report's bed status lets the tab show, in column order. */
export function shownParams(report: Report): ParamSpec[] {
  const bed = report.bed?.parameters as unknown as Record<string, { status?: string } | undefined> | undefined;
  return PARAM_SPECS.filter((p) => bed?.[p.name]?.status === 'PASS');
}

/** The marks shown beside the receivers table, each with its parameter and only while that
 * parameter is shown: EDT's two (row 37 (1)-(2)), T30's (row 46), STI's note (MQ3). */
export function paramMarks(report: Report): { param: string; text: string }[] {
  const shown = new Set(shownParams(report).map((p) => p.name));
  const out: { param: string; text: string }[] = [];
  if (shown.has('edt_s')) for (const m of EDT_MARKS) out.push({ param: 'edt_s', text: m });
  if (shown.has('t30_s')) out.push({ param: 't30_s', text: T30_MARK });
  if (shown.has('sti')) out.push({ param: 'sti', text: `STI: ${STI_NOTE}` });
  return out;
}

/** The value at a dot path of the report (`spps.point_receivers.0.label`). */
export function at(root: unknown, path: string): unknown {
  let v: unknown = root;
  for (const k of path.split('.')) {
    if (v === null || typeof v !== 'object') return undefined;
    v = Array.isArray(v) ? v[Number(k)] : (v as Record<string, unknown>)[k];
  }
  return v;
}

/** A shown number: `text` is `toFixed(digits)` of the report's value at `path`, times `scale`. */
export interface Num {
  path: string;
  digits: number;
  scale?: number;
  text: string;
}

/** A shown string: the report's string at `path`. */
export interface Str {
  path: string;
  text: string;
}

/** The number at `path`, or null when the report holds none there. */
export function num(report: unknown, path: string, digits: number, scale?: number): Num | null {
  const v = at(report, path);
  if (typeof v !== 'number' || !Number.isFinite(v)) return null;
  return { path, digits, ...(scale === undefined ? {} : { scale }), text: (scale === undefined ? v : v * scale).toFixed(digits) };
}

/** The string at `path`, or null. */
export function str(report: unknown, path: string): Str | null {
  const v = at(report, path);
  return typeof v === 'string' ? { path, text: v } : null;
}

/** A band's label: `125 Hz`, `1 kHz`, `1.25 kHz`; as a `Num` of `bands_hz` with its unit. */
export function bandNum(report: Report, index: number): { num: Num; unit: string } | null {
  const hz = report.bands_hz[index];
  if (hz === undefined) return null;
  if (hz < 1000) return { num: num(report, `bands_hz.${index}`, 0) as Num, unit: 'Hz' };
  const digits = hz % 1000 === 0 ? 0 : hz % 100 === 0 ? 1 : 2;
  return { num: num(report, `bands_hz.${index}`, digits, 0.001) as Num, unit: 'kHz' };
}

/** A band's label as plain text (the `<select>` options): `${hz} Hz` or `${hz / 1000} kHz`. */
export function bandText(hz: number): string {
  return hz >= 1000 ? `${hz / 1000} kHz` : `${hz} Hz`;
}

/** Which band a table shows: an index into `bands_hz`, or the bands summed. */
export type BandSel = number | 'sum';

export interface Refusal {
  code: Str;
  /** The refusal's kind (`range_not_reached`), when the report names one. */
  why: Str | null;
  /** `lost_particles` only: the band's lost share, %, shown before `LOST_REFUSED`. */
  lost?: Num;
}

/** One parameter of one receiver in one band (or receiver-wide), as shown. */
export interface Cell {
  param: string;
  receiver: string;
  /** `bands_hz` value, or `sum`, or `all` for a receiver-wide parameter. */
  band: string;
  /** `ok`/`wide` with a range, `value` (STI: no range), `refused`. */
  status: 'ok' | 'wide' | 'value' | 'refused';
  value: Num | null;
  lo: Num | null;
  hi: Num | null;
  refusal: Refusal | null;
  /** STI's MQ3 note, or why an EDT is unchecked. */
  note: string | null;
  /** Decision 56: the band's lost share, %, when it is a warning (`lost_share_warning`, from 0.3 %
   * lost), shown before `LOST_WARNING`. */
  lost?: Num;
  /** Backlog 80: the code of the advice item (`report.advice`) that explains this value, when it
   * is refused, wide or warned; the page marks the cell `[data-advice]`. */
  advice?: string;
}

function solverKey(report: Report): 'spps' | 'tcr' {
  return report.solver === 'tcr' ? 'tcr' : 'spps';
}

/** The report's point receivers' labels, in order. */
export function receivers(report: Report): string[] {
  const s = (report as unknown as Record<string, { point_receivers?: { label: string }[] } | null>)[solverKey(report)];
  return (s?.point_receivers ?? []).map((r) => r.label);
}

/** Which source's echogram a value is read from: a source's name, or null for the sources summed
 * (the receiver's own `bands` and `aggregate`). */
export type SourceSel = string | null;

/** The sources with their own echogram at the receivers (`per_source`, written when the run's
 * echogram per source is on), in `order` (the open project's sources, by name) and then in the
 * first such receiver's (`config.xml`'s, which lists them last first); empty with fewer than two,
 * where the summed echogram is the one source's. Backlog 77 (parity R11). */
export function sources(report: Report, order: readonly string[] = []): string[] {
  const s = (report as unknown as Record<string, { point_receivers?: { per_source?: { source: string }[] }[] } | null>)[solverKey(report)];
  const first = (s?.point_receivers ?? []).find((rx) => (rx.per_source ?? []).length > 0);
  const names = (first?.per_source ?? []).map((p) => p.source);
  const rank = (n: string) => (order.includes(n) ? order.indexOf(n) : order.length + names.indexOf(n));
  return names.length >= 2 ? [...names].sort((a, b) => rank(a) - rank(b)) : [];
}

/** Several sources summed, with no echogram per source: the onset-relative parameters are refused
 * `several_sources` (ISO 3382-1 defines them per source and receiver), and this says how to get
 * them; null otherwise. */
export function sourceNote(report: Report): string | null {
  if (sources(report).length) return null;
  const rxs = at(report, `${solverKey(report)}.point_receivers`);
  if (!Array.isArray(rxs)) return null;
  const several = rxs.some((_, r) =>
    report.bands_hz.some((_, b) => PARAM_SPECS.some((p) => at(report, `${solverKey(report)}.point_receivers.${r}.bands.${b}.parameters.${p.name}.not_evaluable.error.why.why`) === 'several_sources')),
  );
  return several
    ? 'This run has several sources and no echogram per source. EDT, T20, T30, C50, C80, D50 and Ts are defined per source and receiver: turn on "Echogram per source" in the simulation settings and run again.'
    : null;
}

/** Where receiver `r`'s echogram for `src` is: the receiver itself for the sources summed, its
 * `per_source` entry of that name otherwise; null when it has none. */
function echogramPath(report: Report, r: number, src: SourceSel): string | null {
  const rx = `${solverKey(report)}.point_receivers.${r}`;
  if (src === null) return rx;
  const list = at(report, `${rx}.per_source`);
  const k = Array.isArray(list) ? list.findIndex((p) => (p as { source?: string }).source === src) : -1;
  return k < 0 ? null : `${rx}.per_source.${k}`;
}

/** Source `src`'s name as receiver `r`'s echogram carries it (a path, for gate (a)); null for the
 * sources summed or a source the receiver has none for. */
export function sourceLabel(report: Report, r: number, src: SourceSel): Str | null {
  if (src === null) return null;
  const p = echogramPath(report, r, src);
  return p === null ? null : str(report, `${p}.source`);
}

/** Where a parameter's `Evaluated` is for receiver `r` in band `band`, from source `src`'s
 * echogram (null: the sources summed); null where the report has no place for it (G of the bands
 * summed, STI of one source, a source the receiver has no echogram for). */
export function paramPath(report: Report, spec: ParamSpec, r: number, band: BandSel, src: SourceSel = null): string | null {
  const rx = echogramPath(report, r, src);
  if (rx === null) return null;
  if (spec.name === 'sti') {
    if (src !== null) return null;
    const shown = at(report, `${rx}.sti.shown`);
    return typeof shown === 'string' ? `${rx}.sti.${shown}` : null;
  }
  if (spec.name === 'dba') return `${rx}.aggregate.dba.level_db`;
  const base = band === 'sum' ? `${rx}.aggregate` : `${rx}.bands.${band}`;
  if (spec.name === 'g_db') return band === 'sum' ? null : `${base}.g_db`;
  return `${base}.parameters.${spec.name}`;
}

/** Why an EDT is unchecked, in this tab's words; null when `edt_validated` is true. */
function edtNote(report: Report, path: string): string | null {
  const params = path.slice(0, path.lastIndexOf('.'));
  if (at(report, `${params}.edt_validated`) === true) return null;
  const note = String(at(report, `${params}.edt.validation_note`) ?? '');
  const why: string[] = [];
  if (/broadband/.test(note)) why.push('the bands summed');
  if (/no direct path/.test(note)) why.push('no direct path from the source');
  if (/receivers over/.test(note)) why.push('receiver larger than one metre in radius');
  return `unchecked: ${why.length ? why.join('; ') : 'outside the tested cases'}`;
}

/** The code of the first advice item that explains the value at `path`, or undefined. */
export function adviceAt(report: Report, path: string): string | undefined {
  const list = (report as unknown as { advice?: { code: string; values: string[] }[] }).advice;
  return list?.find((a) => a.values.includes(path))?.code;
}

/** One cell. */
export function cell(report: Report, spec: ParamSpec, r: number, band: BandSel, src: SourceSel = null): Cell | null {
  const c = cellAt(report, spec, r, band, src);
  if (c === null) return null;
  const path = paramPath(report, spec, r, band, src) as string;
  const advice = adviceAt(report, path);
  return advice === undefined ? c : { ...c, advice };
}

function cellAt(report: Report, spec: ParamSpec, r: number, band: BandSel, src: SourceSel): Cell | null {
  const path = paramPath(report, spec, r, band, src);
  if (path === null) return null;
  const e = at(report, path);
  if (e === null || typeof e !== 'object') return null;
  const receiver = receivers(report)[r] ?? '';
  const bandKey = spec.scope === 'receiver' ? 'all' : band === 'sum' ? 'sum' : String(report.bands_hz[band]);
  const base = { param: spec.name, receiver, band: bandKey, lo: null, hi: null, refusal: null, note: null };
  if ('not_evaluable' in e) {
    const why = str(report, `${path}.not_evaluable.error.why.why`) ?? str(report, `${path}.not_evaluable.error.kind`);
    // Decision 56: the share a `lost_particles` refusal names, as a percentage of the report's fraction.
    const lost = why?.text === 'lost_particles' ? num(report, `${path}.not_evaluable.error.why.share`, 2, 100) : null;
    return { ...base, status: 'refused', value: null, refusal: { code: str(report, `${path}.not_evaluable.code`) as Str, why, ...(lost ? { lost } : {}) } };
  }
  const value = num(report, `${path}.value`, spec.digits);
  if (!value) return null;
  if (spec.name === 'sti') {
    const lost = num(report, `${path}.lost_share_warning`, 2, 100);
    return { ...base, status: 'value', value, note: STI_NOTE, ...(lost ? { lost } : {}) };
  }
  const status = at(report, `${path}.status`);
  const lo = num(report, `${path}.lo`, spec.digits);
  const hi = num(report, `${path}.hi`, spec.digits);
  // A value with no range is not one this tab may show alone (row 37 (3)): refused as such.
  if ((status !== 'ok' && status !== 'wide') || !lo || !hi) return null;
  // Decision 56: the band lost from 0.3 % of its particles.
  const lost = num(report, `${path}.lost_share_warning`, 2, 100);
  return { ...base, status, value, lo, hi, note: spec.name === 'edt_s' ? edtNote(report, path) : null, ...(lost ? { lost } : {}) };
}

/** The receivers table: one row per receiver, one cell per shown parameter (null: no place). */
export function receiverRows(report: Report, band: BandSel, src: SourceSel = null): { receiver: Str; cells: (Cell | null)[] }[] {
  const specs = shownParams(report);
  return receivers(report).map((_, r) => ({
    receiver: str(report, `${solverKey(report)}.point_receivers.${r}.label`) as Str,
    cells: specs.map((s) => cell(report, s, r, band, src)),
  }));
}

/** An RT series of one receiver: per band its value and range (null where the table shows no
 * value: refused, or not one `cell` lets through) and the paths read. */
export interface Series {
  param: string;
  label: string;
  paths: string[];
  values: (number | null)[];
  loPaths: string[];
  lo: (number | null)[];
  hiPaths: string[];
  hi: (number | null)[];
}

/** The shown reverberation times of receiver `r`, per band: what the chart draws and the RT
 * table prints, from one read. Each point is the band's `cell` (the tables' filter: PASS
 * parameters only, a value only with its range and an `ok`/`wide` status), so the chart draws
 * no value the table does not show, and draws the range the table shows beside it. */
export function rtSeries(report: Report, r: number, src: SourceSel = null): Series[] {
  return shownParams(report)
    .filter((s) => s.rt)
    .map((s) => {
      const base = report.bands_hz.map((_, b) => paramPath(report, s, r, b, src));
      const cells = report.bands_hz.map((_, b) => cell(report, s, r, b, src));
      const shown = (c: Cell | null): c is Cell & { value: Num; lo: Num; hi: Num } => c !== null && c.status !== 'refused' && !!c.value && !!c.lo && !!c.hi;
      const read = (n: Num | null) => (n ? (at(report, n.path) as number) : null);
      return {
        param: s.name,
        label: s.label,
        paths: base.map((p) => `${p}.value`),
        values: cells.map((c) => (shown(c) ? read(c.value) : null)),
        loPaths: base.map((p) => `${p}.lo`),
        lo: cells.map((c) => (shown(c) ? read(c.lo) : null)),
        hiPaths: base.map((p) => `${p}.hi`),
        hi: cells.map((c) => (shown(c) ? read(c.hi) : null)),
      };
    });
}

export interface DinView {
  group: Str;
  use: Str;
  target: Num | null;
  refusal: Refusal | null;
  volume: Num | null;
  note: Str | null;
}

/** DIN 18041's target for `group` (A1 to A5) at the room's volume, from `report.room`. */
export function din(report: Report, group: string): DinView | null {
  const list = at(report, 'room.din18041');
  if (!Array.isArray(list)) return null;
  const i = list.findIndex((d) => (d as { group?: string }).group === group);
  if (i < 0) return null;
  const p = `room.din18041.${i}`;
  const refused = at(report, `${p}.target_s.not_evaluable`) !== undefined;
  return {
    group: str(report, `${p}.group`) as Str,
    use: str(report, `${p}.use`) as Str,
    target: num(report, `${p}.target_s.value`, 2),
    refusal: refused ? { code: str(report, `${p}.target_s.not_evaluable.code`) as Str, why: null } : null,
    volume: num(report, 'room.volume_m3', 0),
    note: str(report, 'room.din18041_note'),
  };
}

/** The target's value, for the chart (the line drawn is the number shown). */
export function dinTarget(report: Report, group: string): number | null {
  const d = din(report, group);
  return d?.target ? (at(report, d.target.path) as number) : null;
}

/** The DIN groups the report carries, in order. */
export function dinGroups(report: Report): string[] {
  const list = at(report, 'room.din18041');
  return Array.isArray(list) ? list.map((d) => String((d as { group?: string }).group)) : [];
}

export interface AbsorptionRow {
  materialId: Num;
  /** The surface groups the open project gives this material id (names, not numbers). */
  names: string[];
  area: Num;
  /** Per band, `S·α`, m². */
  bands: (Num | null)[];
}

/** The absorption by surface group and each band's total, from `report.room`. */
export function absorption(report: Report, names: ReadonlyMap<number, string[]>): { rows: AbsorptionRow[]; totals: (Num | null)[] } | null {
  const surfaces = at(report, 'room.surfaces');
  if (!Array.isArray(surfaces)) return null;
  const rows = surfaces.map((s, i) => ({
    materialId: num(report, `room.surfaces.${i}.material_id`, 0) as Num,
    names: names.get((s as { material_id: number }).material_id) ?? [],
    area: num(report, `room.surfaces.${i}.area_m2`, 1) as Num,
    bands: report.bands_hz.map((_, b) => num(report, `room.surfaces.${i}.bands.${b}.absorption_area_m2`, 2)),
  }));
  return { rows, totals: report.bands_hz.map((_, b) => num(report, `room.bands.${b}.absorption_area_m2`, 2)) };
}

export interface ClassicalRow {
  band: number;
  cells: { key: string; label: string; unit: string; value: Num | null; refusal: Refusal | null }[];
}

/** The Sabine/Eyring table: an SPPS run's reference (Sabine, Eyring, and Kuttruff where it
 * applies), or a TCR run's own main results (absorption area, time and level per theory). */
export function classical(report: Report): ClassicalRow[] {
  const ev = (path: string, label: string, unit: string, digits: number) => {
    const n = num(report, `${path}.value`, digits);
    const refused = at(report, `${path}.not_evaluable`) !== undefined;
    return {
      key: path.slice(path.lastIndexOf('.') + 1),
      label,
      unit,
      value: n,
      refusal: refused ? { code: str(report, `${path}.not_evaluable.code`) as Str, why: null } : null,
    };
  };
  const plain = (path: string, key: string, label: string, unit: string, digits: number) => ({ key, label, unit, value: num(report, path, digits), refusal: null });
  if (report.solver === 'tcr') {
    return report.bands_hz.map((_, b) => ({
      band: b,
      cells: [
        plain(`tcr.bands.${b}.sabine.absorption_area_m2`, 'sabine_a', 'Sabine A', 'm²', 1),
        plain(`tcr.bands.${b}.sabine.reverberation_time_s`, 'sabine_t', 'Sabine', 's', 2),
        plain(`tcr.bands.${b}.sabine.level_db`, 'sabine_l', 'Sabine L', 'dB', 1),
        plain(`tcr.bands.${b}.eyring.absorption_area_m2`, 'eyring_a', 'Eyring A', 'm²', 1),
        plain(`tcr.bands.${b}.eyring.reverberation_time_s`, 'eyring_t', 'Eyring', 's', 2),
        plain(`tcr.bands.${b}.eyring.level_db`, 'eyring_l', 'Eyring L', 'dB', 1),
      ],
    }));
  }
  if (at(report, 'spps.reference.status') !== 'computed') return [];
  const rows = report.bands_hz.map((_, b) => ({
    band: b,
    cells: [
      ev(`spps.reference.bands.${b}.sabine_s`, 'Sabine', 's', 2),
      ev(`spps.reference.bands.${b}.eyring_s`, 'Eyring', 's', 2),
      ev(`spps.reference.bands.${b}.kuttruff_s`, 'Kuttruff', 's', 2),
    ],
  }));
  // Kuttruff's time applies only where every wall is Lambert with scattering 1: a column refused
  // in every band says nothing a row needs to repeat, so it is left out.
  const kept = [0, 1, 2].filter((k) => rows.some((r) => r.cells[k].value !== null));
  return rows.map((r) => ({ band: r.band, cells: kept.map((k) => r.cells[k]) }));
}

/** A receiver's decay curve in one band (or the bands summed), from source `src`'s echogram (null:
 * the sources summed): the report's points, as drawn. */
export function decay(report: Report, r: number, band: BandSel, src: SourceSel = null): { path: string; t: number[]; db: number[] } | null {
  const rx = echogramPath(report, r, src);
  if (rx === null) return null;
  const path = band === 'sum' ? `${rx}.aggregate.decay_curve` : `${rx}.bands.${band}.decay_curve`;
  const pts = at(report, `${path}.points`);
  if (!Array.isArray(pts)) return null;
  return { path, t: pts.map((p) => (p as number[])[0]), db: pts.map((p) => (p as number[])[1]) };
}

/** The run to show after the active variant changes: the newest OK run of that variant, or
 * null to keep the current one. */
export function runForVariant(rows: readonly { run: string; status: string; variant?: string | null }[], variant: string | null): string | null {
  for (let i = rows.length - 1; i >= 0; i--) {
    if ((rows[i].variant ?? null) === variant && rows[i].status === 'OK') return rows[i].run;
  }
  return null;
}

// ---- the run-quality advisor (backlog 80) -------------------------------------------------------------

/** How a setting's value is shown: its unit, decimals and scale (a time step in ms). */
const SETTING_NUM: Record<Setting, { unit: string; digits: number; scale?: number }> = {
  receiver_radius: { unit: 'm', digits: 2 },
  particles_per_source: { unit: '', digits: 0 },
  duration: { unit: 's', digits: 3 },
  time_step: { unit: 'ms', digits: 0, scale: 1000 },
  extinction_exponent: { unit: '', digits: 0 },
  preserve_boundary: { unit: '', digits: 0 },
  echogram_per_source: { unit: '', digits: 0 },
};

/** One advice item as the "Why values are missing" card shows it: each word and number a path
 * into `report.advice` (gate (a)). */
export interface AdviceCard {
  index: number;
  code: Str;
  cause: Str;
  words: Str;
  label: Str | null;
  /** A number setting's value now and the value Apply sets, as `Num`s of the report; null for an
   * on/off setting, which `fromWord`/`toWord` say. */
  from: Num | null;
  to: Num | null;
  fromWord: string | null;
  toWord: string | null;
  unit: string;
  why: Str | null;
  note: Str | null;
  /** What "Apply and re-run" sends; null when the core offers no Apply. */
  apply: { setting: Setting; from: number | boolean; to: number | boolean } | null;
  values: string[];
}

/** Why the open project would refuse `apply` (`SceneState.advice_conflicts`, in the advisor's own
 * words), or null when it would take it: the card then offers no Apply, whatever the run's own
 * meshing allowed (Q3). */
export function applyConflict(apply: { setting: Setting; to: number | boolean }, conflicts: readonly Pick<ApplyConflict, 'setting' | 'to' | 'why'>[]): string | null {
  return conflicts.find((c) => c.setting === apply.setting && c.to === apply.to)?.why ?? null;
}

/** The report's advice (results version 17), one card per item, in the core's order; none in a
 * report without it. */
export function adviceCards(report: Report): AdviceCard[] {
  const list = (report as unknown as { advice?: Advice[] }).advice;
  if (!Array.isArray(list)) return [];
  return list.map((a, i) => {
    const p = `advice.${i}`;
    const f = a.fix;
    const setting = f.setting ?? null;
    const spec = setting ? SETTING_NUM[setting] : { unit: '', digits: 0 };
    const word = (v: unknown) => (typeof v === 'boolean' ? (v ? 'on' : 'off') : null);
    const from = f.from ?? null;
    const to = f.to ?? null;
    return {
      index: i,
      code: str(report, `${p}.code`) as Str,
      cause: str(report, `${p}.cause`) as Str,
      words: str(report, `${p}.fix.words`) as Str,
      label: str(report, `${p}.fix.label`),
      from: num(report, `${p}.fix.from`, spec.digits, spec.scale),
      to: num(report, `${p}.fix.to`, spec.digits, spec.scale),
      fromWord: word(from),
      toWord: word(to),
      unit: spec.unit,
      why: str(report, `${p}.fix.why_no_apply`),
      note: str(report, `${p}.fix.note`),
      apply: setting !== null && from !== null && to !== null ? { setting, from, to } : null,
      values: a.values,
    };
  });
}
