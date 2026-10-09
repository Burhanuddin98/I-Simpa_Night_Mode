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
  type MeshNowReport,
  type ReportView,
  type ResultsState,
  type RepairReport,
  type RunStarted,
  type RunStreamBatch,
  type RunsView,
  type SceneState,
  type SolversStatus,
  type Unit,
  type Up,
} from './backend';
import type { Setting, UiIssue } from './bindings/ipc';
import type { BandKind, Op, ReflectionLaw } from './bindings/schema';
import { regroupFaces } from './chrome/sceneModel';
import { mapOfGroup, surfaceMapPlan } from './chrome/groupsModel';
import { newBoxZone } from './chrome/zones';
import { emptyLog, endLine, foldEvent, needsSavePrompt, progressText } from './flow';
import { logProgress } from './features/simulate/runTime';
import { decodeMesh } from './mesh';
import { earPlane, PLANE_RESOLUTION_M, roomBox } from './chrome/planes';
import {
  addMaterial,
  addReceiver,
  addSource,
  addSurfaceReceiver,
  addFittingZone,
  mergeGroups,
  moveFaces,
  newCuttingPlane,
  rename,
  libraryMaterial,
  newReceiver,
  newSource,
  newSceneReceiver,
  nextName,
  replaceFittingZone,
  replaceMaterial,
  replaceSurfaceReceiver,
  setSourceEnabled as setSourceEnabledOp,
  type Vec3,
  withLaw,
} from './ops';
import {
  appendLines,
  type ConsoleLine,
  importRequestStore,
  libraryStore,
  spectrumLibraryStore,
  log,
  logAll,
  meshStore,
  promptStore,
  type PromptChoice,
  refusalStore,
  reportStore,
  resultsStore,
  type RunLog,
  runLinesStore,
  runsStore,
  runStore,
  sceneStore,
  selectedRunStore,
  selectionStore,
  type SolverName,
  solversStatusStore,
  solverStore,
  deviceStore,
  gpuStatusStore,
  type SppsDevice,
  repairStore,
  meshNowStore,
  fittingZonesStore,
  Store,
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
  forgetRuns();
  fire(refreshRuns());
  return state;
}

/**
 * The Results step's run and the verdicts checked so far belong to one project's runs root:
 * after New, an Open, an import, or a Save as into another folder, neither may be shown. Kept,
 * the Results step read "OK Results verified" for the previous project's run (M11 review 2,
 * app 3).
 */
function forgetRuns(): void {
  selectedRunStore.set(null);
  resultsStore.set(new Map());
  reportStore.set(new Map());
}

/** The folder of a project file: its runs root's parent. */
function folderOf(path: string | null | undefined): string | null {
  return path ? path.replace(/[\\/][^\\/]*$/, '').toLowerCase() : null;
}

/** Opens a `.simpa` (no prompt: callers that leave the project go through `openPath`). */
export async function openProject(path: string): Promise<SceneState> {
  refusalStore.set(new Map());
  const state = await run(`Could not open ${path}`, async () => accept(await backend.sceneOpen(path)));
  forgetRuns();
  fire(refreshRuns());
  return state;
}

/** The landing page's example `id` (src-tauri/src/examples.rs), after the save prompt: the core
 * copies it into the user's Documents and opens the copy as Open opens a `.simpa`. `null` when
 * the user cancelled or a run is active. */
export async function openExample(id: string): Promise<SceneState | null> {
  if (refuseDuringRun('Open')) return null;
  if (!(await confirmDiscard())) return null;
  refusalStore.set(new Map());
  const state = await run('Could not open the example', async () => accept(await backend.exampleOpen(id)));
  forgetRuns();
  fire(refreshRuns());
  return state;
}

export async function importModel(path: string, unit: Unit, up: Up, repairIfRefused = false): Promise<SceneState> {
  refusalStore.set(new Map());
  repairStore.set(null);
  const state = await run(`Could not import ${path}`, async () => accept(await backend.modelImport(path, unit, up)));
  forgetRuns();
  fire(refreshRuns());
  // G8: the import dialog's choice, upstream's "Repair model": only a model the check refuses is repaired.
  if (repairIfRefused && state.check?.verdict === 'refused') {
    const r = await repairModel();
    if (r) return r.outcome.state;
  }
  return state;
}

