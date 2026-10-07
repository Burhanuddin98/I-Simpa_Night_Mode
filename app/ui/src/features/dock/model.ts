// The dock package's pure logic (docs/investigations/2026-09-29-m11/PLAN.md 3.3, 9.2): what the
// Console shows around a run's lines (one live PROGRESS line, and a counts strip computed from
// the lines received), the tab badges, and the words of a Runs row. No store, no backend, only
// erasable TypeScript, tested by model.test.ts under `node --test`.
//
// Every number a Runs row prints is a string Rust formatted from run.json (PLAN.md 2.3, T14), or
// a count. The UI formats no float.
import type { ReasonUi, RunRow, RunStatusUi, SolverCheck } from '../../bindings/ipc.ts';
import { rowCounts } from '../../flow.ts';
import type { ActiveRun, ClassCounts, ConsoleLine, RunLog } from '../../store.ts';

/** The Console's classes, in the order the counts strip prints them. */
export const CLASSES: readonly (keyof ClassCounts)[] = ['PROGRESS', 'INFO', 'OK', 'WARN', 'FAIL'];

// ---- the Console ----------------------------------------------------------------------------------

/** Whether a run's streamed counts equal its `run.json`'s. */
export interface CountsCheck {
  match: boolean;
  manifest: ClassCounts;
}

export type ConsoleItem =
  /** A line of `consoleStore`, as it was logged. */
  | { kind: 'line'; key: string; line: ConsoleLine }
  /**
   * The active run's one PROGRESS line, updated in place (T15): SPPS's latest `#` line exactly as
   * it printed it, or the stage while no `#` line has come yet.
   */
  | {
      kind: 'live';
      key: string;
      run: string;
      solver: string;
      stage: string | null;
      /** The latest `#` line, verbatim (`#25.22`); empty before the first. */
      verbatim: string;
      cancelling: boolean;
    }
  /** A run's class counts, computed from the lines the stream delivered (`runLinesStore`). */
  | {
      kind: 'counts';
      key: string;
      run: string;
      /** "Run #n" once the run is listed, else its folder's name. */
      label: string;
      counts: ClassCounts;
      /** Against `run.json`'s `lines`, once the run has ended and is listed with them. */
      check: CountsCheck | null;
      /** Stream events skipped, and solver lines received twice (both 0 when nothing was lost). */
      gaps: number;
      dupes: number;
    };

export function countsEqual(a: ClassCounts, b: ClassCounts): boolean {
  return CLASSES.every((c) => a[c] === b[c]);
}

function countsItem(run: string, log: RunLog, rows: readonly RunRow[] | null, live: boolean): ConsoleItem {
  const row = rows?.find((r) => r.run === run);
  const manifest = !live && row ? rowCounts(row) : null;
  return {
    kind: 'counts',
    key: `c:${run}`,
    run,
    label: row && row.number > 0 ? `Run #${row.number}` : run,
    counts: log.counts,
    check: manifest ? { match: countsEqual(log.counts, manifest), manifest } : null,
    gaps: log.gaps,
    dupes: log.dupes,
  };
}

/**
 * The Console's items, in order: every logged line; after a run's last line, that run's counts
 * strip; and at the bottom, the active run's live PROGRESS line and its counts. A run with a log
 * but no line in the Console gets its strip at the end. PROGRESS lines are never items of their
 * own: SPPS prints thousands, and they are counted (T15).
 */
export function consoleItems(
  lines: readonly ConsoleLine[],
  logs: ReadonlyMap<string, RunLog>,
  active: ActiveRun | null,
  rows: readonly RunRow[] | null,
): ConsoleItem[] {
  const liveRun = active?.run;
  const last = new Map<string, number>();
  lines.forEach((l, i) => {
    if (l.run !== undefined && logs.has(l.run)) last.set(l.run, i);
  });
  const after = new Map<number, string[]>();
  const trailing: string[] = [];
  for (const run of logs.keys()) {
    if (run === liveRun) continue;
    const i = last.get(run);
    if (i === undefined) trailing.push(run);
    else after.set(i, [...(after.get(i) ?? []), run]);
  }
  const out: ConsoleItem[] = [];
  lines.forEach((line, i) => {
    out.push({ kind: 'line', key: `l${i}`, line });
    for (const run of after.get(i) ?? []) out.push(countsItem(run, logs.get(run) as RunLog, rows, false));
  });
  for (const run of trailing) out.push(countsItem(run, logs.get(run) as RunLog, rows, false));
  if (active && liveRun !== undefined) {
    out.push({
      kind: 'live',
      key: `p:${liveRun}`,
      run: liveRun,
      solver: active.solver.toUpperCase(),
      stage: active.stage,
      verbatim: active.progressText ? `#${active.progressText}` : '',
      cancelling: active.status === 'cancelling',
    });
    const log = logs.get(liveRun);
    if (log) out.push(countsItem(liveRun, log, rows, true));
  }
  return out;
}

