// Gate (d), Stop-Process on app.exe mid-solve (docs/investigations/2026-09-29-m11/PLAN.md 4.1,
// m11-d-kill). Its own session: the app is killed on purpose, with no chance to clean up, so only
// the Job Object's KILL_ON_JOB_CLOSE can end the solver. The run folder is left with no run.json;
// the after spec reads how the Runs tab lists it.
import { strict as assert } from 'node:assert';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { m10, waitForHooks } from '../lib/hooks.ts';
import { m11, privateSpps, project, recordRun, runsRootOf, sleepUntil, waitRun } from '../lib/m11.ts';
import { alive, processesFrom, stopProcess } from '../lib/procs.ts';
import { MANIFEST_FILE } from '../lib/runs.ts';

const LONG = () => project('long', 'box_long.simpa');

describe('M11 kill', () => {
  before(async () => {
    await browser.setTimeout({ script: 120_000 });
    await waitForHooks(['idle', 'openProject', 'runStart', 'waitRun', 'runState', 'pid']);
  });

  it('m11-d-kill: Stop-Process on app.exe mid-solve leaves no spps.exe 2 s later', async () => {
    const pid = await m11.pid();
    const spps = privateSpps();
    await m10.openProject(LONG());
    await m11.runStart('spps');
    const run = await waitRun(null, 'progress', 120_000);
    recordRun('kill', run);
    const running = processesFrom(spps);
    assert.ok(running.length > 0, `no spps.exe runs from ${spps} before the kill`);

    // The session dies with the app: no browser command from here on.
    const t0 = Date.now();
    stopProcess(pid);
    await sleepUntil(t0 + 2_000);
    const q0 = Date.now();
    const left = processesFrom(spps);
    console.log(`m11-d-kill receipt: run ${run}; spps.exe before: pids ${running.join(', ')}; at t0 + ${q0 - t0} ms: ${left.length ? `pids ${left.join(', ')}` : 'none'}; app.exe alive: ${alive(pid)}`);
    assert.equal(alive(pid), false, 'app.exe survived Stop-Process');
    assert.deepEqual(left, [], `spps.exe still runs from ${spps} 2 s after Stop-Process`);
    // Nothing in the app ran after the kill: no run.json, now or a moment later.
    const manifest = path.join(runsRootOf(LONG()), run, MANIFEST_FILE);
    await new Promise((r) => setTimeout(r, 1_000));
    assert.equal(existsSync(manifest), false, `${manifest} exists: something wrote it after the kill`);
  });
});
