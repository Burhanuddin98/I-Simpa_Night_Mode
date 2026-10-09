// Cutting planes, the sound-level map on a plane (wow list W1, parity M41;
// docs/investigations/2026-10-04-wow-w1w5/PLAN.md). A pure module, tested by planes.test.ts under
// `node --test`. The plane itself is the core's (`SurfaceReceiverShape::CuttingPlane`), added and
// edited only through the checked apply, whose `cutting_plane_invalid` has the last word; these
// helpers place a new one and read a typed field before it is sent.
import type { RunData } from '../bindings/ipc.ts';
import type { SurfaceReceiver, SurfaceReceiverShape } from '../bindings/schema.ts';
import { parseStrictDecimal } from '../numbers.ts';
import type { F64, Vec3 } from '../ops.ts';

/** Upstream's new plane sits 1.6 m above the model's lowest point (e_scene_recepteurss_recepteurcoupe.h:101-103). */
export const EAR_HEIGHT_M = 1.6;
/**
 * The new plane's cell size. Upstream's is 0.5 m (`_proprietes.h:51`); 1 m here, because a map's
 * texture is faces x steps (M12 P3: Elmia's two planes, 2,858 faces, are 114 MB at 10,000 steps)
 * and a 0.5 m plane over a whole hall has four times the faces of a 1 m one. The field takes 0.5.
 */
export const PLANE_RESOLUTION_M = 1;

export type XYZ = [number, number, number];
export interface Box3 {
  min: XYZ;
  max: XYZ;
}

type Plane = Extract<SurfaceReceiverShape, { kind: 'cutting_plane' }>;

const finite = (v: F64): v is number => typeof v === 'number' && Number.isFinite(v);

/** The model's box from the check (`bbox_min`, `extents_m`); null without a finite one. */
export function roomBox(check: { bbox_min: XYZ; extents_m: XYZ } | null | undefined): Box3 | null {
  if (!check) return null;
  const min = check.bbox_min;
  const max = min.map((v, i) => v + check.extents_m[i]) as XYZ;
  return [...min, ...max].every((v) => Number.isFinite(v)) ? { min: [...min] as XYZ, max } : null;
}

/** Upstream's new plane over `box` at `height` above its floor: A (xmin, ymax), B (xmin, ymin), C (xmax, ymin). */
export function earPlane(box: Box3, height = EAR_HEIGHT_M): { a: XYZ; b: XYZ; c: XYZ } {
  const z = box.min[2] + height;
  return { a: [box.min[0], box.max[1], z], b: [box.min[0], box.min[1], z], c: [box.max[0], box.min[1], z] };
}

/** |q - p| in float32, as the solver's `vec3::length` computes it. */
function length32(p: readonly number[], q: readonly number[]): number {
  const d = [0, 1, 2].map((i) => Math.fround(Math.fround(q[i]) - Math.fround(p[i])));
  return Math.fround(Math.sqrt(Math.fround(Math.fround(d[0] * d[0]) + Math.fround(d[1] * d[1]) + Math.fround(d[2] * d[2]))));
}

/**
 * The solver's grid (base_core_configuration.cpp:289-292): `ceilf(|BC| / r)` cells along BC (u)
 * and `ceilf(|BA| / r)` along BA (v), each cell two faces of the map. Null where the core refuses.
 */
export function planeCells(a: Vec3, b: Vec3, c: Vec3, resolution: F64): { u: number; v: number; cells: number; faces: number } | null {
  if (![...a, ...b, ...c].every(finite) || !finite(resolution) || !(resolution > 0)) return null;
  const r = Math.fround(resolution);
  const u = Math.ceil(Math.fround(length32(b as number[], c as number[]) / r));
  const v = Math.ceil(Math.fround(length32(b as number[], a as number[]) / r));
  if (!(u > 0 && v > 0)) return null;
  return { u, v, cells: u * v, faces: 2 * u * v };
}

/** The z of a level plane (its three corners at one finite z); null for a tilted one or not a plane. */
export function levelHeight(shape: SurfaceReceiverShape): number | null {
  if (shape.kind !== 'cutting_plane') return null;
  const z = [shape.a[2], shape.b[2], shape.c[2]];
  return z.every(finite) && z[0] === z[1] && z[1] === z[2] ? (z[0] as number) : null;
}

/** A level plane's height above the room's floor; null for a tilted one. */
export function heightAboveFloor(shape: SurfaceReceiverShape, box: Box3): number | null {
  const z = levelHeight(shape);
  return z === null ? null : z - box.min[2];
}

