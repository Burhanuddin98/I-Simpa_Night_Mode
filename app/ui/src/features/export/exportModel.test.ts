// W9's export model (exportModel.ts) held to a hand-made report: the rows are the Acoustics tab's
// cells (PASS parameters only, a value only with its range, or its refusal), each number is the
// report's own double at the row's path, and the CSV and JSON give those numbers back exactly.
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { Report, ReportView } from '../../bindings/ipc';
import { at, PARAM_SPECS, T30_MARK } from '../acoustics/model.ts';
import { chartFileName, csvField, exportName, EXPORT_KINDS, exportRefusal, paramRows, paramsCsv, paramsJson, parseCsv } from './exportModel.ts';

const value = (v: number, status = 'ok', d = 0.01) => ({ value: v, mc_sd: d, status, lo: v - d, hi: v + d });
const refused = (why: string) => ({ not_evaluable: { code: 'params_not_evaluable', message: 'm', error: { kind: 'not_evaluable', why: { why } } } });
const params = (t30: unknown) => ({
  spl_db: value(60.04123456789),
  t15_s: value(0.52),
  t20_s: value(0.555),
  t30_s: t30,
  c80_db: { value: 3.5, mc_sd: 0.1, status: 'ok' },
  edt: { validated: true },
  edt_validated: true,
});
function report(fail: string[] = ['edt_s', 'g_db', 'dba', 'c50_db', 'd50', 'ts_s']): Report {
  const rx = (label: string) => ({
    label,
    bands: [
      { freq_hz: 500, parameters: params(value(0.6)), g_db: value(10) },
      { freq_hz: 1000, parameters: params(refused('range_not_reached')), g_db: value(9) },
    ],
    aggregate: { parameters: params(value(0.58)), dba: { level_db: value(55.5) } },
    sti: { shown: 'male', male: { value: 0.624, mc_sd: null } },
  });
  return {
    results_version: 12,
    bed: { parameters: Object.fromEntries(PARAM_SPECS.map((p) => [p.name, { status: fail.includes(p.name) ? 'FAIL' : 'PASS' }])) },
    solver: 'spps',
    bands_hz: [500, 1000],
    spps: { point_receivers: [rx('R1'), rx('Seat, "front"')] },
    tcr: null,
  } as unknown as Report;
}

test('rows: the shown parameters only, every receiver, band and the bands summed; each number the report\'s at its path', () => {
  const r = report();
  const rows = paramRows(r);
  const params = [...new Set(rows.map((x) => x.parameter))];
  assert.deepEqual(params, ['spl_db', 't15_s', 't20_s', 't30_s', 'c80_db', 'sti']);
  // say NO: a FAIL parameter is not exported, whatever the report holds for it.
  assert.ok(!rows.some((x) => x.parameter === 'g_db' || x.parameter === 'dba'));
  // spl: 2 receivers x (2 bands + sum); sti: once a receiver.
  assert.equal(rows.filter((x) => x.parameter === 'spl_db').length, 6);
  assert.deepEqual(rows.filter((x) => x.parameter === 'sti').map((x) => x.band), ['all', 'all']);
  for (const x of rows) {
    if (x.value === null) continue;
    assert.equal(x.value, at(r, `${x.path}.value`), `${x.path}`);
    if (x.lo !== null) assert.equal(x.lo, at(r, `${x.path}.lo`));
    if (x.hi !== null) assert.equal(x.hi, at(r, `${x.path}.hi`));
  }
  const spl = rows.find((x) => x.parameter === 'spl_db' && x.band === '500' && x.receiver === 'R1')!;
  assert.equal(spl.value, 60.04123456789, 'the report\'s double, not the tab\'s one decimal');
  assert.equal(spl.status, 'ok');
  assert.equal(spl.path, 'spps.point_receivers.0.bands.0.parameters.spl_db');
});

test('rows: a refusal has its code and no number; a value with no range is not shown alone; T30 carries its mark', () => {
  const rows = paramRows(report());
  const t30 = rows.find((x) => x.parameter === 't30_s' && x.band === '1000')!;
  assert.equal(t30.status, 'refused');
  assert.deepEqual([t30.value, t30.lo, t30.hi], [null, null, null]);
  assert.equal(t30.refusal, 'params_not_evaluable');
  assert.equal(t30.why, 'range_not_reached');
  assert.equal(t30.marks, T30_MARK);
  // say NO: C80 has a value but no range in this report: no number leaves.
  const c80 = rows.find((x) => x.parameter === 'c80_db')!;
  assert.equal(c80.status, 'not shown');
  assert.equal(c80.value, null);
  const sti = rows.find((x) => x.parameter === 'sti')!;
  assert.equal(sti.status, 'value');
  assert.equal(sti.note, 'noise range not computed');
  assert.equal(sti.path, 'spps.point_receivers.0.sti.male');
});

