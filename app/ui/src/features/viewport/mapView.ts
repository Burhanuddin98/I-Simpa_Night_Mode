// How a surface map is looked at (wow list W5: parity R46 smooth colour, R47 fixed range, R48
// iso-contours, R51 value probe; docs/investigations/2026-10-04-wow-w1w5/PLAN.md). Pure, tested by
// mapView.test.ts under `node --test`. The GPU side is resultsLayer.ts's.
//
// - Smooth colour is upstream's (`isimpa/3dengine/Core/Recepteurs_surfacique.cpp:321-363, 424-431`):
//   a node's energy at a step is the sum of its linked faces' energies there over the number of
//   linked faces, a face being linked when its energy over all steps is finite and not 0, and a
//   non-finite record read as 0. The level of that mean is interpolated across each face (the
//   value, not the RGB). It is a picture between the faces, not a value of the file: the probe and
//   the legend say so.
// - The probe reads one face's own record from the `.csbin` (SMAP's float32, bit for bit), never a
//   smoothed or interpolated value.
import type { SurfaceMap } from '../../resultsData.ts';
import { parseStrictDecimal } from '../../numbers.ts';
import { levelDb, type Range } from './mapData.ts';

/** The most faces a node may link for the GPU's average loop; a map above it is drawn flat only. */
export const MAX_NODE_FACES = 64;
/** The widest fixed colour range, dB. */
export const MAX_RANGE_DB = 200;
/** The contour spacings offered, dB. */
export const CONTOUR_STEPS_DB: readonly number[] = [1, 3, 6];

export interface NodeFaces {
  /** Node n links faces[offsets[n] .. offsets[n + 1]), in face order. */
  offsets: Uint32Array;
  faces: Uint32Array;
  /** The most faces any node links. */
  maxFaces: number;
}

const finiteOr0 = (v: number) => (Number.isFinite(v) ? v : 0);

/** Whether face f carries energy at some step: its records' sum, each non-finite one read as 0, is finite and not 0 (upstream's rule). */
function linked(m: SurfaceMap, f: number): boolean {
  let sum = 0;
  for (let k = m.offsets[f]; k < m.offsets[f + 1]; k++) sum += finiteOr0(m.values[k]);
  return sum !== 0 && Number.isFinite(sum);
}

/** Each node's linked faces, as a CSR list. */
export function nodeFaces(m: SurfaceMap): NodeFaces {
  const live = new Uint8Array(m.faceCount);
  const count = new Uint32Array(m.nodeCount);
  for (let f = 0; f < m.faceCount; f++) {
    if (!linked(m, f)) continue;
    live[f] = 1;
    for (let c = 0; c < 3; c++) count[m.indices[3 * f + c]]++;
  }
  const offsets = new Uint32Array(m.nodeCount + 1);
  let maxFaces = 0;
  for (let n = 0; n < m.nodeCount; n++) {
    offsets[n + 1] = offsets[n] + count[n];
    if (count[n] > maxFaces) maxFaces = count[n];
  }
  const faces = new Uint32Array(offsets[m.nodeCount]);
  const at = offsets.slice(0, m.nodeCount);
  for (let f = 0; f < m.faceCount; f++) {
    if (!live[f]) continue;
    for (let c = 0; c < 3; c++) faces[at[m.indices[3 * f + c]]++] = f;
  }
  return { offsets, faces, maxFaces };
}

/** Face f's value at the step: its record, 0 where it has none. */
function faceEnergy(m: SurfaceMap, f: number, step: number): number {
  for (let k = m.offsets[f]; k < m.offsets[f + 1]; k++) if (m.steps[k] === step) return m.values[k];
  return 0;
}

/** Node n's energy at the step (upstream's mean); 0 for a node with no linked face. */
export function nodeEnergy(m: SurfaceMap, nf: NodeFaces, n: number, step: number): number {
  const a = nf.offsets[n];
  const b = nf.offsets[n + 1];
  if (b === a) return 0;
  let sum = 0;
  for (let k = a; k < b; k++) sum += finiteOr0(faceEnergy(m, nf.faces[k], step));
  return sum / (b - a);
}

