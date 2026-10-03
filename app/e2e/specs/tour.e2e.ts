// The app tour, to be recorded as a video: the corrected Elmia concert hall from the first frame
// to the last, solved for real at the new-project defaults, then its results, its surface map and
// its particles. Not a gate id: it asserts only that each step reached the state it claims.
// Run by `tools/gates/m11.ps1 -Only e2e -Spec tour -ScreensDir <dir>`, which builds the release
// app, stages the verified solvers and passes M11_SCREENS.
//
// It writes no picture. Into M11_SCREENS: progress.log, one line per thing shown, with the numbers
// read from the page; `tour start` is the first line of a pass and `tour end` the last (a screen
// recorder keys on them). The hall is copied to `<M11_SCREENS>\work-<stamp>\elmia`, so its runs
// land there (B:), never in the repository.
//
// The hall: tests/fixtures/rooms/elmia_corrected.simpa (the corrected mesh with tutorial 2's
// materials, three sources, six receivers and its two cutting planes, which are the maps). Never
// the raw upstream elmia.ply. The Simulate step's fields are typed back to the new-project
// defaults (150,000 particles, 10 s, 1 ms, energetic; core's SppsSettings::for_bands), the
// project's own bands kept, particles saved 100 for the playback. The extinction exponent (upstream's
// trans_epsilon) has no field in the editor, so it is set to its default 7 through the project's
// settings op, and the log says so. The window is left as it is (no resize, move or focus).
import { strict as assert } from 'node:assert';
import { appendFileSync, copyFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { clickSelector } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { need, showTab } from '../lib/m11.ts';
import { readPbin } from '../lib/m12files.ts';
import { env } from '../lib/types.ts';

const CTRL = '';
const ENTER = '';
const BACKSPACE = '';
/** The pause between steps, so a person watching can follow. */
const PACE_MS = 4_000;
/** The pause between the parts of one step. */
const BEAT_MS = 1_800;
/** A first run longer than this skips the variant (step 9). */
const VARIANT_IF_UNDER_S = 300;

const OUT = () => need('M11_SCREENS');
const STAMP = new Date(Date.now() - new Date().getTimezoneOffset() * 60_000).toISOString().replace(/[-:]/g, '').replace('T', '-').slice(0, 15);
const WORK = () => path.join(OUT(), `work-${STAMP}`);
const repo = (rel: string) => path.join(env('M11_REPO'), rel);

// ---- the log ---------------------------------------------------------------------------------

function progress(line: string): void {
  const d = new Date();
  const z = (n: number) => String(n).padStart(2, '0');
  const t = `${d.getFullYear()}-${z(d.getMonth() + 1)}-${z(d.getDate())} ${z(d.getHours())}:${z(d.getMinutes())}:${z(d.getSeconds())}`;
  mkdirSync(OUT(), { recursive: true });
  try {
    appendFileSync(path.join(OUT(), 'progress.log'), `${t} ${line}\n`);
  } catch {
    // A reader holding the log must not stop the tour.
  }
  console.log(`tour: ${line}`);
}

const pause = (ms: number) => browser.pause(ms);

/** The visible text of the first element matching `sel`, whitespace collapsed ('' if none). */
const text = (sel: string): Promise<string> =>
  browser.execute((s: string) => ((document.querySelector(s) as HTMLElement | null)?.innerText ?? '').replace(/\s+/g, ' ').trim(), sel);

/** Every match's visible text, whitespace collapsed. */
const texts = (sel: string): Promise<string[]> =>
  browser.execute((s: string) => [...document.querySelectorAll<HTMLElement>(s)].map((e) => (e.innerText ?? '').replace(/\s+/g, ' ').trim()), sel);

/** Scrolls the element into view smoothly (its scroll containers included), then lets it settle. */
async function reveal(sel: string, settle = 1_200): Promise<void> {
  await $(sel).waitForExist({ timeout: 30_000 });
  await browser.execute((s: string) => document.querySelector(s)?.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'start' }), sel);
  await pause(settle);
}

/** Replaces a field's text as a user does: click, select all, type, Enter. */
async function commit(selector: string, value: string): Promise<void> {
  const el = await $(selector);
  await el.waitForDisplayed({ timeout: 30_000 });
  await el.scrollIntoView({ block: 'center' });
  await el.click();
  await browser.keys([CTRL, 'a']);
  await browser.keys(BACKSPACE);
  for (const ch of value) {
    await browser.keys(ch);
    await pause(60);
  }
  await browser.keys(ENTER);
  await m10.idle();
}

// ---- the 3D view -----------------------------------------------------------------------------

