// The M10 and M11 actions (M10 PLAN.md 2.2, M11 PLAN.md 3.2): the only code that calls the M10
// and M11 commands. The menus, the packages and the e2e hooks all go through these, so a hook
// exercises exactly what a click does, minus the native dialog.
//
// Every response replaces `sceneStore` whole; its Console lines are appended; the mesh is
// refetched when `info.geometry_rev` moved. A failure is logged as a FAIL line with its code and
// rethrown, so a caller that must know (a hook, a dialog) can; UI handlers use `fire`.
import { open, save as saveDialog } from '@tauri-apps/plugin-dialog';
import {
  asCmdError,
  backend,
  Channel,
  type AppEvent,
  type EditOutcome,
  type LibraryMaterial,
  type ResultsState,
  type RunStarted,
  type RunStreamBatch,
  type RunsView,
  type SceneState,
  type SolversStatus,
  type Unit,
  type Up,
} from './backend';
import type { UiIssue } from './bindings/ipc';
import type { Op, ReflectionLaw } from './bindings/schema';
import { emptyLog, endLine, foldEvent, needsSavePrompt, progressText } from './flow';
import { decodeMesh } from './mesh';
import {
  addMaterial,
  addReceiver,
  addSource,
  libraryMaterial,
  newReceiver,
  newSource,
  nextName,
  replaceMaterial,
  setSourceEnabled as setSourceEnabledOp,
  type Vec3,
  withLaw,
} from './ops';
import {
  appendLines,
  type ConsoleLine,
  importRequestStore,
  libraryStore,
  log,
  logAll,
  meshStore,
  promptStore,
  type PromptChoice,
  refusalStore,
  resultsStore,
  type RunLog,
  runLinesStore,
  runsStore,
  runStore,
  sceneStore,
  selectedRunStore,
  type SolverName,
  solversStatusStore,
  solverStore,
} from './store';

/** A rejected action's `{code, message}`, for a package that shows it inline (packages never
 * import backend.ts, PLAN.md 2.4 rule 7). */
export { asCmdError } from './backend';

/** Runs an action from a UI handler: its failure is already in the Console. */
export function fire(p: Promise<unknown>): void {
  p.catch(() => {});
}

async function fetchMesh(state: SceneState): Promise<void> {
  const rev = state.info.geometry_rev;
  if (meshStore.get()?.geometryRev === rev) return;
  const mesh = decodeMesh(await backend.sceneMesh());
  // A later response may have moved on while this one was in flight.
  if (sceneStore.get()?.info.geometry_rev === mesh.geometryRev) meshStore.set(mesh);
}

/** Takes a response: the state, its lines, and the mesh if the geometry changed. */
async function accept(state: SceneState): Promise<SceneState> {
  sceneStore.set(state);
  logAll(state.lines);
  await fetchMesh(state);
  return state;
}

async function run<T>(what: string, call: () => Promise<T>): Promise<T> {
  try {
    return await call();
  } catch (e) {
    const err = asCmdError(e);
    log('FAIL', `${what}: ${err.message} (${err.code})`);
    throw e;
  }
}

/** The state after startup: a `--project` opened at launch, with its check lines. */
export async function loadInitialState(): Promise<void> {
  const state = await run('Reading the project', () => backend.sceneState());
  if (state) await accept(state);
}

/** A new, empty project, after the save prompt (row 22, A9); `null` when the user cancelled or
 * a run is active. */
export async function newProject(name = 'Untitled'): Promise<SceneState | null> {
  if (refuseDuringRun('New project')) return null;
  if (!(await confirmDiscard())) return null;
  refusalStore.set(new Map());
  const state = await run('Could not create a project', async () => accept(await backend.sceneNew(name)));
  fire(refreshRuns());
  return state;
}

/** Opens a `.simpa` (no prompt: callers that leave the project go through `openPath`). */
export async function openProject(path: string): Promise<SceneState> {
  refusalStore.set(new Map());
  const state = await run(`Could not open ${path}`, async () => accept(await backend.sceneOpen(path)));
  selectedRunStore.set(null);
  resultsStore.set(new Map());
  fire(refreshRuns());
  return state;
}

