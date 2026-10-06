// The surface map's data, before the GPU (M12 P3: R38, R49, R50, R52). Pure, tested by
// mapData.test.ts under `node --test`.
//
// - The texture is faces x steps, face-major: texel i = face * steps + step, wrapped into rows of
//   the GPU's texture size limit (a map wider than one row would otherwise be cut). One float32
//   per texel, the `.csbin` value bit for bit (SMAP v1 carries them as the file stores them);
//   0 where the file has no record for that face and step.
// - A level is upstream's `10 log10(E / 1e-12)` (`isimpa/3dengine/Core/Recepteurs_surfacique.cpp`,
//   "Conversion en dB"), for record types SPL_STANDART and SPL_GAIN; a cell with no energy has no
//   level and is not coloured.
// - Levels run black through the brand red to white (decision 63), each stop lighter than the last by an
//   even step (CIELAB L* 3 to 98, steps 9 to 13; picked at even OKLab lightness), so a louder cell always
//   reads brighter. A difference from the baseline runs through black at 0, the cool ramp (the approved
//   design's COOL) where this run is quieter and the hot ramp where it is louder.
import type { SurfaceMap } from '../../resultsData.ts';

export const HOT: readonly string[] = ['#0B0B0E', '#420C12', '#72111A', '#A31823', '#D81F2D', '#EF5F5A', '#FD938B', '#FFC5BE', '#FFF7F2'];
export const COOL: readonly string[] = ['#0E1116', '#15212C', '#1C3446', '#264B63', '#356683', '#4A84A5', '#6AA6C6', '#A3CDE3'];

/** The deepest level range drawn, dB below the top. */
export const LEVEL_DEPTH_DB = 60;
/** float32 values read as levels are good to about 3e-7 dB: rounding to whole dB allows this. */
const DB_SLACK = 1e-5;

/** The record types whose values are energies (`docs/formats/csbin.md`, recordType 0 and 1). */
export const ENERGY_RECORD_TYPES: readonly number[] = [0, 1];

export interface MapLayout {
  faces: number;
  steps: number;
  width: number;
  height: number;
}

/** Faces x steps texels in rows of at most `maxSize`; refused when the rows would not fit. */
export function mapLayout(faces: number, steps: number, maxSize: number): MapLayout | { error: string } {
  const n = faces * steps;
  if (!(faces > 0 && steps > 0)) return { error: 'the map has no faces or no steps' };
  const width = Math.min(maxSize, n);
  const height = Math.ceil(n / width);
  if (height > maxSize) {
    return { error: `${faces} faces x ${steps} steps is more than one ${maxSize} x ${maxSize} texture holds` };
  }
  return { faces, steps, width, height };
}

/** The texel of (face, step): the same arithmetic as the shader's `mapTexel`. */
export function texelOf(l: MapLayout, face: number, step: number): { x: number; y: number } {
  const i = face * l.steps + step;
  return { x: i % l.width, y: Math.floor(i / l.width) };
}

/** The texture's data: every record's float32 bits at its texel, 0 elsewhere. */
export function denseValues(m: SurfaceMap, l: MapLayout): Float32Array {
  const out = new Uint32Array(l.width * l.height);
  const src = new Uint32Array(m.values.buffer, m.values.byteOffset, m.values.length);
  for (let f = 0; f < m.faceCount; f++) {
    for (let k = m.offsets[f]; k < m.offsets[f + 1]; k++) {
      const s = m.steps[k];
      if (s < l.steps) out[f * l.steps + s] = src[k];
    }
  }
  return new Float32Array(out.buffer);
}

/** Upstream's level of an energy, dB re 1e-12; null for none. */
export function levelDb(e: number): number | null {
  if (!(e > 0) || !Number.isFinite(e)) return null;
  return 10 * Math.log10(e) + 120;
}

export interface Range {
  lo: number;
  hi: number;
}

/** Whole dB around every level in the map, at most `LEVEL_DEPTH_DB` deep; null when none. */
export function levelRange(m: SurfaceMap): Range | null {
  let min = Infinity;
  let max = -Infinity;
  for (let k = 0; k < m.recordCount; k++) {
    const l = levelDb(m.values[k]);
    if (l === null) continue;
    if (l < min) min = l;
    if (l > max) max = l;
  }
  if (max === -Infinity) return null;
  const hi = Math.ceil(max - DB_SLACK);
  let lo = Math.max(Math.floor(min + DB_SLACK), hi - LEVEL_DEPTH_DB);
  if (lo >= hi) lo = hi - 1;
  return { lo, hi };
}

