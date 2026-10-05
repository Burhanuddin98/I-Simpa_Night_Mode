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
  EchogramView,
  EditOutcome,
  EventsProbeReport,
  FloatProbe,
  LibraryMaterial,
  Prepared,
  ProjectInfo,
  ReportView,
  ResultsState,
  RunDataIndex,
  RunEventBatch,
  RunStarted,
  RunStreamBatch,
  RunsView,
  SceneState,
  Setting,
  SolversStatus,
  StartupInfo,
} from './bindings/ipc';
import type { BandKind, Op } from './bindings/schema';
import { opText } from './ops';
import { busyStore, type SolverName } from './store';

/** An advice Apply's arguments as JSON text: a finite number or on/off each (the core reads them
 * exactly, as it reads an op). */
function adviceText(a: { setting: Setting; from: number | boolean; to: number | boolean }): string {
  for (const v of [a.from, a.to]) if (typeof v === 'number' && !Number.isFinite(v)) throw new Error(`adviceApply: ${v} is not a finite number`);
  return JSON.stringify({ setting: a.setting, from: a.from, to: a.to });
}

export type {
  AppEvent,
  CmdError,
  EchogramView,
  ReportView,
  RunDataIndex,
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

/** `invoke` with a raw body and headers (W9's export). */
async function invokeWith<T>(cmd: string, body: Uint8Array, options: { headers: Record<string, string> }): Promise<T> {
  busyStore.set(busyStore.get() + 1);
  try {
    return await tauriInvoke<T>(cmd, body, options);
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
  /** The landing page's example `id`: the core writes a fresh copy into Documents\Night Mode\Examples
   * (never over a file) and opens it as `scene_open` does. */
  exampleOpen: (id: string) => invoke<SceneState>('example_open', { id }),
  modelImport: (path: string, unit: Unit, up: Up) => invoke<SceneState>('model_import', { path, unit, up }),
  /** `null` saves to the project's own path; a path is Save As. */
  projectSave: (path: string | null) => invoke<SceneState>('project_save', { path }),
  /** The checked apply. The op goes as `opText`, never `JSON.stringify` (-0, NaN). */
  editApply: (op: Op) => invoke<EditOutcome>('edit_apply', { op: opText(op) }),
  /** A band preset (PQ3): the core rebands the project and applies it as one checked edit. */
  editReband: (kind: BandKind, lowestHz: number, highestHz: number) =>
    invoke<EditOutcome>('edit_reband', { kind, lowest_hz: lowestHz, highest_hz: highestHz }),
  /** "Apply" on a run-quality advice item (backlog 80): `{setting, from, to}` as JSON text, read
   * by the core's exact reader; refused `ADVICE_PROJECT_CHANGED` when the project's value is no
   * longer `from`. */
  adviceApply: (apply: { setting: Setting; from: number | boolean; to: number | boolean }) =>
    invoke<EditOutcome>('advice_apply', { apply: adviceText(apply) }),
  /** New group from selection (row 15, G19): the core picks the material and applies it as one
   * checked edit. */
  editRegroup: (faces: number[]) => invoke<EditOutcome>('edit_regroup', { faces }),
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

  // M12 (docs/investigations/2026-10-03-m12/PLAN.md, P1 item 3): the Results step's reads. The
  // report is the CLI's (`simpa results --json`); a parameter is shown only when its
  // `report.bed.parameters[name].status` is PASS (gate (b)). The bytes decode in resultsData.ts.
  runReport: (run: string) => invoke<ReportView>('run_report', { run }),
  runData: (run: string) => invoke<RunDataIndex>('run_data', { run }),
  /** A surface map, `path` as `runData` lists it, as SMAP bytes (`decodeSurfaceMap`). */
  runSurfaceMap: (run: string, path: string) => invoke<ArrayBuffer>('run_surface_map', { run, path }),
  /** One band's saved particles as PART bytes (`decodeParticles`). */
  runParticles: (run: string, bandHz: number) => invoke<ArrayBuffer>('run_particles', { run, band_hz: bandHz }),
  runEchogram: (run: string, receiver: string) => invoke<EchogramView>('run_echogram', { run, receiver }),
  /** W9: writes `bytes` to `path` (the save dialog's) as a `kind` file; the core checks both. */
  exportWrite: (kind: 'csv' | 'json' | 'png', path: string, bytes: Uint8Array) =>
    invokeWith<number>('export_write', bytes, { headers: { 'x-export-path': encodeURIComponent(path), 'x-export-kind': kind } }),
};
