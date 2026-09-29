// The viewport package's gate ids (PLAN.md 3 and 6.1), and the package's extra checks.
//   m10-a-highlight  raw hall: __m10.highlightedFaceCount() > 0 and the chip "FAIL · n faces
//                    highlighted" visible; teaching room: 0 and no chip
//   m10-d            the box: double-click the ceiling (aimAtFace) selects faces [10, 11], group
//                    Ceiling; a wall double-click selects 2 faces, not the Walls group's 8
//   m10-g            the hall, every step, both view tabs, all dock tabs, then the box: one canvas
// Extra (PLAN.md 3, "Other checks"): a pick after an orbit maps to the right project face; the
// Plan tab is a top orthographic view; a placement click puts a receiver 1.2 m (a source 1.5 m)
// above the floor hit, and a click on a wall places nothing.
//
// Every pointer action is a real WebDriver pointer sequence on the canvas; the hooks only aim
// the camera and read state.
import { strict as assert } from 'node:assert';
import path from 'node:path';
import { clickSelector } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { env } from '../lib/types.ts';

const repo = (rel: string) => path.join(env('M10_REPO'), rel);
const TEACHING_ROOM = () => repo('tests/fixtures/ui/teaching_room.simpa');
const BOX = () => repo('tests/fixtures/rooms/tutorial1_box.simpa');
const CORRECTED_HALL = () => repo('testdata/elmia_corrected.ply');

type Point = { x: number; y: number };
type Sel = { faces: number[]; groups: string[] };
type Cam = {
  view: string;
  projection: 'perspective' | 'orthographic';
  position: [number, number, number];
  direction: [number, number, number];
  target: [number, number, number] | null;
};
type Project = {
  point_receivers: { name: string; position: [number, number, number] }[];
  sources: { name: string; position: [number, number, number] }[];
};

const VIEWPORT_HOOKS = ['highlightedFaceCount', 'selection', 'facesOfGroup', 'aimAtFace', 'frame', 'faceClientPoint', 'cameraState'];

const canvases = () => browser.execute(() => document.querySelectorAll('canvas').length);
const selection = () => hook<Sel>('selection');
const facesOfGroup = (name: string) => hook<number[]>('facesOfGroup', name);
const aimAtFace = (face: number) => hook<Point | null>('aimAtFace', face);
const cameraState = () => hook<Cam>('cameraState');

/** A left click at a client point, as the mouse does it. */
async function clickAt(p: Point): Promise<void> {
  await browser
    .action('pointer', { parameters: { pointerType: 'mouse' } })
    .move({ x: p.x, y: p.y, origin: 'viewport', duration: 0 })
    .down({ button: 0 })
    .up({ button: 0 })
    .perform();
  await m10.idle();
}

/** A double-click at a client point: two clicks inside the system's double-click time. */
async function doubleClickAt(p: Point): Promise<void> {
  await browser
    .action('pointer', { parameters: { pointerType: 'mouse' } })
    .move({ x: p.x, y: p.y, origin: 'viewport', duration: 0 })
    .down({ button: 0 })
    .up({ button: 0 })
    .pause(60)
    .down({ button: 0 })
    .up({ button: 0 })
    .perform();
  await m10.idle();
}

/** Clicks far enough apart in time that no two make a double-click. */
const settleClick = () => browser.pause(700);

/** The project, once `ready` says the edit a click started has landed. */
async function projectWhen(ready: (p: Project) => boolean, what: string): Promise<Project> {
  let last: Project | null = null;
  await browser.waitUntil(
    async () => {
      last = JSON.parse(await m10.projectJson()) as Project;
      return ready(last);
    },
    { timeout: 15_000, interval: 100, timeoutMsg: `${what} did not happen within 15 s` },
  );
  await m10.idle();
  return last as unknown as Project;
}

async function chip(): Promise<{ shown: boolean; text: string } | null> {
  return browser.execute(() => {
    const el = document.querySelector<HTMLElement>('[data-part="check-chip"]');
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return { shown: r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== 'hidden', text: el.textContent ?? '' };
  });
}

