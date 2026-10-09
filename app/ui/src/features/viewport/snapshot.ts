// W9's PNG of the 3D view (wow list; parity R56; docs/investigations/2026-10-04-wow-w2w3w9/PLAN.md),
// here because only the viewport package makes a canvas (m10.ps1's lint).
// The pure half is tested by snapshot.test.ts under `node --test`; `encodeViewPng` runs in the page.
//
// The image is the frame exactly as drawn (read back from the GPU after a render, so it is what the
// canvas shows, not a re-render at another size), over the window's background colour, which the
// canvas shows through. When a map is shown, a strip below the frame carries its legend: the title,
// the colour bar and the three labels the screen's legend shows, the project, run and step time.
// The strip draws only those texts, each one the on-screen legend's, so the PNG holds no number the
// screen's results regions do not (gate (a)); `lastStrip` keeps what was drawn for the e2e.
//
// R63 (parity): a chart of the Acoustics tab as a PNG, `encodeChartPng`, here for the same reason:
// the chart's own canvas read back (its axes and labels as drawn, the series the legend shows),
// over the panel's background (`over`), with a heading above it naming the chart, the run and the
// key. The heading holds only words and the chart's own key: no number is added to the chart.

/** The strip's height below the frame, px. */
export const STRIP_PX = 64;

/** `#rrggbb` as numbers; null for anything else. */
export function hexRgb(c: string): [number, number, number] | null {
  const m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(c.trim());
  return m ? [parseInt(m[1], 16), parseInt(m[2], 16), parseInt(m[3], 16)] : null;
}

/** GPU rows (bottom first) turned top first. */
export function flipRows(px: Uint8ClampedArray | Uint8Array, w: number, h: number): Uint8ClampedArray {
  const out = new Uint8ClampedArray(px.length);
  const row = 4 * w;
  for (let y = 0; y < h; y++) out.set(px.subarray((h - 1 - y) * row, (h - y) * row), y * row);
  return out;
}

/** Premultiplied RGBA over an opaque background: c + bg (255 - a) / 255, then opaque. */
export function composite(px: Uint8ClampedArray, bg: [number, number, number]): Uint8ClampedArray {
  const out = new Uint8ClampedArray(px.length);
  for (let i = 0; i < px.length; i += 4) {
    const a = px[i + 3];
    for (let c = 0; c < 3; c++) out[i + c] = Math.min(255, px[i + c] + Math.round((bg[c] * (255 - a)) / 255));
    out[i + 3] = 255;
  }
  return out;
}

export interface StripInput {
  project: string | null;
  run: string | null;
  legend: { title: string; lo: string; mid: string; hi: string; gradient: string } | null;
  time: string;
  note: string | null;
}

export interface StripLines {
  heading: string;
  title: string;
  lo: string;
  mid: string;
  hi: string;
  note: string | null;
}

/** What the strip draws, or null for no strip (no map shown). */
export function stripLines(s: StripInput | null): StripLines | null {
  if (!s || !s.legend) return null;
  const heading = [s.project, s.run, s.time].filter((x): x is string => !!x).join(' · ');
  return { heading, title: s.legend.title, lo: s.legend.lo, mid: s.legend.mid, hi: s.legend.hi, note: s.note };
}

