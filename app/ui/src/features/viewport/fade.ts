// Distance fade (the UI study's 3D layer 6): the far side of the room sinks toward the background, so
// near and far separate at a glance. Done in the surfaces' and edges' own shaders, by depth from the
// camera across the model's bounding sphere: nothing at its near side, `FADE_MAX` at its far side.
// Surfaces mix toward the page's background colour; the edges, already see-through, lose opacity.
// Only those two layers: the selection, markers, planes and the Results step's map are never faded
// (the map's colours are measurements). Off in the plan view and its inset, where depth is height.

/** How far the far side goes toward the background (surfaces) or toward clear (edges). */
export const FADE_MAX = 0.6;
/** The page's background (theme.css --bg), which the canvas shows through. */
export const FADE_BG = '#09090b';

/** The depths, from the camera, where the fade starts and ends: the model's bounding sphere's near and far side. */
export function fadeRange(cameraToCentre: number, radius: number): { near: number; far: number } {
  return { near: Math.max(0, cameraToCentre - radius), far: cameraToCentre + radius };
}

const anchor = (src: string, at: string, what: string): void => {
  if (!src.includes(at)) throw new Error(`fade: the ${what} shader has no '${at}' (three.js changed its chunks)`);
};

/**
 * The shaders with the fade added just before the colour is written: `mix` toward `nmFadeColor` for
 * opaque surfaces, `alpha` for lines drawn see-through. `nmFadeMax` 0 turns it off.
 */
export function withFade(vertex: string, fragment: string, mode: 'mix' | 'alpha'): { vertex: string; fragment: string } {
  for (const [src, what] of [[vertex, 'vertex'], [fragment, 'fragment']] as const) anchor(src, '#include <common>', what);
  anchor(vertex, '#include <project_vertex>', 'vertex');
  anchor(fragment, '#include <opaque_fragment>', 'fragment');
  const amount = 'nmFadeMax * smoothstep(nmFadeNear, nmFadeFar, vNmDepth)';
  const apply = mode === 'mix' ? `outgoingLight = mix(outgoingLight, nmFadeColor, ${amount});` : `diffuseColor.a *= 1.0 - ${amount};`;
  return {
    vertex: vertex
      .replace('#include <common>', '#include <common>\nvarying float vNmDepth;')
      .replace('#include <project_vertex>', '#include <project_vertex>\n\tvNmDepth = -mvPosition.z;'),
    fragment: fragment
      .replace(
        '#include <common>',
        '#include <common>\nvarying float vNmDepth;\nuniform float nmFadeNear;\nuniform float nmFadeFar;\nuniform float nmFadeMax;\nuniform vec3 nmFadeColor;',
      )
      .replace('#include <opaque_fragment>', `${apply}\n\t#include <opaque_fragment>`),
  };
}