export async function importModel(path: string, unit: Unit, up: Up): Promise<SceneState> {
  refusalStore.set(new Map());
  const state = await run(`Could not import ${path}`, async () => accept(await backend.modelImport(path, unit, up)));
  selectedRunStore.set(null);
  fire(refreshRuns());
  return state;
}

const MESH_EXTENSIONS = ['ply', 'obj', 'stl'];

/** Opens a file by path, after the save prompt: a `.simpa` directly, an upstream `.proj` as a
 * new unsaved project (it carries its own units), a mesh through the import dialog (unit and up
 * axis are the user's to confirm, never guessed). Nothing opens while a run is active. */
export async function openPath(path: string): Promise<void> {
  if (refuseDuringRun('Open')) return;
  if (!(await confirmDiscard())) return;
  const ext = path.split('.').pop()?.toLowerCase() ?? '';
  if (MESH_EXTENSIONS.includes(ext)) importRequestStore.set({ path });
  else if (ext === 'proj') await importProj(path);
  else await openProject(path);
}

/** File › Open…: the native dialog, then `openPath`. */
export async function openDialog(): Promise<void> {
  if (refuseDuringRun('Open')) return;
  const path = await open({
    multiple: false,
    directory: false,
    filters: [
      { name: 'Project or room model', extensions: ['simpa', 'proj', ...MESH_EXTENSIONS] },
      { name: 'Night Mode project', extensions: ['simpa'] },
      { name: 'I-Simpa project', extensions: ['proj'] },
      { name: 'Room model', extensions: MESH_EXTENSIONS },
    ],
  });
  if (typeof path === 'string') await openPath(path);
}

/** Save as: to `path`, or to a path the native dialog asks for. */
export async function saveAs(path?: string): Promise<SceneState | null> {
  const target =
    path ??
    (await saveDialog({
      defaultPath: `${sceneStore.get()?.info.name ?? 'Untitled'}.simpa`,
      filters: [{ name: 'Night Mode project', extensions: ['simpa'] }],
    }));
  if (typeof target !== 'string') return null;
  const state = await run(`Could not save ${target}`, async () => accept(await backend.projectSave(target)));
  // Another folder is another runs root.
  fire(refreshRuns());
  return state;
}

/** Save: to the project's own path, or Save as for a project never saved. */
export async function save(): Promise<SceneState | null> {
  const state = sceneStore.get();
  if (!state) return null;
  if (!state.info.path) return saveAs();
  return run('Could not save', async () => accept(await backend.projectSave(null)));
}

function fileRefusals(fieldKey: string | undefined, outcome: EditOutcome): void {
  if (!fieldKey) return;
  const next = new Map(refusalStore.get());
  if (outcome.applied) next.delete(fieldKey);
  else next.set(fieldKey, outcome.refusals);
  refusalStore.set(next);
}

/**
 * The checked apply. A refusal is `applied: false` with `refusals`, filed under `fieldKey`
 * (issues.ts) for the field to show; the next accepted edit under the same key clears it.
 */
export async function apply(op: Op, fieldKey?: string): Promise<EditOutcome> {
  return run('Edit failed', async () => {
    const outcome = await backend.editApply(op);
    await accept(outcome.state);
    fileRefusals(fieldKey, outcome);
    return outcome;
  });
}

/** Undoes one edit. Nothing to undo, or no project, is not an error. */
export async function undo(): Promise<SceneState | null> {
  if (!sceneStore.get()) return null;
  return run('Undo failed', async () => accept(await backend.editUndo()));
}

export async function redo(): Promise<SceneState | null> {
  if (!sceneStore.get()) return null;
  return run('Redo failed', async () => accept(await backend.editRedo()));
}

/** The project in its canonical file form, for the e2e hooks. */
export async function projectJson(): Promise<string> {
  return run('Reading the project failed', () => backend.projectJson());
}

/** Receivers sit 1.2 m above a floor hit (ear height), sources 1.5 m (PLAN.md 6.1). */
export const PLACE_HEIGHT_M = { receiver: 1.2, source: 1.5 } as const;

