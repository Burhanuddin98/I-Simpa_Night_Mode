// State that lives outside React (the islands rule: React owns the chrome, stores feed it).
// PLAN.md 2.1. Only actions.ts writes the backend's truth (sceneStore, meshStore); the packages
// read it and write only their own UI state (selection, tool, step).
import { useSyncExternalStore } from 'react';
import type {
  GpuStatus,
  LibraryMaterial,
  LibrarySpectrum,
  LineClass,
  ReportView,
  ResultsState,
  RunLineClass,
  RunsView,
  SceneState,
  SolversStatus,
  UiIssue,
} from './bindings/ipc';
import type { ProgressPoint } from './features/simulate/runTime';
import type { SceneMesh } from './mesh';
import type { StepKey } from './steps';

export class Store<T> {
  private listeners = new Set<() => void>();
  private value: T;
  constructor(value: T) {
    this.value = value;
  }
  get = (): T => this.value;
  set(value: T): void {
    this.value = value;
    this.listeners.forEach((l) => l());
  }
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };
}

export function useStore<T>(store: Store<T>): T {
  return useSyncExternalStore(store.subscribe, store.get);
}

/** A Console line's class, as its text label: the core's line classes, PROGRESS included. */
export type ConsoleTag = LineClass | 'PROGRESS';

/**
 * A part of a Console line: plain text, or a run diagnostic (M11 PLAN.md 3.4 rule 1) that the
 * Console renders as a leaf `<span data-diagnostic=field data-run=run>`, so the no-acoustic-number
 * check can prove it is the manifest's own value.
 */
export type LinePart =
  | { text: string }
  | { text: string; diagnostic: 'loss_pct' | 'loss_limit_pct' | 'elapsed_s' | 'progress_pct'; run: string };

export interface ConsoleLine {
  time: string;
  tag: ConsoleTag;
  text: string;
  /** Who wrote it: the app (default), a solver, or TetGen while a run meshes. */
  source?: 'app' | 'solver' | 'mesh';
  /** The run a solver or mesh line, or a run's app line, belongs to. */
  run?: string;
  /** Solver and TetGen text is verbatim: shown exactly as the program printed it. */
  verbatim?: boolean;
  /** `text` split into parts, when some are diagnostics. */
  parts?: LinePart[];
}

export const consoleStore = new Store<ConsoleLine[]>([]);
export type Status = { kind: 'ready' | 'busy' | 'bad'; text: string };
export const statusStore = new Store<Status>({ kind: 'ready', text: 'Ready' });

/** The backend's truth: replaced with every M10 response, never edited in place. */
export const sceneStore = new Store<SceneState | null>(null);
/** The decoded `scene_mesh`, refetched when `info.geometry_rev` changes. */
export const meshStore = new Store<SceneMesh | null>(null);
/** The workflow step shown (the step bar writes it; so do the e2e hooks). */
export const stepStore = new Store<StepKey>('geometry');

export type Selection =
  | { kind: 'none' }
  /** A viewport pick or flood-fill: face indices, and the names of their groups. */
  | { kind: 'faces'; faces: number[]; groups: string[] }
  | { kind: 'group'; id: string }
  /** Several surface groups, Ctrl+clicked in the scene list (C1: what Merge acts on), in the
   * order picked: the first is the one the others merge into. */
  | { kind: 'groups'; ids: string[] }
  | { kind: 'material'; id: string }
  | { kind: 'source'; id: string }
  | { kind: 'receiver'; id: string };
export const selectionStore = new Store<Selection>({ kind: 'none' });

export type Tool = 'select' | 'orbit' | 'place-receiver' | 'place-source';
export const toolStore = new Store<Tool>('select');

/** Refusals of the checked apply by field key (issues.ts `fieldKey`); a field's entry is cleared
 * by the next accepted edit filed under the same key. */
export const refusalStore = new Store<ReadonlyMap<string, UiIssue[]>>(new Map());

/** G16: the open project's fitting zones as the scene list shows them (read from the project file on every scene change); null when unknown. */
export const fittingZonesStore = new Store<{ id: string; name: string; enabled: boolean; kind: 'box' | 'scene' }[] | null>(null);

/** Commands in flight (the backend wrapper counts them); the `idle()` hook waits for 0. */
export const busyStore = new Store<number>(0);

/** A mesh file waiting for the import dialog's unit and up axis (File › Open… on a mesh). */
export const importRequestStore = new Store<{ path: string } | null>(null);

/**
 * What the viewport has drawn: `live` once a real viewport has mounted, and the geometry
 * revision of its last frame. `idle()` waits for the latest revision only when `live`.
 */
export const viewportStore = new Store<{ live: boolean; drawnRev: number | null }>({ live: false, drawnRev: null });

// ---- M11 (docs/investigations/2026-09-29-m11/PLAN.md 3.1) -------------------------------------

export type SolverName = 'spps' | 'tcr';
/** Where SPPS runs (decision 70): upstream's spps.exe on the CPU, or spps-gpu.exe on the GPU. A
 * run option, never saved in the project; TCR always runs on the CPU. */
export type SppsDevice = 'cpu' | 'gpu';