export type RangeField = { ok: true; range: Range } | { ok: false; message: string };

/** A fixed colour range typed as its two ends, dB. */
export function parseRange(loText: string, hiText: string): RangeField {
  const lo = parseStrictDecimal(loText);
  const hi = parseStrictDecimal(hiText);
  if (!lo.ok) return { ok: false, message: `"${loText}" is not a number: write the low end in dB, like 40` };
  if (!hi.ok) return { ok: false, message: `"${hiText}" is not a number: write the high end in dB, like 70` };
  if (!(lo.value < hi.value)) return { ok: false, message: `The low end must be below the high end: ${lo.value} dB is not below ${hi.value} dB` };
  if (hi.value - lo.value > MAX_RANGE_DB) return { ok: false, message: `A range is at most ${MAX_RANGE_DB} dB wide: ${hi.value - lo.value} dB is not` };
  return { ok: true, range: { lo: lo.value, hi: hi.value } };
}

/** `120 ms`: a step's start time from the run's float32 time step (the transport's label). */
export function stepTime(step: number, dt: number | null | undefined): string {
  if (!dt) return `step ${step}`;
  const ms = step * dt * 1000;
  return `${ms < 100 ? ms.toFixed(1) : Math.round(ms)} ms`;
}

/** A float32's bits. */
export function valueBits(v: number): number {
  return new Uint32Array(new Float32Array([v]).buffer)[0];
}

export interface ProbeView {
  face: number;
  step: number;
  /** What, band and time. */
  title: string;
  /** The level, `64.8 dB` (a difference `+3.0 dB`); null where the file has no energy. */
  level: string | null;
  /** The face's record's bits (this run's), null where it has none. */
  bits: number | null;
  /** The file's value as stored, or why there is no number. */
  value: string;
  /** Under smooth colour: what the number is and what the colours are. */
  note: string | null;
}

const signed = (d: number) => `${d > 0 ? '+' : d < 0 ? '−' : ''}${Math.abs(d).toFixed(1)} dB`;

/** The probe of face `face` at `step`: the face's own `.csbin` record, never a smoothed value. */
export function probeOf(
  m: SurfaceMap,
  face: number,
  step: number,
  o: { what: string; band: string; dtS: number | null | undefined; smooth: boolean; base?: SurfaceMap | null },
): ProbeView {
  const title = `${o.what} · ${o.band} · ${stepTime(step, o.dtS)}`;
  const note = o.smooth ? "The face's own value from the file. The colours between faces are smoothed, not values." : null;
  let rec: number | null = null;
  for (let k = m.offsets[face]; k < m.offsets[face + 1]; k++) if (m.steps[k] === step) rec = m.values[k];
  const la = rec === null ? null : levelDb(rec);
  const bits = rec === null ? null : valueBits(rec);
  if (o.base) {
    let rb: number | null = null;
    for (let k = o.base.offsets[face]; k < o.base.offsets[face + 1]; k++) if (o.base.steps[k] === step) rb = o.base.values[k];
    const lb = rb === null ? null : levelDb(rb);
    if (la === null || lb === null) return { face, step, title, level: null, bits, value: 'No energy in one of the runs at this step', note };
    return { face, step, title, level: signed(la - lb), bits, value: `this run ${la.toFixed(1)} dB, baseline ${lb.toFixed(1)} dB`, note };
  }
  if (la === null || rec === null) return { face, step, title, level: null, bits: null, value: 'No energy at this step', note };
  return { face, step, title, level: `${la.toFixed(1)} dB`, bits, value: `file value ${rec.toExponential(4)}`, note };
}

/** The legend's line for contours every `stepDb` dB; null when off. */
export function contourText(stepDb: number): string | null {
  return stepDb > 0 ? `Contours every ${stepDb} dB, on the smoothed levels` : null;
}
