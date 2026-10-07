// C1's spec (docs/investigations/2026-10-07-blank-geometry/SPEC.md, order of work 6): a stranger's
// blank 6 x 10 x 3 m box, written here as a raw OBJ, all the way to results, driven by the pointer
// and the keys a user would use. Its checks, each with its control:
//   c1-import     the groupless OBJ (one object, no g, no usemtl) imports through the dialog as one
//                 group named for the file, 12 faces, and the Geometry step says the file had no
//                 groups. Control: the same box with six `g` groups imports as six named groups,
//                 and the step says nothing of the kind
//   c1-carve      from the 3D view: a double-click and New group from selection carves the floor,
//                 the ceiling and the south wall; one north triangle becomes a group and the other
//                 is moved into it (Move to group); the two west triangles each become a group and
//                 are merged (Ctrl+click both rows, Merge), keeping the first's name and material.
//                 Each is one undo step, and undoing the merge gives back the project text exactly
//   c1-rename     F2 on each group's row renames it: floor, ceiling, wall south, wall north, wall
//                 west, wall east; each row shows its face count. Control: another group's name is
//                 refused (GROUP_NAME_TAKEN) and the project is unchanged
//   c1-materials  five groups take a library material; a new material's absorption is typed per
//                 band (a pasted row) and its scattering in one cell; its transmission loss is typed
//                 in one band (the Transmission tab) and switched off again; the east wall takes it.
//                 Control: Run is blocked (MATERIALS_UNASSIGNED) until the last group has one
//   c1-source     + Source puts S1 off the room's centre; its power is typed (95 dB), its spectrum
//                 picked from upstream's list (ES_VL, the core's values on the project's bands), one
//                 band's level typed (keeps the others), its directivity made unidirectional and
//                 back. Each is one undo step. Control: a zero direction is refused by the validator
//   c1-receivers  two receivers (one moved) and an ear-height plane
//   c1-run-cpu    SPPS on the CPU from the Run button: OK, no particle lost to loops, the solver's
//                 stderr empty; the Results step verified, the Acoustics tab ready, the plane's map
//   c1-run-gpu    the same project on the GPU: OK, the device named, the Acoustics tab ready
//   c1-grouped    the grouped box to results: its six groups picked at once (Ctrl+click), one
//                 material for all, a source, a receiver, SPPS on the CPU OK
//
// Its files, under <M11_WORK>\blank (C:): the two OBJ files, the saved projects, their runs.
import { strict as assert } from 'node:assert';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { clickSelector } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { env } from '../lib/types.ts';

const WORK = () => path.join(env('M11_WORK'), 'blank');

type Point = { x: number; y: number };
type Sel = { faces: number[]; groups: string[] };
interface Group {
  id: string;
  name: string;
  material: string;
}
interface Material {
  id: string;
  name: string;
  absorption: (number | string)[];
  scattering: (number | string)[];
  transmission_loss_db: (number | string | null)[] | null;
}
interface Source {
  id: string;
  name: string;
  position: number[];
  power: { global_db: number; shape: { kind: string; relative_db?: number[] } };
  directivity: { kind: string; direction?: number[] };
}
interface ProjectFile {
  surface_groups: Group[];
  materials: Material[];
  sources: Source[];
  point_receivers: { id: string; name: string; position: number[] }[];
  surface_receivers: { id: string; name: string; shape: { kind: string } }[];
  geometry: { vertices: (number | string)[][]; faces: [number, number, number, string][] };
  bands: { frequencies_hz: number[] };
}
interface Row {
  run: string;
  status: string;
  gpu_device?: string | null;
  reasons?: { code: string }[];
}

const project = async (): Promise<ProjectFile> => JSON.parse(await m10.projectJson());
const facesOf = (p: ProjectFile, group: string) => p.geometry.faces.flatMap((f, i) => (f[3] === group ? [i] : []));
const groupNamed = (p: ProjectFile, name: string) => p.surface_groups.find((g) => g.name === name);
const materialOf = (p: ProjectFile, g: Group) => p.materials.find((m) => m.id === g.material)?.name;

