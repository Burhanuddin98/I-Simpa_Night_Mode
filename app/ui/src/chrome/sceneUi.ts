// UI state and verbs the scene package keeps for itself. The backend's truth stays in store.ts
// and changes only through actions.ts; this holds what the chrome needs between its own panels:
// selecting a scene entity, and Del and F2 on it.
/** What the key handlers read: React's synthetic event and the window's native one both have it. */
type KeyEvent = Pick<KeyboardEvent, 'key' | 'target' | 'ctrlKey' | 'altKey' | 'metaKey' | 'preventDefault'>;
import * as actions from '../actions';
import { fieldKey } from '../issues';
import type { PointReceiver, Source } from '../bindings/schema';
import { addReceiver, addSource, removeReceiver, removeSource } from '../ops';
import { groupRenameStore, log, sceneStore, selectionStore, stepStore, Store } from '../store';
import { deleteGroupOps, groupDeleteProblem, inUseSentence } from './groupsModel';
import { copyName, displayName } from './sceneModel';

/** Bumped by F2 on a selected source or receiver: the Sources panel focuses its name field. */
export const renameRequestStore = new Store<number>(0);

export function requestRename(): void {
  stepStore.set('sources');
  renameRequestStore.set(renameRequestStore.get() + 1);
}

/** Selects a source or receiver and shows the step that edits it. */
export function selectPoint(kind: 'source' | 'receiver', id: string): void {
  selectionStore.set({ kind, id });
  stepStore.set('sources');
}

/** Selects a surface group and shows the Materials step, where its material is set. */
export function selectGroup(id: string): void {
  selectionStore.set({ kind: 'group', id });
  stepStore.set('materials');
}

/** The field key a whole entity's refusals are filed under (a removal, say). */
export const entityKey = (kind: 'source' | 'point_receiver', id: string) => fieldKey(kind, id);

/**
 * Removes the selected source or receiver. The checked apply refuses a removal that adds an
 * error (the only source: SOURCE_NONE); the refusal is filed under the entity and shown in the
 * Sources panel, and the selection stays.
 */
export async function removeSelected(): Promise<void> {
  const sel = selectionStore.get();
  if (sel.kind === 'source' || sel.kind === 'receiver') await removeEntity(sel.kind, sel.id);
}

/** Removes one source or receiver by id (the scene list's remove buttons); the selection clears if it was that one. */
export async function removeEntity(kind: 'source' | 'receiver', id: string): Promise<void> {
  const out =
    kind === 'source'
      ? await actions.apply(removeSource(id), entityKey('source', id))
      : await actions.apply(removeReceiver(id), entityKey('point_receiver', id));
  const sel = selectionStore.get();
  if (out.applied && sel.kind === kind && sel.id === id) selectionStore.set({ kind: 'none' });
}

/** A39: what Ctrl+C copied, a source or a receiver as it was then: an in-app clipboard of elements,
 * never text (a text field keeps its own Ctrl+C and Ctrl+V). Kept for the session, across projects. */
export type Copied = { kind: 'source'; item: Source } | { kind: 'receiver'; item: PointReceiver };
export const clipboardStore = new Store<Copied | null>(null);

/** A39, Ctrl+C: copies the selected source or receiver. False when neither is selected (the key then
 * does what it does anyway). */
export function copySelected(): boolean {
  const sel = selectionStore.get();
  const view = sceneStore.get()?.view;
  if (!view || (sel.kind !== 'source' && sel.kind !== 'receiver')) return false;
  const copied: Copied | null =
    sel.kind === 'source'
      ? ((s) => (s ? { kind: 'source', item: s } : null))(view.sources.find((s) => s.id === sel.id))
      : ((r) => (r ? { kind: 'receiver', item: r } : null))(view.point_receivers.find((r) => r.id === sel.id));
  if (!copied) return false;
  clipboardStore.set(copied);
  log('INFO', `Copied ${copied.kind} ${copied.item.name}: Ctrl+V pastes a copy at the same position`);
  return true;
}

/**
 * A39, Ctrl+V: a copy of what Ctrl+C copied, at the end of its list, with a new id, no solver id
 * (the core numbers it) and the name `<name> copy`; everything else as copied, its position too.
 * One checked edit, one undo step, refused like any added source or receiver (outside this room,
 * say: the Console says why), and the copy is then selected. False when nothing is copied, no
 * project is open, or the paste was refused.
 */
