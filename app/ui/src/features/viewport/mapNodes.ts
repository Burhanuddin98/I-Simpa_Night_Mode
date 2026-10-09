// The Results step's GPU code in TSL (decision 68 (b)): the surface map, its read-back passes, and
// the particle playback's keep rules, written once and compiled by three.js to WGSL on WebGPU and to
// GLSL on its WebGL2 fallback. A line-for-line port of the GLSL resultsLayer.ts carried before: the
// texel arithmetic, the window, the node mean, the level and the ramps are the same operations in the
// same order, so the read-back hooks keep comparing bits with the file.
import { Vector3, type Texture } from 'three';
import { T } from './tsl';
import { COOL, HOT, rampFloats } from './mapData';
import { MAX_NODE_FACES } from './mapView';
import { finite } from './nodes';
import type { KeepOps } from './particles';
import { MAX_WINDOW_STEPS } from './window';

const { abs, Break, clamp, float, floor, Fn, fract, fwidth, If, int, ivec2, log2, Loop, max, min, mix, select, smoothstep, texture, textureLoad, uniform, uniformArray, vec2, vec3, vec4, Discard } = T;

type N = any;

/** The map's textures and layout, shared by the draw and the read-back passes (one texture, one rule). */
export interface MapUniforms {
  map: N;
  base: N;
  adj: N;
  adjWidth: N;
  width: N;
  steps: N;
  win: N;
}

/** How the map is drawn: the step, a difference or a level, smooth colour, contours and the range. */
export interface LookUniforms {
  step: N;
  diff: N;
  /** Parity R42: 1 for a parameter map, whose values are drawn as they are (s, dB, a fraction), not as levels. */
  linear: N;
  smooth: N;
  iso: N;
  lo: N;
  hi: N;
}

export function mapUniforms(placeholder: Texture): MapUniforms {
  return {
    map: texture(placeholder),
    base: texture(placeholder),
    adj: texture(placeholder),
    adjWidth: uniform(1, 'int'),
    width: uniform(1, 'int'),
    steps: uniform(1, 'int'),
    win: uniform(1, 'int'),
  };
}

export function lookUniforms(): LookUniforms {
  return { step: uniform(0, 'int'), diff: uniform(0, 'int'), linear: uniform(0, 'int'), smooth: uniform(0, 'int'), iso: uniform(0), lo: uniform(0), hi: uniform(1) };
}

/** The texel of (face, step): i = face * steps + step, in rows of `width` (mapData.ts `texelOf`). */
export function mapTexel(u: MapUniforms, t: N, face: N, step: N): N {
  const i = face.mul(u.steps).add(step);
  return textureLoad(t, ivec2(i.mod(u.width), i.div(u.width))).x;
}

/**
 * The time window (window.ts): the face's texels over steps max(0, step - win + 1) .. step, a non-finite
 * one as 0, added in step order and divided by the steps held; win 1 is the texel itself.
 */
export function faceValue(u: MapUniforms, t: N, face: N, step: N): N {
  const out = float(0).toVar();
  If(u.win.lessThanEqual(1), () => {
    out.assign(mapTexel(u, t, face, step));
  }).Else(() => {
    const from = max(int(0), step.sub(u.win).add(1)).toVar();
    const s = float(0).toVar();
    Loop(MAX_WINDOW_STEPS, ({ i }: { i: N }) => {
      const j = from.add(i).toVar();
      If(j.greaterThan(step), () => {
        Break();
      });
      const e = mapTexel(u, t, face, j).toVar();
      If(finite(e), () => {
        s.addAssign(e);
      });
    });
    out.assign(s.div(float(step.sub(from).add(1))));
  });
  return out;
}

/** Upstream's level, dB re 1e-12; the caller checks e > 0. The GLSL's own arithmetic: 10 log2(e) log10(2) + 120. */
export function levelDb(e: N): N {
  return log2(e).mul(10).mul(0.3010299956639812).add(120);
}

/** This run's level minus the baseline's at (face, step); `ok` false where either has no energy. */
export function diffDb(u: MapUniforms, face: N, step: N): { d: N; ok: N } {
  const a = faceValue(u, u.map, face, step).toVar();
  const b = faceValue(u, u.base, face, step).toVar();
  const ok = a.greaterThan(0).and(b.greaterThan(0)).and(finite(a)).and(finite(b)).toVar();
  return { ok, d: select(ok, levelDb(a).sub(levelDb(b)), float(0)) };
}

/** W5: a node's energy, upstream's mean of its linked faces' window values (mapView.ts `nodeEnergy`). */
export function nodeEnergy(u: MapUniforms, t: N, start: N, n: N, step: N): N {
  const s = float(0).toVar();
  Loop(MAX_NODE_FACES, ({ i }: { i: N }) => {
    If(i.greaterThanEqual(n), () => {
      Break();
    });
    const j = start.add(i).toVar();
    const face = int(textureLoad(u.adj, ivec2(j.mod(u.adjWidth), j.div(u.adjWidth))).x.add(0.5)).toVar();
    const e = faceValue(u, t, face, step).toVar();
    If(finite(e), () => {
      s.addAssign(e);
    });
  });
  return select(n.greaterThan(0), s.div(float(n)), float(0));
}

const vecs = (stops: readonly string[]): Vector3[] => {
  const f = rampFloats(stops);
  return stops.map((_, i) => new Vector3(f[3 * i], f[3 * i + 1], f[3 * i + 2]));
};
const HOT_STOPS = uniformArray(vecs(HOT), 'vec3');
const COOL_STOPS = uniformArray(vecs(COOL), 'vec3');

