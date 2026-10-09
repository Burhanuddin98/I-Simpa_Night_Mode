// The materials package's gate id and extra checks (PLAN.md 3 and 6.2).
//   m10-c  teaching room, Materials step, focus [data-grid-cell="0:0"], a synthetic paste of
//          tests/fixtures/ui/materials_6x6.tsv (bytes as read by Node), saveAs: the saved bytes
//          equal tests/fixtures/ui/materials_6x6.expected.json; the committed start file differs
//          from it (asserted first).
// The extra checks (not gate ids): the copy round trip is exact; row fill; 1.5 on a used material
// refused as MATERIAL_VALUE_OUT_OF_RANGE and '0,5' as NOT_A_NUMBER; tutorial1_box's third-octave
// headers in ascending order; a header-row paste maps by frequency.
//
// A synthetic ClipboardEvent exercises the grid's own copy and paste handlers, not the OS
// clipboard: one manual paste from Excel at hand-over covers that (PLAN.md 9).
import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { clickSelector } from '../lib/dom.ts';
import { compareFiles } from '../lib/files.ts';
import { hook, m10, openMaterialsTable, waitForHooks } from '../lib/hooks.ts';
import { env } from '../lib/types.ts';

const repo = (rel: string) => path.join(env('M10_REPO'), rel);
const TEACHING_ROOM = () => repo('tests/fixtures/ui/teaching_room.simpa');
const BOX = () => repo('tests/fixtures/rooms/tutorial1_box.simpa');
const TSV = () => repo('tests/fixtures/ui/materials_6x6.tsv');
const EXPECTED = () => repo('tests/fixtures/ui/materials_6x6.expected.json');

// WebDriver key codes (the specs import nothing from wdio).
const ENTER = '\uE007';
const SHIFT = '\uE008';
const CTRL = '\uE009';
const END = '\uE010';

const THIRDS = [
  '50', '63', '80', '100', '125', '160', '200', '250', '315', '400', '500', '630', '800',
  '1k', '1.25k', '1.6k', '2k', '2.5k', '3.15k', '4k', '5k', '6.3k', '8k', '10k', '12.5k', '16k', '20k',
];

interface SavedMaterial {
  id: string;
  name: string;
  absorption: number[];
  scattering: number[];
}

async function project(): Promise<{ materials: SavedMaterial[] }> {
  return JSON.parse(await m10.projectJson());
}

/** Opens `file`, shows the Materials step and opens the library's table (its own window since the
 * 2026-10-09 GUI audit), with the grid mounted. */
async function onMaterials(file: string): Promise<void> {
  await m10.openProject(file);
  await m10.setStep('materials');
  await openMaterialsTable();
}

/** The fixture's text, byte for byte as committed (CRLF kept). */
function fixtureTsv(): string {
  const bytes = readFileSync(TSV());
  const text = bytes.toString('utf8');
  assert.ok(Buffer.from(text, 'utf8').equals(bytes), 'the TSV reads back to its own bytes');
  assert.ok(text.includes('\r\n'), 'the TSV keeps its CRLF endings');
  return text;
}

/**
 * Dispatches a synthetic clipboard event at the focused element, which must be in the grid (as
 * the real Ctrl+C or Ctrl+V would target it), waits for the app to settle, and returns the
 * event's text/plain data afterwards (what a copy wrote).
 */
async function clipboard(type: 'copy' | 'paste', text = ''): Promise<string> {
  const out = await browser.execute(
    (t: string, txt: string) => {
      const grid = document.querySelector('[data-materials-grid]');
      const target = document.activeElement;
      if (!grid || !target || !grid.contains(target)) {
        return { error: `the focus is on ${target?.tagName ?? 'nothing'}, not in the materials grid` };
      }
      const dt = new DataTransfer();
      if (t === 'paste') dt.setData('text/plain', txt);
      const ev = new ClipboardEvent(t, { clipboardData: dt, bubbles: true, cancelable: true });
      if (ev.clipboardData === null) Object.defineProperty(ev, 'clipboardData', { value: dt });
      target.dispatchEvent(ev);
      return { data: dt.getData('text/plain'), handled: ev.defaultPrevented };
    },
    type,
    text,
  );
  if ('error' in out) throw new Error(out.error);
  assert.ok(out.handled, `the grid did not take the ${type} event`);
  await m10.idle();
  return out.data;
}

