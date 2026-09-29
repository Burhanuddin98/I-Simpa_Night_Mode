// The M11 gate spec (docs/investigations/2026-09-29-m11/PLAN.md 4.1, 4.2): gate (a), (b), (c) and
// (e), m11-h (no solver-computed acoustic number, the diagnostic allowance proven), and
// m11-r22-default (the core refuses upstream's placeholder material at Run). One session; the
// tests run in order and share the box run and the hall run.
//
// Written by the foundation against the DOM contract of PLAN.md 3.6; the packages make it true:
// the Run button's label and the Cancel button are the simulate package's, the Runs rows and the
// Console's counts the dock package's, the Results step the simulate package's. Every run lands
// in the gate's copies of the projects on C:, never on B:.
//
// The app window stays as Burhan sees it: nothing here moves, resizes, minimises, hides or
// activates it, and every click is a WebDriver event inside the page.
import { strict as assert } from 'node:assert';
import { spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import {
  collectSnapshot,
  describeViolations,
  type DomSnapshot,
  flagged,
  judge,
  type Plant,
  runResolver,
  sayNoCases,
  SNAPSHOT_CONFIG,
} from '../lib/acoustic.ts';
import { clickSelector } from '../lib/dom.ts';
import { m10, waitForHooks } from '../lib/hooks.ts';
import {
  consoleCount,
  m11,
  need,
  privateSpps,
  project,
  rowStatus,
  runRow,
  runsRootOf,
  showTab,
  shown,
  sleepUntil,
  waitRun,
} from '../lib/m11.ts';
import { machineWideCount, processesFrom } from '../lib/procs.ts';
import { elapsedS, type Manifest, readManifest, runFolders, worstLoss } from '../lib/runs.ts';
import { distribution, nearestRank } from '../lib/stats.ts';
import type { Op } from '../lib/types.ts';

const BOX = () => project('box', 'box_run.simpa');
const HALL = () => project('hall', 'hall_run.simpa');
const MESHFAIL = () => project('meshfail', 'box_run.simpa');
const CORRECTED_HALL = () => path.join(need('M11_REPO'), 'testdata', 'elmia_corrected.ply');
const CLASSES = ['PROGRESS', 'INFO', 'OK', 'WARN', 'FAIL'] as const;
const STEPS = ['geometry', 'materials', 'sources', 'simulate', 'results'];
const TABS = ['acoustics', 'console', 'runs'] as const;

const HOOKS = [
  'idle', 'openProject', 'importModel', 'saveAs', 'edit', 'projectJson', 'runBlockers', 'issues', 'runStart',
  'runCancel', 'runState', 'waitRun', 'runLog', 'runsRows', 'selectRun', 'resultsState', 'pingStats',
  'frameStats.begin', 'frameStats.end', 'pid', 'cameraState',
];

/** A run's folder and its run.json (which must exist). */
function manifestOf(projectFile: string, run: string): { dir: string; m: Manifest } {
  const dir = path.join(runsRootOf(projectFile), run);
  const m = readManifest(dir);
  assert.ok(m, `no run.json in ${dir}`);
  return { dir, m };
}

// Shared between the tests, in order.
let boxRun: string | null = null;
let okRowReasonCodes: number | null = null;
let hallRun: string | null = null;
let meshfailRun: string | null = null;
/** The page during the hall solve (m11-c, before Cancel), judged by m11-h once the run's logs are
 * complete: the progress_pct spans exist only mid-run, so only a mid-run read can prove them. */
const midRun: { where: string; snap: DomSnapshot }[] = [];

async function blockersAttr(): Promise<string | null> {
  const run = await $('[data-part="run"]');
  await run.waitForExist({ timeout: 30_000 });
  return run.getAttribute('data-blockers');
}

/** The run is still solving: the app says running, and the private spps.exe is alive. */
async function stillSolving(run: string, what: string): Promise<void> {
  const state = await m11.runState();
  assert.equal(state?.run, run, `${what}: the run ${run} is no longer the active run (${JSON.stringify(state)})`);
  assert.equal(state?.status, 'running', `${what}: the run is ${state?.status}, not running`);
  assert.ok(processesFrom(privateSpps()).length > 0, `${what}: no spps.exe runs from ${privateSpps()}`);
}

describe('M11 gate', () => {
  before(async () => {
    // Hooks such as idle() may wait longer than WebDriver's default 30 s script timeout.
    await browser.setTimeout({ script: 120_000 });
    await waitForHooks(HOOKS);
  });

  it("m11-a: the box run from the UI: its Runs row reads OK and 'Particles lost 0.00 %', and the Console's counts equal run.json's", async () => {
    // Control: the raw hall blocks Run, so "empty" below is not vacuous.
    await m10.importModel(need('M11_ELMIA_RAW'), 'm', 'z');
    const raw = await blockersAttr();
    console.log(`m11-a receipt: raw hall data-blockers = '${raw}'`);
    assert.notEqual(raw, '', 'the raw hall must block Run');

    await m10.openProject(BOX());
    assert.equal(await blockersAttr(), '', 'nothing blocks Run on the box');
    const t0 = Date.now();
    await (await $('[data-part="run"]')).click();
    const run = await waitRun(null, 'ended', 180_000);
    boxRun = run;
    const { dir, m } = manifestOf(BOX(), run);
    console.log(`m11-a receipt: run ${run} ended in ${Date.now() - t0} ms; run.json status ${m.verdict.status}, lines ${JSON.stringify(m.lines)}`);

    // (1) The row: OK, as an attribute and as text.
    await showTab('runs');
    const status = await rowStatus(run);
    assert.equal(status.attr, 'OK');
    assert.equal(status.text, 'OK');
    const row = await runRow(run);
    okRowReasonCodes = await row.$$('[data-reason-code]').length;
    assert.equal(okRowReasonCodes, 0, 'an OK run shows no reason code');

    // (2) 'Particles lost 0.00 %', the worst band recomputed from run.json with BigInt.
    const worst = worstLoss(m);
    assert.ok(worst, `run.json in ${dir} has no particle statistics`);
    const loss = shown(await (await row.$('[data-part="loss"]')).getText());
    console.log(`m11-a receipt: [data-part="loss"] reads '${loss}'; run.json's worst band ${worst.freq_hz} Hz, ${worst.lost} of ${worst.total}: ${worst.pct} %`);
    assert.equal(loss, `Particles lost ${worst.pct} %`);
    assert.equal(loss, 'Particles lost 0.00 %');

    // (3) The Console's counts, and the stream the app received, equal run.json's.
    const lines = m.lines;
    assert.ok(lines.progress > 0 && lines.ok === 1, `trivial counts in run.json: ${JSON.stringify(lines)}`);
    const want = { PROGRESS: lines.progress, INFO: lines.info, OK: lines.ok, WARN: lines.warn, FAIL: lines.fail };
    await showTab('console');
    const dom: Record<string, number> = {};
    for (const c of CLASSES) dom[c] = await consoleCount(run, c);
    const log = await m11.runLog(run);
    console.log(`m11-a receipt: Console counts ${JSON.stringify(dom)}; streamed ${JSON.stringify(log)}; run.json ${JSON.stringify(want)}`);
    assert.deepEqual(dom, want);
    assert.ok(log, `no run log for ${run}`);
    assert.deepEqual(log.counts, want);
    const total = lines.progress + lines.info + lines.ok + lines.warn + lines.fail;
    assert.deepEqual([log.n, log.min, log.max, log.dupes, log.gaps], [total, 0, total - 1, 0, 0]);
  });

  it('m11-b-ipc: during the corrected-hall solve, 200 IPC pings have p99 < 100 ms', async () => {
    await m10.openProject(HALL());
    assert.equal(await blockersAttr(), '', 'nothing blocks Run on the hall');
    await m11.runStart('spps');
    const run = await waitRun(null, 'progress', 240_000);
    hallRun = run;
    const lat = await m11.pingStats(200);
    console.log(`m11-b-ipc receipt: run ${run}; 200 pings: ${distribution(lat)}`);
    // Control: the pings were taken during the solve.
    await stillSolving(run, 'after the pings');
    assert.equal(lat.length, 200);
    const p99 = nearestRank(lat, 0.99);
    assert.ok(p99 < 100, `p99 ${p99.toFixed(2)} ms is not under 100 ms`);
  });

  it('m11-b-frames: during the same solve, an orbit drag has frame time p95 < 33 ms', async () => {
    assert.ok(hallRun, 'no hall run from m11-b-ipc');
    await stillSolving(hallRun, 'before the drag');
    const before = await m11.cameraState();
    const c = await browser.execute(() => {
      const r = document.querySelector('canvas')?.getBoundingClientRect();
      return r ? { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + r.height / 2), w: r.width } : null;
    });
    assert.ok(c, 'no canvas');
    const x0 = Math.round(c.x - 120);
    await m11.frameBegin();
    // A real WebDriver pointer drag: down, 60 moves of 4 px and 33 ms each, up.
    let drag = browser
      .action('pointer', { parameters: { pointerType: 'mouse' } })
      .move({ x: x0, y: c.y, origin: 'viewport', duration: 0 })
      .down({ button: 0 });
    for (let i = 1; i <= 60; i++) drag = drag.move({ x: x0 + 4 * i, y: c.y, origin: 'viewport', duration: 33 });
    await drag.up({ button: 0 }).perform();
    const intervals = await m11.frameEnd();
    const after = await m11.cameraState();
    console.log(`m11-b-frames receipt: frame intervals ${distribution(intervals)}`);
    // Controls: the drag orbited the camera, and the run was still solving.
    const moved = Math.hypot(...after.position.map((v, k) => v - before.position[k]));
    assert.ok(moved > 0, `the drag did not move the camera (${JSON.stringify(before.position)})`);
    await stillSolving(hallRun, 'after the drag');
    assert.ok(intervals.length >= 50, `only ${intervals.length} frame intervals`);
    const p95 = nearestRank(intervals, 0.95);
    assert.ok(p95 < 33, `p95 ${p95.toFixed(2)} ms is not under 33 ms`);
  });

  it("m11-c: Cancel sets the row to 'Cancelled', and 2 s later no spps.exe runs from the gate's copy", async () => {
    assert.ok(hallRun, 'no hall run from m11-b-ipc');
    const spps = privateSpps();
    // Cancel is in the Simulate panel's running block (PQ2).
    await clickSelector('[data-step="simulate"]');
    const cancel = await $('[data-part="cancel-run"]');
    await cancel.waitForExist({ timeout: 30_000, timeoutMsg: 'no [data-part="cancel-run"] on the Simulate step' });
    // m11-h's mid-run read: the Simulate step with each dock tab, while the hall still solves.
    for (const tab of TABS) {
      await showTab(tab);
      midRun.push({ where: `hall mid-run, step simulate, tab ${tab}`, snap: await browser.execute(collectSnapshot, SNAPSHOT_CONFIG, null) });
    }
    await stillSolving(hallRun, 'after the mid-run reads');
    // Positive control: the private spps.exe runs before the click.
    const running = processesFrom(spps);
    assert.ok(running.length > 0, `no spps.exe runs from ${spps} before the click`);
    const t0 = Date.now();
    await cancel.click();
    await sleepUntil(t0 + 2_000);
    const q0 = Date.now();
    const left = processesFrom(spps);
    console.log(
      `m11-c receipt: spps.exe from the gate's copy before the click: pids ${running.join(', ')}; ` +
        `at t0 + ${q0 - t0} ms: ${left.length ? `pids ${left.join(', ')}` : 'none'} (the query took ${Date.now() - q0} ms); ` +
        `machine-wide spps.exe count ${machineWideCount('spps.exe')} (printed, not asserted: other sessions run solvers)`,
    );
    assert.deepEqual(left, [], `spps.exe still runs from ${spps} 2 s after Cancel`);

    await waitRun(hallRun, 'ended', 60_000);
    await showTab('runs');
    const status = await rowStatus(hallRun);
    assert.equal(status.attr, 'CANCELLED');
    assert.equal(status.text, 'Cancelled');
    const { m } = manifestOf(HALL(), hallRun);
    console.log(`m11-c receipt: run.json verdict ${m.verdict.status}, outcome ${JSON.stringify(m.outcome)}, files ${JSON.stringify(m.files)}`);
    assert.equal(m.verdict.status, 'CANCELLED');
    assert.equal(m.outcome?.cancelled, true);
    assert.equal(m.outcome?.exit_code, null);
    // Control: the solver stopped mid-run.
    assert.ok(m.files.present < m.files.expected, `files ${JSON.stringify(m.files)}: the solver finished`);
  });

  it('m11-e-row: the forced mesh failure lists as FAIL with reason MESH_TETGEN_SKIPPED', async () => {
    const runs = runFolders(runsRootOf(MESHFAIL()));
    assert.equal(runs.length, 1, `the mesh-failure project's runs: ${runs.join(', ')}`);
    const run = runs[0];
    meshfailRun = run;
    const { m } = manifestOf(MESHFAIL(), run);
    const codes = m.verdict.reasons.map((r) => r.code);
    console.log(`m11-e-row receipt: ${run}: run.json stage ${m.stage}, status ${m.verdict.status}, reasons ${codes.join(', ')}`);
    assert.equal(m.stage, 'mesh');
    assert.ok(codes.includes('tetgen_skipped_facets'));

    await m10.openProject(MESHFAIL());
    await showTab('runs');
    const status = await rowStatus(run);
    assert.equal(status.attr, 'FAIL');
    assert.equal(status.text, 'FAIL');
    const row = await runRow(run);
    const reason = await row.$('[data-reason-code="MESH_TETGEN_SKIPPED"]');
    assert.ok(await reason.isExisting(), 'no [data-reason-code="MESH_TETGEN_SKIPPED"] on the row');
    const text = shown(await reason.getText());
    console.log(`m11-e-row receipt: the reason reads '${text}'`);
    assert.ok(text.includes('MESH_TETGEN_SKIPPED') && text.includes('tetgen_skipped_facets'), `the reason shows both codes: '${text}'`);
    // Control: the OK box run's row in m11-a showed no reason code.
    assert.equal(okRowReasonCodes, 0, "m11-a's OK row must have been read, with no reason code");
  });

  it('m11-e-results: the Results step shows no numbers for the failed run', async () => {
    assert.ok(meshfailRun && boxRun, 'no mesh-failure run (m11-e-row) or box run (m11-a)');
    const panel = '[data-props-step="results"]';
    const stateOf = async (run: string, want: string) => {
      await m11.selectRun(run);
      await clickSelector('[data-step="results"]');
      const el = await $(`${panel} [data-results-state]`);
      await el.waitForExist({ timeout: 30_000, timeoutMsg: `no ${panel} [data-results-state]` });
      await browser.waitUntil(async () => (await el.getAttribute('data-results-state')) === want, {
        timeout: 30_000,
        timeoutMsg: `the Results step of ${run} does not read ${want}: ${await el.getAttribute('data-results-state')}`,
      });
      return browser.execute((sel: string) => {
        const p = document.querySelector(sel);
        const copy = p?.cloneNode(true) as HTMLElement | undefined;
        copy?.querySelectorAll('[data-run-label]').forEach((e) => e.remove());
        return {
          text: (p as HTMLElement | null)?.innerText ?? '',
          unlabelled: copy?.textContent ?? '',
          results: p?.querySelectorAll('[data-result]').length ?? -1,
          label: p?.querySelector('[data-run-label]')?.textContent ?? null,
        };
      }, panel);
    };
    // The failed run (the mesh-failure project is open since m11-e-row).
    const failed = await stateOf(meshfailRun, 'refused');
    console.log(`m11-e-results receipt: refused panel: ${JSON.stringify(failed)}`);
    assert.ok(failed.text.includes('RESULTS_RUN_FAILED'), 'the refusal names RESULTS_RUN_FAILED');
    assert.equal(failed.results, 0, 'no [data-result] element');
    assert.ok(!/\d/.test(failed.unlabelled), `a digit on the Results step outside [data-run-label]: ${failed.unlabelled}`);
    // Control: the same panel on m11-a's OK run reads verified.
    await m10.openProject(BOX());
    const ok = await stateOf(boxRun, 'verified');
    console.log(`m11-e-results receipt: control, verified panel: ${JSON.stringify(ok)}`);
    assert.equal(ok.results, 0);
  });

  it('m11-h: no solver-computed acoustic number on any step or dock tab, and every diagnostic proven', async () => {
    assert.ok(boxRun && hallRun && meshfailRun, 'm11-h needs the runs of m11-a, m11-b and m11-e');
    const roots = ['box', 'hall', 'meshfail', 'long', 'room'].map((p) => path.join(need('M11_P'), p, 'runs'));
    const proof = runResolver(roots, runFolders);
    const snapshot = (plant: Plant | null) => browser.execute(collectSnapshot, SNAPSHOT_CONFIG, plant);

    // Say-NO first, on the box with its run shown: the checker must flag every plant.
    await m10.openProject(BOX());
    const { m } = manifestOf(BOX(), boxRun);
    const elapsed = m.outcome ? elapsedS(m.outcome.elapsed_ms) : '0.0';
    const worst = worstLoss(m);
    assert.ok(worst, 'the box run has no particle statistics');
    const tabOf: Record<string, (typeof TABS)[number]> = {
      [SNAPSHOT_CONFIG.acoustics]: 'acoustics',
      '[data-dock-panel="runs"]': 'runs',
      '[data-dock-panel="console"]': 'console',
    };
    for (const c of sayNoCases(boxRun, elapsed, worst.pct)) {
      await showTab(tabOf[c.plant.host] ?? 'console');
      const vs = judge(await snapshot(c.plant), proof);
      console.log(`m11-h say-NO receipt: ${c.name}: ${flagged(c, vs) ? 'flagged' : 'NOT FLAGGED'} (rule ${c.rule})`);
      assert.ok(flagged(c, vs), `the checker let a plant through: ${c.name}\n${describeViolations(vs)}`);
    }

    // The mid-run reads of m11-c, judged now that the hall run's logs are complete. They must
    // hold progress spans where a run shows its progress (the status bar and the Simulate sub
    // above all, whose text is SPPS's 4-significant-digit percentage rounded for display), or
    // the proof of rule 3 (iv) for progress_pct never ran.
    assert.equal(midRun.length, TABS.length, 'm11-c took no mid-run read');
    const progressRegions = new Set<string>();
    let midDiagnostics = 0;
    for (const { where, snap } of midRun) {
      const vs = judge(snap, proof);
      assert.deepEqual(vs, [], `${where}:\n${describeViolations(vs)}`);
      for (const d of snap.diagnostics) if (d.field === 'progress_pct' && d.region) progressRegions.add(d.region);
      midDiagnostics += snap.diagnostics.length;
    }
    console.log(`m11-h receipt: ${midRun.length} mid-run views clean; ${midDiagnostics} diagnostic span(s); progress_pct proven in: ${[...progressRegions].sort().join(', ')}`);
    for (const region of ['statusbar', 'simulate-sub', 'simulate']) {
      assert.ok(progressRegions.has(region), `no progress_pct span in the ${region} region during the solve`);
    }

    // The real scan: three projects, every step, every dock tab.
    let n = 0;
    let diagnostics = 0;
    let verbatim = 0;
    for (const file of [BOX(), HALL(), MESHFAIL()]) {
      await m10.openProject(file);
      await m11.runsRows();
      for (const step of STEPS) {
        await clickSelector(`[data-step="${step}"]`);
        for (const tab of TABS) {
          await showTab(tab);
          await m10.idle();
          const snap = await snapshot(null);
          const vs = judge(snap, proof);
          assert.deepEqual(vs, [], `${path.basename(path.dirname(file))}, step ${step}, tab ${tab}:\n${describeViolations(vs)}`);
          if (tab === 'acoustics') assert.notEqual(snap.acousticsText, null, 'the Acoustics panel is shown');
          diagnostics += snap.diagnostics.length;
          verbatim += snap.verbatim.length;
          n++;
        }
      }
    }
    console.log(`m11-h receipt: ${n} views clean; ${diagnostics} diagnostic span(s) and ${verbatim} verbatim line(s) proven against their runs`);
    assert.equal(n, 3 * STEPS.length * TABS.length);
  });

  it('m11-r22-default: the core refuses the Default placeholder at Run, and the app says so first', async () => {
    const work = need('M11_GATEWORK');
    const simpa = need('M11_SIMPA');
    await m10.importModel(CORRECTED_HALL(), 'm', 'z');
    // A source and a receiver from tutorial 2's hall (the positions hall_run.simpa uses).
    const ref = JSON.parse(readFileSync(path.join(need('M11_REPO'), 'tests', 'fixtures', 'rooms', 'elmia_corrected.simpa'), 'utf8'));
    const source = { ...ref.sources[0], id: randomUUID(), solver_id: null };
    const receiver = { ...ref.point_receivers[0], id: randomUUID(), solver_id: null };
    for (const op of [
      { op: 'add_source', index: 0, source },
      { op: 'add_point_receiver', index: 0, receiver },
    ] as Op[]) {
      const out = await m10.edit(op);
      assert.ok(out.applied, `${op.op} refused: ${JSON.stringify(out.refusals)}`);
    }
    const ph = path.join(work, 'ph.simpa');
    await m10.saveAs(ph);
    const blockers = await blockersAttr();
    const issues = await m11.issues();
    const placeholders = issues.filter((i) => i.rule === 'material_placeholder');
    console.log(`m11-r22-default receipt: data-blockers '${blockers}'; ${placeholders.length} material_placeholder issue(s) of ${issues.length}`);
    assert.equal(blockers, 'MATERIALS_UNASSIGNED');
    assert.deepEqual(await m10.runBlockers(), ['MATERIALS_UNASSIGNED']);
    assert.equal(placeholders.length, 10, 'one per surface group of the hall');
    assert.ok(placeholders.every((i) => i.code === 'MATERIALS_UNASSIGNED'));

    // The core itself refuses: exit 2, stage validate, material_placeholder, no solver output.
    const runs = path.join(work, 'ph-runs');
    const r = spawnSync(simpa, ['run', ph, '--solver', 'spps', '--runs', runs, '--json'], {
      encoding: 'utf8',
      windowsHide: true,
      env: { ...process.env, SIMPA_SOLVERS_DIR: need('M11_SOLVERS') },
    });
    const folders = runFolders(runs);
    assert.equal(folders.length, 1, `simpa run made ${folders.length} run folder(s) under ${runs}: ${r.stderr}`);
    const dir = path.join(runs, folders[0]);
    const m = readManifest(dir);
    assert.ok(m, `no run.json in ${dir}`);
    const solve = path.join(dir, 'solve');
    const solveFiles = existsSync(solve) ? readdirSync(solve, { recursive: true }).filter((f) => statSync(path.join(solve, String(f))).isFile()).length : 0;
    const codes = m.verdict.reasons.map((x) => x.code);
    console.log(`m11-r22-default receipt: simpa run exit ${r.status}; run.json stage ${m.stage}, status ${m.verdict.status}, reasons ${codes.join(', ')}; files under solve/: ${solveFiles}`);
    assert.equal(r.status, 2);
    assert.equal(m.stage, 'validate');
    assert.ok(codes.includes('material_placeholder'));
    assert.equal(m.outcome, null, 'no solver was launched');
    assert.equal(solveFiles, 0);

    // Control: a material on all 10 groups clears the blocker, and the core's rule with it.
    const p = JSON.parse(await m10.projectJson());
    const bands = p.bands.frequencies_hz.length;
    const material = randomUUID();
    const add = await m10.edit({
      op: 'add_material',
      index: p.materials.length,
      material: {
        id: material,
        name: 'Gate control',
        color: '#555560',
        absorption: Array(bands).fill(0.2),
        scattering: Array(bands).fill(0.1),
        reflection_law: 'specular',
        transmission_loss_db: null,
        double_sided: true,
        solver_id: null,
      },
    });
    assert.ok(add.applied, JSON.stringify(add.refusals));
    const assign = await m10.edit({
      op: 'batch',
      ops: p.surface_groups.map((g: { id: string }) => ({ op: 'set_group_material', group: g.id, material })),
    });
    assert.ok(assign.applied, JSON.stringify(assign.refusals));
    const assigned = path.join(work, 'ph_assigned.simpa');
    await m10.saveAs(assigned);
    assert.equal(p.surface_groups.length, 10);
    assert.equal(await blockersAttr(), '');
    const v = spawnSync(simpa, ['validate', assigned, '--json'], { encoding: 'utf8', windowsHide: true });
    const vIssues = JSON.parse(v.stdout) as { code: string }[];
    console.log(`m11-r22-default receipt: control, all 10 groups assigned: data-blockers ''; simpa validate exit ${v.status}, codes ${vIssues.map((i) => i.code).join(', ') || 'none'}`);
    assert.ok(!vIssues.some((i) => i.code === 'material_placeholder'));
  });
});