// ---- the box ------------------------------------------------------------------------------------

const V = [
  [0, 0, 0],
  [6, 0, 0],
  [6, 10, 0],
  [0, 10, 0],
  [0, 0, 3],
  [6, 0, 3],
  [6, 10, 3],
  [0, 10, 3],
];
const QUADS: [string, number[], number[]][] = [
  ['floor', [1, 4, 3, 2], [0, 0, -1]],
  ['ceiling', [5, 6, 7, 8], [0, 0, 1]],
  ['wall south', [1, 2, 6, 5], [0, -1, 0]],
  ['wall north', [3, 4, 8, 7], [0, 1, 0]],
  ['wall west', [1, 5, 8, 4], [-1, 0, 0]],
  ['wall east', [2, 3, 7, 6], [1, 0, 0]],
];

function writeBox(file: string, grouped: boolean): void {
  const lines = ['# a 6 x 10 x 3 m box, metres, Z up', ...V.map((v) => `v ${v.join(' ')}`)];
  for (const [name, q] of QUADS) {
    if (grouped) lines.push(`g ${name.replace(' ', '_')}`);
    lines.push(`f ${q.join(' ')}`);
  }
  writeFileSync(file, lines.join('\n') + '\n', 'ascii');
}

/** The faces of a wall: those whose plane is the wall's (by the project's own mesh), ascending. */
function wallFaces(p: ProjectFile, wall: string): number[] {
  const n = QUADS.find((q) => q[0] === wall)![2];
  const vs = p.geometry.vertices.map((v) => v.map(Number));
  const hits: [number, number][] = [];
  p.geometry.faces.forEach((f, i) => {
    const [a, b, c] = [vs[f[0]], vs[f[1]], vs[f[2]]];
    const u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    const w = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    const x = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
    const len = Math.hypot(...x) || 1;
    if (Math.abs((x[0] * n[0] + x[1] * n[1] + x[2] * n[2]) / len) > 0.999) {
      hits.push([(a[0] + b[0] + c[0]) / 3 * n[0] + (a[1] + b[1] + c[1]) / 3 * n[1] + (a[2] + b[2] + c[2]) / 3 * n[2], i]);
    }
  });
  const top = Math.max(...hits.map((h) => h[0]));
  return hits.filter((h) => Math.abs(h[0] - top) < 1e-9).map((h) => h[1]);
}

// ---- the pointer and the keys --------------------------------------------------------------------

const CTRL = '';

/** A pointer press of `button` (0 left, 2 right) at a client point, as the mouse does it. */
async function pressAt(p: Point, button: 0 | 2, times = 1): Promise<void> {
  let a = browser.action('pointer', { parameters: { pointerType: 'mouse' } }).move({ x: Math.round(p.x), y: Math.round(p.y), origin: 'viewport', duration: 0 });
  for (let i = 0; i < times; i++) {
    if (i > 0) a = a.pause(60);
    a = a.down({ button }).up({ button });
  }
  await a.perform();
  await m10.idle();
}

/** Picks a face in the 3D view: the camera aimed at it, then a click or a double-click. */
async function pick(face: number, double: boolean): Promise<Point> {
  await m10.setStep('geometry');
  await browser.pause(400);
  const pt = await hook<Point | null>('aimAtFace', face);
  assert.ok(pt, `aimAtFace found face ${face}`);
  await pressAt(pt, 0, double ? 2 : 1);
  return pt;
}

const menuItems = () =>
  browser.execute(() => [...document.querySelectorAll('[data-part="viewport-menu"] [data-menu-item]')].map((b) => (b as HTMLElement).dataset.menuItem ?? ''));

