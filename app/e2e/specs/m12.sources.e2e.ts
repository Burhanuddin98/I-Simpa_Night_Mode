// Backlog 77 (parity R11): per-source results on the Acoustics tab. The Elmia hall
// (tests/fixtures/rooms/elmia_corrected.simpa, three sources, echogram per source on) run from the
// app. Each id with its control:
//   b77-picker   no receiver's sphere crosses a wall in the run; the tab offers a Source picker
//                with the report's three sources and the sources summed, and opens on the first. Control: with the sources summed picked, EDT and
//                C80 are refused several_sources at every receiver, as ISO 3382-1 has them
//   b77-numbers  under every source, band, receiver and DIN group, every number on the tab equals
//                `simpa results <run> --json` at the precision shown and every cell's labels name
//                its receiver, parameter, band and source (lib/acoustics.ts); each source shows EDT
//                and C80 with a range at every receiver. Control: the same cell reads a different
//                path under two sources, and a source's cell under another source's name is caught
//
// Its files, under <M11_WORK>\m12-sources (C:): a copy of the hall at 150,000 particles per source
// (the fixture's 1,000,000 take about 6 min a run; this checks what is shown per source, not the
// values' noise, B:\data\m12\b78-mesh\FINDINGS.md). The repository is only read.
import { strict as assert } from 'node:assert';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { cellLabelMismatch, numberMismatch, SOURCES_SUMMED, stringMismatch } from '../lib/acoustics.ts';
import { cliReport, everySelection, pick, repo, type Row, runSpps, scan, showRun, type View } from '../lib/acousticsTab.ts';
import { hook, m10, waitForHooks } from '../lib/hooks.ts';
import { env } from '../lib/types.ts';

const WORK = () => path.join(env('M11_WORK'), 'm12-sources');
const PARTICLES = 150_000;

type Json = Record<string, unknown>;
type PerSource = { source: string; bands: { parameters: Record<string, Json> }[] };
type Receiver = { label: string; per_source: PerSource[]; bands: { parameters: Record<string, Json> }[] };

/** The hall, copied with its particle count lowered and nothing else changed. */
function ownHall(dir: string): string {
  mkdirSync(dir, { recursive: true });
  const p = JSON.parse(readFileSync(repo('tests/fixtures/rooms/elmia_corrected.simpa'), 'utf8')) as { solvers: { spps: Json } };
  assert.equal(p.solvers.spps.echogram_per_source, true, 'the fixture writes an echogram per source');
  p.solvers.spps.particles_per_source = PARTICLES;
  const to = path.join(dir, 'elmia.simpa');
  writeFileSync(to, JSON.stringify(p, null, 2));
  return to;
}