/** The plane with its three corners at `z`, x and y kept. */
export function withLevelHeight(shape: Plane, z: number): Plane {
  const at = (p: Vec3): Vec3 => [p[0], p[1], z];
  return { ...shape, a: at(shape.a), b: at(shape.b), c: at(shape.c) };
}

/** M41: the plane's three corners, upstream's A, B, C, as the core stores them. */
export const CORNERS = ['a', 'b', 'c'] as const;
export type Corner = (typeof CORNERS)[number];

/** M41: the plane with one coordinate of one corner moved, the others kept: any orientation, as upstream's
 * free corners (`e_scene_recepteurss_recepteurcoupe_proprietes.h`). The fourth corner is A + C - B. */
export function withCorner(shape: Plane, corner: Corner, axis: 0 | 1 | 2, value: number): Plane {
  const p = [...shape[corner]] as Vec3;
  p[axis] = value;
  return { ...shape, [corner]: p };
}

/**
 * M41: the unit normal of the plane through A, B, C (BC x BA, the solver's grid axes), and the angle it
 * makes with the vertical, degrees: 0 for a level plane, 90 for an upright one. Null for collinear or
 * non-finite corners (which the core refuses as `cutting_plane_invalid`).
 */
export function planeTilt(shape: Plane): { normal: XYZ; tiltDeg: number } | null {
  if (![...shape.a, ...shape.b, ...shape.c].every(finite)) return null;
  const [a, b, c] = [shape.a, shape.b, shape.c] as unknown as XYZ[];
  const bc = [c[0] - b[0], c[1] - b[1], c[2] - b[2]];
  const ba = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
  const n: XYZ = [bc[1] * ba[2] - bc[2] * ba[1], bc[2] * ba[0] - bc[0] * ba[2], bc[0] * ba[1] - bc[1] * ba[0]];
  const len = Math.hypot(...n);
  if (!(len > 1e-12 * Math.hypot(...bc) * Math.hypot(...ba))) return null;
  const unit = n.map((x) => x / len) as XYZ;
  return { normal: unit, tiltDeg: (Math.acos(Math.min(1, Math.abs(unit[2]))) * 180) / Math.PI };
}

export type Field = { ok: true; value: number } | { ok: false; message: string };

const notANumber = (text: string): Field => ({ ok: false, message: `"${text}" is not a number: write digits with a decimal point, like 1.6` });

/** A height above the floor, strictly between the floor and the ceiling: a plane on either would lie on the room's surface or outside it. */
export function parseHeightAboveFloor(text: string, box: Box3): Field {
  const p = parseStrictDecimal(text);
  if (!p.ok) return notANumber(text);
  const top = box.max[2] - box.min[2];
  if (!(p.value > 0)) return { ok: false, message: `The plane must lie above the floor: ${p.value} m is not` };
  if (!(p.value < top)) return { ok: false, message: `The plane must lie below the ceiling, ${top} m above the floor: ${p.value} m is not` };
  return { ok: true, value: p.value };
}

/** M41: a corner coordinate, any finite number of metres; the core checks the corners make a plane. */
export function parseCoordinate(text: string): Field {
  const p = parseStrictDecimal(text);
  if (!p.ok) return notANumber(text);
  return { ok: true, value: p.value };
}

/** A cell size above 0 m; the core checks it against the plane's sides too. */
export function parseResolution(text: string): Field {
  const p = parseStrictDecimal(text);
  if (!p.ok) return notANumber(text);
  if (!(p.value > 0)) return { ok: false, message: `The cell size must be above 0 m: ${p.value} m is not` };
  return { ok: true, value: p.value };
}

/** The enabled cutting planes of the project whose names no cutting-plane map of `run` holds. */
export function planesNotInRun(receivers: readonly SurfaceReceiver[], run: RunData | null | undefined): string[] {
  if (!run) return [];
  const held = new Set(run.surfaces.filter((s) => s.cutting_plane).flatMap((s) => s.receivers.map((r) => r.name)));
  return receivers.filter((r) => r.enabled && r.shape.kind === 'cutting_plane' && !held.has(r.name)).map((r) => r.name);
}

/** What the Results step says about planes a run does not hold; null when it holds them all. */
export function rerunText(names: readonly string[]): string | null {
  if (!names.length) return null;
  if (names.length === 1) return `${names[0]} is not in this run: run SPPS again to map it.`;
  const list = `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}`;
  return `${list} are not in this run: run SPPS again to map them.`;
}
