// The Acoustics tab's model (model.ts) held to a hand-made report: only PASS parameters, every
// value with its range and status or its refusal, STI's note, EDT's marks, numbers as paths into
// the report, the DIN target, the absorption by group, the Sabine/Eyring table, the variant's run.
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { Report } from '../../bindings/ipc';
import {
  absorption,
  at,
  bandNum,
  bandText,
  cell,
  classical,
  decay,
  din,
  EDT_MARKS,
  MQ2_WORDING,
  PARAM_SPECS,
  receiverRows,
  rtSeries,
  runForVariant,
  shownParams,
  STI_NOTE,
} from './model.ts';

const value = (v: number, status = 'ok', d = 0.01) => ({ value: v, mc_sd: d, status, lo: v - d, hi: v + d });
const refused = (why: string) => ({ not_evaluable: { code: 'params_not_evaluable', message: 'm 1.0', error: { kind: 'not_evaluable', why: { why } } } });
const params = (t30: unknown, edtValidated = true) => ({
  spl_db: value(60.04),
  edt_s: value(0.61, 'wide', 0.05),
  t20_s: value(0.555),
  t30_s: t30,
  c50_db: value(1.25),
  c80_db: value(3.5),
  d50: value(0.555),
  ts_s: value(0.0405, 'wide', 0.002),
  edt: edtValidated ? { validated: true } : { validated: false, validation_note: 'not yet validated: receivers over 1 m read EDT up to 7 % low' },
  edt_validated: edtValidated,
});
const bed = (fail: string[]) => ({
  summary: 'beds/summary.json',
  summary_sha256: 'x',
  parameters: Object.fromEntries(PARAM_SPECS.map((p) => [p.name, { status: fail.includes(p.name) ? 'FAIL' : 'PASS', reasons: [], notes: [] }])),
});
function report(fail: string[] = ['t30_s', 'edt_s', 'g_db', 'dba']): Report {
  const rx = (label: string, edtValidated: boolean) => ({
    label,
    bands: [
      { freq_hz: 500, parameters: params(value(0.6), edtValidated), g_db: value(10.04), decay_curve: { from_s: 0, points: [[0, 0], [0.1, -5.5]] } },
      { freq_hz: 1000, parameters: params(refused('range_not_reached'), edtValidated), g_db: value(9.96), decay_curve: null },
    ],
    aggregate: { parameters: params(value(0.58), false), dba: { level_db: value(55.55) }, decay_curve: { from_s: 0, points: [[0, 0]] } },
    sti: { shown: 'male', male: { value: 0.624, mc_sd: null }, female: { value: 0.6, mc_sd: null } },
  });
  return {
    results_version: 12,
    bed: bed(fail),
    solver: 'spps',
    bands_hz: [500, 1000],
    spps: {
      point_receivers: [rx('R1', true), rx('R2', false)],
      reference: {
        status: 'computed',
        bands: [
          { sabine_s: { value: 0.667, mc_sd: null }, eyring_s: { value: 0.6, mc_sd: null }, kuttruff_s: refused('x') },
          { sabine_s: { value: 0.65, mc_sd: null }, eyring_s: { value: 0.59, mc_sd: null }, kuttruff_s: refused('x') },
        ],
      },
    },
    tcr: null,
    room: {
      status: 'computed',
      volume_m3: 179.99999999999991,
      area_m2: 216,
      din18041: [
        { group: 'A1', use: 'music', target_s: { value: 1.0846, mc_sd: null } },
        { group: 'A3', use: 'teaching, communication', target_s: { value: 0.5517, mc_sd: null } },
        { group: 'A5', use: 'sport', target_s: { not_evaluable: { code: 'params_din_out_of_range', message: '', error: {} } } },
      ],
      din18041_note: 'DIN 18041:2016-03 ... 80 % occupied',
      surfaces: [
        { material_id: 1, faces: 2, area_m2: 60, bands: [{ freq_hz: 500, absorption: 0.1, absorption_area_m2: 6 }, { freq_hz: 1000, absorption: 0.1, absorption_area_m2: 6 }] },
        { material_id: 2, faces: 8, area_m2: 156, bands: [{ freq_hz: 500, absorption: 0.2, absorption_area_m2: 31.2 }, { freq_hz: 1000, absorption: 0.2, absorption_area_m2: 31.2 }] },
      ],
      bands: [{ freq_hz: 500, absorption_area_m2: 37.2 }, { freq_hz: 1000, absorption_area_m2: 37.2 }],
    },
  } as unknown as Report;
}

