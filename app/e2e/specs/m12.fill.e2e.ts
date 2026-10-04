// Wow list W2 (parity R40; docs/investigations/2026-10-04-wow-w2w3w9/PLAN.md): sound filling the
// hall, the cumulative map. Each id with its control:
//   w2-fill    Cumulative on: sampled (face, step) texels read back from the GPU equal upstream's
//              rule computed here from the `.csbin` by this spec's own reader (each record added
//              into its step in float32, a non-finite one as 0, then the running sum in float32),
//              bit for bit; the legend says cumulative. Controls: a sampled texel differs from the
//              instantaneous record at that cell, the sums grow along a face, and the frame
//              differs from the instantaneous one. Screenshot w2-fill.png
//   w2-probe   the probe over a face shows its running sum's bits and level, and says it is summed
//   w2-refuse  a difference is not shown cumulative: the switch is disabled, the map says why,
//              and the texels are the file's instantaneous records again; difference off, the
//              cumulative map comes back
//
// The box: tests/fixtures/rooms/outputs_box.simpa, copied into this spec's own folder on C:
// (M11_P), run once from the app with particles saved 0.
import { strict as assert } from 'node:assert';
import { copyFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { clickSelector, RESULTS_STEP_CURRENT } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { f32, level, readCsbin, type Csbin } from '../lib/m12files.ts';
import { env } from '../lib/types.ts';

const HOOKS = ['idle', 'openProject', 'edit', 'projectJson', 'runStart', 'runState', 'runsRows', 'selectRun', 'setStep', 'm12Map', 'm12Texels', 'm12SetStep', 'wowMapFacePoint', 'wowFramePixels'];

interface MapState {
  run: string;
  path: string;
  bandHz: number | null;
  kind: string;
  cumulative: boolean;
  faces: number;
  steps: number;
  error: string | null;
}

const shown = (s: string | null) => (s ?? '').replace(/\s+/g, ' ').trim();
const frame = async () => (await hook<{ hash: number } | null>('wowFramePixels'))?.hash ?? null;
const bitsOf = (v: number) => new Uint32Array(new Float32Array([v]).buffer)[0];

async function mapOf(run: string, want: (m: MapState) => boolean, what: string): Promise<MapState> {
  let last: MapState | null = null;
  await browser.waitUntil(async () => (last = await hook<MapState | null>('m12Map')) !== null && last.run === run && last.error === null && want(last), {
    timeout: 60_000,
    interval: 250,
    timeoutMsg: `the map did not show ${what}: ${JSON.stringify(last)}`,
  });
  return last as unknown as MapState;
}

/** Upstream's cumulative rule from the file (Recepteurs_surfacique.cpp:337-387), written from the rule, not the app. */
function cumulative(c: Csbin): number[][] {
  return c.faces.map((f) => {
    const e = new Array<number>(c.timeSteps).fill(0);
    for (const [s, b] of f.records) {
      if (s >= c.timeSteps) continue;
      const v = f32(b);
      e[s] = Math.fround(e[s] + (Number.isFinite(v) ? v : 0));
    }
    for (let t = 1; t < e.length; t++) e[t] = Math.fround(e[t] + e[t - 1]);
    return e;
  });
}

async function hover(p: { x: number; y: number }): Promise<void> {
  await browser.action('pointer', { parameters: { pointerType: 'mouse' } }).move({ x: p.x, y: p.y, origin: 'viewport', duration: 0 }).perform();
}

describe('W2: sound filling the hall (the cumulative map)', () => {
  let runsRoot = '';
  let run = '';
  let file: Csbin;
  let sums: number[][] = [];
  let mapPath = '';

  before(async () => {
    await waitForHooks(HOOKS);
    const dir = path.join(env('M11_P'), 'm12-fill');
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
    console.log(`receipt w2: run ${run} under ${runsRoot}`);
  });

  it('w2-fill: the cumulative texels are upstream\'s float32 running sums of the file, bit for bit; the picture changes', async () => {
    const m = await mapOf(run, (x) => x.bandHz === 1000 && x.kind === 'level' && !x.cumulative, 'the 1 kHz instantaneous map');
    mapPath = m.path;
    file = readCsbin(path.join(runsRoot, run, 'solve', ...m.path.split('/')));
    sums = cumulative(file);
    // A face and steps where the running sum has energy and differs from that step's own record.
    const picks: [number, number][] = [];
    for (let face = 0; face < file.faces.length && picks.length < 3; face++) {
      const recs = file.faces[face].records;
      if (recs.length < 3) continue;
      const first = Math.min(...recs.map(([s]) => s));
      for (const s of [first + 2, first + 10, Math.min(file.timeSteps - 1, first + 40)]) {
        const own = recs.find(([t]) => t === s);
        if (sums[face][s] > 0 && (!own || f32(own[1]) !== sums[face][s]) && picks.length < 3 && !picks.some(([a]) => a === face)) picks.push([face, s]);
      }
    }
    assert.equal(picks.length, 3, 'three cells where the running sum is not the step\'s own record');
    await hook('m12SetStep', picks[1][1]);
    const flat = await frame();
    await clickSelector('[data-part="map-cumulative"]');
    await mapOf(run, (x) => x.bandHz === 1000 && x.cumulative && x.path === mapPath, 'the cumulative map');
    await hook('m12SetStep', picks[1][1]);
    const got = await hook<number[]>('m12Texels', picks);
    for (const [i, [face, s]] of picks.entries()) {
      const want = sums[face][s];
      const own = file.faces[face].records.find(([t]) => t === s);
      console.log(`receipt w2-fill: face ${face} step ${s}: GPU 0x${(got[i] >>> 0).toString(16)}, file's running sum 0x${bitsOf(want).toString(16)} (${(level(want) as number).toFixed(2)} dB), the step's own record ${own ? f32(own[1]).toExponential(4) : 'none'}`);
      assert.equal(got[i] >>> 0, bitsOf(want), `face ${face} step ${s}`);
    }
    // Control: the sums grow along a face.
    const [face0] = picks[0];
    assert.ok(sums[face0][file.timeSteps - 1] >= sums[face0][picks[0][1]] && sums[face0][file.timeSteps - 1] > 0);
    assert.notEqual(await frame(), flat, 'the control: the cumulative frame differs from the instantaneous one');
    assert.match(shown(await $('[data-part="map-legend"] .vp-legend-title').getText()), /cumulative level from 0 ms/);
    assert.match(shown(await $('[data-part="cumulative-note"]').getText()), /builds up and holds/);
    await hook('m12SetStep', Math.min(file.timeSteps - 1, 30));
    await browser.saveScreenshot(path.join(env('M11_SCREENS'), 'w2-fill.png'));
  });

  it('w2-probe: over a face, its running sum\'s bits and level, said to be summed', async () => {
    const step = Math.min(file.timeSteps - 1, 30);
    await hook('m12SetStep', step);
    let probed = false;
    for (let face = 0; face < file.faces.length && !probed; face++) {
      if (!(sums[face][step] > 0)) continue;
      const p = await hook<{ x: number; y: number } | null>('wowMapFacePoint', face);
      if (!p) continue;
      await hover(p);
      const card = await $('[data-part="map-probe"]');
      await card.waitForDisplayed({ timeout: 10_000 });
      await browser.waitUntil(async () => (await card.getAttribute('data-probe-face')) === String(face), { timeout: 10_000, timeoutMsg: `the probe did not take face ${face}` });
      const want = sums[face][step];
      const lvl = shown(await card.$('[data-probe="level"]').getText());
      console.log(`receipt w2-probe: face ${face} step ${step}: card bits ${await card.getAttribute('data-probe-bits')} "${lvl}", file's running sum 0x${bitsOf(want).toString(16)}`);
      assert.equal(await card.getAttribute('data-probe-bits'), bitsOf(want).toString(16));
      assert.equal(lvl, `${(level(want) as number).toFixed(1)} dB`);
      assert.match(shown(await card.$('[data-probe="value"]').getText()), /summed from the first step to this one/);
      probed = true;
    }
    assert.ok(probed, 'no face with a running sum could be hovered');
  });

  it('w2-refuse: a difference is not shown cumulative; the switch says no and the texels are the file\'s records again', async () => {
    await clickSelector('[data-part="map-diff"]');
    const m = await mapOf(run, (x) => !x.cumulative, 'the map with the difference switch on');
    assert.equal(await $('[data-part="map-cumulative"]').getAttribute('disabled'), 'true', 'say NO: the switch is off for a difference');
    const why = shown(await $('[data-part="cumulative-refused"]').getText());
    console.log(`receipt w2-refuse: "${why}" (map kind ${m.kind})`);
    assert.match(why, /A difference between two runs is shown instantaneous only/);
    // The texels are the instantaneous records, the m12-c rule.
    const face = file.faces.findIndex((f) => f.records.length >= 3);
    const [s, b] = file.faces[face].records[1];
    assert.equal(((await hook<number[]>('m12Texels', [[face, s]]))[0]) >>> 0, b >>> 0);
    await clickSelector('[data-part="map-diff"]');
    await mapOf(run, (x) => x.cumulative && x.kind === 'level', 'the cumulative map back');
    await clickSelector('[data-part="map-cumulative"]');
    await mapOf(run, (x) => !x.cumulative, 'the instantaneous map back');
  });
});