function ramp(stops: N, count: number, t: N): N {
  const u = clamp(t, 0, 1).mul(count - 1).toVar();
  const k = min(int(count - 2), int(floor(u))).toVar();
  return mix(stops.element(k), stops.element(k.add(1)), u.sub(float(k)));
}

/** The level ramp, black through red to white (decision 63), as the shader's 0-1 sRGB numbers, written raw. */
export const hot = (t: N): N => ramp(HOT_STOPS, HOT.length, t);
/** The quieter-than-baseline ramp. */
export const cool = (t: N): N => ramp(COOL_STOPS, COOL.length, t);

/**
 * The map's colour per fragment (a `fragmentNode`, raw), from the vertex stage's flat face value
 * (x: has energy, y: level) and smooth node value (x: every corner has energy, y: level).
 */
export function mapColour(look: LookUniforms, flat: N, smooth: N, alpha: N = float(1), keep: N = null): N {
  return Fn(() => {
    // Smooth where every corner of the face has energy, else the face's own flat colour.
    const smoothHere = look.smooth.equal(1).and(smooth.x.greaterThan(0.999)).toVar();
    const l = select(smoothHere, smooth.y, flat.y).toVar();
    const td = l.div(max(look.hi, 1e-6));
    const diffC = select(td.lessThan(0), cool(td.negate()), hot(td));
    const levelC = hot(l.sub(look.lo).div(max(look.hi.sub(look.lo), 1e-6)));
    const c = select(look.diff.equal(1), diffC, levelC).toVar();
    // Contours on the smoothed level; the derivative is taken outside any branch (WGSL's uniformity rule).
    const f = l.div(max(look.iso, 1e-6));
    const w = max(fwidth(f), 1e-6);
    const d = abs(fract(f.add(0.5)).sub(0.5));
    const line = float(1).sub(smoothstep(w.mul(0.5), w.mul(1.5), d));
    If(smoothHere.and(look.iso.greaterThan(0)), () => {
      c.assign(mix(c, vec3(0.93, 0.93, 0.94), line.mul(0.85)));
    });
    Discard(flat.x.lessThan(0.5));
    // Item 10 (spread.ts): the reveal's mask, a fragment not reached yet is not drawn; the colour is untouched.
    if (keep) Discard(keep.not());
    return vec4(c, alpha);
  })();
}

/** The vertex stage's flat value of a map face at the look's step: (1, level or difference) where it has one, else (0, 0). */
export function faceLevel(u: MapUniforms, look: LookUniforms, face: N): N {
  return Fn(() => {
    const ok = float(0).toVar();
    const lvl = float(0).toVar();
    If(look.diff.equal(1), () => {
      const r = diffDb(u, face, look.step);
      If(r.ok, () => {
        ok.assign(1);
        lvl.assign(r.d);
      });
    }).ElseIf(look.linear.equal(1), () => {
      // A parameter map: the face's value as it is; NaN (refused) is not drawn.
      const e = mapTexel(u, u.map, face, int(0)).toVar();
      If(finite(e), () => {
        ok.assign(1);
        lvl.assign(e);
      });
    }).Else(() => {
      const e = faceValue(u, u.map, face, look.step).toVar();
      If(e.greaterThan(0).and(finite(e)), () => {
        ok.assign(1);
        lvl.assign(levelDb(e));
      });
    });
    return vec2(ok, lvl);
  })();
}

/** The vertex stage's smooth value at a map node (its linked faces `start`, `n`): (1, level) where it has energy, else (0, 0). */
export function nodeLevel(u: MapUniforms, look: LookUniforms, start: N, n: N): N {
  return Fn(() => {
    const ok = float(0).toVar();
    const lvl = float(0).toVar();
    If(look.smooth.equal(1), () => {
      const a = nodeEnergy(u, u.map, start, n, look.step).toVar();
      If(look.diff.equal(1), () => {
        const b = nodeEnergy(u, u.base, start, n, look.step).toVar();
        If(a.greaterThan(0).and(b.greaterThan(0)), () => {
          ok.assign(1);
          lvl.assign(levelDb(a).sub(levelDb(b)));
        });
      }).ElseIf(a.greaterThan(0), () => {
        ok.assign(1);
        lvl.assign(levelDb(a));
      });
    });
    return vec2(ok, lvl);
  })();
}

/** The keep rules' arithmetic on TSL nodes (particles.ts `KeepOps`). */
export const NODE_OPS: KeepOps<N, N> = {
  c: (x) => float(x),
  sub: (a, b) => a.sub(b),
  add: (a, b) => a.add(b),
  abs: (a) => abs(a),
  le: (a, b) => a.lessThanEqual(b),
  ge: (a, b) => a.greaterThanEqual(b),
  gt: (a, b) => a.greaterThan(b),
  and: (a, b) => a.and(b),
};

/** particles.ts `rampPlace` on the GPU: where on the hot ramp a record's energy is drawn. */
export function rampPlaceNode(energy: N, logMax: N): N {
  const db = select(energy.greaterThan(0), log2(energy).mul(10).mul(0.3010299956639812), float(-1e9));
  return float(0.55).add(clamp(db.sub(logMax).add(40).div(40), 0, 1).mul(0.45));
}

/** particles.ts `trailAlpha` on the GPU. */
export function trailAlphaNode(step: N, head: N, length: N): N {
  return float(1).sub(clamp(step.sub(head).div(max(length, 1)), 0, 1).mul(0.85));
}