test('Acoustics: the words are MQ2s and no mark says validated', () => {
  assert.ok(MQ2_WORDING.startsWith('Computed to ISO 3382-1 / IEC 60268-16'));
  for (const t of [MQ2_WORDING, ...EDT_MARKS, STI_NOTE]) assert.ok(!/validated/i.test(t), t);
});

test('Acoustics: only the parameters the report says PASS are shown, read from report.bed', () => {
  assert.deepEqual(shownParams(report()).map((p) => p.name), ['spl_db', 't20_s', 'c50_db', 'c80_db', 'd50', 'ts_s', 'sti']);
  // Says no: the same report with SPL failed and T30 passed shows the other set.
  assert.deepEqual(shownParams(report(['spl_db'])).map((p) => p.name), ['g_db', 'edt_s', 't20_s', 't30_s', 'c50_db', 'c80_db', 'd50', 'ts_s', 'sti', 'dba']);
  // A report with no bed shows nothing; a status other than PASS hides.
  assert.deepEqual(shownParams({ ...report(), bed: undefined } as unknown as Report), []);
  const odd = report();
  (odd.bed.parameters as unknown as Record<string, { status: string }>).spl_db.status = 'pass';
  assert.ok(!shownParams(odd).some((p) => p.name === 'spl_db'));
  // Every row of the receivers table has exactly the shown parameters' cells.
  for (const row of receiverRows(report(), 0)) {
    assert.equal(row.cells.length, 7);
    assert.ok(row.cells.every((c) => c === null || !['t30_s', 'edt_s', 'g_db', 'dba'].includes(c.param)));
  }
});

test('Acoustics: a value carries its range and status, each a path into the report at its decimals', () => {
  const r = report();
  const spl = PARAM_SPECS.find((p) => p.name === 'spl_db')!;
  const c = cell(r, spl, 0, 0)!;
  assert.equal(c.status, 'ok');
  assert.deepEqual(c.value, { path: 'spps.point_receivers.0.bands.0.parameters.spl_db.value', digits: 1, text: '60.0' });
  assert.equal(c.lo?.path, 'spps.point_receivers.0.bands.0.parameters.spl_db.lo');
  assert.equal(c.hi?.text, (60.05).toFixed(1));
  for (const n of [c.value!, c.lo!, c.hi!]) assert.equal((at(r, n.path) as number).toFixed(n.digits), n.text);
  const ts = cell(r, PARAM_SPECS.find((p) => p.name === 'ts_s')!, 1, 'sum')!;
  assert.equal(ts.status, 'wide');
  assert.equal(ts.band, 'sum');
  assert.equal(ts.value?.path, 'spps.point_receivers.1.aggregate.parameters.ts_s.value');
  assert.equal(ts.value?.text, '0.041');
});

test('Acoustics: a refusal is shown by its code and kind, never as a number', () => {
  const r = report(['spl_db']);
  const c = cell(r, PARAM_SPECS.find((p) => p.name === 't30_s')!, 0, 1)!;
  assert.equal(c.status, 'refused');
  assert.equal(c.value, null);
  assert.deepEqual(c.refusal, {
    code: { path: 'spps.point_receivers.0.bands.1.parameters.t30_s.not_evaluable.code', text: 'params_not_evaluable' },
    why: { path: 'spps.point_receivers.0.bands.1.parameters.t30_s.not_evaluable.error.why.why', text: 'range_not_reached' },
  });
  // A value with no range is not shown alone (row 37 (3)).
  const bare = report();
  (at(bare, 'spps.point_receivers.0.bands.0.parameters') as Record<string, unknown>).spl_db = { value: 60, mc_sd: null };
  assert.equal(cell(bare, PARAM_SPECS[0], 0, 0), null);
});

test('Acoustics: STI shows its value with the noise-range note; dB(A) and G where the report has them', () => {
  const r = report(['spl_db']);
  const sti = cell(r, PARAM_SPECS.find((p) => p.name === 'sti')!, 1, 0)!;
  assert.equal(sti.status, 'value');
  assert.equal(sti.note, STI_NOTE);
  assert.equal(sti.band, 'all');
  assert.equal(sti.value?.path, 'spps.point_receivers.1.sti.male.value');
  assert.equal(sti.value?.text, '0.62');
  assert.equal(cell(r, PARAM_SPECS.find((p) => p.name === 'dba')!, 0, 1)?.value?.path, 'spps.point_receivers.0.aggregate.dba.level_db.value');
  assert.equal(cell(r, PARAM_SPECS.find((p) => p.name === 'g_db')!, 0, 'sum'), null, 'no G for the bands summed');
});