test('rows (decision 56): a lost-particle warning or refusal leaves with its share, the report\'s double', () => {
  const r = report();
  const band = at(r, 'spps.point_receivers.0.bands.0.parameters') as Record<string, Record<string, unknown>>;
  band.t30_s.lost_share_warning = 0.0041;
  band.t20_s = {
    not_evaluable: {
      code: 'params_not_evaluable',
      message: 'm',
      error: { kind: 'not_evaluable', quantity: 'T20', why: { why: 'lost_particles', share: 0.0123, limit: 0.01 } },
    },
  };
  const rows = paramRows(r);
  const pick = (p: string) => rows.find((x) => x.parameter === p && x.band === '500' && x.receiver === 'R1')!;
  assert.equal(pick('t30_s').status, 'ok');
  assert.equal(pick('t30_s').lost_share, 0.0041);
  assert.equal(pick('t20_s').status, 'refused');
  assert.equal(pick('t20_s').why, 'lost_particles');
  assert.equal(pick('t20_s').lost_share, 0.0123);
  // say NO: a value without the warning, and another refusal, carry none.
  assert.equal(pick('spl_db').lost_share, null);
  assert.equal(rows.find((x) => x.parameter === 't30_s' && x.band === '1000')!.lost_share, null);
  // The CSV carries it in its own column, back to the same double.
  const table = parseCsv(paramsCsv(rows));
  const col = table[0].indexOf('lost_share');
  assert.ok(col > 0, 'a lost_share column');
  const t30 = table.find((row) => row[table[0].indexOf('path')] === pick('t30_s').path)!;
  assert.equal(Number(t30[col]), 0.0041);
});

test('CSV: quoted where it must be, and every number parses back to the report\'s double', () => {
  assert.equal(csvField('plain'), 'plain');
  assert.equal(csvField('Seat, "front"'), '"Seat, ""front"""');
  assert.equal(csvField('a\nb'), '"a\nb"');
  const r = report();
  const rows = paramRows(r);
  const text = paramsCsv(rows);
  const table = parseCsv(text);
  assert.deepEqual(table[0], ['receiver', 'source', 'parameter', 'label', 'unit', 'band_hz', 'status', 'value', 'lo', 'hi', 'refusal', 'why', 'note', 'lost_share', 'marks', 'path']);
  assert.equal(table.length, rows.length + 1);
  const col = (k: string) => table[0].indexOf(k);
  for (const row of table.slice(1)) {
    const p = row[col('path')];
    for (const k of ['value', 'lo', 'hi']) {
      if (row[col(k)] === '') continue;
      assert.equal(Number(row[col(k)]), at(r, `${p}.${k}`), `${p}.${k}`);
    }
  }
  assert.ok(table.some((row) => row[col('receiver')] === 'Seat, "front"'), 'a label with a comma and quotes survives');
  assert.ok(text.endsWith('\r\n'));
});

test('JSON: the rows with their numbers, the wording and the marks, nothing rounded', () => {
  const r = report();
  const j = JSON.parse(paramsJson(r, { run: 'run-1', project: 'Box' })) as { run: string; solver: string; bands_hz: number[]; wording: string; rows: { value: number | null; path: string }[] };
  assert.equal(j.run, 'run-1');
  assert.deepEqual(j.bands_hz, [500, 1000]);
  assert.match(j.wording, /^Computed to ISO 3382-1/);
  for (const x of j.rows) if (x.value !== null) assert.equal(x.value, at(r, `${x.path}.value`));
});

test('refusals: nothing to export without a run, while reading, or for refused results', () => {
  assert.match(exportRefusal({ onResults: false, run: 'r', view: null }) ?? '', /Results step/);
  assert.match(exportRefusal({ onResults: true, run: null, view: null }) ?? '', /Select a run/);
  assert.match(exportRefusal({ onResults: true, run: 'r', view: null }) ?? '', /still being read/);
  const refusedView = { report: null, state: { verified: false, refusal: { code: 'x' } }, surface_groups: [] } as unknown as ReportView;
  assert.match(exportRefusal({ onResults: true, run: 'r', view: refusedView }) ?? '', /refused/);
  const ok = { report: report(), state: { verified: true }, surface_groups: [] } as unknown as ReportView;
  assert.equal(exportRefusal({ onResults: true, run: 'r', view: ok }), null);
  // say NO: a report whose bed passes nothing has nothing to export.
  const none = { ...ok, report: report(PARAM_SPECS.map((p) => p.name)) } as unknown as ReportView;
  assert.match(exportRefusal({ onResults: true, run: 'r', view: none }) ?? '', /no parameters/);
});

test('names and kinds: the extension the core will check, a name a person can read', () => {
  assert.deepEqual(Object.keys(EXPORT_KINDS), ['csv', 'json', 'png']);
  assert.equal(exportName('Outputs box', 3, 'csv'), 'Outputs box - run 3 - parameters.csv');
  assert.equal(exportName('a/b:c', 1, 'png'), 'a_b_c - run 1 - view.png');
  assert.equal(exportName(null, null, 'json'), 'parameters.json');
});

test('R63: a chart image is named by the project, the run and the chart, safe for Windows', () => {
  assert.equal(chartFileName('Outputs box', 3, 'reverberation time'), 'Outputs box - run 3 - reverberation time chart.png');
  assert.equal(chartFileName('a/b:c', 1, 'decay'), 'a_b_c - run 1 - decay chart.png');
  assert.equal(chartFileName(null, null, 'spectrum'), 'spectrum chart.png');
});
