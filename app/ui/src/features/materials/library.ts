// "Add from library" (M11 PLAN.md 1.2, row 22 item M1): upstream's 11 reference materials, as the
// core gives them (`material_library`, never retyped here), and which of them the project already
// holds. A pure module: no store, no backend, only erasable TypeScript, tested by library.test.ts
// under `node --test`.
import type { LibraryMaterial } from '../../bindings/ipc.ts';
import type { Material } from '../../bindings/schema.ts';

export interface LibraryRow {
  entry: LibraryMaterial;
  /** The id of a project material with this entry's name, if there is one. */
  inProject: string | null;
}

/**
 * The library's entries in the core's order ("100% absorbing" to "0% absorbing"), each with the
 * project material of the same name, if any. An entry already in the project is not added a
 * second time: the one there is assigned from the Material list instead (two materials of one
 * name could not be told apart in that list).
 */
export function libraryRows(library: readonly LibraryMaterial[], materials: readonly Pick<Material, 'id' | 'name'>[]): LibraryRow[] {
  const byName = new Map<string, string>();
  for (const m of materials) if (!byName.has(m.name)) byName.set(m.name, m.id);
  return library.map((entry) => ({ entry, inProject: byName.get(entry.name) ?? null }));
}

/** The id of the material an add appended: the last of the new list, when the list grew by one. */
export function addedMaterial(before: readonly { id: string }[], after: readonly { id: string }[]): string | null {
  if (after.length !== before.length + 1) return null;
  const last = after[after.length - 1];
  return before.some((m) => m.id === last.id) ? null : last.id;
}