/** Level per (face, step) where the map has energy. */
function levels(m: SurfaceMap): Map<number, number> {
  const out = new Map<number, number>();
  for (let f = 0; f < m.faceCount; f++) {
    for (let k = m.offsets[f]; k < m.offsets[f + 1]; k++) {
      const l = levelDb(m.values[k]);
      if (l !== null) out.set(f * m.timeStepCount + m.steps[k], l);
    }
  }
  return out;
}

/** ±(whole dB, at least 1) around this run's level minus the baseline's, where both have one. */
export function diffRange(a: SurfaceMap, base: SurfaceMap): Range {
  const lb = levels(base);
  let worst = 0;
  for (const [cell, l] of levels(a)) {
    const b = lb.get(cell);
    if (b !== undefined) worst = Math.max(worst, Math.abs(l - b));
  }
  const d = Math.max(1, Math.ceil(worst - DB_SLACK));
  return { lo: -d, hi: d };
}

/** Why `base` cannot be subtracted from `a` cell by cell, or null when it can. */
export function surfaceMismatch(a: SurfaceMap, base: SurfaceMap): string | null {
  if (a.faceCount !== base.faceCount) return `the baseline's map has ${base.faceCount} faces, this run's ${a.faceCount}`;
  if (a.timeStepCount !== base.timeStepCount) return `the baseline has ${base.timeStepCount} steps, this run ${a.timeStepCount}`;
  if (a.nodeCount !== base.nodeCount) return `the baseline's map has ${base.nodeCount} nodes, this run's ${a.nodeCount}`;
  const pa = new Uint32Array(a.positions.buffer, a.positions.byteOffset, a.positions.length);
  const pb = new Uint32Array(base.positions.buffer, base.positions.byteOffset, base.positions.length);
  for (let i = 0; i < pa.length; i++) if (pa[i] !== pb[i]) return "the baseline's map nodes are not this run's";
  for (let i = 0; i < a.indices.length; i++) if (a.indices[i] !== base.indices[i]) return "the baseline's map faces are not this run's";
  return null;
}

const hex3 = (c: string): [number, number, number] => [parseInt(c.slice(1, 3), 16), parseInt(c.slice(3, 5), 16), parseInt(c.slice(5, 7), 16)];

/** The design's `ramp`: a colour along `stops` at t in [0, 1], 0-255 per channel. */
export function rampColor(stops: readonly string[], t: number): [number, number, number] {
  const u = Math.max(0, Math.min(1, t));
  const n = stops.length - 1;
  const k = Math.min(n - 1, Math.floor(u * n));
  const f = u * n - k;
  const a = hex3(stops[k]);
  const b = hex3(stops[k + 1]);
  return [0, 1, 2].map((i) => Math.round(a[i] + (b[i] - a[i]) * f)) as [number, number, number];
}

/** The stops as 0-1 RGB, for the shader. */
export const rampFloats = (stops: readonly string[]): number[] => stops.flatMap((c) => hex3(c).map((v) => v / 255));

/** The legend bar's CSS gradient. */
export function legendGradient(kind: 'level' | 'diff'): string {
  const stops = kind === 'level' ? [...HOT] : [...COOL].reverse().concat(HOT.slice(1));
  return `linear-gradient(90deg, ${stops.join(', ')})`;
}

const dbText = (v: number): string => {
  const s = Number.isInteger(v) ? String(Math.abs(v)) : Math.abs(v).toFixed(1);
  return v < 0 ? `−${s}` : s;
};

/** The legend's three labels: whole dB at the ends, the middle to a tenth when it falls between. */
export function legendLabels(r: Range, kind: 'level' | 'diff'): { lo: string; mid: string; hi: string } {
  const mid = (r.lo + r.hi) / 2;
  if (kind === 'diff') return { lo: dbText(r.lo), mid: dbText(mid), hi: `+${dbText(r.hi)} dB` };
  return { lo: dbText(r.lo), mid: dbText(mid), hi: `${dbText(r.hi)} dB` };
}
