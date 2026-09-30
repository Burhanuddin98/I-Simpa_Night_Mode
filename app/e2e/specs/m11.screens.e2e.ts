// Screenshots of M11's screens for the investigation folder. Not a gate id: nothing is asserted
// beyond the page being in the state the picture claims. Run by
// `tools/gates/m11.ps1 -Only e2e -Spec screens [-ScreensDir <dir>]`, which passes M11_SCREENS.
// Every run lands in the gate's copies of the projects on C:. The window is left as it is:
// the pictures are the page's own (WebDriver's screenshot), so nothing is moved or resized.
//   1-simulate-mid-run.png      the corrected hall solving: the Simulate step's running block,
//                               the step bar's percentage, the Console's live PROGRESS line
//   2-console-box-run.png       the box after its run: the run's lines, tags and counts strip
//   3-runs-box-run.png          the Runs tab after the box run, its row opened
//   4-runs-mesh-failure.png     the forced mesh failure's row: FAIL, MESH_TETGEN_SKIPPED
//   5-results-refused.png       the Results step for that run: refused, no value
import { strict as assert } from 'node:assert';
import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { clickSelector } from '../lib/dom.ts';
import { m10, waitForHooks } from '../lib/hooks.ts';
import { m11, need, project, rowStatus, runRow, runsRootOf, showTab, waitRun } from '../lib/m11.ts';
import { runFolders } from '../lib/runs.ts';

const BOX = () => project('box', 'box_run.simpa');
const HALL = () => project('hall', 'hall_run.simpa');
const MESHFAIL = () => project('meshfail', 'box_run.simpa');

/** Settles the app and the next two frames, moves the pointer off the view, then saves. */
async function shoot(name: string): Promise<void> {
  const dir = need('M11_SCREENS');
  mkdirSync(dir, { recursive: true });
  await browser
    .action('pointer', { parameters: { pointerType: 'mouse' } })
    .move({ x: 2, y: 2, origin: 'viewport', duration: 0 })
    .perform();
  await browser.execute(() => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r()))));
  const file = path.join(dir, name);
  await browser.saveScreenshot(file);
  console.log(`screenshot ${file}`);
}

describe('M11 screens', () => {
  before(async () => {
    await browser.setTimeout({ script: 120_000 });
    await waitForHooks(['idle', 'openProject', 'runStart', 'runCancel', 'runState', 'waitRun', 'selectRun', 'setStep']);
  });

  it('screens: the Simulate step mid-run on the hall', async () => {
    await m10.openProject(HALL());
    await m10.setStep('simulate');
    await showTab('console');
    await m11.runStart('spps');
    const run = await waitRun(null, 'progress', 240_000);
    // A few seconds into the solve, so the percentage has moved.
    await browser.pause(3_000);
    const state = await m11.runState();
    assert.equal(state?.run, run);
    assert.equal(state?.status, 'running');
    await $('[data-part="running-head"] [data-diagnostic="progress_pct"]').waitForExist({ timeout: 30_000 });
    await shoot('1-simulate-mid-run.png');
    await m11.runCancel();
    await waitRun(run, 'ended', 60_000);
  });

  it('screens: the Console and the Runs tab after the box run', async () => {
    await m10.openProject(BOX());
    await m10.setStep('simulate');
    await (await $('[data-part="run"]')).click();
    const run = await waitRun(null, 'ended', 180_000);
    await m10.idle();
    await showTab('console');
    await $(`[data-run-counts="${run}"]`).waitForExist({ timeout: 30_000 });
    await shoot('2-console-box-run.png');
    await showTab('runs');
    await m11.selectRun(run);
    await (await runRow(run)).$('[data-part="detail"]').waitForExist({ timeout: 30_000 });
    assert.equal((await rowStatus(run)).attr, 'OK');
    await shoot('3-runs-box-run.png');
  });

  it('screens: the Runs tab and the Results step for the forced mesh failure', async () => {
    const runs = runFolders(runsRootOf(MESHFAIL()));
    assert.equal(runs.length, 1, runs.join(', '));
    const run = runs[0];
    await m10.openProject(MESHFAIL());
    await m10.setStep('geometry');
    await showTab('runs');
    await m11.selectRun(run);
    await (await runRow(run)).$('[data-reason-code="MESH_TETGEN_SKIPPED"]').waitForExist({ timeout: 30_000 });
    assert.equal((await rowStatus(run)).attr, 'FAIL');
    await shoot('4-runs-mesh-failure.png');
    await clickSelector('[data-step="results"]');
    await browser.waitUntil(
      async () => (await $('[data-props-step="results"] [data-results-state]').getAttribute('data-results-state')) === 'refused',
      { timeout: 30_000, timeoutMsg: 'the Results step does not read refused' },
    );
    await shoot('5-results-refused.png');
  });
});
