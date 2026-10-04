// The Acoustics tab as an e2e spec drives and reads it (M12, docs/investigations/2026-10-03-m12/
// PLAN.md): shared by m12.acoustics (gate (a), (b), (e), (f)) and m12.bedplant (gate (b)'s plant).
// Moved here from m12.acoustics.e2e.ts unchanged by P4, so both specs drive the tab one way.
// The tab's own expectations (labels, words, marks) are in acoustics.ts; nothing here imports
// the UI.
import { strict as assert } from 'node:assert';
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { EDT_MARKS, type LabelledCell, labelPattern, PARAM_LABELS, type NumEl, type SeriesView, T30_MARK } from './acoustics.ts';
import { hook, m10 } from './hooks.ts';
import { env } from './types.ts';

export const repo = (rel: string) => path.join(env('M11_REPO'), rel);
export const PANEL = '[data-dock-panel="acoustics"]';

export interface Row {
  run: string;
  number: number;
  status: string;
  variant?: string | null;
}
export interface Series extends SeriesView {
  param: string;
}
export interface View {
  state: string;
  run: string | null;
  receiver: number;
  band: string;
  din: string;
  series: Series[];
}
export interface ScanCell {
  param: string;
  receiver: string;
  band: string;
  status: string;
  nums: string[];
  refusal: string | null;
  note: string | null;
  /** The cell as a reader takes it: its row and column heads and its card's selections, read
   * from the rendered DOM, with every JSON path inside it (lib/acoustics.ts `cellLabelMismatch`). */
  labelled: LabelledCell;
}
export interface Scan {
  state: string | null;
  run: string | null;
  nums: NumEl[];
  strs: { path: string; text: string }[];
  labels: { kind: string; param: string | null; text: string }[];
  cells: ScanCell[];
  params: string[];
  stray: string;
  options: { control: string; value: string; text: string }[];
  text: string;
}

/** A bed summary's statuses (`beds/summary.json`, or a plant of its shape). */
export interface BedSummary {
  parameters: Record<string, { status: string; reasons?: string[] }>;
}

/** `beds/summary.json` as committed: what the build compiled in. */
export const summary = (): BedSummary => JSON.parse(readFileSync(repo('beds/summary.json'), 'utf8')) as BedSummary;

/** Copies the box into `dir`, so its runs root is that spec's own. */
export function ownBox(dir: string): string {
  mkdirSync(dir, { recursive: true });
  const to = path.join(dir, 'box_run.simpa');
  copyFileSync(repo('tests/fixtures/ui/box_run.simpa'), to);
  return to;
}

/** `simpa results <run> --json`, parsed: the CLI's report, outside the app. The CLI inherits this
 * process's environment, so it reads the bed as the app does (`$SIMPA_BED_DEMOTE` included). */
export function cliReport(project: string, run: string): Record<string, unknown> {
  const dir = path.join(path.dirname(project), 'runs', run);
  const out = execFileSync(env('M11_SIMPA'), ['results', dir, '--json'], { maxBuffer: 1 << 30, encoding: 'utf8' });
  return JSON.parse(out) as Record<string, unknown>;
}

/** Runs SPPS from the app and waits for it to end OK; its Runs row. */
export async function runSpps(): Promise<Row> {
  const before = new Set((await hook<Row[]>('runsRows')).map((r) => r.run));
  await hook('setSolver', 'spps');
  await hook('runStart', 'spps');
  let fresh: Row | undefined;
  await browser.waitUntil(
    async () => {
      if ((await hook<unknown>('runState')) !== null) return false;
      fresh = (await hook<Row[]>('runsRows')).find((r) => !before.has(r.run) && r.status !== 'RUNNING');
      return fresh !== undefined;
    },
    { timeout: 900_000, interval: 500, timeoutMsg: 'no SPPS run ended within 900 s' },
  );
  await m10.idle();
  assert.equal(fresh?.status, 'OK', `the run ended ${fresh?.status}`);
  return fresh as Row;
}

/** Shows the Results step and the Acoustics tab, and waits until it shows `run`, ready. */
export async function showRun(run: string): Promise<void> {
  await m10.setStep('results');
  await hook('dockTab', 'acoustics');
  await browser.waitUntil(
    async () => {
      const v = await hook<View | null>('acousticsView');
      return v !== null && v.run === run && v.state === 'ready';
    },
    { timeout: 60_000, interval: 250, timeoutMsg: `the Acoustics tab did not show ${run} ready` },
  );
}

/** Picks an option of one of the tab's controls, as a user does, and waits for the page. */
export async function pick(control: string, value: string): Promise<void> {
  const sel = await $(`${PANEL} select[data-control="${control}"]`);
  await sel.waitForExist({ timeout: 10_000 });
  await sel.selectByAttribute('value', value);
  await browser.waitUntil(async () => (await sel.getValue()) === value, { timeout: 10_000 });
  await m10.idle();
}

