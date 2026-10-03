// PQ3's spec (docs/investigations/2026-10-03-pq3/PLAN.md, order of work 4): the Simulate settings
// editor, driven by WebDriver clicks and keys on the fields a user would use. Its checks, each
// with the control that lets it fail:
//   pq3-settings-edit      particles, time step and one band typed or clicked on the box; the
//                          refusals of a bad value are shown inline and change nothing; Run saves
//                          first, and the run's config.xml carries the edited values (control:
//                          the values before the edits are not the ones read back)
//   pq3-settings-bands-off every SPPS band off: Run is disabled with NO_BAND_COMPUTED on a FAIL
//                          row and under the bands; TCR, which computes its own bands, is not
//                          blocked (the control); undo gives the bands back
//   pq3-settings-preset    a band preset, confirmed, moves the project onto its bands; one undo
//                          gives back the project text exactly
//
// The box is copied into this spec's own folder under M11_P (on C:), so its runs never mix with
// the gate's. The window stays visible and unfocused (m11.conf.ts).
import { strict as assert } from 'node:assert';
import { copyFileSync, mkdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { clickSelector } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { env } from '../lib/types.ts';

const CTRL = '';
const ENTER = '';
const BACKSPACE = '';

interface Row {
  run: string;
  number: number;
  status: string;
}
interface Spps {
  particles_per_source: number;
  time_step_s: number;
  bands_computed: boolean[];
}
interface ProjectFile {
  bands: { kind: string; frequencies_hz: number[] };
  environment: { relative_humidity_percent: number };
  solvers: { spps: Spps; tcr: { bands_computed: boolean[] } };
}

const project = async (): Promise<ProjectFile> => JSON.parse(await m10.projectJson());

function ownCopy(from: string, folder: string): string {
  const dir = path.join(env('M11_P'), 'settings', folder);
  mkdirSync(dir, { recursive: true });
  const to = path.join(dir, path.basename(from));
  copyFileSync(from, to);
  return to;
}

/** Replaces the text of the field at `selector`, as a user would: click, select all, type, Enter. */
async function commit(selector: string, text: string): Promise<void> {
  const el = await $(selector);
  await el.waitForDisplayed({ timeout: 30_000 });
  await el.click();
  await browser.keys([CTRL, 'a']);
  await browser.keys(BACKSPACE);
  for (const ch of text) await browser.keys(ch);
  await browser.keys(ENTER);
  await m10.idle();
}

const inputValue = (sel: string) => browser.execute((s: string) => document.querySelector<HTMLInputElement>(s)?.value ?? null, sel);

const issueCodes = (scope: string) =>
  browser.execute((s: string) => [...document.querySelectorAll(`${s} [data-issue-code]`)].map((e) => e.getAttribute('data-issue-code') ?? ''), scope);

const blockersOf = async (sel: string): Promise<string[]> => ((await $(sel).getAttribute('data-blockers')) ?? '').split(' ').filter(Boolean);

/** Clicks a Run button and waits for the new run to end (as m11.simulate.e2e.ts does). */
async function runByClick(selector: string, timeout = 180_000): Promise<Row> {
  const before = new Set((await hook<Row[]>('runsRows')).map((r) => r.run));
  await clickSelector(selector);
  let fresh: Row | undefined;
  await browser.waitUntil(
    async () => {
      if ((await hook<unknown>('runState')) !== null) return false;
      fresh = (await hook<Row[]>('runsRows')).find((r) => !before.has(r.run) && r.status !== 'RUNNING');
      return fresh !== undefined;
    },
    { timeout, interval: 250, timeoutMsg: `no new run ended within ${timeout} ms after clicking ${selector}` },
  );
  await m10.idle();
  return fresh as Row;
}

/** `simulation@<attr>` and each `freq_enum/bfreq` of a config.xml, by regular expression. */
function readConfig(file: string): { sim: Record<string, string>; bands: [string, string][] } {
  const xml = readFileSync(file, 'utf8');
  const simTag = /<simulation\b([^>]*)>/.exec(xml);
  assert.ok(simTag, `no <simulation> in ${file}`);
  const sim: Record<string, string> = {};
  for (const m of simTag[1].matchAll(/(\w+)="([^"]*)"/g)) sim[m[1]] = m[2];
  const bands = [...xml.matchAll(/<bfreq\b[^>]*\bfreq="(\d+)"[^>]*\bdocalc="(\d)"/g)].map((m) => [m[1], m[2]] as [string, string]);
  return { sim, bands };
}

describe('PQ3 settings editor', () => {
  let box = '';

  before(async () => {
    await waitForHooks(['idle', 'openProject', 'runsRows', 'runState', 'setStep', 'setSolver', 'projectJson', 'undo', 'undoDepth', 'dirty']);
    box = ownCopy(path.join(env('M11_P'), 'box', 'box_run.simpa'), 'box');
  });

  it('pq3-settings-edit: typed particles, time step and a band off reach the run’s config.xml; a bad value is refused inline', async () => {
    await m10.openProject(box);
    await hook('setSolver', 'spps');
    await m10.setStep('simulate');
    await browser.waitUntil(async () => (await inputValue('[data-setting="particles"] input[data-field="particles"]')) !== null, {
      timeout: 30_000,
      timeoutMsg: 'the settings fields did not load',
    });
    const before = await project();
    const want = { particles: 12_000, stepMs: '5', step: 0.005, band: 1 };
    assert.notEqual(before.solvers.spps.particles_per_source, want.particles, 'the control: a value the box does not hold');
    assert.notEqual(before.solvers.spps.time_step_s, want.step);
    assert.equal(before.solvers.spps.bands_computed[want.band], true);

    // Refused inline, project unchanged: 0 particles, and 120 % humidity.
    const text0 = await m10.projectJson();
    await commit('[data-setting="particles"] input[data-field="particles"]', '0');
    assert.ok((await issueCodes('[data-setting="particles"]')).includes('PARTICLE_COUNT_INVALID'), 'the particle refusal is shown under its field');
    await commit('[data-setting="air"] input[data-field="humidity"]', '120');
    assert.ok((await issueCodes('[data-setting="air"]')).includes('ATMOSPHERE_INVALID'), 'the air refusal is shown under its field');
    assert.equal(await m10.projectJson(), text0, 'a refused edit changes nothing');
    assert.equal(await hook<boolean>('dirty'), false);
    await browser.keys(''); // Esc: the field shows the project's value again

    // The edits.
    await commit('[data-setting="particles"] input[data-field="particles"]', String(want.particles));
    await commit('[data-setting="time_step"] input[data-field="time_step"]', want.stepMs);
    await clickSelector(`[data-setting="bands"] [data-band="${want.band}"] input`);
    await m10.idle();
    await browser.waitUntil(async () => (await project()).solvers.spps.bands_computed[want.band] === false, {
      timeout: 10_000,
      timeoutMsg: 'the band click did not reach the project',
    });
    const after = await project();
    console.log(
      `pq3-settings-edit receipt: particles ${before.solvers.spps.particles_per_source} -> ${after.solvers.spps.particles_per_source}, time step ${before.solvers.spps.time_step_s} -> ${after.solvers.spps.time_step_s} s, bands_computed ${JSON.stringify(after.solvers.spps.bands_computed)}, steps readout '${await $('[data-part="steps"]').getText()}'`,
    );
    assert.equal(after.solvers.spps.particles_per_source, want.particles);
    assert.ok(Object.is(after.solvers.spps.time_step_s, want.step), `time step ${after.solvers.spps.time_step_s}`);
    assert.equal(await hook<boolean>('dirty'), true, 'the edits mark the project dirty');
    assert.equal(await hook<number>('undoDepth'), 3, 'three edits, three undo steps');
    assert.equal(await $('[data-part="steps"]').getText(), '400 steps');

    // Run: it saves first, then runs the file; its config.xml carries the edited values.
    const r = await runByClick('[data-part="run-panel"]');
    assert.equal(r.status, 'OK', `run ${r.run} ${r.status}`);
    assert.equal(await hook<boolean>('dirty'), false, 'Run saved the project first');
    const config = readConfig(path.join(path.dirname(box), 'runs', r.run, 'solve', 'config.xml'));
    console.log(`pq3-settings-edit receipt: ${r.run} config.xml nbparticules=${config.sim.nbparticules} pasdetemps=${config.sim.pasdetemps} docalc ${JSON.stringify(config.bands)}`);
    assert.equal(config.sim.nbparticules, String(want.particles));
    assert.equal(config.sim.pasdetemps, String(want.step));
    assert.deepEqual(
      config.bands.map(([, d]) => d),
      before.bands.frequencies_hz.map((_, i) => (i === want.band ? '0' : '1')),
    );
    assert.deepEqual(
      config.bands.map(([f]) => Number(f)),
      before.bands.frequencies_hz,
    );
  });

  it('pq3-settings-bands-off: every SPPS band off disables Run with NO_BAND_COMPUTED shown; TCR is not blocked', async () => {
    await hook('setSolver', 'spps');
    await m10.setStep('simulate');
    const start = await project();
    const depth0 = await hook<number>('undoDepth');
    const on = start.solvers.spps.bands_computed.map((b, i) => [b, i] as const).filter(([b]) => b);
    for (const [, i] of on) {
      await clickSelector(`[data-setting="bands"] [data-band="${i}"] input`);
      await m10.idle();
    }
    await browser.waitUntil(async () => (await project()).solvers.spps.bands_computed.every((b) => !b), {
      timeout: 10_000,
      timeoutMsg: 'not every band is off',
    });
    await browser.waitUntil(async () => (await blockersOf('[data-part="run-panel"]')).includes('NO_BAND_COMPUTED'), {
      timeout: 10_000,
      timeoutMsg: `Run does not carry NO_BAND_COMPUTED: ${await blockersOf('[data-part="run-panel"]')}`,
    });
    const panel = await blockersOf('[data-part="run-panel"]');
    const top = await blockersOf('[data-part="run"]');
    const failRows = await browser.execute(() =>
      [...document.querySelectorAll('[data-preflight][data-state="FAIL"]')].map((e) => [...e.querySelectorAll('[data-code]')].map((c) => c.getAttribute('data-code'))).flat(),
    );
    const inline = await issueCodes('[data-setting="bands"]');
    console.log(`pq3-settings-bands-off receipt: data-blockers panel '${panel.join(' ')}', top '${top.join(' ')}'; FAIL rows ${JSON.stringify(failRows)}; under the bands ${JSON.stringify(inline)}`);
    assert.equal(await $('[data-part="run-panel"]').isEnabled(), false, 'Run is disabled');
    assert.equal(await $('[data-part="run"]').isEnabled(), false, 'the top Run is disabled too');
    assert.ok(top.includes('NO_BAND_COMPUTED'));
    assert.ok(failRows.includes('NO_BAND_COMPUTED'), 'the refusal is on a FAIL row of Before running');
    assert.ok(inline.includes('NO_BAND_COMPUTED'), 'and under the bands');
    assert.ok((await hook<string[]>('runBlockers')).includes('NO_BAND_COMPUTED'));

    // The control: TCR computes its own bands, all on, so it may run.
    await clickSelector('[data-solver="tcr"]');
    await browser.waitUntil(async () => !(await blockersOf('[data-part="run-panel"]')).includes('NO_BAND_COMPUTED'), {
      timeout: 10_000,
      timeoutMsg: 'TCR is blocked by the SPPS bands',
    });
    assert.deepEqual(await blockersOf('[data-part="run-panel"]'), [], 'TCR may run');
    await clickSelector('[data-solver="spps"]');

    // Undo gives every band back.
    while ((await hook<number>('undoDepth')) > depth0) await m10.undo();
    assert.deepEqual((await project()).solvers.spps.bands_computed, start.solvers.spps.bands_computed);
    await browser.waitUntil(async () => (await blockersOf('[data-part="run-panel"]')).length === 0, {
      timeout: 10_000,
      timeoutMsg: 'Run is still blocked after the undos',
    });
  });

  it('pq3-settings-preset: a confirmed band preset moves the project onto its bands, and one undo restores it exactly', async () => {
    await hook('setSolver', 'spps');
    await m10.setStep('simulate');
    const text0 = await m10.projectJson();
    const before = JSON.parse(text0) as ProjectFile;
    const depth0 = await hook<number>('undoDepth');
    await $('select[data-field="band-preset"]').selectByAttribute('value', 'third_octave-100-5000');
    await clickSelector('[data-part="reband"]');
    await $('[data-part="reband-confirm"]').waitForDisplayed({ timeout: 10_000 });
    assert.equal(JSON.stringify(JSON.parse(await m10.projectJson()).bands), JSON.stringify(before.bands), 'nothing changes before the confirm');
    await clickSelector('[data-part="reband-apply"]');
    await m10.idle();
    await browser.waitUntil(async () => (await project()).bands.kind === 'third_octave', { timeout: 10_000, timeoutMsg: 'the preset did not apply' });
    const after = await project();
    console.log(
      `pq3-settings-preset receipt: bands ${before.bands.kind} ${JSON.stringify(before.bands.frequencies_hz)} -> ${after.bands.kind} ${JSON.stringify(after.bands.frequencies_hz)}; SPPS computed ${JSON.stringify(after.solvers.spps.bands_computed)}; undo depth ${depth0} -> ${await hook<number>('undoDepth')}`,
    );
    assert.deepEqual(after.bands.frequencies_hz, [100, 125, 160, 200, 250, 315, 400, 500, 630, 800, 1000, 1250, 1600, 2000, 2500, 3150, 4000, 5000]);
    assert.equal(after.solvers.spps.bands_computed.length, 18);
    assert.equal(after.solvers.tcr.bands_computed.length, 18);
    assert.equal(await hook<number>('undoDepth'), depth0 + 1, 'one undo step');
    await m10.undo();
    assert.equal(await m10.projectJson(), text0, 'one undo gives back the project text exactly');
    assert.equal(await hook<number>('undoDepth'), depth0);
  });
});
