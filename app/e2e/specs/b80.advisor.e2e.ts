// Backlog 80, the run-quality advisor (M12b; docs/investigations/2026-10-04-advisor/PLAN.md, T10;
// decisions 49, 54, 56, 57). Run by `tools/gates/m12.ps1` through `m11.ps1 -Only e2e -Spec
// b80.advisor` on m11.ps1's harness. Each id with its control:
//   b80-pre    tutorial 2 as upstream ships it (tutorial_2.proj imported by `simpa import-proj`:
//              `-q2` without `-Y`, 0.31 m receivers) shows, on the Simulate step and before any
//              run, "this meshing splits the walls; it can lose particles" (`mesh_splits_walls`,
//              Apply: -Y off → on) and "receivers small for this room" (`receivers_small`, Apply:
//              Receiver radius 0.31 m → 0.6 m); the advice never blocks Run. Control: the Elmia
//              fixture (-Y, 0.6 m, a million particles) shows neither
//   b80-h      m10-h and m11-h's rules 1 and 2 on the Simulate step with those rows shown: no
//              number next to s, ms, dB or % and no parameter name followed by a number in the
//              page's text, inputs and geometry facts aside (dom.ts). Control: the same reading
//              catches a planted "T30 1.96 s"
//   b80-after  the arm-B fixture (tests/fixtures/rooms/elmia_arm_b.simpa, backlog 78's arm B: S01
//              alone, 150,000 particles, 0.31 m): run; on the Results step, "Why values are
//              missing" holds `range_below_zero`, which names the Receiver radius from the run's
//              0.31 to 0.6 m; every number on the card equals `simpa results --json` at its path;
//              each T30 refused `range_below_zero` is marked `[data-advice=range_below_zero]`.
//              "Apply and re-run" sets the radius to 0.6 as one undo step and runs again; the new
//              run's T30 at every receiver-band the first run refused `range_below_zero` is shown
//              (a value with its range), none refused so. Control: the first run's CLI report
//              refuses at least one T30 `range_below_zero`
//   b80-undo   Apply on the first run's card once the project holds 0.6 is refused ("the project
//              changed since this run"): no run starts, the project is unchanged. Undo then
//              restores the run's 0.31 exactly
//
// Its files, under <M11_WORK>\b80 (C:): the imported tutorial 2, a copy of the arm-B fixture and
// its runs. The repository is only read.
import { strict as assert } from 'node:assert';
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { numberMismatch, stringMismatch } from '../lib/acoustics.ts';
import { cliReport, pick, repo, type Row, runSpps, showRun } from '../lib/acousticsTab.ts';
import { ACOUSTIC_NUMBER, allConsoleLines, PARAMETER_NUMBER, textOutsideInputsAndGeometry } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { env } from '../lib/types.ts';

const WORK = () => path.join(env('M11_WORK'), 'b80');

interface AdviceRowView {
  key: string;
  label: string | null;
  change: string | null;
  apply: { setting: string; from: number | boolean; to: number | boolean } | null;
}

async function simulateAdvice(): Promise<AdviceRowView[]> {
  // `simulateView` registers when the Simulate panel mounts, so it exists only once the Simulate step is open.
  await waitForHooks(['simulateView'], 30_000);
  const v = await hook<{ advice: AdviceRowView[] | null }>('simulateView');
  return v.advice ?? [];
}

function projectRadius(json: string): number {
  return (JSON.parse(json) as { solvers: { spps: { receiver_radius_m: number } } }).solvers.spps.receiver_radius_m;
}

type Json = Record<string, unknown>;

