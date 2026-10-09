// C1 (docs/investigations/2026-10-07-blank-geometry/SPEC.md): the logic of carving a blank
// geometry into named groups in the scene list and the 3D view. Ctrl+click picks several groups,
// Merge folds them into the first picked, F2 renames one, and "Move to" sends picked faces into
// an existing group. G18 adds an empty group and deletes one only when empty. A pure module: no store, no backend, only erasable TypeScript, tested by
// groupsModel.test.ts under `node --test`.
import type { Op } from '../bindings/schema.ts';

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

/** The key two group names collide under, as the core's `validate::group_name_key`: trimmed,
 * without case. */
export const groupNameKey = (name: string): string => name.trim().toLowerCase();

/**
 * Why `name` cannot be the group `id`'s new name, or null: blank, or another group's name
 * already, compared trimmed and without case as the core compares them (it refuses the same
 * rename, `name_taken`). The project is unchanged on a refusal. The name sent is `name.trim()`.
 */
export function renameProblem(name: string, id: string, groups: readonly GroupLike[]): { code: string; message: string } | null {
  const t = name.trim();
  if (!t) return { code: 'GROUP_NAME_EMPTY', message: 'A surface group needs a name.' };
  const other = groups.find((g) => g.id !== id && groupNameKey(g.name) === groupNameKey(t));
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

// ---- G18: add an empty group; delete an empty one ---------------------------------------------

export const ADD_GROUP_LABEL = 'Add surface group';
export const DELETE_GROUP_LABEL = 'Delete group';

/** What deleting a group needs to know of the project (the scene view, structurally). */
export interface GroupDeleteView {
  surface_groups: readonly GroupLike[];
  surface_receivers: readonly { name: string; shape: { kind: string; groups?: readonly string[] } }[];
  variants: readonly { id: string; overrides: readonly { group: string }[] }[];
}

/**
 * Why the group `id`, holding `faces` faces, cannot be deleted, in plain words, or null when it
 * can. Upstream refuses a group that is not empty ("The group is not empty. You can't delete
 * it!", e_scene_groupesurfaces.h:104-107); this says what to do instead. A surface map drawn on
 * the group holds it too (the core refuses that, `in_use`).
 */
export function groupDeleteProblem(id: string, faces: number, view: GroupDeleteView): { code: string; message: string } | null {
  const g = view.surface_groups.find((x) => x.id === id);
  if (!g) return null;
  if (faces > 0) {
    return {
      code: 'GROUP_NOT_EMPTY',
      message: `${g.name} still holds ${faces} ${faces === 1 ? 'face' : 'faces'}, so it cannot be deleted. Move them to another group first (pick them in the 3D view, right-click, Move to group), or merge this group into another.`,
    };
  }
  const map = view.surface_receivers.find((r) => r.shape.kind === 'scene' && (r.shape.groups ?? []).includes(id));
  if (map) {
    return {
      code: 'GROUP_IN_USE',
      message: `${g.name} is part of the surface map ${map.name}, so it cannot be deleted.`,
    };
  }
  return null;
}

/**
 * The edit that deletes the empty group `id`: each variant's override on it cleared first (an
 * override of a group with no faces changes nothing), then the group, as one batch and one
 * undo step.
 */
export function deleteGroupOps(id: string, view: GroupDeleteView): Op {
  const clear: Op[] = view.variants
    .filter((v) => v.overrides.some((o) => o.group === id))
    .map((v) => ({ op: 'set_variant_override', variant: v.id, group: id, material: null }));
  const remove: Op = { op: 'remove_surface_group', id };
  return clear.length ? { op: 'batch', ops: [...clear, remove] } : remove;
}

/** The core's `in_use` refusal ("surface group <id> is still used by fitting zone 'Z'") as a
 * plain sentence about `name`, without the id. */
export function inUseSentence(name: string, message: string): string {
  const by = /is still used by (.+)$/.exec(message)?.[1];
  return by ? `${name} cannot be deleted: ${by} still uses it.` : message;
}
