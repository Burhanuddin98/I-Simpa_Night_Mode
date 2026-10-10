// The RT chart's arrays and series (AcousticsPane.tsx `RtChart`), kept apart from the DOM so a test can
// hold them: the shown reverberation times, and the DIN target's line with its shaded fifth either side,
// drawn only when there is a target. With no target (the room's volume outside the group's range, the
// room not read) there is no target series at all, and the legend says why instead of promising a line.
import type uPlot from 'uplot';
import type { DinView } from './model.ts';

/** One shown parameter's values per band (`rtSeries`): a gap where the tables show none. */
export interface RtLine {
  param: string;
  label: string;
  values: (number | null)[];
  hi: (number | null)[];
}

/** The target's shading, a fifth either side of it. */
export const TARGET_BAND_FILL = 'rgba(161,161,170,0.12)';
/** How far the shading reaches either side of the target. */
export const TARGET_SPREAD = 0.2;

export interface RtPlot {
  data: uPlot.AlignedData;
  /** uPlot's series, the bands' x first. */
  series: uPlot.Series[];
  bands: uPlot.Band[];
  /** The largest range end shown, s (0 with none). */
  top: number;
  /** The uPlot indices of the target's line, its lower and its upper edge; empty with no target. */
  targetSeries: number[];
}

/** A target the chart can draw: a finite time above zero. */
export function drawableTarget(target: number | null | undefined): number | null {
  return typeof target === 'number' && Number.isFinite(target) && target > 0 ? target : null;
}

/** The target's key, said only while the target is drawn. */
export const TARGET_WORDS = 'target, shaded a fifth either side';

/** The RT chart's key for the target: `TARGET_WORDS` when there is one to draw, else why there is none
 * (the refusal's own words and the room's volume, from `din`), never a promise of a line that is not drawn. */
export function rtKey(target: number | null, group: string, d: DinView | null): string {
  if (drawableTarget(target) !== null) return TARGET_WORDS;
  if (!d) return 'no target, so none is drawn: the room was not read';
  const why = d.range ? `: ${d.range.text}, this room is ${d.volume?.text ?? '–'} m³` : '';
  return `no DIN 18041 ${group} target, so none is drawn${why}`;
}

/** The RT chart over `bands` bands: `lines` each in `colour(param)` at stroke scale `k`, and the target
 * (`targetInk`, dashed) with its shading when `target` is one. */
export function rtPlot(bands: number, lines: readonly RtLine[], target: number | null, k: number, colour: (param: string) => string, targetInk: string): RtPlot {
  const x = Array.from({ length: bands }, (_, i) => i);
  const t = drawableTarget(target);
  const n = lines.length;
  const top = Math.max(0, ...lines.flatMap((s) => s.hi.filter((v): v is number => v !== null)));
  const own: uPlot.Series[] = lines.map((s) => ({ label: s.label, stroke: colour(s.param), width: 2 * k, points: { size: 8 * k, fill: colour(s.param) }, spanGaps: false }));
  if (t === null) {
    return { data: [x, ...lines.map((s) => s.values)], series: [{}, ...own], bands: [], top, targetSeries: [] };
  }
  const flat = (f: number) => x.map(() => t * f);
  return {
    data: [x, ...lines.map((s) => s.values), flat(1), flat(1 - TARGET_SPREAD), flat(1 + TARGET_SPREAD)],
    series: [
      {},
      ...own,
      { label: 'DIN target', stroke: targetInk, width: 1.5 * k, dash: [4 * k, 4 * k], points: { show: false } },
      { label: 'lower', stroke: 'transparent', points: { show: false } },
      { label: 'upper', stroke: 'transparent', points: { show: false } },
    ],
    // uPlot fills from the first series down to the second: upper to lower.
    bands: [{ series: [n + 3, n + 2], fill: TARGET_BAND_FILL }],
    top,
    targetSeries: [n + 1, n + 2, n + 3],
  };
}
