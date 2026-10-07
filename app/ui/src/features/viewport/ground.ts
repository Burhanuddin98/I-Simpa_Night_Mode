// Ground contact (the UI study's 3D layer 8): a plane just under the room with a fine grey grid that
// fades out away from it, and a soft dark shadow under its footprint, so the room sits on something
// instead of floating in black. One transparent plane drawn by its own small shader. A drawing aid
// with no acoustic meaning: grey, never the red of the sound-level planes, and not pickable (picking
// runs on the model's BVH, not on the scene).
import { T } from './tsl.ts';
import type { Box } from './geometry.ts';

const { abs, Discard, float, Fn, fract, fwidth, length, max, min, positionWorld, smoothstep, vec4 } = T;

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

/** The ground's uniforms (engine.ts lays them out per model). */
export interface GroundUniforms {
  centre: any;
  half: any;
  step: any;
  footMin: any;
  footMax: any;
  soft: any;
  /** The grid's colour, written raw as the WebGL ShaderMaterial wrote it. */
  line: any;
  lineA: any;
  shadowA: any;
}

/**
 * The ground's colour (a `fragmentNode`, raw, as the ShaderMaterial wrote it): grid lines one pixel wide
 * at any distance (fwidth), faded out from 55 % to 100 % of the half-size; the shadow a soft box over the
 * footprint (a signed distance, smoothed by `soft`).
 */
export function groundColour(u: GroundUniforms): any {
  return Fn(() => {
    const xy = positionWorld.xy;
    const cell = xy.div(u.step);
    const g = abs(fract(cell.sub(0.5)).sub(0.5)).div(fwidth(cell));
    const line = float(1).sub(min(min(g.x, g.y), 1));
    const r = length(xy.sub(u.centre)).div(u.half);
    const a = line.mul(u.lineA).mul(float(1).sub(smoothstep(0.55, 1, r)));
    const q = abs(xy.sub(u.footMin.add(u.footMax).mul(0.5))).sub(u.footMax.sub(u.footMin).mul(0.5));
    const sd = length(max(q, 0)).add(min(max(q.x, q.y), 0));
    const s = u.shadowA.mul(float(1).sub(smoothstep(u.soft.negate(), u.soft.mul(1.5), sd)));
    const alpha = a.add(s.mul(float(1).sub(a))).toVar();
    Discard(alpha.lessThanEqual(0));
    return vec4(u.line.mul(a).div(alpha), alpha);
  })();
}
