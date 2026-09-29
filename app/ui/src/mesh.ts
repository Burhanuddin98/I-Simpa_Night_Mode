// Decodes `scene_mesh` (PLAN.md 1.9; app/src-tauri/src/scene.rs `mesh_bytes`). A pure module,
// tested by mesh.test.ts under `node --test`.
//
//   offset 0            u32  magic 0x4853454D ("MESH")
//          4            u32  version 1
//          8            u32  nv, vertices
//          12           u32  nf, faces
//          16           u64  geometry_rev
//          24           f64 x 3nv   positions, world metres, bit-exact from the project
//          24+24nv      u32 x 3nf   vertex indices, in project face order
//          24+24nv+12nf u32 x nf    group index (position in view.surface_groups)
// Little-endian throughout.

export const MESH_MAGIC = 0x4853454d;
export const MESH_VERSION = 1;
const HEADER = 24;

export interface SceneMesh {
  geometryRev: number;
  vertexCount: number;
  faceCount: number;
  /** x, y, z per vertex, as the project holds them (no copy of the buffer). */
  positions: Float64Array;
  /** a, b, c per face, in project face order. */
  indices: Uint32Array;
  /** Per face, its group's position in `view.surface_groups` (0xFFFFFFFF if unknown). */
  groups: Uint32Array;
}

/** Throws on a buffer that is not a version-1 mesh of the size its header states. */
export function decodeMesh(buf: ArrayBuffer): SceneMesh {
  if (buf.byteLength < HEADER) throw new Error(`scene_mesh: ${buf.byteLength} bytes, shorter than the header`);
  const view = new DataView(buf);
  const magic = view.getUint32(0, true);
  if (magic !== MESH_MAGIC) throw new Error(`scene_mesh: magic 0x${magic.toString(16)}, not "MESH"`);
  const version = view.getUint32(4, true);
  if (version !== MESH_VERSION) throw new Error(`scene_mesh: version ${version}, expected ${MESH_VERSION}`);
  const nv = view.getUint32(8, true);
  const nf = view.getUint32(12, true);
  const rev = view.getBigUint64(16, true);
  const want = HEADER + 24 * nv + 16 * nf;
  if (buf.byteLength !== want) throw new Error(`scene_mesh: ${buf.byteLength} bytes, the header says ${want}`);
  const faceStart = HEADER + 24 * nv;
  // Typed arrays read the platform's byte order; every WebView2 platform is little-endian.
  return {
    geometryRev: Number(rev),
    vertexCount: nv,
    faceCount: nf,
    positions: new Float64Array(buf, HEADER, 3 * nv),
    indices: new Uint32Array(buf, faceStart, 3 * nf),
    groups: new Uint32Array(buf, faceStart + 12 * nf, nf),
  };
}