/** The view's menu on the picked faces, and one of its entries clicked. */
async function viewMenu(pt: Point, item: string): Promise<void> {
  await pressAt(pt, 2);
  const items = await menuItems();
  assert.ok(items.includes(item), `the view's menu offers ${item}: ${JSON.stringify(items)}`);
  await clickSelector(`[data-part="viewport-menu"] [data-menu-item="${item}"]`);
  await m10.idle();
}

/** New group from selection on `faces`, returns the group it made. */
async function newGroup(faces: number[], double: boolean): Promise<Group> {
  const before = new Set((await project()).surface_groups.map((g) => g.id));
  const pt = await pick(faces[0], double);
  assert.deepEqual((await hook<Sel>('selection')).faces, faces, 'the pick took exactly these faces');
  await viewMenu(pt, 'new-group-from-selection');
  const added = (await project()).surface_groups.filter((g) => !before.has(g.id));
  assert.equal(added.length, 1);
  return added[0];
}

const row = (id: string) => `.scene [data-entity="surface_group:${id}"]`;

/** A Ctrl+click on an element, as the keyboard and the mouse do it together. */
async function ctrlClick(selector: string): Promise<void> {
  const el = await $(selector);
  await el.scrollIntoView({ block: 'nearest' });
  const r = await browser.execute((s: string) => {
    const b = document.querySelector(s)!.getBoundingClientRect();
    return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
  }, selector);
  await browser.actions([
    browser.action('key').down(CTRL).pause(50).pause(50).pause(50).up(CTRL),
    browser.action('pointer', { parameters: { pointerType: 'mouse' } }).move({ x: Math.round(r.x), y: Math.round(r.y), origin: 'viewport', duration: 0 }).down({ button: 0 }).up({ button: 0 }).pause(50),
  ]);
  await m10.idle();
}

/** F2 on a group's row, the name typed, Enter. */
async function renameRow(id: string, name: string): Promise<void> {
  await clickSelector(row(id));
  await browser.keys('F2');
  const input = await $('[data-part="group-name-input"]');
  await input.waitForDisplayed({ timeout: 10_000 });
  await input.setValue(name);
  await browser.keys('Enter');
  await m10.idle();
}

async function typeInto(selector: string, text: string): Promise<void> {
  const el = await $(selector);
  await el.waitForDisplayed({ timeout: 30_000 });
  await el.click();
  await el.setValue(text);
  await browser.keys('Enter');
  await m10.idle();
}

/** A synthetic paste at the focused grid cell (the grid's own handler; m10.materials does the same). */
async function pasteInGrid(text: string): Promise<void> {
  const out = await browser.execute((txt: string) => {
    const grid = document.querySelector('[data-materials-grid]');
    const target = document.activeElement;
    if (!grid || !target || !grid.contains(target)) return `the focus is on ${target?.tagName ?? 'nothing'}`;
    const dt = new DataTransfer();
    dt.setData('text/plain', txt);
    const ev = new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true });
    if (ev.clipboardData === null) Object.defineProperty(ev, 'clipboardData', { value: dt });
    target.dispatchEvent(ev);
    return ev.defaultPrevented ? 'ok' : 'not taken';
  }, text);
  assert.equal(out, 'ok');
  await m10.idle();
}

/** Picks a group's row and the material option, in the Materials step. */
async function giveMaterial(groupId: string, materialId: string): Promise<void> {
  await clickSelector(row(groupId));
  await m10.setStep('materials');
  await clickSelector(`[data-material-option="${materialId}"]`);
  await m10.idle();
}

/** A library material in the project, added through + From library if it is not yet. */
async function libraryMaterial(name: string): Promise<string> {
  const have = (await project()).materials.find((m) => m.name === name);
  if (have) return have.id;
  const lib = await hook<{ name: string; reference_id: number }[]>('materialLibrary');
  const entry = lib.find((e) => e.name === name);
  assert.ok(entry, `the library has ${name}`);
  await clickSelector('[data-action="library"]');
  await clickSelector(`[data-library-add="${entry.reference_id}"]`);
  await m10.idle();
  const m = (await project()).materials.find((x) => x.name === name);
  assert.ok(m);
  return m.id;
}

