// Corner shading: ambient occlusion baked per vertex when a model opens (the UI study's 3D layer 5).
// From each corner of each face, a fixed fan of short rays goes into the room; the share that meets
// another surface within `aoReach` darkens that corner. Corners, the floor under a balcony and stair
// steps go darker, so the room reads in depth. Computed once on the picking BVH, no cost per frame,
// and deterministic (the same fan every time), so the gates' frames stay comparable.
//
// A face's stored normal points out of the room (the faces are drawn BackSide, seen from inside), so
// the rays go along -n. Per vertex: a wall of two large triangles is shaded by its four corners alone,
// so a coarse room darkens a little everywhere, not only in its corners.
import { T } from './tsl.ts';
import type { Box, Vec } from './geometry.ts';

const { attribute, float, mix } = T;

/** Rays per sample point. */
export const AO_RAYS = 16;
/** How dark a fully enclosed point goes: its colour times 1 - AO_STRENGTH. */
export const AO_STRENGTH = 0.75;

/** How far a ray looks, metres: a twelfth of the room's longest side, between 0.5 and 4 m. */
export function aoReach(box: Box | null): number {
  if (!box) return 0.5;
  const longest = Math.max(box.max[0] - box.min[0], box.max[1] - box.min[1], box.max[2] - box.min[2]);
  return Math.min(4, Math.max(0.5, longest / 12));
}

/** `n` cosine-weighted directions over the +z hemisphere, on a Fibonacci spiral (fixed, not random). */
export function hemisphere(n = AO_RAYS): Vec[] {
  const golden = Math.PI * (3 - Math.sqrt(5));
  return Array.from({ length: n }, (_, i) => {
    const u = (i + 0.5) / n;
    const r = Math.sqrt(u);
    return [r * Math.cos(i * golden), r * Math.sin(i * golden), Math.sqrt(1 - u)] as Vec;
  });
}

/** Whether a ray from `origin` along unit `dir` meets a surface within `far`. */
export type HitTest = (origin: Vec, dir: Vec, far: number) => boolean;

/**
 * One value per vertex of the non-indexed `positions` (9 numbers a face): 1 in the open, down to
 * 1 - AO_STRENGTH where every ray is stopped. `index` (3 a face, the project's vertex numbers) lets
 * coplanar faces that share a vertex share its rays. Faces `from` to `to` only, into `out`, with
 * `done` carried between calls: the engine bakes a hall in slices across frames (Elmia, 7860 faces,
 * took 0.58 s in one piece on Grace).
 */
export function bakeAo(
  positions: ArrayLike<number>,
  index: ArrayLike<number>,
  hit: HitTest,
  reach: number,
  from = 0,
  to = Infinity,
  out = new Float32Array(Math.floor(positions.length / 9) * 3).fill(1),
  done = new Map<string, number>(),
): Float32Array {
  const faces = Math.min(to, Math.floor(positions.length / 9));
  const fan = hemisphere();
  const lift = 0.01 * reach;
  for (let f = from; f < faces; f++) {
    const p = (k: number): Vec => [positions[9 * f + 3 * k], positions[9 * f + 3 * k + 1], positions[9 * f + 3 * k + 2]];
    const [a, b, c] = [p(0), p(1), p(2)];
    const e1 = sub(b, a);
    const e2 = sub(c, a);
    const n = cross(e1, e2);
    const len = Math.hypot(...n);
    if (!(len > 0)) {
      out.fill(1, 3 * f, 3 * f + 3);
      continue;
    }
    const inward: Vec = [-n[0] / len, -n[1] / len, -n[2] / len];
    const t = norm(cross(inward, Math.abs(inward[2]) < 0.9 ? [0, 0, 1] : [1, 0, 0]));
    const s = cross(inward, t);
    const centroid: Vec = [(a[0] + b[0] + c[0]) / 3, (a[1] + b[1] + c[1]) / 3, (a[2] + b[2] + c[2]) / 3];
    for (let k = 0; k < 3; k++) {
      const key = `${index[3 * f + k]}:${inward.map((x) => Math.round(x * 100)).join(',')}`;
      let v = done.get(key);
      if (v === undefined) {
        const q = p(k);
        // 5 % toward the centroid and a little into the room, so the fan does not start on an edge.
        const o: Vec = [0, 1, 2].map((i) => q[i] + 0.05 * (centroid[i] - q[i]) + lift * inward[i]) as Vec;
        let blocked = 0;
        for (const d of fan) {
          const dir: Vec = [0, 1, 2].map((i) => d[0] * t[i] + d[1] * s[i] + d[2] * inward[i]) as Vec;
          if (hit(o, dir, reach)) blocked++;
        }
        v = 1 - (AO_STRENGTH * blocked) / fan.length;
        done.set(key, v);
      }
      out[3 * f + k] = v;
    }
  }
  return out;
}

const sub = (a: Vec, b: Vec): Vec => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const cross = (a: Vec, b: Vec): Vec => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm = (a: Vec): Vec => {
  const l = Math.hypot(...a);
  return [a[0] / l, a[1] / l, a[2] / l];
};

/**
 * The factor the faces' colour is multiplied by before it is written: the baked value (attribute
 * `nmAo`) by `aoMix` (1 on, 0 off: the Style menu's "Darker corners"). TSL, so both backends run it.
 */
export function aoFactor(aoMix: unknown): any {
  return mix(float(1), attribute('nmAo', 'float'), aoMix as number);
}
