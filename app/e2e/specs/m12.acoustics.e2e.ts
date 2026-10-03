// M12 P2's spec, the Acoustics tab (docs/investigations/2026-10-03-m12/PLAN.md, gate (a), (b),
// (e), (f)). Each id with its control:
//   m12-a  the box run (box_run.simpa, SPPS): every number the Acoustics tab shows, under every
//          band, receiver and DIN group it offers, equals `simpa results <run> --json` at the
//          precision shown (lib/acoustics.ts `numberMismatch`), every string its JSON string;
//          no digit is shown anywhere else on the tab; every value carries its range and status
//          or its refusal; STI carries "noise range not computed"; the words are MQ2's; and the
//          word "validated" is nowhere on the page. Every cell's visible row and column heads
//          (and its card's Band or Receiver selection), read from the rendered DOM, name the
//          receiver, parameter and band of the JSON paths inside it (the assay's MED finding:
//          a value right by its path but under the wrong label passed before); every mark is
//          one of the known marks, word for word. Control: at least one value per receiver, a
//          planted wrong digit is caught by the same comparison, and two cells' paths swapped
//          are caught by the label check
//   m12-b  0 elements for a parameter whose status in beds/summary.json is not PASS, under every
//          selection, and its name nowhere in the tab's or the Results panel's text; the run's
//          report carries the file's statuses; every PASS parameter has elements and cells,
//          each with its range and status or its refusal, T30 drawn, EDT with row 37's two
//          marks. With all eleven PASS the hiding half is empty here: m12.bedplant's
//          m12-b-plant proves it on a FAIL planted through core ($SIMPA_BED_DEMOTE)
//   m12-e  the DIN 18041 A3 target shown for the 180 m3 box reads 0.55 s, from the report's
//          own path. Control: A1 reads its own value, not 0.55
//   m12-f  a variant (the rear wall in a 0.6 curtain) run beside the baseline: switching the
//          variant switch shows the newest run of that variant, and the RT series drawn and the
//          RT numbers shown are that run's JSON, 0 mismatches, both ways. Control: the two
//          runs' series differ, so the check can tell them apart
//
// Its files, under <M11_WORK>\m12 (C:): a copy of the box and its runs. The repository is only
// read (beds/summary.json, the fixture). The tab is driven and read by lib/acousticsTab.ts, shared
// with m12.bedplant.e2e.ts (P4).
import { strict as assert } from 'node:assert';
import path from 'node:path';
import { cellLabelMismatch, MARKS, MQ2_WORDING, notPassed, numberMismatch, PARAM_LABELS, seriesMismatches, strayDigits, stringMismatch } from '../lib/acoustics.ts';
import {
  assertPassRendered,
  bedSweep,
  cliReport,
  everySelection,
  ownBox,
  PANEL,
  pick,
  type Row,
  runSpps,
  scan,
  type ScanCell,
  showRun,
  summary,
  type View,
} from '../lib/acousticsTab.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { env } from '../lib/types.ts';

const WORK = () => path.join(env('M11_WORK'), 'm12');

