// The dock package's pure logic (docs/investigations/2026-09-29-m11/PLAN.md 3.3, 9.2): what the
// Console shows around a run's lines (one live PROGRESS line, and a counts strip computed from
// the lines received), the tab badges, and the words of a Runs row. No store, no backend, only
// erasable TypeScript, tested by model.test.ts under `node --test`.
//
// Every number a Runs row prints is a string Rust formatted from run.json (PLAN.md 2.3, T14), or
// a count. The UI formats no float.
import type { RunRow, RunStatusUi, SolverCheck } from '../../bindings/ipc.ts';
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

export function solverLabel(solver: string | null | undefined): string {
  if (solver === 'spps') return 'SPPS';
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

/** Whether the run's executables were checked against the verified build before it (C7). */
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

/** The file name of a path, either separator. */
export function baseName(path: string): string {
  const i = Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/'));
  return i < 0 ? path : path.slice(i + 1);
}

// ---- free text from the core -------------------------------------------------------------------------

// The no-acoustic-number check's two patterns, the same as app/e2e/lib/dom.ts ACOUSTIC_NUMBER
// and PARAMETER_NUMBER (the UI cannot import the harness). In M11 a number next to a unit may
// be shown only as a diagnostic the check can prove against run.json (PLAN.md 3.4 rule 1, 4.2).
// A reason's detail is the core's prose, which the check cannot prove: `particle_loss_excess`
// quotes the loss to four decimals ("2000 Hz: 3 of 150000 (0.0020 %)"). Such a detail is not
// shown; the row says it is in run.json, and the loss itself is shown per band, proven.
const UNIT_NUMBER = /\d\s*(dB|s|ms|%)(?![\p{L}\p{N}])/u;
const PARAMETER_NUMBER =
  /(?:\b(?:T15|T20|T30|T60|RT60|EDT|RT|C50|C80|D50|Ts|STI|SPL|LF|LFC|G)\b|\b(?:[Ss]abine|[Ee]yring|[Rr]everberation time)\b)\s*[:=·]?\s*[-+]?\d/;

export type Detail = { kind: 'none' } | { kind: 'shown'; text: string } | { kind: 'withheld' };

/** A reason's detail (or other core text) as the row may show it. */
export function detailView(text: string | null | undefined): Detail {
  const t = (text ?? '').trim();
  if (!t) return { kind: 'none' };
  if (UNIT_NUMBER.test(t) || PARAMETER_NUMBER.test(t)) return { kind: 'withheld' };
  return { kind: 'shown', text: t };
}