/**
 * Adds a receiver or a source at `point` (already lifted by the caller), at the end of its list,
 * named `R<n>` or `S<n>` with the first free n. A point outside the room is refused by the
 * checked apply and the project is left unchanged.
 */
export async function placeAt(kind: 'receiver' | 'source', point: Vec3): Promise<EditOutcome> {
  const view = sceneStore.get()?.view;
  if (!view) throw new Error('placeAt: no project is open');
  const id = crypto.randomUUID();
  if (kind === 'receiver') {
    const name = nextName('R', view.point_receivers.map((r) => r.name));
    return apply(addReceiver(view.point_receivers.length, newReceiver(id, name, point)), `point_receiver:new:position`);
  }
  const name = nextName('S', view.sources.map((s) => s.name));
  return apply(addSource(view.sources.length, newSource(id, name, point)), `source:new:position`);
}

/** The issues of the current state and the refusals of one field, for inline messages. */
export function refusalsFor(fieldKey: string): UiIssue[] {
  return refusalStore.get().get(fieldKey) ?? [];
}

// ---- M11 (docs/investigations/2026-09-29-m11/PLAN.md 3.2) -------------------------------------
// The run, the Runs tab, the results state, the solvers, the library, the save prompt and the
// close request. Still the only code that calls the backend.

/** The next run's `ActiveRun.id`. */
let nextRunId = 1;

/**
 * A Run accepted and not yet answered by `run_start`: set before `runStart`'s first await (the
 * save, the solvers' check) and cleared once `run_start` has answered. `runStore` is set only
 * after those awaits, so without it a second Run in that window (a double click, F5 held down,
 * the menu) passed the check, reached `run_start`, and its RUN_ACTIVE refusal cleared the first
 * run's state while that run went on (M11 review 2, M3 and E1).
 */
let starting = false;

/** New, Open, Import and Run wait while a run is starting or active (PQ4): the run would be
 * orphaned from the project the Runs tab lists. */
function refuseDuringRun(what: string): boolean {
  if (runStore.get() !== null) {
    log('INFO', `${what}: a run is active: cancel it first`);
    return true;
  }
  if (starting) {
    log('INFO', `${what}: a run is starting`);
    return true;
  }
  return false;
}

/**
 * Before New, Open and Exit (row 22, A9): when the project holds something to lose, asks
 * "Save changes to <name>?" through `promptStore` (the dialog answers). Save runs Save, or Save
 * as for a project never saved, and goes on only if that saved; Don't save goes on; Cancel stops.
 * `true` when the caller may go on.
 */
export async function confirmDiscard(): Promise<boolean> {
  const info = sceneStore.get()?.info ?? null;
  if (!needsSavePrompt(info)) return true;
  if (promptStore.get()) return false;
  const choice = await new Promise<PromptChoice>((resolve) => {
    promptStore.set({
      name: info?.name ?? 'Untitled',
      resolve: (c) => {
        promptStore.set(null);
        resolve(c);
      },
    });
  });
  if (choice === 'cancel') return false;
  if (choice === 'discard') return true;
  const saved = await save().catch(() => null);
  return saved !== null && !(sceneStore.get()?.info.dirty ?? false);
}

/** The solvers' status, refreshed at boot and before each run. */
export async function refreshSolvers(): Promise<SolversStatus | null> {
  try {
    const status = await backend.solversStatus();
    solversStatusStore.set(status);
    return status;
  } catch (e) {
    const err = asCmdError(e);
    log('FAIL', `Checking the solvers: ${err.message} (${err.code})`);
    return null;
  }
}

/** Upstream's reference materials, from the core, once at boot. */
export async function loadLibrary(): Promise<void> {
  const lib = await run('Reading the material library', () => backend.materialLibrary());
  libraryStore.set(lib);
}

/** The Runs tab's rows for the open project. */
export async function refreshRuns(): Promise<RunsView | null> {
  if (!sceneStore.get()) {
    runsStore.set(null);
    return null;
  }
  const view = await run('Listing the runs', () => backend.runsList());
  runsStore.set(view);
  if (selectedRunStore.get() === null && view.rows.length > 0) selectedRunStore.set(view.rows[view.rows.length - 1].run);
  return view;
}

