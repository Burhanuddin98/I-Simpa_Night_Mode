// R72 (parity, upstream IHM/simpleGraphDialogs.cpp:548-553: an axis's minimum, maximum and tick
// spacing set by hand): the axes a chart of the Acoustics tab is drawn on when the person sets
// them, read from what was typed. A pure module, tested by chartAxes.test.ts under `node --test`.
//
// Only the frame moves: the values drawn are the same, a value outside the range typed is simply
// off the chart, and "Auto" gives the chart its own range back.

/** A range typed by hand, [from, to], from below to. */
export type Span = readonly [number, number];

/** The axes set by hand; an axis left out keeps the chart's own range and ticks. */
export interface HandAxes {
  x?: Span;
  y?: Span;
  /** The spacing of the value axis's ticks, in its unit. */
  yStep?: number;
}

/** The most ticks a typed spacing may draw on an axis: a finer one would be a wall of labels. */
export const MAX_TICKS = 50;

/** What the axes editor holds, as typed (empty: not set). */
export interface AxesText {
  xFrom?: string;
  xTo?: string;
  yFrom?: string;
  yTo?: string;
  yStep?: string;
}

/** A typed number: `1.5`, `-60`, `1,5` (a decimal comma, as a German keyboard types it). */
function parse(t: string | undefined): number | null | 'bad' {
  const s = (t ?? '').trim().replace(',', '.').replace('−', '-');
  if (s === '') return null;
  if (!/^[-+]?(\d+\.?\d*|\.\d+)$/.test(s)) return 'bad';
  const v = Number(s);
  return Number.isFinite(v) ? v : 'bad';
}

/** The axes the text sets, or why it does not: both ends or neither, from below to, a spacing
 * above 0 giving at most `MAX_TICKS` ticks over the range (the typed one, else `yRange`, the
 * chart's own). */
export function handAxes(t: AxesText, yRange: Span | null = null): { axes: HandAxes; error: null } | { axes: null; error: string } {
  const out: HandAxes = {};
  const span = (from: string | undefined, to: string | undefined, name: string): Span | null | string => {
    const a = parse(from);
    const b = parse(to);
    if (a === 'bad' || b === 'bad') return `${name}: type a number`;
    if (a === null && b === null) return null;
    if (a === null || b === null) return `${name}: give both ends, or neither`;
    if (!(a < b)) return `${name}: "from" must be below "to"`;
    return [a, b];
  };
  const x = span(t.xFrom, t.xTo, 'Horizontal axis');
  if (typeof x === 'string') return { axes: null, error: x };
  if (x) out.x = x;
  const y = span(t.yFrom, t.yTo, 'Vertical axis');
  if (typeof y === 'string') return { axes: null, error: y };
  if (y) out.y = y;
  const step = parse(t.yStep);
  if (step === 'bad') return { axes: null, error: 'Tick spacing: type a number' };
  if (step !== null) {
    if (!(step > 0)) return { axes: null, error: 'Tick spacing: above 0' };
    const over = out.y ?? yRange;
    if (over && (over[1] - over[0]) / step > MAX_TICKS) return { axes: null, error: `Tick spacing: at most ${MAX_TICKS} ticks over the axis; take a wider spacing` };
    out.yStep = step;
  }
  return { axes: out, error: null };
}

/** The ticks at whole multiples of `step` from `min` to `max` (a little past each end is kept off),
 * at most `MAX_TICKS + 1`. */
export function stepTicks(min: number, max: number, step: number): number[] {
  if (!(step > 0) || !Number.isFinite(min) || !Number.isFinite(max) || max < min) return [];
  const out: number[] = [];
  const first = Math.ceil(min / step - 1e-9);
  for (let k = first; out.length <= MAX_TICKS; k++) {
    const v = k * step;
    if (v > max + step * 1e-9) break;
    // Rounded to the step's own decimals, so 0.1 * 3 reads 0.3.
    out.push(Number(v.toFixed(Math.max(0, Math.min(12, -Math.floor(Math.log10(step)) + 2)))));
  }
  return out;
}