export async function pasteCopied(): Promise<boolean> {
  const c = clipboardStore.get();
  const view = sceneStore.get()?.view;
  if (!c || !view) return false;
  const id = crypto.randomUUID();
  const out =
    c.kind === 'source'
      ? await actions.apply(
          addSource(view.sources.length, { ...c.item, id, solver_id: null, name: copyName(c.item.name, view.sources.map((s) => s.name)) }),
          'source:new:paste',
        )
      : await actions.apply(
          addReceiver(view.point_receivers.length, {
            ...c.item,
            id,
            solver_id: null,
            name: copyName(c.item.name, view.point_receivers.map((r) => r.name)),
          }),
          'point_receiver:new:paste',
        );
  if (!out.applied) return false;
  selectPoint(c.kind, id);
  log('OK', `Pasted a copy of ${c.kind} ${c.item.name}, at the same position: move it in its properties`);
  return true;
}

/** The latest group edit's own problem, `{code, message}`: a rename the UI refused, a delete of a
 * group that holds faces, a command the core rejected. One that concerns a single group carries its
 * id, `group`, and shows under that group's row, scrolled into view; any other shows under the
 * Surfaces in the scene list. The next group edit clears it. */
export interface GroupProblem {
  code: string;
  message: string;
  group?: string;
}
export const groupProblemStore = new Store<GroupProblem | null>(null);

/**
 * Deletes the surface group `id` (parity G18, A29), only when it is empty, as upstream does: one
 * that holds faces, or that a surface map is drawn on, is refused with a plain sentence saying why
 * and what to do (`groupDeleteProblem`), and nothing changes. An empty group goes in one checked
 * edit and one undo step, its variants' overrides with it. The selection clears if it was that group.
 */
export async function deleteGroup(id: string): Promise<void> {
  groupProblemStore.set(null);
  const scene = sceneStore.get();
  if (!scene) return;
  const view = scene.view;
  const name = displayName(view.surface_groups.find((g) => g.id === id)?.name ?? id);
  // The names as the scene list shows them (BRAS's `mat_CR4_concrete` reads `Concrete`).
  const shown = { ...view, surface_groups: view.surface_groups.map((g) => ({ ...g, name: displayName(g.name) })) };
  const problem = groupDeleteProblem(id, scene.groups.find((g) => g.id === id)?.faces ?? 0, shown);
  if (problem) {
    groupProblemStore.set({ ...problem, group: id });
    log('FAIL', `Delete group refused: ${problem.message}`);
    return;
  }
  try {
    const out = await actions.apply(deleteGroupOps(id, view), `surface_group:${id}:delete`);
    if (!out.applied) return;
    const sel = selectionStore.get();
    if (sel.kind === 'group' && sel.id === id) selectionStore.set({ kind: 'none' });
    log('OK', `Deleted group ${name}`);
  } catch (e) {
    const err = actions.asCmdError(e);
    groupProblemStore.set({ code: err.code, message: err.code === 'OP_IN_USE' ? inUseSentence(name, err.message) : err.message, group: id });
  }
}

/** Keys that belong to a text field while it has focus. */
export function typing(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName);
}

/**
 * Del removes and F2 renames the selected source, receiver or surface group (PLAN.md 7.5, points 1 and 3): in the
 * scene list and the Sources panel, and, through `onWindowEntityKey`, anywhere else, the 3D view
 * included (Burhan 2026-10-06: Del did nothing after picking a source in the view). A text field
 * keeps its own keys.
 */
export function onEntityKey(e: KeyEvent): void {
  if (typing(e.target) || e.ctrlKey || e.altKey || e.metaKey) return;
  const sel = selectionStore.get();
  // C1: F2 on a selected surface group edits its name in the scene list; G18: Del deletes it if empty.
  if (sel.kind === 'group' && e.key === 'F2') {
    e.preventDefault();
    groupRenameStore.set(sel.id);
    return;
  }
  if (sel.kind === 'group' && e.key === 'Delete') {
    e.preventDefault();
    actions.fire(deleteGroup(sel.id));
    return;
  }
  if (sel.kind !== 'source' && sel.kind !== 'receiver') return;
  if (e.key === 'Delete') {
    e.preventDefault();
    actions.fire(removeSelected());
  } else if (e.key === 'F2') {
    e.preventDefault();
    requestRename();
  }
}

/** The window's listener: Del and F2 outside the panels that already handle them (their handler marks the event). */
export function onWindowEntityKey(e: KeyboardEvent): void {
  if (!e.defaultPrevented) onEntityKey(e);
}