interface Cam {
  view: string;
  position: number[];
  target: number[] | null;
}
const camText = async () => {
  const c = await hook<Cam>('cameraState');
  return `${c.view} camera at (${c.position.map((v) => v.toFixed(1)).join(', ')})${c.target ? ` looking at (${c.target.map((v) => v.toFixed(1)).join(', ')})` : ''}`;
};

async function canvasCentre(): Promise<{ x: number; y: number; w: number; h: number }> {
  const c = await browser.execute(() => {
    const r = document.querySelector('canvas.viewport-canvas')?.getBoundingClientRect();
    return r ? { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + r.height / 2), w: r.width, h: r.height } : null;
  });
  assert.ok(c, 'no 3D view canvas');
  return c;
}

/** A slow orbit: a real pointer drag of (dx, dy) px across the 3D view, over `ms`. */
async function orbit(dx: number, dy: number, ms: number): Promise<void> {
  const c = await canvasCentre();
  const n = Math.max(10, Math.round(ms / 33));
  const x0 = c.x - dx / 2;
  const y0 = c.y - dy / 2;
  let a = browser
    .action('pointer', { parameters: { pointerType: 'mouse' } })
    .move({ x: Math.round(x0), y: Math.round(y0), origin: 'viewport', duration: 0 })
    .down({ button: 0 });
  for (let i = 1; i <= n; i++) a = a.move({ x: Math.round(x0 + (dx * i) / n), y: Math.round(y0 + (dy * i) / n), origin: 'viewport', duration: 33 });
  await a.up({ button: 0 }).perform();
  await browser
    .action('pointer', { parameters: { pointerType: 'mouse' } })
    .move({ x: 2, y: 2, origin: 'viewport', duration: 0 })
    .perform();
}

/** The Frame model tool, as a click does. */
async function frameModel(): Promise<void> {
  await clickSelector('[data-tool="frame"]');
  await m10.idle();
}

// ---- the project -----------------------------------------------------------------------------

interface Named {
  id: string;
  name: string;
}
interface ProjectFile {
  surface_groups: (Named & { material: string })[];
  materials: (Named & { absorption: number[] })[];
  sources: (Named & { position: number[]; power: { global_db: number } })[];
  point_receivers: (Named & { position: number[] })[];
  surface_receivers: (Named & { shape: { kind: string; resolution_m?: number } })[];
  geometry: { faces: unknown[] };
  bands: { kind: string; frequencies_hz: number[] };
  solvers: { spps: Record<string, unknown> & { bands_computed: boolean[] } };
  variants: (Named & { overrides: { group: string; material: string }[] })[];
}
interface Row {
  run: string;
  number: number;
  status: string;
  elapsed_s?: string | null;
}

const project = async (): Promise<ProjectFile> => JSON.parse(await m10.projectJson());
const xyz = (p: number[]) => `(${p.map((v) => String(v)).join(', ')})`;
/** The bands SPPS computes, as indices into the project's bands. */
const computed = (p: ProjectFile) => p.solvers.spps.bands_computed.flatMap((on, i) => (on ? [i] : []));

/** Clicks the Run button on the Simulate step and waits for that run to end; logs its progress
 * every 15 s and its wall time from the click to the row's end. */
async function runFromButton(label: string): Promise<{ row: Row; wallS: number }> {
  const before = new Set((await hook<Row[]>('runsRows')).map((r) => r.run));
  const btn = await $('[data-part="run-panel"]');
  await btn.waitForEnabled({ timeout: 60_000, timeoutMsg: `the Run button stayed disabled: ${await btn.getAttribute('data-blockers')}` });
  await btn.scrollIntoView({ block: 'center' });
  const t0 = Date.now();
  await btn.click();
  progress(`${label}: Run clicked (SPPS)`);
  let fresh: Row | undefined;
  let lastLog = 0;
  await browser.waitUntil(
    async () => {
      const st = await hook<{ progressText: string; stage: string | null } | null>('runState');
      if (st !== null) {
        if (Date.now() - lastLog >= 15_000) {
          lastLog = Date.now();
          const live = (await texts('.console-line.PROGRESS .text')).slice(-1)[0] ?? '';
          progress(`${label}: ${Math.round((Date.now() - t0) / 1000)} s, stage ${st.stage ?? '-'}, progress "${st.progressText}", Console live line "${live}"`);
        }
        return false;
      }
      fresh = (await hook<Row[]>('runsRows')).find((r) => !before.has(r.run) && r.status !== 'RUNNING');
      return fresh !== undefined;
    },
    { timeout: 3_000_000, interval: 500, timeoutMsg: `${label}: no run ended within 50 min` },
  );
  const wallS = (Date.now() - t0) / 1000;
  await m10.idle();
  const row = fresh as Row;
  progress(`${label}: run ${row.number} (${row.run}) ended ${row.status}; wall time ${wallS.toFixed(1)} s from the click to the row's end; the row's own elapsed ${row.elapsed_s ?? '-'}`);
  assert.equal(row.status, 'OK', `${label}: run ${row.run} ended ${row.status}`);
  return { row, wallS };
}

