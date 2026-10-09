// Typed calls to the app's `window.__m10` test hooks (app/ui/src/testhooks.ts, PLAN.md 2.5).
// Every call that changes the project waits for `idle()` before it returns, so the next DOM read
// sees the settled state.
import type { Op } from './types.ts';

/** Calls `window.__m10[name](...args)` in the page and returns its (awaited) result. */
export async function hook<T>(name: string, ...args: unknown[]): Promise<T> {
  const out = await browser.execute(
    async (n: string, a: unknown[]) => {
      const api = (window as unknown as { __m10?: Record<string, (...x: unknown[]) => unknown> }).__m10;
      const fn = api?.[n];
      if (typeof fn !== 'function') return { __hookError: `window.__m10.${n} is not registered` };
      try {
        return { __hookValue: await fn(...a) };
      } catch (e) {
        return { __hookError: String(e instanceof Error ? e.message : e) };
      }
    },
    name,
    args,
  );
  const r = out as { __hookValue?: T; __hookError?: string };
  if (r.__hookError !== undefined) throw new Error(`__m10.${name}: ${r.__hookError}`);
  return r.__hookValue as T;
}

/** Waits until every named hook is registered (the app booted and its components mounted). */
export async function waitForHooks(names: string[], timeout = 60_000): Promise<void> {
  await browser.waitUntil(async () => hook<boolean>('ready', names).catch(() => false), {
    timeout,
    interval: 200,
    timeoutMsg: `hooks not registered within ${timeout} ms: ${names.join(', ')}`,
  });
}

async function settled<T>(value: Promise<T>): Promise<T> {
  const v = await value;
  await hook('idle');
  return v;
}

export interface Refusal {
  code: string;
  rule: string;
  path: string;
  field: string;
  message: string;
}

export const m10 = {
  idle: () => hook<true>('idle'),
  openProject: (path: string) => settled(hook('openProject', path)),
  importModel: (path: string, unit = 'm', up = 'z') => settled(hook('importModel', path, unit, up)),
  saveAs: (path: string) => settled(hook('saveAs', path)),
  edit: (op: Op) => settled(hook<{ applied: boolean; refusals: Refusal[] }>('edit', op)),
  undo: () => settled(hook('undo')),
  redo: () => settled(hook('redo')),
  undoDepth: () => hook<number>('undoDepth'),
  projectJson: () => hook<string>('projectJson'),
  runBlockers: () => hook<string[]>('runBlockers'),
  dirty: () => hook<boolean>('dirty'),
  setStep: (key: string) => settled(hook('setStep', key)),
};

/**
 * Opens the materials table, which has its own window since the 2026-10-09 GUI audit (MaterialsSheet.tsx), from the
 * Materials step, and waits for the grid's and the library's hooks. The window sits over the middle of the app:
 * `closeMaterialsTable` closes it before the scene list or the view under it is clicked.
 */
export async function openMaterialsTable(): Promise<void> {
  if (!(await $('[data-materials-sheet]').isExisting())) await (await $('[data-action="edit-materials"]')).click();
  await waitForHooks(['materialsGrid', 'materialsCopy', 'materialLibrary']);
}

export async function closeMaterialsTable(): Promise<void> {
  const close = await $('[data-action="close-materials"]');
  if (await close.isExisting()) await close.click();
}
