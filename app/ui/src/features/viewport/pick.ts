// The model's GPU geometry and its picking BVH (PLAN.md 6.1, finding F6). Imports three.js and
// three-mesh-bvh only, so pick.test.ts runs it under `node --test` on real meshes.
//
// - Positions arrive as the project's f64 and go to the GPU as f32; the index stays in project
//   face order.
// - The BVH is built with `indirect: true`: it sorts its own buffer and leaves the geometry's
//   index alone, so a hit's faceIndex is the project's face index. Without it, three-mesh-bvh
//   reorders the index in place and a pick names the wrong face (and the wrong group).
// - Picks honour BackSide, as the faces are drawn: a face is hit only from the room's side.
import { BackSide, BufferAttribute, BufferGeometry, type Ray, type Vector3 } from 'three';
import { MeshBVH } from 'three-mesh-bvh';

/** The model as three.js draws it: f32 positions, the project's index. */
export function modelGeometry(positions: Float64Array, indices: Uint32Array): BufferGeometry {
  const geometry = new BufferGeometry();
  geometry.setAttribute('position', new BufferAttribute(new Float32Array(positions), 3));
  geometry.setIndex(new BufferAttribute(new Uint32Array(indices), 1));
  return geometry;
}

/** The picking BVH, indirect, so the geometry's index keeps project face order. */
export function pickingBvh(geometry: BufferGeometry): MeshBVH {
  return new MeshBVH(geometry, { indirect: true });
}

/** The first face a ray meets from the room's side (BackSide), in project face numbering. */
export function firstFace(bvh: MeshBVH, ray: Ray): { face: number; distance: number; point: Vector3 } | null {
  const hit = bvh.raycastFirst(ray, BackSide);
  if (!hit || typeof hit.faceIndex !== 'number') return null;
  return { face: hit.faceIndex, distance: hit.distance, point: hit.point };
}
