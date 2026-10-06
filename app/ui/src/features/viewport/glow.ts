// The sources' glow (Burhan 2026-10-06 01:25: "like they were this glowing bulb, and they shot this red
// sort of glow onto the rest of the geometry, lighting it up"). The surfaces are drawn with a matcap,
// which ignores lights, so the glow is a term added to the faces' colour in their shaders: a soft pool
// of warm red around each enabled source, falling off with distance. No shadows and no walls in the
// way: it marks where a source is, it does not simulate light. Kept apart from the selection's red by
// its shape (a fall-off, never a flat wash) and a warmer hue. It pulses slowly, except under
// prefers-reduced-motion or WebDriver (the gates compare frames), where it holds still at mid pulse.
import { T } from './tsl.ts';
import type { Box } from './geometry.ts';

const { Break, dot, exp, float, Fn, If, Loop, min, positionWorld } = T;

/** At most this many sources glow (the shader's array length); the first ones in the project's order. */
export const GLOW_MAX = 16;
/** The glow's colour, linear RGB: a warmer red than the selection's #E0202E. */
export const GLOW_RGB = [1, 0.22, 0.08] as const;
/** The added light at a source's own position, at full pulse. */
export const GLOW_PEAK = 0.42;
/** One full pulse, ms. */
export const PULSE_MS = 2600;
/** The pulse's phase when nothing may move: the sprite at its drawn size. */
export const STILL_PHASE = 0.5;

/** The glow's 1/e radius, metres: a tenth of the room's longest side, between 1.5 and 8 m. */
export function glowRadius(box: Box | null): number {
  if (!box) return 1.5;
  const longest = Math.max(box.max[0] - box.min[0], box.max[1] - box.min[1], box.max[2] - box.min[2]);
  return Math.min(8, Math.max(1.5, 0.1 * longest));
}

/** The pulse's phase at `ms`, 0 (dim) to 1 (bright), eased: a cosine. */
export function pulsePhase(ms: number): number {
  return 0.5 - 0.5 * Math.cos((2 * Math.PI * ms) / PULSE_MS);
}

/** The glow's strength at a phase: 75 % to 100 % of the peak, so it never goes out. */
export function glowLevel(phase: number): number {
  return GLOW_PEAK * (0.75 + 0.25 * phase);
}

/** The source sprite's size factor at a phase: 0.94 to 1.06, exactly 1 at `STILL_PHASE`. */
export function spriteScale(phase: number): number {
  return 0.94 + 0.12 * phase;
}

/** The glow's uniforms, shared by every surface material (engine.ts). */
export interface GlowUniforms {
  /** GLOW_MAX source positions, the first `count` used. */
  pos: any;
  count: any;
  radius: any;
  level: any;
  color: any;
}

/**
 * The light the glow adds to a face's colour before it is written: the sum of exp(-d^2 / r^2) over
 * the sources (capped at 1.5), times the colour and the pulse's level. d is from the fragment's world
 * position, as before.
 */
export function glowTerm(u: GlowUniforms): any {
  return Fn(() => {
    const sum = float(0).toVar();
    Loop(GLOW_MAX, ({ i }: { i: any }) => {
      If(i.greaterThanEqual(u.count), () => {
        Break();
      });
      const d = positionWorld.sub(u.pos.element(i));
      sum.addAssign(exp(dot(d, d).negate().div(u.radius.mul(u.radius))));
    });
    return u.color.mul(u.level).mul(min(sum, 1.5));
  })();
}
