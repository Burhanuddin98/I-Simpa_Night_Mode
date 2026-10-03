// Scope row 15 (1)'s spec (docs/investigations/2026-10-03-row15/PLAN.md, order of work 4): face
// regrouping (G19) and grouped receivers on `.proj` import (M37), driven by the pointer and the
// clicks a user would make. Its checks, each with its control:
//   row15-regroup           the teaching room: a double-click picks the left wall's 2 faces, a
//                           right click on them opens the view's menu, New group from selection
//                           makes `Group 1` with their material (Painted plaster) holding exactly
//                           those faces, selected in the Scene panel, one undo step; a material
//                           picked for it in the Materials step lands on it alone; two undos give
//                           back the project text exactly. Control: with no faces picked the
//                           Edit menu's entry is disabled and a right click opens no menu
//   row15-receiver-folders  tutorial 1's `.proj` with Receiver 1 moved into `Stalls / Front` and
//                           Receiver 2 into `Balcony` (the folders upstream's GUI makes):
//                           opened, both receivers are listed with their folder, and the scene
//                           filter finds one by its folder. Control: the unedited `.proj` lists the
//                           same receivers with no folder
//
// Its files, under <M11_WORK>\groups (C:): a copy of the teaching room and the edited `.proj`.
// The repository and upstream's checkout are only read.
import { strict as assert } from 'node:assert';
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { crc32, inflateRawSync } from 'node:zlib';
import { clickSelector } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { env } from '../lib/types.ts';

const repo = (rel: string) => path.join(env('M11_REPO'), rel);
/** Upstream's checkout, read only: the gate's -Upstream when it passes it, else its default. */
const upstream = (rel: string) => path.join(process.env.M11_UPSTREAM ?? 'B:\\repos\\I-Simpa-upstream', rel);
const TEACHING_ROOM = () => repo('tests/fixtures/ui/teaching_room.simpa');
const TUTORIAL_1 = () => upstream('src/isimpa/resources/doc/tutorial/tutorial 1/tutorial_1.proj');
const WORK = () => path.join(env('M11_WORK'), 'groups');

type Point = { x: number; y: number };
type Sel = { faces: number[]; groups: string[] };
interface Named {
  id: string;
  name: string;
}
interface Group extends Named {
  material: string;
}
interface Receiver extends Named {
  group?: string | null;
}
interface ProjectFile {
  surface_groups: Group[];
  materials: Named[];
  point_receivers: Receiver[];
  geometry: { faces: [number, number, number, string][] };
}

const project = async (): Promise<ProjectFile> => JSON.parse(await m10.projectJson());
const faceIds = (p: ProjectFile, group: string) =>
  p.geometry.faces.flatMap((f, i) => (f[3] === group ? [i] : []));

/** A pointer press of `button` (0 left, 2 right) at a client point, as the mouse does it. */
async function pressAt(p: Point, button: 0 | 2, times = 1): Promise<void> {
  let a = browser.action('pointer', { parameters: { pointerType: 'mouse' } }).move({ x: p.x, y: p.y, origin: 'viewport', duration: 0 });
  for (let i = 0; i < times; i++) {
    if (i > 0) a = a.pause(60);
    a = a.down({ button }).up({ button });
  }
  await a.perform();
  await m10.idle();
}

const menuShown = () => browser.execute(() => document.querySelector('[data-part="viewport-menu"]') !== null);

// ---- the edited .proj (M37) -----------------------------------------------------------------

/** The entries of a zip (stored or deflated), by name, in archive order. */
function unzip(bytes: Buffer): [string, Buffer][] {
  let eocd = bytes.length - 22;
  while (eocd >= 0 && bytes.readUInt32LE(eocd) !== 0x06054b50) eocd--;
  assert.ok(eocd >= 0, 'no end of central directory');
  const count = bytes.readUInt16LE(eocd + 10);
  let p = bytes.readUInt32LE(eocd + 16);
  const out: [string, Buffer][] = [];
  for (let i = 0; i < count; i++) {
    assert.equal(bytes.readUInt32LE(p), 0x02014b50);
    const method = bytes.readUInt16LE(p + 10);
    const size = bytes.readUInt32LE(p + 20);
    const nameLen = bytes.readUInt16LE(p + 28);
    const extra = bytes.readUInt16LE(p + 30);
    const comment = bytes.readUInt16LE(p + 32);
    const local = bytes.readUInt32LE(p + 42);
    const name = bytes.subarray(p + 46, p + 46 + nameLen).toString('utf8');
    const data = local + 30 + bytes.readUInt16LE(local + 26) + bytes.readUInt16LE(local + 28);
    const raw = bytes.subarray(data, data + size);
    out.push([name, method === 8 ? inflateRawSync(raw) : Buffer.from(raw)]);
    p += 46 + nameLen + extra + comment;
  }
  return out;
}