/** SPPS on `device` from the Simulate step's choice and its Run button; the run's row. */
async function runFromButton(device: 'cpu' | 'gpu'): Promise<Row> {
  await m10.setStep('simulate');
  await clickSelector(`[data-solver="${device === 'gpu' ? 'spps-gpu' : 'spps'}"]`);
  const blockers = await m10.runBlockers();
  assert.deepEqual(blockers, [], `Run is free: ${JSON.stringify(blockers)}`);
  const before = new Set((await hook<Row[]>('runsRows')).map((r) => r.run));
  await clickSelector('[data-part="run-panel"]');
  const run = await hook<string>('waitRun', null, 'ended', 900_000);
  assert.ok(!before.has(run));
  const r = (await hook<Row[]>('runsRows')).find((x) => x.run === run);
  assert.ok(r);
  return r;
}

/** Each band's lost-to-loops share, the stderr files, from the run folder beside the project. */
function runFolder(projectFile: string, run: string) {
  const dir = path.join(path.dirname(projectFile), 'runs', run);
  const manifest = JSON.parse(readFileSync(path.join(dir, 'run.json'), 'utf8'));
  const lost = (manifest.particles?.bands ?? []).map((b: { lost_by_infinite_loops: number; lost_by_meshing_problems: number; total: number }) =>
    (b.lost_by_infinite_loops + b.lost_by_meshing_problems) / b.total,
  );
  const stderr = ['solver.stderr.txt', path.join('mesh', 'tetgen.stderr.txt')].map((f) => {
    const p = path.join(dir, f);
    return existsSync(p) ? readFileSync(p, 'utf8') : null;
  });
  return { lost, stderr };
}

async function resultsReady(run: string): Promise<string> {
  await hook('selectRun', run);
  await m10.setStep('results');
  await hook('dockTab', 'acoustics');
  let state = '';
  await browser.waitUntil(
    async () => {
      state = await browser.execute(() => document.querySelector('[data-acoustics]')?.getAttribute('data-acoustics-state') ?? '');
      return state === 'ready' || state === 'refused' || state === 'error';
    },
    { timeout: 300_000, timeoutMsg: 'the Acoustics tab did not settle' },
  );
  return state;
}

// ---- the spec ------------------------------------------------------------------------------------

