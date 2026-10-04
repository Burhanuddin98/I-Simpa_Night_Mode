// Wow list W5 (parity R46, R47, R48, R51; docs/investigations/2026-10-04-wow-w1w5/PLAN.md): how
// a surface map is looked at. Each id with its control:
//   w5-smooth    Smooth colour on: at sampled (node, step) pairs the node mean the map shader
//                computes, read back from the GPU, equals upstream's rule computed here from the
//                `.csbin` by this spec's own reader (sum of the linked faces' values over their
//                count, a face linked when its records sum to a finite non-zero); the legend says
//                the colours are smoothed. Controls: a sampled mean differs from each of its faces'
//                own values, and the frame differs from the flat one
//   w5-contours  contours are offered only with smooth colour; every 3 dB they change the frame
//                and the legend names them; smooth off takes them off
//   w5-range     Fixed colour range: the typed ends are the legend's and the shader's, kept when
//                the band changes; low >= high is refused inline and changes nothing; off gives the
//                map's own range back (the file's whole dB, as m12-p3-maps computes it)
//   w5-probe     the pointer over a face of the cutting-plane map shows that face's own record:
//                its float32 bits equal the file's, its level to 0.1 dB, the band and the step's
//                time; a face with no record at the step shows no number; off the map, no card.
//                Screenshot to $M11_SCREENS (w5-probe.png)
//
// The box: tests/fixtures/rooms/outputs_box.simpa (6 x 10 x 3 m, floor surface receiver and the
// cutting plane "Cut"), copied into this spec's own folder on C: (M11_P), run once from the app.
import { strict as assert } from 'node:assert';
import { copyFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { clickSelector, RESULTS_STEP_CURRENT } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { f32, level, readCsbin, type Csbin } from '../lib/m12files.ts';
import { env } from '../lib/types.ts';

const HOOKS = ['idle', 'openProject', 'edit', 'projectJson', 'runStart', 'runState', 'runsRows', 'selectRun', 'setStep', 'm12Map', 'm12SetStep', 'wowLook', 'wowNodeEnergies', 'wowMapFacePoint', 'wowProbe', 'wowOffMapPoint', 'wowFramePixels'];

interface MapState {
  run: string;
  path: string;
  bandHz: number | null;
  kind: string;
  faces: number;
  steps: number;
  step: number;
  error: string | null;
}
interface Look {
  smooth: boolean;
  isoDb: number;
  lo: number;
  hi: number;
  smoothRefusal: string | null;
  fixed: { lo: number; hi: number } | null;
}

const shown = (s: string | null) => (s ?? '').replace(/\s+/g, ' ').trim();
const look = () => hook<Look>('wowLook');
const frame = async () => (await hook<{ hash: number } | null>('wowFramePixels'))?.hash ?? null;

async function mapOf(run: string, want: (m: MapState) => boolean, what: string): Promise<MapState> {
  let last: MapState | null = null;
  await browser.waitUntil(async () => (last = await hook<MapState | null>('m12Map')) !== null && last.run === run && last.error === null && want(last), {
    timeout: 60_000,
    interval: 250,
    timeoutMsg: `the map did not show ${what}: ${JSON.stringify(last)}`,
  });
  return last as unknown as MapState;
}

/** Upstream's node mean from the file (Recepteurs_surfacique.cpp:321-363, 424-431), written from the rule, not the app. */
function nodeMeans(c: Csbin) {
  const linked = c.faces.map((f) => {
    const sum = f.records.reduce((a, [, b]) => a + (Number.isFinite(f32(b)) ? f32(b) : 0), 0);
    return sum !== 0 && Number.isFinite(sum);
  });
  const faces: number[][] = Array.from({ length: c.nodes }, () => []);
  c.faces.forEach((f, i) => {
    if (linked[i]) for (const v of f.vertices) faces[v].push(i);
  });
  const at = (face: number, step: number) => {
    const r = c.faces[face].records.find(([s]) => s === step);
    const v = r ? f32(r[1]) : 0;
    return Number.isFinite(v) ? v : 0;
  };
  return { faces, mean: (n: number, step: number) => (faces[n].length ? faces[n].reduce((a, f) => a + at(f, step), 0) / faces[n].length : 0), at };
}

async function hover(p: { x: number; y: number }): Promise<void> {
  await browser.action('pointer', { parameters: { pointerType: 'mouse' } }).move({ x: p.x, y: p.y, origin: 'viewport', duration: 0 }).perform();
}

describe('W5: smooth colour, contours, fixed range and the value probe', () => {
  let runsRoot = '';
  let run = '';

  before(async () => {
    await waitForHooks(HOOKS);
    const dir = path.join(env('M11_P'), 'm12-mapview');
    mkdirSync(dir, { recursive: true });
    mkdirSync(env('M11_SCREENS'), { recursive: true });
    const box = path.join(dir, 'outputs_box.simpa');
    copyFileSync(path.join(env('M11_REPO'), 'tests', 'fixtures', 'rooms', 'outputs_box.simpa'), box);
    runsRoot = path.join(dir, 'runs');
    await m10.openProject(box);
    const p = JSON.parse(await m10.projectJson()) as { solvers: { spps: Record<string, unknown> } };
    p.solvers.spps.particles_saved = 0;
    assert.equal((await m10.edit({ op: 'set_solver_settings', settings: p.solvers })).applied, true);
    const before = new Set((await hook<{ run: string }[]>('runsRows')).map((r) => r.run));
    await hook('runStart', 'spps');
    let fresh: { run: string; status: string } | undefined;
    await browser.waitUntil(
      async () => {
        if ((await hook<unknown>('runState')) !== null) return false;
        fresh = (await hook<{ run: string; status: string }[]>('runsRows')).find((r) => !before.has(r.run) && r.status !== 'RUNNING');
        return fresh !== undefined;
      },
      { timeout: 300_000, interval: 250, timeoutMsg: 'the run did not end within 300 s' },
    );
    await m10.idle();
    const f = fresh as unknown as { run: string; status: string };
    assert.equal(f.status, 'OK');
    run = f.run;
    await hook('selectRun', run);
    await m10.setStep('results');
    await $(RESULTS_STEP_CURRENT).waitForExist({ timeout: 30_000 });
    console.log(`receipt w5: run ${run} under ${runsRoot}`);
  });

  it('w5-smooth: the node means the shader computes are upstream\'s, from the file; the picture changes', async () => {
    const m = await mapOf(run, (x) => x.bandHz === 1000 && x.kind === 'level', 'the 1 kHz level map');
    const file = readCsbin(path.join(runsRoot, run, 'solve', ...m.path.split('/')));
    await hook('m12SetStep', 2);
    const flat = await frame();
    assert.equal((await look()).smooth, false);
    await clickSelector('[data-part="map-smooth"]');
    const l = await look();
    assert.equal(l.smoothRefusal, null);
    assert.equal(l.smooth, true);
    const smooth = await frame();
    assert.notEqual(smooth, flat, 'the control: smooth colour changes the frame');
    const { faces, mean, at } = nodeMeans(file);
    // Nodes shared by 2 or more faces, at steps where the mean has energy, spread over the map.
    const pairs: [number, number][] = [];
    for (let n = 0; n < file.nodes && pairs.length < 400; n++) {
      if (faces[n].length < 2) continue;
      for (const s of [0, 2, 5, 20]) if (mean(n, s) > 0) pairs.push([n, s]);
    }
    assert.ok(pairs.length >= 3, `only ${pairs.length} shared nodes with energy`);
    const picks = [pairs[0], pairs[Math.floor(pairs.length / 2)], pairs[pairs.length - 1]];
    const got = await hook<number[]>('wowNodeEnergies', picks);
    let differs = false;
    for (const [i, [n, s]] of picks.entries()) {
      const want = mean(n, s);
      const gpu = f32(got[i]);
      console.log(`receipt w5-smooth: node ${n} step ${s}: ${faces[n].length} faces, GPU ${gpu.toExponential(7)}, file rule ${want.toExponential(7)} (${(level(want) as number).toFixed(4)} dB)`);
      assert.ok(Math.abs(gpu - want) <= 1e-6 * want, `node ${n} step ${s}`);
      if (faces[n].every((f) => at(f, s) !== want)) differs = true;
    }
    assert.ok(differs, 'the control: a node mean is not just one face\'s value');
    const note = shown(await $('[data-part="legend-note"]').getText());
    assert.match(note, /Smoothed between faces; the probe reads each face's own value/);
  });

  it('w5-contours: only with smooth colour; every 3 dB they draw and the legend names them; smooth off removes them', async () => {
    assert.equal((await look()).smooth, true);
    const before = await frame();
    await clickSelector('[data-map-contours="3"]');
    const l = await look();
    assert.equal(l.isoDb, 3);
    const after = await frame();
    assert.notEqual(after, before, 'contours change the frame');
    assert.match(shown(await $('[data-part="legend-note"]').getText()), /Contours every 3 dB, on the smoothed levels/);
    await clickSelector('[data-part="map-smooth"]');
    const off = await look();
    assert.deepEqual([off.smooth, off.isoDb], [false, 0]);
    assert.equal(await $('[data-map-contours="3"]').getAttribute('disabled'), 'true', 'say NO: no contours on a flat map');
    // Back on for the probe's screenshot below.
    await clickSelector('[data-part="map-smooth"]');
    await clickSelector('[data-map-contours="3"]');
  });

  it('w5-range: typed ends are the legend\'s and the shader\'s, kept across bands; low >= high refused; off restores', async () => {
    const m = await mapOf(run, (x) => x.bandHz === 1000, '1 kHz');
    const auto = await look();
    await clickSelector('[data-part="map-fixed"]');
    assert.deepEqual((await look()).fixed, { lo: auto.lo, hi: auto.hi }, 'on, it starts at the map\'s own range');
    const set = async (lo: string, hi: string) => {
      for (const [k, t] of [['lo', lo], ['hi', hi]]) {
        // Typed as a person types (select all, then the keys): a WebDriver clear does not reach
        // React's state, and the new text would be appended to the old.
        const el = await $(`[data-field="range.${k}"]`);
        await el.click();
        await browser.keys(['Control', 'a']);
        await browser.keys(t.split(''));
      }
      await browser.keys('Enter');
    };
    await set('30', '80');
    await browser.waitUntil(async () => (await look()).hi === 80, { timeout: 10_000, timeoutMsg: 'the range was not applied' });
    const lab = async (k: string) => shown(await $(`[data-part="map-legend"] [data-legend="${k}"]`).getText());
    assert.deepEqual([await lab('lo'), await lab('hi')], ['30', '80 dB']);
    assert.match(shown(await $('[data-part="map-legend"] .vp-legend-title').getText()), /fixed range/);
    assert.deepEqual([(await look()).lo, (await look()).hi], [30, 80]);
    await clickSelector('[data-map-band="500"]');
    await mapOf(run, (x) => x.bandHz === 500, '500 Hz');
    assert.deepEqual([(await look()).lo, (await look()).hi, await lab('hi')], [30, 80, '80 dB'], 'kept across bands');
    // say NO
    await set('90', '80');
    const refused = await $('[data-part="range-refused"]');
    await refused.waitForDisplayed({ timeout: 10_000 });
    console.log(`receipt w5-range: 90..80: "${shown(await refused.getText())}"`);
    assert.match(shown(await refused.getText()), /The low end must be below the high end: 90 dB is not below 80 dB\. Not applied\./);
    assert.deepEqual([(await look()).lo, (await look()).hi], [30, 80], 'nothing changed');
    await clickSelector('[data-part="map-fixed"]');
    await clickSelector('[data-map-band="1000"]');
    const back = await mapOf(run, (x) => x.bandHz === 1000, '1 kHz again');
    const file = readCsbin(path.join(runsRoot, run, 'solve', ...back.path.split('/')));
    let lo = Infinity;
    let hi = -Infinity;
    for (const f of file.faces) for (const [, b] of f.records) {
      const x = level(f32(b));
      if (x !== null) {
        lo = Math.min(lo, x);
        hi = Math.max(hi, x);
      }
    }
    const top = Math.ceil(hi - 1e-5);
    const bottom = Math.max(Math.floor(lo + 1e-5), top - 60);
    await browser.waitUntil(async () => (await look()).fixed === null && (await look()).hi === top, { timeout: 10_000, timeoutMsg: 'the map\'s own range did not come back' });
    assert.deepEqual([(await look()).lo, await lab('hi')], [bottom, `${top} dB`]);
    console.log(`receipt w5-range: fixed 30..80 dB held over 500 Hz; off: ${bottom}..${top} dB, the file's (${m.path})`);
  });

  it('w5-probe: over a face, its own record from the file; no record, no number; off the map, no card', async () => {
    await clickSelector('[data-map-kind="plane|"]');
    const m = await mapOf(run, (x) => x.bandHz === 1000 && /cut/i.test(x.path), 'the cutting-plane map');
    const file = readCsbin(path.join(runsRoot, run, 'solve', ...m.path.split('/')));
    // A step at which some faces have a record and some do not.
    let step = -1;
    for (let s = 0; s < file.timeSteps && step < 0; s++) {
      const n = file.faces.filter((f) => f.records.some(([t, b]) => t === s && f32(b) > 0)).length;
      if (n > 3 && n < file.faces.length) step = s;
    }
    assert.ok(step >= 0, 'a step with faces with and without energy');
    await hook('m12SetStep', step);
    const withRec = file.faces.map((f, i) => ({ i, r: f.records.find(([t]) => t === step) })).filter((x) => x.r && f32(x.r[1]) > 0);
    let probed = false;
    for (const { i, r } of withRec) {
      const p = await hook<{ x: number; y: number } | null>('wowMapFacePoint', i);
      if (!p) continue;
      await hover(p);
      const card = await $('[data-part="map-probe"]');
      await card.waitForDisplayed({ timeout: 10_000 });
      await browser.waitUntil(async () => (await card.getAttribute('data-probe-face')) === String(i), { timeout: 10_000, timeoutMsg: `the probe did not take face ${i}` });
      const bits = (r as [number, number])[1] >>> 0;
      const want = `${(level(f32(bits)) as number).toFixed(1)} dB`;
      const title = shown(await card.$('[data-probe="title"]').getText());
      const lvl = shown(await card.$('[data-probe="level"]').getText());
      const dt = 0.01;
      console.log(`receipt w5-probe: face ${i} step ${step}: card "${title}" "${lvl}" bits ${await card.getAttribute('data-probe-bits')}, file 0x${bits.toString(16)} (${want})`);
      assert.equal(await card.getAttribute('data-probe-bits'), bits.toString(16));
      assert.equal(lvl, want);
      assert.match(title, /^Cutting planes · 1 kHz · /);
      const ms = step * Math.fround(dt) * 1000;
      assert.ok(title.endsWith(`${ms < 100 ? ms.toFixed(1) : Math.round(ms)} ms`), title);
      assert.match(shown(await card.$('[data-probe="face"]').getText()), /The face's own value from the file\. The colours between faces are smoothed, not values\./);
      assert.equal(await browser.execute(() => document.querySelector('[data-part="map-probe"]')?.closest('[data-results-region]') != null), true);
      await browser.saveScreenshot(path.join(env('M11_SCREENS'), 'w5-probe.png'));
      probed = true;
      break;
    }
    assert.ok(probed, 'no face with energy could be hovered');
    // say NO: a face with no record at this step shows no number.
    const without = file.faces.map((f, i) => ({ i, r: f.records.find(([t]) => t === step) })).filter((x) => !x.r);
    let saidNo = false;
    for (const { i } of without) {
      const p = await hook<{ x: number; y: number } | null>('wowMapFacePoint', i);
      if (!p) continue;
      await hover(p);
      const card = await $('[data-part="map-probe"]');
      await browser.waitUntil(async () => (await card.getAttribute('data-probe-face')) === String(i), { timeout: 10_000, timeoutMsg: `the probe did not take face ${i}` });
      assert.equal(shown(await card.$('[data-probe="value"]').getText()), 'No energy at this step');
      assert.equal(await card.$('[data-probe="level"]').isExisting(), false);
      console.log(`receipt w5-probe: face ${i} has no record at step ${step}: no number`);
      saidNo = true;
      break;
    }
    if (!saidNo) console.log('note w5-probe: every face without a record at the step is hidden from the camera; the no-record case is the unit test\'s');
    const off = await hook<{ x: number; y: number } | null>('wowOffMapPoint');
    assert.ok(off, 'a point on the canvas off the map');
    await hover(off);
    await browser.waitUntil(async () => !(await $('[data-part="map-probe"]').isExisting()), { timeout: 10_000, timeoutMsg: 'the card stayed off the map' });
  });
});
