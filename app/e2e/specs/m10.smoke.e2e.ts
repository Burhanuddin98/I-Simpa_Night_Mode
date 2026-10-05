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
    const landing = await browser.execute(() => ({
      examples: [...document.querySelectorAll('[data-part="landing"] [data-example]')].map((e) => e.getAttribute('data-example')),
      actions: [...document.querySelectorAll('[data-part="landing"] [data-landing-action]')].map((e) => e.getAttribute('data-landing-action')),
    }));
    assert.deepEqual(landing, { examples: ['elmia', 'bras-cr2', 'bras-cr4'], actions: ['new-project', 'open'] });
    const title = await browser.getTitle();
    assert.equal(title, 'I-Simpa Night Mode');
  });
});
