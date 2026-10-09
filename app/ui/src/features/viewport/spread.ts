// Results spreading from the source (Burhan's 10-05 UI list, item 10):
//   - A surface map, when it opens, is revealed from the sources outward over REVEAL_MS: a mask on the
//     map's fragments (a fragment farther from every source than the front is not drawn yet), never a
//     change of a texel or of a colour. Once the front has passed the farthest face the mask is off and
//     the map draws exactly as before; the read-back hooks read the texture, which the mask never touches.
//   - While particle playback plays, a wavefront: a sphere of radius c·t about each source, t the
//     playback's time since the emission and c the project's speed of sound from its temperature (the
//     solver's own rule, `343.2·√(T/293.15)`, `Celerite_du_son.cpp:46`), so it runs through the direct sound's
//     particles. Drawn as a faint rim, clipped to
//     the room's box, labelled with nothing.
// Neither moves under prefers-reduced-motion or WebDriver (the gates compare frames): the map shows at
// once and no wavefront is drawn. Pure functions tested by spread.test.ts under `node --test`; the
// shader nodes at the end are the GPU side (resultsLayer.ts, engine.ts).
import { T } from './tsl.ts';
import { easeOut } from './build.ts';

const { abs, Break, dot, float, Fn, If, Loop, min, normalView, positionViewDirection, positionWorld, pow, sqrt } = T;

/** The reveal's length, ms. */
export const REVEAL_MS = 1400;
/** The reveal's front when no reveal runs: past anything, so the mask keeps every fragment. */
export const NO_REVEAL = 1e9;
/** At most this many sources reveal and send a wavefront (the shader's array length). */
export const SPREAD_MAX = 16;

type V3 = readonly [number, number, number] | readonly number[];

/** The speed of sound at `temperatureC`, m/s, as the solver derives it; null for a temperature that is not a number. */
export function speedOfSound(temperatureC: unknown): number | null {
  const t = typeof temperatureC === 'number' ? temperatureC : typeof temperatureC === 'string' && temperatureC.trim() !== '' ? Number(temperatureC) : NaN;
  if (!Number.isFinite(t) || t <= -273.15) return null;
  return 343.2 * Math.sqrt((t + 273.15) / 293.15);
}

/** The front's distance from the sources `elapsedMs` into a reveal reaching `reach` metres; NO_REVEAL once done. */
export function revealRadius(elapsedMs: number, reach: number): number {
  const f = elapsedMs / REVEAL_MS;
  if (f >= 1 || !(reach > 0)) return NO_REVEAL;
  return easeOut(Math.max(0, f)) * reach * 1.01;
}

/** How far the front must travel: the farthest of `points` (x, y, z flat) from its nearest source. */
export function revealReach(points: ArrayLike<number>, sources: readonly V3[]): number {
  if (sources.length === 0) return 0;
  let far = 0;
  for (let i = 0; i + 2 < points.length; i += 3) {
    let near = Infinity;
    for (const s of sources) near = Math.min(near, Math.hypot(points[i] - s[0], points[i + 1] - s[1], points[i + 2] - s[2]));
    if (near > far) far = near;
  }
  return far;
}

/**
 * The wavefront's radius at playback time `t` (fractional steps) for a run of time step `dtS` seconds whose
 * sources emit at step `start`: c·dtS·(t − start + 1), metres, since a saved record is where its particle is at
 * the end of its step (CR4: every direct particle of step 4 lies 1.716 m = 343.2 m/s × 5 ms from its source);
 * null before the emission or without a time step or c.
 */
export function wavefrontRadius(t: number, start: number, dtS: number | null, c: number | null): number | null {
  if (!dtS || !(dtS > 0) || !c || !(c > 0) || !(t >= start)) return null;
  return c * dtS * (t - start + 1);
}

/** Whether the wavefront about `source` of radius `r` still meets the room's box (min, max): not once it encloses it. */
export function wavefrontInRoom(source: V3, r: number, min: V3, max: V3): boolean {
  let far = 0;
  for (const x of [min[0], max[0]]) for (const y of [min[1], max[1]]) for (const z of [min[2], max[2]]) far = Math.max(far, Math.hypot(x - source[0], y - source[1], z - source[2]));
  return r < far;
}

/** The reveal's mask (a fragment's keep test): within `radius` of one of the first `count` of `pos`, or no source. */
export function revealKeep(pos: any, count: any, radius: any): any {
  return Fn(() => {
    const near = float(1e12).toVar();
    Loop(SPREAD_MAX, ({ i }: { i: any }) => {
      If(i.greaterThanEqual(count), () => {
        Break();
      });
      const d = positionWorld.sub(pos.element(i));
      near.assign(min(near, dot(d, d)));
    });
    return count.equal(0).or(sqrt(near).lessThanEqual(radius));
  })();
}

/** The wavefront's opacity: a rim, bright where the sphere is seen edge-on and faint face-on, so it reads as a ring. */
export function rimOpacity(peak: number): any {
  const facing = abs(dot(normalView, positionViewDirection));
  return pow(float(1).sub(facing), 2.5).mul(peak).add(float(0.04));
}

/** The room's box as a mask: a fragment outside it is not drawn. */
export function insideBox(lo: any, hi: any): any {
  const p = positionWorld;
  return p.x.greaterThanEqual(lo.x).and(p.y.greaterThanEqual(lo.y)).and(p.z.greaterThanEqual(lo.z)).and(p.x.lessThanEqual(hi.x)).and(p.y.lessThanEqual(hi.y)).and(p.z.lessThanEqual(hi.z));
}
