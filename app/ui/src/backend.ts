// Typed calls to the app's commands. Result types come from the generated bindings
// (ui/src/bindings, from the Rust types); schema values go out as JSON text and are read by the
// core's exact reader, never by a serde_json-typed command argument.
//
// Only actions.ts calls the M10 commands, and only selftest.ts the M9 project commands
// (tools/gates/m10.ps1 lints both). Every call is counted in `busyStore` for the `idle()` hook.
import { Channel, invoke as tauriInvoke, type InvokeArgs } from '@tauri-apps/api/core';
import type {
  CmdError,
  EditOutcome,
  EventsProbeReport,
  FloatProbe,
  Prepared,
  ProjectInfo,
  RunEventBatch,
  SceneState,
  StartupInfo,
} from './bindings/ipc';
import type { Op } from './bindings/schema';
import { opText } from './ops';
import { busyStore } from './store';

export type { CmdError, EditOutcome, ProjectInfo, RunEventBatch, SceneState, StartupInfo };

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
  editUndo: () => invoke<SceneState>('edit_undo'),
  editRedo: () => invoke<SceneState>('edit_redo'),
  /** The geometry as raw bytes (mesh.ts decodes them). */
  sceneMesh: () => invoke<ArrayBuffer>('scene_mesh'),
};
