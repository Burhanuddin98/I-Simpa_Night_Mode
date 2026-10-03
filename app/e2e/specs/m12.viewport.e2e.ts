// M12 P3's spec (docs/investigations/2026-10-03-m12/PLAN.md, P3; gate (c), (d), MQ4): the
// Results step's viewport, the surface map and particle playback on one timeline. Its checks,
// each with the control that lets it fail:
//   m12-c       3 sampled (face, step) texels of the map, read back from the GPU texture, equal
//               the .csbin's float32 bit for bit (read here by lib/m12files.ts, not by the app);
//               controls: the three values differ from each other, a cell with no record reads
//               0, and the texture is float32 and unfiltered
//   m12-d       at 5 steps the particles the playback draw lets through, counted on the GPU,
//               equal the particles alive in the .pbin at that step, and the map is at the same
//               step (one timeline); control: the steps include different counts and a 0
//   m12-mq4     a run with particles saved 0 says "No particles saved for this run" and how to
//               turn it on, with the file size for this run (control: the run with particles
//               saved shows no such notice)
//   m12-p3-maps the legend's range is the map's own (whole dB, 60 dB deep) and black-red-yellow;
//               another band loads that band's file; the difference from the baseline run is
//               this run's level minus the baseline's at sampled cells; Play moves map and
//               particles together
//
// The gate run: tests/fixtures/rooms/outputs_box.simpa (a 10 x 6 x 3 m box with a floor surface
// receiver and a cutting plane, 2,000 particles, 1 s in 10 ms steps, 500 Hz and 1 kHz), copied
// into this spec's own folder on C: (M11_P), run twice from the app: run A with particles saved 0
// and seed 2 (the baseline, MQ4), run B with particles saved 40 and seed 1. B's .pbin is a few kB.
import { strict as assert } from 'node:assert';
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { clickSelector, RESULTS_STEP_CURRENT } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { waitRun } from '../lib/m11.ts';
import { f32, level, readCsbin, readPbin, type Csbin } from '../lib/m12files.ts';
import { env } from '../lib/types.ts';

interface MapState {
  run: string;
  path: string;
  bandHz: number | null;
  kind: 'level' | 'diff';
  faces: number;
  steps: number;
  step: number;
  texture: { type: string; format: string; filter: string; width: number; height: number; bytes: number };
  legend: { lo: string; mid: string; hi: string; gradient: string } | null;
  baseline: string | null;
  error: string | null;
}
interface ParticleState {
  run: string;
  bandHz: number;
  step: number;
  mapStep: number | null;
  rendered: number;
  particles: number;
  records: number;
  bufferBytes: number;
}

const HOOKS = ['idle', 'openProject', 'edit', 'projectJson', 'runStart', 'waitRun', 'selectRun', 'setStep', 'm12Map', 'm12Texels', 'm12DiffTexels', 'm12SetStep', 'm12Particles'];

const mapState = () => hook<MapState | null>('m12Map');
const particleState = () => hook<ParticleState | null>('m12Particles');
const shown = (s: string | null) => (s ?? '').replace(/\s+/g, ' ').trim();

/** Waits until the map shows `run` (and `bandHz`, `kind` when given), loaded with no error. */
async function mapOf(run: string, want: { bandHz?: number | null; kind?: 'level' | 'diff' } = {}): Promise<MapState> {
  let last: MapState | null = null;
  await browser.waitUntil(
    async () => {
      last = await mapState();
      return (
        last !== null &&
        last.run === run &&
        last.error === null &&
        (want.bandHz === undefined || last.bandHz === want.bandHz) &&
        (want.kind === undefined || last.kind === want.kind)
      );
    },
    { timeout: 60_000, interval: 250, timeoutMsg: `the map did not show ${run} ${JSON.stringify(want)}: ${JSON.stringify(last)}` },
  );
  return last as unknown as MapState;
}

/** The run folder's `solve/<rel>`. */
const solveFile = (runsRoot: string, run: string, rel: string) => path.join(runsRoot, run, 'solve', ...rel.split('/'));

