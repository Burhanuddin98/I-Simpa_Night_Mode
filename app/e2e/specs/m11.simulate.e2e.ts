// The simulate package's spec (docs/investigations/2026-09-29-m11/PLAN.md 9.1): the Simulate
// step, the Run button and the Results step, driven by WebDriver clicks on the controls a user
// would press. Its extra checks, each with the control that lets it fail:
//   m11-sim-preflight  "Before running" reads FAIL (with the codes) on the placeholder hall and
//                      OK on the box; every Run blocker sits on a FAIL row; the settings shown
//                      are the project file's own
//   m11-sim-last-run   the panel's big Run button runs the box; the idle block then reads
//                      "Run <n> · Baseline", OK, the loss and limit the backend formatted (equal to
//                      a BigInt recomputation from run.json), and the solver's WARN count; first,
//                      the planted-loss run's idle block reads its known 0.82 % (M11 review F1)
//   m11-sim-numbers    the Simulate and Results steps show no number next to a unit outside
//                      [data-input] (in its own region) but in a diagnostic span that proves
//                      itself, on the box and on the planted-loss run; the Results panel has no
//                      [data-result] and no digit outside [data-run-label] and the Results
//                      regions (M12: `[data-results-region]`, dom.ts RESULTS_REGIONS); on the
//                      Simulate step the Acoustics tab and every Results region hold no digit, so
//                      a number off the Results step still fails
//   m11-sim-tcr        the radio switches the label to "Run TCR"; a TCR run of the box ends OK
//                      with no loss line
//   m11-sim-link       the "Run <n>" link selects that run on the Results step
//   m11-sim-running    during the hall's solve: "Solving · <p> %" (p one of the run's '#' lines
//                      at the digits shown, as m11-h proves it, and rising from sample to sample:
//                      M11 review F3), Run disabled with RUN_ACTIVE and
//                      "Running …", the elapsed clock; a click on Cancel ends it Cancelled with no
//                      spps.exe left running from the gate's private copy
//
// Projects are copied into their own folders under M11_P (on C:), so this spec's runs never mix
// with the gate's. The app window stays visible and unfocused (m11.conf.ts); nothing here moves,
// resizes, hides or activates it.
import { strict as assert } from 'node:assert';
import { copyFileSync, mkdirSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { GRAMMAR } from '../lib/acoustic.ts';
import { ACOUSTIC_NUMBER, clickSelector, EXEMPT_REGIONS, PARAMETER_NUMBER } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { PLANTED } from '../lib/plant-loss.ts';
import { processesFrom } from '../lib/procs.ts';
import { decimalAbove, limitPct, type Manifest, progressValues, readManifest, roundedTo, worstLoss } from '../lib/runs.ts';
import { env } from '../lib/types.ts';

interface Row {
  run: string;
  number: number;
  status: string;
  solver?: string | null;
  variant?: string | null;
  loss?: { worst_pct: string; limit_pct: string } | null;
  lines?: { progress: number; info: number; ok: number; warn: number; fail: number } | null;
}
interface PreflightDom {
  key: string;
  state: string;
  label: string;
  codes: string[];
}

const PROGRESS = GRAMMAR.progress_pct;

/** Copies a gate project into this spec's own folder, so its runs root is this spec's. */
function ownCopy(from: string, folder: string): string {
  const dir = path.join(env('M11_P'), 'simulate', folder);
  mkdirSync(dir, { recursive: true });
  const to = path.join(dir, path.basename(from));
  copyFileSync(from, to);
  return to;
}

const runDir = (project: string, run: string) => path.join(path.dirname(project), 'runs', run);

/** The planted-loss run's project (m11.ps1, M11 review F1), opened where the run was made: its
 * run.json names that path, so a copy would list none of it. Only read here, never run. */
const lossProject = () => path.join(env('M11_P'), 'loss', 'box_run.simpa');

/** `<run>/run.json`, which must exist. */
function runJson(project: string, run: string): Manifest {
  const m = readManifest(runDir(project, run));
  assert.ok(m, `no run.json in ${runDir(project, run)}`);
  return m;
}

/** The worst band's loss and the limit as displayed, recomputed in Node with BigInt (runs.ts). */
function lossTexts(m: Manifest): { pct: string; limit: string } {
  const w = worstLoss(m);
  assert.ok(w, 'run.json has no particle statistics');
  return { pct: `${w.pct} %`, limit: `${limitPct(m.loss_limit)} %` };
}

/** The text of the first element matching `sel`, exactly (textContent), or null. */
const text = (sel: string) => browser.execute((s: string) => document.querySelector(s)?.textContent ?? null, sel);

const preflight = (): Promise<PreflightDom[]> =>
  browser.execute(() =>
    [...document.querySelectorAll('[data-props-step="simulate"] [data-preflight]')].map((el) => ({
      key: el.getAttribute('data-preflight') ?? '',
      state: el.getAttribute('data-state') ?? '',
      label: el.querySelector('.sim-state')?.textContent ?? '',
      codes: [...el.querySelectorAll('[data-code]')].map((c) => c.getAttribute('data-code') ?? ''),
    })),
  );

const blockersOf = async (sel: string): Promise<string[]> =>
  ((await $(sel).getAttribute('data-blockers')) ?? '').split(' ').filter(Boolean);

/**
 * Clicks `selector` (a Run button) and waits until a run that was not listed before has ended
 * and the app is idle again. Robust to a run that ends before the first poll: it compares the
 * Runs rows, not the stream.
 */
async function runByClick(selector: string, timeout = 180_000): Promise<Row> {
  const before = new Set((await hook<Row[]>('runsRows')).map((r) => r.run));
  await clickSelector(selector);
  let fresh: Row | undefined;
  await browser.waitUntil(
    async () => {
      if ((await hook<unknown>('runState')) !== null) return false;
      fresh = (await hook<Row[]>('runsRows')).find((r) => !before.has(r.run) && r.status !== 'RUNNING');
      return fresh !== undefined;
    },
    { timeout, interval: 250, timeoutMsg: `no new run ended within ${timeout} ms after clicking ${selector}` },
  );
  await m10.idle();
  return fresh as Row;
}

/**
 * This package's regions (the props panel, the Run button, the Simulate sub): their text with
 * diagnostics hidden, and inputs and geometry facts hidden only inside their own regions
 * (dom.ts EXEMPT_REGIONS: here, the Simulate settings), their text with nothing hidden, their
 * diagnostic spans, and every `[data-input]` or `[data-geometry]` outside its regions (`strays`,
 * which must be none: M11 review F2).
 */
const scanProps = () =>
  browser.execute((exempt: Record<string, Record<string, string>>) => {
    const props = document.querySelector<HTMLElement>('[data-props-step]');
    if (!props) return null;
    const regions = [props, document.querySelector<HTMLElement>('[data-part="run"]'), document.querySelector<HTMLElement>('[data-step="simulate"] [data-part="sub"]')].filter(
      (e): e is HTMLElement => e !== null,
    );
    const diags = regions.flatMap((r) =>
      [...r.querySelectorAll<HTMLElement>('[data-diagnostic]')].map((e) => ({
        field: e.getAttribute('data-diagnostic') ?? '',
        run: e.getAttribute('data-run') ?? '',
        text: e.textContent ?? '',
        leaf: e.children.length === 0,
      })),
    );
    const whole = regions.map((r) => r.innerText).join('\n');
    // Every tooltip in the regions, the regions' own included (M11 review 2, app 4).
    const titles = regions
      .flatMap((r) => [r, ...r.querySelectorAll<HTMLElement>('[title]')])
      .flatMap((e) => (e.hasAttribute('title') ? [e.getAttribute('title') ?? ''] : []));
    const hide = regions.flatMap((r) => [...r.querySelectorAll<HTMLElement>('[data-diagnostic]')]);
    const strays: string[] = [];
    for (const attr of Object.keys(exempt)) {
      for (const r of regions) {
        for (const e of r.querySelectorAll<HTMLElement>(`[${attr}]`)) {
          if (Object.values(exempt[attr]).some((sel) => e.closest(sel) !== null)) hide.push(e);
          else strays.push(`[${attr}] "${(e.textContent ?? '').slice(0, 80)}"`);
        }
      }
    }
    const before = hide.map((e) => e.style.display);
    hide.forEach((e) => (e.style.display = 'none'));
    try {
      return { step: props.getAttribute('data-props-step'), text: regions.map((r) => r.innerText).join('\n'), whole, diags, strays, titles };
    } finally {
      hide.forEach((e, i) => (e.style.display = before[i]));
    }
  }, EXEMPT_REGIONS);

/** The running block's head and its progress span, read in one go (both move during a solve). */
const runningHead = () =>
  browser.execute(() => {
    const head = document.querySelector('[data-part="running-head"]');
    const span = head?.querySelector('[data-diagnostic="progress_pct"]');
    return { head: head?.textContent ?? null, p: span?.textContent ?? null, run: span?.getAttribute('data-run') ?? null };
  });

/**
 * The Results step as read by gate (e): its state, run, label, the [data-result] count outside
 * the Results regions, and its text and tooltips with the run label and the Results regions
 * removed. M12 narrows it (P1 BUILD.md, "Left open"): a number may sit in a `[data-results-region]`
 * of the Results panel while the Results step is current; anywhere else in the panel it still
 * fails.
 */
const resultsDom = () =>
  browser.execute(() => {
    const aside = document.querySelector('[data-props-step="results"]');
    if (!aside) return null;
    const clone = aside.cloneNode(true) as HTMLElement;
    clone.querySelectorAll('[data-run-label], [data-results-region]').forEach((e) => e.remove());
    const state = aside.querySelector('[data-results-state]');
    return {
      state: state?.getAttribute('data-results-state') ?? null,
      run: state?.getAttribute('data-run') ?? null,
      label: aside.querySelector('[data-run-label]')?.textContent ?? null,
      results: clone.querySelectorAll('[data-result]').length,
      text: clone.textContent ?? '',
      // Tooltips are read as surely as text (M11 review 2, app 4).
      titles: [...clone.querySelectorAll('[title]')].map((e) => e.getAttribute('title') ?? ''),
    };
  });

/** Off the Results step: the Acoustics tab's text, and each Results region's. */
const offResults = () =>
  browser.execute(() => ({
    acoustics: (document.querySelector('[data-dock-panel="acoustics"]') as HTMLElement | null)?.innerText ?? null,
    regions: [...document.querySelectorAll<HTMLElement>('[data-results-region]')].map((e) => e.innerText),
  }));

/**
 * `shown` (before its ` %`) is one of the run's `#` lines at the digits shown, rounded half up:
 * the rule m11-h proves a progress span by (lib/acoustic.ts `progressProven`).
 */
function provenProgress(shown: string, lines: string[]): boolean {
  const d = (shown.split('.')[1] ?? '').length;
  const v = roundedTo(shown, d);
  return v !== null && lines.some((l) => roundedTo(l, d) === v);
}

describe('M11 simulate', () => {
  let box = '';
  let boxRun: Row | null = null;

  before(async () => {
    await waitForHooks(['idle', 'openProject', 'importModel', 'runsRows', 'runState', 'waitRun', 'setStep', 'setSolver', 'projectJson']);
    box = ownCopy(path.join(env('M11_P'), 'box', 'box_run.simpa'), 'box');
  });

  it('m11-sim-preflight: Before running reads FAIL on the placeholder hall and OK on the box', async () => {
    await m10.importModel(path.join(env('M11_REPO'), 'testdata', 'elmia_corrected.ply'), 'm', 'z');
    await m10.setStep('simulate');
    await browser.waitUntil(async () => (await preflight()).length > 0, { timeout: 30_000, timeoutMsg: 'no Before running rows' });
    const hall = await preflight();
    const blockers = await blockersOf('[data-part="run"]');
    console.log(`m11-sim-preflight receipt: hall rows ${JSON.stringify(hall)}; data-blockers ${blockers.join(' ')}`);
    const materials = hall.find((r) => r.key === 'materials');
    // Re-pinned 2026-10-07 (C4): the row's words are Ready / Blocked since 0aaae74 (10-06, "Step
    // labels and badges in words", Burhan's 04:55 order); data-state still carries OK / FAIL.
    assert.deepEqual([materials?.state, materials?.label], ['FAIL', 'Blocked'], 'the materials row fails, as text');
    assert.ok(materials?.codes.includes('MATERIALS_UNASSIGNED'), `materials codes: ${materials?.codes}`);
    assert.equal(hall.find((r) => r.key === 'model')?.state, 'OK', 'the corrected hall passes its model check');
    // Every blocker of the Run button is named on a FAIL row: the list never reads OK while Run is blocked.
    const onFail = new Set(hall.filter((r) => r.state === 'FAIL').flatMap((r) => r.codes));
    for (const b of blockers) if (b !== 'RUN_ACTIVE') assert.ok(onFail.has(b), `${b} is on no FAIL row`);
    for (const r of hall) assert.equal(r.label, r.state === 'OK' ? 'Ready' : 'Blocked', `${r.key}: label '${r.label}' for state ${r.state}`);

    // Control: the box may run, so every row reads OK and Run carries no blocker.
    await m10.openProject(box);
    await browser.waitUntil(async () => (await preflight()).every((r) => r.state === 'OK'), {
      timeout: 30_000,
      timeoutMsg: `the box's rows are not all OK: ${JSON.stringify(await preflight())}`,
    });
    const rows = await preflight();
    assert.deepEqual(
      rows.map((r) => r.key),
      // Re-pinned 2026-10-07 (C4): 0959499 (10-06, the result cube sized before every SPPS run,
      // Burhan 06:39) added the memory row, between air and solvers.
      ['materials', 'model', 'source', 'receivers', 'air', 'memory', 'solvers'],
    );
    assert.ok(rows.every((r) => r.label === 'Ready' && r.codes.length === 0));
    assert.deepEqual(await blockersOf('[data-part="run"]'), []);
    assert.deepEqual(await blockersOf('[data-part="run-panel"]'), []);

    // The settings shown are the project file's own: since PQ3 each is an input field holding
    // the stored value (the time step in ms), and an input's value is not page text.
    const file = JSON.parse(await m10.projectJson());
    const spps = file.solvers.spps;
    const setting = (k: string) =>
      browser.execute((sel: string) => document.querySelector<HTMLInputElement>(sel)?.value ?? null, `[data-setting="${k}"] input[data-field]`);
    await browser.waitUntil(async () => (await setting('particles')) !== null, { timeout: 30_000, timeoutMsg: 'the settings did not load' });
    assert.equal(await setting('particles'), String(spps.particles_per_source));
    assert.equal(await setting('duration'), String(spps.duration_s));
    assert.equal(await setting('time_step'), String(Number((spps.time_step_s * 1000).toPrecision(12))));
    console.log(`m11-sim-preflight receipt: box settings particles ${await setting('particles')}, duration ${await setting('duration')} s, time step ${await setting('time_step')} ms, bands ${await text('[data-setting="bands"] [data-part="bands-summary"]')}, air ${await setting('air')} °C`);
  });

  it("m11-sim-last-run: the panel's Run runs the box; the idle block reads the run's own record", async () => {
    // The control that lets the loss checks below fail (M11 review F1): the box loses 0
    // particles, so an idle block that printed a constant 0.00 passed them. The planted-loss
    // run (m11.ps1) carries a known loss, worst 1,234 of 150,000 at 500 Hz.
    const loss = lossProject();
    const plantedRuns = readdirSync(path.join(path.dirname(loss), 'runs'));
    assert.equal(plantedRuns.length, 1, `m11.ps1 makes the one planted-loss run: ${plantedRuns.join(', ')}`);
    const [planted] = plantedRuns;
    const pw = lossTexts(runJson(loss, planted));
    assert.equal(pw.pct, `${PLANTED.worst_pct} %`, "run.json's worst band is the plant");
    await m10.openProject(loss);
    try {
      await m10.setStep('simulate');
      await browser.waitUntil(async () => (await $('[data-part="last-run"]').getAttribute('data-run')) === planted, {
        timeout: 30_000,
        timeoutMsg: 'the idle block does not show the planted-loss run',
      });
      const plantedShown = {
        loss: await text('[data-part="last-loss"] [data-diagnostic="loss_pct"]'),
        limit: await text('[data-part="last-loss"] [data-diagnostic="loss_limit_pct"]'),
      };
      console.log(`m11-sim-last-run receipt: planted-loss run ${planted}: the idle block reads ${JSON.stringify(plantedShown)}; run.json ${JSON.stringify(pw)}`);
      assert.equal(plantedShown.loss, pw.pct);
      assert.equal(plantedShown.loss, '0.82 %');
      assert.equal(plantedShown.limit, pw.limit);
    } finally {
      // The box again, whatever happened: the tests after this one run the box, and nothing may
      // ever run in the planted-loss project.
      await m10.openProject(box);
    }
    await m10.setStep('simulate');
    await browser.waitUntil(async () => (await blockersOf('[data-part="run-panel"]')).length === 0 && (await $('[data-part="run-panel"]').isEnabled()), {
      timeout: 30_000,
      timeoutMsg: 'Run is not free on the box',
    });
    boxRun = await runByClick('[data-part="run-panel"]');
    const m = runJson(box, boxRun.run);
    console.log(`m11-sim-last-run receipt: ${boxRun.run} #${boxRun.number} ${boxRun.status}, loss ${JSON.stringify(boxRun.loss)}, run.json lines.warn ${m.lines.warn}`);
    assert.equal(boxRun.status, 'OK');
    assert.equal(m.verdict.status, 'OK');
    await browser.waitUntil(async () => (await $('[data-part="last-run"]').getAttribute('data-run')) === boxRun?.run, {
      timeout: 30_000,
      timeoutMsg: 'the idle block does not show the new run',
    });
    assert.equal(await text('[data-part="run-link"]'), `Run ${boxRun.number} · Baseline`);
    assert.equal(await text('[data-part="last-status"]'), 'OK');
    assert.equal(await $('[data-part="last-status"]').getAttribute('data-status'), 'OK');
    // The loss and limit: the backend's strings, equal to run.json recomputed with BigInt.
    const want = lossTexts(m);
    assert.equal(`${boxRun.loss?.worst_pct} %`, want.pct);
    assert.equal(`${boxRun.loss?.limit_pct} %`, want.limit);
    assert.equal(await text('[data-part="last-loss"] [data-diagnostic="loss_pct"]'), want.pct);
    assert.equal(await text('[data-part="last-loss"] [data-diagnostic="loss_limit_pct"]'), want.limit);
    assert.equal(await $('[data-part="last-loss"] [data-diagnostic="loss_pct"]').getAttribute('data-run'), boxRun.run);
    assert.equal(await text('[data-part="last-warnings"] .v'), String(m.lines.warn));
    // The step bar's Simulate sub names the run and its status.
    assert.equal(await text('[data-step="simulate"] [data-part="sub"]'), `run ${boxRun.number} OK`);
  });

  it('m11-sim-numbers: no number next to a unit but a diagnostic that proves itself; the Results step shows none', async () => {
    assert.ok(boxRun, 'needs m11-sim-last-run');
    const want = lossTexts(runJson(box, boxRun.run));
    const expected: Record<string, string> = { loss_pct: want.pct, loss_limit_pct: want.limit };
    for (const step of ['simulate', 'results']) {
      await clickSelector(`[data-step="${step}"]`);
      await m10.idle();
      if (step === 'results') {
        await browser.waitUntil(async () => !['checking', null].includes((await resultsDom())?.state ?? null), {
          timeout: 30_000,
          timeoutMsg: 'the Results step did not settle',
        });
      }
      const s = await scanProps();
      assert.ok(s, 'the properties panel is shown');
      console.log(`m11-sim-numbers receipt: step ${s.step}, ${s.diags.length} diagnostic span(s) ${JSON.stringify(s.diags)}`);
      assert.deepEqual(s.strays, [], `step ${step}: [data-input] or [data-geometry] outside its regions`);
      const hit = s.text.match(ACOUSTIC_NUMBER);
      assert.equal(hit, null, `step ${step}: "${hit?.[0]}" outside [data-input] and the diagnostics`);
      const param = s.whole.match(PARAMETER_NUMBER);
      assert.equal(param, null, `step ${step}: "${param?.[0]}" names a parameter with a number`);
      const tip = s.titles.find((t) => ACOUSTIC_NUMBER.test(t) || PARAMETER_NUMBER.test(t));
      assert.equal(tip, undefined, `step ${step}: a tooltip holds a number next to a unit, or a parameter's number: "${tip}"`);
      for (const d of s.diags) {
        assert.ok(d.leaf, `${d.field} is not a leaf span`);
        assert.ok((GRAMMAR as Record<string, RegExp>)[d.field]?.test(d.text), `${d.field} '${d.text}' fails its grammar`);
        assert.equal(d.run, boxRun.run, `${d.field} names run ${d.run}`);
        assert.equal(d.text, expected[d.field], `${d.field} is not the manifest's value`);
      }
      if (step === 'simulate') {
        assert.deepEqual(s.diags.map((d) => d.field).sort(), ['loss_limit_pct', 'loss_pct']);
        // M12: off the Results step the Acoustics tab and every Results region hold no digit.
        await hook('dockTab', 'acoustics');
        const off = await offResults();
        console.log(`m11-sim-numbers receipt: Simulate step, Acoustics tab ${JSON.stringify(off.acoustics)}, ${off.regions.length} Results region(s)`);
        assert.ok(!/\d/.test(off.acoustics ?? ''), `a digit in the Acoustics tab off the Results step: ${off.acoustics}`);
        for (const r of off.regions) assert.ok(!/\d/.test(r), `a digit in a Results region off the Results step: ${r}`);
        await hook('dockTab', 'console');
      }
      if (step === 'results') {
        const r = await resultsDom();
        console.log(`m11-sim-numbers receipt: results ${JSON.stringify(r)}`);
        assert.ok(r);
        assert.equal(r.results, 0, 'no [data-result] outside the Results regions');
        assert.equal(r.run, boxRun.run, 'the newest run is shown');
        assert.equal(r.state, 'verified', 'the OK box run verifies');
        assert.ok(!/\d/.test(r.text), `a digit outside [data-run-label]: ${r.text}`);
        const digitTip = r.titles.find((t) => /\d/.test(t));
        assert.equal(digitTip, undefined, `a tooltip with a digit on the Results step: "${digitTip}"`);
        assert.equal(s.diags.length, 0, 'the Results step holds no diagnostic');
      }
    }

    // The same on the planted-loss run (M11 review F1), whose loss is not 0.00: each diagnostic
    // must be its own run.json's value, not the box's and not a constant.
    const loss = lossProject();
    const [planted] = readdirSync(path.join(path.dirname(loss), 'runs'));
    const pw = lossTexts(runJson(loss, planted));
    await m10.openProject(loss);
    try {
      await clickSelector('[data-step="simulate"]');
      await m10.idle();
      await browser.waitUntil(async () => (await $('[data-part="last-run"]').getAttribute('data-run')) === planted, {
        timeout: 30_000,
        timeoutMsg: 'the idle block does not show the planted-loss run',
      });
      const s = await scanProps();
      assert.ok(s, 'the properties panel is shown');
      console.log(`m11-sim-numbers receipt: planted-loss run, step ${s.step}, ${JSON.stringify(s.diags)}`);
      assert.deepEqual(s.strays, []);
      assert.equal(s.text.match(ACOUSTIC_NUMBER), null);
      assert.deepEqual(
        s.diags.map((d) => [d.field, d.run, d.text]).sort(),
        [
          ['loss_limit_pct', planted, pw.limit],
          ['loss_pct', planted, pw.pct],
        ],
      );
      assert.notEqual(pw.pct, '0.00 %');
    } finally {
      await m10.openProject(box);
    }
  });

  it('m11-sim-tcr: the radio switches the label to Run TCR, and a TCR run of the box ends OK with no loss line', async () => {
    await m10.setStep('simulate');
    assert.equal(await text('[data-part="run-label"]'), 'Run SPPS');
    await clickSelector('[data-solver="tcr"]');
    await browser.waitUntil(async () => (await text('[data-part="run-label"]')) === 'Run TCR', {
      timeout: 10_000,
      timeoutMsg: 'the Run label did not become Run TCR',
    });
    assert.equal(await $('[data-solver="tcr"]').getAttribute('aria-checked'), 'true');
    assert.equal(await $('[data-solver="spps"]').getAttribute('aria-checked'), 'false');
    assert.equal(await text('[data-part="run-panel"]'), 'Run TCR');
    const keys = await browser.execute(() => [...document.querySelectorAll('[data-setting]')].map((e) => e.getAttribute('data-setting')));
    assert.deepEqual(keys, ['method', 'bands', 'air']);

    const tcr = await runByClick('[data-part="run"]');
    console.log(`m11-sim-tcr receipt: ${tcr.run} #${tcr.number} ${tcr.solver} ${tcr.status}, loss ${JSON.stringify(tcr.loss)}`);
    assert.equal(tcr.solver, 'tcr');
    assert.equal(tcr.status, 'OK');
    assert.equal(runJson(box, tcr.run).verdict.status, 'OK');
    assert.equal(tcr.loss ?? null, null, 'TCR tracks no particles');
    await browser.waitUntil(async () => (await $('[data-part="last-run"]').getAttribute('data-run')) === tcr.run, {
      timeout: 30_000,
      timeoutMsg: 'the idle block does not show the TCR run',
    });
    assert.equal(await text('[data-part="last-status"]'), 'OK');
    assert.equal(await $('[data-part="last-loss"]').isExisting(), false, 'no loss line for TCR');
    // Control: the SPPS run of the same box did show one (m11-sim-last-run). Back to SPPS.
    await clickSelector('[data-solver="spps"]');
    await browser.waitUntil(async () => (await text('[data-part="run-label"]')) === 'Run SPPS', {
      timeout: 10_000,
      timeoutMsg: 'the Run label did not return to Run SPPS',
    });
  });

  it('m11-sim-link: the "Run <n>" link selects that run on the Results step', async () => {
    await m10.setStep('simulate');
    const rows = await hook<Row[]>('runsRows');
    const last = rows.filter((r) => r.status !== 'RUNNING').sort((a, b) => b.number - a.number)[0];
    assert.ok(last && boxRun && last.run !== boxRun.run, 'needs two runs of the box');
    // Control: another run is selected first, and the Results step shows it.
    await hook('selectRun', boxRun.run);
    await m10.setStep('results');
    await browser.waitUntil(async () => (await resultsDom())?.run === boxRun?.run, { timeout: 10_000, timeoutMsg: 'the control run is not shown' });
    await m10.setStep('simulate');
    await clickSelector(`[data-part="run-link"][data-run="${last.run}"]`);
    await browser.waitUntil(async () => (await $('[data-props-step]').getAttribute('data-props-step')) === 'results', {
      timeout: 10_000,
      timeoutMsg: 'the link did not open the Results step',
    });
    await browser.waitUntil(async () => !['checking', null].includes((await resultsDom())?.state ?? null), {
      timeout: 30_000,
      timeoutMsg: 'the Results step did not settle',
    });
    const r = await resultsDom();
    console.log(`m11-sim-link receipt: ${JSON.stringify(r)}`);
    assert.ok(r);
    assert.equal(r.run, last.run);
    assert.equal(r.label, `Run ${last.number} · Baseline`);
    assert.ok(['verified', 'refused'].includes(r.state ?? ''), `state ${r.state}`);
    assert.equal(r.results, 0);
    assert.ok(!/\d/.test(r.text), `a digit outside [data-run-label]: ${r.text}`);
  });

  it('m11-sim-running: the hall shows Solving · <p> %, Run is RUN_ACTIVE, and Cancel ends it Cancelled', async () => {
    const hall = ownCopy(path.join(env('M11_P'), 'hall', 'hall_run.simpa'), 'hall');
    await m10.openProject(hall);
    await m10.setStep('simulate');
    assert.deepEqual(await blockersOf('[data-part="run"]'), [], 'the hall may run');
    await clickSelector('[data-part="run"]');
    const name = await hook<string>('waitRun', null, 'progress', 300_000);

    // During the solve: the running block, the Run button, the step bar's sub. Sampled a few
    // times, each percentage later checked against the solver's own '#' lines.
    const samples: string[] = [];
    for (let i = 0; i < 5; i++) {
      await browser.waitUntil(async () => (await runningHead()).p !== null, {
        timeout: 30_000,
        timeoutMsg: `no 'Solving · <p> %' head: '${(await runningHead()).head}'`,
      });
      const h = await runningHead();
      assert.ok(h.p !== null && h.head !== null);
      assert.equal(h.head, `Solving · ${h.p}`);
      assert.match(h.p, PROGRESS);
      assert.equal(h.run, name);
      samples.push(h.p.replace(/ %$/, ''));
      await browser.pause(300);
    }
    assert.equal(await $('[data-part="run"]').isEnabled(), false, 'Run is disabled during a run');
    assert.ok((await blockersOf('[data-part="run"]')).includes('RUN_ACTIVE'));
    assert.match((await text('[data-part="run-label"]')) ?? '', /^Running( \d{1,3}(\.\d{1,2})? %|…)$/);
    assert.match((await text('[data-part="elapsed-clock"]')) ?? '', /^\d+:\d{2}(:\d{2})?$/);
    assert.equal(await $('[data-part="run-panel"]').isExisting(), false, 'the idle block is hidden while running');
    assert.match((await text('[data-step="simulate"] [data-part="sub"]')) ?? '', /^(\d{1,3}(\.\d{1,2})? %|running)$/);
    const spps = path.join(env('M11_SOLVERS'), 'spps.exe');
    const alive = processesFrom(spps);
    console.log(`m11-sim-running receipt: ${name}, progress samples ${samples.join(', ')}; spps.exe from the private copy: ${alive.join(', ')}`);
    assert.ok(alive.length > 0, 'the private spps.exe runs during the solve');

    // Cancel, as a user presses it.
    const t0 = Date.now();
    await clickSelector('[data-part="cancel-run"]');
    await hook<string>('waitRun', name, 'ended', 60_000);
    console.log(`m11-sim-running receipt: ended ${Date.now() - t0} ms after the Cancel click`);
    assert.deepEqual(processesFrom(spps), [], 'no spps.exe runs from the private copy after Cancel');
    const rows = await hook<Row[]>('runsRows');
    const row = rows.find((r) => r.run === name);
    assert.equal(row?.status, 'CANCELLED');
    assert.equal(runJson(hall, name).verdict.status, 'CANCELLED');
    await browser.waitUntil(async () => (await text('[data-part="last-status"]')) === 'Cancelled', {
      timeout: 30_000,
      timeoutMsg: 'the idle block does not read Cancelled',
    });
    assert.equal(await text('[data-step="simulate"] [data-part="sub"]'), `run ${row?.number} Cancelled`);
    assert.equal(await text('[data-part="run-label"]'), 'Run SPPS');
    assert.deepEqual(await blockersOf('[data-part="run"]'), [], 'Run is free again');

    // Every percentage shown was the solver's own, at the digits shown (m11-h's rule).
    const lines = progressValues(runDir(hall, name));
    console.log(`m11-sim-running receipt: ${lines.length} '#' lines in ${runDir(hall, name)}`);
    assert.ok(lines.length > 0, `no '#' line in ${runDir(hall, name)}'s solver.stdout.txt`);
    for (const s of samples) assert.ok(provenProgress(s, lines), `${s} % is no '#' line of ${name} at the digits shown`);
    // And the readout moved with the solve (M11 review F3): each sample above the one before. A
    // head frozen at its first value passed the check above, since that value is one of the
    // run's '#' lines (the review's mutation M23: 0.01 five times). Judged after Cancel, so a
    // failure here leaves no run going.
    for (let i = 1; i < samples.length; i++) {
      assert.equal(decimalAbove(samples[i], samples[i - 1]), true, `the progress readout did not move: ${samples.join(', ')}`);
    }
  });
});
