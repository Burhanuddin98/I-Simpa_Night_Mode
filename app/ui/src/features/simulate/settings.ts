// The Simulate settings editor's pure logic (docs/investigations/2026-10-03-pq3/PLAN.md, PQ3):
// what a field accepts, the step count and the particle file's size beside the fields, the band
// presets, and the settings an edit sends. A pure module: no store, no backend, only erasable
// TypeScript, tested by settings.test.ts under `node --test`.
//
// Numbers. A typed value is read by `parseStrictDecimal` (Rust's reader rounds the same way), and
// a time step typed in milliseconds becomes seconds by moving the decimal point in the text, never
// by dividing a float: `2.1 / 1000` is 0.0021000000000000003, not the 0.0021 the user wrote.
import type { BandKind, BandSet, Environment, MeshSettings, SolverSettings, SppsSettings } from '../../bindings/schema.ts';
import { NOT_A_NUMBER, parseStrictDecimal, type Parsed } from '../../numbers.ts';

/** A number as the schema stores it: finite values as numbers, non-finite ones as strings. */
type F64 = number | string;

/** The largest value a solver's C `int` holds: the particle counts' upper bound. */
export const SOLVER_INT_MAX = 2_147_483_647;

/** A particle count step counter: a particle's step counter is 16 bits (`MAX_TIME_STEPS`). */
export const MAX_TIME_STEPS = 65_536;

/** The UI code of a count that is not a whole number from 0 to 2,147,483,647. */
export const NOT_A_COUNT = 'NOT_A_COUNT';

export type ParsedCount = { ok: true; value: number } | { ok: false; code: typeof NOT_A_COUNT; text: string };

/**
 * A particle count as typed: digits only (an optional `+`), from 0 to 2,147,483,647. Never
 * "auto" (PQ3 C7: the true number is shown and taken).
 */
export function parseCount(text: string): ParsedCount {
  const t = text.trim();
  if (!/^\+?\d+$/.test(t)) return { ok: false, code: NOT_A_COUNT, text };
  const digits = t.replace(/^\+/, '');
  if (BigInt(digits) > BigInt(SOLVER_INT_MAX)) return { ok: false, code: NOT_A_COUNT, text };
  return { ok: true, value: Number(digits) };
}

/**
 * `text` (a decimal, plain or with an exponent, as `String(number)` or a user writes it) times
 * 10^`places`, as a plain decimal, by moving the point in the string. Leading and trailing zeros
 * are dropped; zero is `0`.
 */
export function moveDecimalPoint(text: string, places: number): string {
  const m = /^([+-]?)(\d*)(?:\.(\d*))?(?:[eE]([+-]?\d+))?$/.exec(text.trim());
  if (!m || (m[2] === '' && (m[3] ?? '') === '')) throw new Error(`moveDecimalPoint: '${text}' is not a decimal`);
  const sign = m[1] === '-' ? '-' : '';
  const int = m[2];
  const frac = m[3] ?? '';
  const digits = int + frac;
  const point = int.length + Number(m[4] ?? '0') + places;
  let a: string;
  let b: string;
  if (point <= 0) {
    a = '0';
    b = '0'.repeat(-point) + digits;
  } else if (point >= digits.length) {
    a = digits + '0'.repeat(point - digits.length);
    b = '';
  } else {
    a = digits.slice(0, point);
    b = digits.slice(point);
  }
  a = a.replace(/^0+(?=\d)/, '');
  b = b.replace(/0+$/, '');
  if (/^0*$/.test(a + b)) return '0';
  return `${sign}${a}${b ? `.${b}` : ''}`;
}

/** A time step typed in milliseconds, as the seconds value its decimal names (bit for bit). */
export function secondsFromMs(text: string): Parsed {
  const parsed = parseStrictDecimal(text);
  if (!parsed.ok) return parsed;
  const value = Number(moveDecimalPoint(text.trim(), -3));
  return Number.isFinite(value) ? { ok: true, value } : { ok: false, code: NOT_A_NUMBER, text };
}

/** A stored time step (seconds) as the millisecond text its field shows. */
export function timeStepInputText(v: F64): string {
  return typeof v === 'number' ? moveDecimalPoint(String(v), 3) : v;
}

/** A stored real as its field shows it: the shortest round-trip spelling. */
export function realInputText(v: F64): string {
  return typeof v === 'number' ? String(v) : v;
}

/**
 * The step count as the solver computes it, `(int)ceil(duration / step)` in `float`
 * (`base_core_configuration.cpp:94`), and in `f64`, the larger: the validator's rule
 * (`step_count_overflow`). `null` when either value is not above 0 and finite.
 */
export function stepCount(duration: F64, step: F64): number | null {
  if (typeof duration !== 'number' || typeof step !== 'number') return null;
  const d32 = Math.fround(duration);
  const s32 = Math.fround(step);
  if (!(duration > 0 && step > 0 && d32 > 0 && s32 > 0 && Number.isFinite(d32) && Number.isFinite(s32))) return null;
  const n = Math.max(Math.ceil(duration / step), Math.ceil(Math.fround(d32 / s32)));
  return Number.isFinite(n) ? n : null;
}