/** The tab as shown: its marked numbers, strings and words, its cells, and its text with all of
 * them, its controls and the run label removed (`stray`), which must hold no digit. */
export const scan = (): Promise<Scan> =>
  browser.execute((panelSel: string) => {
    const panel = document.querySelector<HTMLElement>(panelSel);
    const root = panel?.querySelector<HTMLElement>('[data-acoustics]') ?? null;
    const all = (sel: string) => (panel ? [...panel.querySelectorAll<HTMLElement>(sel)] : []);
    const clone = panel ? (panel.cloneNode(true) as HTMLElement) : null;
    clone?.querySelectorAll('[data-num], [data-str], [data-label], select, [data-run-label]').forEach((e) => e.remove());
    return {
      state: root?.getAttribute('data-acoustics-state') ?? null,
      run: root?.getAttribute('data-run') ?? null,
      nums: all('[data-num]').map((e) => ({
        path: e.getAttribute('data-json') ?? '',
        text: e.textContent ?? '',
        digits: e.getAttribute('data-digits'),
        scale: e.getAttribute('data-scale'),
      })),
      strs: all('[data-str]').map((e) => ({ path: e.getAttribute('data-json') ?? '', text: e.textContent ?? '' })),
      labels: all('[data-label]').map((e) => ({ kind: e.getAttribute('data-label') ?? '', param: e.getAttribute('data-param'), text: e.textContent ?? '' })),
      cells: all('[data-cell]').map((e) => ({
        param: e.getAttribute('data-param') ?? '',
        receiver: e.getAttribute('data-receiver') ?? '',
        band: e.getAttribute('data-band') ?? '',
        status: e.getAttribute('data-status') ?? '',
        nums: [...e.querySelectorAll('[data-num]')].map((n) => n.getAttribute('data-json') ?? ''),
        refusal: e.querySelector('[data-refusal]')?.getAttribute('data-refusal') ?? null,
        note: e.querySelector('[data-note]')?.getAttribute('data-note') ?? null,
        labelled: (() => {
          const tr = e.closest('tr');
          const table = e.closest('table');
          const idx = tr ? [...tr.children].indexOf(e) : -1;
          const th = table?.querySelector('thead tr')?.children[idx] ?? null;
          const card = e.closest('section');
          const selected = (control: string) => {
            const sel = card?.querySelector<HTMLSelectElement>(`select[data-control="${control}"]`);
            return sel ? (sel.options[sel.selectedIndex]?.textContent ?? '') : '';
          };
          return {
            table: table?.getAttribute('data-part') ?? '',
            rowHead: tr?.children[0]?.textContent ?? '',
            colHead: th?.querySelector('[data-label="param"]')?.textContent ?? th?.textContent ?? '',
            band: selected('band'),
            receiver: selected('receiver'),
            // The Source picker sits in the tab's head, above every card (backlog 77).
            source: (() => {
              const sel = panel?.querySelector<HTMLSelectElement>('select[data-control="source"]');
              return sel ? (sel.options[sel.selectedIndex]?.textContent ?? '') : '';
            })(),
            paths: [...e.querySelectorAll('[data-json]')].map((n) => n.getAttribute('data-json') ?? ''),
          };
        })(),
      })),
      params: [...document.querySelectorAll('[data-param]')].map((e) => e.getAttribute('data-param') ?? ''),
      stray: clone?.textContent ?? '',
      options: all('select[data-control] option').map((o) => ({
        control: o.closest('select')?.getAttribute('data-control') ?? '',
        value: (o as HTMLOptionElement).value,
        text: o.textContent ?? '',
      })),
      text: panel?.innerText ?? '',
    };
  }, PANEL);

/** Every selection the tab offers: each band, then each receiver, then each DIN group, then each
 * source with the first band and receiver (a run with several sources, backlog 77). */
export async function everySelection(visit: (what: string) => Promise<void>): Promise<void> {
  const s = await scan();
  const values = (c: string) => s.options.filter((o) => o.control === c).map((o) => o.value);
  for (const src of values('source')) {
    await pick('source', src);
    await visit(`source ${src || 'summed'}`);
  }
  if (values('source').length) await pick('source', values('source')[0]);
  for (const b of values('band')) {
    await pick('band', b);
    await visit(`band ${b}`);
  }
  for (const r of values('receiver')) {
    await pick('receiver', r);
    await visit(`receiver ${r}`);
  }
  for (const g of values('din-group')) {
    await pick('din-group', g);
    await visit(`DIN ${g}`);
  }
  await pick('din-group', 'A3');
  await pick('receiver', values('receiver')[0]);
  await pick('band', values('band')[0]);
}

/** What gate (b) saw over every selection: the parameters with elements anywhere on the page,
 * every cell of the tab, the parameters drawn, and the EDT marks' texts. */
export interface BedSweep {
  shown: Set<string>;
  cells: (ScanCell & { what: string })[];
  drawn: Set<string>;
  edtMarks: string[];
  /** Every mark seen beside the receivers table, by the parameter it is shown with. */
  marks: Record<string, string[]>;
}

