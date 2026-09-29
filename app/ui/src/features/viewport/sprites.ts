// Marker images as RGBA bytes, for three.js `DataTexture`s (PLAN.md 2.4, rule 3: never a
// `CanvasTexture`, which would need a second canvas). A pure module, tested by
// sprites.test.ts. The looks follow the design (concept-b-approved.dc.html:118-129): the source
// is a red dot with a two-step glow, a receiver a white ring on the background colour.

export type Rgb = readonly [number, number, number];

export const RED: Rgb = [0xe0, 0x20, 0x2e];
export const WHITE: Rgb = [0xed, 0xed, 0xef];
export const BG: Rgb = [0x09, 0x09, 0x0b];

/** Coverage of a disc edge at distance `d` from radius `r`, one pixel wide. */
function edge(d: number, r: number, px: number): number {
  return Math.min(1, Math.max(0, (r - d) / px + 0.5));
}

function image(size: number, shade: (u: number, px: number) => [Rgb, number]): Uint8Array {
  const out = new Uint8Array(size * size * 4);
  const c = (size - 1) / 2;
  const px = 2 / size; // one pixel in radius units (radius 1 is the image's half width)
  for (let j = 0; j < size; j++)
    for (let i = 0; i < size; i++) {
      const u = Math.hypot(i - c, j - c) / (size / 2);
      const [rgb, a] = shade(u, px);
      const o = 4 * (j * size + i);
      out[o] = rgb[0];
      out[o + 1] = rgb[1];
      out[o + 2] = rgb[2];
      out[o + 3] = Math.round(Math.min(1, Math.max(0, a)) * 255);
    }
  return out;
}

/**
 * The source marker: a solid dot of radius `dot` (in half-widths) and a glow that falls to
 * nothing at the rim, like the design's `box-shadow: 0 0 18px ..0.85, 0 0 44px ..0.35`.
 */
export function glowPixels(size: number, dot = 0.25): Uint8Array {
  return image(size, (u, px) => {
    const core = edge(u, dot, px);
    const r = Math.max(0, u - dot);
    const glow = (0.85 * Math.exp(-((r / 0.16) ** 2)) + 0.35 * Math.exp(-((r / 0.42) ** 2))) * Math.max(0, 1 - u);
    return [RED, Math.max(core, u >= 1 ? 0 : glow)];
  });
}

/**
 * A ring between radii `inner` and `outer` (half-widths) in `ring`, filled inside with `fill`
 * (or transparent when `fill` is null).
 */
export function ringPixels(size: number, inner: number, outer: number, ring: Rgb, fill: Rgb | null): Uint8Array {
  return image(size, (u, px) => {
    const disc = edge(u, outer, px);
    const hole = edge(u, inner, px);
    if (fill) {
      // Inside the hole: the fill, blended to the ring at its edge.
      const k = hole;
      const rgb: Rgb = [
        Math.round(fill[0] * k + ring[0] * (1 - k)),
        Math.round(fill[1] * k + ring[1] * (1 - k)),
        Math.round(fill[2] * k + ring[2] * (1 - k)),
      ];
      return [rgb, disc];
    }
    return [ring, disc * (1 - hole)];
  });
}