/** The inline messages of the Materials step and of its table's window, as `{code, text}`. */
async function issueLines(): Promise<{ code: string; text: string }[]> {
  return browser.execute(() =>
    [...document.querySelectorAll('[data-part="materials"] [data-issue-code], [data-materials-sheet] [data-issue-code]')].map((e) => ({
      code: e.getAttribute('data-issue-code') ?? '',
      text: e.textContent ?? '',
    })),
  );
}

async function cellIssue(cell: string): Promise<string | null> {
  return (await $(`[data-grid-cell="${cell}"]`)).getAttribute('data-cell-issue');
}

/** Focuses a cell, opens its editor with Enter, types `text` over the selected value, Enter. */
async function typeInto(cell: string, text: string): Promise<void> {
  await clickSelector(`[data-grid-cell="${cell}"]`);
  await browser.keys(ENTER);
  const editor = await $('[data-grid-editor]');
  await editor.waitForExist({ timeout: 10_000 });
  await browser.keys(text);
  assert.equal(await editor.getValue(), text, 'the editor holds what was typed');
  await browser.keys(ENTER);
  await m10.idle();
}

const same = (a: readonly number[], b: readonly number[]) => a.length === b.length && a.every((v, i) => Object.is(v, b[i]));

describe('M10 materials', () => {
  before(async () => {
    await waitForHooks(['idle', 'openProject', 'saveAs', 'setStep', 'projectJson', 'undoDepth']);
  });

  it('m10-c: the pasted 6x6 block saves byte-identical to materials_6x6.expected.json', async () => {
    // Control first: the committed start file differs from the expected one, so the byte
    // comparison below can fail.
    assert.notEqual(compareFiles(TEACHING_ROOM(), EXPECTED()), null, 'the start file must differ from the expected file');
    const tsv = fixtureTsv();
    await onMaterials(TEACHING_ROOM());
    const depth = await m10.undoDepth();
    await clickSelector('[data-grid-cell="0:0"]');
    await clipboard('paste', tsv);
    assert.equal(await m10.undoDepth(), depth + 1, 'one paste is one undo step');
    const fails = (await issueLines()).filter((l) => l.text.startsWith('FAIL'));
    assert.deepEqual(fails, [], 'the paste raised no FAIL line');
    const out = path.join(env('M10_WORK'), 'c.simpa');
    await m10.saveAs(out);
    const diff = compareFiles(out, EXPECTED());
    assert.equal(diff, null, diff ?? '');
  });

  it('materials: Ctrl+C copies the exact shortest round-trip values, and pasting them back is exact', async () => {
    await onMaterials(TEACHING_ROOM());
    await clickSelector('[data-grid-cell="0:0"]');
    await clipboard('paste', fixtureTsv());

    // Every band cell, copied.
    await clickSelector('[data-grid-cell="0:0"]');
    await browser.keys([CTRL, 'a']);
    const copied = await clipboard('copy');
    const p = await project();
    const want = p.materials.map((m) => `${m.absorption.map(String).join('\t')}\r\n`).join('');
    assert.equal(copied, want);
    assert.ok(copied.includes('0.39509836780726515'), 'the 17-digit cell is copied whole, not as its 0.40 display');
    const cells = copied.trimEnd().split('\r\n').map((line) => line.split('\t'));
    cells.forEach((line, r) => line.forEach((cell, b) => assert.ok(Object.is(Number(cell), p.materials[r].absorption[b]), `${r}:${b} ${cell}`)));
    assert.equal(await hook<string>('materialsCopy'), copied, 'the hook and the copy event agree');
    const shown = await browser.execute(() => document.querySelector('[data-grid-cell="3:5"]')?.textContent ?? null);
    assert.equal(shown, '≈0.40', 'the display marks its rounding');

    // The wood panel's row, copied and pasted onto the curtain's: the same doubles arrive.
    const depth = await m10.undoDepth();
    await clickSelector('[data-grid-cell="3:0"]');
    await browser.keys([SHIFT, END]);
    const row = await clipboard('copy');
    assert.equal(row, `${p.materials[3].absorption.map(String).join('\t')}\r\n`);
    await clickSelector('[data-grid-cell="4:0"]');
    await clipboard('paste', row);
    const q = await project();
    assert.ok(same(q.materials[4].absorption, p.materials[3].absorption), JSON.stringify(q.materials[4].absorption));
    assert.equal(await m10.undoDepth(), depth + 1);
  });

  it('materials: row fill copies the focused cell across its row as one undo step (Ctrl+R and the button)', async () => {
    await onMaterials(TEACHING_ROOM());
    await clickSelector('[data-grid-cell="0:0"]');
    await clipboard('paste', fixtureTsv());
    const depth = await m10.undoDepth();

    // Painted plaster at 1k is 0.03.
    await clickSelector('[data-grid-cell="2:3"]');
    await browser.keys([CTRL, 'r']);
    await m10.idle();
    let p = await project();
    assert.deepEqual(p.materials[2].absorption, [0.03, 0.03, 0.03, 0.03, 0.03, 0.03]);
    assert.equal(await m10.undoDepth(), depth + 1, 'the fill is one undo step (and Ctrl+R did not reload the page)');

    // Mineral-fibre tile at 250 is 0.70, filled with the button.
    await clickSelector('[data-grid-cell="1:1"]');
    await clickSelector('[data-action="fill-row"]');
    await m10.idle();
    p = await project();
    assert.deepEqual(p.materials[1].absorption, [0.7, 0.7, 0.7, 0.7, 0.7, 0.7]);
    assert.equal(await m10.undoDepth(), depth + 2);

    // One Ctrl+Z takes the whole fill back.
    await clickSelector('[data-grid-cell="0:0"]');
    await browser.keys([CTRL, 'z']);
    await m10.idle();
    p = await project();
    assert.deepEqual(p.materials[1].absorption, [0.5, 0.7, 0.6, 0.7, 0.7, 0.5]);
    assert.equal(await m10.undoDepth(), depth + 1);
  });

  it('materials: 1.5 on a used material is refused as MATERIAL_VALUE_OUT_OF_RANGE, and 0,5 as NOT_A_NUMBER, the project unchanged', async () => {
    await onMaterials(TEACHING_ROOM());
    const before = await m10.projectJson();
    const depth = await m10.undoDepth();

    // Painted plaster, used by three walls, at 250.
    await typeInto('2:1', '1.5');
    let lines = await issueLines();
    const range = lines.find((l) => l.code === 'MATERIAL_VALUE_OUT_OF_RANGE');
    assert.ok(range, JSON.stringify(lines));
    assert.ok(range.text.startsWith('Error: ') && range.text.trim().endsWith('MATERIAL_VALUE_OUT_OF_RANGE'), range.text);
    assert.equal(await cellIssue('2:1'), 'MATERIAL_VALUE_OUT_OF_RANGE', 'the cell is outlined');
    assert.equal(await m10.projectJson(), before);
    assert.equal(await m10.undoDepth(), depth);

    await typeInto('2:1', '0,5');
    lines = await issueLines();
    const nan = lines.find((l) => l.code === 'NOT_A_NUMBER');
    assert.ok(nan, JSON.stringify(lines));
    assert.ok(nan.text.startsWith('Error: ') && nan.text.trim().endsWith('NOT_A_NUMBER') && nan.text.includes("'0,5'"), nan.text);
    assert.ok(!lines.some((l) => l.code === 'MATERIAL_VALUE_OUT_OF_RANGE'), 'the messages are the latest attempt');
    assert.equal(await cellIssue('2:1'), 'NOT_A_NUMBER');
    assert.equal(await m10.projectJson(), before);
    assert.equal(await m10.undoDepth(), depth);

    // Control: 0.5 is accepted, one undo step, and the messages clear.
    await typeInto('2:1', '0.5');
    assert.equal(await m10.undoDepth(), depth + 1);
    assert.deepEqual(await issueLines(), []);
    assert.equal((await project()).materials[2].absorption[1], 0.5);
  });

  it("materials: tutorial1_box's third-octave band headers read 50 … 20k in ascending order", async () => {
    await onMaterials(BOX());
    const heads = await browser.execute(() =>
      [...document.querySelectorAll('[data-materials-grid] th[data-band-hz]')].map((th) => ({
        hz: Number(th.getAttribute('data-band-hz')),
        label: th.querySelector('[data-part="band-label"]')?.textContent ?? '',
      })),
    );
    assert.deepEqual(heads.map((h) => h.label), THIRDS);
    for (let i = 1; i < heads.length; i++) assert.ok(heads[i].hz > heads[i - 1].hz, `${heads[i - 1].label} before ${heads[i].label}`);
    // Twenty-seven bands scroll sideways, under the themed scrollbar.
    const scroll = await browser.execute(() => {
      const el = document.querySelector<HTMLElement>('[data-materials-grid] .mg-scroll');
      if (!el) return null;
      return { wider: el.scrollWidth > el.clientWidth, color: getComputedStyle(el).getPropertyValue('scrollbar-color') };
    });
    assert.ok(scroll && scroll.wider, 'the third-octave grid scrolls horizontally');
    assert.notEqual(scroll.color, 'auto', 'the scroll container is themed');
  });

  it('materials: a paste with a header row maps its columns by frequency', async () => {
    await onMaterials(TEACHING_ROOM());
    const depth = await m10.undoDepth();

    // Columns out of order and spelt three ways, pasted from the curtain's 500 cell.
    await clickSelector('[data-grid-cell="4:2"]');
    await clipboard('paste', '4 kHz\t125 Hz\t1k\r\n0.9\t0.05\t0.6\r\n');
    let p = await project();
    assert.deepEqual(p.materials[4].absorption, [0.05, 0.1, 0.1, 0.6, 0.1, 0.9]);
    assert.equal(await m10.undoDepth(), depth + 1);

    // A name column maps rows by name, wherever the focus is.
    await clickSelector('[data-grid-cell="0:0"]');
    await clipboard('paste', 'Material\t500 Hz\t2k\r\nGlass window\t0.2\t0.3\r\n');
    p = await project();
    assert.deepEqual(p.materials[5].absorption, [0.1, 0.1, 0.2, 0.1, 0.3, 0.1]);
    assert.deepEqual(p.materials[0].absorption, [0.1, 0.1, 0.1, 0.1, 0.1, 0.1]);
    assert.equal(await m10.undoDepth(), depth + 2);

    // Refused before any op, each with its code under the grid.
    const before = await m10.projectJson();
    for (const [text, code] of [
      ['3 kHz\t125 Hz\r\n0.1\t0.2\r\n', 'PASTE_HEADER'],
      ['0.1\t0.2\r\n0.3\r\n', 'PASTE_SHAPE'],
      ['0.1\t0,5\r\n', 'NOT_A_NUMBER'],
    ]) {
      await clickSelector('[data-grid-cell="0:0"]');
      await clipboard('paste', text);
      const lines = await issueLines();
      assert.ok(lines.some((l) => l.code === code && l.text.startsWith('Error: ') && l.text.trim().endsWith(code)), `${code}: ${JSON.stringify(lines)}`);
      assert.equal(await m10.projectJson(), before, `${code} changed the project`);
      assert.equal(await m10.undoDepth(), depth + 2);
    }
  });
});
