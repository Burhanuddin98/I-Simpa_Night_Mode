// Band columns, band labels and the natural sorts of the materials grid (PLAN.md 6.2). A pure
// module: no store, no backend, only erasable TypeScript, tested by bands.test.ts under
// `node --test`.

/** A number as the schema stores it: finite values as numbers, non-finite ones as strings. */
export type F64 = number | string;

/** One band column of the grid: where it sits in the project's per-band arrays, and its label. */
export interface BandColumn {
  /** Index into `bands.frequencies_hz` and into every per-band array of the project. */
  index: number;
  hz: number;
  label: string;
}

/** A band's short label: `125`, `1k`, `1.25k`, `20k`. */
export function bandLabel(hz: number): string {
  // hz / 1000 is correctly rounded, so 3150 gives the double nearest 3.15, spelt "3.15".
  return hz >= 1000 ? `${String(hz / 1000)}k` : String(hz);
}

/** The grid's columns: one per band, in ascending frequency (the natural band sort). */
export function bandColumns(frequenciesHz: readonly number[]): BandColumn[] {
  return frequenciesHz
    .map((hz, index) => ({ index, hz, label: bandLabel(hz) }))
    .sort((a, b) => a.hz - b.hz || a.index - b.index);
}

/** `125`, `125 Hz`, `125Hz`, `1k`, `1 kHz`, `1.25k`: a number and an optional unit. */
const BAND_TEXT = /^([0-9]+)(?:\.([0-9]+))?\s*(k|khz|hz)?$/i;

/**
 * Reads a band label as a frequency in Hz, or null. A label must carry a unit or a `k` suffix
 * (`125 Hz`, `1k`, `1 kHz`); a bare number (`125`) is read only when `allowBare` is set, which
 * the paste does in a header whose first cell is text. The `k` scaling shifts the decimal point
 * in the text, so `3.15k` is exactly 3150, never 3150.0000000000005.
 */
export function parseBandLabel(text: string, allowBare = false): number | null {
  const m = BAND_TEXT.exec(text.trim());
  if (!m) return null;
  const whole = m[1];
  const frac = m[2] ?? '';
  const unit = m[3]?.toLowerCase();
  if (!unit && !allowBare) return null;
  if (unit === 'k' || unit === 'khz') {
    const f = frac.padEnd(3, '0');
    return Number(`${whole}${f.slice(0, 3)}.${f.slice(3) || '0'}`);
  }
  return Number(frac ? `${whole}.${frac}` : whole);
}

/** The band range as text, for messages: `125 … 4k`. */
export function bandRange(columns: readonly BandColumn[]): string {
  if (columns.length === 0) return 'no bands';
  return `${columns[0].label} … ${columns[columns.length - 1].label}`;
}

// ---- natural sort -----------------------------------------------------------------------------

const RUNS = /[0-9]+|[^0-9]+/g;
const DIGITS = /^[0-9]/;

function cmp(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

function compareDigits(a: string, b: string): number {
  const x = a.replace(/^0+(?=[0-9])/, '');
  const y = b.replace(/^0+(?=[0-9])/, '');
  return x.length - y.length || cmp(x, y);
}

/**
 * Natural order of names: digit runs compare by value, so `Material 2` comes before
 * `Material 10`; text runs compare case-insensitively. The order is total: names equal up to
 * case and leading zeros fall back to plain code-unit order. Locale-independent on purpose.
 */
export function naturalCompare(a: string, b: string): number {
  const x = a.match(RUNS) ?? [];
  const y = b.match(RUNS) ?? [];
  const n = Math.min(x.length, y.length);
  for (let i = 0; i < n; i++) {
    const p = x[i];
    const q = y[i];
    const c = DIGITS.test(p) && DIGITS.test(q) ? compareDigits(p, q) : cmp(p.toLowerCase(), q.toLowerCase());
    if (c !== 0) return Math.sign(c);
  }
  if (x.length !== y.length) return x.length < y.length ? -1 : 1;
  return cmp(a, b);
}

/** Band values in ascending order; non-finite values (strings) after every number. */
export function compareF64(a: F64, b: F64): number {
  if (typeof a === 'number' && typeof b === 'number') return a < b ? -1 : a > b ? 1 : 0;
  if (typeof a === 'number') return -1;
  if (typeof b === 'number') return 1;
  return cmp(a, b);
}