/** Shows `run` on the Results step (a Runs row click, the Simulate step's "Run n" link). */
export function selectRun(run: string | null): void {
  selectedRunStore.set(run);
}

/** Whether `run`'s results verify, fetched once per run; never a value. */
export async function resultsFor(runName: string): Promise<ResultsState> {
  const cached = resultsStore.get().get(runName);
  if (cached) return cached;
  const state = await run(`Checking the results of ${runName}`, () => backend.runResults(runName));
  const next = new Map(resultsStore.get());
  next.set(runName, state);
  resultsStore.set(next);
  return state;
}

/**
 * The run stream's handler for the run with `ActiveRun.id` `id`, the one writer of
 * `runLinesStore` and a run's Console lines. Each event is filed under the run this channel
 * named in its own `started` event, never under whatever `runStore` holds, and `runStore` is
 * updated only while it still holds this run (M11 review 2, E1). A batch is folded locally and
 * each store set once, so thousands of PROGRESS lines cost one render per batch.
 */
function runEvents(id: number): (batch: RunStreamBatch) => void {
  let runName: string | undefined;
  return (batch) => {
    const lines: Omit<ConsoleLine, 'time'>[] = [];
    const held = runStore.get();
    let current = held?.id === id ? held : null;
    let logs: Map<string, RunLog> | null = null;
    let ended: string | null = null;
    let finished = false;
    for (const e of batch.events) {
      if (e.kind === 'started') runName = e.run;
      if (runName !== undefined) {
        logs ??= new Map(runLinesStore.get());
        const f = foldEvent(logs.get(runName) ?? emptyLog(), runName, e);
        logs.set(runName, f.log);
        if (f.line) lines.push(f.line);
      }
      switch (e.kind) {
        case 'started':
          if (current) current = { ...current, run: e.run, status: current.status === 'cancelling' ? 'cancelling' : 'running' };
          break;
        case 'stage':
          if (current) current = { ...current, stage: e.stage };
          break;
        case 'line':
          if (current && e.class === 'PROGRESS' && e.source === 'solver') {
            current = { ...current, progress: e.progress ?? null, progressText: progressText(e.text) };
          }
          break;
        case 'ended':
          current = null;
          lines.push(endLine(e.row));
          ended = e.row.run;
          finished = true;
          break;
        case 'failed':
          current = null;
          lines.push({ tag: 'FAIL', text: `The run could not record itself: ${e.error.message} (${e.error.code})`, source: 'app' });
          finished = true;
          break;
      }
    }
    if (logs) runLinesStore.set(logs);
    if (runStore.get()?.id === id) runStore.set(current);
    appendLines(lines);
    if (ended !== null) selectRun(ended);
    if (finished) fire(refreshRuns());
  };
}

/**
 * Run (PQ1: the run is of what is on screen, so it saves first): with no path, Save as (a
 * cancelled dialog runs nothing); with unsaved changes, Save. Then the solvers are checked and
 * the run starts; its events arrive through the channel. Returns what `run_start` answered, or
 * `null` when nothing was started.
 */
export async function runStart(solver: SolverName = solverStore.get()): Promise<RunStarted | null> {
  if (refuseDuringRun('Run')) return null;
  const state = sceneStore.get();
  if (!state) return null;
  starting = true;
  try {
    if (!state.info.path || state.info.dirty) {
      const saved = await save();
      if (!saved || saved.info.dirty || !saved.info.path) return null;
      log('INFO', `Saved ${saved.info.path} before the run`);
    }
    await refreshSolvers();
    const info = sceneStore.get()?.info;
    const id = nextRunId++;
    runStore.set({
      id,
      solver,
      variant: info?.active_variant ?? null,
      stage: null,
      progress: null,
      progressText: '',
      startedAt: Date.now(),
      status: 'starting',
    });
    const channel = new Channel<RunStreamBatch>();
    channel.onmessage = runEvents(id);
    try {
      const started = await backend.runStart(solver, channel);
      log('INFO', `${solver.toUpperCase()} run started: ${started.project_path}`);
      return started;
    } catch (e) {
      // This run's state only: never a run that another start put there.
      if (runStore.get()?.id === id) runStore.set(null);
      const err = asCmdError(e);
      log('FAIL', `Could not start the run: ${err.message} (${err.code})`);
      throw e;
    }
  } finally {
    starting = false;
  }
}

