// Wow list W3 (parity R54; docs/investigations/2026-10-04-wow-w2w3w9/PLAN.md): particle trails.
// Each id with its control:
//   w3-count   trails 5 and 1 (upstream's ray) steps long: at 5 steps the segments the trail draw
//              keeps, counted on the GPU (its own material in count mode), equal the rule from the
//              `.pbin` read by this spec's reader: each particle alive at the step keeps
//              min(N, step - first) segments; and the map, the dots and the trails are at the one
//              step. Controls: the counts differ among the steps and N, a step past every
//              particle's end keeps 0. Screenshot w3-trails.png
//   w3-play    while Play runs at 0.1x (Burhan 2026-10-04 15:08), the trails move on the dots' one
//              timeline a step at a time: every sample's segments drawn equal the .pbin's rule at
//              the step drawn, and the frames drawn never skip a step. Control: the samples span
//              several steps with different counts
//   w3-refuse  a run that saved no particles offers no trails: the length chips are disabled and
//              say why, and nothing is drawn
//
// The box: tests/fixtures/rooms/outputs_box.simpa, copied into this spec's own folder on C:
// (M11_P), run twice from the app: particles saved 0, then 40.
import { strict as assert } from 'node:assert';
import { copyFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { clickSelector, RESULTS_STEP_CURRENT } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { readPbin } from '../lib/m12files.ts';
import { env } from '../lib/types.ts';

const HOOKS = ['idle', 'openProject', 'edit', 'projectJson', 'runStart', 'runState', 'runsRows', 'selectRun', 'setStep', 'm12Map', 'm12SetStep', 'm12Particles', 'wowTrails', 'm12DrawnSteps'];

interface TrailState {
  steps: number;
  on: boolean;
  segments: number;
  bytes: number;
  refusal: string | null;
  step: number;
  drawn: number;
}

const trails = () => hook<TrailState>('wowTrails');

describe('W3: particle trails', () => {
  let runsRoot = '';
  let runNone = '';
  let runSaved = '';

  async function runWith(saved: number, seed: number): Promise<string> {
    const p = JSON.parse(await m10.projectJson()) as { solvers: { spps: Record<string, unknown> } };
    p.solvers.spps.particles_saved = saved;
    p.solvers.spps.random_seed = seed;
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
    return f.run;
  }

  before(async () => {
    await waitForHooks(HOOKS);
    const dir = path.join(env('M11_P'), 'm12-trails');
    mkdirSync(dir, { recursive: true });
    mkdirSync(env('M11_SCREENS'), { recursive: true });
    const box = path.join(dir, 'outputs_box.simpa');
    copyFileSync(path.join(env('M11_REPO'), 'tests', 'fixtures', 'rooms', 'outputs_box.simpa'), box);
    runsRoot = path.join(dir, 'runs');
    await m10.openProject(box);
    runNone = await runWith(0, 2);
    runSaved = await runWith(40, 1);
    await m10.setStep('results');
    await $(RESULTS_STEP_CURRENT).waitForExist({ timeout: 30_000 });
    console.log(`receipt w3: run ${runNone} (particles saved 0), run ${runSaved} (40) under ${runsRoot}`);
  });

  it('w3-count: at 5 steps the segments drawn equal each live particle\'s last N steps from the .pbin', async () => {
    await hook('selectRun', runSaved);
    await browser.waitUntil(async () => (await hook<{ run: string } | null>('m12Particles'))?.run === runSaved, { timeout: 60_000, timeoutMsg: 'no particles loaded' });
    const file = readPbin(path.join(runsRoot, runSaved, 'solve', 'Particles', '1000', 'particles.pbin'));
    const rule = (s: number, n: number) => file.spans.reduce((c, [first, k]) => (k > 0 && s >= first && s <= first + k - 1 ? c + Math.min(n, s - first) : c), 0);
    await $('[data-trails="5"]').waitForEnabled({ timeout: 10_000 });
    await clickSelector('[data-trails="5"]');
    await browser.waitUntil(async () => (await trails()).on, { timeout: 10_000, timeoutMsg: 'trails did not switch on' });
    const t0 = await trails();
    assert.equal(t0.segments, file.records - file.spans.filter(([, n]) => n > 0).length, 'one segment per pair of consecutive records');
    const living = file.alive.map((n, s) => [n, s] as const).filter(([n]) => n > 0).map(([, s]) => s);
    const last = living[living.length - 1];
    const steps = [living[1] ?? living[0], living[Math.floor(living.length / 4)], living[Math.floor(living.length / 2)], last, Math.min(file.stepsMax - 1, last + 1)];
    const seen = new Set<number>();
    for (const n of [5, 1]) {
      if (n === 1) await clickSelector('[data-trails="1"]');
      for (const s of steps) {
        await hook('m12SetStep', s);
        const t = await trails();
        const p = await hook<{ step: number; mapStep: number | null }>('m12Particles');
        const want = rule(s, n);
        console.log(`receipt w3-count: ${n} step(s), step ${s}: drawn ${t.drawn}, the .pbin's rule ${want}; dots at ${p.step}, map at ${p.mapStep}`);
        assert.equal(t.steps, n);
        assert.equal(t.step, s);
        assert.equal(p.step, s, 'dots and trails on one timeline');
        assert.equal(t.drawn, want, `${n} steps at step ${s}`);
        seen.add(want);
      }
    }
    assert.ok(seen.size >= 3, `the control: different counts among the steps and lengths (${[...seen].join(', ')})`);
    if (last + 1 < file.stepsMax) assert.equal(rule(last + 1, 5), 0, 'the control: past every particle\'s end, none');
    console.log(`receipt w3-count: ${t0.segments} segments, ${t0.bytes} B on the GPU`);
    await clickSelector('[data-trails="20"]');
    await hook('m12SetStep', living[Math.floor(living.length / 4)]);
    await browser.saveScreenshot(path.join(env('M11_SCREENS'), 'w3-trails.png'));
  });

  it('w3-play: at 0.1x the trails follow Play a step at a time on the dots\' timeline', async () => {
    await hook('selectRun', runSaved);
    await browser.waitUntil(async () => (await hook<{ run: string } | null>('m12Particles'))?.run === runSaved, { timeout: 60_000, timeoutMsg: 'no particles loaded' });
    const file = readPbin(path.join(runsRoot, runSaved, 'solve', 'Particles', '1000', 'particles.pbin'));
    const rule = (s: number, n: number) => file.spans.reduce((c, [first, k]) => (k > 0 && s >= first && s <= first + k - 1 ? c + Math.min(n, s - first) : c), 0);
    await clickSelector('[data-trails="5"]');
    await browser.waitUntil(async () => (await trails()).steps === 5, { timeout: 10_000, timeoutMsg: 'trails 5 did not switch on' });
    await clickSelector('[data-anim-speed="0.1"]');
    await hook('m12SetStep', 0);
    await hook('m12DrawnSteps');
    await clickSelector('[data-part="anim-play"]');
    const samples: { step: number; drawn: number }[] = [];
    const t0 = Date.now();
    while (Date.now() - t0 < 2_500) {
      const t = await trails();
      samples.push({ step: t.step, drawn: t.drawn });
    }
    await clickSelector('[data-part="anim-play"]');
    const log = await hook<{ t: number; step: number }[]>('m12DrawnSteps');
    const distinct = [...new Set(samples.map((x) => x.step))];
    console.log(`receipt w3-play: ${samples.length} samples over steps ${distinct.join(',')}; frames drawn at steps ${[...new Set(log.map((f) => f.step))].join(',')}`);
    for (const x of samples) assert.equal(x.drawn, rule(x.step, 5), `trails at step ${x.step}`);
    for (let i = 1; i < log.length; i++) assert.ok([0, 1].includes(log[i].step - log[i - 1].step), `frames drawn at step ${log[i - 1].step} then ${log[i].step}`);
    assert.ok(distinct.length >= 5, `the control: only ${distinct.length} steps sampled`);
    assert.ok(new Set(distinct.map((s) => rule(s, 5))).size >= 2, 'the control: different counts among the steps');
    await clickSelector('[data-anim-speed="0.01"]');
    await clickSelector('[data-trails="0"]');
  });

  it('w3-refuse: a run with no particles saved offers no trails, and says why', async () => {
    await hook('selectRun', runNone);
    await $('[data-part="particles-none"]').waitForExist({ timeout: 60_000 });
    const chip = await $('[data-trails="5"]');
    assert.equal(await chip.getAttribute('disabled'), 'true', 'say NO: no trails without particles');
    assert.match((await chip.getAttribute('title')) ?? '', /No particles saved for this run/);
    const t = await trails();
    console.log(`receipt w3-refuse: ${JSON.stringify(t)}`);
    assert.equal(t.on, false);
    assert.equal(t.drawn, 0);
    assert.equal(t.refusal, 'No particles saved for this run');
  });
});