// ---- results ---------------------------------------------------------------------------------

const PANEL = '[data-dock-panel="acoustics"]';

async function acousticsShows(run: string): Promise<void> {
  await browser.waitUntil(
    async () => {
      const v = await hook<{ state: string; run: string | null } | null>('acousticsView');
      return v !== null && v.run === run && v.state === 'ready';
    },
    { timeout: 120_000, interval: 250, timeoutMsg: `the Acoustics tab did not show ${run} ready` },
  );
}

async function pick(control: string, value: string): Promise<void> {
  const sel = await $(`${PANEL} select[data-control="${control}"]`);
  await sel.waitForExist({ timeout: 10_000 });
  await sel.selectByAttribute('value', value);
  await browser.waitUntil(async () => (await sel.getValue()) === value, { timeout: 10_000 });
  await m10.idle();
}

const options = (control: string): Promise<{ value: string; text: string }[]> =>
  browser.execute(
    (s: string) => [...document.querySelectorAll<HTMLOptionElement>(s)].map((o) => ({ value: o.value, text: (o.textContent ?? '').trim() })),
    `${PANEL} select[data-control="${control}"] option`,
  );

/** A table's body rows as a reader takes them: each row's head, then each cell's value (or its
 * refusal code), with the column heads. */
const tableRows = (part: string): Promise<string[]> =>
  browser.execute((sel: string) => {
    const t = document.querySelector(sel);
    if (!t) return [];
    const heads = [...(t.querySelector('thead tr')?.children ?? [])].map((h) => (h as HTMLElement).innerText.replace(/\s+/g, ' ').trim());
    return [...t.querySelectorAll('tbody tr, tfoot tr')].map((tr) => {
      const cells = [...tr.children] as HTMLElement[];
      const head = cells[0]?.innerText.replace(/\s+/g, ' ').trim() ?? '';
      const rest = cells.slice(1).map((c, i) => {
        const h = heads[i + 1] ?? '';
        if (c.getAttribute('data-status') === 'refused') return `${h} refused ${c.querySelector('[data-refusal]')?.getAttribute('data-refusal') ?? ''}`;
        const v = (c.querySelector('.ac-value') as HTMLElement | null)?.innerText ?? c.innerText;
        return `${h} ${v.replace(/\s+/g, ' ').trim()}`;
      });
      return `${head}: ${rest.join(', ')}`;
    });
  }, `${PANEL} [data-part="${part}"]`);

interface MapState {
  run: string;
  path: string;
  bandHz: number | null;
  kind: 'level' | 'diff';
  faces: number;
  steps: number;
  step: number;
  legend: { lo: string; mid: string; hi: string; title?: string } | null;
  baseline: string | null;
}
interface ParticleState {
  run: string;
  bandHz: number;
  step: number;
  rendered: number;
  particles: number;
}

async function mapOf(run: string, want: { bandHz?: number | null; kind?: 'level' | 'diff' } = {}): Promise<MapState> {
  let last: MapState | null = null;
  await browser.waitUntil(
    async () => {
      last = await hook<MapState | null>('m12Map');
      return last !== null && last.run === run && (want.bandHz === undefined || last.bandHz === want.bandHz) && (want.kind === undefined || last.kind === want.kind);
    },
    { timeout: 240_000, interval: 250, timeoutMsg: `the map did not show ${run} ${JSON.stringify(want)}: ${JSON.stringify(last)}` },
  );
  return last as unknown as MapState;
}

const legendText = async () => `${await text('[data-part="map-legend"] .vp-legend-title')} ${await text('[data-legend="lo"]')} / ${await text('[data-legend="mid"]')} / ${await text('[data-legend="hi"]')}`;

/** Moves the time slider from `from` to `to` over `ms`, as a hand dragging it would (eased: slower
 * at the start, where the sound spreads fastest); `at` is called at each of `marks` (fractions). */
async function sweep(from: number, to: number, ms: number, marks: number[] = [], at?: (step: number) => Promise<void>): Promise<void> {
  const t0 = Date.now();
  const left = [...marks].sort((a, b) => a - b);
  for (;;) {
    const f = Math.min(1, (Date.now() - t0) / ms);
    const s = Math.round(from + (to - from) * Math.pow(f, 1.6));
    await hook('m12SetStep', s);
    while (left.length && f >= left[0]) {
      left.shift();
      if (at) await at(s);
    }
    if (f >= 1) break;
    await pause(30);
  }
}

