// The app tour, recorded as one plain video (Burhan 2026-10-04 15:08: "all I needed really was a
// simple video"; the maps and the particles seen close, at the right angle, held long enough to
// read, the particles at a speed where the sphere expands step by step). BRAS's CR4 auditorium:
// open and orbit, the model check, a quick pass through materials, sources and Simulate, the Run
// click (the solve itself is cut from the video afterwards, from the MARK lines), then Results:
// per-source acoustics, the cutting-plane map in smooth colour with contours, the particles at the
// slow playback speed with the time readout, and trails. Not a gate id: it asserts only that each
// step reached the state it claims. Run by `tools/gates/m11.ps1 -Only e2e -Spec tour -ScreensDir
// <dir>` with TOUR_HALL set.
//
// It writes no picture, except with TOUR_PROBE set: then it opens TOUR_HALL's first OK run on the
// Results step and saves one PNG per camera angle into $TOUR_PROBE (a %TEMP% folder), to choose
// the angles; those are looked at and deleted.
//
// With TOUR_WINDOW set (a folder) and TOUR_WINDOW_RUN (a run of TOUR_HALL), it saves the map
// window's before/after pair: the 1 kHz plane map at step TOUR_WINDOW_STEP (300 by default), flat
// colour, with the window Off (`cr4-before.png`) and at 10 ms (`cr4-after.png`), the app window only.
//
// Into M11_SCREENS: progress.log, one line per thing shown; `tour start` first and `tour end` last
// (the recorder keys on them), and `MARK <name> <epoch ms>` lines for the cut. The hall is copied
// to `<M11_SCREENS>\work-<stamp>\cr4`, so its run lands there (B:), never in the repository.
import { strict as assert } from 'node:assert';
import { appendFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { clickSelector } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { need, showTab } from '../lib/m11.ts';

/** A beat between the parts of one step. */
const BEAT_MS = 1_500;
/** How long a map, a table or a frame is held still to be read. */
const HOLD_MS = 4_000;

const OUT = () => need('M11_SCREENS');
const STAMP = new Date(Date.now() - new Date().getTimezoneOffset() * 60_000).toISOString().replace(/[-:]/g, '').replace('T', '-').slice(0, 15);
const WORK = () => path.join(OUT(), `work-${STAMP}`);

function progress(line: string): void {
  const d = new Date();
  const z = (n: number, w = 2) => String(n).padStart(w, '0');
  const t = `${d.getFullYear()}-${z(d.getMonth() + 1)}-${z(d.getDate())} ${z(d.getHours())}:${z(d.getMinutes())}:${z(d.getSeconds())}.${z(d.getMilliseconds(), 3)}`;
  mkdirSync(OUT(), { recursive: true });
  try {
    appendFileSync(path.join(OUT(), 'progress.log'), `${t} ${line}\n`);
  } catch {
    // A reader holding the log must not stop the tour.
  }
  console.log(`tour: ${line}`);
}
const mark = (name: string) => progress(`MARK ${name} ${Date.now()}`);

const pause = (ms: number) => browser.pause(ms);

const text = (sel: string): Promise<string> =>
  browser.execute((s: string) => ((document.querySelector(s) as HTMLElement | null)?.innerText ?? '').replace(/\s+/g, ' ').trim(), sel);

/** Scrolls the element to the top of its scroll container (a table's rows into the dock's view). */
async function toTop(sel: string, settle = 800): Promise<void> {
  await $(sel).waitForExist({ timeout: 30_000 });
  await browser.execute((s: string) => document.querySelector(s)?.scrollIntoView({ behavior: 'smooth', block: 'start', inline: 'start' }), sel);
  await pause(settle);
}

async function reveal(sel: string, settle = 1_000): Promise<void> {
  await $(sel).waitForExist({ timeout: 30_000 });
  await browser.execute((s: string) => document.querySelector(s)?.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'start' }), sel);
  await pause(settle);
}

// ---- the 3D view -----------------------------------------------------------------------------

