// Test hooks for the M10 and M11 e2e harness (M10 PLAN.md 2.5, M11 PLAN.md 3.5), on
// `window.__m10` only when the app was started with `--e2e`. What a gate says is "shown" is read from the DOM; the hooks cover what
// the DOM cannot show, and drive the same actions the menus call, minus the native dialogs that
// WebDriver cannot reach.
//
// Each package registers its own hooks when its component mounts (`registerHook` returns the
// unregister function for the effect's cleanup).
import * as actions from './actions';
import type { Unit, Up } from './backend';
import type { Op } from './bindings/schema';
import { joinBlockers } from './flow';
import { STEPS, type StepKey } from './steps';
import {
  busyStore,
  meshStore,
  promptStore,
  resultsStore,
  runLinesStore,
  runsStore,
  runStore,
  sceneStore,
  solversStatusStore,
  type SolverName,
  stepStore,
  viewportStore,
} from './store';

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
    // What the Run button's data-blockers shows: the project's, the solvers' and RUN_ACTIVE.
    runBlockers: () => joinBlockers(sceneStore.get()?.run_blockers ?? null, solversStatusStore.get(), runStore.get() !== null) ?? [],
    dirty: () => sceneStore.get()?.info.dirty ?? false,
    setStep: (key: StepKey) => {
      if (!STEPS.some((s) => s.key === key)) throw new Error(`setStep: no step '${key}'`);
      stepStore.set(key);
      return true;
    },
  };
}

/** The run named `run`, or the next run to start when `run` is null, has reached `until`. */
type RunMilestone = 'started' | 'solve' | 'progress' | 'ended';

function runReached(name: string, until: RunMilestone): boolean {
  const active = runStore.get();
  const ended = runsStore.get()?.rows.some((r) => r.run === name && r.status !== 'RUNNING') ?? false;
  const log = runLinesStore.get().get(name);
  switch (until) {
    case 'started':
      return log !== undefined;
    case 'solve':
      return ended || (active?.run === name && active.stage === 'solve');
    case 'progress':
      return ended || (active?.run === name && active.progress !== null);
    case 'ended':
      return ended && active?.run !== name;
  }
}

/**
 * Resolves with the run's name once it has reached `until`: `started` (its folder exists),
 * `solve` (the solver was launched), `progress` (SPPS printed a `#` line) or `ended` (its
 * `run.json` is written and listed). `null` means the active run, else the next one to start.
 * Rejects after `ms`.
 */
async function waitRun(run: string | null, until: RunMilestone, ms = 180_000): Promise<string> {
  const t0 = performance.now();
  const before = new Set(runLinesStore.get().keys());
  let name = run;
  for (;;) {
    if (name === null) {
      // The active run, else the first run that started after this call.
      const fresh = [...runLinesStore.get().keys()].find((k) => !before.has(k));
      name = runStore.get()?.run ?? fresh ?? null;
    }
    if (name !== null && runReached(name, until)) return name;
    if (performance.now() - t0 > ms) {
      throw new Error(`waitRun(${run}, ${until}): not reached after ${ms} ms (run ${name}, stage ${runStore.get()?.stage})`);
    }
    await sleep(50);
  }
}

/** `n` sequential `ping` round trips, each timed in the page (gate (b)). */
async function pingStats(n: number): Promise<number[]> {
  const out: number[] = [];
  for (let i = 0; i < n; i++) {
    const t0 = performance.now();
    await actions.ping();
    out.push(performance.now() - t0);
  }
  return out;
}

/** The intervals between animation frames from `begin` to `end` (gate (b)). */
const frames = { on: false, last: 0, intervals: [] as number[] };
function frameLoop(t: number): void {
  if (!frames.on) return;
  if (frames.last) frames.intervals.push(t - frames.last);
  frames.last = t;
  requestAnimationFrame(frameLoop);
}

/** The M11 foundation's hooks (PLAN.md 3.5). */
function runHooks(): Record<string, Hook> {
  return {
    runStart: (solver: SolverName) => actions.runStart(solver),
    runCancel: () => actions.runCancel(),
    runState: () => {
      const r = runStore.get();
      return r ? { ...r } : null;
    },
    waitRun: (run: string | null, until: RunMilestone, ms?: number) => waitRun(run, until, ms),
    runLog: (run: string) => {
      const log = runLinesStore.get().get(run);
      if (!log) return null;
      const seqs = [...log.seqs];
      return {
        counts: { ...log.counts },
        n: log.seqs.size,
        min: seqs.length ? Math.min(...seqs) : null,
        max: seqs.length ? Math.max(...seqs) : null,
        dupes: log.dupes,
        gaps: log.gaps,
      };
    },
    runsRows: async () => (await actions.refreshRuns())?.rows ?? [],
    selectRun: (run: string | null) => {
      actions.selectRun(run);
      return true;
    },
    resultsState: async (run: string) => {
      await actions.resultsFor(run);
      return resultsStore.get().get(run) ?? null;
    },
    pingStats: (n: number) => pingStats(n),
    'frameStats.begin': () => {
      frames.on = true;
      frames.last = 0;
      frames.intervals = [];
      requestAnimationFrame(frameLoop);
      return true;
    },
    'frameStats.end': () => {
      frames.on = false;
      return [...frames.intervals];
    },
    pid: () => actions.startupPid(),
    solversStatus: () => actions.refreshSolvers(),
    openProj: async (path: string) => {
      await actions.importProj(path);
      return idle();
    },
    // Opens a path the way File › Open… does after its dialog: the save prompt first.
    openPath: async (path: string) => {
      await actions.openPath(path);
      return idle();
    },
    newProject: async () => {
      await actions.newProject();
      return idle();
    },
    promptOpen: () => {
      const p = promptStore.get();
      return p ? { name: p.name } : null;
    },
  };
}

/** Installs `window.__m10` (once): the foundation's hooks plus whatever packages register. */
export function installTestHooks(): void {
  if (installed) return;
  installed = true;
  for (const [name, fn] of Object.entries({ ...foundationHooks(), ...runHooks() })) hooks.set(name, fn);
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
