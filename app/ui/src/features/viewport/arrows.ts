// Parity M30: a directional source's arrow in the 3D view, as upstream draws one from a unidirectional
// or a balloon source (`e_scene_sources_source.h:153-169`): along the direction the source emits, the
// direction SPPS normalises and launches its particles along (`sppsNantes.cpp:82-83`). Upstream draws
// no arrow on a point receiver (its call is commented out, `e_scene_recepteursp_recepteur.h:173`), and
// neither does this. A pure module, tested by arrows.test.ts.
import type { Box, Vec } from './geometry.ts';

/** The arrow's length: 8 % of the room's longest side, at least 0.3 m and at most 3 m. */
export function arrowLength(box: Box | null): number {
  if (!box) return 0.5;
  const longest = Math.max(box.max[0] - box.min[0], box.max[1] - box.min[1], box.max[2] - box.min[2]);
  return Math.min(3, Math.max(0.3, 0.08 * longest));
}

const norm = (v: Vec) => Math.hypot(v[0], v[1], v[2]);
const scale = (v: Vec, k: number): Vec => [v[0] * k, v[1] * k, v[2] * k];
const add = (a: Vec, b: Vec): Vec => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const cross = (a: Vec, b: Vec): Vec => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];

/**
 * The arrow from `origin` along `direction` (any length but zero), `length` long: its shaft and four
 * barbs at the tip, each a quarter of the length back and a tenth out, in two planes at right angles,
 * so it reads as an arrow from any side. Segments of six numbers each; empty for a zero or non-finite
 * direction.
 */
export function arrowSegments(origin: Vec, direction: Vec, length: number): number[] {
  const n = norm(direction);
  if (!(n > 0) || !Number.isFinite(n) || !origin.every(Number.isFinite)) return [];
  const d = scale(direction, 1 / n);
  const tip = add(origin, scale(d, length));
  // Two directions at right angles to d: across it from the axis it is least along.
  const helper: Vec = Math.abs(d[2]) < 0.9 ? [0, 0, 1] : [1, 0, 0];
  const u0 = cross(d, helper);
  const u = scale(u0, 1 / norm(u0));
  const v = cross(d, u);
  const back = add(tip, scale(d, -0.25 * length));
  const out: number[] = [...origin, ...tip];
  for (const side of [u, scale(u, -1), v, scale(v, -1)]) out.push(...tip, ...add(back, scale(side, 0.1 * length)));
  return out;
}
