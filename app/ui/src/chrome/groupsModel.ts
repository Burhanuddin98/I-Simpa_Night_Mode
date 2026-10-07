// C1 (docs/investigations/2026-10-07-blank-geometry/SPEC.md): the logic of carving a blank
// geometry into named groups in the scene list and the 3D view. Ctrl+click picks several groups,
// Merge folds them into the first picked, F2 renames one, and "Move to" sends picked faces into
// an existing group. A pure module: no store, no backend, only erasable TypeScript, tested by
// groupsModel.test.ts under `node --test`.

/** The selection kinds this module reads and makes (the store's `Selection`, structurally). */
export type GroupSelection =
  | { kind: 'none' }
  | { kind: 'group'; id: string }
  | { kind: 'groups'; ids: string[] }
  | { kind: string; [key: string]: unknown };

interface GroupLike {
  id: string;
  name: string;
}

/** The menu entries' words. */
export const MERGE_LABEL = 'Merge groups';
export const RENAME_GROUP_LABEL = 'Rename group';
export const MOVE_TO_LABEL = 'Move to group';

/**
 * A Ctrl+click on the group `id` in the scene list: adds it to the picked groups, or takes it
 * out when it is picked already. One group left is a plain group selection; none is none. Any
 * other selection (a source, faces) starts over from this group.
 */
export function toggleGroup(sel: GroupSelection, id: string): GroupSelection {
  const picked = sel.kind === 'group' ? [sel.id as string] : sel.kind === 'groups' ? [...(sel.ids as string[])] : [];
  const next = picked.includes(id) ? picked.filter((x) => x !== id) : [...picked, id];
  if (next.length === 0) return { kind: 'none' };
  if (next.length === 1) return { kind: 'group', id: next[0] };
  return { kind: 'groups', ids: next };
}

/** Whether `id` is among the picked groups. */
export function groupPicked(sel: GroupSelection, id: string): boolean {
  return sel.kind === 'group' ? sel.id === id : sel.kind === 'groups' ? (sel.ids as string[]).includes(id) : false;
}

/** What Merge would do with the selection: every picked group into the first picked, or null
 * (fewer than two picked, or a picked id the project no longer has). */
export function mergePlan(sel: GroupSelection, groups: readonly GroupLike[]): { into: GroupLike; from: GroupLike[] } | null {
  if (sel.kind !== 'groups') return null;
  const found = (sel.ids as string[]).map((id) => groups.find((g) => g.id === id));
  if (found.length < 2 || found.some((g) => g === undefined)) return null;
  const [into, ...from] = found as GroupLike[];
  return { into, from };
}

/** The single group a selection names, for F2: a group selection's, else null. */
export function renameTarget(sel: GroupSelection): string | null {
  return sel.kind === 'group' ? (sel.id as string) : null;
}

/**
 * Why `name` cannot be the group `id`'s new name, or null: blank, or another group's name
 * already (the group list is how faces are told apart; two alike are refused here, as New group
 * from selection refuses them in the core). The project is unchanged on a refusal.
 */
export function renameProblem(name: string, id: string, groups: readonly GroupLike[]): { code: string; message: string } | null {
  const t = name.trim();
  if (!t) return { code: 'GROUP_NAME_EMPTY', message: 'A surface group needs a name.' };
  const other = groups.find((g) => g.id !== id && g.name === t);
  if (other) return { code: 'GROUP_NAME_TAKEN', message: `Another surface group is named '${t}'. Pick another name.` };
  return null;
}

/**
 * The groups "Move to group" offers for picked faces: every group, in project order, but the
 * one group that already holds all of them (moving there would change nothing).
 */
export function moveTargets<G extends GroupLike>(groups: readonly G[], pickedGroupNames: readonly string[]): G[] {
  const only = new Set(pickedGroupNames).size === 1 ? pickedGroupNames[0] : null;
  return groups.filter((g) => g.name !== only);
}
