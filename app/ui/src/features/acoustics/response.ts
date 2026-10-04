// The response window's model (Burhan 2026-10-04 11:45: "the resulting IR in the Dockyard
// black, white and red spectrogram colour theme, in a window that can be opened"). A pure
// module, tested by response.test.ts under `node --test`.
//
// What it is, honestly: SPPS counts energy, not pressure. A point receiver's `.recp` series is
// Pa² per time step in each octave band (`energy_pa2` in the report), an energy echogram, not a
// pressure impulse response; no waveform is synthesised from it. The window draws that series
// as a time x band map, each bin's level in dB re the map's maximum, clipped 60 dB down, in the
// Dockyard map: black, then deep red (#3a0b10, theme.css --red-deep), red (#e0202e, --red),
// white at the maximum.
//
// The rules it carries (the Acoustics tab's, model.ts):
// - **Every number is a path into the report (gate (a)).** Band labels are the band's own
//   `freq_hz`, time ticks are `spps.time_step_s` times a whole number of steps (`data-scale`),
//   the source's emission is `spps.sources.i.emission_s`. The colour bar's ends are the map's
//   own span, words not the report's (`SPAN_LABELS`, marked `[data-label="span"]`).
// - **The image is drawn from the same arrays the paths name**: `energy` is read at `paths`.
// - **The time axis ends where the energy does** (Burhan 2026-10-04: CR4's 10 s run is black
//   past about 3 s): `cropCols`, shortly after the last bin above the floor in any band; the
//   window's "Full run" shows every step, and the window says which is shown.
import type { Report } from '../../bindings/ipc';
import { at, num, type Num, type SourceSel, str, type Str } from './model.ts';

/** The window's title and its one line on what the map is. */
export const RESPONSE_TITLE = 'Energy response per band (SPPS echogram)';
export const RESPONSE_NOTE =
  'The energy SPPS counted at the receiver per time step in each octave band, Pa² per step: an energy echogram, not a pressure impulse response.';

/** The span of the map below its maximum, dB. */
export const SPAN_DB = 60;

/** The colour bar's labels: the span's ends and its thirds, dB re the map's maximum. */
export const SPAN_LABELS: readonly { db: number; text: string }[] = [
  { db: 0, text: '0 dB' },
  { db: -20, text: '−20' },
  { db: -40, text: '−40' },
  { db: -60, text: '−60 dB' },
];

export type Rgb = readonly [number, number, number];

/** The Dockyard map's stops, at a fraction of the span from its bottom (−60 dB) to its top. */
export const COLOUR_STOPS: readonly { t: number; rgb: Rgb }[] = [
  { t: 0, rgb: [0, 0, 0] },
  { t: 1 / 3, rgb: [0x3a, 0x0b, 0x10] },
  { t: 2 / 3, rgb: [0xe0, 0x20, 0x2e] },
  { t: 1, rgb: [0xff, 0xff, 0xff] },
];

/** The map's colour at `t` (0 the span's bottom, 1 its top), linear between the stops; `t` is
 * clamped to [0, 1]. Refuses NaN: a bin with no level has no colour. */
export function colourAt(t: number): Rgb {
  if (Number.isNaN(t)) throw new RangeError('colourAt: NaN');
  const x = Math.min(1, Math.max(0, t));
  for (let i = 1; i < COLOUR_STOPS.length; i++) {
    const a = COLOUR_STOPS[i - 1];
    const b = COLOUR_STOPS[i];
    if (x <= b.t) {
      const f = (x - a.t) / (b.t - a.t);
      return [0, 1, 2].map((k) => Math.round(a.rgb[k] + f * (b.rgb[k] - a.rgb[k]))) as unknown as Rgb;
    }
  }
  return COLOUR_STOPS[COLOUR_STOPS.length - 1].rgb;
}

/** The colour bar as a CSS gradient, bottom (the span's bottom) to top, through the same stops
 * at the same fractions: CSS interpolates sRGB linearly between stops, as `colourAt` does. */
