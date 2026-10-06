// The dimensions overlay (Burhan 2026-10-06 03:09: "this scale, that you can turn on and off that shows
// just how tall, wide and broad the geometry is"): three measuring lines beside the room's box, length
// (x), width (y) and height (z), each with its size in metres. The box is the model check's
// (`roomBox`), the one the Room model panel's Dimensions read, and the labels are written the same
// way, so the two never disagree. A geometry fact, not an acoustic number.
//
// The lines sit just outside the box on the sides facing the camera, so they never cross the room:
// length along the near y side, width along the near x side, height up the corner beside the far end
// of the width line, at the room's silhouette.
import type { Box, Vec } from './geometry.ts';

export type DimKey = 'length' | 'width' | 'height';

export interface DimLine {
  key: DimKey;
  /** What the label says, as the panel writes it: "Length 41.45 m". */
  text: string;
  /** Where the label goes: the measuring line's middle. */
  mid: Vec;
}

const NAMES: Record<DimKey, string> = { length: 'Length', width: 'Width', height: 'Height' };

/** The size written as the Room model panel writes it (two decimals). */
export function dimText(key: DimKey, metres: number): string {
  return `${NAMES[key]} ${metres.toFixed(2)} m`;
}

/** A number on a ruler's tick, and where it goes. */
export interface DimMark {
  text: string;
  p: Vec;
}

/**
 * The numbered ticks' spacing, metres (Burhan 03:13: "to get a sense of scale i need to know how much
 * one metre is ... one metre increments kinda helps"): every 5 m up to a 60 m room, then 10, then 25,
 * so the numbers never crowd. A tick every metre whatever the room.
 */
export function majorStep(longest: number): number {
  return longest <= 60 ? 5 : longest <= 150 ? 10 : 25;
}

/**
 * The three measuring lines for `box` seen from `eye`: their labels, the numbered ticks, and the line
 * segments to draw, 6 numbers a segment: first each measuring line with an extension line from the box
 * to each of its ends (9), then the ruler's ticks, one a metre from the room's edge, longer where numbered.
 */
export function dimensionLines(box: Box, eye: Vec): { lines: DimLine[]; marks: DimMark[]; segments: number[] } {
  const [x0, y0, z0] = box.min;
  const [x1, y1, z1] = box.max;
  const off = 0.04 * Math.max(x1 - x0, y1 - y0, z1 - z0);
  const tick = 0.4 * off;
  // The near side in y (for the length line) and in x (for the width line), and outward signs.
  const ny = eye[1] < (y0 + y1) / 2 ? -1 : 1;
  const nx = eye[0] < (x0 + x1) / 2 ? -1 : 1;
  const yEdge = ny < 0 ? y0 : y1;
  const xEdge = nx < 0 ? x0 : x1;
  const yl = yEdge + ny * off;
  const xw = xEdge + nx * off;
  // Height: standing on the width line's far end, beside the room's corner there.
  const yFar = ny < 0 ? y1 : y0;
  const yh = yFar;
  const seg: number[] = [];
  const add = (a: Vec, b: Vec) => seg.push(...a, ...b);
  // Length, along x.
  add([x0, yl, z0], [x1, yl, z0]);
  for (const x of [x0, x1]) add([x, yEdge, z0], [x, yl + ny * tick, z0]);
  // Width, along y.
  add([xw, y0, z0], [xw, y1, z0]);
  for (const y of [y0, y1]) add([xEdge, y, z0], [xw + nx * tick, y, z0]);
  // Height, along z.
  add([xw, yh, z0], [xw, yh, z1]);
  for (const z of [z0, z1]) add([xEdge, yFar, z], [xw + nx * tick, yh, z]);
  // The rulers: from each line's start, along it, ticks pointing outward (away from the room).
  const major = majorStep(Math.max(x1 - x0, y1 - y0, z1 - z0));
  const marks: DimMark[] = [];
  const ruler = (start: Vec, along: Vec, out: Vec, length: number) => {
    for (let k = 0; k <= length + 1e-9; k++) {
      const p: Vec = [start[0] + k * along[0], start[1] + k * along[1], start[2] + k * along[2]];
      const numbered = k % major === 0;
      const t = (numbered ? 0.5 : 0.25) * off;
      add(p, [p[0] + t * out[0], p[1] + t * out[1], p[2] + t * out[2]]);
      if (numbered) marks.push({ text: String(k), p: [p[0] + 0.95 * off * out[0], p[1] + 0.95 * off * out[1], p[2] + 0.95 * off * out[2]] });
    }
  };
  ruler([x0, yl, z0], [1, 0, 0], [0, ny, 0], x1 - x0);
  ruler([xw, y0, z0], [0, 1, 0], [nx, 0, 0], y1 - y0);
  ruler([xw, yh, z0], [0, 0, 1], [nx, 0, 0], z1 - z0);
  // The sizes sit further out than the numbers, so the two never collide.
  const far = 2.1 * off;
  const lines: DimLine[] = [
    { key: 'length', text: dimText('length', x1 - x0), mid: [(x0 + x1) / 2, yl + ny * far, z0] },
    { key: 'width', text: dimText('width', y1 - y0), mid: [xw + nx * far, (y0 + y1) / 2, z0] },
    { key: 'height', text: dimText('height', z1 - z0), mid: [xw + nx * far, yh, (z0 + z1) / 2] },
  ];
  return { lines, marks, segments: seg };
}
