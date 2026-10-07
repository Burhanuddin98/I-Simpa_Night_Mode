// The dock package's checks (docs/investigations/2026-09-29-m11/PLAN.md 9.2): the tabs' badges,
// the Console and the Runs tab, read from the page and held against the run folders' run.json
// and logs. The gate's own ids that read the dock (m11-a's row and counts, m11-c's Cancelled
// row, m11-d-after, m11-e-row) are the gate spec's; these are the package's extra checks:
//   m11-dock-interrupted  the control folder with no run.json lists as Interrupted, with its
//                         reason, and a click selects it
//   m11-dock-live         during a run the Console badge reads "live" and one PROGRESS line is
//                         shown however many arrive, its value rising sample by sample (M11
//                         review F3); the Console follows the bottom and stays
//                         put when scrolled up; Cancel lists the run as Cancelled; after it, the
//                         badge reads "<n> fail" with n the Console's FAIL lines
//   m11-dock-row          an OK run's row and its opened record equal run.json: the per-band
//                         loss (BigInt), the limit, the solver time, the exe sha256 and the
//                         verified mark, the mesh sha256, the line counts; the Console's counts
//                         strip equals run.json; every verbatim line is a line of the run's logs;
//                         and the planted-loss run's row, band by band (M11 review F1)
//   m11-dock-meshfail     the forced mesh failure reads FAIL with MESH_TETGEN_SKIPPED and
//                         tetgen_skipped_facets; its solver check reads "not recorded"
//   m11-dock-h            in the dock, every diagnostic span passes its grammar and equals
//                         run.json, and no other number carries a unit (with a planted say-no)
//
// Runs are made on the gate's copies on C: (M11_P); nothing here writes into the repository.
import { strict as assert } from 'node:assert';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { ACOUSTIC_NUMBER, clickSelector, consoleLines, PARAMETER_NUMBER } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { PLANTED } from '../lib/plant-loss.ts';
import { processesFrom } from '../lib/procs.ts';
import { decimalAbove } from '../lib/runs.ts';
import { env } from '../lib/types.ts';

const P = (name: string, file: string) => path.join(env('M11_P'), name, file);
const BOX = () => P('box', 'box_run.simpa');
const LONG = () => P('long', 'box_long.simpa');
const MESHFAIL = () => P('meshfail', 'box_run.simpa');
/** The planted-loss run's project (m11.ps1, M11 review F1). */
const LOSS = () => P('loss', 'box_run.simpa');
/** The empty run folder m11.ps1 makes in the long box's runs root (PLAN.md 5). */
const INTERRUPTED = '20260101-000000-000-spps';
const CLASSES = ['PROGRESS', 'INFO', 'OK', 'WARN', 'FAIL'] as const;

interface Band {
  freq_hz: number;
  lost_by_infinite_loops: number;
  lost_by_meshing_problems: number;
  total: number;
}
interface Manifest {
  stage: string;
  exe: { path: string; sha256: string };
  solvers?: { name: string; matches: boolean }[];
  solver_manifest?: { source: 'embedded' | 'override'; sha256: string } | null;
  mesh: { mbin_sha256: string } | null;
  outcome: { exit_code: number | null; cancelled: boolean; elapsed_ms: number } | null;
  lines: Record<'progress' | 'info' | 'ok' | 'warn' | 'fail' | 'unclassified', number>;
  particles: { bands: Band[] } | null;
  loss_limit: number;
  verdict: { status: string; reasons: { code: string; detail: string }[]; warnings: unknown[] };
}
interface Row {
  run: string;
  status: string;
}

const runDir = (project: string, run: string) => path.join(path.dirname(project), 'runs', run);
const manifest = (project: string, run: string): Manifest =>
  JSON.parse(readFileSync(path.join(runDir(project, run), 'run.json'), 'utf8'));

// ---- the numbers, recomputed in Node (PLAN.md 2.3, T14) ------------------------------------------