/** Cancel: the core stops the run and ends the solver's Job Object; the row reads Cancelled. */
export async function runCancel(): Promise<boolean> {
  const current = runStore.get();
  if (current) runStore.set({ ...current, status: 'cancelling' });
  return run('Could not cancel the run', () => backend.runCancel());
}

/** Opens an upstream `.proj` (no unit dialog: it carries its own units). Not saved: Save as
 * follows. A refusal is a FAIL line with its code, and the project is unchanged. */
export async function importProj(path: string): Promise<SceneState> {
  refusalStore.set(new Map());
  const state = await run(`Could not open ${path}`, async () => accept(await backend.projImport(path)));
  fire(refreshRuns());
  return state;
}

/** Adds a library material to the project (row 22, M1): one `add_material`, one undo step. */
export async function addFromLibrary(entry: LibraryMaterial): Promise<EditOutcome> {
  const view = sceneStore.get()?.view;
  if (!view) throw new Error('addFromLibrary: no project is open');
  const m = libraryMaterial(entry, crypto.randomUUID(), view.bands.frequencies_hz.length);
  return apply(addMaterial(view.materials.length, m), `material:new:library`);
}

/** Switches a source on or off (row 22, M26). */
export async function setSourceEnabled(id: string, enabled: boolean): Promise<EditOutcome> {
  return apply(setSourceEnabledOp(id, enabled), `source:${id}:enabled`);
}

/** Sets one reflection law for every band of a material (row 22, M5): one undo step. */
export async function setLaw(materialId: string, law: ReflectionLaw): Promise<EditOutcome> {
  const m = sceneStore.get()?.view.materials.find((x) => x.id === materialId);
  if (!m) throw new Error(`setLaw: no material ${materialId}`);
  return apply(replaceMaterial(withLaw(m, law)), `material:${materialId}:reflection_law`);
}

/** The close request (the window's close button, Alt+F4, WM_CLOSE): the save prompt, then quit,
 * unless the answer is Cancel. The backend cancels an active run before it exits (PQ4). */
async function onCloseRequested(): Promise<void> {
  if (promptStore.get()) return;
  if (!(await confirmDiscard())) return;
  await run('Could not quit', () => backend.appQuit());
}

/**
 * A channel for the backend's app events. A close request is acknowledged at once by registering
 * a fresh channel: the backend takes a request nobody acknowledged within 5 s for a hung UI and
 * closes the window past the save prompt, so without the acknowledgement a second click on the
 * close button (the prompt open, or just cancelled) would lose unsaved work. The channel is
 * fresh because re-sending one restarts its message index on the Rust side, and this side would
 * then wait for an index that never comes.
 */
function appEventsChannel(): Channel<AppEvent> {
  const channel = new Channel<AppEvent>();
  channel.onmessage = (e) => {
    if (e.kind !== 'close_requested') return;
    fire(run('Acknowledging the close request', () => backend.appEvents(appEventsChannel())));
    fire(onCloseRequested());
  };
  return channel;
}

/** At boot: the channel the backend sends the close request on. */
export async function listenAppEvents(): Promise<void> {
  await run('Registering the app events', () => backend.appEvents(appEventsChannel()));
}

let pid: number | null = null;

/** This process's id, from `app_startup` (the gate's WM_CLOSE and Stop-Process use it). */
export async function startupPid(): Promise<number> {
  if (pid === null) pid = (await run('Reading the startup info', () => backend.startup())).pid;
  return pid;
}

/** One IPC round trip, for the gate's latency measurement. */
export async function ping(): Promise<string> {
  return backend.ping();
}
