// The particles' warm incandescent ramp (decision 69, Burhan 20:13: "MANY SHADES OF RED, ORANGE,
// YELLOW ... UBER BEAUTIFUL"). Pure, tested by warmRamp.test.ts under `node --test`.
//
// By a particle's level in dB re the loudest particle of the band, over the top WARM_DEPTH_DB: ember at
// the floor through crimson, the brand red, vermilion, orange, amber and yellow to white-hot at the top.
// Interpolated in OKLab, so each step between stops is even to the eye and lightness only rises: a
// louder particle always reads brighter (decision 63's rule, kept). The surface maps keep decision 63's
// own ramp (mapData.ts HOT); this one is for light, drawn additively and tone mapped.

export const WARM: readonly string[] = ['#1A0505', '#5A0A12', '#E0202E', '#FF4A1C', '#FF8A1F', '#FFC23A', '#FFE680', '#FFF8EA'];
/** The levels the ramp spans, dB below the loudest particle. */
export const WARM_DEPTH_DB = 60;
/** Entries in the ramp's lookup texture. */
export const WARM_LUT_SIZE = 256;

const hex3 = (c: string): [number, number, number] => [parseInt(c.slice(1, 3), 16) / 255, parseInt(c.slice(3, 5), 16) / 255, parseInt(c.slice(5, 7), 16) / 255];
const toLinear = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const toSrgb = (c: number) => (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);

/** sRGB (0-1) to OKLab (Bjorn Ottosson's matrices). */
export function oklab(rgb: readonly [number, number, number]): [number, number, number] {
  const [r, g, b] = rgb.map(toLinear);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
}

/** OKLab to sRGB (0-1), clamped into the gamut. */
export function fromOklab([L, a, b]: readonly [number, number, number]): [number, number, number] {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const lin = [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s];
  return lin.map((c) => Math.min(1, Math.max(0, toSrgb(Math.min(1, Math.max(0, c)))))) as [number, number, number];
}

const STOPS_LAB = WARM.map((c) => oklab(hex3(c)));

/** The ramp at t in [0, 1], sRGB 0-1: the stops evenly spaced, interpolated in OKLab. */
export function warmAt(t: number): [number, number, number] {
  const u = Math.min(1, Math.max(0, t)) * (WARM.length - 1);
  const k = Math.min(WARM.length - 2, Math.floor(u));
  const f = u - k;
  const a = STOPS_LAB[k];
  const b = STOPS_LAB[k + 1];
  return fromOklab([a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f]);
}

/** The ramp as WARM_LUT_SIZE RGBA8 texels (sRGB), for a 1-D lookup texture. */
export function warmLut(n = WARM_LUT_SIZE): Uint8Array {
  const out = new Uint8Array(4 * n);
  for (let i = 0; i < n; i++) {
    const c = warmAt(i / (n - 1));
    out.set([Math.round(c[0] * 255), Math.round(c[1] * 255), Math.round(c[2] * 255), 255], 4 * i);
  }
  return out;
}

/** Where on the ramp a particle's energy sits: 0 at WARM_DEPTH_DB below the loudest (`logMax`, dB) or less, 1 at it. */
export function warmPlace(energy: number, logMax: number): number {
  if (!(energy > 0)) return 0;
  return Math.min(1, Math.max(0, (10 * Math.log10(energy) - logMax + WARM_DEPTH_DB) / WARM_DEPTH_DB));
}

/** The legend's CSS gradient for the ramp (its stops sampled in OKLab, so the bar shows what the GPU draws). */
export function warmGradient(samples = 16): string {
  const parts: string[] = [];
  for (let i = 0; i < samples; i++) {
    const c = warmAt(i / (samples - 1)).map((v) => Math.round(v * 255).toString(16).padStart(2, '0'));
    parts.push(`#${c.join('')}`);
  }
  return `linear-gradient(90deg, ${parts.join(', ')})`;
}

/** The legend's labels: the floor, the middle and the top, dB re the loudest particle. */
export function warmLabels(): { lo: string; mid: string; hi: string } {
  return { lo: `−${WARM_DEPTH_DB}`, mid: `−${WARM_DEPTH_DB / 2}`, hi: '0 dB' };
}