/** A zip of `entries`, every one stored (method 0). */
function zip(entries: [string, Buffer][]): Buffer {
  const locals: Buffer[] = [];
  const centrals: Buffer[] = [];
  let offset = 0;
  for (const [name, data] of entries) {
    const n = Buffer.from(name, 'utf8');
    const crc = crc32(data);
    const lh = Buffer.alloc(30);
    lh.writeUInt32LE(0x04034b50, 0);
    lh.writeUInt16LE(20, 4);
    lh.writeUInt32LE(crc, 14);
    lh.writeUInt32LE(data.length, 18);
    lh.writeUInt32LE(data.length, 22);
    lh.writeUInt16LE(n.length, 26);
    const ch = Buffer.alloc(46);
    ch.writeUInt32LE(0x02014b50, 0);
    ch.writeUInt16LE(20, 4);
    ch.writeUInt16LE(20, 6);
    ch.writeUInt32LE(crc, 16);
    ch.writeUInt32LE(data.length, 20);
    ch.writeUInt32LE(data.length, 24);
    ch.writeUInt16LE(n.length, 28);
    ch.writeUInt32LE(offset, 42);
    locals.push(lh, n, data);
    centrals.push(ch, n);
    offset += 30 + n.length + data.length;
  }
  const cd = Buffer.concat(centrals);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(cd.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, cd, end]);
}

const RECEIVER1 = '<recepteurp name="Receiver 1" eid="8" wxid="1473"';
const RECEIVER2 = '<recepteurp name="Receiver 2" eid="8" wxid="1632"';

/** Tutorial 1's receivers moved into folders, as `geometry_import_proj.rs`'s `tutorial1_grouped`. */
function grouped(xml: string): string {
  const close = '</recepteurp>';
  const at1 = xml.indexOf(RECEIVER1);
  const at2 = xml.indexOf(RECEIVER2);
  assert.ok(at1 >= 0 && at2 > at1, 'tutorial 1 lists Receiver 1, then Receiver 2');
  const end1 = xml.indexOf(close, at1) + close.length;
  const end2 = xml.indexOf(close, at2) + close.length;
  return (
    xml.slice(0, at1) +
    '<recepteursp name="Stalls" eid="7" wxid="9001" exp="1"><recepteursp name="Front" eid="7" wxid="9002" exp="1">' +
    xml.slice(at1, end1) +
    '</recepteursp></recepteursp>' +
    xml.slice(end1, at2) +
    '<recepteursp name="Balcony" eid="7" wxid="9003" exp="1">' +
    xml.slice(at2, end2) +
    '</recepteursp>' +
    xml.slice(end2)
  );
}

/** Tutorial 1's `.proj` with its receivers in folders, written to `to` (the run reports left out). */
function writeGroupedProj(to: string): void {
  const entries = unzip(readFileSync(TUTORIAL_1())).filter(([n]) => !n.includes('/report/') && !n.endsWith('/'));
  const config = entries.find(([n]) => n.endsWith('/projet_config.xml'));
  assert.ok(config, 'tutorial 1 has a projet_config.xml');
  config[1] = Buffer.from(grouped(config[1].toString('utf8')), 'utf8');
  writeFileSync(to, zip(entries));
}

const receiverRows = () =>
  browser.execute(() =>
    [...document.querySelectorAll('.scene [data-entity^="point_receiver:"]')].map((row) => ({
      name: row.querySelector('.row-name')?.textContent ?? '',
      folder: row.querySelector('[data-receiver-group]')?.getAttribute('data-receiver-group') ?? null,
    })),
  );