/** The stages of a run as the Console's live line and a Running row name them. */
const STAGES: Record<string, string> = {
  solvers: 'Checking the solvers',
  geometry: 'Checking the model',
  validate: 'Validating the project',
  mesh: 'Meshing',
  export: 'Writing the solver input',
  pre_launch: 'Launching the solver',
  solve: 'Solving',
};

export function stageLabel(stage: string | null | undefined): string {
  if (!stage) return 'Starting';
  return STAGES[stage] ?? stage;
}

/**
 * SPPS's percentage (the text after its `#`) as a `progress_pct` diagnostic shows it: as printed
 * when it has at most two decimals, else rounded half up to two, in integers. SPPS prints four
 * significant digits (`std::cout.precision(4)`, progressionInfo.h:154), so below 10 % it prints
 * three or more decimals (`1.234`, `0.06667`, `1e-05`), which the field's grammar,
 * `^\d{1,3}(\.\d{1,2})? %$`, does not allow; m11-h proves the value against the run's `#` lines
 * at the digits shown (PLAN.md 4.2). No float is formatted. `null` when the text is not a
 * non-negative decimal.
 */
export function progressPct(text: string): string | null {
  const m = /^(\d*)(?:\.(\d*))?(?:[eE]([+-]?\d+))?$/.exec(text.trim());
  if (!m || (m[1] === '' && (m[2] ?? '') === '')) return null;
  const whole = m[1] === '' ? '0' : m[1];
  const frac = m[2] ?? '';
  if (m[3] === undefined && frac.length <= 2) return frac ? `${whole}.${frac}` : whole;
  let n = BigInt(`${whole}${frac}`);
  let k = frac.length - Number(m[3] ?? '0');
  if (k < 0) {
    n *= 10n ** BigInt(-k);
    k = 0;
  }
  const hundredths = k <= 2 ? n * 10n ** BigInt(2 - k) : (n * 2n + 10n ** BigInt(k - 2)) / (2n * 10n ** BigInt(k - 2));
  return `${hundredths / 100n}.${String(hundredths % 100n).padStart(2, '0')}`;
}

// ---- the tabs' badges ------------------------------------------------------------------------------

/** The Console's badge: "live" while a run is active, else "<n> fail", else none. */
export function consoleBadge(
  lines: readonly Pick<ConsoleLine, 'tag'>[],
  runActive: boolean,
): { kind: 'live' | 'fail'; text: string } | null {
  if (runActive) return { kind: 'live', text: 'live' };
  const fails = lines.filter((l) => l.tag === 'FAIL').length;
  return fails > 0 ? { kind: 'fail', text: `${fails} fail` } : null;
}

/** The Runs tab's badge: the number of rows listed, or none while no list is loaded. */
export function runsBadge(view: { rows: readonly unknown[] } | null): string | null {
  return view ? String(view.rows.length) : null;
}

// ---- a Runs row -------------------------------------------------------------------------------------

/** The rows as the design lists them: the newest first. */
export function newestFirst(rows: readonly RunRow[]): RunRow[] {
  return [...rows].reverse();
}

/** A row read from `run.json`; Running and Interrupted rows have none, nor does an unreadable one. */
export function hasManifest(row: RunRow): boolean {
  return row.status !== 'RUNNING' && row.status !== 'INTERRUPTED' && !row.manifest_error;
}

/**
 * The Variant column: the variant's name in the open project, "Baseline" for the project's own
 * materials, "unknown variant" for an id the project no longer has. A row with no `run.json`
 * records no variant ("—"), except the active run, whose variant the app knows.
 */
export function variantLabel(
  row: RunRow,
  variants: readonly { id: string; name: string }[] | null,
  active: Pick<ActiveRun, 'run' | 'variant'> | null,
): string {
  const name = (id: string | null | undefined) =>
    !id ? 'Baseline' : (variants?.find((v) => v.id === id)?.name ?? 'unknown variant');
  if (row.status === 'RUNNING' && active?.run === row.run) return name(active.variant);
  if (!hasManifest(row)) return '—';
  return name(row.variant);
}

