// What the Materials step derives from the project view: effective materials under the active
// variant, "used by n", the selected groups, row order, new rows (PLAN.md 6.2). A pure module:
// no store, no backend, only erasable TypeScript, tested by model.test.ts under `node --test`.
import type { Material, SurfaceGroup, Variant } from '../../bindings/schema.ts';
import { compareF64, naturalCompare, type F64 } from './bands.ts';

/** The per-band quantities the grid edits. Transmission is shown read-only. */
export type Quantity = 'absorption' | 'scattering';

export interface ViewLike {
  surface_groups: readonly SurfaceGroup[];
  materials: readonly Material[];
  variants: readonly Variant[];
  active_variant?: string | null;
}

/** The active variant, if one is set and exists. */
export function activeVariant(view: ViewLike): Variant | undefined {
  return view.active_variant ? view.variants.find((v) => v.id === view.active_variant) : undefined;
}

/** The material a group has under `variant`: its override there, else the base project's. */
export function effectiveMaterial(group: SurfaceGroup, variant: Variant | undefined): string {
  return variant?.overrides.find((o) => o.group === group.id)?.material ?? group.material;
}

/** Surface groups per material id, by effective material under the active variant. */
export function usage(view: ViewLike): Map<string, number> {
  const variant = activeVariant(view);
  const out = new Map<string, number>();
  for (const g of view.surface_groups) {
    const m = effectiveMaterial(g, variant);
    out.set(m, (out.get(m) ?? 0) + 1);
  }
  return out;
}

/** The fields of the store's `Selection` this module reads (a pure module cannot import it). */
export interface SelectionLike {
  kind: string;
  id?: string;
  /** Group names, for a face selection. */
  groups?: readonly string[];
}

/** The surface groups a selection names, in project order: a group, or the groups of faces. */
export function selectedGroupIds(sel: SelectionLike, groups: readonly SurfaceGroup[]): string[] {
  if (sel.kind === 'group' && sel.id) return groups.some((g) => g.id === sel.id) ? [sel.id] : [];
  if (sel.kind === 'faces' && sel.groups) {
    const names = new Set(sel.groups);
    return groups.filter((g) => names.has(g.name)).map((g) => g.id);
  }
  return [];
}

// ---- row order --------------------------------------------------------------------------------

/** Project order, or a natural sort by name, or by one band's value of the shown quantity. */
export type SortState =
  | { kind: 'project' }
  | { kind: 'name'; dir: 1 | -1 }
  | { kind: 'band'; band: number; dir: 1 | -1 };

/** A header click: ascending, then descending, then back to project order. */
export function nextSort(current: SortState, clicked: 'name' | number): SortState {
  if (current.kind === 'name' && clicked === 'name') return current.dir === 1 ? { kind: 'name', dir: -1 } : { kind: 'project' };
  if (current.kind === 'band' && current.band === clicked) return current.dir === 1 ? { ...current, dir: -1 } : { kind: 'project' };
  return clicked === 'name' ? { kind: 'name', dir: 1 } : { kind: 'band', band: clicked, dir: 1 };
}

/**
 * The materials in display order. Ties keep project order in either direction, so the order
 * is total and a sort never shuffles equal rows.
 */
export function sortRows(materials: readonly Material[], sort: SortState, quantity: Quantity): Material[] {
  const indexed = materials.map((m, i) => ({ m, i }));
  if (sort.kind === 'project') return materials.slice();
  const key = (m: Material): F64 => (sort.kind === 'band' ? (m[quantity][sort.band] ?? '') : '');
  indexed.sort((a, b) => {
    const c = sort.kind === 'name' ? naturalCompare(a.m.name, b.m.name) : compareF64(key(a.m), key(b.m));
    return c * sort.dir || a.i - b.i;
  });
  return indexed.map((x) => x.m);
}

// ---- display ----------------------------------------------------------------------------------

/** Transmission, read-only in M10: `off`, `on in every band` or `on in n bands`. */
export function transmissionText(m: Material): string {
  const on = (m.transmission_loss_db ?? []).filter((r) => r !== null).length;
  if (on === 0) return 'off';
  if (on === m.absorption.length) return 'on in every band';
  return `on in ${on} of ${m.absorption.length} bands`;
}

/** The design's α mini-bars: a height in px per band, 2 to 24 (design: max(2, round(α·24))). */
export function alphaBars(values: readonly F64[]): number[] {
  return values.map((v) => (typeof v === 'number' && Number.isFinite(v) ? Math.min(24, Math.max(2, Math.round(v * 24))) : 2));
}

/** An area in m² as a geometry fact: one decimal at most; `—` for a non-finite value. */
export function formatArea(m2: number | null | undefined): string {
  if (typeof m2 !== 'number' || !Number.isFinite(m2)) return '—';
  return String(Math.round(m2 * 10) / 10);
}

// ---- new rows ---------------------------------------------------------------------------------

/** Swatches for new materials, the fixture's neutral greys in turn. */
const SWATCHES = ['#6b6b73', '#8a8a93', '#a3a3ab', '#55555e', '#b8b8c0'];

/**
 * A new library material with `bands` values: absorption and scattering 0.1 in every band
 * (the shell spec's user material), specular, double-sided, no transmission, no pinned solver id.
 */
export function newMaterial(id: string, name: string, bands: number, index: number): Material {
  return {
    id,
    name,
    color: SWATCHES[index % SWATCHES.length],
    absorption: Array.from({ length: bands }, () => 0.1),
    scattering: Array.from({ length: bands }, () => 0.1),
    reflection_law: 'specular',
    transmission_loss_db: null,
    double_sided: true,
    solver_id: null,
  };
}
