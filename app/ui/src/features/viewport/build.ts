// The room builds itself when it opens (the UI study's motion list: "Import assembles the hall in
// building order ... the ceiling settles last", his "how things get built up"): a cut rises from the
// floor to the ceiling over `BUILD_MS` (6 s), the surfaces and edges above it not yet drawn, a faint warm
// line where it cuts (not red: red is for actions and selection), while the camera swings `SWING_DEG`
// into its framing. Only when a new model loads; never under prefers-reduced-motion or WebDriver.

/** Burhan 04:08: "can you make this sort of animation last a bit longer, like 6 seconds". */
export const BUILD_MS = 6000;
export const SWING_DEG = 25;
/** The cut's height when no build is running: above anything. */
export const NO_CUT = 1e9;

/** Eased progress: a gentle start and finish, an even rise between (at 6 s a fast start would stall half way). */
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

const anchor = (src: string, at: string, what: string): void => {
  if (!src.includes(at)) throw new Error(`build: the ${what} shader has no '${at}' (three.js changed its chunks)`);
};

/**
 * The shaders with the cut: fragments above `nmBuildZ` discarded, and for surfaces a faint warm line
 * `nmBuildBand` deep just under the cut.
 */
export function withBuild(vertex: string, fragment: string, mode: 'surface' | 'line'): { vertex: string; fragment: string } {
  for (const [src, what] of [[vertex, 'vertex'], [fragment, 'fragment']] as const) anchor(src, '#include <common>', what);
  anchor(vertex, '#include <project_vertex>', 'vertex');
  anchor(fragment, '#include <opaque_fragment>', 'fragment');
  const line =
    mode === 'surface'
      ? '\n\toutgoingLight += vec3(1.0, 0.86, 0.78) * 0.35 * (1.0 - smoothstep(0.0, nmBuildBand, nmBuildZ - vNmWorldZ));'
      : '';
  return {
    vertex: vertex
      .replace('#include <common>', '#include <common>\nvarying float vNmWorldZ;')
      .replace('#include <project_vertex>', '#include <project_vertex>\n\tvNmWorldZ = (modelMatrix * vec4(transformed, 1.0)).z;'),
    fragment: fragment
      .replace('#include <common>', '#include <common>\nvarying float vNmWorldZ;\nuniform float nmBuildZ;\nuniform float nmBuildBand;')
      .replace('#include <opaque_fragment>', `if (vNmWorldZ > nmBuildZ) discard;${line}\n\t#include <opaque_fragment>`),
  };
}