describe('C1: a blank geometry to results', () => {
  const blank = () => path.join(WORK(), 'box_blank.obj');
  const grouped = () => path.join(WORK(), 'box_grouped.obj');
  const saved = () => path.join(WORK(), 'blank', 'box.simpa');
  const ids: Record<string, string> = {};

  before(async () => {
    await waitForHooks(['idle', 'openImportDialog', 'projectJson', 'undo', 'redo', 'undoDepth', 'setStep', 'selection', 'aimAtFace', 'frame', 'saveAs', 'runsRows', 'waitRun', 'selectRun', 'dockTab', 'materialLibrary', 'm12Map']);
    mkdirSync(path.join(WORK(), 'blank'), { recursive: true });
    mkdirSync(path.join(WORK(), 'grouped'), { recursive: true });
    writeBox(blank(), false);
    writeBox(grouped(), true);
  });

  async function importThroughDialog(file: string): Promise<void> {
    await hook('openImportDialog', file);
    await clickSelector('[data-part="import-confirm"]');
    await browser.waitUntil(async () => !(await $('[data-part="import-dialog"]').isExisting()), { timeout: 30_000 });
    await m10.idle();
    await m10.setStep('geometry');
  }

  it('c1-import: a groupless OBJ is one group named for the file, and the Geometry step says so', async () => {
    // Control first: the grouped box names its six groups, and no note.
    await importThroughDialog(grouped());
    const g = await project();
    assert.deepEqual(
      g.surface_groups.map((x) => [x.name, facesOf(g, x.id).length]),
      QUADS.map(([n]) => [n.replace(' ', '_'), 2]),
    );
    assert.equal(await $('[data-part="ungrouped-note"]').isExisting(), false, 'the control: no note for a grouped file');

    await importThroughDialog(blank());
    const p = await project();
    console.log(`c1-import receipt: ${JSON.stringify(p.surface_groups.map((x) => [x.name, facesOf(p, x.id).length]))}`);
    assert.equal(p.surface_groups.length, 1);
    assert.equal(p.surface_groups[0].name, 'box_blank');
    assert.equal(facesOf(p, p.surface_groups[0].id).length, 12);
    const note = await $('[data-part="ungrouped-note"]');
    await note.waitForDisplayed({ timeout: 10_000 });
    assert.match(await note.getText(), /No groups in the file/);
    assert.equal(await $(`${row(p.surface_groups[0].id)} [data-group-faces]`).getAttribute('data-group-faces'), '12');
    assert.equal(await $('[data-part="check-verdict"]').getText(), 'Passed');
    ids.base = p.surface_groups[0].id;
  });

  it('c1-carve: walls carved from the view, a face moved into a group, two groups merged; each one undo step', async () => {
    await hook('frame');
    const p0 = await project();
    for (const wall of ['floor', 'ceiling', 'wall south']) {
      const faces = wallFaces(p0, wall);
      assert.equal(faces.length, 2);
      const g = await newGroup(faces, true);
      assert.deepEqual(facesOf(await project(), g.id), faces, wall);
      ids[wall] = g.id;
    }
    // North: one triangle to a new group, the other moved into it.
    const north = wallFaces(p0, 'wall north');
    const gn = await newGroup([north[0]], false);
    const depth = await m10.undoDepth();
    const pt = await pick(north[1], false);
    await viewMenu(pt, `move-to-group:${gn.id}`);
    assert.deepEqual(facesOf(await project(), gn.id), north, 'Move to group took the second triangle');
    assert.equal(await m10.undoDepth(), depth + 1, 'one undo step');
    ids['wall north'] = gn.id;

    // West: each triangle its own group, then merged by Ctrl+click and Merge.
    const west = wallFaces(p0, 'wall west');
    const w1 = await newGroup([west[0]], false);
    const w2 = await newGroup([west[1]], false);
    const text0 = await m10.projectJson();
    const depth0 = await m10.undoDepth();
    await clickSelector(row(w1.id));
    await ctrlClick(row(w2.id));
    const bar = await $('[data-action="merge-groups"]');
    await bar.waitForDisplayed({ timeout: 10_000 });
    await bar.click();
    await m10.idle();
    const p1 = await project();
    assert.equal(p1.surface_groups.some((g) => g.id === w2.id), false, 'the second group is gone');
    assert.deepEqual(facesOf(p1, w1.id), west);
    assert.equal(p1.surface_groups.find((g) => g.id === w1.id)?.name, w1.name, "the first's name is kept");
    assert.equal(await m10.undoDepth(), depth0 + 1);
    await m10.undo();
    assert.equal(await m10.projectJson(), text0, 'undoing the merge gives back the project text exactly');
    await m10.redo();
    assert.deepEqual(facesOf(await project(), w1.id), west);
    ids['wall west'] = w1.id;
    ids['wall east'] = ids.base;
    assert.deepEqual(facesOf(await project(), ids.base), wallFaces(p0, 'wall east'), 'what is left of the file group is the east wall');
  });

  it('c1-rename: F2 renames each group; each row shows its face count; a taken name is refused', async () => {
    for (const [wall, id] of Object.entries(ids).filter(([k]) => k !== 'base')) {
      await renameRow(id, wall);
      assert.equal((await project()).surface_groups.find((g) => g.id === id)?.name, wall);
      assert.equal(await $(`${row(id)} [data-group-faces]`).getAttribute('data-group-faces'), '2', `${wall} has 2 faces`);
    }
    const p = await project();
    assert.deepEqual(p.surface_groups.map((g) => g.name).sort(), QUADS.map((q) => q[0]).sort());
    // Control: another group's name.
    const text = await m10.projectJson();
    await renameRow(ids.floor, 'ceiling');
    assert.equal(await m10.projectJson(), text, 'the project is unchanged');
    assert.equal(await $('[data-part="group-issues"] [data-issue-code="GROUP_NAME_TAKEN"]').isExisting(), true);
    // A rename is one undo step.
    const depth = await m10.undoDepth();
    await renameRow(ids.floor, 'floor slab');
    assert.equal(await m10.undoDepth(), depth + 1);
    await m10.undo();
    assert.equal(groupNamed(await project(), 'floor')?.id, ids.floor);
  });

  it('c1-materials: library materials, absorption and scattering typed per band, transmission on and off', async () => {
    const plan: [string, string][] = [
      ['floor', '10% absorbing'],
      ['ceiling', '50% absorbing'],
      ['wall south', '20% absorbing'],
      ['wall north', '20% absorbing'],
      ['wall west', '30% absorbing'],
    ];
    for (const [wall, name] of plan) {
      await clickSelector(row(ids[wall]));
      await m10.setStep('materials');
      const m = await libraryMaterial(name);
      await giveMaterial(ids[wall], m);
      assert.equal(materialOf(await project(), (await project()).surface_groups.find((g) => g.id === ids[wall])!), name);
    }
    // Control: the east wall still has the placeholder, and Run says so.
    assert.ok((await m10.runBlockers()).includes('MATERIALS_UNASSIGNED'));

    // A new material, its absorption typed per band as one pasted row.
    await m10.setStep('materials');
    const before = new Set((await project()).materials.map((m) => m.id));
    await clickSelector('[data-action="add-material"]');
    await m10.idle();
    const mat = (await project()).materials.find((m) => !before.has(m.id))!;
    const n = mat.absorption.length;
    const alpha = Array.from({ length: n }, (_, i) => Math.round((0.05 + (0.6 * i) / Math.max(1, n - 1)) * 1000) / 1000);
    const rowIndex = await browser.execute((id: string) => [...document.querySelectorAll('[data-material-id]')].findIndex((r) => (r as HTMLElement).dataset.materialId === id), mat.id);
    await clickSelector(`[data-grid-cell="${rowIndex}:0"]`);
    await pasteInGrid(alpha.join('\t'));
    assert.deepEqual((await project()).materials.find((m) => m.id === mat.id)!.absorption.map(Number), alpha);
    // Scattering, one cell typed.
    await clickSelector('[data-quantity-tab="scattering"]');
    await $(`[data-grid-cell="${rowIndex}:0"]`).doubleClick();
    await browser.keys(['0', '.', '2', '5', 'Enter']);
    await m10.idle();
    assert.equal(Number((await project()).materials.find((m) => m.id === mat.id)!.scattering[0]), 0.25);
    // Transmission: typed in band 1, then switched off again.
    await clickSelector('[data-quantity-tab="transmission"]');
    await $(`[data-grid-cell="${rowIndex}:0"]`).doubleClick();
    await browser.keys(['2', '0', 'Enter']);
    await m10.idle();
    const tl = (await project()).materials.find((m) => m.id === mat.id)!.transmission_loss_db;
    assert.deepEqual(tl, [20, ...Array.from({ length: n - 1 }, () => null)]);
    await clickSelector(`[data-grid-cell="${rowIndex}:0"]`);
    await browser.keys(['Backspace', 'Enter']);
    await m10.idle();
    assert.equal((await project()).materials.find((m) => m.id === mat.id)!.transmission_loss_db, null, 'switched off, no transmission at all');
    await clickSelector('[data-quantity-tab="absorption"]');

    await giveMaterial(ids['wall east'], mat.id);
    assert.ok(!(await m10.runBlockers()).includes('MATERIALS_UNASSIGNED'), 'every group has a material');
  });

  it('c1-source: + Source off the centre; power, spectrum, a band level and directivity edited, each one undo step', async () => {
    await m10.setStep('sources');
    await clickSelector('[data-part="add-source"]');
    let s = (await project()).sources[0];
    console.log(`c1-source receipt: S1 at ${JSON.stringify(s.position)}`);
    assert.notDeepEqual(s.position, [3, 5, 1.5], 'not the box centroid');
    const depth = await m10.undoDepth();

    await typeInto('[data-props-step="sources"] [data-field="power.global_db"]', '95');
    s = (await project()).sources[0];
    assert.equal(s.power.global_db, 95);
    assert.equal(await m10.undoDepth(), depth + 1);

    // ES_VL from upstream's list: the core's values on the project's bands.
    const spectra = await browser.execute(() => [...(document.querySelector('[data-props-step="sources"] [data-field="spectrum"]') as HTMLSelectElement).options].map((o) => o.text));
    assert.ok(spectra.includes('ES_VL') && spectra.includes('Pink noise'), JSON.stringify(spectra));
    await $('[data-props-step="sources"] [data-field="spectrum"]').selectByVisibleText('ES_VL');
    await m10.idle();
    s = (await project()).sources[0];
    assert.equal(s.power.shape.kind, 'custom');
    assert.equal(s.power.global_db, 95, 'the overall level is kept');
    assert.equal(s.power.shape.relative_db?.length, (await project()).bands.frequencies_hz.length);

    // One band's level typed: 1 kHz to 90 dB; the others keep theirs.
    const freqs = (await project()).bands.frequencies_hz;
    const k = freqs.indexOf(1000);
    const levelsBefore = await browser.execute(() => [...document.querySelectorAll('[data-props-step="sources"] [data-field^="power.band."]')].map((e) => (e as HTMLInputElement).value));
    await typeInto(`[data-props-step="sources"] [data-field="power.band.${k}"]`, '90');
    s = (await project()).sources[0];
    const rel = s.power.shape.relative_db!.map(Number);
    assert.ok(Math.abs(rel[k] - 90) < 1e-9, `1 kHz is 90 dB: ${rel[k]}`);
    const levelsAfter = await browser.execute(() => [...document.querySelectorAll('[data-props-step="sources"] [data-field^="power.band."]')].map((e) => (e as HTMLInputElement).value));
    levelsBefore.forEach((v, i) => i !== k && assert.equal(levelsAfter[i], v, `band ${freqs[i]} keeps its level`));
    assert.equal(await $('[data-props-step="sources"] [data-field="spectrum"]').getValue(), 'custom');

    // Directivity: unidirectional along +x, a zero direction refused, back to omni.
    await $('[data-props-step="sources"] [data-field="directivity"]').selectByAttribute('value', 'unidirectional');
    await m10.idle();
    assert.deepEqual((await project()).sources[0].directivity, { kind: 'unidirectional', direction: [1, 0, 0] });
    const text = await m10.projectJson();
    await typeInto('[data-props-step="sources"] [data-field="direction.x"]', '0');
    assert.equal(await m10.projectJson(), text, 'a zero direction is refused, the project unchanged');
    assert.match(await $('[data-props-step="sources"] [data-part="emission"]').getText(), /direction_vector_zero/, "and the validator's refusal is shown");
    await browser.keys('Escape');
    await $('[data-props-step="sources"] [data-field="directivity"]').selectByAttribute('value', 'omni');
    await m10.idle();
    assert.deepEqual((await project()).sources[0].directivity, { kind: 'omni' });
    assert.equal(await m10.undoDepth(), depth + 5, 'power, spectrum, band, unidirectional, omni: five undo steps');
  });

  it('c1-receivers: two receivers and an ear-height plane', async () => {
    await m10.setStep('sources');
    await clickSelector('[data-part="add-receiver"]');
    await typeInto('[data-props-step="sources"] [data-field="position.y"]', '8');
    await clickSelector('[data-part="add-receiver"]');
    await clickSelector('[data-part="add-plane"]');
    const p = await project();
    assert.equal(p.point_receivers.length, 2);
    assert.equal(p.point_receivers[0].position[1], 8);
    assert.equal(p.surface_receivers.filter((r) => r.shape.kind === 'cutting_plane').length, 1);
    await m10.saveAs(saved());
  });

  let cpuRun = '';
  it('c1-run-cpu: SPPS on the CPU runs OK, loses nothing, and the Results step shows the acoustics and the plane map', async () => {
    const r = await runFromButton('cpu');
    const f = runFolder(saved(), r.run);
    console.log(`c1-run-cpu receipt: ${r.run} ${r.status} ${JSON.stringify(r.reasons)}; lost per band ${JSON.stringify(f.lost)}; stderr ${JSON.stringify(f.stderr)}`);
    assert.equal(r.status, 'OK', JSON.stringify(r.reasons));
    assert.ok(f.lost.length > 0 && f.lost.every((x: number) => x < 0.001), 'no band loses 0.1 % of its particles');
    assert.deepEqual(f.stderr, ['', ''], "the solver's and TetGen's stderr are empty");
    cpuRun = r.run;
    assert.equal(await resultsReady(r.run), 'ready');
    await clickSelector('[data-map-kind="plane|"]');
    let m: { run: string; error: string | null; faces: number; path: string } | null = null;
    await browser.waitUntil(async () => (m = await hook('m12Map')) !== null && m.run === r.run && m.error === null && m.faces > 0, {
      timeout: 60_000,
      timeoutMsg: `no plane map: ${JSON.stringify(m)}`,
    });
    console.log(`c1-run-cpu receipt: map ${JSON.stringify(m)}`);
  });

  it('c1-run-gpu: the same project on the GPU runs OK, and the Acoustics tab is ready', async () => {
    const gpu = await hook<{ available: boolean; reason?: string | null }>('gpuStatus');
    assert.equal(gpu.available, true, `the GPU entry is available: ${JSON.stringify(gpu)}`);
    const r = await runFromButton('gpu');
    const f = runFolder(saved(), r.run);
    console.log(`c1-run-gpu receipt: ${r.run} ${r.status} on ${r.gpu_device}; lost per band ${JSON.stringify(f.lost)}`);
    assert.equal(r.status, 'OK', JSON.stringify(r.reasons));
    assert.ok(r.gpu_device, 'the row names the device');
    assert.notEqual(r.run, cpuRun);
    assert.equal(await resultsReady(r.run), 'ready');
  });

  it('c1-grouped: the grouped box to results: six groups picked at once take one material', async () => {
    await importThroughDialog(grouped());
    let p = await project();
    await clickSelector(row(p.surface_groups[0].id));
    for (const g of p.surface_groups.slice(1)) await ctrlClick(row(g.id));
    assert.equal((await hook<Sel>('selection')).groups.length, 6, 'six groups picked');
    await m10.setStep('materials');
    const m = await libraryMaterial('30% absorbing');
    await clickSelector(`[data-material-option="${m}"]`);
    await m10.idle();
    p = await project();
    assert.ok(p.surface_groups.every((g) => g.material === m), 'every group took it');
    await m10.setStep('sources');
    await clickSelector('[data-part="add-source"]');
    await clickSelector('[data-part="add-receiver"]');
    const file = path.join(WORK(), 'grouped', 'box.simpa');
    await m10.saveAs(file);
    const r = await runFromButton('cpu');
    console.log(`c1-grouped receipt: ${r.run} ${r.status} ${JSON.stringify(r.reasons)}`);
    assert.equal(r.status, 'OK', JSON.stringify(r.reasons));
    assert.equal(await resultsReady(r.run), 'ready');
  });
});
