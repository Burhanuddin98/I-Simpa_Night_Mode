// Ground contact (the UI study's 3D layer 8): a plane just under the room with a fine grey grid that
// fades out away from it, and a soft dark shadow under its footprint, so the room sits on something
// instead of floating in black. One transparent plane drawn by its own small shader. A drawing aid
// with no acoustic meaning: grey, never the red of the sound-level planes, and not pickable (picking
// runs on the model's BVH, not on the scene).
import type { Box } from './geometry.ts';

/** The grid's spacings, metres; the one used gives 10 to 25 cells across the room's longest side. */
const STEPS = [0.5, 1, 2, 5, 10, 20];

export interface GroundLayout {
  /** Height of the plane: just under the model's lowest point. */
  z: number;
  /** The footprint's centre, and the plane's half-size from it. */
  centre: [number, number];
  half: number;
  /** Grid spacing, metres. */
  step: number;
  /** The shadow's soft edge, metres. */
  soft: number;
}

export function gridStep(longest: number): number {
  return STEPS.find((s) => longest / s <= 25) ?? STEPS[STEPS.length - 1];
}

export function groundLayout(box: Box): GroundLayout {
  const dx = box.max[0] - box.min[0];
  const dy = box.max[1] - box.min[1];
  const longest = Math.max(dx, dy, box.max[2] - box.min[2]);
  return {
    z: box.min[2] - 0.002 * longest,
    centre: [(box.min[0] + box.max[0]) / 2, (box.min[1] + box.max[1]) / 2],
    half: 1.6 * Math.max(dx, dy),
    step: gridStep(longest),
    soft: 0.06 * Math.max(dx, dy),
  };
}

export const GROUND_VERTEX = /* glsl */ `
varying vec2 vXY;
void main() {
  vec4 world = modelMatrix * vec4(position, 1.0);
  vXY = world.xy;
  gl_Position = projectionMatrix * viewMatrix * world;
}`;

/**
 * Grid lines one pixel wide at any distance (fwidth), faded out from 55 % to 100 % of the half-size;
 * the shadow a soft box over the footprint (a signed distance, smoothed by `uSoft`).
 */
export const GROUND_FRAGMENT = /* glsl */ `
uniform vec2 uCentre;
uniform float uHalf;
uniform float uStep;
uniform vec2 uFootMin;
uniform vec2 uFootMax;
uniform float uSoft;
uniform vec3 uLine;
uniform float uLineA;
uniform float uShadowA;
varying vec2 vXY;
void main() {
  vec2 cell = vXY / uStep;
  vec2 g = abs(fract(cell - 0.5) - 0.5) / fwidth(cell);
  float line = 1.0 - min(min(g.x, g.y), 1.0);
  float r = length(vXY - uCentre) / uHalf;
  float a = line * uLineA * (1.0 - smoothstep(0.55, 1.0, r));
  vec2 q = abs(vXY - 0.5 * (uFootMin + uFootMax)) - 0.5 * (uFootMax - uFootMin);
  float sd = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0);
  float s = uShadowA * (1.0 - smoothstep(-uSoft, 1.5 * uSoft, sd));
  float alpha = a + s * (1.0 - a);
  if (alpha <= 0.0) discard;
  gl_FragColor = vec4(uLine * a / alpha, alpha);
}`;
