// Typed calls to the app's commands. Result types come from the generated bindings
// (ui/src/bindings, from the Rust types); schema values go out as JSON text and are read by the
// core's exact reader, never by a serde_json-typed command argument.
import { Channel, invoke } from '@tauri-apps/api/core';
import type {
  CmdError,
  EventsProbeReport,
  FloatProbe,
  Prepared,
  ProjectInfo,
  RunEventBatch,
  StartupInfo,
} from './bindings/ipc';
import type { Op } from './bindings/schema';

export type { CmdError, ProjectInfo, RunEventBatch, StartupInfo };

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
};
