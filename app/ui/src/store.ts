// State that lives outside React (the islands rule: React owns the chrome, stores feed it).
import { useSyncExternalStore } from 'react';
import type { LineClass } from './bindings/ipc';
import type { ProjectInfo } from './backend';

export class Store<T> {
  private listeners = new Set<() => void>();
  constructor(private value: T) {}
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
export const projectStore = new Store<ProjectInfo | null>(null);
export type Status = { kind: 'ready' | 'busy' | 'bad'; text: string };
export const statusStore = new Store<Status>({ kind: 'ready', text: 'Ready' });

function clock(): string {
  return new Date().toLocaleTimeString('en-GB', { hour12: false });
}

export function log(tag: LineClass, text: string): void {
  consoleStore.set([...consoleStore.get(), { time: clock(), tag, text }]);
}
