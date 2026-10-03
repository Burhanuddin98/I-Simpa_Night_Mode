// Pure pieces of the M11 run and save flows (docs/investigations/2026-09-29-m11/PLAN.md 3.2),
// shared by actions.ts, the test hooks and the packages' components. No store, no backend, only
// erasable TypeScript, tested by flow.test.ts under `node --test`.
import type { ProjectInfo, RunRow, RunStreamEvent, SceneState, SolversStatus } from './bindings/ipc.ts';
import type { ClassCounts, ConsoleLine, ConsoleTag, LinePart, RunLine, RunLog, SolverName } from './store.ts';

/** The app's own run blocker: a run is active (the backend refuses a second one). */
export const RUN_ACTIVE = 'RUN_ACTIVE';

/**
 * Why the project may not run with `solver`, as UI codes: `SceneState.run_blockers`, then that
 * solver's own errors (`SceneState.solver_issues`, PQ3's `NO_BAND_COMPUTED`), each once. The
 * other solver's do not block it. `null` with no project. What `joinBlockers` takes as `project`.
 */
export function projectBlockers(
  scene: Pick<SceneState, 'run_blockers' | 'solver_issues'> | null,
  solver: SolverName,
): string[] | null {
  if (!scene) return null;
  const out = [...scene.run_blockers];
  for (const i of scene.solver_issues[solver]) {
    if (i.severity === 'error' && !out.includes(i.code)) out.push(i.code);
  }
  return out;
}

/**
 * Why Run is disabled, as UI codes: the project's own (`SceneState.run_blockers`), then the
 * solvers' (`SOLVER_NOT_FOUND`, `SOLVER_UNVERIFIED`), then `RUN_ACTIVE`. `null` with no project.
 * The Run button's `data-blockers` and the `runBlockers` test hook both read this.
 */
export function joinBlockers(
  project: readonly string[] | null,
  solvers: SolversStatus | null,
  runActive: boolean,
): string[] | null {
  if (project === null) return null;
  const out = [...project];
  for (const b of solvers?.blockers ?? []) if (!out.includes(b)) out.push(b);
  if (runActive && !out.includes(RUN_ACTIVE)) out.push(RUN_ACTIVE);
  return out;
}

/**
 * Whether leaving the project should ask to save it (row 22, A9): only when the user changed
 * something that is not on disk. A project with a file and unsaved edits asks; so does a project
 * never saved that has been edited. A project never saved and never edited (a new empty one, or a
 * model just imported, whose source file is untouched) holds no work of the user's, and does not
 * ask (product question PQ7 of M11's foundation, default "does not ask").
 */
export function needsSavePrompt(info: Pick<ProjectInfo, 'dirty' | 'path' | 'undo_depth'> | null): boolean {
  if (!info || !info.dirty) return false;
  return info.path !== null || info.undo_depth > 0;
}

export function emptyCounts(): ClassCounts {
  return { PROGRESS: 0, INFO: 0, OK: 0, WARN: 0, FAIL: 0 };
}

export function emptyLog(): RunLog {
  return { counts: emptyCounts(), seqs: new Set(), dupes: 0, lines: [], lastEventSeq: -1, gaps: 0 };
}

/** A PROGRESS line's text after its `#`, exactly as the solver printed it (`#25.22` is `25.22`). */
export function progressText(text: string): string {
  const i = text.indexOf('#');
  return i < 0 ? text.trim() : text.slice(i + 1).trim();
}

/** What one streamed event does to its run's log, and the Console line it adds, if any. */
export interface Folded {
  log: RunLog;
  line: RunLine | null;
}

/**
 * Folds one stream event into `log` (a new object; the seq set is shared and grown): the event
 * `seq` is checked for continuity, a solver line is counted by class and its core `solver_seq`
 * recorded (a repeat counts as a dupe), and every line but PROGRESS becomes a verbatim Console
 * line of the run. A mesh line is shown, never counted: the manifest counts the solver's lines.
 */
export function foldEvent(log: RunLog, run: string, e: RunStreamEvent): Folded {
  const gaps = log.gaps + (e.seq === log.lastEventSeq + 1 ? 0 : 1);
  const next: RunLog = { ...log, lastEventSeq: e.seq, gaps };
  if (e.kind !== 'line') return { log: next, line: null };
  let line: RunLine | null = null;
  if (e.class !== 'PROGRESS') {
    line = { tag: e.class as ConsoleTag, text: e.text, source: e.source, run, verbatim: true };
    next.lines = [...log.lines, line];
  }
  if (e.source === 'solver') {
    next.counts = { ...log.counts, [e.class]: log.counts[e.class] + 1 };
    const s = e.solver_seq ?? -1;
    if (next.seqs.has(s)) next.dupes = log.dupes + 1;
    else next.seqs.add(s);
  }
  return { log: next, line };
}