/** Gate (b) over every selection: each of `hidden` has 0 `[data-param]` elements on the page, its
 * name in neither the tab's nor the Results panel's text, and is not drawn. Returns what was
 * shown, for the caller's check that every PASS parameter is. */
export async function bedSweep(hidden: string[], bedOf: BedSummary): Promise<BedSweep> {
  const out: BedSweep = { shown: new Set(), cells: [], drawn: new Set(), edtMarks: [], marks: {} };
  await everySelection(async (what) => {
    const s = await scan();
    for (const n of hidden) {
      assert.equal(s.params.filter((p) => p === n).length, 0, `${what}: ${n} (${bedOf.parameters[n]?.status}) has elements`);
      assert.ok(!labelPattern(n).test(s.text), `${what}: "${PARAM_LABELS[n]}" is in the Acoustics tab's text`);
    }
    const props = await browser.execute(() => (document.querySelector('[data-props-step="results"]') as HTMLElement | null)?.innerText ?? '');
    for (const n of hidden) assert.ok(!labelPattern(n).test(props), `${what}: "${PARAM_LABELS[n]}" is in the Results panel`);
    const v = await hook<View>('acousticsView');
    for (const n of hidden) assert.ok(!v.series.some((x) => x.param === n), `${what}: ${n} is drawn`);
    s.params.forEach((p) => out.shown.add(p));
    s.cells.forEach((c) => out.cells.push({ ...c, what }));
    v.series.forEach((x) => out.drawn.add(x.param));
    const marks = await browser.execute(
      (sel: string) =>
        [...document.querySelectorAll(`${sel} .ac-marks .ac-mark`)].map((e) => ({ param: e.closest('[data-param]')?.getAttribute('data-param') ?? '', text: e.textContent ?? '' })),
      PANEL,
    );
    for (const m of marks) {
      const list = (out.marks[m.param] ??= []);
      if (!list.includes(m.text)) list.push(m.text);
      if (m.param === 'edt_s' && !out.edtMarks.includes(m.text)) out.edtMarks.push(m.text);
    }
  });
  return out;
}

/** Gate (b)'s other half: every parameter in `passed` has elements and cells, and each of its
 * cells carries its range and status (`ok`/`wide` with `.lo` and `.hi`), STI its value with
 * MQ3's note, or a refusal with its code (row 37 (3)); each of `mustRange` shows a value with its
 * range in at least one cell. EDT, where shown, carries both of row 37's marks; T30 row 46's. Returns a line per
 * parameter for the receipt. */
export function assertPassRendered(passed: string[], sweep: BedSweep, mustRange: string[] = ['edt_s', 't20_s', 't30_s']): string[] {
  const lines: string[] = [];
  for (const n of passed) {
    assert.ok(sweep.shown.has(n), `${n} is PASS and has no element`);
    const cells = sweep.cells.filter((c) => c.param === n);
    assert.ok(cells.length > 0, `${n} is PASS and has no cell`);
    let ranged = 0;
    let refused = 0;
    for (const c of cells) {
      const at = `${c.what}: ${n} at ${c.receiver} ${c.band}`;
      if (c.status === 'refused') {
        assert.ok(c.refusal && /^[a-z_]+$/.test(c.refusal), `${at} refused without its code`);
        refused++;
      } else if (n === 'sti') {
        assert.equal(c.status, 'value', at);
        assert.equal(c.note, 'noise range not computed', `${at}: STI without MQ3's note`);
      } else {
        assert.ok(['ok', 'wide'].includes(c.status), `${at}: status ${c.status}`);
        assert.ok(c.nums.some((p) => p.endsWith('.lo')) && c.nums.some((p) => p.endsWith('.hi')), `${at} has no range`);
        ranged++;
      }
    }
    // A run may refuse a parameter in every cell (the gate's box refuses dB(A): its 250 Hz SPL is
    // truncated); that is its status, shown with its code, and m12-a holds every code to the JSON.
    // The reverberation times this gate names (T30 by decision 46, EDT with its marks) must show
    // values with their ranges on the box.
    if (mustRange.includes(n)) assert.ok(ranged > 0, `${n} is PASS but no cell shows a value with its range (${refused} refused)`);
    lines.push(`${n} ${cells.length} cells (${ranged} with range, ${refused} refused)`);
  }
  if (passed.includes('edt_s')) {
    for (const m of EDT_MARKS) assert.ok(sweep.edtMarks.includes(m), `EDT is shown without row 37's mark "${m}" (seen ${JSON.stringify(sweep.edtMarks)})`);
  }
  // T30, shown by decision 46, carries its mark wherever it appears (the assay's LOW finding).
  if (passed.includes('t30_s')) {
    assert.deepEqual(sweep.marks.t30_s ?? [], [T30_MARK], `T30 is shown without decision 46's mark (seen ${JSON.stringify(sweep.marks)})`);
  }
  return lines;
}
