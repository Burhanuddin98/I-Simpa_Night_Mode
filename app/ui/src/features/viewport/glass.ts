// The glass case (the UI study's 3D layer 3): in See-through, a near wall seen edge-on is drawn more
// solid and one seen face-on clearer, the way a glass case shows its edges and lets you look through
// its panes. The See-through slider's opacity is scaled by the view angle: `FACE_ON` times it looking
// straight at a wall, `EDGE_ON` times it at a grazing angle, never past `CAP`. 0 on the slider is
// still invisible.

import { T } from './tsl.ts';

const { abs, dot, float, min, mix, normalView, positionViewDirection, pow } = T;

export const FACE_ON = 0.4;
export const EDGE_ON = 2.5;
export const CAP = 0.85;

/** The opacity factor at the cosine between the view and the face normal (1 face-on, 0 edge-on), before the cap. */
export function glassFactor(cosView: number): number {
  const f = (1 - Math.abs(cosView)) ** 2;
  return FACE_ON + (EDGE_ON - FACE_ON) * f;
}

/** The see-through layer's opacity (a material's `opacityNode`): `opacity` scaled by the view angle (`glassFactor`), capped at `CAP`. */
export function glassOpacity(opacity: any): any {
  const facing = float(1).sub(abs(dot(normalView, positionViewDirection)));
  return min(CAP, opacity.mul(mix(FACE_ON, EDGE_ON, pow(facing, 2))));
}
