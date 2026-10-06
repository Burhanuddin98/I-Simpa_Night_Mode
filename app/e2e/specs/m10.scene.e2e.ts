// The scene package's gate ids (PLAN.md 3 and 6.3), and the extra checks section 3 gives it.
//   m10-a-run        raw hall: [data-part=run] disabled, data-blockers holds GEOMETRY_REFUSED;
//                    teaching room: data-blockers is exactly M11_PENDING
//   m10-b-materials  corrected hall: [data-step="materials"] [data-part="sub"] reads "0 of 10 set";
//                    tutorial1_box.simpa reads "3 of 3 set"
//   m10-e-outside    R1's position.x = 20 + Enter: [data-issue-code="RECEIVER_OUTSIDE"] shown,
//                    projectJson() and undoDepth() unchanged; x = 4 accepted
//   m10-e-label      R1's name "a/b" + Enter: [data-issue-code="LABEL_UNSAFE"] shown, project
//                    unchanged; "Front row" accepted
// Extra checks (not gate ids, but they must pass): the step subs, the dirty dot, adding and
// switching a variant, the scene filter, themed scrollbars on every scroll container, and the
// import dialog's preselected unit and up axis.
//
// Text is typed as real key events, one character per action (a key pressed twice in one action
// is a repeat, not a second character), into the field the UI shows.
import { strict as assert } from 'node:assert';
import { randomUUID } from 'node:crypto';
import path from 'node:path';
import { clickSelector } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { env, type Op, type Vec3 } from '../lib/types.ts';

const repo = (rel: string) => path.join(env('M10_REPO'), rel);
const TEACHING_ROOM = () => repo('tests/fixtures/ui/teaching_room.simpa');
const CORRECTED_HALL = () => repo('testdata/elmia_corrected.ply');
const BOX = () => repo('tests/fixtures/rooms/tutorial1_box.simpa');

/** WebDriver key codes (the specs import nothing from wdio). */
const CTRL = '\uE009';
const ENTER = '\uE007';
const BACKSPACE = '\uE003';

interface Named {
  id: string;
  name: string;
  position: Vec3;
}
interface ProjectFile {
  point_receivers: Named[];
  sources: Named[];
  variants: { id: string; name: string }[];
  active_variant: string | null;
}
const project = async (): Promise<ProjectFile> => JSON.parse(await m10.projectJson());

/** Replaces the text of the field at `selector`, as a user would: click, select all, type. */
async function typeInto(selector: string, text: string): Promise<void> {
  const el = await $(selector);
  await el.waitForDisplayed({ timeout: 30_000 });
  await el.click();
  await browser.keys([CTRL, 'a']);
  await browser.keys(BACKSPACE);
  for (const ch of text) await browser.keys(ch);
}

/** Types `text` into the field and presses Enter, then waits for the app to settle. */
async function commit(selector: string, text: string): Promise<void> {
  await typeInto(selector, text);
  await browser.keys(ENTER);
  await m10.idle();
}

async function subs(): Promise<Record<string, string | null>> {
  return browser.execute(() =>
    Object.fromEntries(
      [...document.querySelectorAll('[data-step]')].map((el) => [
        el.getAttribute('data-step') ?? '?',
        el.querySelector('[data-part="sub"]')?.textContent ?? null,
      ]),
    ),
  );
}

async function selectReceiver(name: string): Promise<Named> {
  const r = (await project()).point_receivers.find((x) => x.name === name);
  assert.ok(r, `no receiver named ${name}`);
  await clickSelector(`.scene [data-entity="point_receiver:${r.id}"]`);
  await $('[data-field="position.x"]').waitForDisplayed({ timeout: 30_000 });
  return r;
}

