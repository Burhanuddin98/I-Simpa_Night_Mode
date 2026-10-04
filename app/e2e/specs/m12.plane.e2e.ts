// Wow list W1 (parity M41; docs/investigations/2026-10-04-wow-w1w5/PLAN.md): the ear-height
// plane, placed from the UI through the core's ops and check, mapped by SPPS once it runs again.
// Each id with its control:
//   w1-add     Sources & receivers › + Ear-height plane adds one cutting plane through the checked
//              apply, at upstream's corners over the model's box, 1.6 m above its floor, 1 m cells,
//              as one undo step; the 3D view draws its outline and its cell grid. Control: before
//              the click the project has no such plane, and Undo removes it
//   w1-refuse  a cell size larger than a side is refused by the core (`cutting_plane_invalid`)
//              inline, the project unchanged; a height above the ceiling and a word are refused by
//              the field before the core; a height of 1.2 m moves all three corners to it
//   w1-rerun   the run made before the plane says on the Results step that the plane is not in
//              it and SPPS must run again. Control: the run made after says nothing of the kind
//   w1-map     after SPPS runs again, the cutting-plane map holds the plane by name with 2 x cells
//              faces (the panel's count), three of its texels equal the file's float32 bit for bit,
//              and the map draws on screen. Screenshots to $M11_SCREENS (w1-*.png)
//
// The box: tests/fixtures/rooms/outputs_box.simpa (6 x 10 x 3 m, a floor surface receiver and a
// cutting plane "Cut"), copied into this spec's own folder on C: (M11_P), run from the app with
// particles saved 0.
import { strict as assert } from 'node:assert';
import { copyFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { clickSelector, RESULTS_STEP_CURRENT } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { f32, readCsbin } from '../lib/m12files.ts';
import { env } from '../lib/types.ts';

const HOOKS = ['idle', 'openProject', 'edit', 'projectJson', 'undo', 'undoDepth', 'runStart', 'runState', 'runsRows', 'selectRun', 'setStep', 'm12Map', 'm12Texels', 'm12MapPixels', 'm12SetStep', 'frame', 'planeOutlines'];

interface Plane {
  id: string;
  name: string;
  enabled: boolean;
  shape: { kind: string; a?: number[]; b?: number[]; c?: number[]; resolution_m?: number };
}
interface Project {
  geometry: { vertices: number[][] };
  surface_receivers: Plane[];
  solvers: { spps: Record<string, unknown> };
}
interface MapState {
  run: string;
  path: string;
  bandHz: number | null;
  kind: string;
  faces: number;
  steps: number;
  error: string | null;
}

const project = async () => JSON.parse(await m10.projectJson()) as Project;
const planesOf = (p: Project) => p.surface_receivers.filter((r) => r.shape.kind === 'cutting_plane');
const shown = (s: string | null) => (s ?? '').replace(/\s+/g, ' ').trim();
const screens = () => env('M11_SCREENS');

async function typeInto(selector: string, text: string): Promise<void> {
  const el = await $(selector);
  await el.waitForDisplayed({ timeout: 30_000 });
  await el.click();
  await el.setValue(text);
  await browser.keys('Enter');
  await m10.idle();
}

describe('W1: the ear-height plane', () => {
  let box = '';
  let runsRoot = '';
  let runA = '';
  let runB = '';
  let planeId = '';
  let cells = { u: 0, v: 0 };

  async function runSpps(): Promise<string> {
    const p = await project();
    p.solvers.spps.particles_saved = 0;
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
    const dir = path.join(env('M11_P'), 'm12-plane');
    mkdirSync(dir, { recursive: true });
    mkdirSync(screens(), { recursive: true });
    box = path.join(dir, 'outputs_box.simpa');
    copyFileSync(path.join(env('M11_REPO'), 'tests', 'fixtures', 'rooms', 'outputs_box.simpa'), box);
    runsRoot = path.join(dir, 'runs');
    await m10.openProject(box);
    runA = await runSpps();
    console.log(`receipt w1: run A ${runA} (before the plane) under ${runsRoot}`);
  });

  it('w1-add: + Ear-height plane adds upstream\'s plane at the floor + 1.6 m through the core, drawn in the view', async () => {
    await m10.setStep('sources');
    const before = await project();
    assert.deepEqual(planesOf(before).map((p) => p.name), ['Cut'], 'the control: only the fixture\'s plane');
    const depth = await m10.undoDepth();
    await clickSelector('[data-part="add-plane"]');
    await m10.idle();
    const after = await project();
    const added = planesOf(after).filter((p) => p.name !== 'Cut');
    assert.equal(added.length, 1, `one new plane: ${JSON.stringify(planesOf(after))}`);
    const p = added[0];
    planeId = p.id;
    assert.equal(p.name, 'Plane 1');
    assert.equal(p.enabled, true);
    const v = before.geometry.vertices;
    const min = [0, 1, 2].map((i) => Math.min(...v.map((x) => x[i])));
    const max = [0, 1, 2].map((i) => Math.max(...v.map((x) => x[i])));
    const z = min[2] + 1.6;
    console.log(`receipt w1-add: box ${JSON.stringify(min)}..${JSON.stringify(max)}; plane A ${JSON.stringify(p.shape.a)} B ${JSON.stringify(p.shape.b)} C ${JSON.stringify(p.shape.c)} r ${p.shape.resolution_m}`);
    // e_scene_recepteurss_recepteurcoupe.h:101-103.
    assert.deepEqual(p.shape.a, [min[0], max[1], z]);
    assert.deepEqual(p.shape.b, [min[0], min[1], z]);
    assert.deepEqual(p.shape.c, [max[0], min[1], z]);
    assert.equal(p.shape.resolution_m, 1);
    assert.equal(await m10.undoDepth(), depth + 1, 'one undo step');
    const row = await $(`[data-plane="${p.id}"]`);
    await row.waitForDisplayed({ timeout: 30_000 });
    const cellText = shown(await row.$('[data-part="plane-cells"]').getText());
    const m = /^(\d+) × (\d+)$/.exec(cellText);
    assert.ok(m, `cells "${cellText}"`);
    cells = { u: Number(m[1]), v: Number(m[2]) };
    assert.deepEqual(cells, { u: Math.ceil(max[0] - min[0]), v: Math.ceil(max[1] - min[1]) });
    assert.equal(await row.$('[data-field="plane.height"]').getValue(), '1.6');
    const outlines = await hook<{ name: string; corners: number[][]; u: number; v: number; gridLines: number }[]>('planeOutlines');
    const o = outlines.find((x) => x.name === 'Plane 1');
    assert.ok(o, `drawn: ${JSON.stringify(outlines.map((x) => x.name))}`);
    assert.equal(o.gridLines, cells.u - 1 + cells.v - 1, 'the cell grid is drawn');
    assert.ok(Math.abs(o.corners[3][0] - max[0]) < 1e-6 && Math.abs(o.corners[3][1] - max[1]) < 1e-6, 'the fourth corner is A + C - B');
    await hook('frame');
    await browser.saveScreenshot(path.join(screens(), 'w1-plane-placed.png'));
    // The control: Undo removes it, Redo is not needed (the next test adds through the button again).
    await m10.undo();
    assert.deepEqual(planesOf(await project()).map((x) => x.name), ['Cut']);
    await clickSelector('[data-part="add-plane"]');
    await m10.idle();
    planeId = planesOf(await project()).find((x) => x.name === 'Plane 1')?.id ?? '';
    assert.ok(planeId);
  });

  it('w1-refuse: the core refuses a cell larger than a side, the field a height outside the room; 1.2 m moves the plane', async () => {
    await m10.setStep('sources');
    const sel = (f: string) => `[data-plane="${planeId}"] [data-field="${f}"]`;
    const json0 = await m10.projectJson();
    await typeInto(sel('plane.resolution'), '50');
    const row = await $(`[data-plane="${planeId}"]`);
    const issue = await row.$('[data-issue-code]');
    await issue.waitForDisplayed({ timeout: 30_000 });
    const text = shown(await issue.getText());
    console.log(`receipt w1-refuse: cell 50 m: "${text}"`);
    assert.match(text, /cutting_plane_invalid/);
    assert.match(text, /larger than a side/);
    assert.match(text, /Refused; the project is unchanged/);
    assert.equal(await m10.projectJson(), json0, 'the core refused: the project is unchanged');
    // The field's own NO, before the core: above the ceiling, and a word.
    for (const t of ['5', 'abc']) {
      await typeInto(sel('plane.height'), t);
      const msg = shown(await row.$('[data-issue-code="PLANE_FIELD"]').getText());
      console.log(`receipt w1-refuse: height "${t}": "${msg}"`);
      assert.match(msg, t === '5' ? /below the ceiling, 3 m above the floor/ : /not a number/);
      assert.equal(await m10.projectJson(), json0);
    }
    await typeInto(sel('plane.resolution'), '1');
    await typeInto(sel('plane.height'), '1.2');
    const p = planesOf(await project()).find((x) => x.id === planeId) as Plane;
    assert.deepEqual([p.shape.a?.[2], p.shape.b?.[2], p.shape.c?.[2]], [1.2, 1.2, 1.2]);
    assert.equal(p.shape.resolution_m, 1);
    console.log(`receipt w1-refuse: height 1.2 m: A ${JSON.stringify(p.shape.a)}`);
  });

  it('w1-rerun: the run made before the plane says it is not in it; SPPS runs again and the run after says nothing', async () => {
    await hook('selectRun', runA);
    await m10.setStep('results');
    await $(RESULTS_STEP_CURRENT).waitForExist({ timeout: 30_000 });
    const note = await $('[data-part="plane-rerun"]');
    await note.waitForDisplayed({ timeout: 60_000 });
    const t = shown(await note.getText());
    console.log(`receipt w1-rerun: run A: "${t}"`);
    assert.equal(t, 'Plane 1 is not in this run: run SPPS again to map it.');
    assert.equal(await browser.execute(() => document.querySelector('[data-part="plane-rerun"]')?.closest('[data-results-region]') != null), true);
    runB = await runSpps();
    await hook('selectRun', runB);
    await browser.waitUntil(async () => (await hook<MapState | null>('m12Map'))?.run === runB, { timeout: 60_000, timeoutMsg: 'run B did not show' });
    assert.equal(await $('[data-part="plane-rerun"]').isExisting(), false, 'the control: the run after holds the plane');
    console.log(`receipt w1-rerun: run B ${runB} holds it`);
  });

  it('w1-map: the plane\'s map holds 2 x cells faces by name, its texels equal the file, and it draws', async () => {
    await hook('selectRun', runB);
    await m10.setStep('results');
    await clickSelector('[data-map-kind="plane|"]');
    let m: MapState | null = null;
    await browser.waitUntil(async () => (m = await hook<MapState | null>('m12Map')) !== null && m.run === runB && m.error === null && m.bandHz === 1000 && /cut/i.test(m.path), {
      timeout: 60_000,
      timeoutMsg: `no cutting-plane map for run B: ${JSON.stringify(m)}`,
    });
    const ms = m as unknown as MapState;
    const file = readCsbin(path.join(runsRoot, runB, 'solve', ...ms.path.split('/')));
    console.log(`receipt w1-map: ${ms.path}: receivers ${JSON.stringify(file.receivers)}; map ${ms.faces} faces x ${ms.steps} steps`);
    assert.equal(ms.faces, file.faces.length);
    let first = 0;
    const r = file.receivers.find((x, i) => {
      if (x.name === 'Plane 1') return true;
      first += file.receivers[i].faces;
      return false;
    });
    assert.ok(r, 'the map holds Plane 1 by name');
    assert.equal(r.faces, 2 * cells.u * cells.v, `2 x ${cells.u} x ${cells.v} cells`);
    const cellsWith: { face: number; step: number; bits: number }[] = [];
    for (let f = first; f < first + r.faces; f++) for (const [step, bits] of file.faces[f].records) if (f32(bits) > 0) cellsWith.push({ face: f, step, bits });
    assert.ok(cellsWith.length >= 3, `only ${cellsWith.length} cells of Plane 1 hold energy`);
    const picks = [cellsWith[0], cellsWith[Math.floor(cellsWith.length / 2)], cellsWith[cellsWith.length - 1]];
    const got = await hook<number[]>('m12Texels', picks.map((c) => [c.face, c.step]));
    for (const [i, c] of picks.entries()) {
      console.log(`receipt w1-map: Plane 1 face ${c.face} step ${c.step}: texel 0x${(got[i] >>> 0).toString(16)} file 0x${(c.bits >>> 0).toString(16)}`);
      assert.equal(got[i] >>> 0, c.bits >>> 0);
    }
    await hook('m12SetStep', picks[0].step);
    const px = await hook<{ pixels: number; changed: number } | null>('m12MapPixels');
    console.log(`receipt w1-map: the map changes ${px?.changed} of ${px?.pixels} pixels at step ${picks[0].step}`);
    assert.ok(px && px.changed > 100);
    await browser.saveScreenshot(path.join(screens(), 'w1-plane-map.png'));
  });
});