/** The run in progress: written only by the run actions in actions.ts. */
export interface ActiveRun {
  /** Which start of this page's this is: a run's stream and its `run_start` answer touch this
   * store only while it still holds their run (M11 review 2, M3/E1). */
  id: number;
  /** The run folder's name, once the core has made it (the `started` event). */
  run?: string;
  solver: SolverName;
  /** SPPS on the GPU or the CPU (A5). */
  device?: SppsDevice;
  variant: string | null;
  /** The stage the run is in: solvers, geometry, validate, mesh, export, pre_launch, solve. */
  stage: string | null;
  /** The last PROGRESS line's percentage. */
  progress: number | null;
  /** The last PROGRESS line's text after its `#`, exactly as the solver printed it. */
  progressText: string;
  startedAt: number;
  /** When the solve stage began, on this page's clock (`Date.now()`): the live finish time's origin. */
  solveAt?: number;
  /** When the last PROGRESS line arrived, on this page's clock. */
  progressAt?: number;
  /** The PROGRESS lines the live finish projects from, each a share above every one before it, the last the highest
   * yet (runTime.ts `logProgress`, `liveFinishMs`). The bar shows `progress`, the share as last printed. */
  progressLog?: readonly ProgressPoint[];
  status: 'starting' | 'running' | 'cancelling';
}
export const runStore = new Store<ActiveRun | null>(null);

export type ClassCounts = Record<RunLineClass, number>;

/**
 * One run's solver lines as the stream delivered them: counts per class, the core's arrival
 * indices received (so the e2e can prove none was lost or doubled), and the lines other than
 * PROGRESS, which are counted, never rendered one by one (T15).
 */
/** A run's Console line before it is timed on arrival. */
export type RunLine = Omit<ConsoleLine, 'time'>;

export interface RunLog {
  counts: ClassCounts;
  seqs: Set<number>;
  dupes: number;
  lines: RunLine[];
  /** The stream's own `seq`: the last one received, and whether one was ever skipped. */
  lastEventSeq: number;
  gaps: number;
}
export const runLinesStore = new Store<ReadonlyMap<string, RunLog>>(new Map());

/** The Runs tab's rows; refreshed on open, save, import, a run's end, and the tab shown. */
export const runsStore = new Store<RunsView | null>(null);
/** The run the Results step shows (a Runs row click, the Simulate step's "Run n" link). */
export const selectedRunStore = new Store<string | null>(null);
/** Whether each run's results verify: never a value (M12 is the first that may show one). */
export const resultsStore = new Store<ReadonlyMap<string, ResultsState>>(new Map());
/** Each run's report as `run_report` read it (M12 P2: the Acoustics tab), fetched once per run. */
export const reportStore = new Store<ReadonlyMap<string, ReportView>>(new Map());
/** The solver the Simulate step runs: session state, not saved in the project. */
export const solverStore = new Store<SolverName>('spps');
/** Where the Simulate step runs SPPS: session state, not saved in the project (decision 70). */
export const deviceStore = new Store<SppsDevice>('cpu');
/** Whether SPPS can run on the GPU here (`spps-gpu --probe`, asked once per session); `null` until
 * the answer arrives. */
export const gpuStatusStore = new Store<GpuStatus | null>(null);
/** The four executables, checked against the verified build (at boot and before each run). */
export const solversStatusStore = new Store<SolversStatus | null>(null);

export type PromptChoice = 'save' | 'discard' | 'cancel';
/** The save prompt before New, Open and Exit (row 22, A9): open while set; the dialog answers. */
export const promptStore = new Store<{ name: string; resolve: (choice: PromptChoice) => void } | null>(null);
/** Upstream's reference materials, from the core (`material_library`), at boot. */
export const libraryStore = new Store<LibraryMaterial[]>([]);
/** C1: upstream's reference spectra on the open project's bands (`spectrum_library`), for the
 * bands they were read for (`frequencies_hz` joined); refetched when the bands change. */
export const spectrumLibraryStore = new Store<{ bands: string; list: LibrarySpectrum[] }>({ bands: '', list: [] });
/** C1: the surface group whose name the scene list is editing (F2), or null. */
export const groupRenameStore = new Store<string | null>(null);

function clock(): string {
  return new Date().toLocaleTimeString('en-GB', { hour12: false });
}

export function log(tag: ConsoleTag, text: string, extra?: Omit<ConsoleLine, 'time' | 'tag' | 'text'>): void {
  consoleStore.set([...consoleStore.get(), { time: clock(), tag, text, ...extra }]);
}

/** Appends ready-made lines (a run's batch) in one render. */
export function appendLines(lines: readonly Omit<ConsoleLine, 'time'>[]): void {
  if (lines.length === 0) return;
  const time = clock();
  consoleStore.set([...consoleStore.get(), ...lines.map((l) => ({ ...l, time }))]);
}

/** Appends several lines at once (one render). */
export function logAll(lines: readonly { class: LineClass; text: string }[]): void {
  if (lines.length === 0) return;
  const time = clock();
  consoleStore.set([...consoleStore.get(), ...lines.map((l) => ({ time, tag: l.class, text: l.text }))]);
}
