// M12 gate (b)'s plant (docs/investigations/2026-10-03-m12/BUILD-P4.md). Gate (b) says a
// parameter whose bed status is not PASS has 0 DOM elements. With every parameter PASS in
// beds/summary.json (decision 46), m12.acoustics' m12-b has nothing to hide, so it cannot show
// the hiding works. This spec runs the app with a FAIL planted where the app reads the bed, in
// core, at runtime: m11.conf.ts sets $SIMPA_BED_DEMOTE to tests/fixtures/beds/summary-c80-fail.json
// for this spec's session only, and core (results::bed) demotes C80 from it. The UI holds no list:
// if C80 disappears, it disappeared because the report core built says FAIL.
//   m12-b-plant  under every selection, C80 has 0 `[data-param]` elements on the page, its name
//                is in neither the tab's nor the Results panel's text, and it is not drawn;
//                `simpa results --json` (the same environment) carries C80 FAIL with the plant's
//                reason, and every other parameter PASS. Control: every other parameter is
//                rendered with its range and status (or its refusal), and C80 is PASS in the
//                committed summary, so only the plant hides it
//
// Its files, under <M11_WORK>\m12-bedplant (C:): a copy of the box and its run.
import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { notPassed, PARAM_LABELS } from '../lib/acoustics.ts';
import { assertPassRendered, type BedSummary, bedSweep, cliReport, ownBox, repo, runSpps, showRun, summary } from '../lib/acousticsTab.ts';
import { m10, waitForHooks } from '../lib/hooks.ts';
import { env } from '../lib/types.ts';

const WORK = () => path.join(env('M11_WORK'), 'm12-bedplant');
const PLANT = 'tests/fixtures/beds/summary-c80-fail.json';

describe('M12 gate (b): a FAIL planted through core has no element', () => {
  let box = '';
  let json: Record<string, unknown> = {};

  before(async () => {
    // The session was started with the plant (m11.conf.ts), and this process carries it to the CLI.
    assert.equal(path.resolve(env('SIMPA_BED_DEMOTE')), path.resolve(repo(PLANT)), '$SIMPA_BED_DEMOTE is the plant');
    await waitForHooks(['idle', 'openProject', 'setStep', 'dockTab', 'runStart', 'runState', 'runsRows', 'setSolver', 'acousticsView']);
    box = ownBox(WORK());
    await m10.openProject(box);
    const run = await runSpps();
    json = cliReport(box, run.run);
    await showRun(run.run);
  });

  it('m12-b-plant: C80 planted FAIL through core has no element; every other parameter rendered', async () => {
    const plant = JSON.parse(readFileSync(repo(PLANT), 'utf8')) as BedSummary;
    const hidden = notPassed(plant);
    assert.deepEqual(hidden, ['c80_db'], 'the plant fails C80 only');
    // Control: C80 passed in the committed summary, so nothing but the plant can hide it.
    const real = summary();
    assert.equal(real.parameters.c80_db.status, 'PASS', 'C80 is PASS in beds/summary.json');
    // The page first: C80 has no element anywhere, under every selection; the others are rendered.
    // The tab's own parameters (PARAM_LABELS); the M12c entries are shown elsewhere.
    const passed = Object.keys(real.parameters).filter((n) => n !== 'c80_db' && n in PARAM_LABELS);
    const sweep = await bedSweep(hidden, plant);
    const lines = assertPassRendered(passed, sweep);
    // The report core built in this environment: C80 FAIL with the plant's reason, the rest as
    // the committed summary says (all PASS).
    const bed = (json.bed as BedSummary).parameters;
    assert.equal(bed.c80_db.status, 'FAIL', 'report.bed c80_db');
    assert.ok((bed.c80_db.reasons ?? []).some((r) => r.includes('SIMPA_BED_DEMOTE')), `C80's reasons name the plant: ${JSON.stringify(bed.c80_db.reasons)}`);
    for (const n of passed) assert.equal(bed[n]?.status, real.parameters[n].status, `report.bed ${n}`);
    assert.equal(json.validated_by_bed, false, 'validated_by_bed with a parameter FAIL');

    console.log(
      `m12-b-plant receipt: plant ${PLANT}; hidden ${JSON.stringify(hidden)}; report.bed c80_db ${bed.c80_db.status} (${JSON.stringify(bed.c80_db.reasons)}); with elements ${JSON.stringify([...sweep.shown].sort())}; ${lines.join('; ')}`,
    );
    assert.ok(!sweep.shown.has('c80_db'), 'C80 has elements');
  });
});
