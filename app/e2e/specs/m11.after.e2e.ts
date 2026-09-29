// After gate (d) (docs/investigations/2026-09-29-m11/PLAN.md 4.1, m11-d-after, not in the gate
// text): a fresh session lists the killed run honestly. The kill left a run folder with no
// run.json: its row reads Interrupted, with the reason "no run.json", never dropped and never
// guessed. The run the close cancelled reads Cancelled (the control). The run names come from
// the close and kill specs, through the gate's work folder.
import { strict as assert } from 'node:assert';
import { m10, waitForHooks } from '../lib/hooks.ts';
import { project, recordedRun, rowStatus, runRow, showTab, shown } from '../lib/m11.ts';

const LONG = () => project('long', 'box_long.simpa');

describe('M11 after', () => {
  before(async () => {
    await browser.setTimeout({ script: 120_000 });
    await waitForHooks(['idle', 'openProject', 'runsRows']);
  });

  it('m11-d-after: the killed run lists as Interrupted, with its reason, and the closed one as Cancelled', async () => {
    const killed = recordedRun('kill');
    const closed = recordedRun('close');
    await m10.openProject(LONG());
    await showTab('runs');

    const k = await rowStatus(killed);
    const kText = shown(await (await runRow(killed)).getText());
    console.log(`m11-d-after receipt: killed run ${killed}: data-status ${k.attr}, status '${k.text}', row '${kText}'`);
    assert.equal(k.attr, 'INTERRUPTED');
    assert.equal(k.text, 'Interrupted');
    assert.ok(kText.includes('no run.json'), `the Interrupted row does not give its reason 'no run.json': '${kText}'`);

    // Control: the run the close cancelled, which did write its run.json.
    const c = await rowStatus(closed);
    console.log(`m11-d-after receipt: closed run ${closed}: data-status ${c.attr}, status '${c.text}'`);
    assert.equal(c.attr, 'CANCELLED');
    assert.equal(c.text, 'Cancelled');
  });
});