/** Up to `n` cells with a positive value, spread over the faces and steps. */
function sampleCells(c: Csbin, n: number): { face: number; step: number; bits: number }[] {
  const all: { face: number; step: number; bits: number }[] = [];
  c.faces.forEach((f, face) => {
    for (const [step, bits] of f.records) if (f32(bits) > 0) all.push({ face, step, bits });
  });
  assert.ok(all.length >= n, `only ${all.length} cells with energy in the map`);
  const out: typeof all = [];
  for (let i = 0; i < n; i++) out.push(all[Math.floor((i * (all.length - 1)) / Math.max(1, n - 1))]);
  return out;
}

describe('M12 P3 viewport: surface maps and particle playback', () => {
  let box = '';
  let runsRoot = '';
  let runA = '';
  let runB = '';

  async function runWith(saved: number, seed: number): Promise<string> {
    const p = JSON.parse(await m10.projectJson()) as { solvers: { spps: Record<string, unknown> } };
    p.solvers.spps.particles_saved = saved;
    p.solvers.spps.random_seed = seed;
    const r = await m10.edit({ op: 'set_solver_settings', settings: p.solvers });
    assert.equal(r.applied, true, `settings refused: ${JSON.stringify(r.refusals)}`);
    await hook('runStart', 'spps');
    const run = await waitRun(null, 'ended', 300_000);
    await m10.idle();
    return run;
  }

  before(async () => {
    await waitForHooks(HOOKS);
    const dir = path.join(env('M11_P'), 'm12-viewport');
    mkdirSync(dir, { recursive: true });
    box = path.join(dir, 'outputs_box.simpa');
    copyFileSync(path.join(env('M11_REPO'), 'tests', 'fixtures', 'rooms', 'outputs_box.simpa'), box);
    runsRoot = path.join(dir, 'runs');
    await m10.openProject(box);
    runA = await runWith(0, 2);
    runB = await runWith(40, 1);
    console.log(`receipt m12 viewport: run A ${runA} (particles saved 0), run B ${runB} (particles saved 40) under ${runsRoot}`);
    assert.ok(existsSync(path.join(runsRoot, runB, 'run.json')));
  });

  it('m12-mq4: a run with no particles saved says so, and how to turn it on with the file size', async () => {
    await hook('selectRun', runA);
    await m10.setStep('results');
    await $(RESULTS_STEP_CURRENT).waitForExist({ timeout: 30_000 });
    const none = await $('[data-part="particles-none"]');
    await none.waitForDisplayed({ timeout: 60_000 });
    const title = shown(await none.$('[data-part="particles-none-title"]').getText());
    const how = shown(await none.$('[data-part="particles-none-how"]').getText());
    // 1,000 particles x 100 steps x 16 bytes x 1 source = 1.6 MB a band (upstream's formula).
    console.log(`receipt m12-mq4: "${title}" / "${how}"`);
    assert.equal(title, 'No particles saved for this run');
    assert.match(how, /Simulate › Particles saved for playback/);
    assert.match(how, /1,000 saved particles write about 1\.6 MB a band/);
    assert.equal(await particleState(), null, 'no particle buffer for a run that saved none');
    const inRegion = await browser.execute((sel: string) => document.querySelector(sel)?.closest('[data-results-region]') != null, '[data-part="particles-none"]');
    assert.equal(inRegion, true, 'the notice is inside a results region (m10-h, m11-h)');
  });

  it('m12-c: 3 sampled texels of the map equal the .csbin float32s bit for bit', async () => {
    await hook('selectRun', runB);
    const m = await mapOf(runB, { bandHz: 1000, kind: 'level' });
    assert.match(m.path, /^Surface receiver\/1000 Hz\/Sound level\.csbin$/);
    assert.deepEqual([m.texture.type, m.texture.format, m.texture.filter], ['float32', 'R32F', 'nearest']);
    const file = readCsbin(solveFile(runsRoot, runB, m.path));
    assert.equal(m.faces, file.faces.length);
    assert.equal(m.steps, file.timeSteps);
    const cells = sampleCells(file, 3);
    assert.equal(new Set(cells.map((c) => c.bits)).size, 3, 'the control: three different values');
    // A cell with no record: 0 in the texture.
    const emptyFace = file.faces.findIndex((f) => f.records.length < file.timeSteps);
    const used = new Set(file.faces[emptyFace].records.map(([s]) => s));
    let emptyStep = 0;
    while (used.has(emptyStep)) emptyStep++;
    const got = await hook<number[]>('m12Texels', [...cells.map((c) => [c.face, c.step]), [emptyFace, emptyStep]]);
    for (const [i, c] of cells.entries()) {
      console.log(`receipt m12-c: face ${c.face} step ${c.step}: texel 0x${got[i].toString(16)} file 0x${c.bits.toString(16)} (${f32(c.bits)})`);
      assert.equal(got[i] >>> 0, c.bits >>> 0, `face ${c.face} step ${c.step}`);
    }
    assert.equal(got[3], 0, `face ${emptyFace} step ${emptyStep} has no record: 0`);
    console.log(`receipt m12-c: texture ${m.texture.width} x ${m.texture.height} ${m.texture.format}, ${m.texture.bytes} B for ${m.faces} faces x ${m.steps} steps`);
  });

  it('m12-d: at 5 steps the particles drawn equal the particles alive in the .pbin, on the map’s timeline', async () => {
    await hook('selectRun', runB);
    await mapOf(runB, { bandHz: 1000 });
    assert.equal(await $('[data-part="particles-none"]').isExisting(), false, 'the control: run B saved particles');
    let p: ParticleState | null = null;
    await browser.waitUntil(async () => (p = await particleState()) !== null && p.run === runB, {
      timeout: 60_000,
      timeoutMsg: 'no particles loaded for run B',
    });
    const file = readPbin(path.join(runsRoot, runB, 'solve', 'Particles', '1000', 'particles.pbin'));
    const ps = p as unknown as ParticleState;
    assert.equal(ps.particles, file.particles);
    assert.equal(ps.records, file.records);
    const alive = file.alive;
    const living = alive.map((n, s) => [n, s] as const).filter(([n]) => n > 0).map(([, s]) => s);
    assert.ok(living.length >= 2, `particles alive at only ${living.length} steps`);
    const peak = living.reduce((a, s) => (alive[s] > alive[a] ? s : a), living[0]);
    const last = living[living.length - 1];
    const picks = [living[0], peak, living[Math.floor(living.length / 2)], last];
    if (last + 1 < alive.length) picks.push(last + 1);
    for (const s of living) if (new Set(picks).size < 5 && !picks.includes(s)) picks.push(s);
    const steps = [...new Set(picks)].slice(0, 5);
    assert.equal(steps.length, 5);
    assert.ok(new Set(steps.map((s) => alive[s])).size >= 2, 'the control: different counts among the steps');
    for (const s of steps) {
      await hook('m12SetStep', s);
      const now = (await particleState()) as ParticleState;
      console.log(`receipt m12-d: step ${s}: drawn ${now.rendered}, alive in the .pbin ${alive[s]}, map at step ${now.mapStep}`);
      assert.equal(now.step, s);
      assert.equal(now.mapStep, s, 'map and particles on one timeline');
      assert.equal(now.rendered, alive[s], `step ${s}`);
    }
    console.log(`receipt m12-d: particle buffer ${ps.bufferBytes} B for ${ps.records} records`);
  });

  it('m12-p3-maps: legend, band choice, difference from the baseline, and Play on one timeline', async () => {
    await hook('selectRun', runB);
    const m = await mapOf(runB, { bandHz: 1000, kind: 'level' });
    const file = readCsbin(solveFile(runsRoot, runB, m.path));
    // The legend: whole dB around every level in the file, at most 60 dB deep.
    let lo = Infinity;
    let hi = -Infinity;
    for (const f of file.faces) for (const [, b] of f.records) {
      const l = level(f32(b));
      if (l !== null) {
        lo = Math.min(lo, l);
        hi = Math.max(hi, l);
      }
    }
    const top = Math.ceil(hi - 1e-5);
    const bottom = Math.max(Math.floor(lo + 1e-5), top - 60);
    const legend = await $('[data-part="map-legend"]');
    await legend.waitForDisplayed({ timeout: 30_000 });
    assert.equal(await legend.getAttribute('data-results-region'), '', 'the legend is a results region (m10-h, m11-h)');
    const lab = async (k: string) => shown(await legend.$(`[data-legend="${k}"]`).getText());
    console.log(`receipt m12-p3-maps: legend ${await lab('lo')} / ${await lab('mid')} / ${await lab('hi')}, file ${lo.toFixed(3)}..${hi.toFixed(3)} dB`);
    assert.equal(await lab('lo'), String(bottom));
    assert.equal(await lab('hi'), `${top} dB`);
    assert.match(m.legend?.gradient ?? '', /#0B0B0E.*#B0161F.*#FCD270/i, 'black through red to yellow');

    // Band choice: 500 Hz loads the 500 Hz file.
    await clickSelector('[data-map-band="500"]');
    const m500 = await mapOf(runB, { bandHz: 500 });
    assert.match(m500.path, /^Surface receiver\/500 Hz\/Sound level\.csbin$/);
    const f500 = readCsbin(solveFile(runsRoot, runB, m500.path));
    const c500 = sampleCells(f500, 1)[0];
    assert.equal((await hook<number[]>('m12Texels', [[c500.face, c500.step]]))[0] >>> 0, c500.bits >>> 0);
    await clickSelector('[data-map-band="1000"]');
    await mapOf(runB, { bandHz: 1000 });

    // The difference from the baseline (run A, the same surface).
    await clickSelector('[data-part="map-diff"]');
    const d = await mapOf(runB, { kind: 'diff' });
    assert.equal(d.baseline, runA);
    const base = readCsbin(solveFile(runsRoot, runA, d.path));
    const both: { face: number; step: number; want: number }[] = [];
    file.faces.forEach((f, face) => {
      const bmap = new Map(base.faces[face].records);
      for (const [step, bits] of f.records) {
        const la = level(f32(bits));
        const lb = bmap.has(step) ? level(f32(bmap.get(step) as number)) : null;
        if (la !== null && lb !== null) both.push({ face, step, want: la - lb });
      }
    });
    assert.ok(both.length >= 3, `only ${both.length} cells hold energy in both runs`);
    const picks = [both[0], both[Math.floor(both.length / 2)], both[both.length - 1]];
    const got = await hook<number[]>('m12DiffTexels', picks.map((c) => [c.face, c.step]));
    for (const [i, c] of picks.entries()) {
      console.log(`receipt m12-p3-maps: diff face ${c.face} step ${c.step}: drawn ${f32(got[i]).toFixed(5)} dB, files ${c.want.toFixed(5)} dB`);
      assert.ok(Math.abs(f32(got[i]) - c.want) < 1e-3, `face ${c.face} step ${c.step}`);
    }
    assert.ok(new Set(picks.map((c) => Math.round(c.want * 1e3))).size > 1 || picks[0].want !== 0, 'the control: not all zero');
    assert.match(shown(await legend.$('[data-legend="hi"]').getText()), /^\+\d+ dB$/);
    await clickSelector('[data-part="map-diff"]');
    await mapOf(runB, { kind: 'level' });

    // Play: map and particles move together.
    await hook('m12SetStep', 0);
    await clickSelector('[data-part="anim-play"]');
    await browser.waitUntil(async () => ((await mapState())?.step ?? 0) >= 5, { timeout: 30_000, timeoutMsg: 'Play did not move the map' });
    await clickSelector('[data-part="anim-play"]');
    const ms = await mapState();
    const pst = await particleState();
    assert.equal(pst?.step, ms?.step, 'one timeline');
    assert.equal(pst?.mapStep, ms?.step);
    console.log(`receipt m12-p3-maps: paused at step ${ms?.step}: map and particles together`);
  });
});