/** Every receiver-band (`<receiver>.<band>`, per source too) whose T30 the report refuses `range_below_zero`. */
function belowZeroT30(rep: Json): string[] {
  const out: string[] = [];
  const rxs = (rep.spps as { point_receivers: { label: string; bands: { parameters: { t30_s: Json } }[]; per_source: { source: string; bands: { parameters: { t30_s: Json } }[] }[] }[] }).point_receivers;
  const why = (e: Json) => ((e.not_evaluable as Json | undefined)?.error as { why?: { why?: string } } | undefined)?.why?.why;
  for (const r of rxs) {
    r.bands.forEach((b, j) => {
      if (why(b.parameters.t30_s) === 'range_below_zero') out.push(`${r.label}.${j}`);
    });
    for (const s of r.per_source)
      s.bands.forEach((b, j) => {
        if (why(b.parameters.t30_s) === 'range_below_zero') out.push(`${r.label}.${s.source}.${j}`);
      });
  }
  return out;
}

function t30At(rep: Json, key: string): Json {
  const parts = key.split('.');
  const rxs = (rep.spps as { point_receivers: { label: string; bands: { parameters: { t30_s: Json } }[]; per_source: { source: string; bands: { parameters: { t30_s: Json } }[] }[] }[] }).point_receivers;
  const r = rxs.find((x) => x.label === parts[0]);
  assert.ok(r, key);
  if (parts.length === 2) return r.bands[Number(parts[1])].parameters.t30_s;
  const s = r.per_source.find((x) => x.source === parts[1]);
  assert.ok(s, key);
  return s.bands[Number(parts[2])].parameters.t30_s;
}

/** The new run that a click started, ended OK: its Runs row. */
async function nextRun(before: Set<string>): Promise<Row> {
  let fresh: Row | undefined;
  await browser.waitUntil(
    async () => {
      if ((await hook<unknown>('runState')) !== null) return false;
      fresh = (await hook<Row[]>('runsRows')).find((r) => !before.has(r.run) && r.status !== 'RUNNING');
      return fresh !== undefined;
    },
    { timeout: 900_000, interval: 500, timeoutMsg: 'no SPPS run ended within 900 s of Apply and re-run' },
  );
  await m10.idle();
  assert.equal(fresh?.status, 'OK', `the re-run ended ${fresh?.status}`);
  return fresh as Row;
}

