// Typed calls to the app's commands. Result types come from the generated bindings
// (ui/src/bindings, from the Rust types); schema values go out as JSON text and are read by the
// core's exact reader, never by a serde_json-typed command argument.
//
// Only actions.ts calls the M10 and M11 commands, and only selftest.ts the M9 project commands
// (tools/gates/m10.ps1 and m11.ps1 lint them). Every call is counted in `busyStore` for the
// `idle()` hook; `run_start` answers at once, so a run in progress never holds `idle()`.
import { Channel, invoke as tauriInvoke, type InvokeArgs } from '@tauri-apps/api/core';
import type {
  AppEvent,
  CmdError,
  EditOutcome,
  EventsProbeReport,
  FloatProbe,
  LibraryMaterial,
  Prepared,
  ProjectInfo,
  ResultsState,
  RunEventBatch,
  RunStarted,
  RunStreamBatch,
  RunsView,
  SceneState,
  SolversStatus,
  StartupInfo,
} from './bindings/ipc';
import type { BandKind, Op } from './bindings/schema';
import { opText } from './ops';
import { busyStore, type SolverName } from './store';

export type {
  AppEvent,
  CmdError,
  EditOutcome,
  LibraryMaterial,
  ProjectInfo,
  ResultsState,
  RunEventBatch,
  RunStarted,
  RunStreamBatch,
  RunsView,
  SceneState,
  SolversStatus,
  StartupInfo,
};
export { Channel };
export type { SolverName };

/** A mesh file's length unit and vertical axis, as `model_import` names them. */
export type Unit = 'm' | 'cm' | 'mm' | 'ft' | 'in';
export type Up = 'y' | 'z';

async function invoke<T>(cmd: string, args?: InvokeArgs): Promise<T> {
  busyStore.set(busyStore.get() + 1);
  try {
    return await tauriInvoke<T>(cmd, args);
  } finally {
    busyStore.set(busyStore.get() - 1);
  }
}

/** Every rejected command carries `{code, message}`; anything else is wrapped as UNKNOWN. */
export function asCmdError(e: unknown): CmdError {
  if (e && typeof e === 'object' && 'code' in e && 'message' in e) {
    return { code: String(e.code), message: String(e.message) };
  }
  return { code: 'UNKNOWN', message: String(e) };
}

export const backend = {
  startup: () => invoke<StartupInfo>('app_startup'),
  ping: () => invoke<string>('ping'),
  panicProbe: () => invoke<null>('panic_probe'),
  panicProbeUnguarded: () => invoke<null>('panic_probe_unguarded'),
  /** How many times the unguarded probe's body has started (Tauri re-sends a dropped call once). */
  unguardedPanicRuns: () => invoke<number>('unguarded_panic_runs'),
  benchPrepare: (bytes: number, seed: number) => invoke<Prepared>('bench_prepare', { bytes, seed }),
  /** The raw bytes of `ipc::Response`: an ArrayBuffer, no JSON step. */
  benchTake: (token: number) => invoke<ArrayBuffer>('bench_take', { token }),
  benchClear: () => invoke<number>('bench_clear'),
  runEventsProbe: (onEvent: Channel<RunEventBatch>, lines: number, spacingUs: number) =>
    invoke<EventsProbeReport>('run_events_probe', { on_event: onEvent, lines, spacing_us: spacingUs }),
  exactFloatProbe: (text: string) => invoke<FloatProbe>('exact_float_probe', { text }),
  projectNew: (name: string) => invoke<ProjectInfo>('project_new', { name }),
  projectOpen: (path: string) => invoke<ProjectInfo>('project_open', { path }),
  projectInfo: () => invoke<ProjectInfo | null>('project_info'),
  /** The project as canonical `.simpa` text. */
  projectJson: () => invoke<string>('project_json'),
  projectApply: (op: Op) => invoke<ProjectInfo>('project_apply', { op: JSON.stringify(op) }),
  projectUndo: () => invoke<ProjectInfo>('project_undo'),
  projectRedo: () => invoke<ProjectInfo>('project_redo'),
  selftestReport: (report: object) =>
    invoke<boolean>('selftest_report', { report: JSON.stringify(report) }),

  // M10 (PLAN.md 1.5). Each returns the whole SceneState, or the mesh bytes.
  sceneState: () => invoke<SceneState | null>('scene_state'),
  sceneNew: (name: string) => invoke<SceneState>('scene_new', { name }),
  sceneOpen: (path: string) => invoke<SceneState>('scene_open', { path }),
  modelImport: (path: string, unit: Unit, up: Up) => invoke<SceneState>('model_import', { path, unit, up }),
  /** `null` saves to the project's own path; a path is Save As. */
  projectSave: (path: string | null) => invoke<SceneState>('project_save', { path }),
  /** The checked apply. The op goes as `opText`, never `JSON.stringify` (-0, NaN). */
  editApply: (op: Op) => invoke<EditOutcome>('edit_apply', { op: opText(op) }),
  /** A band preset (PQ3): the core rebands the project and applies it as one checked edit. */
  editReband: (kind: BandKind, lowestHz: number, highestHz: number) =>
    invoke<EditOutcome>('edit_reband', { kind, lowest_hz: lowestHz, highest_hz: highestHz }),
  editUndo: () => invoke<SceneState>('edit_undo'),
  editRedo: () => invoke<SceneState>('edit_redo'),
  /** The geometry as raw bytes (mesh.ts decodes them). */
  sceneMesh: () => invoke<ArrayBuffer>('scene_mesh'),

  // M11 (docs/investigations/2026-09-29-m11/PLAN.md 2.10). A run answers at once and reports
  // through `onEvent`; nothing here returns a solver-computed number.
  runStart: (solver: SolverName, onEvent: Channel<RunStreamBatch>) =>
    invoke<RunStarted>('run_start', { solver, on_event: onEvent }),
  runCancel: () => invoke<boolean>('run_cancel'),
  runsList: () => invoke<RunsView>('runs_list'),
  runResults: (run: string) => invoke<ResultsState>('run_results', { run }),
  projImport: (path: string) => invoke<SceneState>('proj_import', { path }),
  materialLibrary: () => invoke<LibraryMaterial[]>('material_library'),
  solversStatus: () => invoke<SolversStatus>('solvers_status'),
  appEvents: (onEvent: Channel<AppEvent>) => invoke<null>('app_events', { on_event: onEvent }),
  appQuit: () => invoke<null>('app_quit'),
};
