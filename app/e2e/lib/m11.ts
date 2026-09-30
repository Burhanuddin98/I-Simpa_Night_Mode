// The M11 specs' shared pieces (PLAN.md 3.5, 3.6, 4.1): typed calls to the foundation's run hooks,
// the DOM reads of the Runs tab, the Console's counts and the Results step, the environment
// tools/gates/m11.ps1 passes, and the hand-off between the close, kill and after specs (each is
// its own session, so the run names travel through a file in the gate's work folder).
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { hook } from './hooks.ts';

/** Environment the gate passes (tools/gates/m11.ps1). */
export function need(name: string): string {
  const v = process.env[name];
  if (!v) throw new Error(`${name} unset: run tools/gates/m11.ps1`);
  return v;
}

/** The gate's private copy of the verified solvers: "no spps.exe" means none from this path. */
export const privateSpps = () => path.join(need('M11_SOLVERS'), 'spps.exe');

/** A project copied to C: by the gate (`<work>\p\<name>\<file>`), and its runs root. */
export const project = (name: string, file: string) => path.join(need('M11_P'), name, file);
export const runsRootOf = (projectFile: string) => path.join(path.dirname(projectFile), 'runs');

export type Milestone = 'started' | 'solve' | 'progress' | 'ended';

export interface RunState {
  run?: string;
  solver: string;
  stage: string | null;
  progress: number | null;
  progressText: string;
  status: 'starting' | 'running' | 'cancelling';
}

export interface RunLog {
  counts: Record<'PROGRESS' | 'INFO' | 'OK' | 'WARN' | 'FAIL', number>;
  n: number;
  min: number | null;
  max: number | null;
  dupes: number;
  gaps: number;
}

export interface RowView {
  run: string;
  number: number;
  status: string;
  elapsed_s?: string | null;
  loss?: { worst_pct: string; limit_pct: string } | null;
  reasons: { code: string; ui_code: string; detail: string }[];
}

export interface Cam {
  view: string;
  projection: string;
  position: number[];
  direction: number[];
  target: number[] | null;
}

/**
 * Waits for a run to reach `until`, in slices the WebDriver script timeout allows. `null` means
 * the next run to start (call it right after starting one): its name is taken at `started`
 * first, then the milestone is waited for by name.
 */
export async function waitRun(run: string | null, until: Milestone, ms: number): Promise<string> {
  const deadline = Date.now() + ms;
  const name = run ?? (await hook<string>('waitRun', null, 'started', Math.min(25_000, ms)));
  for (;;) {
    const left = deadline - Date.now();
    try {
      return await hook<string>('waitRun', name, until, Math.max(1, Math.min(20_000, left)));
    } catch (e) {
      if (Date.now() >= deadline || !/not reached/.test(String(e))) throw e;
    }
  }
}

export const m11 = {
  runStart: (solver: 'spps' | 'tcr') => hook<unknown>('runStart', solver),
  runCancel: () => hook<boolean>('runCancel'),
  runState: () => hook<RunState | null>('runState'),
  runLog: (run: string) => hook<RunLog | null>('runLog', run),
  runsRows: () => hook<RowView[]>('runsRows'),
  selectRun: (run: string | null) => hook<true>('selectRun', run),
  resultsState: (run: string) =>
    hook<{
      run: string;
      verified: boolean;
      refusal?: { code: string; ui_code: string } | null;
      unverified?: { code: string; ui_code: string } | null;
    }>('resultsState', run),
  pingStats: (n: number) => hook<number[]>('pingStats', n),
  frameBegin: () => hook<true>('frameStats.begin'),
  frameEnd: () => hook<number[]>('frameStats.end'),
  pid: () => hook<number>('pid'),
  cameraState: () => hook<Cam>('cameraState'),
  issues: () => hook<{ code: string; rule: string; path: string; severity: string }[]>('issues'),
};

/** Resolves at wall-clock time `t` (ms since the epoch). */
export async function sleepUntil(t: number): Promise<void> {
  const left = t - Date.now();
  if (left > 0) await new Promise<void>((r) => setTimeout(r, left));
}

/** Text as shown, whitespace collapsed. */
export const shown = (s: string) => s.replace(/\s+/g, ' ').trim();

/** Shows a dock tab, as a click does. */
export async function showTab(tab: 'acoustics' | 'console' | 'runs'): Promise<void> {
  const t = await $(`[data-dock-tab="${tab}"]`);
  await t.waitForExist({ timeout: 30_000 });
  await t.click();
}

/** An element as `$` gives it (the specs import no WebdriverIO type). */
export type El = ReturnType<typeof $>;

/** The Runs row of `run` (the Runs tab must be shown), once it exists. */
export async function runRow(run: string): Promise<El> {
  const row = $(`[data-run-row="${run}"]`);
  await row.waitForExist({ timeout: 30_000, timeoutMsg: `no Runs row [data-run-row="${run}"] within 30 s` });
  return row;
}

/** A Runs row's status: its `data-status` and the text of its `[data-part="status"]`. */
export async function rowStatus(run: string): Promise<{ attr: string | null; text: string }> {
  const row = await runRow(run);
  const status = await row.$('[data-part="status"]');
  return { attr: await row.getAttribute('data-status'), text: shown(await status.getText()) };
}

/** The Console's count of `cls` for `run`, from `[data-run-counts=<run>] [data-count=<cls>]` (the
 * Console tab must be shown): the number in its text, with the class word and separators gone. */
export async function consoleCount(run: string, cls: string): Promise<number> {
  const el = await $(`[data-run-counts="${run}"] [data-count="${cls}"]`);
  await el.waitForExist({ timeout: 30_000, timeoutMsg: `no [data-run-counts="${run}"] [data-count="${cls}"]` });
  const digits = shown(await el.getText()).replace(cls, '').replace(/\D/g, '');
  return digits === '' ? Number.NaN : Number(digits);
}

// ---- the close, kill and after specs' hand-off ----------------------------------------------

const handoff = () => path.join(need('M11_GATEWORK'), 'd-runs.json');

/** Records a (d) spec's run name for the after spec. */
export function recordRun(key: 'close' | 'kill', run: string): void {
  const f = handoff();
  const all = existsSync(f) ? (JSON.parse(readFileSync(f, 'utf8')) as Record<string, string>) : {};
  all[key] = run;
  writeFileSync(f, JSON.stringify(all, null, 2));
}

/** The run a (d) spec recorded. */
export function recordedRun(key: 'close' | 'kill'): string {
  const f = handoff();
  if (!existsSync(f)) throw new Error(`${f} is missing: the close and kill specs did not record their runs`);
  const run = (JSON.parse(readFileSync(f, 'utf8')) as Record<string, string>)[key];
  if (!run) throw new Error(`the ${key} spec recorded no run in ${f}`);
  return run;
}