/** A band's loss in hundredths of a percent, rounded half up, in integers. */
function lossHundredths(lost: bigint, total: bigint): bigint {
  return total === 0n ? 0n : (lost * 20_000n + total) / (2n * total);
}
const pct = (h: bigint) => `${h / 100n}.${String(h % 100n).padStart(2, '0')}`;
const bandLost = (b: Band) => BigInt(b.lost_by_infinite_loops) + BigInt(b.lost_by_meshing_problems);
const bandPct = (b: Band) => pct(lossHundredths(bandLost(b), BigInt(b.total)));
/** The largest band's loss, the first on a tie. */
function worstPct(bands: Band[]): string {
  let worst = -1n;
  for (const b of bands) {
    const h = lossHundredths(bandLost(b), BigInt(b.total));
    if (h > worst) worst = h;
  }
  return pct(worst);
}
/** `floor(ms / 100 + 0.5) / 10`, printed from the integer tenths (the same IEEE operations). */
function elapsedS(ms: number): string {
  const t = Math.max(0, Math.floor(ms / 100 + 0.5));
  return `${Math.floor(t / 10)}.${t % 10}`;
}
const limitPct = (limit: number) => String(limit * 100);

// ---- the page --------------------------------------------------------------------------------------

async function showTab(key: 'acoustics' | 'console' | 'runs'): Promise<void> {
  await clickSelector(`[data-dock-tab="${key}"]`);
  await browser.waitUntil(
    async () => (await browser.execute(() => document.querySelector('.dock-body')?.getAttribute('data-dock-panel'))) === key,
    { timeout: 10_000, timeoutMsg: `the ${key} tab did not show` },
  );
}

const count = (selector: string) => browser.execute((s: string) => document.querySelectorAll(s).length, selector);
const textOf = (selector: string) =>
  browser.execute((s: string) => document.querySelector(s)?.textContent ?? null, selector);
const attrOf = (selector: string, name: string) =>
  browser.execute((s: string, n: string) => document.querySelector(s)?.getAttribute(n) ?? null, selector, name);

async function rowShown(run: string): Promise<string> {
  const sel = `[data-run-row="${run}"]`;
  await $(sel).waitForExist({ timeout: 30_000, timeoutMsg: `no Runs row ${run}` });
  return sel;
}

interface Scroll {
  top: number;
  height: number;
  client: number;
  atBottom: boolean;
}
const consoleScroll = () => hook<Scroll | null>('consoleScroll');
// The scroll event a scrollTop write raises is dispatched on the next frame. During a live run a
// render (a PROGRESS batch) can land in between, and the pane, not yet told, puts the Console
// back at the bottom: m11-dock-live failed once so (C4 run 20261007-154608, top 2122 = the
// bottom) and passed on the rerun (20261007-155028). The event is dispatched here in the same
// task, so the pane hears of the scroll before any render can run. Backlog 95 owns the app side.
const scrollConsole = (to: 'top' | 'bottom') =>
  browser.execute((where: string) => {
    const el = document.querySelector<HTMLElement>('[data-dock-panel="console"]');
    if (!el) return;
    el.scrollTop = where === 'top' ? 0 : el.scrollHeight;
    el.dispatchEvent(new Event('scroll'));
  }, to);

/** One FAIL line in the Console: a results check of a run that does not exist (RUN_NOT_FOUND,
 * logged by actions.ts with its code; the hook itself only sees the rejection). */
async function addFailLine(): Promise<void> {
  await assert.rejects(hook('resultsState', 'no-such-run'));
}

/** The FAIL badge must equal the Console's FAIL lines, read with the Console shown. */
async function assertFailBadge(): Promise<number> {
  await showTab('console');
  const fails = await count('.console-line.FAIL');
  const badge = await textOf('[data-dock-tab="console"] [data-part="fail-count"]');
  assert.equal(badge, fails > 0 ? `${fails} fail` : null, `the badge must count the Console's ${fails} FAIL lines`);
  return fails;
}

/** Every stream log under `dir`, at any depth: TetGen's, its diagnostic rerun's and
 * preprocess.exe's all feed a run's mesh lines. */
function streamLogs(dir: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { recursive: true, encoding: 'utf8' })
    .filter((f) => f.endsWith('.stdout.txt') || f.endsWith('.stderr.txt'))
    .map((f) => path.join(dir, f));
}

/** Every line of the run's logs for `source`, trailing whitespace and line ends dropped. */
function logLines(runFolder: string, source: string): Set<string> {
  const files =
    source === 'mesh'
      ? streamLogs(path.join(runFolder, 'mesh'))
      : ['solver.stdout.txt', 'solver.stderr.txt'].map((f) => path.join(runFolder, f)).filter((p) => existsSync(p));
  const out = new Set<string>();
  for (const p of files) for (const l of readFileSync(p, 'utf8').split(/\r?\n/)) out.add(l.trimEnd());
  return out;
}