describe('Backlog 80: the run-quality advisor', () => {
  let armB = '';
  let firstRun: Row | undefined;
  let firstJson: Json = {};

  before(async () => {
    // `simulateView` is not here: it registers only once the Simulate step is open (simulateAdvice waits for it).
    await waitForHooks(['idle', 'openProject', 'setStep', 'dockTab', 'runStart', 'runState', 'runsRows', 'selectRun', 'setSolver', 'acousticsView', 'projectJson', 'undoDepth']);
    mkdirSync(WORK(), { recursive: true });
  });

  it('b80-pre: tutorial 2 as upstream ships it gets both warnings before its run', async () => {
    const proj = path.join(env('M11_UPSTREAM'), 'src', 'isimpa', 'resources', 'doc', 'tutorial', 'tutorial 2', 'tutorial_2.proj');
    const t2 = path.join(WORK(), 'tutorial2.simpa');
    execFileSync(env('M11_SIMPA'), ['import-proj', proj, t2], { encoding: 'utf8' });
    await m10.openProject(t2);
    await hook('setSolver', 'spps');
    await m10.setStep('simulate');
    await $('[data-part="run-quality"]').waitForExist({ timeout: 30_000 });
    const rows = await simulateAdvice();
    const mesh = rows.find((r) => r.key === 'mesh_splits_walls');
    const small = rows.find((r) => r.key === 'receivers_small');
    assert.ok(mesh, `no mesh_splits_walls in ${JSON.stringify(rows.map((r) => r.key))}`);
    assert.ok(small, `no receivers_small in ${JSON.stringify(rows.map((r) => r.key))}`);
    assert.equal(mesh.change, 'off → on');
    assert.equal(small.change, '0.31 m → 0.6 m');
    const meshText = await $('[data-advice="mesh_splits_walls"]').getText();
    assert.match(meshText, /This meshing splits the walls; it can lose particles/);
    assert.ok(await $('[data-advice="receivers_small"] [data-part="advice-apply"]').isExisting());
    // Advice never blocks Run.
    const blockers = await hook<string[]>('runBlockers');
    assert.ok(!blockers.some((b) => /ADVICE|RECEIVERS_SMALL|MESH_SPLITS/.test(b)), `${blockers}`);

    // Control: the Elmia fixture shows neither.
    const fx = path.join(WORK(), 'elmia_corrected.simpa');
    copyFileSync(repo('tests/fixtures/rooms/elmia_corrected.simpa'), fx);
    await m10.openProject(fx);
    await m10.setStep('simulate');
    const keys = (await simulateAdvice()).map((r) => r.key);
    assert.ok(!keys.includes('mesh_splits_walls') && !keys.includes('receivers_small'), `${keys}`);
  });

  it('b80-h: the Simulate step with the advice shown holds no acoustic number (m10-h, m11-h rules 1-2)', async () => {
    const t2 = path.join(WORK(), 'tutorial2.simpa');
    await m10.openProject(t2);
    await hook('setSolver', 'spps');
    await m10.setStep('simulate');
    await $('[data-advice="receivers_small"]').waitForExist({ timeout: 30_000 });
    const text = await textOutsideInputsAndGeometry();
    assert.equal(ACOUSTIC_NUMBER.exec(text)?.[0] ?? null, null, 'a number next to a unit on the Simulate step');
    assert.equal(PARAMETER_NUMBER.exec(text)?.[0] ?? null, null, 'a parameter name followed by a number on the Simulate step');
    // Control: the same reading catches a plant in an advice row.
    await browser.execute(() => {
      const el = document.querySelector('[data-advice="receivers_small"]');
      const s = document.createElement('span');
      s.id = 'b80-plant';
      s.textContent = ' T30 1.96 s';
      el?.appendChild(s);
    });
    const planted = await textOutsideInputsAndGeometry();
    await browser.execute(() => document.getElementById('b80-plant')?.remove());
    assert.ok(ACOUSTIC_NUMBER.test(planted) && PARAMETER_NUMBER.test(planted), 'the plant was not caught');
  });

  it('b80-after: a range_below_zero T30 names the receiver radius; Apply and re-run shows the T30', async () => {
    armB = path.join(WORK(), 'elmia_arm_b.simpa');
    copyFileSync(repo('tests/fixtures/rooms/elmia_arm_b.simpa'), armB);
    await m10.openProject(armB);
    firstRun = await runSpps();
    firstJson = cliReport(armB, firstRun.run);
    const refusedKeys = belowZeroT30(firstJson);
    assert.ok(refusedKeys.length > 0, 'control: the arm-B run refuses no T30 range_below_zero');
    await showRun(firstRun.run);

    const card = '[data-dock-panel="acoustics"] [data-part="advice-card"] [data-advice="range_below_zero"]';
    await $(card).waitForExist({ timeout: 30_000 });
    // Every number and string on the card is the report's, at its path.
    const marks = await browser.execute((sel: string) => {
      const root = document.querySelector(sel);
      const nums = [...(root?.querySelectorAll('[data-num]') ?? [])].map((e) => ({
        path: e.getAttribute('data-json') ?? '',
        text: e.textContent ?? '',
        digits: e.getAttribute('data-digits'),
        scale: e.getAttribute('data-scale'),
      }));
      const strs = [...(root?.querySelectorAll('[data-str]') ?? [])].map((e) => ({ path: e.getAttribute('data-json') ?? '', text: e.textContent ?? '' }));
      return { nums, strs, text: root?.textContent ?? '' };
    }, card);
    assert.ok(marks.nums.length >= 2, 'the card shows the radius from and to');
    for (const n of marks.nums) assert.equal(numberMismatch(n, firstJson), null, n.path);
    for (const t of marks.strs) assert.equal(stringMismatch(t, firstJson), null, t.path);
    assert.match(marks.text, /Receiver radius/);
    const advice = (firstJson.advice as { code: string; fix: { setting: string; from: number; to: number } }[]).find((a) => a.code === 'range_below_zero');
    assert.ok(advice);
    assert.equal(advice.fix.setting, 'receiver_radius');
    assert.equal(Math.fround(advice.fix.from), Math.fround(0.31));
    assert.equal(advice.fix.to, 0.6);
    // The refused T30 cells point to the card.
    // The refused T30 cells point to the card: the RT table of some receiver shows one.
    const receivers = (firstJson.spps as { point_receivers: unknown[] }).point_receivers.length;
    let marked = 0;
    for (let i = 0; i < receivers && marked === 0; i++) {
      await pick('receiver', String(i));
      marked = await $$('[data-dock-panel="acoustics"] [data-cell][data-param="t30_s"][data-advice="range_below_zero"]').length;
    }
    assert.ok(marked > 0, 'no T30 cell names range_below_zero under any receiver');

    const depth = await hook<number>('undoDepth');
    const before = new Set((await hook<Row[]>('runsRows')).map((r) => r.run));
    await $(`${card} [data-part="advice-apply-rerun"]`).click();
    const second = await nextRun(before);
    assert.equal(projectRadius(await m10.projectJson()), 0.6);
    assert.equal(await hook<number>('undoDepth'), depth + 1, 'Apply is one undo step');
    const secondJson = cliReport(armB, second.run);
    assert.deepEqual(belowZeroT30(secondJson), [], 'the re-run still refuses a T30 range_below_zero');
    for (const k of refusedKeys) {
      const e = t30At(secondJson, k);
      assert.ok(typeof e.value === 'number' && typeof e.lo === 'number' && typeof e.hi === 'number', `${k}: T30 not shown after the re-run: ${JSON.stringify(e).slice(0, 200)}`);
    }
    await showRun(second.run);
    console.log(`receipt b80-after: ${refusedKeys.length} T30 range_below_zero at 0.31 m (${firstRun.run}), 0 at 0.6 m (${second.run})`);
  });

  it('b80-undo: Apply is refused once the project changed since the run; undo restores the run\'s radius', async () => {
    assert.ok(firstRun);
    await hook('selectRun', firstRun.run);
    await showRun(firstRun.run);
    const card = '[data-dock-panel="acoustics"] [data-part="advice-card"] [data-advice="range_below_zero"]';
    await $(card).waitForExist({ timeout: 30_000 });
    const depth = await hook<number>('undoDepth');
    const before = new Set((await hook<Row[]>('runsRows')).map((r) => r.run));
    // The Console's lines are on the page only while its tab is open: read them there, and count the refusal
    // lines rather than slice by position (the pane may trim old lines).
    const refusal = (l: string) => /ADVICE_PROJECT_CHANGED/.test(l) && /the project changed since this run/.test(l);
    const consoleNow = async () => {
      await hook('dockTab', 'console');
      await m10.idle();
      return allConsoleLines();
    };
    const refusalsBefore = (await consoleNow()).filter(refusal).length;
    await hook('dockTab', 'acoustics');
    await $(card).waitForExist({ timeout: 30_000 });
    await $(`${card} [data-part="advice-apply-rerun"]`).click();
    await m10.idle();
    await browser.pause(1000);
    const lines = await consoleNow();
    assert.ok(lines.filter(refusal).length > refusalsBefore, `no new refusal line in ${JSON.stringify(lines.slice(-8))}`);
    assert.equal(await hook<number>('undoDepth'), depth, 'the refused Apply changed the history');
    assert.equal((await hook<Row[]>('runsRows')).filter((r) => !before.has(r.run)).length, 0, 'a refused Apply started a run');
    assert.equal(projectRadius(await m10.projectJson()), 0.6);
    await m10.undo();
    assert.equal(projectRadius(await m10.projectJson()), 0.31, 'undo restores the run\'s radius exactly');
  });
});