describe('Backlog 77: per-source results on the Acoustics tab', () => {
  let hall = '';
  let run: Row | undefined;
  let json: Json = {};
  let names: string[] = [];

  before(async () => {
    await waitForHooks(['idle', 'openProject', 'setStep', 'dockTab', 'runStart', 'runState', 'runsRows', 'selectRun', 'setSolver', 'acousticsView']);
    hall = ownHall(WORK());
    await m10.openProject(hall);
    run = await runSpps();
    json = cliReport(hall, run.run);
    // No receiver's sphere crosses a wall: SPPS would read its level low (the fixture's 0.6 m).
    const verdict = (JSON.parse(readFileSync(path.join(path.dirname(hall), 'runs', run.run, 'run.json'), 'utf8').replace(/^﻿/, '')) as { verdict: { warnings: { code: string; detail: string }[] } }).verdict;
    const crossing = verdict.warnings.filter((w) => w.code === 'receiver_sphere_crosses_surface');
    assert.deepEqual(crossing, [], 'receiver spheres crossing a surface');
    const rxs = (json.spps as { point_receivers: Receiver[] }).point_receivers;
    // In the project's order: config.xml, and so the report, lists the sources last first.
    const inReport = rxs[0].per_source.map((p) => p.source);
    names = (JSON.parse(readFileSync(hall, 'utf8')) as { sources: { name: string }[] }).sources.map((s) => s.name).filter((n) => inReport.includes(n));
    assert.deepEqual([...names].sort(), [...inReport].sort(), 'every source of the report is a project source');
    await showRun(run.run);
  });

  it('b77-picker: a Source picker with the three sources and the sources summed, opening on the first', async () => {
    assert.equal(names.length, 3, `the report's sources: ${names.join(', ')}`);
    const s = await scan();
    const opts = s.options.filter((o) => o.control === 'source');
    assert.deepEqual(
      opts.map((o) => [o.value, o.text]),
      [...names.map((n) => [n, n]), ['', SOURCES_SUMMED]],
      'source options',
    );
    const v = await hook<View & { source: string | null }>('acousticsView');
    assert.equal(v.source, names[0], 'opens on the first source');
    // Control: the sources summed refuse EDT and C80 at every receiver (several_sources).
    await pick('source', '');
    const summed = await scan();
    const refused = summed.cells.filter((c) => c.labelled.table === 'receivers-table' && ['edt_s', 'c80_db'].includes(c.param));
    assert.ok(refused.length > 0, 'EDT and C80 cells under the sources summed');
    const why = (p: string) => String(((json as Json) && p.split('.').reduce<unknown>((v, k) => (v as Record<string, unknown> | undefined)?.[k], json)) ?? '');
    for (const c of refused) {
      assert.equal(c.status, 'refused', `${c.param} at ${c.receiver} summed`);
      assert.ok(c.labelled.paths.length > 0 && c.labelled.paths.every((p) => !p.includes('.per_source.')), `${c.param} at ${c.receiver} summed is the receiver's own`);
      const code = c.labelled.paths.find((p) => p.endsWith('.not_evaluable.code'));
      assert.ok(code, `${c.param} at ${c.receiver}: a refusal with its code`);
      assert.equal(why(code.replace(/code$/, 'error.why.why')), 'several_sources', `${c.param} at ${c.receiver}`);
    }
    await pick('source', names[0]);
  });

  it('b77-numbers: under every source and selection, the numbers are the JSON and the labels name the source', async () => {
    assert.ok(run);
    const mismatches: string[] = [];
    let compared = 0;
    const perSource = new Map<string, Set<string>>();
    await everySelection(async (what) => {
      const s = await scan();
      assert.equal(s.state, 'ready', what);
      for (const n of s.nums) {
        const m = numberMismatch(n, json);
        if (m) mismatches.push(`${what}: ${m}`);
        compared++;
      }
      for (const t of s.strs) {
        const m = stringMismatch(t, json);
        if (m) mismatches.push(`${what}: ${m}`);
      }
      for (const c of s.cells) {
        const m = cellLabelMismatch(c.labelled, json);
        if (m) mismatches.push(`${what}: ${m}`);
        const src = c.labelled.source ?? '';
        if (src && src !== SOURCES_SUMMED && c.labelled.table === 'receivers-table' && ['edt_s', 'c80_db'].includes(c.param) && c.status !== 'refused') {
          assert.ok(c.nums.some((p) => p.endsWith('.lo')) && c.nums.some((p) => p.endsWith('.hi')), `${what}: ${c.param} at ${c.receiver} has no range`);
          if (!perSource.has(src)) perSource.set(src, new Set());
          perSource.get(src)?.add(`${c.receiver}|${c.param}`);
        }
      }
    });
    assert.deepEqual(mismatches, [], `${mismatches.length} of ${compared} numbers or labels`);
    assert.ok(compared > 0);
    // Each source shows EDT and C80 at every receiver.
    const labels = (json.spps as { point_receivers: Receiver[] }).point_receivers.map((r) => r.label);
    for (const n of names) {
      const seen = perSource.get(n) ?? new Set<string>();
      for (const l of labels) for (const p of ['edt_s', 'c80_db']) assert.ok(seen.has(`${l}|${p}`), `${n}: ${p} at ${l} shown with a range`);
    }
    // Control: one cell, two sources, two paths; and S01's cell under S02's name is caught.
    await pick('source', names[0]);
    const a = (await scan()).cells.find((c) => c.labelled.table === 'receivers-table' && c.param === 'c80_db' && c.status !== 'refused');
    await pick('source', names[1]);
    const b = (await scan()).cells.find((c) => c.labelled.table === 'receivers-table' && c.param === 'c80_db' && c.receiver === a?.receiver);
    assert.ok(a && b, 'C80 at the same receiver under two sources');
    assert.notDeepEqual(a.labelled.paths, b.labelled.paths);
    assert.notEqual(cellLabelMismatch({ ...a.labelled, source: names[1] }, json), null, "a source's cell under another source's name");
    await pick('source', names[0]);
  });
});