/** The Runs tab's solver column: `SPPS`, `SPPS on the GPU` (the run's `gpu_device`, or the
 * active run's device before its run.json exists), `TCR`. */
export function solverLabel(solver: string | null | undefined, onGpu = false): string {
  if (solver === 'spps') return onGpu ? 'SPPS on the GPU' : 'SPPS';
  if (solver === 'tcr') return 'TCR';
  return '—';
}

/** The status's colour class. The status is always its word too: never colour alone. */
export function statusTone(status: RunStatusUi): 'ok' | 'fail' | 'muted' | 'warn' | 'live' {
  switch (status) {
    case 'OK':
      return 'ok';
    case 'FAIL':
    case 'CRASH':
      return 'fail';
    case 'CANCELLED':
      return 'muted';
    case 'INTERRUPTED':
      return 'warn';
    case 'RUNNING':
      return 'live';
  }
}

export function warningsText(n: number): string {
  return `${n} warning${n === 1 ? '' : 's'}`;
}

/** The first 12 hex digits of a sha256; the whole value goes in the title. */
export function shortSha(sha: string): string {
  return sha.slice(0, 12);
}

/** The run's exit code; an NTSTATUS crash code also in hex, as the verdict names it. */
export function exitText(code: number | null | undefined): string {
  if (code === null || code === undefined) return 'no exit code';
  if (code > 0xffff) return `exit ${code} (0x${code.toString(16).toUpperCase().padStart(8, '0')})`;
  return `exit ${code}`;
}

/**
 * What a run's recorded checks say of themselves (C7): none recorded, all matching, or which
 * failed. Not the build's verdict, which also needs a check of the solver the run executed: the
 * Runs row shows the core's (`buildMark`).
 */
export interface SolversMark {
  kind: 'verified' | 'unverified' | 'unrecorded';
  /** Every checked file when verified; the files that failed otherwise. */
  names: string[];
}

export function solversMark(checks: readonly SolverCheck[] | null | undefined): SolversMark {
  if (!checks || checks.length === 0) return { kind: 'unrecorded', names: [] };
  const bad = checks.filter((c) => !c.matches).map((c) => c.name);
  return bad.length === 0 ? { kind: 'verified', names: checks.map((c) => c.name) } : { kind: 'unverified', names: bad };
}

/** The Runs row's mark for the run's solver build (backlog 38). */
export interface BuildMark extends SolversMark {
  /** The reason the build is not verified, as the backend gives it; null when verified. */
  reason: ReasonUi | null;
}

/** The core's code for a run whose run.json records no check of its executables. */
const BUILD_UNRECORDED = 'solver_build_unrecorded';

/**
 * The Runs row's mark for the solver build: the core's verdict as `runs_list` sends it
 * (`RunRow.solver_build`, from `results::solver_build`, which the Results step's answer comes from
 * too), never one worked out here from the checks (C5). `unrecorded` when the core says no check
 * was recorded, or the row has no run.json that reads. The names, for the title, are the checked
 * files, or the ones that failed (`solversMark`).
 */
export function buildMark(row: Pick<RunRow, 'solvers' | 'solver_build'>): BuildMark {
  const names = solversMark(row.solvers).names;
  const b = row.solver_build;
  if (b?.status === 'verified') return { kind: 'verified', names, reason: null };
  const reason = b?.status === 'unverified' ? b.reason : null;
  return { kind: !reason || reason.code === BUILD_UNRECORDED ? 'unrecorded' : 'unverified', names, reason };
}

/** The file name of a path, either separator. */
export function baseName(path: string): string {
  const i = Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/'));
  return i < 0 ? path : path.slice(i + 1);
}

// ---- the dock's height (C2, decision-log row 74) -----------------------------------------------------
//
// The dock's top edge drags it from the tab strip alone (folded) to the whole work area, where the
// 3D view is hidden (maximised). In between, the 3D view always keeps VIEW_MIN, so the Simulate
// step's live run stays in sight: a drag past that point maximises, a drag below DOCK_MIN folds.
// Either way the height the dock had before the drag is kept, and is where it comes back to.
// `room` is the dock's column less the gap above it: the 3D view and the dock share it.

