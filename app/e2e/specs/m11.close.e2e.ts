// Gate (d), closing the window mid-solve (docs/investigations/2026-09-29-m11/PLAN.md 4.1,
// m11-d-close). Its own session: the app is closed on purpose, by a real WM_CLOSE to its main
// window, the message the close button and Alt+F4 send. Nothing here activates, moves or hides
// the window.
//
// First the save prompt (row 22, A9): with the project dirty, WM_CLOSE must not close the window;
// the prompt appears, and Cancel keeps the app alive. A second WM_CLOSE while the prompt is open,
// and a third just after Cancel, both within the 5 s the backend gives an unanswered request
// before it takes the UI for hung, must not close past the prompt either (the UI acknowledges
// each request at once; FOUNDATION.md F-11). Then, with the project saved and a run solving,
// WM_CLOSE again: the app cancels the run, waits for its run.json, and exits.
import { strict as assert } from 'node:assert';
import path from 'node:path';
import { m10, waitForHooks } from '../lib/hooks.ts';
import { m11, privateSpps, project, recordRun, runsRootOf, sleepUntil, waitRun } from '../lib/m11.ts';
import { alive, closeMainWindow, processesFrom, windowState } from '../lib/procs.ts';
import { readManifest } from '../lib/runs.ts';

const LONG = () => project('long', 'box_long.simpa');
/** main.rs: a second close request within this long of one the UI never acknowledged is let
 * through as a hung UI. The repeated closes below must land inside it to test anything. */
const HUNG_UI_MS = 5_000;

const promptShown = () => $('[data-prompt]').isExisting();

async function waitPrompt(what: string): Promise<void> {
  await $('[data-prompt]').waitForExist({ timeout: 15_000, timeoutMsg: `${what}: no save prompt` });
}

async function cancelPrompt(): Promise<void> {
  await (await $('[data-prompt] [data-choice="cancel"]')).click();
  await browser.waitUntil(async () => !(await promptShown()), { timeout: 10_000, timeoutMsg: 'the prompt stayed after Cancel' });
}

describe('M11 close', () => {
  before(async () => {
    await browser.setTimeout({ script: 120_000 });
    await waitForHooks(['idle', 'openProject', 'saveAs', 'edit', 'undo', 'dirty', 'projectJson', 'runStart', 'waitRun', 'runState', 'pid', 'promptOpen']);
  });

  it('m11-d-close: closing the window mid-solve prompts first when dirty, then leaves no spps.exe 2 s later', async () => {
    const pid = await m11.pid();
    const spps = privateSpps();
    await m10.openProject(LONG());
    const p = JSON.parse(await m10.projectJson());
    const r1 = p.point_receivers[0];

    // One edit: the project is dirty.
    const out = await m10.edit({ op: 'rename', target: { kind: 'point_receiver', id: r1.id }, name: `${r1.name} (edited)` });
    assert.ok(out.applied, JSON.stringify(out.refusals));
    assert.equal(await m10.dirty(), true);

    // WM_CLOSE: the prompt, not a closed window.
    const tFirst = Date.now();
    assert.equal(closeMainWindow(pid), true, 'WM_CLOSE was not delivered to the main window');
    await waitPrompt('the first WM_CLOSE on a dirty project');
    console.log(`m11-d-close receipt: the first WM_CLOSE (dirty) showed the prompt: '${(await $('[data-prompt]').getText()).replace(/\s+/g, ' ')}'`);
    // A second WM_CLOSE while the prompt is open (a double click on the close button).
    assert.equal(closeMainWindow(pid), true, 'the second WM_CLOSE was not delivered');
    const tSecond = Date.now() - tFirst;
    await browser.pause(500);
    assert.ok(alive(pid), `the second WM_CLOSE, ${tSecond} ms after the first, closed the app past the save prompt`);
    assert.ok(await promptShown(), 'the prompt closed on the second WM_CLOSE');
    await cancelPrompt();
    // A third, just after Cancel: the prompt again.
    assert.equal(closeMainWindow(pid), true, 'the third WM_CLOSE was not delivered');
    const tThird = Date.now() - tFirst;
    await waitPrompt(`WM_CLOSE ${tThird} ms after the first, just after Cancel`);
    console.log(`m11-d-close receipt: WM_CLOSE again at +${tSecond} ms (prompt open) and at +${tThird} ms (after Cancel): the prompt each time, the app alive`);
    assert.ok(tThird < HUNG_UI_MS, `the repeated closes took ${tThird} ms, past the ${HUNG_UI_MS} ms they must fall inside to test anything`);
    await cancelPrompt();
    // Control: the close requests were intercepted, not ignored and not obeyed.
    assert.ok(alive(pid), 'the app exited after Cancel');
    const shown = windowState(pid).windows.find((w) => w.title === 'I-Simpa Night Mode');
    assert.ok(shown && shown.visible && !shown.minimised, `the window is gone or hidden after Cancel: ${JSON.stringify(windowState(pid).windows)}`);
    assert.equal(await m10.dirty(), true, 'Cancel keeps the unsaved edit');

    // Undo and save: a clean project on disk, then a run.
    await m10.undo();
    await m10.saveAs(LONG());
    assert.equal(await m10.dirty(), false);
    await m11.runStart('spps');
    const run = await waitRun(null, 'progress', 120_000);
    recordRun('close', run);
    // Positive control: the private spps.exe runs before the close.
    const running = processesFrom(spps);
    assert.ok(running.length > 0, `no spps.exe runs from ${spps} before the close`);

    // WM_CLOSE mid-solve. The session dies with the app: no browser command from here on.
    const t0 = Date.now();
    assert.equal(closeMainWindow(pid), true, 'the WM_CLOSE mid-solve was not delivered');
    await sleepUntil(t0 + 2_000);
    const q0 = Date.now();
    const left = processesFrom(spps);
    console.log(`m11-d-close receipt: run ${run}; spps.exe before: pids ${running.join(', ')}; at t0 + ${q0 - t0} ms: ${left.length ? `pids ${left.join(', ')}` : 'none'}`);
    assert.deepEqual(left, [], `spps.exe still runs from ${spps} 2 s after WM_CLOSE`);
    let exited = -1;
    while (Date.now() - t0 < 5_000) {
      if (!alive(pid)) {
        exited = Date.now() - t0;
        break;
      }
      await new Promise((r) => setTimeout(r, 100));
    }
    console.log(`m11-d-close receipt: app.exe ${exited >= 0 ? `exited ${exited} ms after WM_CLOSE` : 'STILL RUNS 5 s after WM_CLOSE'}`);
    assert.ok(exited >= 0, 'app.exe did not exit within 5 s of WM_CLOSE');
    // The close path cancelled the run and waited for its record.
    const m = readManifest(path.join(runsRootOf(LONG()), run));
    assert.ok(m, `the closed run ${run} has no run.json: the close did not wait for it`);
    console.log(`m11-d-close receipt: run.json verdict ${m.verdict.status}, outcome ${JSON.stringify(m.outcome)}`);
    assert.equal(m.verdict.status, 'CANCELLED');
  });
});
