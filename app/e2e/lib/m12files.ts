// The M12 viewport spec's own readers of the solver's files (gate (c), (d)): a `.csbin` and a
// `.pbin` read here from the bytes on disk, by docs/formats/csbin.md and pbin.md, and by nothing
// of the app's (not `results_data.rs`, not `resultsData.ts`), so a fault in the app's path to
// the screen cannot also be in the reference it is checked against. Tested by m12files.test.ts
// on the committed fixtures.
import { readFileSync } from 'node:fs';

export interface CsbinFace {
  /** [time step, the value's float32 bits], in file order. */
  records: [number, number][];
}

export interface Csbin {
  nodes: number;
  timeSteps: number;
  recordType: number;
  /** Every receiver's faces, in file order: the map's face index is the position here. */
  faces: CsbinFace[];
}

/** A `.csbin` (version 3), every record stride from its stored lengths. Throws on a short file. */
export function readCsbin(path: string): Csbin {
  const b = readFileSync(path);
  const d = new DataView(b.buffer, b.byteOffset, b.byteLength);
  const need = (o: number, n: number) => {
    if (o + n > b.byteLength) throw new Error(`${path}: ends at ${b.byteLength}, a record needs ${o + n}`);
  };
  need(0, 44);
  if (d.getInt32(0, true) !== 3) throw new Error(`${path}: version ${d.getInt32(0, true)}, not 3`);
  const [lh, ln, lr, lf, lv] = [4, 8, 12, 16, 20].map((o) => d.getUint32(o, true));
  const nodes = d.getInt32(24, true);
  const receivers = d.getInt32(28, true);
  const timeSteps = d.getInt32(32, true);
  const recordType = d.getInt32(40, true);
  let o = lh + nodes * ln;
  const faces: CsbinFace[] = [];
  for (let r = 0; r < receivers; r++) {
    need(o, 8);
    const quantFaces = d.getInt32(o + 4, true);
    o += lr;
    for (let f = 0; f < quantFaces; f++) {
      need(o, 16);
      const n = d.getInt32(o + 12, true);
      o += lf;
      const records: [number, number][] = [];
      for (let k = 0; k < n; k++) {
        need(o, 8);
        records.push([d.getUint16(o, true), d.getUint32(o + 4, true)]);
        o += lv;
      }
      faces.push({ records });
    }
  }
  if (o !== b.byteLength) throw new Error(`${path}: ${b.byteLength - o} bytes after the last record`);
  return { nodes, timeSteps, recordType, faces };
}

/** The float32 a value's bits stand for. */
export function f32(bits: number): number {
  const u = new Uint32Array([bits]);
  return new Float32Array(u.buffer)[0];
}

export interface Pbin {
  particles: number;
  records: number;
  stepsMax: number;
  /** Per step 0..stepsMax-1: the particles with a record there. */
  alive: number[];
}

/** A `.pbin` (Windows layout, 28/8/16): the particles alive at each step. */
export function readPbin(path: string): Pbin {
  const b = readFileSync(path);
  const d = new DataView(b.buffer, b.byteOffset, b.byteLength);
  if (b.byteLength < 28) throw new Error(`${path}: shorter than the header`);
  const particles = d.getUint32(0, true);
  const stepsMax = d.getUint32(20, true);
  const alive = new Array<number>(stepsMax).fill(0);
  let o = 28;
  let records = 0;
  for (let i = 0; i < particles; i++) {
    if (o + 8 > b.byteLength) throw new Error(`${path}: particle ${i} runs past the end`);
    const n = d.getUint32(o, true);
    const first = d.getUint16(o + 4, true);
    o += 8 + 16 * n;
    records += n;
    for (let k = 0; k < n; k++) if (first + k < stepsMax) alive[first + k]++;
  }
  if (o !== b.byteLength) throw new Error(`${path}: ${b.byteLength - o} bytes after the last particle`);
  return { particles, records, stepsMax, alive };
}

/** Upstream's level of an energy, dB re 1e-12 (`Recepteurs_surfacique.cpp`); null for none. */
export function level(e: number): number | null {
  return e > 0 && Number.isFinite(e) ? 10 * Math.log10(e / 1e-12) : null;
}
