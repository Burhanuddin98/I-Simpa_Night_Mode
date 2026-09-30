// A page reload mid-run (M11 review 2, app 2). PQ4 (no New, Open or Import during a run) was held
// only in the page's memory: after a reload the page had no record of the run, showed no Cancel,
// enabled Run, and New replaced the project while spps.exe went on solving. The page must take
// the run back (Cancel shown, Run blocked), the backend must refuse an Open by itself (the hook
// opens without the page's guard, as `openProject` is the no-prompt primitive), and Cancel from
// the reloaded page must end the run. Its own session and its own copy of the long box, so no
// other spec's runs folder changes. Nothing here activates, moves or hides the window.
import { strict as assert } from 'node:assert';
import path from 'node:path';
import { consoleLines } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { m11, privateSpps, project, runsRootOf, showTab, waitRun } from '../lib/m11.ts';
import { processesFrom } from '../lib/procs.ts';
import { readManifest } from '../lib/runs.ts';

const LONG = () => project('reload', 'box_long.simpa');
const OTHER = () => project('box', 'box_run.simpa');
const HOOKS = ['idle', 'openProject', 'projectJson', 'runStart', 'waitRun', 'runState', 'runBlockers', 'setStep'];

describe('M11 reload', () => {
  before(async () => {
    await browser.setTimeout({ script: 120_000 });
    await waitForHooks(HOOKS);
  });

  it('m11-reload: a page reloaded mid-run takes the run back, New and Open stay refused, and Cancel ends the run', async () => {
    const spps = privateSpps();
    await m10.openProject(LONG());
    await m11.runStart('spps');
    const run = await waitRun(null, 'progress', 120_000);
    assert.ok(processesFrom(spps).length > 0, 'the private spps.exe solves before the reload');

    // The reload: everything in the page's memory is gone; the backend's run is not.
    await browser.refresh();
    await waitForHooks(HOOKS);
    await browser.waitUntil(async () => (await m11.runState())?.run === run, {
      timeout: 30_000,
      timeoutMsg: `the reloaded page did not take run ${run} back`,
    });
    const state = await m11.runState();
    assert.equal(state?.status, 'running');
    const blockers = await m10.runBlockers();
    assert.ok(blockers.includes('RUN_ACTIVE'), `Run is not blocked after the reload: ${JSON.stringify(blockers)}`);
    await m10.setStep('simulate');
    await $('[data-part="cancel-run"]').waitForExist({ timeout: 10_000, timeoutMsg: 'no Cancel control after the reload' });
    await showTab('console');
    const warns = await consoleLines('WARN');
    assert.ok(warns.some((t) => t.includes('page was reloaded')), `no line tells the user: ${JSON.stringify(warns)}`);

    // The backend refuses an Open by itself, and the project stays.
    const before = await m10.projectJson();
    let refused = '';
    try {
      await hook('openProject', OTHER());
    } catch (e) {
      refused = String(e);
    }
    assert.ok(refused, 'the backend opened another project during the run');
    const fails = await consoleLines('FAIL');
    const line = fails.find((t) => t.startsWith('Could not open') && t.includes('(RUN_ACTIVE)'));
    assert.ok(line, `no RUN_ACTIVE refusal in the Console: ${JSON.stringify(fails)}`);
    assert.equal(await m10.projectJson(), before, 'the project changed during the run');
    assert.ok(processesFrom(spps).length > 0, 'the run stopped before Cancel');
    console.log(`m11-reload receipt: run ${run} taken back after the reload (status ${state?.status}, blockers ${blockers.join(' ')}); Open refused: '${line}'`);

    // Cancel from the reloaded page ends the run.
    await (await $('[data-part="cancel-run"]')).click();
    await browser.waitUntil(async () => (await m11.runState()) === null, {
      timeout: 30_000,
      timeoutMsg: 'the reloaded page still shows the run 30 s after Cancel',
    });
    await browser.waitUntil(async () => processesFrom(spps).length === 0, {
      timeout: 10_000,
      timeoutMsg: 'spps.exe still runs from the private copy after Cancel',
    });
    const m = readManifest(path.join(runsRootOf(LONG()), run));
    assert.ok(m, `run ${run} has no run.json after Cancel`);
    assert.equal(m.verdict.status, 'CANCELLED');
    console.log(`m11-reload receipt: Cancel from the reloaded page: run.json ${m.verdict.status}, no spps.exe from the private copy`);
  });
});