/** `150,000`: an integer grouped by thousands, the same on every machine (no locale). */
function grouped(n: number | bigint): string {
  return String(n).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

/** `10,000 steps`, said plainly when it is past what the solver can count; an em dash when none. */
export function stepCountText(duration: F64, step: F64): string {
  const n = stepCount(duration, step);
  if (n === null) return '—';
  const text = `${grouped(n)} steps`;
  return n >= MAX_TIME_STEPS ? `${text}, above the ${grouped(MAX_TIME_STEPS - 1)} the solver can count` : text;
}

/**
 * The particle file's size per band, in bytes, by upstream's formula (`e_core_sppscore.h:201`):
 * saved particles x steps x 16 bytes (four floats a step) x active sources. Exact (`bigint`).
 */
export function pbinBytes(saved: number, steps: number, activeSources: number): bigint {
  return BigInt(saved) * BigInt(steps) * 16n * BigInt(activeSources);
}

/** `160 MB`, `1.1 GB`: decimal units (upstream's "Mo" is 10^6 bytes), to a tenth, half up. */
export function sizeText(bytes: bigint): string {
  const units: [string, bigint][] = [
    ['TB', 10n ** 12n],
    ['GB', 10n ** 9n],
    ['MB', 10n ** 6n],
    ['kB', 10n ** 3n],
  ];
  for (const [name, size] of units) {
    if (bytes >= size) {
      const tenths = (bytes * 10n + size / 2n) / size;
      const int = tenths / 10n;
      const frac = tenths % 10n;
      return `${grouped(int)}${frac ? `.${frac}` : ''} ${name}`;
    }
  }
  return `${bytes} B`;
}

// ---- band presets (C26) ---------------------------------------------------------------------------

/** The nominal frequencies the core allows (`schema::bands`), ascending. */
const NOMINAL: Record<BandKind, readonly number[]> = {
  octave: [63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000],
  third_octave: [
    50, 63, 80, 100, 125, 160, 200, 250, 315, 400, 500, 630, 800, 1000, 1250, 1600, 2000, 2500, 3150, 4000, 5000, 6300,
    8000, 10000, 12500, 16000, 20000,
  ],
};

export interface BandPreset {
  key: string;
  label: string;
  kind: BandKind;
  lowest_hz: number;
  highest_hz: number;
}

const preset = (kind: BandKind, lowest_hz: number, highest_hz: number, label: string): BandPreset => ({
  key: `${kind}-${lowest_hz}-${highest_hz}`,
  label,
  kind,
  lowest_hz,
  highest_hz,
});

/**
 * Upstream's four presets (`e_core_core_bfreqselection.h:90-140`) and ours, the new-project
 * default (decision 43: STI needs the seven octaves 125 Hz to 8 kHz), first.
 */
export const BAND_PRESETS: readonly BandPreset[] = [
  preset('octave', 125, 8000, 'Octaves 125 Hz – 8 kHz (new project, STI)'),
  preset('octave', 63, 16000, 'Octaves 63 Hz – 16 kHz (all)'),
  preset('octave', 125, 4000, 'Octaves 125 Hz – 4 kHz (building, road)'),
  preset('third_octave', 50, 20000, 'Third-octaves 50 Hz – 20 kHz (all)'),
  preset('third_octave', 100, 5000, 'Third-octaves 100 Hz – 5 kHz (building, road)'),
];

/** Every band of a preset. */
export function presetBands(p: BandPreset): number[] {
  const all = NOMINAL[p.kind];
  return all.filter((f) => f >= p.lowest_hz && f <= p.highest_hz);
}

/** The preset `bands` is, or null for any other set. */
export function bandPresetOf(bands: BandSet): BandPreset | null {
  return (
    BAND_PRESETS.find((p) => {
      if (p.kind !== bands.kind) return false;
      const want = presetBands(p);
      return want.length === bands.frequencies_hz.length && want.every((f, i) => f === bands.frequencies_hz[i]);
    }) ?? null
  );
}

// ---- the settings an edit sends --------------------------------------------------------------------

/** The solver settings with SPPS fields replaced; everything else as stored. */
export function withSpps(s: SolverSettings, patch: Partial<SppsSettings>): SolverSettings {
  return { ...s, spps: { ...s.spps, ...patch } };
}

/** The solver settings with meshing fields replaced (`-Y`, backlog 80); everything else as stored. */
export function withMeshing(s: SolverSettings, patch: Partial<MeshSettings>): SolverSettings {
  return { ...s, meshing: { ...s.meshing, ...patch } };
}

/** The environment with the air's fields replaced; everything else as stored. */
export function withAir(
  env: Environment,
  patch: Partial<Pick<Environment, 'temperature_c' | 'relative_humidity_percent' | 'pressure_pa'>>,
): Environment {
  return { ...env, ...patch };
}