export function colourBarCss(): string {
  const stops = COLOUR_STOPS.map((s) => `rgb(${s.rgb.join(', ')}) ${+(s.t * 100).toFixed(4)}%`);
  return `linear-gradient(to top, ${stops.join(', ')})`;
}

/** A level re the maximum, clipped `span` dB down: -Infinity (no energy) and anything lower read
 * as `-span`. Refuses NaN, a level above the maximum, and a span that is not above 0. */
export function clipDb(db: number, span = SPAN_DB): number {
  if (!(span > 0)) throw new RangeError(`clipDb: span ${span}`);
  if (Number.isNaN(db)) throw new RangeError('clipDb: NaN');
  if (db > 1e-9) throw new RangeError(`clipDb: ${db} dB is above the maximum`);
  return Math.max(Math.min(db, 0), -span);
}

/** Where a level lies in the span, 0 at `-span`, 1 at the maximum. */
export function levelT(db: number, span = SPAN_DB): number {
  return (clipDb(db, span) + span) / span;
}

export interface DbMatrix {
  /** Per row (band), per column (step): 10·log10(e / max), -Infinity where e is 0. */
  db: number[][];
  max: number;
  /** The first bin that holds the maximum. */
  at: { row: number; col: number };
}

/** Each bin's level in dB re the largest bin. Null (no map) when there is no row, the rows differ
 * in length, a value is negative or not finite, or every bin is 0. */
export function dbMatrix(rows: readonly (readonly number[])[]): DbMatrix | null {
  if (rows.length === 0 || rows[0].length === 0) return null;
  const cols = rows[0].length;
  let max = 0;
  let where = { row: -1, col: -1 };
  for (let r = 0; r < rows.length; r++) {
    if (rows[r].length !== cols) return null;
    for (let c = 0; c < cols; c++) {
      const e = rows[r][c];
      if (typeof e !== 'number' || !Number.isFinite(e) || e < 0) return null;
      if (e > max) {
        max = e;
        where = { row: r, col: c };
      }
    }
  }
  if (!(max > 0)) return null;
  return { db: rows.map((row) => row.map((e) => (e > 0 ? 10 * Math.log10(e / max) : -Infinity))), max, at: where };
}

/** The bands summed, step by step; null when the rows differ in length or there is none. */
export function broadband(rows: readonly (readonly number[])[]): number[] | null {
  if (rows.length === 0) return null;
  const cols = rows[0].length;
  if (rows.some((r) => r.length !== cols)) return null;
  return Array.from({ length: cols }, (_, c) => rows.reduce((s, r) => s + r[c], 0));
}

/** A series' level per step in dB re its maximum, clipped `span` down; null where `dbMatrix` is. */
export function decayDb(series: readonly number[], span = SPAN_DB): number[] | null {
  const m = dbMatrix([series]);
  return m ? m.db[0].map((d) => clipDb(d, span)) : null;
}

/** The map as RGBA pixels, one per bin: `cols` wide (the first `cols` steps; all by default), one
 * row per band, the highest band on top (image row 0 is the last band). Refuses a column count
 * outside 1 to the map's steps. */
export function mapPixels(m: DbMatrix, span = SPAN_DB, cols = m.db[0].length): Uint8ClampedArray {
  const rows = m.db.length;
  const all = m.db[0].length;
  if (!Number.isInteger(cols) || cols < 1 || cols > all) throw new RangeError(`mapPixels: ${cols} columns of ${all}`);
  const px = new Uint8ClampedArray(rows * cols * 4);
  for (let b = 0; b < rows; b++) {
    const y = rows - 1 - b;
    for (let c = 0; c < cols; c++) {
      const [r, g, bl] = colourAt(levelT(m.db[b][c], span));
      const i = (y * cols + c) * 4;
      px[i] = r;
      px[i + 1] = g;
      px[i + 2] = bl;
      px[i + 3] = 255;
    }
  }
  return px;
}

/** The 1-2-5 interval giving about `target` ticks over `total` (s), and the decimals it reads at. */
export function niceInterval(total: number, target = 6): { interval: number; digits: number } {
  const raw = total / target;
  const p = 10 ** Math.floor(Math.log10(raw));
  const interval = [1, 2, 5, 10].map((m) => m * p).find((v) => v >= raw * (1 - 1e-9)) ?? 10 * p;
  return { interval, digits: Math.max(0, -Math.floor(Math.log10(interval) + 1e-9)) };
}

