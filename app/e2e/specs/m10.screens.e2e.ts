// Screenshots of the three M10 steps for the investigation folder. Not a gate id: nothing is
// asserted beyond the page being in the state the picture claims. Run by
// `tools/gates/m10.ps1 -Only e2e -Spec screens [-ScreensDir <dir>]`, which passes M10_SCREENS.
//   1-geometry-hall.png            the corrected hall imported (m, z): the check passes
//   2-geometry-raw-hall.png        upstream's raw elmia.ply: refused, its faces highlighted
//   3-materials.png                the teaching room, Rear wall selected
//   4-sources.png                  the teaching room, S1 selected
//   5-sources-refused.png          R1 moved to x = 20: RECEIVER_OUTSIDE inline, project unchanged
import { strict as assert } from 'node:assert';
import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { clickSelector } from '../lib/dom.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { env } from '../lib/types.ts';

const repo = (rel: string) => path.join(env('M10_REPO'), rel);
const TEACHING_ROOM = () => repo('tests/fixtures/ui/teaching_room.simpa');
const CORRECTED_HALL = () => repo('testdata/elmia_corrected.ply');
const CTRL = '';
const ENTER = '';
const BACKSPACE = '';

interface Named {
  id: string;
  name: string;
}
const project = async (): Promise<{ surface_groups: Named[]; sources: Named[]; point_receivers: Named[] }> =>
  JSON.parse(await m10.projectJson());

/** Settles the app and the next two frames, moves the pointer off the view, then saves. */
async function shoot(name: string): Promise<void> {
  const dir = env('M10_SCREENS');
  mkdirSync(dir, { recursive: true });
  await browser
    .action('pointer', { parameters: { pointerType: 'mouse' } })
    .move({ x: 2, y: 2, origin: 'viewport', duration: 0 })
    .perform();
  await m10.idle();
  await browser.execute(() => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r()))));
  const file = path.join(dir, name);
  await browser.saveScreenshot(file);
  console.log(`screenshot ${file}`);
}

describe('M10 screens', () => {
  before(async () => {
    await waitForHooks(['idle', 'openProject', 'importModel', 'setStep', 'projectJson', 'frame', 'highlightedFaceCount']);
  });

  it('screens: Geometry with the corrected hall, and with the raw hall refused', async () => {
    await m10.importModel(CORRECTED_HALL(), 'm', 'z');
    await m10.setStep('geometry');
    await clickSelector('[data-dock-tab="console"]');
    await hook('frame');
    assert.equal(await $('[data-part="check-verdict"]').getText(), 'Passed');
    await shoot('1-geometry-hall.png');

    await m10.importModel(env('M10_ELMIA_RAW'), 'm', 'z');
    await hook('frame');
    assert.ok((await hook<number>('highlightedFaceCount')) > 0);
    await shoot('2-geometry-raw-hall.png');
  });

  it('screens: Materials, the teaching room with Rear wall selected', async () => {
    await m10.openProject(TEACHING_ROOM());
    const rear = (await project()).surface_groups.find((g) => g.name === 'Rear wall');
    assert.ok(rear);
    await clickSelector(`.scene [data-entity="surface_group:${rear.id}"]`);
    await m10.idle();
    assert.equal(await browser.execute(() => document.querySelector('[data-props-step]')?.getAttribute('data-props-step')), 'materials');
    await clickSelector('[data-grid-cell="3:0"]');
    await shoot('3-materials.png');
  });

  it('screens: Sources & receivers, S1 selected, then a refused placement', async () => {
    await m10.openProject(TEACHING_ROOM());
    const p = await project();
    await clickSelector(`.scene [data-entity="source:${p.sources[0].id}"]`);
    await $('[data-field="position.x"]').waitForDisplayed({ timeout: 30_000 });
    await shoot('4-sources.png');

    const r1 = p.point_receivers.find((r) => r.name === 'R1');
    assert.ok(r1);
    await clickSelector(`.scene [data-entity="point_receiver:${r1.id}"]`);
    const x = await $('[data-field="position.x"]');
    await x.waitForDisplayed({ timeout: 30_000 });
    await x.click();
    await browser.keys([CTRL, 'a']);
    await browser.keys(BACKSPACE);
    for (const ch of '20') await browser.keys(ch);
    await browser.keys(ENTER);
    await m10.idle();
    await $('[data-issue-code="RECEIVER_OUTSIDE"]').waitForDisplayed({ timeout: 30_000 });
    await shoot('5-sources-refused.png');
  });
});