/**
 * G7: the mesh at `path` replaces the open project's model (upstream's re-import): each new face within
 * 1 cm of an old face keeps that face's surface group, and so its material; the others get new groups
 * (the core's `reassign`). One checked edit, one undo step; the Console says how many faces kept their
 * group. The project stays open, so nothing is discarded and no save prompt is needed.
 */
export async function reimportModel(path: string, unit: Unit, up: Up): Promise<EditOutcome | null> {
  if (refuseDuringRun('Re-import')) return null;
  repairStore.set(null);
  return run(`Could not re-import ${path}`, async () => {
    const outcome = await backend.modelReimport(path, unit, up);
    await accept(outcome.state);
    fileRefusals('model:reimport', outcome);
    return outcome;
  });
}

/** G7: "Replace model, keep groups…": the native dialog for a mesh file, then the import dialog in its re-import mode. */
export async function reimportDialog(): Promise<void> {
  if (refuseDuringRun('Re-import')) return;
  const path = await open({ multiple: false, directory: false, filters: [{ name: 'Room model', extensions: MESH_EXTENSIONS }] });
  if (typeof path === 'string') importRequestStore.set({ path, keepGroups: true });
}

/**
 * G8: Repair the open model: the core's safe fixes (weld vertices within 1 um, remove faces of zero area and
 * repeated faces, turn inward faces out; never holes or intersections). When it changes anything, the
 * repaired mesh is written as a new OBJ beside the original, never over it, and the project takes the
 * repaired geometry as one undo step, checked again. Null while a run is active.
 */
export async function repairModel(): Promise<RepairReport | null> {
  if (refuseDuringRun('Repair')) return null;
  return run('Could not repair the model', async () => {
    const r = await backend.modelRepair();
    await accept(r.outcome.state);
    repairStore.set(r);
    return r;
  });
}

/**
 * G32: Mesh now: TetGen on the open project with its own mesh settings, as a run would mesh it, into a scratch
 * folder on the temp drive. The report is kept with `basis` (the model and mesh settings it was made on) so the
 * panel shows it only while they still hold. Null while a run is active.
 */