/** Time ticks over `cols` steps of `stepS`: at whole multiples of a 1-2-5 interval giving about
 * `target` ticks, each at the whole step `j` nearest it (1 <= j <= cols) and shown at the
 * interval's decimals. Empty when `cols` or `stepS` is not above 0. */
export function timeTicks(stepS: number, cols: number, target = 6): { j: number; digits: number }[] {
  if (!(stepS > 0) || !(cols > 0) || !Number.isFinite(stepS)) return [];
  const { interval: nice, digits } = niceInterval(cols * stepS, target);
  const out: { j: number; digits: number }[] = [];
  for (let n = 1; Math.round((n * nice) / stepS) <= cols; n++) {
    const j = Math.round((n * nice) / stepS);
    if (j >= 1 && !out.some((t) => t.j === j)) out.push({ j, digits });
  }
  return out;
}

/** The margin past the last bin above the floor before the end is rounded up: 5 % of that time. */
export const CROP_MARGIN = 0.05;

/** The last step at which a bin of any band is above the floor (more than `span` dB down is not;
 * exactly `span` down is not). The maximum is 0 dB, so there is always one. */
export function lastAbove(m: DbMatrix, span = SPAN_DB): number {
  let last = -1;
  for (const row of m.db) {
    for (let c = row.length - 1; c > last; c--) {
      if (row[c] > -span) {
        last = c;
        break;
      }
    }
  }
  return last;
}

/**
 * Where the map ends when it is not the full run, in steps: the end of the last bin above the
 * floor in any band (`lastAbove` + 1 steps), plus `CROP_MARGIN`, rounded up to a whole multiple of
 * the 1-2-5 tick interval for that time (and of a step, when the interval is under one). Null,
 * the full run, when a band is still above the floor at the last bin, when that end reaches the
 * run's, or when there is no step.
 */
export function cropCols(m: DbMatrix, stepS: number, span = SPAN_DB): number | null {
  const cols = m.db[0].length;
  if (!(stepS > 0) || !Number.isFinite(stepS)) return null;
  const last = lastAbove(m, span);
  if (last >= cols - 1) return null;
  const t = (last + 1) * stepS * (1 + CROP_MARGIN);
  const iv = Math.max(niceInterval(t).interval, stepS);
  const n = Math.round((Math.ceil(t / iv - 1e-9) * iv) / stepS);
  return n >= cols ? null : n;
}

/** A band's label from its own `freq_hz` path: `125 Hz`, `1 kHz`, `1.25 kHz`. */
export function freqLabel(report: Report, path: string): { num: Num; unit: string } | null {
  const hz = at(report, path);
  if (typeof hz !== 'number' || !Number.isFinite(hz)) return null;
  if (hz < 1000) return { num: num(report, path, 0) as Num, unit: 'Hz' };
  const digits = hz % 1000 === 0 ? 0 : hz % 100 === 0 ? 1 : 2;
  return { num: num(report, path, digits, 0.001) as Num, unit: 'kHz' };
}

export interface ResponseView {
  receiver: Str;
  /** The source shown (its name in `spps.sources`), or null for the sources summed. */
  source: Str | null;
  /** The source whose emission the time axis starts at, and that time; for the sources summed,
   * the first to emit. Null when the report has no source. */
  emission: { source: Str; at: Num } | null;
  /** The step the map starts at: the emission's, `emission_s / time_step_s`. */
  k0: number;
  /** Per band, lowest first: its label and the path of the series drawn. */
  bands: { label: { num: Num; unit: string }; path: string }[];
  /** Per band, the series from `k0` on, read at `bands[b].path`. */
  energy: number[][];
  map: DbMatrix;
  /** The bands summed, dB re its own maximum, clipped `SPAN_DB` down. */
  broadband: number[] | null;
  /** The full run's ticks: each one's column (steps after `k0`) and its time, `spps.time_step_s`
   * times that. */
  ticks: Tick[];
  /** The run's length from `k0`: `spps.time_step_s` times its steps. */
  run: Num;
  /** The time from which every band is at the floor or lower (`lastAbove` + 1 steps), at the
   * step's decimals; null when a band is still above it at the run's end. */
  floor: Num | null;
  /** The map cut short (`cropCols`): its steps, its end and its own ticks; null for the full run. */
  crop: { cols: number; end: Num; ticks: Tick[] } | null;
}

