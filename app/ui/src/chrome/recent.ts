// A7 recent projects and A33's start choice, kept in this browser profile only (a convenience, as
// the panel folds are, fold.tsx): a fresh profile has no recent project and starts on the landing
// page.
import { Store } from '../store';
import { readRecent, withoutRecent, withRecent } from './recentModel';

const KEY = 'nm-recent';
const REOPEN_KEY = 'nm-reopen-last';

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

/** A33: open the newest recent project at start (off until ticked). */
export const reopenLastStore = new Store<boolean>(load(REOPEN_KEY, (v) => v === true, false));

export function setReopenLast(on: boolean): void {
  reopenLastStore.set(on);
  keep(REOPEN_KEY, on);
}

/** A33: why the project chosen to open at start did not open, for the landing page (the Console is behind it). */
export const reopenProblemStore = new Store<string | null>(null);

/** Takes `path` off the recent list (its entry's ×). */
export function forgetRecent(path: string): void {
  const next = withoutRecent(recentStore.get(), path);
  recentStore.set(next);
  keep(KEY, next);
}