export async function meshNow(basis: string): Promise<MeshNowReport | null> {
  if (refuseDuringRun('Mesh now')) return null;
  return run('Could not mesh the model', async () => {
    const r = await backend.meshNow();
    await accept(r.state);
    meshNowStore.set({ report: r, basis });
    return r;
  });
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
  const before = folderOf(sceneStore.get()?.info.path);
  const state = await run(`Could not save ${target}`, async () => accept(await backend.projectSave(target)));
  // Another folder is another runs root.
  if (folderOf(state.info.path) !== before) forgetRuns();
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

/** What "Apply" on a run-quality advice item sends (backlog 80). */
export interface AdviceApply {
  setting: Setting;
  from: number | boolean;
  to: number | boolean;
}

/** The refusal key an advice Apply is filed under: `advice:<setting>`. */
export const adviceKey = (a: AdviceApply) => `advice:${a.setting}`;

/**
 * "Apply" on a run-quality advice item (backlog 80): the one setting from `from` to `to`, through
 * the checked apply, one undo step. Refused by the core when the project's value is no longer
 * `from` (the project changed since the run the advice came from): a FAIL line, nothing changes.
 * A validator refusal is `applied: false`, filed under `adviceKey` as `apply` files its own.
 */
export async function adviceApply(a: AdviceApply): Promise<EditOutcome> {
  return run('Apply failed', async () => {
    const outcome = await backend.adviceApply(a);
    await accept(outcome.state);
    fileRefusals(adviceKey(a), outcome);
    return outcome;
  });
}

/** "Apply and re-run" (the Results step, backlog 80): the Apply, then a run of `solver` when the
 * project took it. `null` when nothing was started. */
export async function adviceApplyAndRerun(a: AdviceApply, solver: SolverName): Promise<RunStarted | null> {
  const outcome = await adviceApply(a);
  if (!outcome.applied) return null;
  return runStart(solver);
}

/**
 * A band preset (PQ3, C26): every band of `kind` from `lowestHz` to `highestHz`, each new band
 * taking every per-band value of the nearest current band, computed and applied by the core as
 * one checked edit and one undo step. Refusals are filed under `fieldKey` as `apply` files them.
 */
export async function reband(kind: BandKind, lowestHz: number, highestHz: number, fieldKey?: string): Promise<EditOutcome> {
  return run('Changing the bands failed', async () => {
    const outcome = await backend.editReband(kind, lowestHz, highestHz);
    await accept(outcome.state);
    fileRefusals(fieldKey, outcome);
    return outcome;
  });
}

/**
 * New group from selection (scope row 15 (1), G19): the faces picked in the 3D view go to a new
 * surface group, `Group <n>`, with the material the core picks: their common one, or upstream's
 * placeholder when they differ, which blocks Run until a material is chosen. One checked edit,
 * one undo step; the new group is then selected, so its material can be set. A selection the
 * core refuses (one that straddles a surface receiver or fitting zone) is a FAIL line, and the
 * project is unchanged. `null` when no faces are picked.
 */
export async function regroupSelection(): Promise<EditOutcome | null> {
  const faces = regroupFaces(selectionStore.get());
  if (!faces) return null;
  return run('New group from selection', async () => {
    const before = new Set((sceneStore.get()?.view.surface_groups ?? []).map((g) => g.id));
    const outcome = await backend.editRegroup(faces);
    await accept(outcome.state);
    const added = outcome.state.view.surface_groups.find((g) => !before.has(g.id));
    if (outcome.applied && added) {
      selectionStore.set({ kind: 'group', id: added.id });
      log('OK', `New group ${added.name}: ${faces.length} ${faces.length === 1 ? 'face' : 'faces'}`);
    }
    return outcome;
  });
}

/**
 * Add surface group (parity G18, upstream's "add a group"): an empty group, `Group <n>`, with
 * upstream's placeholder material, which the core picks (the project's own, or a new one in the
 * same edit). One checked edit, one undo step; the new group is then selected, so its material
 * can be set. Run stays blocked until it has one; faces join it with Move to group. `null` with
 * no project.
 */
export async function addEmptyGroup(): Promise<EditOutcome | null> {
  if (!sceneStore.get()) return null;
  return run('Add surface group', async () => {
    const before = new Set((sceneStore.get()?.view.surface_groups ?? []).map((g) => g.id));
    const outcome = await backend.editAddGroup();
    await accept(outcome.state);
    const added = outcome.state.view.surface_groups.find((g) => !before.has(g.id));
    if (outcome.applied && added) {
      selectionStore.set({ kind: 'group', id: added.id });
      log('OK', `New group ${added.name}, empty: choose its material (Run waits for one), then move faces into it with Move to group`);
    }
    return outcome;
  });
}

/**
 * C1, "Move selection to group": the faces picked in the 3D view into the existing surface group
 * `group`, where they take its material. One checked edit, one undo step; the group is then
 * selected. A move the core refuses (one that would add faces to a surface receiver or zone, or
 * take some out) is a FAIL line, and the project is unchanged. `null` when no faces are picked.
 */
export async function moveSelectionToGroup(group: string): Promise<EditOutcome | null> {
  const faces = regroupFaces(selectionStore.get());
  if (!faces) return null;
  const name = sceneStore.get()?.view.surface_groups.find((g) => g.id === group)?.name ?? group;
  const outcome = await apply(moveFaces(group, faces), `surface_group:${group}:faces`);
  if (outcome.applied) {
    selectionStore.set({ kind: 'group', id: group });
    log('OK', `Moved ${faces.length} ${faces.length === 1 ? 'face' : 'faces'} to ${name}`);
  }
  return outcome;
}

/**
 * C1, Merge groups: the groups picked in the scene list (Ctrl+click) folded into the first
 * picked, which keeps its name and material. One checked edit, one undo step; the merged group
 * is then selected. `null` when fewer than two groups are picked.
 */
export async function mergeSelectedGroups(): Promise<EditOutcome | null> {
  const sel = selectionStore.get();
  if (sel.kind !== 'groups' || sel.ids.length < 2) return null;
  const [into, ...from] = sel.ids;
  const view = sceneStore.get()?.view;
  const names = sel.ids.map((id) => view?.surface_groups.find((g) => g.id === id)?.name ?? id);
  const outcome = await apply(mergeGroups(into, from), `surface_group:${into}:merge`);
  if (outcome.applied) {
    selectionStore.set({ kind: 'group', id: into });
    log('OK', `Merged ${names.slice(1).join(', ')} into ${names[0]}`);
  }
  return outcome;
}

/** C1: renames a surface group (F2 in the scene list), one checked edit and one undo step. */
export async function renameGroup(id: string, name: string): Promise<EditOutcome> {
  return apply(rename('surface_group', id, name), `surface_group:${id}:name`);
}

/** C1: upstream's reference spectra on the open project's bands, fetched once per band set. */
export async function loadSpectrumLibrary(): Promise<void> {
  const bands = sceneStore.get()?.view.bands.frequencies_hz.join(',') ?? '';
  if (!bands || spectrumLibraryStore.get().bands === bands) return;
  const list = await run('Reading the spectrum library', () => backend.spectrumLibrary());
  spectrumLibraryStore.set({ bands, list });
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
 * G48: how far a placement click in the 3D view puts a receiver or a source from the face clicked,
 * metres: straight up from a floor, along the normal into the room from any other face. Starts at
 * PLACE_HEIGHT_M; the place hint edits it for this session.
 */
export const placeOffsetStore = new Store<{ receiver: number; source: number }>({ ...PLACE_HEIGHT_M });

/** G48: where a placement click put a source or receiver: the face, its group, the lift and the point written. */
export interface Placement {
  face: number;
  group: string | null;
  lift: number;
  how: 'above' | 'off';
  point: Vec3;
  /** The face's winding pointed into the room, so the point went to the other side of it. */
  flipped?: boolean;
}
/** G48: this session's placements by source or receiver id (shown while it still stands where it was put). */
export const placementStore = new Store<ReadonlyMap<string, Placement>>(new Map());

/** G48: a placement in words: "1.2 m above face 4 (Floor)", and the face's winding when it looked flipped. */
export function placementText(p: Placement): string {
  const at = `${p.lift} m ${p.how === 'above' ? 'above' : 'off'} face ${p.face}${p.group ? ` (${p.group})` : ''}`;
  return p.flipped ? `${at}, on the room's side: the face looks flipped (wound facing into the room)` : at;
}

/** The refusals that mean a placed point is not in the room, the ones the other side of the face may cure. */
export const OUTSIDE_CODES: ReadonlySet<string> = new Set(['RECEIVER_OUTSIDE', 'SOURCE_OUTSIDE', 'RECEIVER_ON_SURFACE']);

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

/**
 * W1 (parity M41): a new cutting plane over the model's box at ear height, as upstream places
 * one (1.6 m above the lowest point, A, B, C at the box's corners), named `Plane <n>`, cells of
 * `PLANE_RESOLUTION_M`, at the end of the surface receivers. Through the checked apply: the
 * core's `cutting_plane_invalid` refuses a bad one, filed under `surface_receiver:new:shape`.
 * Null without a checked model to place it in.
 */
export async function addEarPlane(): Promise<EditOutcome | null> {
  const state = sceneStore.get();
  const box = roomBox(state?.check);
  if (!state || !box) return null;
  const view = state.view;
  const { a, b, c } = earPlane(box);
  const name = nextName('Plane ', view.surface_receivers.map((r) => r.name));
  const plane = newCuttingPlane(crypto.randomUUID(), name, a, b, c, PLANE_RESOLUTION_M);
  return apply(addSurfaceReceiver(view.surface_receivers.length, plane), 'surface_receiver:new:shape');
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
 * orphaned from the project the Runs tab lists. The backend refuses New, Open and Import during
 * a run too, since this page's memory is lost when it is reloaded. */
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

/** Whether SPPS can run on the GPU here (decision 70): the backend probes once per session. */
export async function refreshGpu(): Promise<void> {
  try {
    const before = gpuStatusStore.get();
    const s = await backend.sppsGpuStatus();
    gpuStatusStore.set(s);
    // Logged when the answer changes: the backend keeps a found device and probes a failure
    // again, so the Simulate step re-asks while there is none.
    if (before?.available !== s.available || before?.device !== s.device || before?.reason !== s.reason) {
      if (s.available) log('INFO', `SPPS on the GPU: ${s.device ?? ''}`);
      else log('INFO', `SPPS on the GPU unavailable: ${s.reason ?? ''}`);
    }
    // A GPU chosen earlier in the session with no device now runs nothing: back to the CPU.
    if (!s.available && deviceStore.get() === 'gpu') deviceStore.set('cpu');
  } catch (e) {
    const err = asCmdError(e);
    gpuStatusStore.set({ available: false, device: null, reason: `${err.message} (${err.code})` });
    log('FAIL', `Probing the GPU: ${err.message} (${err.code})`);
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
  reattach(view);
  return view;
}

/**
 * A run the backend is running that this page has no record of: the page was reloaded mid-run
 * (WebView2's reload, the devtools), which loses everything in its memory, `runStore` included,
 * while the run goes on. Without this the page showed no Cancel, enabled Run, and let New replace
 * the project while the solver ran (M11 review 2, app 2). The page takes the run back as its
 * active run, so Cancel is shown and Run, New and Open wait. Its stream went to the page that
 * was reloaded, so its progress is not shown here; the run is watched through `runs_list` until
 * it ends.
 */
function reattach(view: RunsView): void {
  const name = view.active ?? null;
  if (name === null || runStore.get() !== null || starting) return;
  // A run this page streamed is not lost: a `runs_list` answer from before its `ended` can
  // arrive after it, still naming the run as active.
  if (runLinesStore.get().has(name)) return;
  const row = view.rows.find((r) => r.run === name);
  const id = nextRunId++;
  runStore.set({
    id,
    run: name,
    solver: row?.solver === 'tcr' ? 'tcr' : 'spps',
    variant: row?.variant ?? null,
    stage: null,
    progress: null,
    progressText: '',
    startedAt: Date.now(),
    status: 'running',
  });
  log('WARN', 'A run is still going, and this page no longer receives its progress (the page was reloaded): Cancel still stops it');
  void watchReattached(id);
}

const REATTACHED_POLL_MS = 500;

/** Lists the runs until the backend has no active run, then lets the reattached run go. */
async function watchReattached(id: number): Promise<void> {
  for (;;) {
    await new Promise<void>((r) => setTimeout(r, REATTACHED_POLL_MS));
    if (runStore.get()?.id !== id) return;
    let view: RunsView;
    try {
      view = await backend.runsList();
    } catch {
      continue;
    }
    if ((view.active ?? null) !== null) continue;
    if (runStore.get()?.id !== id) return;
    runStore.set(null);
    runsStore.set(view);
    const last = view.rows[view.rows.length - 1];
    if (last) selectRun(last.run);
    return;
  }
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

/** `run`'s report, as `simpa results --json` prints it, with its bed statuses and the open
 * project's group names (M12 P2), fetched once per run. */
export async function reportFor(runName: string): Promise<ReportView> {
  const cached = reportStore.get().get(runName);
  if (cached) return cached;
  const view = await run(`Reading the results of ${runName}`, () => backend.runReport(runName));
  const next = new Map(reportStore.get());
  next.set(runName, view);
  reportStore.set(next);
  return view;
}

/** W9: asks the save dialog where a `kind` export goes (null: cancelled), unless `path` is given. */
export async function exportPath(kind: 'csv' | 'json' | 'png' | 'wav', defaultName: string, path?: string): Promise<string | null> {
  if (path !== undefined) return path;
  const names = { csv: 'CSV table', json: 'JSON', png: 'PNG image', wav: 'WAV audio' } as const;
  const target = await saveDialog({ defaultPath: defaultName, filters: [{ name: names[kind], extensions: [kind] }] });
  return typeof target === 'string' ? target : null;
}

/** W9: writes an export the core checks (absolute path, the kind's extension, the kind's bytes). */
export async function exportWrite(kind: 'csv' | 'json' | 'png' | 'wav', path: string, bytes: Uint8Array, what: string): Promise<number> {
  const n = await run(`Could not export ${what} to ${path}`, () => backend.exportWrite(kind, path, bytes));
  log('OK', `Exported ${what} to ${path}`);
  return n;
}

/** The Results step's viewport reads (M12 P3, `features/viewport/resultsView.ts`), passed through
 * as the backend returns them: the caller keeps its own generation check and reads a refusal
 * itself (`asCmdError`), so no busy line or Console entry is added here. Here because the backend
 * is called only from this file (M10's lint). */
export const runData = (runName: string) => backend.runData(runName);
export const runSurfaceMap = (runName: string, path: string, maxTexels: number) => backend.runSurfaceMap(runName, path, maxTexels);
export const runParticles = (runName: string, bandHz: number) => backend.runParticles(runName, bandHz);
/** C5: the listening window's read (features/acoustics/AuralWindow.tsx), passed through as the
 * backend returns it; the window reads a refusal itself. */
export const runAuralize = (runName: string, receiver: string, source: string | null, dry: { clip?: string; path?: string }) =>
  backend.runAuralize(runName, receiver, source, dry);

/** C5: asks the open dialog for a WAV to auralize (null: cancelled). */
export async function openWavDialog(): Promise<string | null> {
  const p = await open({ multiple: false, directory: false, filters: [{ name: 'WAV audio', extensions: ['wav'] }] });
  return typeof p === 'string' ? p : null;
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
          if (current) current = { ...current, stage: e.stage, solveAt: e.stage === 'solve' ? (current.solveAt ?? Date.now()) : current.solveAt };
          break;
        case 'line':
          if (current && e.class === 'PROGRESS' && e.source === 'solver') {
            const at = Date.now();
            current = { ...current, progress: e.progress ?? null, progressText: progressText(e.text), progressAt: at, progressLog: logProgress(current.progressLog, current.solveAt, e.progress, at) };
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

/** B3: where a run's live particles go (liveView.ts registers itself; nothing until it has). */
export interface LiveSink {
  begin(runId: number): void;
  batch(runId: number, buf: ArrayBuffer): void;
  end(runId: number): void;
}
let liveSink: LiveSink | null = null;
export function setLiveSink(s: LiveSink | null): void {
  liveSink = s;
}

/**
 * Run (PQ1: the run is of what is on screen, so it saves first): with no path, Save as (a
 * cancelled dialog runs nothing); with unsaved changes, Save. Then the solvers are checked and
 * the run starts; its events arrive through the channel. Returns what `run_start` answered, or
 * `null` when nothing was started.
 */
export async function runStart(
  solver: SolverName = solverStore.get(),
  device: SppsDevice = solver === 'spps' ? deviceStore.get() : 'cpu',
): Promise<RunStarted | null> {
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
      device,
      variant: info?.active_variant ?? null,
      stage: null,
      progress: null,
      progressText: '',
      startedAt: Date.now(),
      status: 'starting',
    });
    const channel = new Channel<RunStreamBatch>();
    const onEvent = runEvents(id);
    channel.onmessage = (batch) => {
      onEvent(batch);
      if (batch.events.some((e) => e.kind === 'ended' || e.kind === 'failed') || batch.last) liveSink?.end(id);
    };
    // B3: only a run on the GPU sends live particles; the channel is passed for every run.
    const live = new Channel<ArrayBuffer>();
    live.onmessage = (buf) => liveSink?.batch(id, buf);
    if (device === 'gpu') liveSink?.begin(id);
    try {
      const started = await backend.runStart(solver, device, channel, live);
      const on = started.gpu_device ? ` on the GPU (${started.gpu_device})` : '';
      log('INFO', `${solver.toUpperCase()} run started${on}: ${started.project_path}`);
      resendCancel(id);
      return started;
    } catch (e) {
      // This run's state only: never a run that another start put there.
      if (runStore.get()?.id === id) runStore.set(null);
      liveSink?.end(id);
      const err = asCmdError(e);
      log('FAIL', `Could not start the run: ${err.message} (${err.code})`);
      throw e;
    }
  } finally {
    starting = false;
  }
}

/**
 * A Cancel pressed while the run was starting went to the backend before `run_start` had
 * registered the run, and was answered `false` with nothing cancelled: the solver then ran to its
 * end while the UI read "Cancelling…" with its Cancel disabled (M11 review 2, M2). Once
 * `run_start` has answered, the run is registered, so a Cancel pressed meanwhile is sent again.
 */
function resendCancel(id: number): void {
  const current = runStore.get();
  if (current?.id === id && current.status === 'cancelling') fire(run('Could not cancel the run', () => backend.runCancel()));
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
  forgetRuns();
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

/**
 * M43: switches a cutting plane or a surface receiver on or off, the receiver replaced whole: one
 * undo step, refusals filed under `surface_receiver:<id>:enabled`. A receiver switched off is left
 * out of the solver's input, as a source is; its solver id is kept, so nothing else renumbers.
 */
export async function setSurfaceReceiverEnabled(id: string, enabled: boolean): Promise<EditOutcome | null> {
  const view = sceneStore.get()?.view;
  const r = view?.surface_receivers.find((x) => x.id === id);
  if (!view || !r) throw new Error(`setSurfaceReceiverEnabled: no surface receiver ${id}`);
  const key = `surface_receiver:${id}:enabled`;
  // M40: a surface map switched on over a surface another enabled map holds could not be written for a run.
  const taken = enabled && r.shape.kind === 'scene' ? r.shape.groups.map((g) => ({ g, map: mapOfGroup(g, view, id) })).find((t) => t.map) : undefined;
  if (taken?.map) {
    const group = view.surface_groups.find((g) => g.id === taken.g)?.name ?? taken.g;
    fileLocal(key, 'MAP_GROUP_TAKEN', { kind: 'surface_receiver', id }, `${group} is in ${taken.map.name} already, and a surface is in one surface map at most: switch ${taken.map.name} off first`);
    return null;
  }
  return apply(replaceSurfaceReceiver({ ...r, enabled }), key);
}

/**
 * G29: a new box fitting zone, 1 m on a side, 10 cm above the room's floor at the middle of its plan, with
 * upstream's values (zones.ts `newBoxZone`), named `Fitting zone <n>`, at the end of the zones:
 * one checked edit, refusals under `fitting_zone:new:shape`. Null without a checked model.
 */
export async function addBoxZone(): Promise<EditOutcome | null> {
  const state = sceneStore.get();
  const box = roomBox(state?.check);
  if (!state || !box) return null;
  const zones = fittingZonesStore.get() ?? [];
  const name = nextName('Fitting zone ', zones.map((z) => z.name));
  const zone = newBoxZone(crypto.randomUUID(), name, box, state.view.bands.frequencies_hz.length);
  const outcome = await apply(addFittingZone(zones.length, zone), 'fitting_zone:new:shape');
  if (outcome.applied) log('OK', `Added ${name}: a 1 m box 10 cm above the floor; set its corners and values in the Geometry step`);
  return outcome;
}

/** A refusal the UI makes itself, filed under `key` as the checked apply files one. */
function fileLocal(key: string, code: string, entity: UiIssue['entity'], message: string): void {
  const next = new Map(refusalStore.get());
  next.set(key, [{ code, rule: '', severity: 'error', path: key, entity, field: key.split(':')[2] ?? '', message }]);
  refusalStore.set(next);
}

/**
 * M40: a new surface map (a scene surface receiver) over the surfaces picked, named
 * `Surface map <n>`, at the end of the surface receivers: one checked edit, one undo step. Its
 * problem instead when nothing is picked or a picked surface is in another map
 * (groupsModel.ts `surfaceMapPlan`); the project is then unchanged.
 */
export async function addSurfaceMap(): Promise<{ outcome: EditOutcome } | { problem: { code: string; message: string } }> {
  const view = sceneStore.get()?.view;
  if (!view) return { problem: { code: 'NO_PROJECT', message: 'Open a project first.' } };
  const plan = surfaceMapPlan(selectionStore.get(), view);
  if ('problem' in plan) return plan;
  const name = nextName('Surface map ', view.surface_receivers.map((r) => r.name));
  const outcome = await apply(addSurfaceReceiver(view.surface_receivers.length, newSceneReceiver(crypto.randomUUID(), name, plan.groups)), 'surface_receiver:new:map');
  if (outcome.applied) log('OK', `Added ${name} over ${plan.groups.length} ${plan.groups.length === 1 ? 'surface' : 'surfaces'}: it has a map once SPPS runs again`);
  return { outcome };
}

/** M43: switches a fitting zone on or off, as `setSurfaceReceiverEnabled`; refusals under `fitting_zone:<id>:enabled`. */
export async function setFittingZoneEnabled(id: string, enabled: boolean): Promise<EditOutcome> {
  const z = fittingZonesStore.get()?.find((x) => x.id === id);
  if (!z) throw new Error(`setFittingZoneEnabled: no fitting zone ${id}`);
  return apply(replaceFittingZone({ ...z, enabled }), `fitting_zone:${id}:enabled`);
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
