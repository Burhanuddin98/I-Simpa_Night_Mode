// The project package's gate ids (docs/investigations/2026-09-29-m11/PLAN.md 4.2, "Row 22, one id
// each", and m11-b18), each with its control:
//   m11-r22-a9   the teaching room, one edit, File › New project: [data-prompt] names the project.
//                Cancel leaves project, dirty flag and undo depth as they were; Don't save gives a
//                new project and leaves the file's sha256 as it was; Save changes the file (with
//                the edit in it), then gives the new project. Control: a clean project shows no
//                prompt; the same prompt comes from File › Open… (the openPath hook).
//   m11-r22-a3   upstream's tutorial_1.proj through File › Open…'s path (openPath), saved as, is
//                byte-identical to `simpa import-proj` of the same file; the step subs show its
//                groups and sources; its notes are INFO lines. Control: an 8-byte bad.proj is a
//                FAIL line with its code, and the project is unchanged.
//   m11-r22-g42  the box: the camera after load (S0), a real pointer orbit (S1, not S0; it stays
//                S1 without a click), then a click on [data-tool="frame"]: S0 again within 1e-9 m.
//   m11-r22-m26  the teaching room: switching S1 off from the scene list is refused inline as
//                SOURCE_NONE, the project unchanged. Control: with S2 added, S1 switched off from
//                the Sources panel is applied (enabled false, "off" in both lists), and undo
//                restores it.
//   m11-r22-m5   the rear wall's material, law Lambert in [data-law]: reflection_law "lambert",
//                one undo step; undo gives back the original project text.
//   m11-r22-m1   "+ From library", "30% absorbing" (reference id 21): the new material's
//                absorption in all 6 bands is exactly the core's value (widen_f32 of the f32 0.3:
//                the shortest decimal that reads back to it, 0.3, which is also what tutorial 1's
//                .proj import gives it), name and colour the core's, law specular; assigned to
//                the Floor, the Materials sub stays "6 of 6 set". The entry then reads "in project".
//   m11-b18      the raw hall: the volume row holds no digit and says the model is refused; the
//                three dimensions share two decimals. Control: the corrected hall shows the volume
//                `simpa check` measures, at the status bar's spelling, inside [data-geometry].
//
// Its files, under <M11_WORK>\project (C:): five copies of the teaching room (a9, a3, m26, m5,
// m1), t1_cli.simpa, t1_app.simpa and bad.proj, 8 in all. The repository and upstream's checkout
// are only read. simpa.exe (beside app.exe, built by the gate) runs `import-proj` and `check`,
// neither of which launches a solver.
import { strict as assert } from 'node:assert';
import { spawnSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { clickSelector } from '../lib/dom.ts';
import { compareFiles } from '../lib/files.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { env, type Op, type Vec3 } from '../lib/types.ts';

const repo = (rel: string) => path.join(env('M11_REPO'), rel);
/** Upstream's checkout, read only: the gate's -Upstream when it passes it, else its default. */
const upstream = (rel: string) => path.join(process.env.M11_UPSTREAM ?? 'B:\\repos\\I-Simpa-upstream', rel);
const TEACHING_ROOM = () => repo('tests/fixtures/ui/teaching_room.simpa');
const BOX = () => repo('tests/fixtures/rooms/tutorial1_box.simpa');
const CORRECTED_HALL = () => repo('testdata/elmia_corrected.ply');
const RAW_HALL = () => upstream('src/isimpa/resources/doc/tutorial/tutorial 2/elmia.ply');
const TUTORIAL_1 = () => upstream('src/isimpa/resources/doc/tutorial/tutorial 1/tutorial_1.proj');
/** The CLI the gate builds beside app.exe. */
const SIMPA = () => path.join(path.dirname(env('M11_APP')), 'simpa.exe');
const WORK = () => path.join(env('M11_WORK'), 'project');

/**
 * The core's `widen_f32` (crates/simpa-core/src/config_xml/num.rs), recomputed: the shortest
 * decimal that reads back to the same f32, as an f64; the plain widening when none does.
 */
function widenF32(x: number): number {
  for (let p = 1; p <= 9; p++) {
    const v = Number(x.toPrecision(p));
    if (Math.fround(v) === x) return v;
  }
  return x;
}

interface Named {
  id: string;
  name: string;
}
interface SourceFile extends Named {
  enabled: boolean;
  position: Vec3;
}
interface MaterialFile extends Named {
  color: string;
  absorption: (number | string)[];
  scattering: (number | string)[];
  reflection_law: string | string[];
}
interface ProjectFile {
  name: string;
  bands: { frequencies_hz: number[] };
  surface_groups: (Named & { material: string })[];
  materials: MaterialFile[];
  sources: SourceFile[];
  point_receivers: (Named & { position: Vec3 })[];
}
interface LibraryEntry {
  reference_id: number;
  name: string;
  absorption: number | string;
  color: string;
}
interface Cam {
  position: number[];
  direction: number[];
  target: number[] | null;
}

const project = async (): Promise<ProjectFile> => JSON.parse(await m10.projectJson());
const sha256 = (file: string) => createHash('sha256').update(readFileSync(file)).digest('hex');

/** A fresh copy of the teaching room on C:, so a Save never writes into the repository. */
function freshRoom(name: string): string {
  const out = path.join(WORK(), `${name}.simpa`);
  copyFileSync(TEACHING_ROOM(), out);
  return out;
}

/** Calls a hook without waiting for it: an action that stops at the save prompt resolves only
 * once the prompt is answered, which the spec does by clicking. */
async function startHook(name: string, ...args: unknown[]): Promise<void> {
  const err = await browser.execute(
    (n: string, a: unknown[]) => {
      const api = (window as unknown as { __m10?: Record<string, (...x: unknown[]) => unknown> }).__m10;
      const fn = api?.[n];
      if (typeof fn !== 'function') return `window.__m10.${n} is not registered`;
      void Promise.resolve(fn(...a)).catch(() => {});
      return null;
    },
    name,
    args,
  );
  if (err) throw new Error(err);
}

const tabName = () => $('[data-part="project-tab"]').getText();

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

/** File › New project, through the menu, as a user does it. */
async function menuNewProject(): Promise<void> {
  await clickSelector('[data-menu="File"]');
  await clickSelector('[data-menu-item="new-project"]');
}

/** The prompt's title once it is shown. */
async function promptTitle(): Promise<string> {
  const title = await $('[data-prompt] [data-part="prompt-title"]');
  await title.waitForDisplayed({ timeout: 30_000 });
  return title.getText();
}

/** Clicks one of the prompt's buttons, waits for it to close and for the app to settle. */
async function answer(choice: 'save' | 'discard' | 'cancel'): Promise<void> {
  await clickSelector(`[data-prompt] [data-choice="${choice}"]`);
  await $('[data-prompt]').waitForExist({ reverse: true, timeout: 30_000 });
  await m10.idle();
}

/** Waits until the open project is a new one (File › New project's "Untitled"). */
async function waitNewProject(): Promise<void> {
  await browser.waitUntil(async () => (await tabName()).startsWith('Untitled') && (await hook<null | object>('promptOpen')) === null, {
    timeout: 30_000,
    timeoutMsg: 'no new project after the prompt',
  });
  await m10.idle();
}

/** The Console's lines of one class, each line's whole text (the dock owns the inner layout). */
async function consoleText(cls: 'FAIL' | 'INFO'): Promise<string[]> {
  await clickSelector('[data-dock-tab="console"]');
  return browser.execute((c: string) => [...document.querySelectorAll(`.console-line.${c}`)].map((e) => e.textContent ?? ''), cls);
}

const dist = (a: readonly number[], b: readonly number[]) => Math.hypot(...a.map((x, i) => x - b[i]));

/** A geometry fact as the app spells it (sceneModel.ts `fact`): at most `digits` decimals, no
 * trailing zeros. */
function fact(v: number, digits: number): string {
  const s = v.toFixed(digits);
  return s.includes('.') ? s.replace(/\.?0+$/, '') : s;
}

function simpa(args: string[]): { status: number | null; stdout: string; stderr: string } {
  const r = spawnSync(SIMPA(), args, { encoding: 'utf8', windowsHide: true });
  return { status: r.status, stdout: r.stdout, stderr: r.stderr };
}

describe('M11 project', () => {
  before(async () => {
    mkdirSync(WORK(), { recursive: true });
    await waitForHooks(['idle', 'openProject', 'importModel', 'saveAs', 'edit', 'undo', 'undoDepth', 'projectJson', 'setStep', 'openPath', 'openProj', 'promptOpen', 'cameraState']);
  });

  it('m11-r22-a9: New and Open ask to save a changed project; Cancel, Don\'t save and Save each do what they say', async () => {
    const file = freshRoom('a9');
    const moveR2 = async (position: Vec3) => {
      const r2 = (await project()).point_receivers.find((r) => r.name === 'R2');
      assert.ok(r2, 'the teaching room has R2');
      const out = await m10.edit({ op: 'move_point_receiver', id: r2.id, position });
      assert.equal(out.applied, true, JSON.stringify(out.refusals));
    };

    // Control: a clean project leaves without a prompt.
    await m10.openProject(file);
    assert.equal(await m10.dirty(), false);
    await menuNewProject();
    await waitNewProject();
    assert.equal(await $('[data-prompt]').isExisting(), false, 'a clean project must not prompt');

    // One edit: New asks, naming the project.
    await m10.openProject(file);
    const name = await tabName();
    await moveR2([7.5, 4, 1.2]);
    assert.equal(await m10.dirty(), true);
    const before = await m10.projectJson();
    const depth = await m10.undoDepth();
    const sha0 = sha256(file);
    await menuNewProject();
    const title = await promptTitle();
    console.log(`m11-r22-a9 receipt: prompt '${title}' for project '${name}'`);
    assert.equal(title, `Save changes to ${name}?`);
    assert.deepEqual(await hook('promptOpen'), { name });

    // Cancel: nothing changes.
    await answer('cancel');
    assert.equal(await m10.projectJson(), before, 'Cancel changed the project');
    assert.equal(await m10.dirty(), true);
    assert.equal(await m10.undoDepth(), depth);
    assert.equal(await tabName(), name);

    // Don't save: a new project, the file untouched.
    await menuNewProject();
    await promptTitle();
    await answer('discard');
    await waitNewProject();
    assert.equal(sha256(file), sha0, "Don't save wrote the file");

    // Save: the file changes, and holds the edit; then the new project.
    await m10.openProject(file);
    await moveR2([7.5, 4, 1.2]);
    await menuNewProject();
    await promptTitle();
    await answer('save');
    await waitNewProject();
    assert.notEqual(sha256(file), sha0, 'Save did not write the file');
    const saved = JSON.parse(readFileSync(file, 'utf8')) as ProjectFile;
    assert.deepEqual(saved.point_receivers.find((r) => r.name === 'R2')?.position, [7.5, 4, 1.2]);

    // Control: File › Open… (after its native dialog) asks the same.
    await m10.openProject(file);
    await moveR2([7, 4, 1.2]);
    const edited = await m10.projectJson();
    await startHook('openPath', BOX());
    assert.equal(await promptTitle(), `Save changes to ${await tabName()}?`);
    await answer('cancel');
    assert.equal(await m10.projectJson(), edited, 'Cancel on Open changed the project');
    assert.equal(await m10.dirty(), true);
  });

  it('m11-r22-a3: an upstream .proj opens through File › Open… and saves byte-identical to simpa import-proj', async () => {
    const proj = TUTORIAL_1();
    assert.ok(existsSync(proj), `no ${proj}`);
    const cli = path.join(WORK(), 't1_cli.simpa');
    const r = simpa(['import-proj', proj, cli, '--json']);
    assert.equal(r.status, 0, `simpa import-proj exited ${r.status}: ${r.stderr}`);
    const report = JSON.parse(r.stdout) as { notes: string[] };

    // Start clean, so Open goes straight through (the prompt has its own id).
    await m10.openProject(freshRoom('a3'));
    await hook('openPath', proj);
    const app = path.join(WORK(), 't1_app.simpa');
    await m10.saveAs(app);
    const diff = compareFiles(app, cli);
    console.log(`m11-r22-a3 receipt: ${app} (${readFileSync(app).length} B) against ${cli}: ${diff ?? 'byte-identical'}`);
    assert.equal(diff, null, diff ?? '');

    // The step subs are the .proj's.
    const p = JSON.parse(readFileSync(cli, 'utf8')) as ProjectFile;
    const placeholder = (id: string) => {
      const m = p.materials.find((x) => x.id === id);
      return !m || (m.name === 'Default' && [...m.absorption, ...m.scattering].every((v) => v === 0));
    };
    const assigned = p.surface_groups.filter((g) => !placeholder(g.material)).length;
    const s = await subs();
    // Re-pinned 2026-10-07 (C4): the subs are in words since 0aaae74 (10-06, "Step labels and
    // badges in words", Burhan's 04:55 order; sceneModel.ts stepSubs), the counts unchanged.
    const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? '' : 's'}`;
    assert.equal(s.materials, `${assigned} of ${p.surface_groups.length} set`);
    assert.equal(s.sources, `${plural(p.sources.filter((x) => x.enabled).length, 'source')} · ${plural(p.point_receivers.length, 'receiver')}`);

    // Its notes are INFO lines.
    const info = await consoleText('INFO');
    for (const note of report.notes) assert.ok(info.some((l) => l.includes(note)), `no INFO line for the note '${note}'`);

    // Control: 8 bytes that are not a zip: a FAIL line with its code, the project unchanged.
    const bad = path.join(WORK(), 'bad.proj');
    writeFileSync(bad, 'NOTAZIP!');
    const before = await m10.projectJson();
    const fails = (await consoleText('FAIL')).length;
    await assert.rejects(hook('openProj', bad), 'a bad .proj must be refused');
    await m10.idle();
    const added = (await consoleText('FAIL')).slice(fails);
    console.log(`m11-r22-a3 receipt: bad.proj -> ${JSON.stringify(added)}`);
    assert.ok(
      added.some((l) => l.includes('bad.proj') && /\(([A-Z][A-Z0-9_]+)\)\s*$/.test(l)),
      `no FAIL line naming bad.proj with its code: ${JSON.stringify(added)}`,
    );
    assert.equal(await m10.projectJson(), before, 'a refused .proj changed the project');
  });

  it('m11-r22-g42: the Frame model button brings the camera back to the framing of the load', async () => {
    await m10.openProject(BOX());
    await clickSelector('[data-tool="select"]');
    const s0 = await hook<Cam>('cameraState');
    const r = await browser.execute(() => {
      const c = document.querySelector('canvas')!.getBoundingClientRect();
      return { x: Math.round(c.left + c.width / 2), y: Math.round(c.top + c.height / 2) };
    });
    // A real orbit: 170 px across and 50 px down.
    await browser
      .action('pointer', { parameters: { pointerType: 'mouse' } })
      .move({ x: r.x, y: r.y, origin: 'viewport', duration: 0 })
      .down({ button: 0 })
      .move({ x: r.x + 85, y: r.y + 25, origin: 'viewport', duration: 100 })
      .move({ x: r.x + 170, y: r.y + 50, origin: 'viewport', duration: 100 })
      .up({ button: 0 })
      .perform();
    await m10.idle();
    const s1 = await hook<Cam>('cameraState');
    const moved = dist(s1.position, s0.position);
    assert.ok(moved > 0.5, `the drag orbited the camera (moved ${moved} m)`);

    // Control: nothing frames it back by itself.
    await browser.pause(500);
    await m10.idle();
    const still = await hook<Cam>('cameraState');
    assert.ok(dist(still.position, s1.position) < 1e-9, 'the camera moved with no input');

    const button = await $('[data-tool="frame"]');
    assert.equal(await button.getAttribute('title'), 'Frame model (Home)');
    await button.click();
    await m10.idle();
    const s2 = await hook<Cam>('cameraState');
    const back = dist(s2.position, s0.position);
    const aim = dist(s2.direction, s0.direction);
    const target = s2.target && s0.target ? dist(s2.target, s0.target) : Number.NaN;
    console.log(`m11-r22-g42 receipt: orbit moved ${moved} m; after Frame: position ${back} m, direction ${aim}, target ${target} m from S0`);
    assert.ok(back < 1e-9 && aim < 1e-9 && target < 1e-9, `Frame model did not restore S0: ${JSON.stringify({ s0, s2 })}`);
  });

  it('m11-r22-m26: switching off the only source is refused inline; with a second source it is applied and undone', async () => {
    await m10.openProject(freshRoom('m26'));
    const p = await project();
    assert.equal(p.sources.length, 1);
    const s1 = p.sources[0];
    assert.equal(s1.enabled, true);
    const before = await m10.projectJson();
    const depth = await m10.undoDepth();

    // Refused, from the scene list, on the Geometry step: the message is inline beside it.
    await m10.setStep('geometry');
    await clickSelector(`.scene [data-source-toggle="${s1.id}"]`);
    await m10.idle();
    const refusal = await $('.scene [data-issue-code="SOURCE_NONE"]');
    await refusal.waitForDisplayed({ timeout: 30_000 });
    const text = await refusal.getText();
    console.log(`m11-r22-m26 receipt: ${text}`);
    assert.ok(text.startsWith('FAIL SOURCE_NONE'), text);
    assert.equal(await m10.projectJson(), before, 'a refused switch changed the project');
    assert.equal(await m10.undoDepth(), depth);
    assert.equal(await $(`.scene [data-source-toggle="${s1.id}"]`).getAttribute('aria-checked'), 'true');

    // Control: with S2, S1 switches off from the Sources panel.
    const s2: Op = {
      op: 'add_source',
      index: 1,
      source: {
        id: randomUUID(),
        name: 'S2',
        enabled: true,
        position: [7, 2, 1.5],
        power: { global_db: 85, shape: { kind: 'pink' } },
        directivity: { kind: 'omni' },
        delay_s: 0,
        group: null,
        solver_id: null,
      },
    };
    const added = await m10.edit(s2);
    assert.equal(added.applied, true, JSON.stringify(added.refusals));
    await m10.setStep('sources');
    const depth2 = await m10.undoDepth();
    await clickSelector(`[data-part="sources-panel"] [data-source-toggle="${s1.id}"]`);
    await m10.idle();
    const q = await project();
    assert.equal(q.sources.find((s) => s.id === s1.id)?.enabled, false);
    assert.equal(await m10.undoDepth(), depth2 + 1);
    assert.ok((await $(`[data-point-row="source:${s1.id}"]`).getText()).includes('off'), 'the Sources list shows off');
    assert.ok((await $(`.scene [data-entity="source:${s1.id}"]`).getText()).includes('off'), 'the scene list shows off');
    assert.equal(await $(`[data-part="sources-panel"] [data-source-toggle="${s1.id}"]`).getText(), 'off');
    assert.equal(await $('[data-issue-code="SOURCE_NONE"]').isExisting(), false, 'the earlier refusal is put away by the new attempt');

    // Undo restores it.
    await m10.undo();
    assert.equal((await project()).sources.find((s) => s.id === s1.id)?.enabled, true);
    assert.equal(await $(`.scene [data-entity="source:${s1.id}"] [data-part="source-off"]`).isExisting(), false);
    assert.equal(await $(`[data-part="sources-panel"] [data-source-toggle="${s1.id}"]`).getAttribute('aria-checked'), 'true');
  });

  it('m11-r22-m5: the Law column sets Lambert on the rear wall material as one undo step, and undo gives the bytes back', async () => {
    await m10.openProject(freshRoom('m5'));
    const p = await project();
    const rear = p.surface_groups.find((g) => g.name === 'Rear wall');
    assert.ok(rear, 'the teaching room has a Rear wall');
    const mid = rear.material;
    assert.equal(p.materials.find((m) => m.id === mid)?.reflection_law, 'specular', 'the control: Lambert is a change');
    const before = await m10.projectJson();
    const depth = await m10.undoDepth();

    await m10.setStep('materials');
    await waitForHooks(['materialsGrid']);
    const select = await $(`select[data-law="${mid}"]`);
    await select.waitForDisplayed({ timeout: 30_000 });
    assert.equal(await select.getValue(), 'specular');
    await select.selectByAttribute('value', 'lambert');
    await m10.idle();
    const q = await project();
    assert.equal(q.materials.find((m) => m.id === mid)?.reflection_law, 'lambert');
    for (const m of q.materials.filter((x) => x.id !== mid)) {
      assert.deepEqual(m, p.materials.find((x) => x.id === m.id), `${m.name} changed too`);
    }
    assert.equal(await m10.undoDepth(), depth + 1);
    assert.equal(await $(`select[data-law="${mid}"]`).getValue(), 'lambert');

    await m10.undo();
    assert.equal(await m10.projectJson(), before, 'undo did not give back the original project');
  });

  it("m11-r22-m1: '30% absorbing' from the library carries the core's exact values, and assigned to the Floor keeps 6 / 6", async () => {
    await m10.openProject(freshRoom('m1'));
    await m10.setStep('materials');
    await waitForHooks(['materialsGrid', 'materialLibrary']);
    const lib = await hook<LibraryEntry[]>('materialLibrary');
    assert.equal(lib.length, 11, JSON.stringify(lib.map((e) => e.name)));
    assert.ok(!lib.some((e) => e.name === 'Default'), 'the placeholder is not in the library');
    const entry = lib.find((e) => e.reference_id === 21);
    assert.ok(entry, 'no reference material 21');
    assert.equal(entry.name, '30% absorbing');
    // The core's widen_f32 of upstream's f32 0.3: the shortest decimal that reads back to the same
    // f32 (config_xml/num.rs), recomputed here, and the value the core's own .proj import gives
    // "30% absorbing" in tutorial 1 (the gate's `simpa import-proj`, M11_T1_CLI). Not
    // f64::from(0.3f32) = 0.30000001192092896: widen_f32 is not that.
    const widened = widenF32(Math.fround(0.3));
    const t1 = JSON.parse(readFileSync(env('M11_T1_CLI'), 'utf8')) as ProjectFile;
    const imported = t1.materials.find((m) => m.name === '30% absorbing');
    assert.ok(imported, "tutorial 1's import has no '30% absorbing'");
    console.log(`m11-r22-m1 receipt: library ${JSON.stringify(entry.absorption)}; widen_f32 recomputed ${JSON.stringify(widened)}; tutorial 1's .proj import ${JSON.stringify(imported.absorption[0])}`);
    assert.ok(Object.is(entry.absorption, widened), `the library's ${entry.absorption} is not widen_f32's ${widened}`);
    assert.equal(JSON.stringify(entry.absorption), JSON.stringify(imported.absorption[0]), 'the library value is what a .proj import makes');

    const p = await project();
    const depth = await m10.undoDepth();
    await clickSelector('[data-action="library"]');
    await clickSelector('[data-library-add="21"]');
    await m10.idle();
    const q = await project();
    assert.equal(q.materials.length, p.materials.length + 1);
    const added = q.materials[q.materials.length - 1];
    const bands = q.bands.frequencies_hz.length;
    assert.equal(bands, 6);
    console.log(`m11-r22-m1 receipt: ${added.name} ${added.color} ${JSON.stringify(added.absorption)} law ${JSON.stringify(added.reflection_law)}`);
    assert.equal(added.absorption.length, bands);
    for (const a of added.absorption) {
      assert.ok(Object.is(a, entry.absorption), `absorption ${a} is not the core's ${entry.absorption}`);
      assert.equal(JSON.stringify(a), JSON.stringify(entry.absorption), 'the same JSON number text');
    }
    assert.deepEqual(added.scattering, Array(bands).fill(0));
    assert.equal(added.name, entry.name);
    assert.equal(added.color, entry.color);
    assert.equal(added.reflection_law, 'specular');
    assert.equal(await m10.undoDepth(), depth + 1, 'one undo step');

    // The entry now reads "in project" and is not added twice.
    await clickSelector('[data-action="library"]');
    const again = await $('[data-library-add="21"]');
    await again.waitForDisplayed({ timeout: 30_000 });
    assert.equal(await again.isEnabled(), false);
    assert.equal(await again.getAttribute('data-in-project'), added.id);
    await browser.keys('\uE00C'); // Esc closes the list

    // Assigned to the Floor: every group still has a material.
    const floor = q.surface_groups.find((g) => g.name === 'Floor');
    assert.ok(floor, 'the teaching room has a Floor');
    await clickSelector(`.scene [data-entity="surface_group:${floor.id}"]`);
    await clickSelector(`[data-material-option="${added.id}"]`);
    await m10.idle();
    assert.equal((await project()).surface_groups.find((g) => g.id === floor.id)?.material, added.id);
    assert.equal((await subs()).materials, '6 of 6 set');
  });

  it('m11-b18: a refused model shows no volume, and the three dimensions share one precision', async () => {
    const dims = () =>
      browser.execute(() => [...document.querySelectorAll('[data-dimension] .v')].map((e) => e.textContent ?? ''));
    const volume = () => $('[data-part="volume"]');

    // The raw hall: refused by the check (the CLI says so too, exit 3).
    const raw = RAW_HALL();
    assert.ok(existsSync(raw), `no ${raw}`);
    assert.equal(simpa(['check', raw, '--unit', 'm', '--up', 'z', '--json']).status, 3, 'the raw hall must be refused');
    await m10.importModel(raw, 'm', 'z');
    await m10.setStep('geometry');
    const refused = await (await volume()).getText();
    const rawDims = await dims();
    console.log(`m11-b18 receipt: raw hall '${refused}', dimensions ${JSON.stringify(rawDims)}`);
    assert.ok(!/\d/.test(refused), `a digit in the refused model's volume row: ${refused}`);
    assert.ok(/refused/.test(refused), refused);
    assert.equal(await (await volume()).getAttribute('data-volume'), 'none');
    assert.equal(rawDims.length, 3);
    for (const d of rawDims) assert.match(d, /^\d+\.\d{2} m$/);

    // Control: the corrected hall shows the air's volume the check measures (backlog 85: the
    // inside of a closed obstacle is not air), as a geometry fact.
    const r = simpa(['check', CORRECTED_HALL(), '--unit', 'm', '--up', 'z', '--json']);
    assert.equal(r.status, 0, r.stderr);
    const measured = (JSON.parse(r.stdout) as { report: { measures: { air_volume_m3: number } } }).report.measures.air_volume_m3;
    await m10.importModel(CORRECTED_HALL(), 'm', 'z');
    const shown = await (await volume()).getText();
    const hallDims = await dims();
    console.log(`m11-b18 receipt: corrected hall '${shown}' (simpa check ${measured} m³), dimensions ${JSON.stringify(hallDims)}`);
    assert.equal(shown, `Air volume ${fact(measured, 1)} m³`);
    assert.equal(await (await volume()).getAttribute('data-volume'), 'air');
    assert.equal(
      await browser.execute(() => !!document.querySelector('[data-part="volume"]')?.closest('[data-geometry]')),
      true,
      'the volume is a geometry fact',
    );
    for (const d of hallDims) assert.match(d, /^\d+\.\d{2} m$/);
  });
});