describe('M12 P2: the Acoustics tab', () => {
  let box = '';
  let baseRun: Row | undefined;
  let baseJson: Record<string, unknown> = {};

  before(async () => {
    await waitForHooks(['idle', 'openProject', 'edit', 'setStep', 'dockTab', 'runStart', 'runState', 'runsRows', 'selectRun', 'setSolver', 'acousticsView']);
    box = ownBox(WORK());
    await m10.openProject(box);
    baseRun = await runSpps();
    baseJson = cliReport(box, baseRun.run);
    await showRun(baseRun.run);
  });

  it('m12-a: every number on the Acoustics tab equals simpa results --json at the precision shown', async () => {
    assert.ok(baseRun);
    const mismatches: string[] = [];
    let compared = 0;
    let strings = 0;
    let labelled = 0;
    let swapCells: ScanCell[] = [];
    const cellsSeen = new Set<string>();
    await everySelection(async (what) => {
      const s = await scan();
      assert.equal(s.state, 'ready', what);
      assert.equal(s.run, baseRun?.run, what);
      for (const n of s.nums) {
        const m = numberMismatch(n, baseJson);
        if (m) mismatches.push(`${what}: ${m}`);
        compared++;
      }
      for (const t of s.strs) {
        const m = stringMismatch(t, baseJson);
        if (m) mismatches.push(`${what}: ${m}`);
        strings++;
      }
      const stray = strayDigits(s.stray);
      assert.equal(stray, null, `${what}: a digit outside the marked numbers: ${stray}`);
      for (const l of s.labels) {
        if (l.kind === 'wording') assert.equal(l.text, MQ2_WORDING, what);
        else if (l.kind === 'param') assert.equal(l.text, PARAM_LABELS[l.param ?? ''], `${what}: label of ${l.param}`);
        else if (l.kind === 'standard') assert.equal(l.text, 'DIN 18041', what);
        else if (l.kind === 'mark') assert.ok(MARKS.includes(l.text), `${what}: a mark not known word for word: ${JSON.stringify(l.text)}`);
        else assert.equal(l.kind, 'group', `${what}: a label of kind ${l.kind}`);
      }
      // The controls' options hold digits too: each is the JSON's. A band option is its index into
      // bands_hz, named by that band (kHz from 1000 Hz); a receiver option its index, named by
      // its label; a DIN option the group the report carries.
      const bandsHz = baseJson.bands_hz as number[];
      const bandOpts = s.options.filter((x) => x.control === 'band');
      assert.deepEqual(bandOpts.map((o) => o.value), [...bandsHz.map((_, i) => String(i)), 'sum'], `${what}: band options`);
      for (const o of bandOpts.filter((x) => x.value !== 'sum')) {
        const hz = bandsHz[Number(o.value)];
        assert.equal(o.text, hz >= 1000 ? `${hz / 1000} kHz` : `${hz} Hz`, `${what}: band option text`);
      }
      const labels = (baseJson.spps as { point_receivers: { label: string }[] }).point_receivers.map((r) => r.label);
      assert.deepEqual(s.options.filter((x) => x.control === 'receiver').map((o) => [o.value, o.text]), labels.map((l, i) => [String(i), l]), `${what}: receiver options`);
      const groups = (baseJson.room as { din18041: { group: string }[] }).din18041.map((d) => d.group);
      assert.deepEqual(s.options.filter((x) => x.control === 'din-group').map((o) => [o.value, o.text]), groups.map((g) => [g, g]), `${what}: DIN options`);
      // Every cell is where its labels say: the row and column heads and the card's selection, as
      // rendered, name the receiver, parameter and band of every JSON path inside it.
      for (const c of s.cells) {
        const m = cellLabelMismatch(c.labelled, baseJson);
        if (m) mismatches.push(`${what}: ${m}`);
        labelled++;
      }
      if (swapCells.length === 0) {
        const a = s.cells.find((c) => c.labelled.table === 'receivers-table' && c.status !== 'refused');
        const b = a && s.cells.find((c) => c.labelled.table === 'receivers-table' && c.status !== 'refused' && c.param !== a.param && c.receiver !== a.receiver);
        if (a && b) swapCells = [a, b];
      }
      // Every value carries its range and status; every refusal its code (row 37 (3), MQ3).
      for (const c of s.cells) {
        cellsSeen.add(`${c.receiver}|${c.param}`);
        if (c.status === 'refused') {
          assert.ok(c.refusal && /^[a-z_]+$/.test(c.refusal), `${what}: ${c.param} at ${c.receiver} refused without its code`);
        } else if (c.param === 'sti') {
          assert.equal(c.status, 'value', `${what}: STI`);
          assert.equal(c.note, 'noise range not computed', `${what}: STI carries MQ3's note`);
        } else {
          assert.ok(['ok', 'wide'].includes(c.status), `${what}: ${c.param} status ${c.status}`);
          assert.ok(c.nums.some((p) => p.endsWith('.lo')) && c.nums.some((p) => p.endsWith('.hi')), `${what}: ${c.param} at ${c.receiver} has no range`);
        }
      }
    });
    // Control: the comparison catches a digit changed by one.
    const first = (await scan()).nums.find((n) => n.digits !== '0');
    assert.ok(first, 'a number with decimals is shown');
    const last = first.text.slice(-1);
    const planted = { ...first, text: first.text.slice(0, -1) + (last === '9' ? '8' : String(Number(last) + 1)) };
    assert.notEqual(numberMismatch(planted, baseJson), null, 'a planted wrong digit is caught');
    // Control: two cells of the page with their paths swapped (another receiver and parameter)
    // are each caught by the label check, so it can tell a mislabelled cell.
    assert.equal(swapCells.length, 2, 'two cells of different receivers and parameters were seen');
    const [ca, cb] = swapCells;
    assert.equal(cellLabelMismatch(ca.labelled, baseJson), null, `control: ${ca.param} at ${ca.receiver} as rendered`);
    const swapA = cellLabelMismatch({ ...ca.labelled, paths: cb.labelled.paths }, baseJson);
    const swapB = cellLabelMismatch({ ...cb.labelled, paths: ca.labelled.paths }, baseJson);
    assert.notEqual(swapA, null, `control: ${ca.param} at ${ca.receiver} with ${cb.param} at ${cb.receiver}'s paths is caught`);
    assert.notEqual(swapB, null, 'control: the other half of the swap is caught');
    // The word "validated" nowhere on the page, text or tooltip.
    const page = await browser.execute(() => [document.body.innerText, ...[...document.querySelectorAll('[title]')].map((e) => e.getAttribute('title') ?? '')].join('\n'));
    assert.ok(!/validated/i.test(page), `"validated" is on screen: ${page.match(/.{0,40}validated.{0,40}/i)?.[0]}`);
    const receivers = ((baseJson.spps as { point_receivers: { label: string }[] }).point_receivers ?? []).map((r) => r.label);
    for (const r of receivers) assert.ok([...cellsSeen].some((k) => k.startsWith(`${r}|`)), `receiver ${r} has no cell`);
    // A picture of the tab beside the receipt (C:, the gate's work folder).
    await browser.saveScreenshot(path.join(WORK(), 'm12-a-acoustics.png'));
    console.log(
      `m12-a receipt: run ${baseRun.run}; ${compared} numbers and ${strings} strings compared over every selection; ${labelled} cells' visible labels held to their paths; ${mismatches.length} mismatches; ${cellsSeen.size} receiver-parameter cells; swap control: ${swapA}`,
    );
    assert.deepEqual(mismatches, []);
    assert.ok(compared > 100, `only ${compared} numbers compared`);
    assert.ok(labelled > 100, `only ${labelled} cells' labels checked`);
  });

  it('m12-b: no element for a parameter not PASS in beds/summary.json; every PASS parameter rendered with its range and status', async () => {
    const s0 = summary();
    const hidden = notPassed(s0);
    // The report the app read carries the file's statuses (core reads them, never the UI).
    const bed = (baseJson.bed as { parameters: Record<string, { status: string }> }).parameters;
    for (const [n, p] of Object.entries(s0.parameters)) assert.equal(bed[n]?.status, p.status, `report.bed ${n}`);
    const sweep = await bedSweep(hidden, s0);
    const passed = Object.keys(s0.parameters).filter((n) => !hidden.includes(n));
    // Every PASS parameter is rendered, each cell with its range and status or its refusal; EDT
    // with row 37's two marks. With every parameter PASS (decision 46), `hidden` is empty here,
    // and the hiding itself is proved by m12.bedplant's m12-b-plant (a FAIL planted through core).
    const lines = assertPassRendered(passed, sweep);
    // T30 (PASS by decision 46) as any other reverberation time: drawn against the DIN band too.
    if (passed.includes('t30_s')) assert.ok(sweep.drawn.has('t30_s'), `T30 is PASS and not drawn (drawn ${JSON.stringify([...sweep.drawn])})`);
    console.log(
      `m12-b receipt: not PASS ${JSON.stringify(hidden)}; PASS ${JSON.stringify(passed)}; with elements ${JSON.stringify([...sweep.shown].sort())}; drawn ${JSON.stringify([...sweep.drawn].sort())}; marks ${JSON.stringify(sweep.marks)}; ${lines.join('; ')}`,
    );
  });

  it('m12-e: the DIN 18041 A3 target for the 180 m3 box reads 0.55 s', async () => {
    await pick('din-group', 'A3');
    const read = () =>
      browser.execute((sel: string) => {
        const t = document.querySelector(`${sel} [data-part="din-target"]`);
        const n = t?.querySelector('[data-num][data-json$=".target_s.value"]');
        const v = t?.querySelector('[data-num][data-json="room.volume_m3"]');
        const g = t?.querySelector('[data-str][data-json$=".group"]');
        return { target: n?.textContent ?? null, path: n?.getAttribute('data-json') ?? null, volume: v?.textContent ?? null, group: g?.textContent ?? null, text: (t as HTMLElement | null)?.innerText ?? null };
      }, PANEL);
    const a3 = await read();
    console.log(`m12-e receipt: ${JSON.stringify(a3)}`);
    assert.equal(a3.target, '0.55');
    assert.equal(a3.group, 'A3');
    assert.equal(a3.volume, '180');
    assert.equal(a3.path, 'room.din18041.2.target_s.value');
    assert.equal(((baseJson.room as { din18041: { target_s: { value: number } }[] }).din18041[2].target_s.value).toFixed(2), '0.55');
    // Control: another group reads its own target.
    await pick('din-group', 'A1');
    const a1 = await read();
    assert.equal(a1.group, 'A1');
    assert.notEqual(a1.target, '0.55');
    assert.equal(a1.target, (baseJson.room as { din18041: { target_s: { value: number } }[] }).din18041[0].target_s.value.toFixed(2));
    await pick('din-group', 'A3');
  });

  it("m12-f: the variant switch replaces the RT series with the other run's JSON values", async () => {
    assert.ok(baseRun);
    // A curtain (0.6) on the rear wall, as a variant: the curtain is unused in the baseline.
    const curtain = '00000000-0000-0000-0000-000000000204';
    const rear = '00000000-0000-0000-0000-000000000105';
    const variant = '00000000-0000-0000-0000-00000000f00f';
    const p = JSON.parse(await m10.projectJson()) as { bands: { frequencies_hz: number[] } };
    const ops = p.bands.frequencies_hz.map((_, band) => ({ op: 'set_material_band', material: curtain, quantity: 'absorption', band, value: 0.6 }));
    const out = await m10.edit({
      op: 'batch',
      ops: [...ops, { op: 'add_variant', index: 0, variant: { id: variant, name: 'Curtain', overrides: [{ group: rear, material: curtain }] } }, { op: 'set_active_variant', variant }],
    });
    assert.ok(out.applied, `the variant edit was refused: ${JSON.stringify(out.refusals)}`);
    const varRun = await runSpps();
    const varJson = cliReport(box, varRun.run);
    const tab = (id: string) => clickVariant(id);

    const check = async (run: Row, json: Record<string, unknown>, label: string): Promise<{ mismatches: string[]; values: string }> => {
      await browser.waitUntil(async () => (await hook<View | null>('acousticsView'))?.run === run.run && (await hook<View>('acousticsView')).state === 'ready', {
        timeout: 60_000,
        timeoutMsg: `${label}: the tab did not switch to ${run.run}`,
      });
      const v = await hook<View>('acousticsView');
      const mismatches: string[] = [];
      assert.ok(v.series.length > 0, `${label}: no RT series is drawn`);
      for (const s of v.series) mismatches.push(...seriesMismatches(s, json, v.receiver, s.param).map((m) => `${label}: ${m}`));
      const s = await scan();
      for (const n of s.nums) {
        const m = numberMismatch(n, json);
        if (m) mismatches.push(`${label}: ${m}`);
      }
      return { mismatches, values: JSON.stringify(v.series.map((x) => x.values)) };
    };

    await tab('baseline');
    const a = await check(baseRun, baseJson, 'baseline');
    await tab(variant);
    const b = await check(varRun, varJson, 'Curtain');
    await tab('baseline');
    const a2 = await check(baseRun, baseJson, 'baseline again');
    console.log(`m12-f receipt: baseline ${baseRun.run} ${a.values}; Curtain ${varRun.run} ${b.values}; mismatches ${a.mismatches.length + b.mismatches.length + a2.mismatches.length}`);
    assert.notEqual(a.values, b.values, 'control: the two runs draw the same series, so the switch is not tested');
    assert.deepEqual([...a.mismatches, ...b.mismatches, ...a2.mismatches], []);
  });
});

/** Clicks a variant tab in the step bar, as a user does. */
async function clickVariant(id: string): Promise<void> {
  const t = await $(`[data-part="variants"] [data-variant="${id}"]`);
  await t.waitForExist({ timeout: 10_000 });
  await t.click();
  await browser.waitUntil(async () => (await t.getAttribute('aria-selected')) === 'true', { timeout: 10_000 });
  await m10.idle();
}