type Vec = [number, number, number];
interface Cam {
  view: string;
  position: number[];
  target: number[] | null;
}
const camText = async () => {
  const c = await hook<Cam>('cameraState');
  return `camera at (${c.position.map((v) => v.toFixed(1)).join(', ')}) looking at (${(c.target ?? []).map((v) => v.toFixed(1)).join(', ')})`;
};

/** The camera eased to `position` looking at `target` over `ms`. */
const fly = (position: Vec, target: Vec, ms: number) => hook('flyCamera', position, target, ms);

/** A slow orbit: a real pointer drag of (dx, dy) px across the 3D view, over `ms`. */
async function orbit(dx: number, dy: number, ms: number): Promise<void> {
  const c = await browser.execute(() => {
    const r = document.querySelector('canvas.viewport-canvas')?.getBoundingClientRect();
    return r ? { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + r.height / 2) } : null;
  });
  assert.ok(c, 'no 3D view canvas');
  const n = Math.max(10, Math.round(ms / 33));
  const x0 = c.x - dx / 2;
  const y0 = c.y - dy / 2;
  let a = browser
    .action('pointer', { parameters: { pointerType: 'mouse' } })
    .move({ x: Math.round(x0), y: Math.round(y0), origin: 'viewport', duration: 0 })
    .down({ button: 0 });
  for (let i = 1; i <= n; i++) a = a.move({ x: Math.round(x0 + (dx * i) / n), y: Math.round(y0 + (dy * i) / n), origin: 'viewport', duration: 33 });
  await a.up({ button: 0 }).perform();
  await browser.action('pointer', { parameters: { pointerType: 'mouse' } }).move({ x: 2, y: 2, origin: 'viewport', duration: 0 }).perform();
}

async function frameModel(): Promise<void> {
  await clickSelector('[data-tool="frame"]');
  await m10.idle();
}

/** Folds a floating panel away, or back, with its own chevron. */
async function fold(panel: 'scene' | 'props' | 'dock', folded: boolean): Promise<void> {
  const b = await $(`[data-fold="${panel}"]`);
  await b.waitForExist({ timeout: 10_000 });
  if ((await b.getAttribute('aria-expanded')) === String(!folded)) return;
  await b.click();
  await browser.waitUntil(async () => (await b.getAttribute('aria-expanded')) === String(!folded), { timeout: 10_000, timeoutMsg: `${panel} did not ${folded ? 'fold' : 'unfold'}` });
  await m10.idle();
}

// ---- the project and its run -----------------------------------------------------------------

interface Row {
  run: string;
  number: number;
  status: string;
}
interface ProjectFile {
  surface_groups: { id: string; name: string; material: string }[];
  materials: { id: string; name: string }[];
  sources: { id: string; name: string; position: number[] }[];
  surface_receivers: { id: string; name: string; shape: { a: number[]; b: number[]; c: number[] } }[];
  geometry: { faces: unknown[] };
}
const project = async (): Promise<ProjectFile> => JSON.parse(await m10.projectJson());

async function runFromButton(): Promise<Row> {
  const before = new Set((await hook<Row[]>('runsRows')).map((r) => r.run));
  const btn = await $('[data-part="run-panel"]');
  await btn.waitForEnabled({ timeout: 60_000, timeoutMsg: `the Run button stayed disabled: ${await btn.getAttribute('data-blockers')}` });
  await btn.scrollIntoView({ block: 'center' });
  await btn.click();
  mark('run-click');
  progress('step 4: Run clicked (SPPS)');
  let fresh: Row | undefined;
  await browser.waitUntil(
    async () => {
      if ((await hook<unknown>('runState')) !== null) return false;
      fresh = (await hook<Row[]>('runsRows')).find((r) => !before.has(r.run) && r.status !== 'RUNNING');
      return fresh !== undefined;
    },
    { timeout: 3_000_000, interval: 500, timeoutMsg: 'no run ended within 50 min' },
  );
  mark('run-end');
  await m10.idle();
  const row = fresh as Row;
  assert.equal(row.status, 'OK', `run ${row.run} ended ${row.status}`);
  return row;
}

