// Test hooks for the M10 e2e harness (PLAN.md 2.5), on `window.__m10` only when the app was
// started with `--e2e`. What a gate says is "shown" is read from the DOM; the hooks cover what
// the DOM cannot show, and drive the same actions the menus call, minus the native dialogs that
// WebDriver cannot reach.
//
// Each package registers its own hooks when its component mounts (`registerHook` returns the
// unregister function for the effect's cleanup).
import * as actions from './actions';
import type { Unit, Up } from './backend';
import type { Op } from './bindings/schema';
import { STEPS, type StepKey } from './steps';
import { busyStore, meshStore, sceneStore, stepStore, viewportStore } from './store';

type Hook = (...args: never[]) => unknown;

const hooks = new Map<string, Hook>();
let installed = false;

/** Registers `fn` as `window.__m10[name]`. Returns the unregister function. */
export function registerHook(name: string, fn: Hook): () => void {
  hooks.set(name, fn);
  return () => {
    if (hooks.get(name) === fn) hooks.delete(name);
  };
}

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

function settled(): boolean {
  if (busyStore.get() !== 0) return false;
  const rev = sceneStore.get()?.info.geometry_rev;
  if (rev === undefined) return true;
  if (meshStore.get()?.geometryRev !== rev) return false;
  const vp = viewportStore.get();
  return !vp.live || vp.drawnRev === rev;
}

/**
 * Resolves when no command is in flight, the mesh of the latest geometry revision is decoded
 * and, once a real viewport has mounted, drawn. Two frames must agree, so a response that
 * lands between them is waited for too. Rejects after `timeoutMs`.
 */
export async function idle(timeoutMs = 60_000): Promise<true> {
  const t0 = performance.now();
  let calm = 0;
  while (calm < 2) {
    if (performance.now() - t0 > timeoutMs) {
      throw new Error(
        `idle(): not settled after ${timeoutMs} ms (busy ${busyStore.get()}, ` +
          `rev ${sceneStore.get()?.info.geometry_rev}, mesh ${meshStore.get()?.geometryRev}, ` +
          `drawn ${viewportStore.get().drawnRev})`,
      );
    }
    calm = settled() ? calm + 1 : 0;
    await new Promise<void>((r) => requestAnimationFrame(() => r()));
    if (calm === 0) await sleep(10);
  }
  return true;
}

/** The foundation's hooks (PLAN.md 2.5). */
function foundationHooks(): Record<string, Hook> {
  return {
    ready: (names: string[]) => names.every((n) => hooks.has(n)),
    idle: (timeoutMs?: number) => idle(timeoutMs),
    openProject: async (path: string) => {
      await actions.openProject(path);
      return idle();
    },
    importModel: async (path: string, unit: Unit, up: Up) => {
      await actions.importModel(path, unit, up);
      return idle();
    },
    saveAs: async (path: string) => {
      await actions.saveAs(path);
      return idle();
    },
    edit: async (op: Op) => {
      const outcome = await actions.apply(op);
      await idle();
      return { applied: outcome.applied, refusals: outcome.refusals };
    },
    undo: async () => {
      await actions.undo();
      return idle();
    },
    redo: async () => {
      await actions.redo();
      return idle();
    },
    undoDepth: () => sceneStore.get()?.info.undo_depth ?? 0,
    projectJson: () => actions.projectJson(),
    runBlockers: () => sceneStore.get()?.run_blockers ?? [],
    dirty: () => sceneStore.get()?.info.dirty ?? false,
    setStep: (key: StepKey) => {
      if (!STEPS.some((s) => s.key === key)) throw new Error(`setStep: no step '${key}'`);
      stepStore.set(key);
      return true;
    },
  };
}

/** Installs `window.__m10` (once): the foundation's hooks plus whatever packages register. */
export function installTestHooks(): void {
  if (installed) return;
  installed = true;
  for (const [name, fn] of Object.entries(foundationHooks())) hooks.set(name, fn);
  const api = new Proxy(
    {},
    {
      get: (_, name) => (typeof name === 'string' ? hooks.get(name) : undefined),
      has: (_, name) => typeof name === 'string' && hooks.has(name),
      ownKeys: () => [...hooks.keys()],
      getOwnPropertyDescriptor: (_, name) =>
        typeof name === 'string' && hooks.has(name)
          ? { configurable: true, enumerable: true, value: hooks.get(name) }
          : undefined,
    },
  );
  Object.defineProperty(window, '__m10', { value: api, configurable: false, enumerable: false });
}
