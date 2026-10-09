// A7, upstream's File › Recent projects (`wxFileHistory`, five files, i_simpa_main.h:42): the
// project files this person opened or saved, newest first. Pure: recent.ts keeps the list in this
// browser profile, Landing.tsx and MenuBar.tsx show it, recentModel.test.ts holds the rules.

/** Upstream's count (`MAX_HISTORY_SHOW`). */
export const RECENT_MAX = 5;

/** Windows paths name one file whatever their case or slashes. */
const same = (a: string, b: string) => a.replace(/\//g, '\\').toLowerCase() === b.replace(/\//g, '\\').toLowerCase();

/** `list` with `path` first, once, at most `RECENT_MAX` long. */
export function withRecent(list: readonly string[], path: string): string[] {
  return [path, ...list.filter((p) => !same(p, path))].slice(0, RECENT_MAX);
}

/** `list` without `path`. */
export function withoutRecent(list: readonly string[], path: string): string[] {
  return list.filter((p) => !same(p, path));
}

/** A stored list read back: only strings, deduplicated, at most `RECENT_MAX`; anything else is none. */
export function readRecent(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.filter((p): p is string => typeof p === 'string' && p.length > 0).reduceRight<string[]>((acc, p) => withRecent(acc, p), []);
}

/**
 * A33, upstream's start on the last project (projet.cpp:1986-2000 reloads it at every start), here a
 * choice that is off until ticked, since the landing page with its examples is this app's start: the
 * project to open at start, or null. Only when the choice is on (off in a fresh profile, so a test
 * session starts on the landing page), nothing was opened from the command line, it is not the
 * self-test, and there is a recent project.
 */
export function projectAtStart(opts: { reopen: boolean; opened: boolean; selftest: boolean; recent: readonly string[] }): string | null {
  if (!opts.reopen || opts.opened || opts.selftest) return null;
  return opts.recent[0] ?? null;
}

/** A recent entry as a menu shows it: the file's name without `.simpa`, and its folder. */
export function recentLabel(path: string): { name: string; folder: string } {
  const cut = Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/'));
  const file = cut >= 0 ? path.slice(cut + 1) : path;
  return { name: file.replace(/\.simpa$/i, ''), folder: cut >= 0 ? path.slice(0, cut) : '' };
}
