// Face selection with the keyboard's modifiers and a drag box (parity G47; upstream
// OpenGlViewer.cpp:338-373 and Docs/surface_selection.rst: Ctrl adds faces to the selection, a
// drag in selection mode picks every face it passes, a double-click takes the coplanar surface).
// In this app a plain drag turns the camera, so the box is Shift+drag:
//   - click: that face alone; Ctrl+click: the face in or out of the selection; Shift+click: added;
//   - double-click: the face's flat surface; with Ctrl or Shift, that surface added;
//   - Shift+drag: a box; every face seen inside it is added to the picked faces.
// A pure module (no three, no store), tested by faceSelect.test.ts under `node --test`.

/** How a pick changes the picked faces. */
export type PickMode = 'replace' | 'add' | 'toggle';

/** The mode a click's modifiers ask for: Ctrl (or Cmd) toggles, Shift adds, neither replaces. */
export function pickMode(e: { ctrlKey: boolean; shiftKey: boolean; metaKey?: boolean }): PickMode {
  if (e.ctrlKey || e.metaKey) return 'toggle';
  if (e.shiftKey) return 'add';
  return 'replace';
}

/**
 * The picked faces after a pick of `picked` in `mode`, from `current` (the faces picked now, or
 * empty when the selection is not faces): sorted, each once. Toggle takes a face out when every
 * face picked is already in, else adds them all (a double-click's surface toggles as one).
 */
export function combineFaces(current: readonly number[], picked: readonly number[], mode: PickMode): number[] {
  const now = new Set(current);
  const add = [...new Set(picked)];
  let out: Set<number>;
  if (mode === 'replace') out = new Set(add);
  else if (mode === 'add') out = new Set([...now, ...add]);
  else if (add.length > 0 && add.every((f) => now.has(f))) {
    out = now;
    for (const f of add) out.delete(f);
  } else out = new Set([...now, ...add]);
  return [...out].sort((a, b) => a - b);
}

export interface Rect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** The box two client points span, edges sorted. */
export function rectOf(a: { x: number; y: number }, b: { x: number; y: number }): Rect {
  return { left: Math.min(a.x, b.x), top: Math.min(a.y, b.y), right: Math.max(a.x, b.x), bottom: Math.max(a.y, b.y) };
}

/** Whether a client point lies in the box, edges included. */
export function inRect(r: Rect, x: number, y: number): boolean {
  return x >= r.left && x <= r.right && y >= r.top && y <= r.bottom;
}

/** At most this many grid rays sample a box (the centroids inside it are cast as well). */
export const BOX_MAX_SAMPLES = 6000;

/**
 * The client points a box is sampled at: a square grid, its spacing the smallest whole number of
 * pixels (at least 3) that keeps the count at or under `max`, starting half a step in from the
 * top-left corner. A box smaller than one step still gets its centre.
 */
export function boxSamples(r: Rect, max = BOX_MAX_SAMPLES): { x: number; y: number }[] {
  const w = r.right - r.left;
  const h = r.bottom - r.top;
  if (!(w >= 0 && h >= 0)) return [];
  let step = 3;
  while (Math.ceil(w / step) * Math.ceil(h / step) > max) step++;
  const out: { x: number; y: number }[] = [];
  for (let y = r.top + step / 2; y <= r.bottom; y += step) for (let x = r.left + step / 2; x <= r.right; x += step) out.push({ x, y });
  if (out.length === 0) out.push({ x: (r.left + r.right) / 2, y: (r.top + r.bottom) / 2 });
  return out;
}
