// UI state and verbs the scene package keeps for itself. The backend's truth stays in store.ts
// and changes only through actions.ts; this holds what the chrome needs between its own panels:
// selecting a scene entity, and Del and F2 on it.
import type { KeyboardEvent } from 'react';
import * as actions from '../actions';
import { fieldKey } from '../issues';
import { removeReceiver, removeSource } from '../ops';
import { selectionStore, stepStore, Store } from '../store';

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
  if (sel.kind === 'source') {
    const out = await actions.apply(removeSource(sel.id), entityKey('source', sel.id));
    if (out.applied) selectionStore.set({ kind: 'none' });
  } else if (sel.kind === 'receiver') {
    const out = await actions.apply(removeReceiver(sel.id), entityKey('point_receiver', sel.id));
    if (out.applied) selectionStore.set({ kind: 'none' });
  }
}

/** Keys that belong to a text field while it has focus. */
export function typing(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName);
}

/**
 * Del removes and F2 renames the selected source or receiver, inside the scene list and the
 * Sources panel only (PLAN.md 7.5, points 1 and 3). A text field keeps its own keys.
 */
export function onEntityKey(e: KeyboardEvent): void {
  if (typing(e.target) || e.ctrlKey || e.altKey || e.metaKey) return;
  const sel = selectionStore.get();
  if (sel.kind !== 'source' && sel.kind !== 'receiver') return;
  if (e.key === 'Delete') {
    e.preventDefault();
    actions.fire(removeSelected());
  } else if (e.key === 'F2') {
    e.preventDefault();
    requestRename();
  }
}
