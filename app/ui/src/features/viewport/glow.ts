// The sources' glow (Burhan 2026-10-06 01:25: "like they were this glowing bulb, and they shot this red
// sort of glow onto the rest of the geometry, lighting it up"). The surfaces are drawn with a matcap,
// which ignores lights, so the glow is a term added to the faces' colour in their shaders: a soft pool
// of warm red around each enabled source, falling off with distance. No shadows and no walls in the
// way: it marks where a source is, it does not simulate light. Kept apart from the selection's red by
// its shape (a fall-off, never a flat wash) and a warmer hue. It pulses slowly, except under
// prefers-reduced-motion or WebDriver (the gates compare frames), where it holds still at mid pulse.
import type { Box } from './geometry.ts';

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

const anchor = (src: string, at: string, what: string): void => {
  if (!src.includes(at)) throw new Error(`glow: the ${what} shader has no '${at}' (three.js changed its chunks)`);
};

/**
 * The faces' shaders with the glow added: the world position passed from the vertex stage, and before
 * the colour is written, the sum of exp(-d^2 / r^2) over the sources (capped at 1.5) times the colour.
 * Throws when an anchor is missing, so a three.js upgrade that moves them fails a test, not silently.
 */
export function withGlow(vertex: string, fragment: string): { vertex: string; fragment: string } {
  for (const [src, what] of [[vertex, 'vertex'], [fragment, 'fragment']] as const) anchor(src, '#include <common>', what);
  anchor(vertex, '#include <project_vertex>', 'vertex');
  anchor(fragment, '#include <opaque_fragment>', 'fragment');
  return {
    vertex: vertex
      .replace('#include <common>', '#include <common>\nvarying vec3 vGlowWorld;')
      .replace('#include <project_vertex>', '#include <project_vertex>\n\tvGlowWorld = (modelMatrix * vec4(transformed, 1.0)).xyz;'),
    fragment: fragment
      .replace(
        '#include <common>',
        `#include <common>
varying vec3 vGlowWorld;
uniform vec3 glowPos[${GLOW_MAX}];
uniform int glowCount;
uniform float glowRadius;
uniform float glowLevel;
uniform vec3 glowColor;`,
      )
      .replace(
        '#include <opaque_fragment>',
        `float glowSum = 0.0;
	for (int i = 0; i < ${GLOW_MAX}; i++) {
		if (i >= glowCount) break;
		vec3 glowD = vGlowWorld - glowPos[i];
		glowSum += exp(-dot(glowD, glowD) / (glowRadius * glowRadius));
	}
	outgoingLight += glowColor * glowLevel * min(glowSum, 1.5);
	#include <opaque_fragment>`,
      ),
  };
}
