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

/**
 * The three measuring lines for `box` seen from `eye`: their labels, and the line segments to draw
 * (each measuring line plus an extension line from the box to each of its ends), 6 numbers a segment.
 */
export function dimensionLines(box: Box, eye: Vec): { lines: DimLine[]; segments: number[] } {
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
  const lines: DimLine[] = [
    { key: 'length', text: dimText('length', x1 - x0), mid: [(x0 + x1) / 2, yl, z0] },
    { key: 'width', text: dimText('width', y1 - y0), mid: [xw, (y0 + y1) / 2, z0] },
    { key: 'height', text: dimText('height', z1 - z0), mid: [xw, yh, (z0 + z1) / 2] },
  ];
  return { lines, segments: seg };
}
