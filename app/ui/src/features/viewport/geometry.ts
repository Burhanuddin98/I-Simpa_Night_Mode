// The viewport's geometry arithmetic, in f64 on the project's own positions: face centroids,
// the placement point, the floor test, the bounding box, the plan label and camera fit, and the
// axis gizmo. A pure module (PLAN.md 2.4, rule 8), tested by geometry.test.ts.
import { faceNormalOf } from './floodfill.ts';

export type Vec = [number, number, number];

/** A face is floor-like when its visible (interior) side faces within 45 degrees of up. */
export const FLOOR_COS = Math.cos(Math.PI / 4);

export function faceCentroid(positions: Float64Array, indices: Uint32Array, f: number): Vec {
  const out: Vec = [0, 0, 0];
  for (let k = 0; k < 3; k++) {
    const v = 3 * indices[3 * f + k];
    out[0] += positions[v] / 3;
    out[1] += positions[v + 1] / 3;
    out[2] += positions[v + 2] / 3;
  }
  return out;
}

/**
 * True when face `f` can take a placement: faces point out of the room, so its interior side
 * faces up when the outward normal points down within 45 degrees of vertical.
 */
export function isFloorLike(normal: Vec): boolean {
  return -normal[2] >= FLOOR_COS;
}

/**
 * Where the ray (origin, direction) meets face `f`'s plane, computed on the project's f64
 * positions. The height is then taken from the plane equation at that x and y, so a level floor
 * gives its own z exactly (its normal's x and y are exactly 0). Null when the ray is parallel
 * to the plane, the face is degenerate, or the plane is behind the origin.
 */
export function rayOnFacePlane(positions: Float64Array, indices: Uint32Array, f: number, origin: Vec, dir: Vec): Vec | null {
  const n = faceNormalOf(positions, indices, f);
  const denom = n[0] * dir[0] + n[1] * dir[1] + n[2] * dir[2];
  if (n[0] === 0 && n[1] === 0 && n[2] === 0) return null;
  if (Math.abs(denom) < 1e-12) return null;
  const a = 3 * indices[3 * f];
  const ax = positions[a];
  const ay = positions[a + 1];
  const az = positions[a + 2];
  const t = (n[0] * (ax - origin[0]) + n[1] * (ay - origin[1]) + n[2] * (az - origin[2])) / denom;
  if (!(t >= 0)) return null;
  const x = origin[0] + t * dir[0];
  const y = origin[1] + t * dir[1];
  let z = origin[2] + t * dir[2];
  if (Math.abs(n[2]) > 1e-9) z = az - (n[0] * (x - ax) + n[1] * (y - ay)) / n[2];
  return [x, y, z];
}

/** Rounds to the millimetre (a placement never claims more than that). */
export function toMillimetre(v: number): number {
  return Math.round(v * 1000) / 1000 + 0;
}

/**
 * The point a placement click writes: the floor point lifted by `lift` (actions.ts
 * `PLACE_HEIGHT_M`: 1.2 m for a receiver, 1.5 m for a source), to the millimetre.
 */
export function placementPoint(floor: Vec, lift: number): Vec {
  return [toMillimetre(floor[0]), toMillimetre(floor[1]), toMillimetre(floor[2] + lift)];
}

export interface Box {
  min: Vec;
  max: Vec;
}

/** The bounding box of the vertices the faces use (as the model check reports it), or null. */
export function faceBounds(positions: Float64Array, indices: Uint32Array): Box | null {
  const min: Vec = [Infinity, Infinity, Infinity];
  const max: Vec = [-Infinity, -Infinity, -Infinity];
  const nv = positions.length / 3;
  for (let i = 0; i < indices.length; i++) {
    const v = indices[i];
    if (v >= nv) continue;
    for (let k = 0; k < 3; k++) {
      const x = positions[3 * v + k];
      if (!Number.isFinite(x)) continue;
      if (x < min[k]) min[k] = x;
      if (x > max[k]) max[k] = x;
    }
  }
  return min.every((x, k) => x <= max[k]) ? { min, max } : null;
}

/** A length in metres for a label: at most two decimals, no trailing zeros. */
export function formatMetres(v: number): string {
  if (!Number.isFinite(v)) return '—';
  const r = Math.round(v * 100) / 100;
  return (r === 0 ? 0 : r).toString();
}

/** The plan label's geometry fact: extents in x and y, e.g. "10 × 6 m". */
export function planDimensions(box: Box): string {
  return `${formatMetres(box.max[0] - box.min[0])} × ${formatMetres(box.max[1] - box.min[1])} m`;
}

/**
 * An orthographic frustum (half extents about the box centre) that shows a `w` by `h` plan
 * with a margin, stretched on one axis to the target's aspect so nothing is distorted.
 */
export function fitOrtho(w: number, h: number, aspect: number, margin = 1.08): { halfW: number; halfH: number } {
  let halfW = Math.max(w, 1e-3) * 0.5 * margin;
  let halfH = Math.max(h, 1e-3) * 0.5 * margin;
  const a = aspect > 0 && Number.isFinite(aspect) ? aspect : 1;
  if (halfW / halfH < a) halfW = halfH * a;
  else halfH = halfW / a;
  return { halfW, halfH };
}

export interface GizmoAxis {
  axis: 'x' | 'y' | 'z';
  /** Screen offset of the axis tip, pixels, y down. */
  dx: number;
  dy: number;
  /** View-space depth of the unit axis: negative points away from the viewer. */
  depth: number;
}

/**
 * The world axes as a camera sees them. `view` is the camera's column-major world-to-view
 * matrix (three.js `matrixWorldInverse.elements`); world axis i maps to its column i.
 */
export function gizmoAxes(view: ArrayLike<number>, length: number): GizmoAxis[] {
  return (['x', 'y', 'z'] as const).map((axis, i) => ({
    axis,
    dx: view[4 * i] * length + 0,
    dy: -view[4 * i + 1] * length + 0,
    depth: view[4 * i + 2] + 0,
  }));
}