export interface Tick {
  col: number;
  num: Num;
}

/**
 * The response of SPPS point receiver `r` to source `src` (null: the sources summed), as the
 * window draws and prints it; null for a TCR run, a receiver or source the report has no
 * echogram for, or an echogram with no energy.
 */
export function responseView(report: Report, r: number, src: SourceSel): ResponseView | null {
  if (report.solver === 'tcr') return null;
  const rx = `spps.point_receivers.${r}`;
  const receiver = str(report, `${rx}.label`);
  if (!receiver) return null;
  let base = rx;
  let source: Str | null = null;
  const srcList = (at(report, 'spps.sources') as { name: string; emission_s: number }[] | undefined) ?? [];
  if (src !== null) {
    const list = at(report, `${rx}.per_source`);
    const k = Array.isArray(list) ? list.findIndex((p) => (p as { source?: string }).source === src) : -1;
    if (k < 0) return null;
    base = `${rx}.per_source.${k}`;
    const si = srcList.findIndex((s) => s.name === src);
    source = si < 0 ? null : str(report, `spps.sources.${si}.name`);
    if (!source) return null;
  }
  const step = at(report, 'spps.time_step_s');
  if (typeof step !== 'number' || !(step > 0)) return null;
  // The emission the time axis starts at: the source shown, or the first to emit.
  let ei = src === null ? -1 : srcList.findIndex((s) => s.name === src);
  if (src === null) srcList.forEach((s, i) => (ei < 0 || s.emission_s < srcList[ei].emission_s ? (ei = i) : null));
  const emission =
    ei < 0 ? null : { source: str(report, `spps.sources.${ei}.name`) as Str, at: num(report, `spps.sources.${ei}.emission_s`, 3) as Num };
  const k0 = ei < 0 ? 0 : Math.round(srcList[ei].emission_s / step);
  const nb = (at(report, `${base}.bands`) as unknown[] | undefined)?.length ?? 0;
  const bands: ResponseView['bands'] = [];
  const energy: number[][] = [];
  for (let b = 0; b < nb; b++) {
    const label = freqLabel(report, `${base}.bands.${b}.freq_hz`);
    const path = `${base}.bands.${b}.energy_pa2`;
    const e = at(report, path);
    if (!label || !Array.isArray(e)) return null;
    bands.push({ label, path });
    energy.push((e as number[]).slice(k0));
  }
  const map = dbMatrix(energy);
  if (!map) return null;
  const sum = broadband(energy);
  const cols = map.db[0].length;
  const stepNum = (digits: number, k: number) => num(report, 'spps.time_step_s', digits, k) as Num;
  const ticksOver = (n: number): Tick[] => timeTicks(step, n).map((t) => ({ col: t.j, num: stepNum(t.digits, t.j) }));
  const runDigits = niceInterval(cols * step).digits;
  const n = cropCols(map, step);
  const cropDigits = n === null ? runDigits : niceInterval(n * step).digits;
  const last = lastAbove(map);
  // The floor's time to the step's own decimals (0.001 s: 3; the f32 widening's last digits
  // ignored): a whole number of steps, not rounded to a tick.
  const stepDigits = Math.max(0, Math.ceil(-Math.log10(step) - 1e-6));
  return {
    receiver,
    source,
    emission,
    k0,
    bands,
    energy,
    map,
    broadband: sum ? decayDb(sum) : null,
    ticks: ticksOver(cols),
    run: stepNum(runDigits, cols),
    floor: last >= cols - 1 ? null : stepNum(stepDigits, last + 1),
    crop: n === null ? null : { cols: n, end: stepNum(cropDigits, n), ticks: ticksOver(n) },
  };
}
