// Distance fade (the UI study's 3D layer 6): the far side of the room sinks toward the background, so
// near and far separate at a glance. Done in the surfaces' and edges' own shaders, by depth from the
// camera across the model's bounding sphere: nothing at its near side, `FADE_MAX` at its far side.
// Surfaces mix toward the page's background colour; the edges, already see-through, lose opacity.
// Only those two layers: the selection, markers, planes and the Results step's map are never faded
// (the map's colours are measurements). Off in the plan view and its inset, where depth is height.

import { T } from './tsl.ts';

const { positionView, smoothstep } = T;

/** How far the far side goes toward the background (surfaces) or toward clear (edges). */
export const FADE_MAX = 0.6;
/** The page's background (theme.css --bg), which the canvas shows through. */
export const FADE_BG = '#0c0a0a';

/** The depths, from the camera, where the fade starts and ends: the model's bounding sphere's near and far side. */
export function fadeRange(cameraToCentre: number, radius: number): { near: number; far: number } {
  return { near: Math.max(0, cameraToCentre - radius), far: cameraToCentre + radius };
}

/** The fade's uniforms (engine.ts sets them before each render). */
export interface FadeUniforms {
  near: any;
  far: any;
  max: any;
}

/**
 * How far a fragment goes toward the background (surfaces: mix toward `FADE_BG`) or toward clear (edges:
 * its opacity times 1 minus this): `max` times its depth's place between `near` and `far`. `max` 0 is off.
 */
export function fadeAmount(u: FadeUniforms): any {
  return u.max.mul(smoothstep(u.near, u.far, positionView.z.negate()));
}