/** `run.json`'s counts for a row, in the Console's classes. */
export function rowCounts(row: RunRow): ClassCounts | null {
  const l = row.lines;
  if (!l) return null;
  return { PROGRESS: l.progress, INFO: l.info, OK: l.ok, WARN: l.warn, FAIL: l.fail };
}

const STATUS_WORD: Record<RunRow['status'], string> = {
  OK: 'OK',
  FAIL: 'FAIL',
  CRASH: 'CRASH',
  CANCELLED: 'Cancelled',
  RUNNING: 'Running',
  INTERRUPTED: 'Interrupted',
};

/** A status as the Runs tab and the Console spell it (PQ5). */
export function statusWord(status: RunRow['status']): string {
  return STATUS_WORD[status];
}

/**
 * The app's Console line for a run that ended: OK for OK, FAIL for FAIL and CRASH, INFO for
 * Cancelled. "Run #n · SPPS finished · OK · Particles lost 0.00 % (limit 1 %)", or the reasons'
 * UI codes. Every number in it is the row's own string, in a diagnostic part.
 */
export function endLine(row: RunRow): Omit<ConsoleLine, 'time'> {
  const tag: ConsoleTag =
    row.status === 'OK' ? 'OK' : row.status === 'FAIL' || row.status === 'CRASH' ? 'FAIL' : 'INFO';
  const solver = (row.solver ?? 'run').toUpperCase();
  const parts: LinePart[] = [{ text: `Run #${row.number} · ${solver} finished · ${statusWord(row.status)}` }];
  if (row.status === 'OK' && row.loss) {
    parts.push({ text: ' · Particles lost ' });
    parts.push({ text: `${row.loss.worst_pct} %`, diagnostic: 'loss_pct', run: row.run });
    parts.push({ text: ' (limit ' });
    parts.push({ text: `${row.loss.limit_pct} %`, diagnostic: 'loss_limit_pct', run: row.run });
    parts.push({ text: ')' });
  } else if (row.reasons.length > 0) {
    parts.push({ text: ` · ${row.reasons.map((r) => `${r.ui_code} (${r.code})`).join(', ')}` });
  }
  return { tag, text: parts.map((p) => p.text).join(''), source: 'app', run: row.run, parts };
}

/**
 * The keys WebView2 takes for a reload of the page (its browser accelerators, on by default):
 * F5 and Ctrl+F5 or Shift+F5, Ctrl+R and Ctrl+Shift+R. A reload mid-run loses the page's record
 * of the run (M11 review 2, app 2), so the app's key handler cancels each of these, text field or
 * not; F5 alone outside a text field is still the app's Run key.
 */
export function isReloadKey(e: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey'>): boolean {
  if (e.altKey) return false;
  return e.key === 'F5' || ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'r');
}

// ---- free text from the core -------------------------------------------------------------------------

// The no-acoustic-number check's two patterns, the same as app/e2e/lib/dom.ts ACOUSTIC_NUMBER
// and PARAMETER_NUMBER (the UI cannot import the harness). In M11 a number next to a unit may
// be shown only as a diagnostic the check can prove against run.json (PLAN.md 3.4 rule 1, 4.2).
// A reason's detail is the core's prose, which the check cannot prove: `particle_loss_excess`
// quotes the loss to four decimals ("2000 Hz: 3 of 150000 (0.0020 %)"). Such a detail is not
// shown, in the text or in a tooltip; the Runs row says it is in run.json, and the loss itself
// is shown per band, proven.
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

/** What a withheld detail's place says instead (the Runs row's title, and every tooltip). */
export const WITHHELD_DETAIL =
  'This detail quotes numbers with units. Until the physics checks behind them pass, only values proven against run.json are shown.';

/**
 * A reason's detail as a tooltip may carry it: the text when the row could show it, the
 * explanation when it is withheld, nothing when there is none. A tooltip is shown to the user as
 * surely as the text is, so the same rule holds (M11 review 2, app 4: the Simulate step put the
 * raw detail, the dock's withheld loss quote included, in a `title`).
 */
export function detailTitle(text: string | null | undefined): string | undefined {
  const d = detailView(text);
  return d.kind === 'shown' ? d.text : d.kind === 'withheld' ? WITHHELD_DETAIL : undefined;
}
