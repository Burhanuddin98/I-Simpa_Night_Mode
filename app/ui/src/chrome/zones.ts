// G28/G29: fitting zones, a volume of the room filled with scattering objects (upstream's
// encombrements). A pure module, tested by zones.test.ts under `node --test`. The zone itself is
// the core's (`FittingZone`), added and edited only through the checked apply, whose
// `fitting_parameters_invalid` has the last word on its values; these helpers make a new box,
// change its bounds or every band at once, and read a typed field before it is sent.
import type { DiffusionLaw, FittingZone } from '../bindings/schema.ts';
import { parseStrictDecimal } from '../numbers.ts';
import type { F64 } from '../ops.ts';
import type { Box3, XYZ } from './planes.ts';

/** Upstream's new zone: absorption 0, mean free path 1 m, uniform diffusion in every band (e_gammeabsorption.cpp:109-113). */
export const ZONE_DEFAULTS = { absorption: 0, mean_free_path_m: 1, diffusion_law: 'uniform' as DiffusionLaw };

/** The side of a new box zone, metres: upstream's new box is (0, 0, 0) to (1, 1, 1). */
export const NEW_BOX_SIDE_M = 1;

/** The diffusion laws, as upstream lists them (coreTypes.h:108-113), with their words. */
export const DIFFUSION_LAWS: readonly { key: DiffusionLaw; label: string }[] = [
  { key: 'uniform', label: 'Uniform' },
  { key: 'uniform_reflection', label: 'Uniform reflection' },
  { key: 'lambert_reflection', label: 'Lambert reflection' },
];

type BoxShape = Extract<FittingZone['shape'], { kind: 'box' }>;
export type BoxZone = Omit<FittingZone, 'shape'> & { shape: BoxShape };
export const isBoxZone = (z: FittingZone): z is BoxZone => z.shape.kind === 'box';

const finite = (v: F64): v is number => typeof v === 'number' && Number.isFinite(v);
const mm = (v: number) => Math.round(v * 1000) / 1000 + 0;

/**
 * A new enabled box zone, `NEW_BOX_SIDE_M` on a side, centred in the room's box, upstream's values
 * in each of `bands` bands, nothing pinned. Upstream puts its new box at the origin, a corner or
 * outside most models; the bottom of the room's box is no better, since a raked floor rises above
 * it (BRAS CR4's: a box there crossed the floor and TetGen stopped on the self-intersection, bed
 * v1q-p1b, runs 20261009-094510-515-tcr and 20261009-094807-290-tcr). The middle of the box is in
 * the air of most rooms; the user then moves it where the fittings are.
 */
export function newBoxZone(id: string, name: string, room: Box3, bands: number): BoxZone {
  const c = [0, 1, 2].map((i) => (room.min[i] + room.max[i]) / 2);
  const h = NEW_BOX_SIDE_M / 2;
  return {
    id,
    name,
    enabled: true,
    shape: { kind: 'box', min: [mm(c[0] - h), mm(c[1] - h), mm(c[2] - h)], max: [mm(c[0] + h), mm(c[1] + h), mm(c[2] + h)], destination: null },
    absorption: Array.from({ length: bands }, () => ZONE_DEFAULTS.absorption),
    mean_free_path_m: Array.from({ length: bands }, () => ZONE_DEFAULTS.mean_free_path_m),
    diffusion_law: Array.from({ length: bands }, () => ZONE_DEFAULTS.diffusion_law),
    solver_id: null,
  };
}

/** Whether `[min, max]` is a box with volume inside `room`'s box (corners on its walls included). */
export function boxProblem(min: readonly F64[], max: readonly F64[], room: Box3 | null): string | null {
  if (![...min, ...max].every(finite)) return 'a corner is not a number';
  const lo = min as number[];
  const hi = max as number[];
  const axis = [0, 1, 2].find((i) => !(lo[i] < hi[i]));
  if (axis !== undefined) return `its ${'XYZ'[axis]} from ${lo[axis]} m is not below its ${'XYZ'[axis]} to ${hi[axis]} m, so it holds no volume`;
  if (room) {
    const out = [0, 1, 2].find((i) => lo[i] < room.min[i] - 1e-9 || hi[i] > room.max[i] + 1e-9);
    if (out !== undefined) {
      return `it reaches outside the room's box on ${'XYZ'[out]} (the room runs from ${mm(room.min[out])} to ${mm(room.max[out])} m): the mesher would cut the room with it`;
    }
  }
  return null;
}

/** The zone with one bound of a box changed: `which` corner's `axis` set to `value`. Its destination flags are kept. */
export function withBound(zone: BoxZone, which: 'min' | 'max', axis: 0 | 1 | 2, value: number): BoxZone {
  const corner = [...zone.shape[which]] as XYZ;
  corner[axis] = value;
  return { ...zone, shape: { ...zone.shape, [which]: corner } };
}

/** The zone with every band of one quantity set to `value` (upstream's average row, copied to each band). */
export function withEveryBand(zone: FittingZone, quantity: 'absorption' | 'mean_free_path_m', value: number): FittingZone;
export function withEveryBand(zone: FittingZone, quantity: 'diffusion_law', value: DiffusionLaw): FittingZone;
export function withEveryBand(zone: FittingZone, quantity: 'absorption' | 'mean_free_path_m' | 'diffusion_law', value: number | DiffusionLaw): FittingZone {
  return { ...zone, [quantity]: zone[quantity].map(() => value) };
}

/** The one value every band holds, or null when they differ (the "All bands" row then reads "varies"). */
export function sameInEveryBand<T>(values: readonly T[]): T | null {
  return values.length > 0 && values.every((v) => v === values[0]) ? values[0] : null;
}

/** A typed absorption (0 to 1) or mean free path (above 0, metres), or why not. */
export function parseZoneValue(text: string, quantity: 'absorption' | 'mean_free_path_m'): { ok: true; value: number } | { ok: false; message: string } {
  const p = parseStrictDecimal(text);
  if (!p.ok) return { ok: false, message: `"${text}" is not a number: write digits with a decimal point, like 0.3` };
  if (quantity === 'absorption' && !(p.value >= 0 && p.value <= 1)) return { ok: false, message: `an absorption is from 0 to 1, not ${p.value}` };
  if (quantity === 'mean_free_path_m' && !(p.value > 0)) return { ok: false, message: `a mean free path is above 0 m, not ${p.value}` };
  return { ok: true, value: p.value };
}

/**
 * The 12 edges of every enabled box zone, as segment end points (x, y, z, x, y, z per edge), for
 * the 3D view; a zone of surfaces is drawn by its own faces, and a disabled zone not at all.
 */
export function zoneEdges(zones: readonly FittingZone[]): { name: string; min: XYZ; max: XYZ; segments: number[] }[] {
  const out: { name: string; min: XYZ; max: XYZ; segments: number[] }[] = [];
  for (const z of zones) {
    if (!z.enabled || !isBoxZone(z) || ![...z.shape.min, ...z.shape.max].every(finite)) continue;
    const lo = z.shape.min as XYZ;
    const hi = z.shape.max as XYZ;
    const p = (i: number): XYZ => [i & 1 ? hi[0] : lo[0], i & 2 ? hi[1] : lo[1], i & 4 ? hi[2] : lo[2]];
    const segments: number[] = [];
    for (let i = 0; i < 8; i++) {
      for (const bit of [1, 2, 4]) if (!(i & bit)) segments.push(...p(i), ...p(i | bit));
    }
    out.push({ name: z.name, min: [...lo] as XYZ, max: [...hi] as XYZ, segments });
  }
  return out;
}