describe('M10 viewport', () => {
  before(async () => {
    await waitForHooks(['idle', 'openProject', 'importModel', 'undoDepth', 'projectJson', ...VIEWPORT_HOOKS]);
  });

  it('m10-a-highlight: the refused faces of the raw hall are highlighted', async () => {
    await m10.importModel(env('M10_ELMIA_RAW'), 'm', 'z');
    const n = await hook<number>('highlightedFaceCount');
    console.log(`m10-a-highlight receipt: raw hall, ${n} faces uploaded to the check-highlight overlay`);
    assert.ok(n > 0, `highlightedFaceCount() = ${n}`);
    const c = await chip();
    assert.ok(c, 'the check chip is in the DOM');
    assert.ok(c.shown, 'the check chip is visible');
    assert.equal(c.text, `FAIL · ${n} faces highlighted`);

    // Negative control: the teaching room passes the check, so nothing is highlighted.
    await m10.openProject(TEACHING_ROOM());
    assert.equal(await hook<number>('highlightedFaceCount'), 0);
    assert.equal(await chip(), null, 'no check chip for a model that passes');
  });

  it('m10-d: a double-click on the box ceiling selects its 2 faces, group Ceiling', async () => {
    await m10.openProject(BOX());
    await hook('frame');
    const ceiling = await facesOfGroup('Ceiling');
    assert.deepEqual(ceiling, [10, 11], 'the fixture: Ceiling is faces 10 and 11');
    const p = await aimAtFace(ceiling[0]);
    assert.ok(p, 'aimAtFace found a point where the first hit is the ceiling');
    await doubleClickAt(p);
    const sel = await selection();
    console.log(`m10-d receipt: ceiling double-click at (${p.x}, ${p.y}) -> ${JSON.stringify(sel)}`);
    assert.deepEqual(sel, { faces: [10, 11], groups: ['Ceiling'] });

    // Control: a wall. The fill follows the plane, not the group: 2 faces, not Walls' 8.
    await settleClick();
    const walls = await facesOfGroup('Walls');
    assert.equal(walls.length, 8);
    const q = await aimAtFace(walls[0]);
    assert.ok(q, 'aimAtFace found a point where the first hit is the wall');
    await doubleClickAt(q);
    const wall = await selection();
    console.log(`m10-d receipt: wall double-click at (${q.x}, ${q.y}) -> ${JSON.stringify(wall)}`);
    assert.equal(wall.faces.length, 2, JSON.stringify(wall));
    assert.ok(wall.faces.includes(walls[0]));
    assert.ok(wall.faces.every((f) => walls.includes(f)));
    assert.deepEqual(wall.groups, ['Walls']);
  });

  it('viewport: a pick after the camera orbits maps to the right project face', async () => {
    await m10.openProject(BOX());
    await hook('frame');
    await settleClick();
    const before = await cameraState();
    const r = await browser.execute(() => {
      const c = document.querySelector('canvas')!.getBoundingClientRect();
      return { x: Math.round(c.left + c.width / 2), y: Math.round(c.top + c.height / 2) };
    });
    // A real drag: orbit by 170 px across and 50 px down.
    await browser
      .action('pointer', { parameters: { pointerType: 'mouse' } })
      .move({ x: r.x, y: r.y, origin: 'viewport', duration: 0 })
      .down({ button: 0 })
      .move({ x: r.x + 85, y: r.y + 25, origin: 'viewport', duration: 100 })
      .move({ x: r.x + 170, y: r.y + 50, origin: 'viewport', duration: 100 })
      .up({ button: 0 })
      .perform();
    await m10.idle();
    const after = await cameraState();
    const moved = Math.hypot(...after.position.map((x, k) => x - before.position[k]));
    assert.ok(moved > 0.5, `the drag orbited the camera (moved ${moved} m)`);
    // OrbitControls.update() re-clamps the target about its cursor on every call (subtract,
    // clampLength, add), which moves it by a few ulps; a pan would move it by metres. A
    // nanometre is the bound.
    assert.ok(after.target && before.target, 'the orbit controls report a target');
    const drift = Math.hypot(...after.target.map((x, k) => x - (before.target as number[])[k]));
    assert.ok(drift < 1e-9, `an orbit keeps the target (it moved ${drift} m: ${JSON.stringify(before.target)} to ${JSON.stringify(after.target)})`);
    assert.deepEqual(await selection(), { faces: [], groups: [] }, 'a drag selects nothing');

    const picked: string[] = [];
    for (let f = 0; f < 12; f++) {
      const p = await hook<Point | null>('faceClientPoint', f);
      if (!p) continue;
      await settleClick();
      await clickAt(p);
      const sel = await selection();
      assert.deepEqual(sel.faces, [f], `a click on face ${f}'s centroid at (${p.x}, ${p.y}) picked ${JSON.stringify(sel)}`);
      picked.push(`${f}:${sel.groups.join('+')}`);
    }
    console.log(`viewport pick receipt: after the orbit, faces picked by clicking their centroids: ${picked.join(', ')}`);
    assert.ok(picked.length >= 3, `only ${picked.length} faces were in view after the orbit`);
  });

  it('viewport: the Plan tab is a top orthographic view, and picks through it', async () => {
    await m10.openProject(TEACHING_ROOM());
    await clickSelector('[data-view="plan"]');
    await m10.idle();
    const cam = await cameraState();
    assert.equal(cam.view, 'plan');
    assert.equal(cam.projection, 'orthographic');
    assert.deepEqual(cam.direction.map((x) => Math.round(x * 1e9) / 1e9 + 0), [0, 0, -1], 'looking straight down');
    assert.equal(await $('[data-view="plan"]').getAttribute('aria-selected'), 'true');
    assert.equal(await $('[data-view="section"]').isEnabled(), false, 'Section stays disabled');
    assert.equal(await canvases(), 1);
    // From above, the floor is what a click takes.
    const floor = await facesOfGroup('Floor');
    const p = await hook<Point | null>('faceClientPoint', floor[0]);
    assert.ok(p, 'the floor is visible in the plan');
    await settleClick();
    await clickAt(p);
    assert.deepEqual(await selection(), { faces: [floor[0]], groups: ['Floor'] });
    await clickSelector('[data-view="perspective"]');
    await m10.idle();
    assert.equal((await cameraState()).projection, 'perspective');
  });

  it('viewport: a placement click puts a receiver 1.2 m and a source 1.5 m above the floor', async () => {
    await m10.openProject(TEACHING_ROOM());
    const start = JSON.parse(await m10.projectJson()) as Project;
    const depth = await m10.undoDepth();
    const floor = await facesOfGroup('Floor');

    await clickSelector('[data-tool="place-receiver"]');
    const p = await aimAtFace(floor[0]);
    assert.ok(p, 'the floor is in view');
    await settleClick();
    await clickAt(p);
    const one = await projectWhen((x) => x.point_receivers.length === start.point_receivers.length + 1, 'the receiver placement');
    const r = one.point_receivers[one.point_receivers.length - 1];
    console.log(`viewport placement receipt: ${r.name} at ${JSON.stringify(r.position)}`);
    assert.equal(r.name, 'R4');
    assert.equal(r.position[2], 1.2, 'ear height above a floor at z = 0');
    assert.ok(r.position[0] > 0 && r.position[0] < 10 && r.position[1] > 0 && r.position[1] < 6, 'inside the room');
    assert.equal(await m10.undoDepth(), depth + 1);

    await clickSelector('[data-tool="place-source"]');
    const q = await aimAtFace(floor[1]);
    assert.ok(q, 'the floor is in view');
    await settleClick();
    await clickAt(q);
    const two = await projectWhen((x) => x.sources.length === start.sources.length + 1, 'the source placement');
    const s = two.sources[two.sources.length - 1];
    console.log(`viewport placement receipt: ${s.name} at ${JSON.stringify(s.position)}`);
    assert.equal(s.position[2], 1.5);
    assert.equal(await m10.undoDepth(), depth + 2);

    // A wall is not a floor: nothing is placed and the project is unchanged.
    await clickSelector('[data-tool="place-receiver"]');
    const wall = (await facesOfGroup('Left wall'))[0];
    const w = await aimAtFace(wall);
    assert.ok(w, 'the wall is in view');
    const before = await m10.projectJson();
    await settleClick();
    await clickAt(w);
    await browser.pause(500);
    await m10.idle();
    assert.equal(await m10.projectJson(), before, 'a wall click places nothing');
    const notice = await browser.execute(() => document.querySelector('[data-part="place-hint"]')?.textContent ?? '');
    assert.ok(notice.startsWith('Not a floor'), `the hint says why: ${notice}`);
    assert.equal(await m10.undoDepth(), depth + 2);
    await clickSelector('[data-tool="select"]');
  });

  it('m10-g: document.querySelectorAll("canvas").length == 1 throughout', async () => {
    const seen: string[] = [];
    const one = async (where: string) => {
      const n = await canvases();
      assert.equal(n, 1, `${n} canvases ${where}`);
      seen.push(where);
    };
    await one('at start');
    await m10.importModel(CORRECTED_HALL(), 'm', 'z');
    await one('after the corrected hall');
    for (const step of ['geometry', 'materials', 'sources', 'simulate', 'results']) {
      await clickSelector(`[data-step="${step}"]`);
      await m10.idle();
      await one(`on step ${step}`);
    }
    for (const view of ['plan', 'perspective']) {
      await clickSelector(`[data-view="${view}"]`);
      await m10.idle();
      await one(`in the ${view} view`);
    }
    for (const tab of ['acoustics', 'console', 'runs']) {
      await clickSelector(`[data-dock-tab="${tab}"]`);
      await m10.idle();
      await one(`on dock tab ${tab}`);
    }
    await m10.openProject(BOX());
    await one('after the box');
    console.log(`m10-g receipt: one canvas at each of ${seen.length} points: ${seen.join('; ')}`);
  });
});
