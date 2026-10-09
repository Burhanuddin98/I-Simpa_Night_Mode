// A7 recent projects, kept in this browser profile only (a convenience, as the panel folds are,
// fold.tsx): a fresh profile has no recent project.
import { Store } from '../store';
import { readRecent, withoutRecent, withRecent } from './recentModel';

const KEY = 'nm-recent';

function load<T>(key: string, read: (v: unknown) => T, none: T): T {
  try {
    return read(JSON.parse(localStorage.getItem(key) ?? 'null'));
  } catch {
    return none;
  }
}

function keep(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Storage refused: the list still holds for this session.
  }
}

/** A7: the project files opened or saved here, newest first. */
export const recentStore = new Store<string[]>(load(KEY, readRecent, []));

/** Notes `path` as the newest recent project. */
export function noteRecent(path: string | null | undefined): void {
  if (!path) return;
  const next = withRecent(recentStore.get(), path);
  recentStore.set(next);
  keep(KEY, next);
}

/** Takes `path` off the recent list (its entry's ×). */
export function forgetRecent(path: string): void {
  const next = withoutRecent(recentStore.get(), path);
  recentStore.set(next);
  keep(KEY, next);
}
