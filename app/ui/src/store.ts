// State that lives outside React (the islands rule: React owns the chrome, stores feed it).
// PLAN.md 2.1. Only actions.ts writes the backend's truth (sceneStore, meshStore); the packages
// read it and write only their own UI state (selection, tool, step).
import { useSyncExternalStore } from 'react';
import type { LineClass, SceneState, UiIssue } from './bindings/ipc';
import type { SceneMesh } from './mesh';
import type { StepKey } from './steps';

export class Store<T> {
  private listeners = new Set<() => void>();
  private value: T;
  constructor(value: T) {
    this.value = value;
  }
  get = (): T => this.value;
  set(value: T): void {
    this.value = value;
    this.listeners.forEach((l) => l());
  }
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };
}

export function useStore<T>(store: Store<T>): T {
  return useSyncExternalStore(store.subscribe, store.get);
}

export interface ConsoleLine {
  time: string;
  tag: LineClass;
  text: string;
}

export const consoleStore = new Store<ConsoleLine[]>([]);
export type Status = { kind: 'ready' | 'busy' | 'bad'; text: string };
export const statusStore = new Store<Status>({ kind: 'ready', text: 'Ready' });

/** The backend's truth: replaced with every M10 response, never edited in place. */
export const sceneStore = new Store<SceneState | null>(null);
/** The decoded `scene_mesh`, refetched when `info.geometry_rev` changes. */
export const meshStore = new Store<SceneMesh | null>(null);
/** The workflow step shown (the step bar writes it; so do the e2e hooks). */
export const stepStore = new Store<StepKey>('geometry');

export type Selection =
  | { kind: 'none' }
  /** A viewport pick or flood-fill: face indices, and the names of their groups. */
  | { kind: 'faces'; faces: number[]; groups: string[] }
  | { kind: 'group'; id: string }
  | { kind: 'material'; id: string }
  | { kind: 'source'; id: string }
  | { kind: 'receiver'; id: string };
export const selectionStore = new Store<Selection>({ kind: 'none' });

export type Tool = 'select' | 'orbit' | 'place-receiver' | 'place-source';
export const toolStore = new Store<Tool>('select');

/** Refusals of the checked apply by field key (issues.ts `fieldKey`); a field's entry is cleared
 * by the next accepted edit filed under the same key. */
export const refusalStore = new Store<ReadonlyMap<string, UiIssue[]>>(new Map());

/** Commands in flight (the backend wrapper counts them); the `idle()` hook waits for 0. */
export const busyStore = new Store<number>(0);

/** A mesh file waiting for the import dialog's unit and up axis (File › Open… on a mesh). */
export const importRequestStore = new Store<{ path: string } | null>(null);

/**
 * What the viewport has drawn: `live` once a real viewport has mounted, and the geometry
 * revision of its last frame. `idle()` waits for the latest revision only when `live`.
 */
export const viewportStore = new Store<{ live: boolean; drawnRev: number | null }>({ live: false, drawnRev: null });

function clock(): string {
  return new Date().toLocaleTimeString('en-GB', { hour12: false });
}

export function log(tag: LineClass, text: string): void {
  consoleStore.set([...consoleStore.get(), { time: clock(), tag, text }]);
}

/** Appends several lines at once (one render). */
export function logAll(lines: readonly { class: LineClass; text: string }[]): void {
  if (lines.length === 0) return;
  const time = clock();
  consoleStore.set([...consoleStore.get(), ...lines.map((l) => ({ time, tag: l.class, text: l.text }))]);
}