describe('Scope row 15 (1): groups', () => {
  before(async () => {
    await waitForHooks(['idle', 'openProject', 'openProj', 'projectJson', 'undo', 'undoDepth', 'setStep', 'selection', 'aimAtFace', 'frame']);
    mkdirSync(WORK(), { recursive: true });
  });

  it('row15-regroup: picked faces become a new group with their material; a material set on it; undo restores the project', async () => {
    const room = path.join(WORK(), 'teaching_room.simpa');
    copyFileSync(TEACHING_ROOM(), room);
    await m10.openProject(room);
    await m10.setStep('geometry');
    await hook('frame');
    const p0 = await project();
    const text0 = await m10.projectJson();
    const depth0 = await m10.undoDepth();
    const left = p0.surface_groups.find((g) => g.name === 'Left wall');
    assert.ok(left, 'the teaching room has a Left wall');
    const plaster = p0.materials.find((m) => m.id === left.material);
    assert.equal(plaster?.name, 'Painted plaster');
    const leftFaces = faceIds(p0, left.id);
    assert.equal(leftFaces.length, 2);

    // Control: nothing picked, so the Edit entry is disabled and a right click opens no menu.
    await clickSelector('[data-menu="Edit"]');
    const entry = await $('[data-menu-item="new-group-from-selection"]');
    await entry.waitForDisplayed({ timeout: 30_000 });
    assert.equal(await entry.isEnabled(), false, 'disabled with no faces picked');
    await browser.keys('\uE00C');
    const target = await hook<Point | null>('aimAtFace', leftFaces[0]);
    assert.ok(target, 'aimAtFace found a point where the first hit is the left wall');
    await pressAt(target, 2);
    assert.equal(await menuShown(), false, 'no menu with no faces picked');

    // A double-click picks the wall's flat surface; a right click on it opens the menu.
    await browser.pause(700);
    await pressAt(target, 0, 2);
    const sel = await hook<Sel>('selection');
    assert.deepEqual(sel, { faces: leftFaces, groups: ['Left wall'] });
    await pressAt(target, 2);
    assert.equal(await menuShown(), true, 'the view menu opens on picked faces');
    await clickSelector('[data-part="viewport-menu"] [data-menu-item="new-group-from-selection"]');
    await m10.idle();

    const p1 = await project();
    assert.equal(p1.surface_groups.length, p0.surface_groups.length + 1);
    const g = p1.surface_groups[p1.surface_groups.length - 1];
    console.log(`row15-regroup receipt: faces ${JSON.stringify(leftFaces)} -> group '${g.name}' material ${g.material} (${plaster?.name})`);
    assert.equal(g.name, 'Group 1');
    assert.equal(g.material, plaster?.id, "the faces' own material");
    assert.deepEqual(faceIds(p1, g.id), leftFaces);
    assert.deepEqual(faceIds(p1, left.id), [], 'the left wall is emptied, and stays');
    assert.ok(p1.surface_groups.some((x) => x.id === left.id));
    assert.equal(await m10.undoDepth(), depth0 + 1, 'one undo step');
    const row = await $(`.scene [data-entity="surface_group:${g.id}"]`);
    await row.waitForDisplayed({ timeout: 30_000 });
    assert.equal(await row.getAttribute('aria-pressed'), 'true', 'the new group is selected');
    assert.match(await row.getText(), /Group 1/);

    // A material for the new group alone, picked in the Materials step.
    const wood = p1.materials.find((m) => m.name === 'Slotted wood panel');
    assert.ok(wood);
    await m10.setStep('materials');
    await clickSelector(`[data-material-option="${wood.id}"]`);
    await m10.idle();
    const p2 = await project();
    assert.equal(p2.surface_groups.find((x) => x.id === g.id)?.material, wood.id);
    for (const x of p1.surface_groups.filter((x) => x.id !== g.id)) {
      assert.equal(p2.surface_groups.find((y) => y.id === x.id)?.material, x.material, `${x.name} unchanged`);
    }
    assert.notEqual(await m10.projectJson(), text0, 'the control: the edited text differs');

    await m10.undo();
    await m10.undo();
    assert.equal(await m10.projectJson(), text0, 'two undos give back the project text exactly');
    assert.equal(await m10.undoDepth(), depth0);
  });

  it('row15-receiver-folders: a .proj with receivers in folders lists each with its folder', async () => {
    // Control: the unedited project's receivers have no folder.
    await hook('openProj', TUTORIAL_1());
    await m10.idle();
    const flat = await receiverRows();
    assert.deepEqual(flat, [
      { name: 'Receiver 1', folder: null },
      { name: 'Receiver 2', folder: null },
    ]);

    const edited = path.join(WORK(), 'tutorial_1_receiver_groups.proj');
    writeGroupedProj(edited);
    await hook('openProj', edited);
    await m10.idle();
    const p = await project();
    console.log(`row15-receiver-folders receipt: ${JSON.stringify(p.point_receivers.map((r) => [r.name, r.group]))}`);
    assert.deepEqual(
      p.point_receivers.map((r) => [r.name, r.group]),
      [
        ['Receiver 1', 'Stalls / Front'],
        ['Receiver 2', 'Balcony'],
      ],
    );
    assert.deepEqual(await receiverRows(), [
      { name: 'Receiver 1', folder: 'Stalls / Front' },
      { name: 'Receiver 2', folder: 'Balcony' },
    ]);

    // The scene filter finds a receiver by its folder.
    const filter = await $('[data-part="scene-filter"]');
    await filter.setValue('balcony');
    await browser.waitUntil(async () => (await receiverRows()).length === 1, { timeout: 10_000, timeoutMsg: 'the filter did not narrow the receivers' });
    assert.deepEqual(await receiverRows(), [{ name: 'Receiver 2', folder: 'Balcony' }]);
    await filter.setValue('');
  });
});
