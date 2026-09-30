// The M11 harness smoke test (foundation, docs/investigations/2026-09-29-m11/PLAN.md 9.0): the
// release app launches under tauri-driver with --e2e, its window is visible and on screen but
// did not take the foreground, the solvers it finds are the gate's private copy of the verified
// build, and one real run of the box goes end to end through the IPC contract: a WebDriver click
// on Run, the stream, run.json, the Runs rows, the results state. The Runs tab, the Console and
// the Simulate panel are the packages'; this reads the hooks and the files.
import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { stepNames } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { processesFrom, processTree, windowState } from '../lib/procs.ts';
import { env } from '../lib/types.ts';

const FIVE_STEPS = ['Geometry', 'Materials', 'Sources & receivers', 'Simulate', 'Results'];
const M11_HOOKS = ['runStart', 'runCancel', 'runState', 'waitRun', 'runLog', 'runsRows', 'selectRun', 'resultsState', 'pingStats', 'pid', 'solversStatus', 'openProj', 'openPath', 'promptOpen'];

interface Check {
  name: string;
  path: string;
  matches: boolean;
}
interface Row {
  run: string;
  status: string;
  loss?: { worst_pct: string; limit_pct: string } | null;
  lines?: { progress: number; info: number; ok: number; warn: number; fail: number } | null;
}
interface Log {
  counts: Record<string, number>;
  n: number;
  min: number | null;
  max: number | null;
  dupes: number;
  gaps: number;
}

/**
 * The window is visible, not minimised, on a monitor, and no window of the app's process tree
 * (app.exe and the WebView2 processes it starts) is the foreground window. Comparing with the
 * app's pid alone let a WebView2 console window through (FOUNDATION.md F-21).
 */
function assertUnfocusedAndShown(pid: number, when: string): void {
  const w = windowState(pid);
  const tree = processTree(pid);
  console.log(`m11-smoke receipt (${when}): foreground pid ${w.foregroundPid}, app pid ${pid}, app tree ${tree.join(',')}, windows ${JSON.stringify(w.windows)}`);
  const main = w.windows.find((x) => x.title === 'I-Simpa Night Mode');
  assert.ok(main, `no app window titled 'I-Simpa Night Mode': ${JSON.stringify(w.windows)}`);
  assert.ok(main.visible && !main.minimised && main.onMonitor, `${when}: the test window must stay visible and on screen`);
  assert.ok(!tree.includes(w.foregroundPid), `${when}: a window of the app's process tree (pid ${w.foregroundPid}) took the foreground`);
}

describe('M11 harness smoke', () => {
  it('m11-smoke: an unfocused, visible app runs the box end to end through the IPC contract', async () => {
    await browser.waitUntil(async () => (await stepNames()).length === 5, {
      timeout: 60_000,
      timeoutMsg: 'the step bar did not show five steps within 60 s',
    });
    assert.deepEqual(await stepNames(), FIVE_STEPS);
    await waitForHooks(['idle', 'openProject', ...M11_HOOKS]);
    const pid = await hook<number>('pid');
    assertUnfocusedAndShown(pid, 'at launch');
    // F9: what the page believes about focus, recorded (WebDriver's events need no OS focus).
    console.log(`m11-smoke receipt: document.hasFocus() = ${await browser.execute(() => document.hasFocus())}`);

    // The solvers: all four found in the gate's private copy (SIMPA_SOLVERS_DIR reached the app
    // through tauri-driver and msedgedriver), each the verified build.
    const solvers = env('M11_SOLVERS').toLowerCase();
    const status = await hook<{ checks: Check[]; blockers: string[] }>('solversStatus');
    console.log(`m11-smoke receipt: solvers ${status.checks.map((c) => `${c.name} ${c.matches ? 'verified' : 'NOT VERIFIED'} ${c.path}`).join('; ')}`);
    assert.deepEqual(status.blockers, []);
    assert.equal(status.checks.length, 4);
    for (const c of status.checks) {
      assert.ok(c.matches, `${c.name} is not the verified build`);
      assert.ok(c.path.toLowerCase().startsWith(solvers), `${c.name} was found at ${c.path}, not in ${solvers}`);
    }

    // The box: nothing blocks Run; a WebDriver click runs it.
    const box = path.join(env('M11_P'), 'box', 'box_run.simpa');
    await m10.openProject(box);
    const run = await $('[data-part="run"]');
    assert.equal(await run.getAttribute('data-blockers'), '');
    assert.equal(await run.isEnabled(), true);
    const t0 = Date.now();
    await run.click();
    const name = await hook<string>('waitRun', null, 'ended', 180_000);
    console.log(`m11-smoke receipt: run ${name} ended in ${Date.now() - t0} ms`);
    const spps = path.join(env('M11_SOLVERS'), 'spps.exe');
    assert.deepEqual(processesFrom(spps), [], 'no spps.exe runs from the private copy after the run');

    // The row, from run.json through runs_list.
    const rows = await hook<Row[]>('runsRows');
    const row = rows.find((r) => r.run === name);
    assert.ok(row, `no Runs row ${name}: ${JSON.stringify(rows)}`);
    assert.equal(row.status, 'OK');
    assert.equal(row.loss?.worst_pct, '0.00');
    assert.equal(row.loss?.limit_pct, '1');

    // The stream against the manifest: every solver line once, per class as run.json counts them.
    const manifest = JSON.parse(readFileSync(path.join(path.dirname(box), 'runs', name, 'run.json'), 'utf8'));
    const lines = manifest.lines as Record<string, number>;
    assert.ok(lines.progress > 0 && lines.ok === 1, `trivial counts: ${JSON.stringify(lines)}`);
    const log = await hook<Log>('runLog', name);
    console.log(`m11-smoke receipt: manifest lines ${JSON.stringify(lines)}; streamed ${JSON.stringify(log)}`);
    assert.deepEqual(log.counts, { PROGRESS: lines.progress, INFO: lines.info, OK: lines.ok, WARN: lines.warn, FAIL: lines.fail });
    const total = lines.progress + lines.info + lines.ok + lines.warn + lines.fail;
    assert.deepEqual([log.n, log.min, log.max, log.dupes, log.gaps], [total, 0, total - 1, 0, 0]);
    // The run verified its executables (C7) and recorded it; its results verify.
    const checks = manifest.solvers as Check[];
    assert.deepEqual(
      checks.map((c) => [c.name, c.matches]),
      [
        ['spps.exe', true],
        ['tetgen.exe', true],
        ['preprocess.exe', true],
      ],
    );
    const results = await hook<{ verified: boolean }>('resultsState', name);
    assert.equal(results.verified, true);

    assertUnfocusedAndShown(pid, 'after the run');
  });
});
