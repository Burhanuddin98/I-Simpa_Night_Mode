// Decodes `run_surface_map` and `run_particles` (M12 PLAN.md P1 item 3; app/src-tauri/src/
// results_data.rs, whose header holds both layouts). A pure module, tested by resultsData.test.ts
// under `node --test`. Every value is a view on the buffer: the file's float32s, bit for bit.
//
//   SMAP v1                                   PART v1
//   0  u32 magic 0x50414D53 ("SMAP")          0  u32 magic 0x54524150 ("PART")
//   4  u32 version 1                          4  u32 version 1
//   8  u32 nn nodes                           8  u32 np particles
//   12 u32 nf faces                           12 u32 nr step records
//   16 u32 nr records                         16 u32 nb_time_step_max
//   20 u32 time-step count                    20 f32 time step, s
//   24 f32 time step, s                       24 i32 band, Hz
//   28 i32 record type                        28 u32 0
//   32 f32 x 3nn positions                    32 u32 x np first time step
//      u32 x 3nf vertices                        u32 x np+1 record offsets
//      u32 x nf receiver                         f32 x 3nr positions
//      u32 x nf+1 record offsets                 f32 x nr energies
//      u32 x nr step
//      f32 x nr value
// Little-endian throughout; every WebView2 platform is little-endian, as mesh.ts assumes.

export const SMAP_MAGIC = 0x50414d53;
export const PART_MAGIC = 0x54524150;
export const LAYOUT_VERSION = 1;
const HEADER = 32;

export interface SurfaceMap {
  nodeCount: number;
  faceCount: number;
  recordCount: number;
  timeStepCount: number;
  /** The file's float32 time step, s. */
  timeStepS: number;
  recordType: number;
  /** x, y, z per node, as the file stores them. */
  positions: Float32Array;
  /** a, b, c per face, into the nodes. */
  indices: Uint32Array;
  /** Per face, its receiver's position in `run_data`'s `receivers` for this file. */
  receivers: Uint32Array;
  /** Face i holds records [offsets[i], offsets[i+1]). */
  offsets: Uint32Array;
  /** Per record, its time step. */
  steps: Uint32Array;
  /** Per record, its value, the file's float32. */
  values: Float32Array;
}

export interface Particles {
  particleCount: number;
  recordCount: number;
  maxSteps: number;
  /** The file's float32 time step, s. */
  timeStepS: number;
  bandHz: number;
  /** Per particle, the time step of its first record. */
  firstStep: Uint32Array;
  /** Particle i holds records [offsets[i], offsets[i+1]); record k is at step firstStep[i] + k - offsets[i]. */
  offsets: Uint32Array;
  /** x, y, z per record. */
  positions: Float32Array;
  /** Per record, its energy. */
  energies: Float32Array;
}

function header(buf: ArrayBuffer, what: string, magic: number, tag: string): DataView {
  if (buf.byteLength < HEADER) throw new Error(`${what}: ${buf.byteLength} bytes, shorter than the header`);
  const view = new DataView(buf);
  const m = view.getUint32(0, true);
  if (m !== magic) throw new Error(`${what}: magic 0x${m.toString(16)}, not "${tag}"`);
  const version = view.getUint32(4, true);
  if (version !== LAYOUT_VERSION) throw new Error(`${what}: version ${version}, expected ${LAYOUT_VERSION}`);
  return view;
}

/** Throws on a buffer that is not an SMAP v1 of the size its header states. */
export function decodeSurfaceMap(buf: ArrayBuffer): SurfaceMap {
  const view = header(buf, 'run_surface_map', SMAP_MAGIC, 'SMAP');
  const nn = view.getUint32(8, true);
  const nf = view.getUint32(12, true);
  const nr = view.getUint32(16, true);
  const want = HEADER + 12 * nn + 12 * nf + 4 * nf + 4 * (nf + 1) + 8 * nr;
  if (buf.byteLength !== want) throw new Error(`run_surface_map: ${buf.byteLength} bytes, the header says ${want}`);
  let o = HEADER;
  const take = <T>(make: (b: ArrayBuffer, at: number, n: number) => T, n: number): T => {
    const out = make(buf, o, n);
    o += 4 * n;
    return out;
  };
  const f32 = (b: ArrayBuffer, at: number, n: number) => new Float32Array(b, at, n);
  const u32 = (b: ArrayBuffer, at: number, n: number) => new Uint32Array(b, at, n);
  return {
    nodeCount: nn,
    faceCount: nf,
    recordCount: nr,
    timeStepCount: view.getUint32(20, true),
    timeStepS: view.getFloat32(24, true),
    recordType: view.getInt32(28, true),
    positions: take(f32, 3 * nn),
    indices: take(u32, 3 * nf),
    receivers: take(u32, nf),
    offsets: take(u32, nf + 1),
    steps: take(u32, nr),
    values: take(f32, nr),
  };
}

/** Face `face`'s value at time step `step`, the file's float32; `null` where it has no record. */
export function surfaceValue(m: SurfaceMap, face: number, step: number): number | null {
  for (let k = m.offsets[face]; k < m.offsets[face + 1]; k++) {
    if (m.steps[k] === step) return m.values[k];
  }
  return null;
}

/** Throws on a buffer that is not a PART v1 of the size its header states. */
export function decodeParticles(buf: ArrayBuffer): Particles {
  const view = header(buf, 'run_particles', PART_MAGIC, 'PART');
  const np = view.getUint32(8, true);
  const nr = view.getUint32(12, true);
  const want = HEADER + 4 * np + 4 * (np + 1) + 16 * nr;
  if (buf.byteLength !== want) throw new Error(`run_particles: ${buf.byteLength} bytes, the header says ${want}`);
  const off = HEADER + 4 * np;
  const pos = off + 4 * (np + 1);
  return {
    particleCount: np,
    recordCount: nr,
    maxSteps: view.getUint32(16, true),
    timeStepS: view.getFloat32(20, true),
    bandHz: view.getInt32(24, true),
    firstStep: new Uint32Array(buf, HEADER, np),
    offsets: new Uint32Array(buf, off, np + 1),
    positions: new Float32Array(buf, pos, 3 * nr),
    energies: new Float32Array(buf, pos + 12 * nr, nr),
  };
}