/** The run folder of `run` among the gate's projects. */
function findRunFolder(run: string): string | null {
  for (const p of readdirSync(env('M11_P'))) {
    const d = path.join(env('M11_P'), p, 'runs', run);
    if (existsSync(d)) return d;
  }
  return null;
}

describe('M11 dock', () => {
  before(async () => {
    await waitForHooks(['idle', 'openProject', 'runStart', 'runCancel', 'waitRun', 'runLog', 'runsRows', 'dockTab', 'consoleScroll']);
    const status = await hook<{ blockers: string[] }>('solversStatus');
    assert.deepEqual(status.blockers, [], 'the gate stages the verified solvers');
  });

  it('m11-dock-interrupted: a run folder with no run.json lists as Interrupted, with its reason, and a click selects it', async () => {
    const folder = runDir(LONG(), INTERRUPTED);
    assert.ok(existsSync(folder), `m11.ps1 makes the control folder ${folder}`);
    assert.ok(!existsSync(path.join(folder, 'run.json')), 'the control has no run.json');
    await m10.openProject(LONG());
    await showTab('runs');
    const sel = await rowShown(INTERRUPTED);
    assert.equal(await attrOf(sel, 'data-status'), 'INTERRUPTED');
    assert.equal(await $(`${sel} [data-part="status"]`).getText(), 'Interrupted');
    const reason = await $(`${sel} [data-reason-code="RESULTS_MANIFEST_MISSING"]`).getText();
    assert.ok(reason.includes('results_manifest_missing'), `the reason shows its core code: ${reason}`);
    const text = await $(sel).getText();
    console.log(`m11-dock-interrupted receipt: ${JSON.stringify(text)}`);
    assert.ok(text.includes('no run.json'), text);
    // It was never guessed: no variant, no warnings count, no solver time.
    assert.equal(await count(`${sel} [data-part="warnings"], ${sel} [data-part="elapsed"]`), 0);
    // A click selects it and opens its record, which says what is not recorded.
    await $(sel).click();
    await browser.waitUntil(async () => (await attrOf(sel, 'aria-selected')) === 'true', { timeout: 10_000 });
    assert.equal(await textOf(`${sel} [data-part="exit"]`), 'no exit code');
    assert.equal(await attrOf(`${sel} [data-part="verified"]`, 'data-verified'), 'unrecorded');
    // The Runs badge counts the rows listed.
    const rows = await hook<Row[]>('runsRows');
    assert.ok(rows.some((r) => r.run === INTERRUPTED && r.status === 'INTERRUPTED'));
    await browser.waitUntil(
      async () => (await textOf('[data-dock-tab="runs"] [data-part="runs-count"]')) === String(rows.length),
      { timeout: 10_000, timeoutMsg: `the Runs badge does not read ${rows.length}` },
    );
  });

  it('m11-dock-live: the Console reads live and shows one PROGRESS line during a run, follows the bottom, stays put when scrolled up; Cancel lists Cancelled', async () => {
    await m10.openProject(LONG());
    await showTab('console');
    // Enough lines that the Console scrolls, each a FAIL line the badge must count.
    for (let i = 0; i < 25; i++) await addFailLine();
    const before = await assertFailBadge();
    const notFound = (await consoleLines('FAIL')).filter((t) => t.includes('RUN_NOT_FOUND')).length;
    assert.ok(before >= 25 && notFound >= 25, `25 FAIL lines with their code: ${notFound} of ${before}`);
    const s0 = await consoleScroll();
    assert.ok(s0 && s0.height > s0.client, `the Console must overflow for the scroll checks: ${JSON.stringify(s0)}`);

    await hook('runStart', 'spps');
    const name = await hook<string>('waitRun', null, 'progress', 180_000);
    const spps = path.join(env('M11_SOLVERS'), 'spps.exe');
    assert.ok(processesFrom(spps).length > 0, 'the private spps.exe is solving');
    await browser.waitUntil(async () => (await textOf('[data-dock-tab="console"] [data-part="live"]')) === 'live', {
      timeout: 10_000,
      timeoutMsg: 'the Console badge does not read live during the run',
    });
    assert.equal(await count('[data-dock-tab="console"] [data-part="fail-count"]'), 0, 'live replaces the fail count');

    // One PROGRESS line, however many arrive: sampled while the counts grow.
    const samples: { lines: number; text: string | null; counted: number }[] = [];
    for (let i = 0; i < 8; i++) {
      const counted = (await hook<{ counts: Record<string, number> }>('runLog', name)).counts.PROGRESS;
      samples.push({
        lines: await count('.console-line.PROGRESS'),
        text: await textOf(`[data-progress-line="${name}"] [data-verbatim="${name}:solver"]`),
        counted,
      });
      await browser.pause(250);
    }
    console.log(`m11-dock-live receipt: run ${name}; PROGRESS samples ${JSON.stringify(samples)}`);
    assert.ok(samples.every((s) => s.lines === 1), 'exactly one PROGRESS line is rendered');
    assert.ok(samples.every((s) => s.text !== null && /^#[0-9.eE+-]+$/.test(s.text)), 'the live line is SPPS\'s own # line');
    assert.ok(samples[samples.length - 1].counted > 1, 'more than one PROGRESS line was received (the control)');
    const strip = Number(await textOf(`[data-run-counts="${name}"] [data-count="PROGRESS"]`));
    assert.ok(strip >= samples[samples.length - 1].counted, 'the counts strip counts the PROGRESS lines received');

    // Follows the bottom while pinned.
    await browser.waitUntil(async () => (await consoleScroll())?.atBottom === true, {
      timeout: 5_000,
      timeoutMsg: 'the Console did not follow the bottom during the run',
    });
    // Scrolled up: a new line and more batches do not move it.
    await scrollConsole('top');
    await browser.pause(300);
    const counted0 = (await hook<{ counts: Record<string, number> }>('runLog', name)).counts.PROGRESS;
    await addFailLine();
    await browser.pause(1_000);
    const up = await consoleScroll();
    const counted1 = (await hook<{ counts: Record<string, number> }>('runLog', name)).counts.PROGRESS;
    assert.equal(up?.top, 0, `scrolled up, the Console must stay put: ${JSON.stringify(up)}`);
    assert.ok(counted1 > counted0, 'batches kept arriving while it stayed put (the control)');
    // Back at the bottom: it follows again.
    await scrollConsole('bottom');
    await browser.pause(300);
    await addFailLine();
    await browser.waitUntil(async () => (await consoleScroll())?.atBottom === true, {
      timeout: 5_000,
      timeoutMsg: 'back at the bottom, the Console did not follow a new line',
    });
    assert.equal(await textOf('[data-dock-tab="console"] [data-part="live"]'), 'live');

    // Cancel: the row reads Cancelled, the live line goes, the badge counts FAIL lines again.
    assert.equal(await hook<boolean>('runCancel'), true);
    await hook('waitRun', name, 'ended', 60_000);
    await browser.waitUntil(async () => processesFrom(spps).length === 0, {
      timeout: 5_000,
      timeoutMsg: 'spps.exe still runs from the private copy after Cancel',
    });
    const m = manifest(LONG(), name);
    assert.equal(m.verdict.status, 'CANCELLED');
    await showTab('runs');
    const sel = await rowShown(name);
    assert.equal(await attrOf(sel, 'data-status'), 'CANCELLED');
    assert.equal(await $(`${sel} [data-part="status"]`).getText(), 'Cancelled');
    await showTab('console');
    assert.equal(await count('.console-line.PROGRESS'), 0, 'no PROGRESS line once the run ended');
    for (const c of CLASSES) {
      assert.equal(
        Number(await textOf(`[data-run-counts="${name}"] [data-count="${c}"]`)),
        m.lines[c.toLowerCase() as keyof Manifest['lines']],
        `${c}: the Console's count equals run.json's`,
      );
    }
    assert.equal(await attrOf(`[data-run-counts="${name}"] [data-part="counts-check"]`, 'data-match'), 'true');
    const after = await assertFailBadge();
    assert.ok(after >= before + 2, `the two FAIL lines added during the run are counted: ${before} then ${after}`);
    // The live line moved with the run (M11 review F3): each sample above the one before. A line
    // frozen at the run's first '#' passed the checks on the samples above (the review's mutation
    // M23). Judged after Cancel, so a failure here leaves no run going.
    for (let i = 1; i < samples.length; i++) {
      const [now, was] = [samples[i].text ?? '', samples[i - 1].text ?? ''];
      assert.equal(decimalAbove(now.slice(1), was.slice(1)), true, `the live PROGRESS line did not move: ${samples.map((s) => s.text).join(', ')}`);
    }
  });

  it('m11-dock-row: an OK run\'s row and record equal its run.json, and the Console\'s counts strip equals run.json', async () => {
    await m10.openProject(BOX());
    await hook('runStart', 'spps');
    const name = await hook<string>('waitRun', null, 'ended', 180_000);
    const m = manifest(BOX(), name);
    assert.equal(m.verdict.status, 'OK');
    const bands = m.particles?.bands ?? [];
    assert.ok(bands.length > 0, 'SPPS wrote its statistics (the control)');
    assert.ok(m.lines.progress > 0 && m.lines.ok === 1, `trivial counts: ${JSON.stringify(m.lines)}`);

    await showTab('runs');
    const sel = await rowShown(name);
    await $(sel).click();
    await browser.waitUntil(async () => (await attrOf(sel, 'aria-selected')) === 'true', { timeout: 10_000 });
    assert.equal(await attrOf(sel, 'data-status'), 'OK');
    assert.equal(await $(`${sel} [data-part="status"]`).getText(), 'OK');
    assert.equal(await count(`${sel} [data-reason-code]`), 0, 'an OK row has no reason code');

    const worst = worstPct(bands);
    assert.equal(await $(`${sel} [data-part="loss"]`).getText(), `Particles lost ${worst} %`);
    assert.equal(await textOf(`${sel} [data-part="loss-limit"] [data-diagnostic="loss_limit_pct"]`), `${limitPct(m.loss_limit)} %`);
    assert.equal(await count(`${sel} [data-part="band-loss"]`), bands.length);
    for (const b of bands) {
      const shown = await textOf(`${sel} [data-part="band-loss"][data-band="${b.freq_hz}"] [data-diagnostic="loss_pct"]`);
      assert.equal(shown, `${bandPct(b)} %`, `${b.freq_hz} Hz: ${bandLost(b)} of ${b.total}`);
    }
    assert.ok(m.outcome, 'an OK run has its outcome');
    assert.equal(await textOf(`${sel} [data-part="elapsed"]`), `${elapsedS(m.outcome.elapsed_ms)} s`);
    assert.equal(await textOf(`${sel} [data-part="exit"]`), `exit ${m.outcome.exit_code}`);

    // The hashes: the exe's sha256 prefix, whole in the title, and the verified mark from `solvers`.
    assert.equal(await textOf(`${sel} [data-part="exe-sha"]`), m.exe.sha256.slice(0, 12));
    assert.equal(await attrOf(`${sel} [data-part="exe-sha"]`, 'title'), m.exe.sha256);
    assert.ok(Array.isArray(m.solvers) && m.solvers.length > 0, 'the app verifies its solvers and records them (C7)');
    const verified = m.solvers.every((c) => c.matches) ? 'yes' : 'no';
    assert.equal(await attrOf(`${sel} [data-part="verified"]`, 'data-verified'), verified);
    assert.equal(await attrOf(`${sel} [data-part="mesh-sha"]`, 'data-sha256'), m.mesh?.mbin_sha256);
    for (const c of CLASSES) {
      assert.equal(
        Number(await textOf(`${sel} [data-manifest-count="${c}"]`)),
        m.lines[c.toLowerCase() as keyof Manifest['lines']],
        `run.json lines ${c}`,
      );
    }
    const receipt = await $(sel).getText();
    console.log(`m11-dock-row receipt: ${name}: ${JSON.stringify(receipt)}`);

    // The Console: the counts strip, the stream's own counts, and run.json agree.
    await showTab('console');
    const log = await hook<{ counts: Record<string, number>; dupes: number; gaps: number }>('runLog', name);
    for (const c of CLASSES) {
      const want = m.lines[c.toLowerCase() as keyof Manifest['lines']];
      assert.equal(Number(await textOf(`[data-run-counts="${name}"] [data-count="${c}"]`)), want, `strip ${c}`);
      assert.equal(log.counts[c], want, `runLog ${c}`);
    }
    assert.deepEqual([log.dupes, log.gaps], [0, 0]);
    assert.equal(await attrOf(`[data-run-counts="${name}"] [data-part="counts-check"]`, 'data-match'), 'true');
    // A class word is coloured only above 0 (M11 review B.10): this run's FAIL 0 is as grey as
    // INFO, which has no colour of its own, and its OK 1 is not (the control).
    const colours = await browser.execute(
      (run: string) =>
        Object.fromEntries(
          ['FAIL', 'INFO', 'OK'].map((c) => {
            const el = document.querySelector(`[data-run-counts="${run}"] .count.${c} .k`);
            return [c, el ? getComputedStyle(el).color : null];
          }),
        ) as Record<string, string | null>,
      name,
    );
    console.log(`m11-dock-row receipt: counts strip class colours ${JSON.stringify(colours)}; run.json lines ${JSON.stringify(m.lines)}`);
    assert.equal(m.lines.fail, 0);
    assert.ok(colours.FAIL && colours.INFO && colours.OK, 'the strip has its class words');
    assert.equal(colours.FAIL, colours.INFO, 'FAIL 0 is not coloured');
    assert.notEqual(colours.OK, colours.INFO, 'OK 1 keeps its colour');
    // Verbatim: every solver and TetGen line shown is a line of the run's own logs.
    const verbatim = await browser.execute(
      (run: string) =>
        [...document.querySelectorAll(`[data-verbatim^="${run}:"]`)].map((e) => ({
          source: (e.getAttribute('data-verbatim') ?? '').split(':').pop() ?? '',
          text: e.textContent ?? '',
        })),
      name,
    );
    assert.ok(verbatim.length > 0, 'the run shows its solver and TetGen lines');
    const folder = runDir(BOX(), name);
    for (const v of verbatim) {
      assert.ok(logLines(folder, v.source).has(v.text.trimEnd()), `not in the ${v.source} logs: ${JSON.stringify(v.text)}`);
    }
    // Every Console line of the run carries its run and source.
    assert.equal(await count(`.console-line[data-run="${name}"]:not([data-source])`), 0);

    // The control that lets the loss checks above fail (M11 review F1): the box run loses 0
    // particles, so a row that printed a constant 0.00 passed them. The planted-loss run
    // (m11.ps1) carries a known loss in four of its six bands, the worst at 500 Hz, not the first.
    const plantedRuns = readdirSync(path.join(path.dirname(LOSS()), 'runs'));
    assert.equal(plantedRuns.length, 1, `m11.ps1 makes the one planted-loss run: ${plantedRuns.join(', ')}`);
    const [planted] = plantedRuns;
    const pm = manifest(LOSS(), planted);
    const pbands = pm.particles?.bands ?? [];
    assert.equal(worstPct(pbands), PLANTED.worst_pct, "run.json's worst band is the plant");
    await m10.openProject(LOSS());
    await showTab('runs');
    const psel = await rowShown(planted);
    await $(psel).click();
    await browser.waitUntil(async () => (await attrOf(psel, 'aria-selected')) === 'true', { timeout: 10_000 });
    assert.equal(await $(`${psel} [data-part="loss"]`).getText(), `Particles lost ${worstPct(pbands)} %`);
    assert.equal(await $(`${psel} [data-part="loss"]`).getText(), 'Particles lost 0.82 %');
    assert.equal((await $(`${psel} [data-part="worst-band"]`).getText()).trim(), `at ${PLANTED.worst_band_hz} Hz`);
    const perBand: string[] = [];
    for (const b of pbands) {
      const cell = `${psel} [data-part="band-loss"][data-band="${b.freq_hz}"]`;
      const shownPct = await textOf(`${cell} [data-diagnostic="loss_pct"]`);
      const shownCount = await textOf(`${cell} .mono`);
      perBand.push(`${b.freq_hz} Hz ${shownCount} ${shownPct}`);
      assert.equal(shownPct, `${bandPct(b)} %`, `${b.freq_hz} Hz: ${bandLost(b)} of ${b.total}`);
      assert.equal(shownCount, `${bandLost(b)} of ${b.total}`, `${b.freq_hz} Hz: the count shown`);
      const [lost, pct] = PLANTED.bands[b.freq_hz];
      assert.deepEqual([shownCount, shownPct], [`${lost} of ${PLANTED.total}`, `${pct} %`], `${b.freq_hz} Hz: the hand-worked plant`);
    }
    console.log(`m11-dock-row receipt: planted-loss run ${planted}: ${perBand.join('; ')}`);
    assert.ok(perBand.some((t) => !t.endsWith(' 0.00 %')), 'a band shows a non-zero loss');
  });

  it('m11-dock-meshfail: the forced mesh failure reads FAIL with MESH_TETGEN_SKIPPED and tetgen_skipped_facets', async () => {
    const runs = readdirSync(path.join(path.dirname(MESHFAIL()), 'runs')).filter((r) =>
      existsSync(path.join(runDir(MESHFAIL(), r), 'run.json')),
    );
    const name = runs.find((r) => manifest(MESHFAIL(), r).verdict.reasons.some((x) => x.code === 'tetgen_skipped_facets'));
    assert.ok(name, `m11.ps1 makes the mesh-failure run: ${runs.join(', ')}`);
    const m = manifest(MESHFAIL(), name);
    assert.equal(m.stage, 'mesh');
    await m10.openProject(MESHFAIL());
    await showTab('runs');
    const sel = await rowShown(name);
    assert.equal(await attrOf(sel, 'data-status'), 'FAIL');
    assert.equal(await $(`${sel} [data-part="status"]`).getText(), 'FAIL');
    const code = await $(`${sel} [data-reason-code="MESH_TETGEN_SKIPPED"]`).getText();
    assert.ok(code.includes('tetgen_skipped_facets'), code);
    assert.equal(await count(`${sel} [data-reason-code]`), m.verdict.reasons.length, 'every reason is listed');
    await $(sel).click();
    await browser.waitUntil(async () => (await attrOf(sel, 'aria-selected')) === 'true', { timeout: 10_000 });
    // A command-line run, refused before it reached the solver: backlog 54's CLI half checks by
    // default, so its solvers key is recorded and matching (the stand-in tetgen.exe verifies
    // through m11.ps1's SIMPA_SOLVER_MANIFEST override, under the name `tetgen.exe`). But that
    // override registers the stand-in's own code sha256, so "every check matches" proves nothing:
    // M8b's fix makes `solver_manifest.source: "override"` force unverified regardless, so the
    // row must read 'no', never 'yes' (the hole a wrong "verified" would otherwise reach a user
    // through).
    assert.ok(Array.isArray(m.solvers) && m.solvers.length > 0, 'solvers is recorded');
    assert.ok(
      (m.solvers as { matches: boolean }[]).every((c) => c.matches),
      `every check matches: ${JSON.stringify(m.solvers)}`,
    );
    assert.equal(m.solver_manifest?.source, 'override', 'the run was checked against the override manifest');
    assert.equal(await attrOf(`${sel} [data-part="verified"]`, 'data-verified'), 'no');
    assert.equal(await attrOf(`${sel} [data-part="verified"]`, 'data-build-code'), 'SOLVER_MANIFEST_OVERRIDE');
    const verifiedText = await textOf(`${sel} [data-part="verified"]`);
    assert.ok(verifiedText?.includes('solver_manifest_override'), verifiedText ?? undefined);
    assert.equal(await textOf(`${sel} [data-part="exe-sha"]`), m.exe.sha256.slice(0, 12));
    assert.equal(await count(`${sel} [data-part="loss"], ${sel} [data-part="bands"]`), 0, 'no particle statistics');
    assert.equal(await textOf(`${sel} [data-part="exit"]`), 'no exit code');
    console.log(`m11-dock-meshfail receipt: ${name}: ${JSON.stringify(await $(sel).getText())}`);
  });

  it('m11-dock-h: in the dock every diagnostic passes its grammar and equals run.json, and no other number carries a unit', async () => {
    const GRAMMAR: Record<string, RegExp> = {
      loss_pct: /^\d{1,3}\.\d{2} %$/,
      loss_limit_pct: /^\d+(\.\d+)? %$/,
      elapsed_s: /^\d+\.\d s$/,
      progress_pct: /^\d{1,3}(\.\d{1,2})? %$/,
    };
    interface Diag {
      field: string;
      run: string;
      band: string | null;
      text: string;
    }
    /** The dock's diagnostics, and its text with them and the verbatim lines hidden. */
    const scan = () =>
      browser.execute(() => {
        const dock = document.querySelector<HTMLElement>('.dock');
        if (!dock) return null;
        const diags = [...dock.querySelectorAll<HTMLElement>('[data-diagnostic]')].map((e) => ({
          field: e.getAttribute('data-diagnostic') ?? '',
          run: e.getAttribute('data-run') ?? '',
          band: e.getAttribute('data-band'),
          text: e.textContent ?? '',
        }));
        const whole = dock.innerText;
        const hidden = [...dock.querySelectorAll<HTMLElement>('[data-diagnostic], [data-verbatim]')];
        const before = hidden.map((e) => e.style.display);
        hidden.forEach((e) => (e.style.display = 'none'));
        try {
          return { diags, rest: dock.innerText, whole };
        } finally {
          hidden.forEach((e, i) => (e.style.display = before[i]));
        }
      });
    const check = (where: string, s: { diags: Diag[]; rest: string; whole: string }): string[] => {
      const bad: string[] = [];
      for (const d of s.diags) {
        const g = GRAMMAR[d.field];
        if (!g) {
          bad.push(`${where}: unknown diagnostic field ${d.field}`);
          continue;
        }
        if (!g.test(d.text)) bad.push(`${where}: ${d.field} "${d.text}" breaks its grammar`);
        const folder = findRunFolder(d.run);
        if (!folder || !existsSync(path.join(folder, 'run.json'))) {
          bad.push(`${where}: ${d.field} "${d.text}" names run ${d.run} with no run.json`);
          continue;
        }
        const m: Manifest = JSON.parse(readFileSync(path.join(folder, 'run.json'), 'utf8'));
        const bands = m.particles?.bands ?? [];
        if (d.field === 'progress_pct') {
          bad.push(`${where}: a progress diagnostic with no run active`);
          continue;
        }
        let want: string | null = null;
        if (d.field === 'loss_pct') {
          const band = d.band === null ? null : bands.find((b) => String(b.freq_hz) === d.band);
          if (d.band === null) want = bands.length ? `${worstPct(bands)} %` : null;
          else if (band) want = `${bandPct(band)} %`;
        } else if (d.field === 'loss_limit_pct') want = `${limitPct(m.loss_limit)} %`;
        else if (d.field === 'elapsed_s') want = m.outcome ? `${elapsedS(m.outcome.elapsed_ms)} s` : null;
        if (d.text !== want) bad.push(`${where}: ${d.field} "${d.text}" is not run.json's "${want}"`);
      }
      const unit = s.rest.match(ACOUSTIC_NUMBER);
      if (unit) bad.push(`${where}: "${unit[0]}" outside a proven diagnostic`);
      const param = s.whole.match(PARAMETER_NUMBER);
      if (param) bad.push(`${where}: "${param[0]}" names a parameter with a number`);
      return bad;
    };

    // Say-NO first: a planted unit number, and a planted diagnostic with a wrong value, are caught.
    await m10.openProject(BOX());
    await showTab('runs');
    const rows = await hook<Row[]>('runsRows');
    const ok = rows.find((r) => r.status === 'OK');
    assert.ok(ok, 'the box has an OK run (m11-dock-row made one)');
    // The same view clean first, so each flag below is the plant's and nothing else's.
    const clean = await scan();
    assert.ok(clean, 'the dock is shown');
    assert.deepEqual(check('say-no, before planting', clean), []);
    for (const plant of ['<span>T30 1.8 s</span>', `<span data-diagnostic="elapsed_s" data-run="${ok.run}">999.9 s</span>`]) {
      await browser.execute((html: string) => {
        const host = document.createElement('div');
        host.setAttribute('data-planted', '');
        host.innerHTML = html;
        document.querySelector('.dock-body')?.appendChild(host);
      }, plant);
      const s = await scan();
      await browser.execute(() => document.querySelectorAll('[data-planted]').forEach((e) => e.remove()));
      assert.ok(s, 'the dock is shown');
      assert.notDeepEqual(check('say-no', s), [], `the checker must flag ${plant}`);
    }

    const seen: string[] = [];
    // LOSS: the planted-loss run, whose loss spans are not 0.00 (M11 review F1).
    for (const project of [BOX(), MESHFAIL(), LONG(), LOSS()]) {
      await m10.openProject(project);
      await showTab('runs');
      const listed = await hook<Row[]>('runsRows');
      // Open every row in turn: only the selected one shows its record.
      for (const r of listed) {
        const sel = await rowShown(r.run);
        await $(sel).click();
        await browser.waitUntil(async () => (await attrOf(sel, 'aria-selected')) === 'true', { timeout: 10_000 });
        const s = await scan();
        assert.ok(s);
        assert.deepEqual(check(`${path.basename(path.dirname(project))}/runs/${r.run}`, s), []);
        seen.push(`runs:${r.run}`);
      }
      await showTab('console');
      const s = await scan();
      assert.ok(s);
      assert.deepEqual(check(`${path.basename(path.dirname(project))}/console`, s), []);
      seen.push(`console:${path.basename(path.dirname(project))}`);
    }
    console.log(`m11-dock-h receipt: ${seen.length} views scanned: ${seen.join(', ')}`);
    assert.ok(seen.some((v) => v.startsWith('runs:')), 'at least one row was scanned');
  });
});