/** The tab strip and the dock's borders: the folded dock's height. */
export const DOCK_STRIP = 36;
/** The least height of a dock with its body shown; a drag below it folds the dock. */
export const DOCK_MIN = 60;
/** The opening height (the dock's fixed height before C2). */
export const DOCK_DEFAULT = 250;
/** The least height the 3D view keeps beside a dock that is not maximised. */
export const VIEW_MIN = 120;
/** The handle's arrow-key step; Shift moves four. */
export const KEY_STEP = 24;
/** From this height the Acoustics tab lays its cards out in rows across the width. */
export const DOCK_TALL = 440;
/** The tallest height kept: a stored value above it is not one this app wrote. */
const STORED_MAX = 10_000;

/** The dock's height when it is not maximised, and whether it is maximised. */
export interface DockSize {
  height: number;
  max: boolean;
}
/** A size and the fold (chrome/fold.tsx), which is the dock's minimum. */
export interface DockPlace {
  size: DockSize;
  folded: boolean;
}

/** The height a dock that is not maximised is drawn at in `room`. */
export function clampHeight(height: number, room: number): number {
  return Math.max(DOCK_MIN, Math.min(height, room - VIEW_MIN));
}

/** The height the unfolded dock takes of `room` (maximised, all of it). */
export function dockHeight(size: DockSize, room: number): number {
  return size.max ? Math.max(room, DOCK_MIN) : clampHeight(size.height, room);
}

/**
 * Where a drag of the top edge to `height` puts a dock that was at `start` when the drag began:
 * folded below DOCK_MIN, maximised when the 3D view would be left less than VIEW_MIN, else that
 * height, in whole pixels. Folding and maximising keep the start height to come back to.
 */
export function dragTo(start: DockSize, height: number, room: number): DockPlace {
  if (height < DOCK_MIN) return { size: { height: start.height, max: false }, folded: true };
  if (room - height < VIEW_MIN) return { size: { height: start.height, max: true }, folded: false };
  return { size: { height: Math.round(height), max: false }, folded: false };
}

/** The maximise button and a double-click on the tab strip: full height and back. A folded
 * dock opens maximised. */
export function toggleMax(p: DockPlace): DockPlace {
  return { size: { height: p.size.height, max: p.folded ? true : !p.size.max }, folded: false };
}

/** Escape: a maximised dock goes back to its height; otherwise nothing (null). */
export function escapeMax(p: DockPlace): DockPlace | null {
  return p.size.max && !p.folded ? { size: { height: p.size.height, max: false }, folded: false } : null;
}

/**
 * A key on the focused handle: the arrows move the edge by KEY_STEP (four with Shift), Home folds,
 * End maximises, Enter or Space toggles maximised. Null when the key does nothing here.
 */
export function keyTo(p: DockPlace, key: string, shift: boolean, room: number): DockPlace | null {
  const step = KEY_STEP * (shift ? 4 : 1);
  switch (key) {
    case 'Home':
      return p.folded ? null : { size: { height: p.size.height, max: false }, folded: true };
    case 'End':
      return p.size.max && !p.folded ? null : { size: { height: p.size.height, max: true }, folded: false };
    case 'Enter':
    case ' ':
      return toggleMax(p);
    case 'ArrowUp':
      if (p.folded) return { size: { height: p.size.height, max: false }, folded: false };
      if (p.size.max) return null;
      return dragTo(p.size, dockHeight(p.size, room) + step, room);
    case 'ArrowDown':
      if (p.folded) return null;
      if (p.size.max) return escapeMax(p);
      return dragTo(p.size, dockHeight(p.size, room) - step, room);
    default:
      return null;
  }
}

/** Whether the Acoustics tab has the height to lay its cards out in rows. */
export function dockTall(p: DockPlace): boolean {
  return !p.folded && (p.size.max || p.size.height >= DOCK_TALL);
}

/**
 * The stored height (localStorage, like the fold): whole pixels from DOCK_MIN up, else the
 * opening height. Maximised is not stored: a new launch never opens with the 3D view hidden.
 */
export function parseDockHeight(text: string | null): number {
  if (text === null || !/^\d{1,5}$/.test(text)) return DOCK_DEFAULT;
  const h = Number(text);
  return h >= DOCK_MIN && h <= STORED_MAX ? h : DOCK_DEFAULT;
}

/** The height as it is stored. */
export function dockHeightText(height: number): string {
  return String(Math.round(Math.max(DOCK_MIN, Math.min(height, STORED_MAX))));
}

// ---- free text from the core -------------------------------------------------------------------------

// A reason's detail is the core's prose, which the no-acoustic-number check cannot prove; the
// rule that withholds one quoting a number with a unit is shared with the Simulate and Results
// steps, which show the same details in their tooltips (flow.ts, M11 review 2, app 4).
export { type Detail, detailView } from '../../flow.ts';