/** The gradient's colour stops out of the legend's CSS `linear-gradient(90deg, ...)`. */
function stops(gradient: string): string[] {
  return [...gradient.matchAll(/#[0-9a-f]{6}/gi)].map((m) => m[0]);
}

/** The frame (top-first RGBA, already composited) and the strip, as PNG bytes. */
export async function encodeViewPng(frame: { width: number; height: number; rgba: Uint8ClampedArray }, strip: StripLines | null, legendGradient: string | null, colours: { bg: string; text: string; dim: string; font: string }): Promise<Uint8Array> {
  const extra = strip ? STRIP_PX : 0;
  const c = document.createElement('canvas');
  c.width = frame.width;
  c.height = frame.height + extra;
  const g = c.getContext('2d');
  if (!g) throw new Error('no 2D canvas to encode the image');
  g.putImageData(new ImageData(new Uint8ClampedArray(frame.rgba), frame.width, frame.height), 0, 0);
  if (strip) {
    const y0 = frame.height;
    g.fillStyle = colours.bg;
    g.fillRect(0, y0, c.width, extra);
    g.fillStyle = colours.text;
    g.font = `12px ${colours.font}`;
    g.textBaseline = 'top';
    g.fillText(strip.heading, 16, y0 + 8);
    g.fillStyle = colours.dim;
    g.fillText(strip.title, 16, y0 + 26);
    const barX = Math.max(16 + g.measureText(strip.heading).width, 16 + g.measureText(strip.title).width) + 24;
    const s = stops(legendGradient ?? '');
    if (s.length >= 2) {
      const grad = g.createLinearGradient(barX, 0, barX + 200, 0);
      s.forEach((col, i) => grad.addColorStop(i / (s.length - 1), col));
      g.fillStyle = grad;
      g.fillRect(barX, y0 + 12, 200, 8);
    }
    g.fillStyle = colours.dim;
    g.font = `11px ${colours.font}`;
    g.textAlign = 'left';
    g.fillText(strip.lo, barX, y0 + 26);
    g.textAlign = 'center';
    g.fillText(strip.mid, barX + 100, y0 + 26);
    g.textAlign = 'right';
    g.fillText(strip.hi, barX + 200, y0 + 26);
    g.textAlign = 'left';
    if (strip.note) g.fillText(strip.note, 16, y0 + 44);
  }
  const blob = await new Promise<Blob | null>((res) => c.toBlob(res, 'image/png'));
  if (!blob) throw new Error('the image could not be encoded as PNG');
  return new Uint8Array(await blob.arrayBuffer());
}

/** Straight (not premultiplied) RGBA, a 2D canvas's pixels, over an opaque background: each
 * channel `c·a + bg·(1 − a)`, then opaque. */
export function over(px: Uint8ClampedArray, bg: [number, number, number]): Uint8ClampedArray {
  const out = new Uint8ClampedArray(px.length);
  for (let i = 0; i < px.length; i += 4) {
    const a = px[i + 3];
    for (let c = 0; c < 3; c++) out[i + c] = Math.round((px[i + c] * a + bg[c] * (255 - a)) / 255);
    out[i + 3] = 255;
  }
  return out;
}

/** R63: what a chart image says above the chart: what it is and of what run, and its key. */
export interface ChartHeading {
  /** `project · Run n · Reverberation time`. */
  title: string;
  /** `R1 · all sources summed`. */
  sub: string;
  keys: { label: string; colour: string; dash?: boolean }[];
}

/** The heading's height above the chart, in the chart's pixels per CSS pixel. */
export const CHART_HEADING_PX = 52;

/**
 * R63: a chart (its canvas's pixels, top first, already over the background) with its heading
 * above it, as PNG bytes. The same unattached encoder canvas as the view's image (one canvas on
 * the page, M10 rule 3): it is never put in the document.
 */
export async function encodeChartPng(chart: { width: number; height: number; rgba: Uint8ClampedArray }, heading: ChartHeading, scale: number, colours: { bg: string; text: string; dim: string; font: string }): Promise<Uint8Array> {
  const k = Math.max(1, scale);
  const extra = Math.round(CHART_HEADING_PX * k);
  const titleFont = `${Math.round(13 * k)}px ${colours.font}`;
  const subFont = `${Math.round(11.5 * k)}px ${colours.font}`;
  const c = document.createElement('canvas');
  let g = c.getContext('2d');
  if (!g) throw new Error('no 2D canvas to encode the image');
  // The image is as wide as the chart, or as its heading when that is wider: never a cut word.
  g.font = titleFont;
  const titleW = g.measureText(heading.title).width;
  g.font = subFont;
  const subW = g.measureText(heading.sub).width + heading.keys.reduce((w, key) => w + Math.round(36 * k) + g!.measureText(key.label).width, Math.round(24 * k));
  c.width = Math.max(chart.width, Math.ceil(Math.max(titleW, subW) + Math.round(24 * k)));
  c.height = chart.height + extra;
  g = c.getContext('2d');
  if (!g) throw new Error('no 2D canvas to encode the image');
  g.fillStyle = colours.bg;
  g.fillRect(0, 0, c.width, c.height);
  g.putImageData(new ImageData(new Uint8ClampedArray(chart.rgba), chart.width, chart.height), 0, extra);
  g.textBaseline = 'top';
  g.fillStyle = colours.text;
  g.font = titleFont;
  g.fillText(heading.title, Math.round(12 * k), Math.round(8 * k));
  g.fillStyle = colours.dim;
  g.font = subFont;
  g.fillText(heading.sub, Math.round(12 * k), Math.round(28 * k));
  // The key, right of the sub line: a swatch and its label each.
  let x = Math.round(12 * k) + g.measureText(heading.sub).width + Math.round(24 * k);
  for (const key of heading.keys) {
    g.fillStyle = key.colour;
    if (key.dash) {
      for (let d = 0; d < 14; d += 6) g.fillRect(x + Math.round(d * k), Math.round(34 * k), Math.round(3 * k), Math.max(1, Math.round(2 * k)));
    } else g.fillRect(x, Math.round(33 * k), Math.round(14 * k), Math.max(2, Math.round(3 * k)));
    x += Math.round(20 * k);
    g.fillStyle = colours.dim;
    g.fillText(key.label, x, Math.round(28 * k));
    x += g.measureText(key.label).width + Math.round(16 * k);
  }
  const blob = await new Promise<Blob | null>((res) => c.toBlob(res, 'image/png'));
  if (!blob) throw new Error('the image could not be encoded as PNG');
  return new Uint8Array(await blob.arrayBuffer());
}

/**
 * WebGPU copies a texture to a buffer in rows padded to 256 bytes: the `w` x `h` RGBA rows of `src`
 * (`bytesPerRow` apart) packed tight.
 */
export function unpadRows(src: Uint8Array, w: number, h: number, bytesPerRow: number): Uint8Array {
  const row = 4 * w;
  if (bytesPerRow < row) throw new RangeError(`unpadRows: ${bytesPerRow} bytes a row for ${w} pixels`);
  if (src.length < bytesPerRow * (h - 1) + row) throw new RangeError(`unpadRows: ${src.length} bytes for ${h} rows of ${bytesPerRow}`);
  if (bytesPerRow === row) return src.slice(0, row * h);
  const out = new Uint8Array(row * h);
  for (let y = 0; y < h; y++) out.set(src.subarray(y * bytesPerRow, y * bytesPerRow + row), y * row);
  return out;
}

/** BGRA bytes (the canvas format WebGPU prefers on Windows) as RGBA, in place. */
export function bgraToRgba<T extends Uint8Array | Uint8ClampedArray>(px: T): T {
  for (let i = 0; i < px.length; i += 4) {
    const b = px[i];
    px[i] = px[i + 2];
    px[i + 2] = b;
  }
  return px;
}
