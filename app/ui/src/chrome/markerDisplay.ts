// Parity G50: how the 3D view draws a source, a point receiver or a fitting zone, upstream's render
// properties of each ("Color" and "Show name", `e_scene_sources_source_rendu.h:46,48`,
// `e_scene_recepteursp_recepteur_rendu.h:46,48`, `e_scene_encombrements_encombrement_rendu.h:57,59`).
// The core keeps it as the element's `display` (`MarkerDisplay`), display only; these helpers read it
// with the kind's defaults and write it back normalised, so setting a property back to its default
// leaves the element as it was. A pure module, tested by markerDisplay.test.ts under `node --test`.
import type { MarkerDisplay } from '../bindings/schema.ts';

export type MarkerKind = 'source' | 'receiver' | 'zone';

/** The view's own colours: the design's red source, white receiver ring, and the line grey of a zone's box. */
export const DEFAULT_MARKER_COLOR: Record<MarkerKind, string> = { source: '#e0202e', receiver: '#ededef', zone: '#ededef' };

/** Upstream's defaults for Show name: on for sources and receivers, off for fitting zones. */
export const DEFAULT_SHOW_NAME: Record<MarkerKind, boolean> = { source: true, receiver: true, zone: false };

type Displayed = { display?: MarkerDisplay | null };

/** The colour the view draws `el` in, `#rrggbb`. */
export function markerColor(el: Displayed, kind: MarkerKind): string {
  return el.display?.color ?? DEFAULT_MARKER_COLOR[kind];
}

/** Whether the view writes `el`'s name beside it. */
export function nameShown(el: Displayed, kind: MarkerKind): boolean {
  return el.display?.show_name ?? DEFAULT_SHOW_NAME[kind];
}

/** Whether `el` has its own colour (not the kind's). */
export function hasOwnColor(el: Displayed): boolean {
  return typeof el.display?.color === 'string';
}

/**
 * `el` with its display changed by `patch` (a colour `#rrggbb` or null for the kind's; Show name), each
 * property equal to the kind's default left out, and `display` null when nothing is left: so a property
 * set back to its default writes the element exactly as before it was set.
 */
export function withDisplay<T extends Displayed>(el: T, kind: MarkerKind, patch: { color?: string | null; show_name?: boolean }): T {
  const now = el.display ?? {};
  const color = 'color' in patch ? patch.color : now.color;
  const show = 'show_name' in patch ? patch.show_name : now.show_name;
  const next: MarkerDisplay = {};
  if (typeof color === 'string') next.color = color.toLowerCase();
  if (typeof show === 'boolean' && show !== DEFAULT_SHOW_NAME[kind]) next.show_name = show;
  return { ...el, display: Object.keys(next).length ? next : null };
}

/** `#rrggbb` as three 0-1 sRGB numbers. */
export function rgb01(hex: string): [number, number, number] {
  const n = Number.parseInt(hex.slice(1), 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}
