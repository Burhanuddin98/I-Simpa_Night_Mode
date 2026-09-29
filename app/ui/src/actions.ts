// The M10 actions (PLAN.md 2.2): the only code that calls the M10 commands. The menus, the
// packages and the e2e hooks all go through these, so a hook exercises exactly what a click
// does, minus the native dialog.
//
// Every response replaces `sceneStore` whole; its Console lines are appended; the mesh is
// refetched when `info.geometry_rev` moved. A failure is logged as a FAIL line with its code and
// rethrown, so a caller that must know (a hook, a dialog) can; UI handlers use `fire`.
import { open, save as saveDialog } from '@tauri-apps/plugin-dialog';
import { asCmdError, backend, type EditOutcome, type SceneState, type Unit, type Up } from './backend';
import type { UiIssue } from './bindings/ipc';
import type { Op } from './bindings/schema';
import { decodeMesh } from './mesh';
import { addReceiver, addSource, newReceiver, newSource, nextName, type Vec3 } from './ops';
import { importRequestStore, log, logAll, meshStore, refusalStore, sceneStore } from './store';

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

export async function newProject(name = 'Untitled'): Promise<SceneState> {
  refusalStore.set(new Map());
  return run('Could not create a project', async () => accept(await backend.sceneNew(name)));
}

export async function openProject(path: string): Promise<SceneState> {
  refusalStore.set(new Map());
  return run(`Could not open ${path}`, async () => accept(await backend.sceneOpen(path)));
}

export async function importModel(path: string, unit: Unit, up: Up): Promise<SceneState> {
  refusalStore.set(new Map());
  return run(`Could not import ${path}`, async () => accept(await backend.modelImport(path, unit, up)));
}

const MESH_EXTENSIONS = ['ply', 'obj', 'stl'];

/** Opens a file by path: a `.simpa` directly, a mesh through the import dialog (unit and up
 * axis are the user's to confirm, never guessed). */
export async function openPath(path: string): Promise<void> {
  const ext = path.split('.').pop()?.toLowerCase() ?? '';
  if (MESH_EXTENSIONS.includes(ext)) importRequestStore.set({ path });
  else await openProject(path);
}

/** File › Open…: the native dialog, then `openPath`. */
export async function openDialog(): Promise<void> {
  const path = await open({
    multiple: false,
    directory: false,
    filters: [
      { name: 'Project or room model', extensions: ['simpa', ...MESH_EXTENSIONS] },
      { name: 'Night Mode project', extensions: ['simpa'] },
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
  return run(`Could not save ${target}`, async () => accept(await backend.projectSave(target)));
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
