// Pure builders of edit ops over the generated `Op` type (PLAN.md 2.2), and `opText`, the only
// way an op becomes text for the backend. A pure module: no store, no backend, only erasable
// TypeScript, tested by ops.test.ts under `node --test`.
import type {
  EntityRef,
  Material,
  MaterialQuantity,
  Op,
  PointReceiver,
  Source,
  Variant,
} from './bindings/schema.ts';

/** A number as the schema stores it: finite values as numbers, non-finite ones as strings. */
export type F64 = number | string;
export type Vec3 = [F64, F64, F64];

/**
 * The op as JSON text for `edit_apply`, which reads it with the core's exact reader.
 *
 * Not `JSON.stringify`: that writes -0 as `0`, losing the sign the core keeps, and NaN or an
 * infinity as `null`. Here -0 is written `-0.0`, and a non-finite number throws (the schema
 * spells those as strings, so a non-finite number is a bug upstream of this call).
 */
export function opText(op: Op): string {
  return write(op, 'op');
}

function write(v: unknown, at: string): string {
  if (v === null) return 'null';
  switch (typeof v) {
    case 'number':
      if (!Number.isFinite(v)) throw new Error(`opText: ${String(v)} at ${at} is not a finite number`);
      return Object.is(v, -0) ? '-0.0' : JSON.stringify(v);
    case 'string':
      return JSON.stringify(v);
    case 'boolean':
      return v ? 'true' : 'false';
    case 'object': {
      if (Array.isArray(v)) return `[${v.map((x, i) => write(x, `${at}[${i}]`)).join(',')}]`;
      const parts: string[] = [];
      for (const [k, x] of Object.entries(v as Record<string, unknown>)) {
        if (x === undefined) continue;
        parts.push(`${JSON.stringify(k)}:${write(x, `${at}.${k}`)}`);
      }
      return `{${parts.join(',')}}`;
    }
    default:
      throw new Error(`opText: a ${typeof v} at ${at} has no JSON form`);
  }
}

// ---- builders ---------------------------------------------------------------------------------

export type EntityKind = EntityRef['kind'];

export const rename = (kind: EntityKind, id: string, name: string): Op => ({
  op: 'rename',
  target: { kind, id } as EntityRef,
  name,
});

export const moveReceiver = (id: string, position: Vec3): Op => ({ op: 'move_point_receiver', id, position });
export const moveSource = (id: string, position: Vec3): Op => ({ op: 'move_source', id, position });

export const addReceiver = (index: number, receiver: PointReceiver): Op => ({ op: 'add_point_receiver', index, receiver });
export const addSource = (index: number, source: Source): Op => ({ op: 'add_source', index, source });
export const removeReceiver = (id: string): Op => ({ op: 'remove_point_receiver', id });
export const removeSource = (id: string): Op => ({ op: 'remove_source', id });

export const setMaterialBand = (material: string, quantity: MaterialQuantity, band: number, value: F64): Op => ({
  op: 'set_material_band',
  material,
  quantity,
  band,
  value,
});

export const addMaterial = (index: number, material: Material): Op => ({ op: 'add_material', index, material });
export const removeMaterial = (id: string): Op => ({ op: 'remove_material', id });

/**
 * Gives `group` the material `material`: on the base project when no variant is active, else as
 * the active variant's override.
 */
export function assignMaterial(group: string, material: string, activeVariant: string | null | undefined): Op {
  return activeVariant
    ? { op: 'set_variant_override', variant: activeVariant, group, material }
    : { op: 'set_group_material', group, material };
}

export const addVariant = (index: number, variant: Variant): Op => ({ op: 'add_variant', index, variant });
export const setActiveVariant = (variant: string | null): Op => ({ op: 'set_active_variant', variant });

/** Several ops as one edit and one undo step, applied all or nothing. */
export const batch = (ops: Op[]): Op => ({ op: 'batch', ops });

// ---- new entities -----------------------------------------------------------------------------

/**
 * The first free name `<prefix><n>` (n = 1, 2, ...) among `taken`: `R4` after R1 to R3, `R2` if
 * only R1 and R3 exist.
 */
export function nextName(prefix: string, taken: readonly string[]): string {
  const used = new Set(taken);
  for (let n = 1; ; n++) if (!used.has(`${prefix}${n}`)) return `${prefix}${n}`;
}

/** A point receiver with the schema's plain defaults: facing +x, no background noise. */
export function newReceiver(id: string, name: string, position: Vec3): PointReceiver {
  return { id, name, position, orientation: [1, 0, 0], background_noise: null, solver_id: null };
}

/** An enabled omni source with a pink spectrum at `globalDb` (the teaching room's is 85). */
export function newSource(id: string, name: string, position: Vec3, globalDb = 85): Source {
  return {
    id,
    name,
    enabled: true,
    position,
    power: { global_db: globalDb, shape: { kind: 'pink' } },
    directivity: { kind: 'omni' },
    delay_s: 0,
    group: null,
    solver_id: null,
  };
}