test('Acoustics: EDT, when shown, carries why it is unchecked where edt_validated is false', () => {
  const r = report(['spl_db']);
  const edt = PARAM_SPECS.find((p) => p.name === 'edt_s')!;
  assert.equal(cell(r, edt, 0, 0)?.note, null);
  assert.equal(cell(r, edt, 1, 0)?.note, 'unchecked: receiver larger than one metre in radius');
  assert.match(cell(r, edt, 0, 'sum')?.note ?? '', /^unchecked/);
  assert.equal(EDT_MARKS.length, 2);
});

test('Acoustics: the RT series are the shown reverberation times, value for value, refusals as gaps', () => {
  const s = rtSeries(report(['spl_db']), 0);
  assert.deepEqual(s.map((x) => x.param), ['edt_s', 't20_s', 't30_s']);
  assert.deepEqual(s[2].values, [0.6, null]);
  assert.equal(s[2].paths[1], 'spps.point_receivers.0.bands.1.parameters.t30_s.value');
  assert.deepEqual(rtSeries(report(), 1).map((x) => x.param), ['t20_s']);
});

test('Acoustics: the DIN target is the report own, by group, with the volume', () => {
  const d = din(report(), 'A3')!;
  assert.equal(d.target?.text, '0.55');
  assert.equal(d.target?.path, 'room.din18041.1.target_s.value');
  assert.equal(d.volume?.text, '180');
  assert.equal(d.group.text, 'A3');
  assert.equal(din(report(), 'A1')?.target?.text, '1.08');
  const a5 = din(report(), 'A5')!;
  assert.equal(a5.target, null);
  assert.equal(a5.refusal?.code.text, 'params_din_out_of_range');
  assert.equal(din(report(), 'A9'), null);
});

test('Acoustics: bands as numbers of bands_hz, and the absorption, classical table and decay as paths', () => {
  const r = report();
  assert.deepEqual(bandNum(r, 0), { num: { path: 'bands_hz.0', digits: 0, text: '500' }, unit: 'Hz' });
  assert.deepEqual(bandNum(r, 1), { num: { path: 'bands_hz.1', digits: 0, scale: 0.001, text: '1' }, unit: 'kHz' });
  assert.equal(bandText(1250), '1.25 kHz');
  assert.equal(bandText(125), '125 Hz');
  const a = absorption(r, new Map([[2, ['Walls', 'Rear wall']]]))!;
  assert.deepEqual(a.rows[1].names, ['Walls', 'Rear wall']);
  assert.deepEqual(a.rows[0].names, []);
  assert.equal(a.rows[1].bands[0]?.text, '31.20');
  assert.equal(a.totals[1]?.path, 'room.bands.1.absorption_area_m2');
  const c = classical(r);
  assert.equal(c[0].cells[0].value?.path, 'spps.reference.bands.0.sabine_s.value');
  // Kuttruff refused in every band: its column is left out; refused in one band only, kept.
  assert.deepEqual(c[0].cells.map((x) => x.label), ['Sabine', 'Eyring']);
  const k = report();
  (at(k, 'spps.reference.bands.0') as Record<string, unknown>).kuttruff_s = { value: 0.62, mc_sd: 0.001 };
  const ck = classical(k);
  assert.deepEqual(ck[1].cells.map((x) => x.label), ['Sabine', 'Eyring', 'Kuttruff']);
  assert.equal(ck[1].cells[2].refusal?.code.text, 'params_not_evaluable');
  assert.deepEqual(decay(r, 0, 0), { path: 'spps.point_receivers.0.bands.0.decay_curve', t: [0, 0.1], db: [0, -5.5] });
  assert.equal(decay(r, 0, 1), null);
});

test("Acoustics: a variant's newest OK run is the one shown after a switch", () => {
  const rows = [
    { run: 'a', status: 'OK', variant: null },
    { run: 'b', status: 'OK', variant: 'v' },
    { run: 'c', status: 'OK', variant: null },
    { run: 'd', status: 'FAIL', variant: null },
  ];
  assert.equal(runForVariant(rows, null), 'c');
  assert.equal(runForVariant(rows, 'v'), 'b');
  assert.equal(runForVariant(rows, 'w'), null);
});