async function clickVariant(id: string): Promise<void> {
  const t = await $(`[data-part="variants"] [data-variant="${id}"]`);
  await t.waitForExist({ timeout: 10_000 });
  await t.click();
  await browser.waitUntil(async () => (await t.getAttribute('aria-selected')) === 'true', { timeout: 10_000 });
  await m10.idle();
}

// ---- the tour --------------------------------------------------------------------------------

describe('Tour: the Elmia hall in I-Simpa Night Mode', function () {
  this.timeout(3_600_000);
  let hall = '';
  let runA = '';
  let wallA = 0;
  let runB = '';
  /** The last step at which a saved particle is alive (1 kHz), from run A's own .pbin. */
  let decayEnd = 0;
  let variantId = '';

  before(async () => {
    await browser.setTimeout({ script: 120_000 });
    mkdirSync(path.join(WORK(), 'elmia'), { recursive: true });
    progress(`tour start; work folder ${WORK()}`);
    await waitForHooks([
      'idle', 'openProject', 'edit', 'projectJson', 'setStep', 'selection', 'cameraState', 'runState', 'runsRows', 'selectRun', 'setSolver', 'dockTab', 'acousticsView',
      'm12Map', 'm12SetStep', 'm12Particles',
    ]);
    hall = path.join(WORK(), 'elmia', 'elmia_corrected.simpa');
    copyFileSync(repo('tests/fixtures/rooms/elmia_corrected.simpa'), hall);
  });

  beforeEach(async () => {
    await pause(PACE_MS);
  });

  afterEach(function () {
    const t = this.currentTest;
    if (t?.state === 'failed') progress(`FAILED at "${t.title}": ${String(t.err?.message ?? '').replace(/\s+/g, ' ').slice(0, 400)}`);
  });

  after(() => {
    progress('tour end');
  });

  it('1: the app starts, the corrected Elmia hall opens, and the view turns around it', async () => {
    progress(`step 1: the app at start: Geometry step "${await text('[data-step="geometry"] [data-part="sub"]')}", no project open`);
    await pause(BEAT_MS);
    await m10.openProject(hall);
    await m10.setStep('geometry');
    await showTab('console');
    await frameModel();
    const p = await project();
    progress(
      `step 1: opened tests/fixtures/rooms/elmia_corrected.simpa (copied to ${hall}): ${p.geometry.faces.length} faces, ${p.surface_groups.length} surface groups; status bar "${await text('.statusbar [data-part="model-fact"]')}"; model check "${await text('[data-part="check-verdict"]')}"; ${await camText()}`,
    );
    await pause(3_000);
    const moves: [string, number, number, number][] = [
      ['turn left round the hall', 360, 0, 4_500],
      ['tilt to look down into it', 0, 160, 3_000],
      ['turn on round the far side', 420, 0, 5_000],
      ['tilt back toward eye level', 0, -110, 2_500],
    ];
    for (const [what, dx, dy, ms] of moves) {
      await orbit(dx, dy, ms);
      progress(`step 1: orbit, ${what}: ${await camText()}`);
      await pause(BEAT_MS);
    }
    await clickSelector('[data-view="plan"]');
    await m10.idle();
    progress(`step 1: the Plan view, the hall from above: ${await camText()}`);
    await pause(3_500);
    await clickSelector('[data-view="perspective"]');
    await frameModel();
    await orbit(-240, 70, 3_500);
    progress(`step 1: back to Perspective, framed and turned: ${await camText()}`);
    await pause(BEAT_MS);
  });

  it('2: the Scene panel’s surface groups, then each group’s material', async () => {
    const p = await project();
    const bands = computed(p);
    const hz = p.bands.frequencies_hz;
    progress(`step 2: the Scene panel lists ${p.surface_groups.length} surface groups: ${p.surface_groups.map((g) => g.name).join(', ')}`);
    for (const g of p.surface_groups) {
      await reveal(`.scene [data-entity="surface_group:${g.id}"]`, 200);
      await clickSelector(`.scene [data-entity="surface_group:${g.id}"]`);
      await m10.idle();
      const sel = await hook<{ faces: number[]; groups: string[] }>('selection');
      progress(`step 2: group "${g.name}" picked in the Scene panel: ${sel.faces.length} faces highlighted in the 3D view`);
      await pause(1_400);
    }
    await m10.setStep('materials');
    await $('[data-materials-grid]').waitForDisplayed({ timeout: 30_000 });
    progress(
      `step 2: the Materials step: the grid lists the project's ${p.materials.length} materials; SPPS computes ${bands.length} of the project's ${hz.length} ${p.bands.kind.replace('_', '-')} bands (${bands.map((i) => hz[i]).join(', ')} Hz)`,
    );
    await pause(BEAT_MS);
    for (const g of p.surface_groups) {
      await reveal(`.scene [data-entity="surface_group:${g.id}"]`, 200);
      await clickSelector(`.scene [data-entity="surface_group:${g.id}"]`);
      await m10.idle();
      const m = p.materials.find((x) => x.id === g.material);
      progress(`step 2: group "${g.name}" uses material "${m?.name}", absorption ${bands.map((i) => `${hz[i]} Hz ${m?.absorption[i]}`).join(', ')}`);
      await pause(1_600);
    }
  });

  it('3: the sources and receivers in the hall', async () => {
    const p = await project();
    await m10.setStep('sources');
    progress(`step 3: the Sources & receivers step: ${p.sources.length} sources, ${p.point_receivers.length} receivers, ${p.surface_receivers.length} surface receivers`);
    for (const s of p.sources) {
      await reveal(`.scene [data-entity="source:${s.id}"]`, 200);
      await clickSelector(`.scene [data-entity="source:${s.id}"]`);
      await $('[data-field="position.x"]').waitForDisplayed({ timeout: 30_000 });
      progress(`step 3: source ${s.name} picked: position ${xyz(s.position)} m, ${s.power.global_db} dB`);
      await pause(BEAT_MS);
    }
    for (const r of p.point_receivers) {
      await reveal(`.scene [data-entity="point_receiver:${r.id}"]`, 200);
      await clickSelector(`.scene [data-entity="point_receiver:${r.id}"]`);
      await $('[data-field="position.x"]').waitForDisplayed({ timeout: 30_000 });
      progress(`step 3: receiver ${r.name} picked: position ${xyz(r.position)} m`);
      await pause(1_500);
    }
    for (const r of p.surface_receivers) {
      await reveal(`.scene [data-entity="surface_receiver:${r.id}"]`, 600);
      progress(`step 3: surface receiver "${r.name}": a ${r.shape.kind.replace('_', ' ')}${r.shape.resolution_m ? ` with ${r.shape.resolution_m} m cells` : ''}, listed in the Scene panel; these are the hall's maps`);
    }
    assert.ok(p.surface_receivers.length > 0, 'the hall has no surface receiver, so there would be no map');
    await pause(BEAT_MS);
    // TOUR_ONLY_SOURCE=<name>: the other sources switched off in the Scene panel, as a user does.
    // With several sources on, every decay parameter of the summed echogram is refused
    // (several_sources: ISO 3382 defines them per source and receiver), and the Acoustics tab
    // shows only the summed values; with one source on, they are that source's.
    const only = process.env.TOUR_ONLY_SOURCE;
    if (only) {
      assert.ok(p.sources.some((s) => s.name === only), `no source named ${only}`);
      for (const s of p.sources.filter((x) => x.name !== only)) {
        const sw = await $(`.scene [data-source-toggle="${s.id}"]`);
        await sw.scrollIntoView({ block: 'center' });
        if ((await sw.getAttribute('data-state')) === 'on') {
          await sw.click();
          await m10.idle();
          await browser.waitUntil(async () => (await sw.getAttribute('data-state')) === 'off', { timeout: 10_000, timeoutMsg: `${s.name} did not switch off` });
        }
        progress(`step 3: source ${s.name} switched off in the Scene panel, so the run has one source (${only}) and its decay parameters are per source`);
        await pause(1_500);
      }
      const on = (await project()).sources.filter((s) => (s as unknown as { enabled: boolean }).enabled).map((s) => s.name);
      assert.deepEqual(on, [only]);
    }
  });

  it('4: the Simulate settings, at the defaults, particles saved 100', async () => {
    await hook('setSolver', 'spps');
    await m10.setStep('simulate');
    await $('[data-setting="particles"] input[data-field="particles"]').waitForDisplayed({ timeout: 30_000 });
    const was = (await project()).solvers.spps;
    progress(
      `step 4: the Simulate step, SPPS: the hall as tutorial 2 left it: ${was.particles_per_source} particles, ${was.duration_s} s, ${Number(was.time_step_s) * 1000} ms, ${was.method}, extinction exponent ${was.extinction_exponent}; the tour types the new-project defaults back`,
    );
    await pause(BEAT_MS);
    const fields: [string, string, string][] = [
      ['particles', '150000', 'particles per source and band'],
      ['particles_saved', '100', 'particles saved for playback'],
      ['duration', '10', 'duration, s'],
      ['time_step', '1', 'time step, ms'],
    ];
    for (const [field, value, what] of fields) {
      await commit(`[data-setting="${field}"] input[data-field="${field}"]`, value);
      const hint = field === 'time_step' ? ` (${await text('[data-part="steps"]')})` : field === 'particles_saved' ? ` (${await text('[data-part="pbin-size"]')})` : '';
      progress(`step 4: typed ${what} = ${value}${hint}`);
      await pause(1_200);
    }
    await clickSelector('[data-method="energetic"]');
    await m10.idle();
    for (const f of ['sound_maps_per_band', 'echogram_per_source']) {
      const box = await $(`input[data-field="${f}"]`);
      await box.scrollIntoView({ block: 'center' });
      if (!(await box.isSelected())) {
        await box.click();
        await m10.idle();
        progress(`step 4: ticked ${f.replace(/_/g, ' ')} (on by default)`);
        await pause(1_000);
      }
    }
    // No field for the extinction exponent: its default through the project's settings op.
    const p0 = await project();
    if (p0.solvers.spps.extinction_exponent !== 7) {
      p0.solvers.spps.extinction_exponent = 7;
      const out = await m10.edit({ op: 'set_solver_settings', settings: p0.solvers });
      assert.ok(out.applied, `the extinction exponent was refused: ${JSON.stringify(out.refusals)}`);
      progress('step 4: extinction exponent (upstream trans_epsilon) set to its default 7 through the project settings op: the editor has no field for it');
    }
    const s = (await project()).solvers.spps;
    const want = { particles_per_source: 150000, particles_saved: 100, duration_s: 10, time_step_s: 0.001, method: 'energetic', extinction_exponent: 7, random_seed: 0 };
    for (const [k, v] of Object.entries(want)) assert.equal(s[k], v, `spps.${k}`);
    const p = await project();
    await reveal('[data-props-step="simulate"] [data-setting="bands"]', 1_500);
    progress(
      `step 4: settings now: ${s.particles_per_source} particles per source and band, ${s.particles_saved} saved, ${s.duration_s} s, ${Number(s.time_step_s) * 1000} ms, ${s.method}, extinction exponent ${s.extinction_exponent}, seed ${s.random_seed}, echogram per source ${s.echogram_per_source}; bands "${await text('[data-props-step="simulate"] [data-setting="bands"] [data-part="bands-summary"]')}" (${computed(p).map((i) => p.bands.frequencies_hz[i]).join(', ')} Hz); ${p.sources.length} sources`,
    );
    progress(`step 4: maps: the hall's ${p.surface_receivers.length} cutting planes (${p.surface_receivers.map((r) => r.name).join(', ')}) are its surface receivers; none added`);
    await pause(BEAT_MS);
  });

  it('5: SPPS solves the hall to completion, the Console streaming', async () => {
    await showTab('console');
    const { row, wallS } = await runFromButton('step 5');
    runA = row.run;
    wallA = wallS;
    await pause(BEAT_MS);
    progress(`step 5: the Console after the run: counts "${await text(`[data-run-counts="${runA}"]`)}"`);
    await showTab('runs');
    await hook('selectRun', runA);
    await $(`[data-run-row="${runA}"]`).waitForExist({ timeout: 30_000 });
    progress(`step 5: the Runs tab: "${(await text(`[data-run-row="${runA}"]`)).slice(0, 300)}"`);
    const pbin = readPbin(path.join(path.dirname(hall), 'runs', runA, 'solve', 'Particles', '1000', 'particles.pbin'));
    decayEnd = pbin.alive.reduce((a, n, s) => (n > 0 ? s : a), 0);
    progress(`step 5: run A's 1 kHz particle file: ${pbin.particles} particles saved, the last alive at step ${decayEnd} of ${pbin.stepsMax}`);
  });

  it('6: Results, the Acoustics tab: receivers, RT against DIN 18041, Sabine and Eyring, absorption, decays', async () => {
    await m10.setStep('results');
    await hook('selectRun', runA);
    await showTab('acoustics');
    await acousticsShows(runA);
    progress(`step 6: the Results step, Acoustics tab, run ${runA}: "${await text(`${PANEL} .ac-head`)}"`);
    await reveal(`${PANEL} .ac-receivers`);
    for (const o of await options('band')) {
      await pick('band', o.value);
      const rows = await tableRows('receivers-table');
      progress(`step 6: receivers table at ${o.text}: ${rows.join(' | ').slice(0, 900)}`);
      await pause(2_200);
    }
    await pick('band', (await options('band')).find((o) => o.text === '1 kHz')?.value ?? '0');
    await reveal(`${PANEL} .ac-rt`);
    const groups = await options('din-group');
    const music = groups.find((g) => g.value === 'A1');
    if (music) await pick('din-group', music.value);
    progress(`step 6: reverberation time per band for ${await text(`${PANEL} select[data-control="receiver"] option:checked`)} against DIN 18041: "${await text(`${PANEL} [data-part="din-target"]`)}"`);
    progress(`step 6: RT table: ${(await tableRows('rt-table')).join(' | ')}`);
    await pause(3_500);
    await reveal(`${PANEL} .ac-classical`);
    progress(`step 6: Sabine / Eyring: ${(await tableRows('classical-table')).join(' | ')}`);
    await pause(3_500);
    await reveal(`${PANEL} .ac-absorption`);
    progress(`step 6: absorption by surface group (S·α, m²): ${(await tableRows('absorption-table')).join(' | ').slice(0, 1200)}`);
    await pause(3_500);
    const receivers = await options('receiver');
    const two = [receivers[0], receivers[Math.min(receivers.length - 1, 3)]];
    for (const r of two) {
      await pick('receiver', r.value);
      await reveal(`${PANEL} .ac-decay`);
      const drawn = await $(`${PANEL} .ac-decay canvas`).isExisting();
      progress(`step 6: decay curve of ${await text(`${PANEL} .ac-decay .ac-sub`)}: ${drawn ? 'drawn' : 'not drawn'}`);
      await pause(3_000);
    }
    await reveal(`${PANEL} .ac-rt`);
  });

  it('7: the surface energy map on the hall, through the decay, in three bands', async () => {
    await $('[data-part="results-viewport"]').waitForExist({ timeout: 120_000 });
    const t0 = Date.now();
    await mapOf(runA);
    const kinds = await texts('[data-map-kind]');
    const title = await text('.vp-map-title');
    progress(`step 7: the map loaded in ${((Date.now() - t0) / 1000).toFixed(1)} s; maps offered: ${kinds.length ? kinds.join(', ') : title}`);
    await clickSelector('[data-map-band="1000"]');
    let m = await mapOf(runA, { bandHz: 1000 });
    await hook('m12SetStep', 0);
    await frameModel();
    await orbit(-200, 110, 3_000);
    progress(`step 7: 1 kHz, ${m.faces} map cells x ${m.steps} steps, ${await camText()}; legend ${await legendText()}`);
    await pause(BEAT_MS);
    const end = Math.min(m.steps - 1, Math.max(400, decayEnd));
    await sweep(0, end, 15_000, [0, 0.25, 0.5, 0.75, 1], async (s) => progress(`step 7: time slider at step ${s}, ${await text('[data-part="anim-time"]')} after the source starts`));
    await pause(BEAT_MS);
    const mid = Math.round(end * 0.25);
    for (const hz of [500, 2000]) {
      await clickSelector(`[data-map-band="${hz}"]`);
      const t1 = Date.now();
      m = await mapOf(runA, { bandHz: hz });
      await hook('m12SetStep', mid);
      progress(`step 7: band changed to ${hz} Hz (loaded in ${((Date.now() - t1) / 1000).toFixed(1)} s), at ${await text('[data-part="anim-time"]')}; legend ${await legendText()}`);
      await sweep(mid, Math.round(end * 0.6), 4_000);
      await pause(BEAT_MS);
    }
    await clickSelector('[data-map-band="1000"]');
    await mapOf(runA, { bandHz: 1000 });
  });

  it('8: particle playback through the hall, until the particles die out', async () => {
    let ps: ParticleState | null = null;
    await browser.waitUntil(async () => (ps = await hook<ParticleState | null>('m12Particles')) !== null && ps.run === runA, { timeout: 120_000, timeoutMsg: 'no particles loaded for run A' });
    await hook('m12SetStep', 0);
    await frameModel();
    await orbit(260, 40, 3_000);
    const p0 = await hook<ParticleState>('m12Particles');
    progress(`step 8: particle playback, ${await text('[data-part="particles-band"]')}: ${p0.particles} saved particles, ${p0.rendered} drawn at step 0; ${await camText()}`);
    await clickSelector('[data-part="anim-play"]');
    await pause(4_000);
    await clickSelector('[data-part="anim-play"]');
    const p1 = await hook<ParticleState>('m12Particles');
    progress(`step 8: Play for 4 s (30 steps a second) reached step ${p1.step} (${await text('[data-part="anim-time"]')}): ${p1.rendered} particles drawn; the tour then moves the time slider faster`);
    const end = Math.max(p1.step + 100, Math.min(decayEnd + 20, ((await hook<MapState | null>('m12Map'))?.steps ?? decayEnd + 21) - 1));
    await sweep(p1.step, end, 17_000, [0.2, 0.4, 0.6, 0.8, 1], async () => {
      const now = await hook<ParticleState>('m12Particles');
      progress(`step 8: ${await text('[data-part="anim-time"]')} (step ${now.step}): ${now.rendered} of ${now.particles} particles still alive and drawn`);
    });
    await pause(3_000);
  });

  it('9: a variant with an absorbing ceiling, its run, the A/B switch and the difference map', async function () {
    if (wallA >= VARIANT_IF_UNDER_S) {
      progress(`step 9: SKIPPED: the first run took ${wallA.toFixed(1)} s, not under ${VARIANT_IF_UNDER_S} s`);
      return;
    }
    const p = await project();
    const ceiling = p.surface_groups.find((g) => g.name === 'ceiling');
    const absorber = p.materials.find((m) => m.name === 'rearaudience');
    assert.ok(ceiling && absorber, 'the hall has a ceiling group and the rearaudience material');
    const was = p.materials.find((m) => m.id === ceiling.material);
    const hz = p.bands.frequencies_hz;
    const bands = computed(p);
    await clickSelector('[data-part="variant-add"]');
    await m10.idle();
    const v = (await project()).variants.slice(-1)[0];
    assert.ok(v, 'the + added a variant');
    variantId = v.id;
    const tab = await $(`[data-part="variants"] [data-variant="${variantId}"]`);
    await tab.doubleClick();
    const name = await $('[data-part="variant-name"]');
    await name.waitForDisplayed({ timeout: 10_000 });
    await browser.keys([CTRL, 'a']);
    for (const ch of 'Absorbing ceiling') {
      await browser.keys(ch);
      await pause(50);
    }
    await browser.keys(ENTER);
    await m10.idle();
    progress(`step 9: a variant added with + and renamed "${(await project()).variants.find((x) => x.id === variantId)?.name}"; it is the active one`);
    await pause(BEAT_MS);
    await m10.setStep('materials');
    await clickSelector(`.scene [data-entity="surface_group:${ceiling.id}"]`);
    await m10.idle();
    await pause(1_200);
    await clickSelector(`[data-material-option="${absorber.id}"]`);
    await m10.idle();
    const ov = (await project()).variants.find((x) => x.id === variantId)?.overrides ?? [];
    assert.deepEqual(ov, [{ group: ceiling.id, material: absorber.id }]);
    progress(
      `step 9: in the variant the ceiling takes "${absorber.name}" (absorption ${bands.map((i) => `${hz[i]} Hz ${absorber.absorption[i]}`).join(', ')}) instead of "${was?.name}" (${bands.map((i) => was?.absorption[i]).join(', ')}); the baseline is unchanged`,
    );
    await pause(BEAT_MS);
    await m10.setStep('simulate');
    await showTab('console');
    const { row } = await runFromButton('step 9 variant');
    runB = row.run;
    await m10.setStep('results');
    await showTab('acoustics');
    const t30 = async () => (await tableRows('rt-table')).join(' | ');
    for (const [id, run, label] of [['baseline', runA, 'Baseline'], [variantId, runB, 'Absorbing ceiling'], ['baseline', runA, 'Baseline'], [variantId, runB, 'Absorbing ceiling']] as const) {
      await clickVariant(id);
      await acousticsShows(run);
      await reveal(`${PANEL} .ac-rt`, 600);
      progress(`step 9: variant switch on "${label}": the tab shows run ${run}; RT table ${await t30()}`);
      await pause(3_500);
    }
    await hook('selectRun', runB);
    await mapOf(runB);
    await clickSelector('[data-map-band="1000"]');
    await mapOf(runB, { bandHz: 1000 });
    await clickSelector('[data-part="map-diff"]');
    const d = await mapOf(runB, { kind: 'diff' });
    const end = Math.min(d.steps - 1, Math.max(400, decayEnd));
    await hook('m12SetStep', Math.round(end * 0.1));
    progress(`step 9: difference map: run ${runB} minus baseline ${d.baseline} at 1 kHz; note "${await text('[data-part="map-diff-note"]')}"; legend ${await legendText()}`);
    await sweep(Math.round(end * 0.1), end, 10_000, [0.5, 1], async (s) => progress(`step 9: difference at step ${s}, ${await text('[data-part="anim-time"]')}`));
    await pause(3_000);
    await clickSelector('[data-part="map-diff"]');
    await mapOf(runB, { kind: 'level' });
    progress('step 9: difference switched off; the variant run’s own map');
    await pause(BEAT_MS);
  });
});
