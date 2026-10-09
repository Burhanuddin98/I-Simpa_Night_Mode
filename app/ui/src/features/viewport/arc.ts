// Camera moves that arc (Burhan's 10-05 UI list, item 6): framing, the view presets, a focus on the
// selection and the recorded tour's flights travel around the target, not on a straight line through
// the model. The target moves on a straight line; the camera's offset from it turns on a great circle
// (its direction slerped) while its length changes geometrically, so the camera orbits the moving
// target and never passes through it. A long target move pulls the camera back a little mid-flight,
// as a DCC tool's focus does, and both ends are exact. A pure module: no three.js, no DOM, tested by
// arc.test.ts under `node --test`.

export type V3 = [number, number, number];

export interface Pose {
  position: V3;
  target: V3;
}

/** A framing, preset or focus move's length, ms. */
export const ARC_MS = 650;
/** How far a long move pulls back mid-flight, as a fraction of the distance to the target. */
export const PULL_BACK = 0.3;

const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const add = (a: V3, b: V3): V3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const scale = (a: V3, k: number): V3 => [a[0] * k, a[1] * k, a[2] * k];
const dot = (a: V3, b: V3): number => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const len = (a: V3): number => Math.hypot(a[0], a[1], a[2]);
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const lerp = (a: V3, b: V3, f: number): V3 => [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f];

/** Eased progress: slow out, slow in (cubic). 0 and 1 are exact. */
export function easeInOut(f: number): number {
  const c = Math.min(1, Math.max(0, f));
  return c < 0.5 ? 4 * c * c * c : 1 - Math.pow(-2 * c + 2, 3) / 2;
}

/**
 * Unit `a` turned towards unit `b` by the fraction `f` of the angle between them, on the great circle.
 * Opposite directions turn about the vertical (z), or about x when they are vertical themselves, so a
 * half turn goes round the room, not over it.
 */
export function slerpDir(a: V3, b: V3, f: number): V3 {
  const c = Math.min(1, Math.max(-1, dot(a, b)));
  const angle = Math.acos(c);
  if (angle < 1e-9) return lerp(a, b, f);
  if (Math.PI - angle < 1e-6) {
    // The turning axis: the vertical's part perpendicular to `a` (x's when `a` is vertical).
    const up: V3 = Math.abs(a[2]) < 0.99 ? [0, 0, 1] : [1, 0, 0];
    const p = sub(up, scale(a, dot(a, up)));
    const axis = scale(p, 1 / len(p));
    // Rodrigues about `axis`, which is perpendicular to `a`.
    const t = angle * f;
    const r = add(scale(a, Math.cos(t)), scale(cross(axis, a), Math.sin(t)));
    return scale(r, 1 / len(r));
  }
  const s = Math.sin(angle);
  const r = add(scale(a, Math.sin((1 - f) * angle) / s), scale(b, Math.sin(f * angle) / s));
  return scale(r, 1 / len(r));
}

/** The pose a fraction `f` (0 to 1, already eased or not) of the way from `from` to `to`, on the arc. */
export function arcPose(from: Pose, to: Pose, f: number): Pose {
  if (f <= 0) return { position: [...from.position], target: [...from.target] };
  if (f >= 1) return { position: [...to.position], target: [...to.target] };
  const target = lerp(from.target, to.target, f);
  const o0 = sub(from.position, from.target);
  const o1 = sub(to.position, to.target);
  const d0 = len(o0);
  const d1 = len(o1);
  if (d0 < 1e-12 || d1 < 1e-12) return { position: lerp(from.position, to.position, f), target };
  const dir = slerpDir(scale(o0, 1 / d0), scale(o1, 1 / d1), f);
  // Geometric in distance, so a zoom feels even; pulled back mid-flight by how far the target travels.
  const travel = len(sub(to.target, from.target));
  const pull = 1 + PULL_BACK * Math.sin(Math.PI * f) * Math.min(1, travel / Math.max(d0, d1));
  const d = d0 * Math.pow(d1 / d0, f) * pull;
  return { position: add(target, scale(dir, d)), target };
}
