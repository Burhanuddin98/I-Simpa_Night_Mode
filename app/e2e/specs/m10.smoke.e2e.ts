// The harness smoke test (foundation): the release app launches under tauri-driver with --e2e,
// the step bar reads back from the DOM, and the test hooks are installed.
import { strict as assert } from 'node:assert';
import { stepNames } from '../lib/dom.ts';
import { hook, waitForHooks } from '../lib/hooks.ts';

const FIVE_STEPS = ['Geometry', 'Materials', 'Sources & receivers', 'Simulate', 'Results'];

describe('M10 harness smoke', () => {
  it('m10-smoke: the built app launches and shows the five steps', async () => {
    await browser.waitUntil(async () => (await stepNames()).length === 5, {
      timeout: 60_000,
      timeoutMsg: 'the step bar did not show five steps within 60 s',
    });
    assert.deepEqual(await stepNames(), FIVE_STEPS);
    await waitForHooks(['idle', 'openProject', 'importModel', 'edit', 'undo', 'projectJson']);
    assert.equal(await hook('idle'), true);
    // No project yet: Run is disabled and names no blocker but the missing project.
    const run = await $('[data-part="run"]');
    assert.equal(await run.isEnabled(), false, 'Run is disabled');
    // No project yet: the landing page stands in for the empty window, with the shipped examples
    // (src-tauri/src/examples.rs) and New project and Open… (Burhan, 2026-10-06 01:24).
    // Re-pinned 2026-10-07 (C4): six examples, in examples.rs's order. 218fccb added BRAS CR1 and
    // CR3 and b01e5dc the industrial hall (upstream tutorial 3), both on 10-06, on purpose; the pin
    // still holds the whole list and its order, not a subset.
    // Re-pinned 2026-10-09 (parity G11): New box room… joins between New project and Open…, on purpose
    // (upstream's New scene, a box room of a given width, length and height).
    const landing = await browser.execute(() => ({
      examples: [...document.querySelectorAll('[data-part="landing"] [data-example]')].map((e) => e.getAttribute('data-example')),
      actions: [...document.querySelectorAll('[data-part="landing"] [data-landing-action]')].map((e) => e.getAttribute('data-landing-action')),
    }));
    assert.deepEqual(landing, { examples: ['elmia', 'industrial', 'bras-cr1', 'bras-cr2', 'bras-cr3', 'bras-cr4'], actions: ['new-project', 'new-box-room', 'open'] });
    const title = await browser.getTitle();
    assert.equal(title, 'I-Simpa Night Mode');
  });
});
