// The room builds itself when it opens (the UI study's motion list: "Import assembles the hall in
// building order ... the ceiling settles last", his "how things get built up"): a cut rises from the
// floor to the ceiling over `BUILD_MS` (2.4 s), the surfaces and edges above it not yet drawn, a faint warm
// line where it cuts (not red: red is for actions and selection), while the camera swings `SWING_DEG`
// into its framing. Only when a new model loads; never under prefers-reduced-motion or WebDriver.

import { T } from './tsl.ts';

const { float, positionWorld, smoothstep, vec3 } = T;

/** Burhan 04:08 asked for 6 s; 04:11, having seen it: "make it 2.4 seconds". */
export const BUILD_MS = 2400;
export const SWING_DEG = 25;
/** The cut's height when no build is running: above anything. */
export const NO_CUT = 1e9;

/** Eased progress: a gentle start and finish, an even rise between (a fast start front-loads the rise and leaves it crawling). */
export function easeOut(f: number): number {
  const c = Math.min(1, Math.max(0, f));
  return c * c * (3 - 2 * c);
}

/** The cut's height at progress `f` (0 to 1) through a room from `minZ` to `maxZ`; `NO_CUT` once done. */
export function buildHeight(f: number, minZ: number, maxZ: number): number {
  if (f >= 1) return NO_CUT;
  return minZ + easeOut(f) * (maxZ - minZ) * 1.02;
}

/** The camera's angle about the vertical through its target, radians, behind its framing at the start and 0 at the end. */
export function swingAngle(f: number): number {
  return -(1 - easeOut(f)) * ((SWING_DEG * Math.PI) / 180);
}

/** The cut as a mask (a material's `maskNode`): a fragment above `cutZ` is discarded. */
export function buildKeep(cutZ: any): any {
  return positionWorld.z.lessThanEqual(cutZ);
}

/** For surfaces, the faint warm line `band` deep just under the cut, added to the colour. */
export function buildLine(cutZ: any, band: any): any {
  return vec3(1, 0.86, 0.78).mul(0.35).mul(float(1).sub(smoothstep(0, band, cutZ.sub(positionWorld.z))));
}