const PANEL = '[data-dock-panel="acoustics"]';
async function acousticsShows(run: string): Promise<void> {
  let last: unknown = null;
  await browser
    .waitUntil(
      async () => {
        const v = await hook<{ state: string; run: string | null } | null>('acousticsView');
        last = v && { state: v.state, run: v.run, ...((v as { error?: unknown }).error ? { error: (v as { error?: unknown }).error } : {}) };
        return v !== null && v.run === run && v.state === 'ready';
      },
      { timeout: 120_000, interval: 250 },
    )
    .catch(() => {
      throw new Error(`the Acoustics tab did not show ${run} ready; last ${JSON.stringify(last)?.slice(0, 400)}`);
    });
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

interface MapState {
  run: string;
  bandHz: number | null;
  kind: string;
  faces: number;
  steps: number;
}
async function mapOf(run: string, bandHz?: number): Promise<MapState> {
  let last: MapState | null = null;
  await browser.waitUntil(
    async () => {
      last = await hook<MapState | null>('m12Map');
      return last !== null && last.run === run && (bandHz === undefined || last.bandHz === bandHz);
    },
    { timeout: 240_000, interval: 250, timeoutMsg: `the map did not show ${run} at ${bandHz}` },
  );
  return last as unknown as MapState;
}
interface Playback {
  step: number;
  steps: number;
  playing: boolean;
  speed: number;
  start: number;
  rate: string;
}
const playback = () => hook<Playback>('m12Playback');
const readoutText = async () => `${await text('[data-part="anim-time"]')}, ${await text('[data-part="anim-stepno"]')}`;

// ---- the camera angles (chosen from TOUR_PROBE's pictures) --------------------------------------

/** The cutting plane's corners a, b, c and its fourth, d = a + c - b; its centre. */
function planeOf(p: ProjectFile): { centre: Vec; a: Vec; b: Vec; c: Vec } {
  const s = p.surface_receivers[0].shape;
  const a = s.a as Vec;
  const c = s.c as Vec;
  return { a, b: s.b as Vec, c, centre: [(a[0] + c[0]) / 2, (a[1] + c[1]) / 2, (a[2] + c[2]) / 2] };
}
const add = (u: Vec, v: Vec): Vec => [u[0] + v[0], u[1] + v[1], u[2] + v[2]];

/** The map from the hall's side, low enough to stay under the ceiling panels; the look point is
 * moved toward the near edge so the plane sits above the bottom cards. */
const MAP_FROM: Vec = [-4, -20, 10];
const MAP_LOOK: Vec = [0, -3.5, -1];
/** The same plane from the other side, for the cumulative map. */
const MAP_FROM_2: Vec = [-6, 19, 10];
const MAP_LOOK_2: Vec = [0, 3.5, -1];
/** The particles around source LS1 from the side at about its height: close for the first steps,
 * then back as the sphere grows. Trails 5 steps long draw each particle's way out from the source. */
const PARTICLES_CLOSE: Vec = [9, -6, 3];
const PARTICLES_FROM: Vec = [19, -13, 6];
const PARTICLES_LOOK: Vec = [0, 0, 0.5];

const PROBE = process.env.TOUR_PROBE ?? '';
/** The response window on a finished CR4 run (Burhan 2026-10-04: "WHAT ABOUT THE IR SPECTROGRAM
 * RESULT"): TOUR_IR is the one picture's path, TOUR_IR_RUN the run, opened in TOUR_HALL itself. */
const IR_SHOT = process.env.TOUR_IR ?? '';
/** The map window's before/after pair (Burhan 2026-10-05 01:59): TOUR_WINDOW is the folder. */
const WINDOW_SHOTS = process.env.TOUR_WINDOW ?? '';

if (WINDOW_SHOTS) {
  describe('Tour pair: the map window off and at 10 ms on the CR4 plane', function () {
    this.timeout(1_200_000);
    it('window', async () => {
      mkdirSync(WINDOW_SHOTS, { recursive: true });
      progress(`tour start; window pair into ${WINDOW_SHOTS}`);
      await waitForHooks(['idle', 'openProject', 'setStep', 'runsRows', 'selectRun', 'm12Map', 'm12SetStep', 'cameraState', 'flyCamera', 'projectJson']);
      await m10.openProject(need('TOUR_HALL'));
      await m10.setStep('results');
      const run = need('TOUR_WINDOW_RUN');
      let rows: Row[] = [];
      await browser.waitUntil(async () => (rows = await hook<Row[]>('runsRows')).some((r) => r.run === run), { timeout: 30_000 }).catch(() => {
        throw new Error(`the project lists no run ${run}: ${JSON.stringify(rows).slice(0, 600)}`);
      });
      await hook('selectRun', run);
      await mapOf(run);
      await clickSelector('[data-map-band="1000"]');
      await mapOf(run, 1000);
      for (const panel of ['scene', 'props', 'dock'] as const) await fold(panel, true);
      const pl = planeOf(await project());
      await fly(add(pl.centre, MAP_FROM), add(pl.centre, MAP_LOOK), 300);
      const step = Number(process.env.TOUR_WINDOW_STEP ?? '300');
      await browser.action('pointer', { parameters: { pointerType: 'mouse' } }).move({ x: 2, y: 2, origin: 'viewport', duration: 0 }).perform();
      for (const [name, ms] of [['cr4-before', 0], ['cr4-after', 10]] as const) {
        await clickSelector(`[data-map-window="${ms}"]`);
        await browser.waitUntil(async () => (await hook<{ windowSteps: number } | null>('m12Map'))?.windowSteps === (ms === 0 ? 1 : 10), { timeout: 30_000, timeoutMsg: `the map did not take the ${ms} ms window` });
        await hook('m12SetStep', step);
        await pause(900);
        const file = path.join(WINDOW_SHOTS, `${name}.png`);
        await browser.saveScreenshot(file);
        progress(`window ${name}: ${ms} ms, step ${step}, ${await camText()}; legend "${await text('[data-part="map-legend"] .vp-legend-title')}"; ${await readoutText()}`);
      }
      progress('tour end');
    });
  });
} else if (IR_SHOT) {
  describe('Tour clip: the response window on CR4, two receivers and the two sources', function () {
    this.timeout(1_200_000);
    afterEach(function () {
      const t = this.currentTest;
      if (t?.state === 'failed') progress(`FAILED at "${t.title}": ${String(t.err?.message ?? '').replace(/\s+/g, ' ').slice(0, 400)}`);
    });
    after(() => progress('tour end'));
    it('ir', async () => {
      await browser.setTimeout({ script: 120_000 });
      progress('tour prep');
      await waitForHooks(['idle', 'openProject', 'setStep', 'runsRows', 'selectRun', 'dockTab', 'acousticsView', 'responseView']);
      for (const panel of ['scene', 'props', 'dock'] as const) await fold(panel, false).catch(() => undefined);
      await m10.openProject(need('TOUR_HALL'));
      await m10.setStep('results');
      const run = need('TOUR_IR_RUN');
      let rows: Row[] = [];
      await browser.waitUntil(async () => (rows = await hook<Row[]>('runsRows')).some((r) => r.run === run), { timeout: 30_000 }).catch(() => {
        throw new Error(`the project lists no run ${run}: ${JSON.stringify(rows).slice(0, 600)}`);
      });
      assert.equal(rows.find((r) => r.run === run)?.status, 'OK', `run ${run}`);
      await hook('selectRun', run);
      await showTab('acoustics');
      await acousticsShows(run);
      const value = async (control: string, name: string) => {
        const o = (await options(control)).find((x) => x.text === name || x.text.startsWith(`${name} `));
        assert.ok(o, `no ${control} "${name}" in ${JSON.stringify(await options(control))}`);
        return o.value;
      };
      const [ls1, ls2, mp1, mp5] = [await value('source', 'LS1'), await value('source', 'LS2'), await value('receiver', 'MP1'), await value('receiver', 'MP5')];
      await pick('source', ls1);
      await pick('receiver', mp1);
      await acousticsShows(run);
      await browser.action('pointer', { parameters: { pointerType: 'mouse' } }).move({ x: 2, y: 2, origin: 'viewport', duration: 0 }).perform();
      const WIN = '[data-response-window]';
      const shown = async (what: string) => {
        await $(WIN).waitForExist({ timeout: 10_000 });
        await m10.idle();
        await browser.execute(() => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r()))));
        const v = await hook<{ receiver: number; source: string | null; cols: number; shown: number; crop: number | null; paths: string[] } | null>('responseView');
        assert.ok(v, 'the response window shows no echogram');
        progress(
          `ir: ${what}: ${(await text(`${WIN} .rw-sub`)) || '?'}; receiver ${v.receiver}, source ${v.source}, ${v.shown} of ${v.cols} steps shown (crop ${v.crop}) x ${v.paths.length} bands; ${(await text(`${WIN} [data-part="response-shown"]`)) || '?'}`,
        );
      };
      progress('tour start');
      await pause(2_000);
      mark('ir-open');
      await clickSelector(`${PANEL} [data-action="open-response"]`);
      await shown('opened');
      await browser.saveScreenshot(IR_SHOT);
      progress(`ir: picture ${IR_SHOT}`);
      await pause(4_500);
      for (const [control, v, what] of [
        ['receiver', mp5, 'receiver MP5'],
        ['source', ls2, 'source LS2'],
        ['receiver', mp1, 'receiver MP1'],
      ] as const) {
        await pick(control, v);
        await shown(what);
        await pause(4_000);
      }
      mark('ir-done');
    });
  });
} else if (PROBE) {
  describe('Tour probe: camera angles over the map and the particles', function () {
    this.timeout(1_200_000);
    it('probe', async () => {
      mkdirSync(PROBE, { recursive: true });
      progress(`tour start; probe ${PROBE}`);
      await waitForHooks(['idle', 'openProject', 'setStep', 'runsRows', 'selectRun', 'm12Map', 'm12SetStep', 'cameraState', 'flyCamera', 'm12Playback', 'projectJson']);
      await m10.openProject(need('TOUR_HALL'));
      await m10.setStep('results');
      let rows: Row[] = [];
      let all: Row[] = [];
      await browser
        .waitUntil(async () => (rows = (all = await hook<Row[]>('runsRows')).filter((r) => r.status === 'OK')).length > 0, { timeout: 30_000 })
        .catch(() => {
          throw new Error(`the project lists no OK run: ${JSON.stringify(all).slice(0, 600)}`);
        });
      const run = rows.sort((a, b) => a.number - b.number)[0].run;
      await hook('selectRun', run);
      await mapOf(run);
      await clickSelector('[data-map-band="1000"]');
      await mapOf(run, 1000);
      for (const panel of ['scene', 'props', 'dock'] as const) await fold(panel, true);
      await clickSelector('[data-part="map-smooth"]');
      await clickSelector('[data-map-contours="3"]').catch(() => undefined);
      const p = await project();
      const pl = planeOf(p);
      const src = p.sources[0].position as Vec;
      const shots: [string, Vec, Vec, number, string][] = [
        ['map-side-80', add(pl.centre, MAP_FROM), add(pl.centre, MAP_LOOK), 80, ''],
        ['map-other-cum-400', add(pl.centre, MAP_FROM_2), add(pl.centre, MAP_LOOK_2), 400, 'cumulative'],
        ['particles-close-6', add(src, PARTICLES_CLOSE), add(src, PARTICLES_LOOK), 6, 'trails'],
        ['particles-close-20', add(src, PARTICLES_CLOSE), add(src, PARTICLES_LOOK), 20, ''],
        ['particles-far-45', add(src, PARTICLES_FROM), add(src, PARTICLES_LOOK), 45, ''],
      ];
      for (const [name, at, look, step, then] of shots) {
        if (then === 'cumulative') await clickSelector('[data-part="map-cumulative"]');
        if (then === 'trails') {
          await clickSelector('[data-part="map-cumulative"]');
          await clickSelector('[data-trails="5"]');
        }
        await fly(at, look, 300);
        await hook('m12SetStep', step);
        await pause(900);
        await browser.saveScreenshot(path.join(PROBE, `${name}.png`));
        progress(`probe ${name}: step ${step}, ${await camText()}; ${await readoutText()}`);
      }
      progress('tour end');
    });
  });
} else {
  describe('Tour: BRAS CR4 auditorium, one plain pass', function () {
    this.timeout(3_600_000);
    let hall = '';
    let run = '';

    before(async () => {
      await browser.setTimeout({ script: 120_000 });
      const src = need('TOUR_HALL');
      mkdirSync(path.join(WORK(), 'cr4'), { recursive: true });
      hall = path.join(WORK(), 'cr4', path.basename(src));
      // One design only: the variant of the last tour is dropped from the copy.
      const p = JSON.parse(readFileSync(src, 'utf8').replace(/^﻿/, '')) as Record<string, unknown>;
      p.variants = [];
      p.active_variant = null;
      writeFileSync(hall, JSON.stringify(p, null, 2));
      progress(`tour start; work folder ${WORK()}`);
      await waitForHooks([
        'idle', 'openProject', 'projectJson', 'setStep', 'selection', 'cameraState', 'flyCamera', 'runState', 'runsRows', 'selectRun', 'dockTab', 'acousticsView',
        'm12Map', 'm12SetStep', 'm12Particles', 'm12Playback',
      ]);
      for (const panel of ['scene', 'props', 'dock'] as const) await fold(panel, false).catch(() => undefined);
    });

    afterEach(function () {
      const t = this.currentTest;
      if (t?.state === 'failed') progress(`FAILED at "${t.title}": ${String(t.err?.message ?? '').replace(/\s+/g, ' ').slice(0, 400)}`);
    });

    after(async () => {
      progress('tour end');
    });

    it('1: CR4 opens, its model check, and an orbit', async () => {
      await pause(2_000);
      await m10.openProject(hall);
      await m10.setStep('geometry');
      await frameModel();
      const p = await project();
      progress(`step 1: opened CR4: ${p.geometry.faces.length} faces, ${p.surface_groups.length} surface groups; ${await camText()}`);
      await reveal('[data-part="geometry-panel"] .check-list', 600);
      progress(`step 1: model check "${await text('[data-part="check-verdict"]')}"`);
      await pause(HOLD_MS);
      await orbit(420, 0, 6_000);
      await orbit(300, 0, 4_000);
      progress(`step 1: orbit: ${await camText()}`);
      await pause(BEAT_MS);
    });

    it('2: materials, sources and the Simulate settings, then Run', async () => {
      const p = await project();
      await m10.setStep('materials');
      await $('[data-materials-grid]').waitForDisplayed({ timeout: 30_000 });
      for (const name of ['mat_CR4_seating', 'mat_CR4_whitePanels']) {
        const g = p.surface_groups.find((x) => x.name === name) ?? p.surface_groups[0];
        await reveal(`.scene [data-entity="surface_group:${g.id}"]`, 200);
        await clickSelector(`.scene [data-entity="surface_group:${g.id}"]`);
        await m10.idle();
        progress(`step 2: group "${g.name}" uses "${p.materials.find((m) => m.id === g.material)?.name}"`);
        await pause(2_500);
      }
      await m10.setStep('sources');
      for (const s of p.sources) {
        await reveal(`.scene [data-entity="source:${s.id}"]`, 200);
        await clickSelector(`.scene [data-entity="source:${s.id}"]`);
        await $('[data-field="position.x"]').waitForDisplayed({ timeout: 30_000 });
        progress(`step 2: source ${s.name} at (${s.position.join(', ')}) m`);
        await pause(2_000);
      }
      await m10.setStep('simulate');
      await $('[data-setting="particles"] input[data-field="particles"]').waitForDisplayed({ timeout: 30_000 });
      progress(`step 2: Simulate: ${await text('[data-props-step="simulate"] [data-setting="particles"]')}`);
      await pause(HOLD_MS);
      await showTab('console');
      const row = await runFromButton();
      run = row.run;
      progress(`step 2: run ${row.number} (${run}) ended ${row.status}`);
      await pause(2_500);
    });

    it('3: Results: the acoustics per source', async () => {
      await m10.setStep('results');
      await hook('selectRun', run);
      await showTab('acoustics');
      await acousticsShows(run);
      const sources = await options('source');
      await pick('band', (await options('band')).find((o) => o.text === '1 kHz')?.value ?? '0');
      for (const o of sources) {
        await pick('source', o.value);
        await acousticsShows(run);
        await toTop(`${PANEL} [data-part="receivers-table"]`);
        progress(`step 3: source "${o.text}", the receivers at 1 kHz`);
        await pause(HOLD_MS);
      }
      await pick('source', sources[0].value);
      await toTop(`${PANEL} [data-part="rt-table"]`);
      progress('step 3: reverberation time per band');
      await pause(HOLD_MS);
    });

    it('4: the cutting-plane map, smooth colour with contours, close and angled', async () => {
      await mapOf(run);
      await clickSelector('[data-map-band="1000"]');
      await mapOf(run, 1000);
      for (const panel of ['scene', 'props', 'dock'] as const) await fold(panel, true);
      await clickSelector('[data-part="map-smooth"]');
      await clickSelector('[data-map-contours="3"]');
      const pl = planeOf(await project());
      await hook('m12SetStep', 40);
      await fly(add(pl.centre, MAP_FROM), add(pl.centre, MAP_LOOK), 3_500);
      progress(`step 4: the plane map at 1 kHz, smooth with contours every 3 dB, ${await camText()}`);
      for (const step of [40, 80, 150, 400]) {
        await hook('m12SetStep', step);
        progress(`step 4: the level at ${await readoutText()}`);
        await pause(HOLD_MS);
      }
      await clickSelector('[data-part="map-cumulative"]');
      await browser.waitUntil(async () => (await text('[data-part="map-legend"] .vp-legend-title')).includes('cumulative'), {
        timeout: 60_000,
        timeoutMsg: 'the cumulative map did not load',
      });
      await fly(add(pl.centre, MAP_FROM_2), add(pl.centre, MAP_LOOK_2), 4_000);
      for (const step of [80, 400]) {
        await hook('m12SetStep', step);
        progress(`step 4: the cumulative level (sound building up) at ${await readoutText()}; ${await camText()}`);
        await pause(HOLD_MS);
      }
      await clickSelector('[data-part="map-cumulative"]');
      await browser.waitUntil(async () => !(await text('[data-part="map-legend"] .vp-legend-title')).includes('cumulative'), { timeout: 60_000 });
    });

    it('5: the particles at the slow speed: the sphere expands step by step', async () => {
      await browser.waitUntil(async () => (await hook<{ run: string } | null>('m12Particles'))?.run === run, { timeout: 120_000, timeoutMsg: 'no particles loaded' });
      const src = (await project()).sources[0].position as Vec;
      await clickSelector('[data-trails="5"]');
      await clickSelector('[data-part="anim-start"]');
      await fly(add(src, PARTICLES_CLOSE), add(src, PARTICLES_LOOK), 3_500);
      const pb = await playback();
      progress(`step 5: particles from the emission (step ${pb.start}), trails 5 steps, ${pb.rate}; ${await camText()}`);
      await pause(2_000);
      for (let i = 0; i < 6; i++) {
        await clickSelector('[data-part="anim-forward"]');
        progress(`step 5: one step forward: ${await readoutText()}`);
        await pause(1_300);
      }
      await clickSelector('[data-part="anim-play"]');
      progress(`step 5: Play at ${(await playback()).rate}`);
      // The camera backs away as the sphere grows (3.4 m of radius a second at 0.01x).
      await fly(add(src, PARTICLES_FROM), add(src, PARTICLES_LOOK), 14_000);
      await pause(1_000);
      await clickSelector('[data-part="anim-play"]');
      progress(`step 5: paused at ${await readoutText()}`);
      await pause(HOLD_MS);
    });

    it('6: longer trails at 0.025x', async () => {
      await clickSelector('[data-trails="20"]');
      await clickSelector('[data-anim-speed="0.025"]');
      await clickSelector('[data-part="anim-play"]');
      progress(`step 6: trails 20 steps, Play at ${(await playback()).rate}`);
      await pause(10_000);
      await clickSelector('[data-part="anim-play"]');
      progress(`step 6: paused at ${await readoutText()}`);
      await pause(HOLD_MS);
      await clickSelector('[data-trails="0"]');
      await clickSelector('[data-anim-speed="0.01"]');
      for (const panel of ['scene', 'props', 'dock'] as const) await fold(panel, false);
      await pause(BEAT_MS);
    });
  });
}