describe('M10 scene', () => {
  before(async () => {
    await waitForHooks(['idle', 'openProject', 'importModel', 'saveAs', 'edit', 'undo', 'undoDepth', 'projectJson', 'openImportDialog']);
  });

  it('m10-a-run: Run is disabled, and why is named', async () => {
    await m10.importModel(env('M10_ELMIA_RAW'), 'm', 'z');
    const run = await $('[data-part="run"]');
    assert.equal(await run.isEnabled(), false, 'Run is disabled on the raw hall');
    const raw = ((await run.getAttribute('data-blockers')) ?? '').split(' ');
    console.log(`m10-a-run receipt: raw hall data-blockers = ${raw.join(' ')}`);
    assert.ok(raw.includes('GEOMETRY_REFUSED'), `data-blockers: ${raw.join(' ')}`);
    assert.deepEqual(raw, await m10.runBlockers(), 'the DOM shows the backend blockers');
    assert.ok(((await run.getAttribute('title')) ?? '').includes('GEOMETRY_REFUSED'), 'the tooltip names the blocker as text');

    // Positive control: a room with nothing wrong has no blocker at all and Run is enabled
    // (M11 wired Run; PLAN.md 4.5), so "disabled" above is not vacuous.
    await m10.openProject(TEACHING_ROOM());
    const room = await run.getAttribute('data-blockers');
    console.log(`m10-a-run receipt: teaching room data-blockers = '${room}'`);
    assert.equal(room, '');
    assert.deepEqual(await m10.runBlockers(), []);
    assert.equal(await run.isEnabled(), true, 'Run is enabled on a clean project');
  });

  it("m10-b-materials: the step bar reads Materials '0 of 10 set' on the corrected hall", async () => {
    const sub = () => $('[data-step="materials"] [data-part="sub"]').getText();
    await m10.importModel(CORRECTED_HALL(), 'm', 'z');
    const hall = await sub();
    console.log(`m10-b-materials receipt: corrected hall reads '${hall}'`);
    assert.equal(hall, '0 of 10 set');
    // Control: every group of the box has a real material.
    await m10.openProject(BOX());
    const box = await sub();
    console.log(`m10-b-materials receipt: tutorial1_box reads '${box}'`);
    assert.equal(box, '3 of 3 set');
  });

  it('m10-e-outside: a receiver placed outside shows RECEIVER_OUTSIDE and changes nothing', async () => {
    await m10.openProject(TEACHING_ROOM());
    const r1 = await selectReceiver('R1');
    const before = await m10.projectJson();
    const depth = await m10.undoDepth();

    await commit('[data-field="position.x"]', '20');
    const shown = await $('[data-issue-code="RECEIVER_OUTSIDE"]');
    await shown.waitForDisplayed({ timeout: 30_000 });
    const text = await shown.getText();
    console.log(`m10-e-outside receipt: ${text}`);
    assert.ok(text.includes('RECEIVER_OUTSIDE'), text);
    assert.equal(await m10.projectJson(), before, 'the project is unchanged');
    assert.equal(await m10.undoDepth(), depth, 'the history is unchanged');

    // Positive control: x = 4 is inside, accepted, one undo step, and the refusal goes away.
    await commit('[data-field="position.x"]', '4');
    assert.equal(await m10.undoDepth(), depth + 1);
    const moved = (await project()).point_receivers.find((r) => r.id === r1.id);
    assert.deepEqual(moved?.position, [4, r1.position[1], r1.position[2]]);
    assert.equal(await $('[data-field="position.x"]').getValue(), '4');
    await $('[data-issue-code="RECEIVER_OUTSIDE"]').waitForExist({ reverse: true, timeout: 30_000 });
  });

  it("m10-e-label: the label 'a/b' is refused with LABEL_UNSAFE", async () => {
    await m10.openProject(TEACHING_ROOM());
    const r1 = await selectReceiver('R1');
    const before = await m10.projectJson();
    const depth = await m10.undoDepth();

    await commit('[data-field="name"]', 'a/b');
    const shown = await $('[data-issue-code="LABEL_UNSAFE"]');
    await shown.waitForDisplayed({ timeout: 30_000 });
    const text = await shown.getText();
    console.log(`m10-e-label receipt: ${text}`);
    assert.ok(text.includes('LABEL_UNSAFE'), text);
    assert.equal(await m10.projectJson(), before, 'the project is unchanged');
    assert.equal(await m10.undoDepth(), depth, 'the history is unchanged');

    // Positive control: a safe label is accepted.
    await commit('[data-field="name"]', 'Front row');
    assert.equal(await m10.undoDepth(), depth + 1);
    assert.equal((await project()).point_receivers.find((r) => r.id === r1.id)?.name, 'Front row');
    assert.equal(await $(`.scene [data-entity="point_receiver:${r1.id}"] .row-name`).getText(), 'Front row');
    await $('[data-issue-code="LABEL_UNSAFE"]').waitForExist({ reverse: true, timeout: 30_000 });
  });

  it('scene: the step subs read the project (closed, refused, no model; assigned; sources · receivers)', async () => {
    await m10.openProject(TEACHING_ROOM());
    assert.deepEqual(await subs(), { geometry: 'room closed', materials: '6 of 6 set', sources: '1 source · 3 receivers', simulate: '', results: '' });
    const r4: Op = {
      op: 'add_point_receiver',
      index: 3,
      receiver: { id: randomUUID(), name: 'R4', position: [6, 3, 1.2], orientation: [1, 0, 0], background_noise: null, solver_id: null },
    };
    assert.equal((await m10.edit(r4)).applied, true);
    assert.equal((await subs()).sources, '1 source · 4 receivers');

    await m10.importModel(env('M10_ELMIA_RAW'), 'm', 'z');
    assert.equal((await subs()).geometry, 'not closed');

    // File › New project, through the menu.
    await clickSelector('[data-menu="File"]');
    await clickSelector('[data-menu-item="new-project"]');
    await m10.idle();
    assert.deepEqual(await subs(), { geometry: 'no model', materials: '0 of 0 set', sources: '0 sources · 0 receivers', simulate: '', results: '' });
  });

  it('scene: the dirty dot follows edits, saves and undo', async () => {
    const dot = () => $('[data-part="project-tab"] [aria-label="Unsaved changes"]').isExisting();
    await m10.openProject(TEACHING_ROOM());
    assert.equal(await dot(), false, 'a project just opened is clean');
    const r2 = (await project()).point_receivers.find((r) => r.name === 'R2');
    assert.ok(r2);
    assert.equal((await m10.edit({ op: 'move_point_receiver', id: r2.id, position: [7.5, 4, 1.2] })).applied, true);
    assert.equal(await dot(), true, 'an edit shows the dot');
    assert.equal(await m10.dirty(), true);
    await m10.saveAs(path.join(env('M10_WORK'), 'scene-dirty.simpa'));
    assert.equal(await dot(), false, 'a save clears it');
    await m10.undo();
    assert.equal(await dot(), true, 'undoing past the save shows it again');
  });

  it('scene: a variant is added, made active, switched and renamed', async () => {
    const tabs = () =>
      browser.execute(() =>
        [...document.querySelectorAll('[data-part="variants"] [role="tab"]')].map((b) => ({
          id: b.getAttribute('data-variant'),
          name: b.textContent,
          on: b.getAttribute('aria-selected') === 'true',
        })),
      );
    await m10.openProject(TEACHING_ROOM());
    assert.deepEqual(await tabs(), [{ id: 'baseline', name: 'Baseline', on: true }]);
    const depth = await m10.undoDepth();

    await clickSelector('[data-part="variant-add"]');
    await m10.idle();
    let t = await tabs();
    assert.equal(t.length, 2);
    assert.equal(t[1].name, 'Variant 1');
    assert.deepEqual(t.map((x) => x.on), [false, true], 'the new variant is active');
    let p = await project();
    assert.equal(p.variants.length, 1);
    assert.equal(p.active_variant, t[1].id);
    assert.equal(await m10.undoDepth(), depth + 1, 'adding and activating is one undo step');

    await clickSelector('[data-part="variants"] [data-variant="baseline"]');
    await m10.idle();
    assert.deepEqual((await tabs()).map((x) => x.on), [true, false]);
    assert.equal((await project()).active_variant, null);

    const vid = t[1].id ?? '';
    await clickSelector(`[data-part="variants"] [data-variant="${vid}"]`);
    await m10.idle();
    assert.equal((await project()).active_variant, vid);
    assert.equal(await $('[data-part="variant"]').getText(), 'Variant 1', 'the status bar names the active variant');

    await $(`[data-part="variants"] [data-variant="${vid}"]`).doubleClick();
    await commit('[data-part="variant-name"]', 'Treated rear wall');
    t = await tabs();
    assert.equal(t[1].name, 'Treated rear wall');
    p = await project();
    assert.equal(p.variants[0].name, 'Treated rear wall');
    assert.equal(p.active_variant, vid);
  });

  it('scene: the filter narrows the list by name and by material', async () => {
    const rows = () =>
      browser.execute(() =>
        [...document.querySelectorAll('.scene [data-entity]')].map(
          (e) => `${(e.getAttribute('data-entity') ?? '').split(':')[0]}:${e.querySelector('.row-name')?.textContent ?? ''}`,
        ),
      );
    await m10.openProject(TEACHING_ROOM());
    const all = await rows();
    assert.equal(all.length, 10, all.join(', '));

    await typeInto('[data-part="scene-filter"]', 'wall');
    assert.deepEqual(await rows(), [
      'surface_group:Left wall',
      'surface_group:Right wall',
      'surface_group:Front wall',
      'surface_group:Rear wall',
    ]);
    await typeInto('[data-part="scene-filter"]', 'plaster');
    assert.deepEqual(await rows(), ['surface_group:Left wall', 'surface_group:Right wall', 'surface_group:Front wall']);
    await typeInto('[data-part="scene-filter"]', 'R2');
    assert.deepEqual(await rows(), ['point_receiver:R2']);
    await typeInto('[data-part="scene-filter"]', '');
    assert.deepEqual(await rows(), all);
  });

  it('scene: every scroll container has the themed scrollbar colours', async () => {
    const probe = () =>
      browser.execute(() => {
        const p = document.createElement('div');
        document.body.appendChild(p);
        const want = getComputedStyle(p).getPropertyValue('scrollbar-color');
        p.remove();
        const found: string[] = [];
        const bad: string[] = [];
        for (const el of document.querySelectorAll<HTMLElement>('body *')) {
          const cs = getComputedStyle(el);
          if (!/(auto|scroll)/.test(`${cs.overflowX} ${cs.overflowY}`)) continue;
          const name = `${el.tagName.toLowerCase()}.${[...el.classList].join('.')}`;
          found.push(name);
          const got = cs.getPropertyValue('scrollbar-color');
          if (got !== want || got === 'auto') bad.push(`${name}: ${got}`);
        }
        return { want, found, bad };
      });
    await m10.openProject(TEACHING_ROOM());
    const seen = new Set<string>();
    for (const step of ['geometry', 'materials', 'sources']) {
      for (const tab of ['console', 'runs']) {
        await clickSelector(`[data-step="${step}"]`);
        await clickSelector(`[data-dock-tab="${tab}"]`);
        const r = await probe();
        assert.ok(r.want && r.want !== 'auto', `the probe's scrollbar-color is ${r.want}`);
        assert.deepEqual(r.bad, [], `${step}/${tab}: ${r.bad.join('; ')}`);
        r.found.forEach((f) => seen.add(f));
      }
    }
    await hook('openImportDialog', CORRECTED_HALL());
    await $('[data-part="import-dialog"]').waitForDisplayed({ timeout: 30_000 });
    const d = await probe();
    assert.deepEqual(d.bad, [], d.bad.join('; '));
    d.found.forEach((f) => seen.add(f));
    await clickSelector('[data-part="import-cancel"]');
    const names = [...seen].join(' ');
    console.log(`scrollbar receipt: ${d.want} on ${[...seen].join(', ')}`);
    for (const region of ['scene-list', 'props', 'dock-body', 'dialog']) {
      assert.ok(names.includes(`.${region}`), `no scroll container .${region} among ${names}`);
    }
  });

  it('scene: the import dialog opens on m and z, shows every choice, and imports', async () => {
    const checked = (field: string) => $(`[data-part="import-dialog"] [data-field="${field}"] [aria-checked="true"]`).getText();
    const options = (field: string) =>
      browser.execute(
        (f: string) =>
          [...document.querySelectorAll(`[data-part="import-dialog"] [data-field="${f}"] [role="radio"]`)].map((b) => b.textContent),
        field,
      );
    await hook('openImportDialog', CORRECTED_HALL());
    await $('[data-part="import-dialog"]').waitForDisplayed({ timeout: 30_000 });
    assert.deepEqual(await options('unit'), ['m', 'cm', 'mm', 'ft', 'in']);
    assert.deepEqual(await options('up'), ['z', 'y']);
    assert.equal(await checked('unit'), 'm');
    assert.equal(await checked('up'), 'z');

    // A changed choice does not carry over to the next opening.
    await clickSelector('[data-part="import-dialog"] [data-field="unit"] [data-option="mm"]');
    assert.equal(await checked('unit'), 'mm');
    await clickSelector('[data-part="import-cancel"]');
    await $('[data-part="import-dialog"]').waitForExist({ reverse: true, timeout: 30_000 });
    await hook('openImportDialog', CORRECTED_HALL());
    await $('[data-part="import-dialog"]').waitForDisplayed({ timeout: 30_000 });
    assert.equal(await checked('unit'), 'm');

    await clickSelector('[data-part="import-confirm"]');
    await m10.idle();
    await $('[data-part="import-dialog"]').waitForExist({ reverse: true, timeout: 30_000 });
    const s = await subs();
    assert.equal(s.geometry, 'room closed');
    assert.equal(s.materials, '0 of 10 set');
  });
});
