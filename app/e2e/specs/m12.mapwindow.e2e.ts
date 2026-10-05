// The map's time window (Burhan 2026-10-05 01:59; app/ui/src/features/viewport/window.ts): each
// face's value at a step is the mean of the file's own per-step values over the last w steps up to
// it (a gap as 0, a non-finite record as 0, float32 adds in step order, over the steps held). Each
// id with its control:
//   mw-default   a 1 ms run opens at 10 ms: the map is drawn with 10 steps and the legend's title
//                says "averaged over 10 ms"; the GPU's drawn values (the draw's own `faceValue`)
//                at sampled cells equal the window mean computed here from the `.csbin` by this
//                spec's own reader, to float32 precision. Controls: one sampled cell has no record
//                at its step (the window fills a gap the raw map leaves blank), and a step's blank
//                faces under the window are no more than the raw map's (fewer where it was patchy)
//   mw-off       Off draws the raw step: the drawn values are the file's records bit for bit, a
//                cell with no record draws nothing, and the legend says nothing of a window; 50 ms
//                takes 50 steps and its values equal the 50-step mean
//   mw-probe     the probe over a face shows the window mean's bits and level, and says it is a
//                mean over the window, with the steps it holds
//   mw-cumulative say NO: the cumulative map takes no window (the chips are disabled, the panel
//                says why) and its texels are the running sums bit for bit; cumulative off, the
//                window comes back
//   mw-diff      the difference averages both runs over the same window: drawn dB equal the
//                levels of the two files' window means subtracted; control: a cell that has no
//                difference at the raw step (one run has no record there) has one under the window
//
// The box: tests/fixtures/rooms/outputs_box.simpa at a 1 ms step and 0.5 s (500 steps, as CR4's
// 1 ms), copied into this spec's own folder on C: (M11_P), run twice (seeds 1 and 2) from the app
// with particles saved 0. No screenshots.
import { strict as assert } from 'node:assert';
import { copyFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { clickSelector, RESULTS_STEP_CURRENT } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { f32, level, readCsbin, type Csbin } from '../lib/m12files.ts';
import { env } from '../lib/types.ts';

const HOOKS = ['idle', 'openProject', 'edit', 'projectJson', 'runStart', 'runState', 'runsRows', 'selectRun', 'setStep', 'm12Map', 'm12Texels', 'm12DrawnTexels', 'm12DiffTexels', 'm12SetStep', 'wowMapFacePoint'];

interface MapState {
  run: string;
  path: string;
  bandHz: number | null;
  kind: string;
  cumulative: boolean;
  windowSteps: number;
  faces: number;
  steps: number;
  baseline: string | null;
  legend: { title: string } | null;
  error: string | null;
}

const shown = (s: string | null) => (s ?? '').replace(/\s+/g, ' ').trim();
const bitsOf = (v: number) => new Uint32Array(new Float32Array([v]).buffer)[0];
const fr = Math.fround;

async function mapOf(run: string, want: (m: MapState) => boolean, what: string): Promise<MapState> {
  let last: MapState | null = null;
  await browser.waitUntil(async () => (last = await hook<MapState | null>('m12Map')) !== null && last.run === run && last.error === null && want(last), {
    timeout: 60_000,
    interval: 250,
    timeoutMsg: `the map did not show ${what}: ${JSON.stringify(last)}`,
  });
  return last as unknown as MapState;
}

/** Each face's value per step as the file holds it (the last record of a step; 0 for none), from the bytes on disk. */
function perStep(c: Csbin): number[][] {
  return c.faces.map((f) => {
    const e = new Array<number>(c.timeSteps).fill(0);
    for (const [s, b] of f.records) if (s < c.timeSteps) e[s] = f32(b);
    return e;
  });
}

/** The window's rule, written from the rule and not the app: float32 adds over max(0, t - w + 1) .. t, a non-finite one as 0, over the steps held. */
function mean(series: number[], t: number, w: number): number {
  const from = Math.max(0, t - w + 1);
  let s = 0;
  for (let k = from; k <= t; k++) s = fr(s + (Number.isFinite(series[k]) ? series[k] : 0));
  return fr(s / (t - from + 1));
}

/** Within float32 precision (the GPU's divide is not correctly rounded): 1e-6 relative. */
const near = (got: number, want: number) => (want === 0 ? got === 0 : Math.abs(got - want) <= 1e-6 * Math.abs(want));

async function hover(p: { x: number; y: number }): Promise<void> {
  await browser.action('pointer', { parameters: { pointerType: 'mouse' } }).move({ x: p.x, y: p.y, origin: 'viewport', duration: 0 }).perform();
}

describe('The map\'s time window', () => {
  let runsRoot = '';
  let runA = '';
  let runB = '';
  let file: Csbin;
  let series: number[][] = [];
  let mapPath = '';

  async function runWith(seed: number): Promise<string> {
    const p = JSON.parse(await m10.projectJson()) as { solvers: { spps: Record<string, unknown> } };
    p.solvers.spps.particles_saved = 0;
    p.solvers.spps.random_seed = seed;
    p.solvers.spps.time_step_s = 0.001;
    p.solvers.spps.duration_s = 0.5;
    const r = await m10.edit({ op: 'set_solver_settings', settings: p.solvers });
    assert.equal(r.applied, true, `settings refused: ${JSON.stringify(r.refusals)}`);
    const before = new Set((await hook<{ run: string }[]>('runsRows')).map((x) => x.run));
    await hook('runStart', 'spps');
    let fresh: { run: string; status: string } | undefined;
    await browser.waitUntil(
      async () => {
        if ((await hook<unknown>('runState')) !== null) return false;
        fresh = (await hook<{ run: string; status: string }[]>('runsRows')).find((x) => !before.has(x.run) && x.status !== 'RUNNING');
        return fresh !== undefined;
      },
      { timeout: 300_000, interval: 250, timeoutMsg: 'the run did not end within 300 s' },
    );
    await m10.idle();
    const f = fresh as unknown as { run: string; status: string };
    assert.equal(f.status, 'OK', `run ${f.run} ended ${f.status}`);
    return f.run;
  }

  before(async () => {
    await waitForHooks(HOOKS);
    const dir = path.join(env('M11_P'), 'm12-mapwindow');
    mkdirSync(dir, { recursive: true });
    const box = path.join(dir, 'outputs_box.simpa');
    copyFileSync(path.join(env('M11_REPO'), 'tests', 'fixtures', 'rooms', 'outputs_box.simpa'), box);
    runsRoot = path.join(dir, 'runs');
    await m10.openProject(box);
    runA = await runWith(1);
    runB = await runWith(2);
    await hook('selectRun', runB);
    await m10.setStep('results');
    await $(RESULTS_STEP_CURRENT).waitForExist({ timeout: 30_000 });
    console.log(`receipt mw: runs ${runA} (baseline), ${runB} under ${runsRoot}`);
  });

  /** Cells (face, step) with a window mean > 0, at step `minT` or later: up to `n`, the first one with no record at its step. */
  function picks(w: number, n: number, minT = 0): [number, number][] {
    const out: [number, number][] = [];
    for (let face = 0; face < series.length && out.length < n; face++) {
      const e = series[face];
      const first = e.findIndex((v) => v > 0);
      if (first < 0) continue;
      for (let t = Math.max(first + 3, minT); t < Math.min(e.length, Math.max(first, minT) + 120) && out.length < n; t++) {
        const gap = !(e[t] > 0);
        if (mean(e, t, w) > 0 && (out.length > 0 || gap) && !out.some(([f]) => f === face)) out.push([face, t]);
      }
    }
    return out;
  }

  it('mw-default: a 1 ms run opens averaged over 10 ms; the drawn values are the file\'s 10-step means', async () => {
    const m = await mapOf(runB, (x) => x.bandHz === 1000 && x.kind === 'level' && !x.cumulative, 'the 1 kHz map');
    mapPath = m.path;
    assert.equal(m.windowSteps, 10, 'the default window is 10 ms, 10 steps of 1 ms');
    file = readCsbin(path.join(runsRoot, runB, 'solve', ...m.path.split('/')));
    assert.equal(file.timeSteps, 500);
    series = perStep(file);
    const cells = picks(10, 4);
    assert.equal(cells.length, 4, 'four cells with energy in their window');
    assert.ok(!(series[cells[0][0]][cells[0][1]] > 0), 'the control: the first cell has no record at its step');
    const got = await hook<number[]>('m12DrawnTexels', cells);
    let exact = 0;
    for (const [i, [face, t]] of cells.entries()) {
      const want = mean(series[face], t, 10);
      const g = f32(got[i]);
      if ((got[i] >>> 0) === bitsOf(want)) exact++;
      console.log(`receipt mw-default: face ${face} step ${t}: GPU ${g.toExponential(6)}, file's 10-step mean ${want.toExponential(6)} (${(level(want) as number).toFixed(2)} dB), the step's own value ${series[face][t] > 0 ? series[face][t].toExponential(4) : 'none'}`);
      assert.ok(near(g, want), `face ${face} step ${t}: ${g} vs ${want}`);
    }
    console.log(`receipt mw-default: ${exact} of ${cells.length} bit for bit, all within 1e-6 relative`);
    // The patchiness: at one step, the faces drawn blank with and without the window.
    const t = cells[1][1];
    const all: [number, number][] = series.map((_, f) => [f, t]);
    const drawn = (await hook<number[]>('m12DrawnTexels', all)).map((b) => f32(b));
    const blankWin = drawn.filter((v) => !(v > 0)).length;
    const blankRaw = series.filter((e) => !(e[t] > 0)).length;
    const blankWant = series.filter((e) => !(mean(e, t, 10) > 0)).length;
    console.log(`receipt mw-default: step ${t}: ${blankRaw} of ${series.length} faces blank raw, ${blankWin} under 10 ms (the rule says ${blankWant})`);
    assert.equal(blankWin, blankWant);
    assert.ok(blankWin <= blankRaw);
    const title = shown(await $('[data-part="map-legend"] .vp-legend-title').getText());
    console.log(`receipt mw-default: legend "${title}"`);
    assert.match(title, /averaged over 10 ms/);
    assert.equal(await $('[data-map-window="10"]').getAttribute('aria-checked'), 'true');
  });

  it('mw-off: Off draws the raw step bit for bit; 50 ms takes 50 steps', async () => {
    await clickSelector('[data-map-window="0"]');
    await mapOf(runB, (x) => x.windowSteps === 1 && x.path === mapPath, 'the map with no window');
    const face = file.faces.findIndex((f) => f.records.length >= 3);
    const [s, b] = file.faces[face].records[1];
    const gapStep = series[face].findIndex((v, k) => k > s && !(v > 0));
    const got = await hook<number[]>('m12DrawnTexels', [[face, s], [face, gapStep]]);
    console.log(`receipt mw-off: face ${face} step ${s}: drawn 0x${(got[0] >>> 0).toString(16)}, file 0x${(b >>> 0).toString(16)}; step ${gapStep} (no record) drawn ${f32(got[1])}`);
    assert.equal(got[0] >>> 0, b >>> 0);
    assert.equal(f32(got[1]), 0, 'say NO: a step with no record draws nothing without the window');
    assert.doesNotMatch(shown(await $('[data-part="map-legend"] .vp-legend-title').getText()), /averaged/);
    await clickSelector('[data-map-window="50"]');
    await mapOf(runB, (x) => x.windowSteps === 50, 'the 50 ms window');
    // Past step 60, so the 50-step window is not clipped at step 0 and differs from the 10-step one.
    const cells = picks(50, 3, 60);
    assert.equal(cells.length, 3, 'three cells past step 60 with energy in their 50-step window');
    const g50 = await hook<number[]>('m12DrawnTexels', cells);
    for (const [i, [f, t]] of cells.entries()) {
      const want = mean(series[f], t, 50);
      console.log(`receipt mw-off: 50 ms face ${f} step ${t}: GPU ${f32(g50[i]).toExponential(6)}, file's 50-step mean ${want.toExponential(6)}, its 10-step mean ${mean(series[f], t, 10).toExponential(6)}`);
      assert.ok(near(f32(g50[i]), want), `face ${f} step ${t}`);
      assert.notEqual(want, mean(series[f], t, 10), 'the control: the 50-step mean is not the 10-step one');
    }
    assert.match(shown(await $('[data-part="map-legend"] .vp-legend-title').getText()), /averaged over 50 ms/);
    await clickSelector('[data-map-window="10"]');
    await mapOf(runB, (x) => x.windowSteps === 10, 'the 10 ms window back');
  });

  it('mw-probe: over a face, the window mean\'s bits and level, said to be a mean over 10 ms', async () => {
    const cells = picks(10, 8);
    let probed = false;
    for (const [face, t] of cells) {
      if (probed) break;
      const p = await hook<{ x: number; y: number } | null>('wowMapFacePoint', face);
      if (!p) continue;
      await hook('m12SetStep', t);
      await hover(p);
      const card = await $('[data-part="map-probe"]');
      await card.waitForDisplayed({ timeout: 10_000 });
      await browser.waitUntil(async () => (await card.getAttribute('data-probe-face')) === String(face) && (await card.getAttribute('data-probe-step')) === String(t), {
        timeout: 10_000,
        timeoutMsg: `the probe did not take face ${face} at step ${t}`,
      });
      const want = mean(series[face], t, 10);
      const lvl = shown(await card.$('[data-probe="level"]').getText());
      const value = shown(await card.$('[data-probe="value"]').getText());
      console.log(`receipt mw-probe: face ${face} step ${t}: card bits ${await card.getAttribute('data-probe-bits')} "${lvl}" "${value}", file's 10-step mean 0x${bitsOf(want).toString(16)}`);
      assert.equal(await card.getAttribute('data-probe-bits'), bitsOf(want).toString(16));
      assert.equal(lvl, `${(level(want) as number).toFixed(1)} dB`);
      assert.match(value, /mean of the file's values over 10 ms \(steps \d+–\d+, \d+ of 10 with energy\)/);
      probed = true;
    }
    assert.ok(probed, 'no face with a window mean could be hovered');
  });

  it('mw-cumulative: say NO, the cumulative map takes no window, and says why; off again, the window is back', async () => {
    await clickSelector('[data-part="map-cumulative"]');
    await mapOf(runB, (x) => x.cumulative && x.path === mapPath, 'the cumulative map');
    const m = await hook<MapState>('m12Map');
    assert.equal(m.windowSteps, 1, 'no window on the cumulative map');
    assert.equal(await $('[data-map-window="10"]').getAttribute('disabled'), 'true');
    const why = shown(await $('[data-part="window-refused"]').getText());
    console.log(`receipt mw-cumulative: "${why}"`);
    assert.match(why, /cumulative map already sums every step from 0 ms/);
    // The texels drawn are the running sums of the file (W2's rule), bit for bit: the window is not in them.
    const face = series.findIndex((e) => e.filter((v) => v > 0).length >= 3);
    const t = series[face].findIndex((v) => v > 0) + 20;
    let s = 0;
    for (let k = 0; k <= t; k++) s = fr(s + (Number.isFinite(series[face][k]) ? series[face][k] : 0));
    const got = await hook<number[]>('m12DrawnTexels', [[face, t]]);
    console.log(`receipt mw-cumulative: face ${face} step ${t}: drawn 0x${(got[0] >>> 0).toString(16)}, running sum 0x${bitsOf(s).toString(16)}`);
    assert.equal(got[0] >>> 0, bitsOf(s));
    assert.doesNotMatch(shown(await $('[data-part="map-legend"] .vp-legend-title').getText()), /averaged/);
    await clickSelector('[data-part="map-cumulative"]');
    await mapOf(runB, (x) => !x.cumulative && x.windowSteps === 10, 'the instantaneous map with its window back');
  });

  it('mw-diff: the difference averages both runs over the same window', async () => {
    await clickSelector('[data-part="map-diff"]');
    const d = await mapOf(runB, (x) => x.kind === 'diff' && x.windowSteps === 10, 'the difference over 10 ms');
    assert.equal(d.baseline, runA);
    const base = perStep(readCsbin(path.join(runsRoot, runA, 'solve', ...d.path.split('/'))));
    const cells: { face: number; t: number; want: number; raw: boolean }[] = [];
    for (let face = 0; face < series.length && cells.length < 4; face++) {
      for (let t = 5; t < 200 && cells.length < 4; t += 7) {
        const a = mean(series[face], t, 10);
        const b = mean(base[face], t, 10);
        if (!(a > 0 && b > 0)) continue;
        const raw = series[face][t] > 0 && base[face][t] > 0;
        // The first cell is one the raw difference leaves blank.
        if (cells.length === 0 && raw) continue;
        if (cells.some((c) => c.face === face)) continue;
        cells.push({ face, t, want: (level(a) as number) - (level(b) as number), raw });
      }
    }
    assert.equal(cells.length, 4, 'four cells with energy in both runs\' windows');
    assert.equal(cells[0].raw, false, 'the control: the first has no difference at the raw step');
    const got = await hook<number[]>('m12DiffTexels', cells.map((c) => [c.face, c.t]));
    for (const [i, c] of cells.entries()) {
      console.log(`receipt mw-diff: face ${c.face} step ${c.t}: drawn ${f32(got[i]).toFixed(5)} dB, files' 10-step means ${c.want.toFixed(5)} dB (raw step ${c.raw ? 'has' : 'has no'} difference)`);
      assert.ok(Math.abs(f32(got[i]) - c.want) < 1e-3, `face ${c.face} step ${c.t}`);
    }
    assert.match(shown(await $('[data-part="map-legend"] .vp-legend-title').getText()), /^Difference from .* · averaged over 10 ms$/);
    await clickSelector('[data-part="map-diff"]');
    await mapOf(runB, (x) => x.kind === 'level' && x.windowSteps === 10, 'the level map back');
  });
});
