// Typed calls to the app's commands. Result types come from the generated bindings
// (ui/src/bindings, from the Rust types); schema values go out as JSON text and are read by the
// core's exact reader, never by a serde_json-typed command argument.
//
// Only actions.ts calls the M10 and M11 commands, and only selftest.ts the M9 project commands
// (tools/gates/m10.ps1 and m11.ps1 lint them). Every call is counted in `busyStore` for the
// `idle()` hook; `run_start` answers at once, so a run in progress never holds `idle()`.
import { Channel, invoke as tauriInvoke, type InvokeArgs } from '@tauri-apps/api/core';
import type {
  AboutInfo,
  AppEvent,
  CmdError,
  EchogramView,
  EditOutcome,
  EventsProbeReport,
  Extra,
  MapParameter,
  FloatProbe,
  GpuStatus,
  LibraryMaterial,
  LibrarySpectrum,
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
  MeshNowReport,
  RepairReport,
  AutosaveStatus,
  RecoveryEntry,
} from './bindings/ipc';
import type { BandKind, Op } from './bindings/schema';
import { opText } from './ops';
import { busyStore, type SolverName, type SppsDevice } from './store';

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
  GpuStatus,
  LibraryMaterial,
  LibrarySpectrum,
  ProjectInfo,
  MeshNowReport,
  RepairReport,
  RecoveryEntry,
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
export type { SolverName, SppsDevice };

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
  /** G11: a new project holding a box room of `width` (x) by `length` (y) by `height` (z) metres. */
  sceneNewBox: (name: string, width: number, length: number, height: number) => invoke<SceneState>('scene_new_box', { name, width, length, height }),
  sceneOpen: (path: string) => invoke<SceneState>('scene_open', { path }),
  /** The landing page's example `id`: the core writes a fresh copy into Documents\Night Mode\Examples
   * (never over a file) and opens it as `scene_open` does. */
  exampleOpen: (id: string) => invoke<SceneState>('example_open', { id }),
  modelImport: (path: string, unit: Unit, up: Up) => invoke<SceneState>('model_import', { path, unit, up }),
  /** G8: the core's safe repairs on the open model, written as a new file beside the original, then one checked edit. */
  modelRepair: () => invoke<RepairReport>('model_repair'),
  /** G7: the mesh replaces the open model, keeping each matching face's surface group. */
  modelReimport: (path: string, unit: Unit, up: Up) => invoke<EditOutcome>('model_reimport', { path, unit, up }),
  /** G32: TetGen on the open project now, as a run would mesh it, into a scratch folder; a run still meshes again. */
  meshNow: () => invoke<MeshNowReport>('mesh_now'),
  /** `null` saves to the project's own path; a path is Save As. */
  projectSave: (path: string | null) => invoke<SceneState>('project_save', { path }),
  /** A34: keeps the open project's unsaved changes for crash recovery (only when they moved), or removes the copy. */
  recoverySave: () => invoke<AutosaveStatus>('recovery_save'),
  /** A34: the copies left by instances that ended without closing, newest first. */
  recoveryList: () => invoke<RecoveryEntry[]>('recovery_list'),
  /** A34: opens a copy as unsaved changes to its project's own file, and removes the copy. */
  recoveryRestore: (key: string) => invoke<SceneState>('recovery_restore', { key }),
  /** A34: removes a copy the person let go; answers the copies still offered. */
  recoveryDiscard: (key: string) => invoke<RecoveryEntry[]>('recovery_discard', { key }),
  /** A6: the project as it is now written to `path`; the open project stays on its own file, unsaved changes kept. */
  projectSaveCopy: (path: string) => invoke<SceneState>('project_save_copy', { path }),
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
  /** Add surface group (G18): an empty group with upstream's placeholder material, one checked edit. */
  editAddGroup: () => invoke<EditOutcome>('edit_add_group'),
  editUndo: () => invoke<SceneState>('edit_undo'),
  editRedo: () => invoke<SceneState>('edit_redo'),
  /** The geometry as raw bytes (mesh.ts decodes them). */
  sceneMesh: () => invoke<ArrayBuffer>('scene_mesh'),

  // M11 (docs/investigations/2026-09-29-m11/PLAN.md 2.10). A run answers at once and reports
  // through `onEvent`; nothing here returns a solver-computed number.
  /** B3: a run on the GPU also sends its saved particles to `onLive` as LIVE v1 batches (ArrayBuffers). */
  runStart: (solver: SolverName, device: SppsDevice, onEvent: Channel<RunStreamBatch>, onLive: Channel<ArrayBuffer>) =>
    invoke<RunStarted>('run_start', { solver, device, on_event: onEvent, on_live: onLive }),
  runCancel: () => invoke<boolean>('run_cancel'),
  runsList: () => invoke<RunsView>('runs_list'),
  /** R2: a run's label (empty clears it), kept beside its run.json; answers the runs listed after it. */
  runLabel: (run: string, label: string) => invoke<RunsView>('run_label', { run, label }),
  /** R4: Explorer on a listed run's folder, started by the core; answers the folder opened. */
  runOpenFolder: (run: string) => invoke<string>('run_open_folder', { run }),
  /** A20: a Help topic (help.rs) opened by the core in the default browser; answers what it opened. */
  helpOpen: (topic: string) => invoke<string>('help_open', { topic }),
  /** A23: this build's version, commit, profile and solvers, for Help › About. */
  appAbout: () => invoke<AboutInfo>('app_about'),
  runResults: (run: string) => invoke<ResultsState>('run_results', { run }),
  projImport: (path: string) => invoke<SceneState>('proj_import', { path }),
  materialLibrary: () => invoke<LibraryMaterial[]>('material_library'),
  /** C1: upstream's reference spectra as source shapes on the open project's bands. */
  spectrumLibrary: () => invoke<LibrarySpectrum[]>('spectrum_library'),
  solversStatus: () => invoke<SolversStatus>('solvers_status'),
  /** SPPS on the GPU (decision 70): `spps-gpu --probe`'s answer, run once per app session. */
  sppsGpuStatus: () => invoke<GpuStatus>('spps_gpu_status'),
  appEvents: (onEvent: Channel<AppEvent>) => invoke<null>('app_events', { on_event: onEvent }),
  appQuit: () => invoke<null>('app_quit'),

  // M12 (docs/investigations/2026-10-03-m12/PLAN.md, P1 item 3): the Results step's reads. The
  // report is the CLI's (`simpa results --json`); a parameter is shown only when its
  // `report.bed.parameters[name].status` is PASS (gate (b)). The bytes decode in resultsData.ts.
  runReport: (run: string, custom: readonly Extra[] = []) => invoke<ReportView>('run_report', { run, custom }),
  runData: (run: string) => invoke<RunDataIndex>('run_data', { run }),
  /**
   * A surface map, `path` as `runData` lists it, as SMAP bytes (`decodeSurfaceMap`). `maxTexels` is the
   * viewport's budget for faces x steps (its texture is dense): a map past it, or past the backend's record
   * bound, arrives at a coarser time bin, which its header carries.
   */
  runSurfaceMap: (run: string, path: string, maxTexels: number) =>
    invoke<ArrayBuffer>('run_surface_map', { run, path, max_texels: maxTexels }),
  /** Parity R42/R73: T30, EDT, C80 or D50 on every face of one per-band map, as PMAP bytes
   * (`decodeParameterMap`): core's values with the receivers' code, and the SMAP they are drawn from. */
  runParameterMap: (run: string, path: string, parameter: MapParameter) => invoke<ArrayBuffer>('run_parameter_map', { run, path, parameter }),
  /** One band's saved particles as PART bytes (`decodeParticles`). */
  runParticles: (run: string, bandHz: number) => invoke<ArrayBuffer>('run_particles', { run, band_hz: bandHz }),
  runEchogram: (run: string, receiver: string) => invoke<EchogramView>('run_echogram', { run, receiver }),
  /** C5: a receiver's synthesised impulse response, or a clip or WAV file convolved with it (or,
   * `room` false, the clip alone at the level it shares with that), as WAV bytes (`readWav`,
   * features/acoustics/aural.ts). */
  runAuralize: (run: string, receiver: string, source: string | null, dry: { clip?: string; path?: string; room?: boolean }) =>
    invoke<ArrayBuffer>('run_auralize', { run, receiver, source, clip: dry.clip ?? null, path: dry.path ?? null, room: dry.room ?? true }),
  /** W9: writes `bytes` to `path` (the save dialog's) as a `kind` file; the core checks both. */
  /** R3: `run` names the run the export is made from; the core notes the file beside it. */
  exportWrite: (kind: 'csv' | 'json' | 'png' | 'wav', path: string, bytes: Uint8Array, run: string | null = null) =>
    invokeWith<number>('export_write', bytes, {
      headers: { 'x-export-path': encodeURIComponent(path), 'x-export-kind': kind, ...(run === null ? {} : { 'x-export-run': encodeURIComponent(run) }) },
    }),
  /** R3: a listed run, not the active one, moved to the Recycle Bin; answers the runs listed after it. */
  runDelete: (run: string) => invoke<RunsView>('run_delete', { run }),
};
