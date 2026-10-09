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
  LOST_REFUSED,
  LOST_WARNING,
  MQ2_WORDING,
  PARAM_SPECS,
  paramMarks,
  receiverRows,
  rtSeries,
  spectrumSeries,
  runForVariant,
  sourceLabel,
  sourceNote,
  sources,
  shownParams,
  customCell,
  decayTable,
  customColumns,
  customSpec,
  parseCustom,
  STI_NOTE,
  T30_MARK,
} from './model.ts';

const value = (v: number, status = 'ok', d = 0.01) => ({ value: v, mc_sd: d, status, lo: v - d, hi: v + d });
const refused = (why: string) => ({ not_evaluable: { code: 'params_not_evaluable', message: 'm 1.0', error: { kind: 'not_evaluable', why: { why } } } });
const params = (t30: unknown, edtValidated = true) => ({
  spl_db: value(60.04),
  edt_s: value(0.61, 'wide', 0.05),
  t15_s: value(0.52),
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
  for (const t of [MQ2_WORDING, ...EDT_MARKS, STI_NOTE, T30_MARK]) assert.ok(!/validated/i.test(t), t);
});

test('Acoustics: only the parameters the report says PASS are shown, read from report.bed', () => {
  assert.deepEqual(shownParams(report()).map((p) => p.name), ['spl_db', 't15_s', 't20_s', 'c50_db', 'c80_db', 'd50', 'ts_s', 'sti']);
  // Says no: the same report with SPL failed and T30 passed shows the other set.
  assert.deepEqual(shownParams(report(['spl_db'])).map((p) => p.name), ['g_db', 'edt_s', 't15_s', 't20_s', 't30_s', 'c50_db', 'c80_db', 'd50', 'ts_s', 'sti', 'dba']);
  // A report with no bed shows nothing; a status other than PASS hides.
  assert.deepEqual(shownParams({ ...report(), bed: undefined } as unknown as Report), []);
  const odd = report();
  (odd.bed.parameters as unknown as Record<string, { status: string }>).spl_db.status = 'pass';
  assert.ok(!shownParams(odd).some((p) => p.name === 'spl_db'));
  // Every row of the receivers table has exactly the shown parameters' cells.
  for (const row of receiverRows(report(), 0)) {
    assert.equal(row.cells.length, 8);
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

test('Acoustics (decision 56): a lost-particle warning and refusal name the share as a number of the report', () => {
  const r = report(['spl_db']);
  const t30 = PARAM_SPECS.find((p) => p.name === 't30_s')!;
  const t20 = PARAM_SPECS.find((p) => p.name === 't20_s')!;
  // No warning in the report: none in the cell.
  assert.equal(cell(r, t30, 0, 0)?.lost, undefined);
  // A warning: the share, in %, a path into the report at its decimals.
  const band = at(r, 'spps.point_receivers.0.bands.0.parameters') as Record<string, Record<string, unknown>>;
  band.t30_s.lost_share_warning = 0.0041;
  const c = cell(r, t30, 0, 0)!;
  assert.equal(c.status, 'ok');
  assert.deepEqual(c.lost, { path: 'spps.point_receivers.0.bands.0.parameters.t30_s.lost_share_warning', digits: 2, scale: 100, text: '0.41' });
  assert.equal(cell(r, t20, 0, 0)?.lost, undefined, 'only the value that carries it');
  assert.doesNotMatch(LOST_WARNING, /\d/, 'the words hold no digit; the share is the number');
  // A refusal lost_particles: the share it names.
  band.t20_s = {
    not_evaluable: {
      code: 'params_not_evaluable',
      message: 'm',
      error: { kind: 'not_evaluable', quantity: 'T20', why: { why: 'lost_particles', share: 0.021, limit: 0.01 } },
    },
  };
  const refusedCell = cell(r, t20, 0, 0)!;
  assert.equal(refusedCell.status, 'refused');
  assert.equal(refusedCell.refusal?.why?.text, 'lost_particles');
  assert.deepEqual(refusedCell.refusal?.lost, {
    path: 'spps.point_receivers.0.bands.0.parameters.t20_s.not_evaluable.error.why.share',
    digits: 2,
    scale: 100,
    text: '2.10',
  });
  assert.doesNotMatch(LOST_REFUSED, /\d/);
  // Says no: another refusal names no share.
  assert.equal(cell(r, t30, 0, 1)?.refusal?.lost, undefined);
  // STI and C80 carry it the same way.
  const sti = PARAM_SPECS.find((p) => p.name === 'sti')!;
  (at(r, 'spps.point_receivers.0.sti.male') as Record<string, unknown>).lost_share_warning = 0.005;
  assert.equal(cell(r, sti, 0, 0)?.lost?.text, '0.50');
  assert.equal(cell(r, sti, 0, 0)?.lost?.path, 'spps.point_receivers.0.sti.male.lost_share_warning');
  band.c80_db.lost_share_warning = 0.0041;
  assert.equal(cell(r, PARAM_SPECS.find((p) => p.name === 'c80_db')!, 0, 0)?.lost?.text, '0.41');
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
  assert.deepEqual(s.map((x) => x.param), ['edt_s', 't15_s', 't20_s', 't30_s']);
  assert.deepEqual(s[3].values, [0.6, null]);
  assert.equal(s[3].paths[1], 'spps.point_receivers.0.bands.1.parameters.t30_s.value');
  assert.deepEqual(s[1].values, [0.52, 0.52], 'T15 drawn as its cells show it');
  assert.deepEqual(rtSeries(report(), 1).map((x) => x.param), ['t15_s', 't20_s']);
});

test('Acoustics: the RT chart goes through the tables filter: only what a cell shows is drawn, with its range', () => {
  // Assay (M12, LOW): the chart drew `.value` wherever it was a number. A value the table does
  // not show (a status other than ok/wide, or no range) must be a gap in the chart too.
  const r = report(['spl_db']);
  const b0 = (r as unknown as { spps: { point_receivers: { bands: { parameters: Record<string, unknown> }[] }[] } }).spps.point_receivers[0].bands[0].parameters;
  b0.t20_s = { value: 0.555, mc_sd: 0.01, status: 'ok' }; // no range: the table shows nothing
  b0.edt_s = { value: 0.61, mc_sd: 0.05, status: 'unknown', lo: 0.56, hi: 0.66 }; // not ok/wide
  const t20 = PARAM_SPECS.find((p) => p.name === 't20_s')!;
  assert.equal(cell(r, t20, 0, 0), null, 'the table shows no T20 here');
  const s = rtSeries(r, 0);
  const by = (n: string) => s.find((x) => x.param === n)!;
  assert.deepEqual(by('t20_s').values, [null, 0.555], 'T20 drawn only where its cell shows it');
  assert.deepEqual(by('edt_s').values, [null, 0.61]);
  // The range drawn is the cell's: lo and hi where the value is drawn, gaps where it is not.
  assert.deepEqual(by('t30_s').values, [0.6, null]);
  assert.deepEqual(by('t30_s').lo, [0.6 - 0.01, null]);
  assert.deepEqual(by('t30_s').hi, [0.6 + 0.01, null]);
  assert.deepEqual(by('t20_s').lo, [null, 0.555 - 0.01]);
  // Every drawn number is a path the gate reads: values, and the range beside them.
  assert.equal(by('t30_s').paths[0], 'spps.point_receivers.0.bands.0.parameters.t30_s.value');
  assert.equal(by('t30_s').loPaths[0], 'spps.point_receivers.0.bands.0.parameters.t30_s.lo');
  assert.equal(by('t30_s').hiPaths[0], 'spps.point_receivers.0.bands.0.parameters.t30_s.hi');
  // Only PASS parameters are drawn (gate (b)), as before.
  assert.deepEqual(rtSeries(report(), 0).map((x) => x.param), ['t15_s', 't20_s']);
});

test('Acoustics R9: the spectrum is the receivers table’s SPL, band by band, gaps where a cell shows none', () => {
  const r = report();
  const s = spectrumSeries(r, 0);
  assert.deepEqual(s.map((x) => x.param), ['spl_db']);
  const spl = PARAM_SPECS.find((p) => p.name === 'spl_db')!;
  // Each drawn level is the value its cell shows, read at the cell's path, with the cell's range.
  r.bands_hz.forEach((_, b) => {
    const c = cell(r, spl, 0, b);
    assert.ok(c && c.value && c.lo && c.hi);
    assert.equal(s[0].values[b], 60.04);
    assert.equal(s[0].paths[b], `spps.point_receivers.0.bands.${b}.parameters.spl_db.value`);
    assert.equal(s[0].lo[b], 60.04 - 0.01);
  });
  // A level the table would not show (no range) is a gap, never drawn.
  const b1 = (r as unknown as { spps: { point_receivers: { bands: { parameters: Record<string, unknown> }[] }[] } }).spps.point_receivers[0].bands[1].parameters;
  b1.spl_db = { value: 61, mc_sd: 0.01, status: 'ok' };
  assert.equal(cell(r, spl, 0, 1), null);
  assert.deepEqual(spectrumSeries(r, 0)[0].values, [60.04, null]);
  // SPL without a bed PASS: no spectrum at all (withheld stays withheld).
  assert.deepEqual(spectrumSeries(report(['spl_db']), 0), []);
});

test('Acoustics: the marks beside the table: EDT row 37, T30 decision 46, STI MQ3, each only with its parameter', () => {
  assert.equal(T30_MARK, 'Ranges on noise-limited T30 values may be slightly narrow: 1 of 833 checked values fell 1.5 ms outside its range.');
  const all = paramMarks(report(['spl_db']));
  assert.deepEqual(
    all.map((m) => [m.param, m.text]),
    [
      ['edt_s', EDT_MARKS[0]],
      ['edt_s', EDT_MARKS[1]],
      ['t30_s', T30_MARK],
      ['sti', `STI: ${STI_NOTE}`],
    ],
  );
  // Say-NO: with T30 and EDT not PASS, neither carries a mark (nothing of theirs is on screen).
  assert.deepEqual(paramMarks(report()).map((m) => m.param), ['sti']);
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

// Backlog 77: two sources. The summed echogram refuses the onset-relative parameters
// (`several_sources`); each source's own echogram, `per_source`, carries them. R2 lists its
// sources in the other order, so a source is found by its name, not its position.
function twoSources(fail: string[] = ['t30_s', 'edt_s', 'g_db', 'dba'], perSource = true): Report {
  const r = report(fail);
  const several = { not_evaluable: { code: 'params_not_evaluable', message: 'm', error: { kind: 'not_evaluable', why: { why: 'several_sources', sources: ['A', 'B'] } } } };
  const src = (source: string, t30: number) => ({
    source,
    file: `x/${source}.recp`,
    arrival_s: 0.01,
    bands: [
      { freq_hz: 500, parameters: params(value(t30)), g_db: value(9.5), decay_curve: { from_s: 0, points: [[0, 0], [0.1, -6]] } },
      { freq_hz: 1000, parameters: params(value(t30 - 0.1)), g_db: value(9.4), decay_curve: null },
    ],
    aggregate: { parameters: params(value(t30 - 0.05), false), dba: { level_db: value(54.5) }, decay_curve: null },
  });
  const rxs = (r as unknown as { spps: { point_receivers: Record<string, unknown>[] } }).spps.point_receivers;
  rxs.forEach((rx, i) => {
    for (const b of rx.bands as { parameters: Record<string, unknown> }[]) for (const k of ['edt_s', 't20_s', 't30_s', 'c50_db', 'c80_db', 'd50', 'ts_s']) b.parameters[k] = several;
    rx.per_source = perSource ? (i === 0 ? [src('A', 0.7), src('B', 0.9)] : [src('B', 0.95), src('A', 0.75)]) : [];
  });
  return r;
}

test('Acoustics (77): the sources are the per-source echograms, and only when there are two or more', () => {
  assert.deepEqual(sources(twoSources()), ['A', 'B']);
  // In the open project's order where it names them (config.xml lists them last first), then the
  // report's.
  assert.deepEqual(sources(twoSources(), ['B', 'A']), ['B', 'A']);
  assert.deepEqual(sources(twoSources(), ['B']), ['B', 'A']);
  assert.deepEqual(sources(twoSources(), ['Z']), ['A', 'B']);
  assert.deepEqual(sources(report()), [], 'one summed echogram, no per-source');
  assert.deepEqual(sources(twoSources(undefined, false)), []);
});

test("Acoustics (77): a source's values are paths into that source's own echogram, found by name", () => {
  const r = twoSources(['spl_db']);
  const t30 = PARAM_SPECS.find((p) => p.name === 't30_s')!;
  const a = cell(r, t30, 0, 0, 'A')!;
  assert.equal(a.status, 'ok');
  assert.equal(a.value?.path, 'spps.point_receivers.0.per_source.0.bands.0.parameters.t30_s.value');
  assert.equal(a.value?.text, '0.70');
  // R2 lists B first: A is its per_source.1.
  assert.equal(cell(r, t30, 1, 0, 'A')?.value?.path, 'spps.point_receivers.1.per_source.1.bands.0.parameters.t30_s.value');
  assert.equal(cell(r, t30, 1, 'sum', 'B')?.value?.path, 'spps.point_receivers.1.per_source.0.aggregate.parameters.t30_s.value');
  for (const n of [a.value!, a.lo!, a.hi!]) assert.equal((at(r, n.path) as number).toFixed(n.digits), n.text);
  // G and dB(A) are the source's own; STI is defined on the sources together, so none per source.
  assert.equal(cell(r, PARAM_SPECS.find((p) => p.name === 'g_db')!, 0, 1, 'B')?.value?.path, 'spps.point_receivers.0.per_source.1.bands.1.g_db.value');
  assert.equal(cell(r, PARAM_SPECS.find((p) => p.name === 'dba')!, 0, 0, 'B')?.value?.path, 'spps.point_receivers.0.per_source.1.aggregate.dba.level_db.value');
  assert.equal(cell(r, PARAM_SPECS.find((p) => p.name === 'sti')!, 0, 0, 'A'), null);
  // The sources summed are what they were: refused several_sources, shown by code and kind.
  const summed = cell(r, t30, 0, 0)!;
  assert.equal(summed.status, 'refused');
  assert.equal(summed.refusal?.why?.text, 'several_sources');
  // Says no: a source the receiver has no echogram for has no place, never the summed value.
  assert.equal(cell(r, t30, 0, 0, 'C'), null);
});

test("Acoustics (77): the receivers table, RT series and decay follow the source, through the same filters", () => {
  const r = twoSources(['spl_db']);
  const rows = receiverRows(r, 0, 'B');
  assert.equal(rows[0].cells.find((c) => c?.param === 't30_s')?.value?.text, '0.90');
  assert.equal(rows[1].cells.find((c) => c?.param === 't30_s')?.value?.text, '0.95');
  const s = rtSeries(r, 1, 'A');
  const t30 = s.find((x) => x.param === 't30_s')!;
  assert.deepEqual(t30.values, [0.75, 0.65]);
  assert.equal(t30.paths[0], 'spps.point_receivers.1.per_source.1.bands.0.parameters.t30_s.value');
  assert.deepEqual(sourceLabel(r, 1, 'A'), { path: 'spps.point_receivers.1.per_source.1.source', text: 'A' });
  assert.equal(sourceLabel(r, 1, null), null);
  assert.deepEqual(decay(r, 0, 0, 'B'), { path: 'spps.point_receivers.0.per_source.1.bands.0.decay_curve', t: [0, 0.1], db: [0, -6] });
  // Gate (b) still holds per source: with T30 not PASS, no source draws it.
  assert.ok(!rtSeries(twoSources(), 0, 'A').some((x) => x.param === 't30_s'));
});

test('Acoustics (77): several sources with no echogram per source say how to get the per-source values', () => {
  assert.match(sourceNote(twoSources(undefined, false)) ?? '', /echogram per source/i);
  assert.equal(sourceNote(twoSources()), null, 'the per-source values are there');
  assert.equal(sourceNote(report()), null, 'one source');
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

// ---- the run-quality advisor after a run (backlog 80, T9) -------------------------------------------

import { adviceCards } from './model.ts';

function advised(): Report {
  const r = report([]) as unknown as Record<string, unknown>;
  r.advice = [
    {
      code: 'range_below_zero',
      cause: 'Monte-Carlo noise: the range reaches below zero.',
      fix: {
        words: 'Use larger receivers (Receiver radius).',
        setting: 'receiver_radius',
        pointer: '/solvers/spps/receiver_radius_m',
        label: 'Receiver radius',
        from: 0.3100000023841858,
        to: 0.6,
        why_no_apply: null,
        bound: 'clearance',
        note: 'This room is outside the rooms the noise model was measured on.',
      },
      values: ['spps.point_receivers.0.bands.1.parameters.t30_s'],
    },
    {
      code: 'decay_not_fitted',
      cause: 'The decay cannot be fitted here.',
      fix: { words: 'No setting.', setting: null, pointer: null, label: null, from: null, to: null, why_no_apply: 'none', bound: null, note: null },
      values: ['spps.point_receivers.1.bands.1.parameters.t30_s'],
    },
    {
      code: 'onset_too_coarse',
      cause: 'The time step is too coarse.',
      fix: {
        words: 'Use a finer time step (Time step).',
        setting: 'time_step',
        pointer: '/solvers/spps/time_step_s',
        label: 'Time step',
        from: 0.004999999888241291,
        to: 0.001,
        why_no_apply: null,
        bound: null,
        note: null,
      },
      values: [],
    },
  ];
  return r as unknown as Report;
}

test('adviceCards: every number is a path into the report, every word the report’s', () => {
  const rep = advised();
  const cards = adviceCards(rep);
  assert.equal(cards.length, 3);
  const [a, b, c] = cards;
  assert.equal(a.code.text, 'range_below_zero');
  assert.equal(a.cause.path, 'advice.0.cause');
  assert.equal(a.words.path, 'advice.0.fix.words');
  assert.deepEqual([a.from?.path, a.from?.text, a.to?.path, a.to?.text, a.unit], ['advice.0.fix.from', '0.31', 'advice.0.fix.to', '0.60', 'm']);
  assert.equal(a.note?.path, 'advice.0.fix.note');
  assert.deepEqual(a.apply, { setting: 'receiver_radius', from: 0.3100000023841858, to: 0.6 });
  // Each shown number is its report value at the shown decimals (gate (a)).
  for (const n of [a.from!, a.to!]) assert.equal(n.text, ((at(rep, n.path) as number) * (n.scale ?? 1)).toFixed(n.digits));
  // No setting: no Apply, the reason is the report's.
  assert.equal(b.apply, null);
  assert.equal(b.why?.path, 'advice.1.fix.why_no_apply');
  // A time step in ms, scaled from the report's seconds.
  assert.deepEqual([c.from?.text, c.from?.scale, c.to?.text, c.unit], ['5', 1000, '1', 'ms']);
});

test('a refused or wide cell names the advice that explains it; an ok one names none', () => {
  const rep = advised();
  const t30 = PARAM_SPECS.find((p) => p.name === 't30_s')!;
  assert.equal(cell(rep, t30, 0, 1)?.advice, 'range_below_zero');
  assert.equal(cell(rep, t30, 1, 1)?.advice, 'decay_not_fitted');
  assert.equal(cell(rep, t30, 0, 0)?.advice, undefined);
  // A report without advice (before results version 17) shows none.
  assert.deepEqual(adviceCards(report([])), []);
});

// Audit fix (b80): an Apply the open project would refuse is not offered.
import { applyConflict } from './model.ts';

test('applyConflict: the -Y Apply is held back while the project refines a surface receiver', () => {
  const cards = adviceCards(advised());
  const yes = { setting: 'preserve_boundary' as const, from: false, to: true };
  const conflicts = [{ setting: 'preserve_boundary' as const, to: true, why: 'refined; -Y forbids that' }];
  assert.equal(applyConflict(yes, conflicts), 'refined; -Y forbids that');
  // Says no: another setting, or no conflict.
  assert.equal(applyConflict(cards[0].apply!, conflicts), null);
  assert.equal(applyConflict(yes, []), null);
});

// R36: a TCR run's receiver levels.
import { tcrLevels } from './model.ts';

test('R36: a TCR receiver shows its direct and total levels per band, and the Global row for the bands summed', () => {
  const band = (freq_hz: number, d: number, s: number | string, e: number) => ({ freq_hz, direct_db: d, total_sabine_db: s, total_eyring_db: e });
  const rep = {
    ...report([]),
    solver: 'tcr',
    spps: null,
    tcr: {
      point_receivers: [
        { label: 'R1', file: 'Punctual receivers/R1.gabe', bands: [band(500, 61.234, 70.25, 69.96), band(1000, 60.5, 69.1, 68.84)], global: { aggregate: 'energetic_sum', direct_db: 63.9, total_sabine_db: 72.71, total_eyring_db: 72.43 } },
        { label: 'R2', file: 'Punctual receivers/R2.gabe', bands: [band(500, 55, 'NaN', 64), band(1000, 54, 63, 62)], global: { aggregate: 'energetic_sum', direct_db: 57.5, total_sabine_db: 66.5, total_eyring_db: 66 } },
      ],
    },
  } as unknown as Report;
  const rows = tcrLevels(rep, 0);
  assert.deepEqual(
    rows.map((r) => [r.receiver.text, r.direct?.text, r.sabine?.text, r.eyring?.text]),
    [
      ['R1', '61.2', '70.3', '70.0'],
      ['R2', '55.0', undefined, '64.0'],
    ],
  );
  assert.equal(rows[0].direct?.path, 'tcr.point_receivers.0.bands.0.direct_db', 'a path into the report, for gate (a)');
  assert.equal(rows[1].sabine, null, 'a value that is not a number is not shown');
  const sum = tcrLevels(rep, 'sum');
  assert.equal(sum[0].sabine?.path, 'tcr.point_receivers.0.global.total_sabine_db');
  assert.equal(sum[0].sabine?.text, '72.7');
  assert.deepEqual(tcrLevels(report([]), 0), [], 'none for an SPPS run');
});

test('Acoustics R15: chosen decay ranges parse, are shown only by their bed, and read the report at their path', () => {
  assert.deepEqual(parseCustom('decay', ' 40, 50,40 '), { ok: true, list: [{ kind: 'decay', span_db: 40 }, { kind: 'decay', span_db: 50 }] });
  assert.deepEqual(parseCustom('decay', ''), { ok: true, list: [] });
  // Says no: outside the core's 10-60 dB, or not a whole number.
  assert.equal(parseCustom('decay', '9').ok, false);
  assert.equal(parseCustom('decay', '61').ok, false);
  assert.equal(parseCustom('decay', '40.5').ok, false);
  assert.equal(customSpec({ kind: 'decay', span_db: 40 }).label, 'T40');
  const r = report() as unknown as Record<string, unknown> & { spps: { point_receivers: { bands: { parameters: Record<string, unknown> }[] }[] } };
  r.custom = [{ kind: 'decay', span_db: 40 }];
  for (const rx of r.spps.point_receivers) for (const b of rx.bands) b.parameters.custom = [{ name: 't40_s', extra: { kind: 'decay', span_db: 40 }, value: value(0.73) }];
  const rep = r as unknown as Report;
  // The bed fixture has no decay_custom entry: withheld.
  assert.deepEqual(customColumns(rep).map((c) => [c.spec.name, c.shown]), [['t40_s', false]]);
  (rep.bed.parameters as unknown as Record<string, unknown>).decay_custom = { status: 'PASS', reasons: [], notes: [] };
  assert.deepEqual(customColumns(rep).map((c) => [c.spec.name, c.shown]), [['t40_s', true]]);
  const c = customCell(rep, 0, 1, 0)!;
  assert.equal(c.status, 'ok');
  assert.deepEqual(c.value, { path: 'spps.point_receivers.1.bands.0.parameters.custom.0.value.value', digits: 2, text: '0.73' });
  assert.equal(customCell(rep, 1, 0, 0), null, 'no second chosen quantity');
});

test('Acoustics R20: chosen C and D limits parse in ms, within 5 to 1000, and are named C<ms> and D<ms>', () => {
  assert.deepEqual(parseCustom('clarity', '30, 100'), { ok: true, list: [{ kind: 'clarity', te_ms: 30 }, { kind: 'clarity', te_ms: 100 }] });
  assert.deepEqual(parseCustom('definition', '80'), { ok: true, list: [{ kind: 'definition', te_ms: 80 }] });
  assert.equal(parseCustom('clarity', '4').ok, false);
  assert.equal(parseCustom('definition', '1001').ok, false);
  assert.deepEqual([customSpec({ kind: 'clarity', te_ms: 30 }).label, customSpec({ kind: 'clarity', te_ms: 30 }).unit], ['C30', 'dB']);
  assert.deepEqual([customSpec({ kind: 'definition', te_ms: 80 }).name, customSpec({ kind: 'definition', te_ms: 80 }).digits], ['d80', 2]);
});

test('Acoustics R27: the Schroeder table is the report decay curve, point by point, only behind its bed', () => {
  const r = report();
  assert.deepEqual(decayTable(r, 0, 0), { withheld: 'withheld: its test bed has not passed' });
  (r.bed.parameters as unknown as Record<string, unknown>).schroeder_table = { status: 'PASS', reasons: [], notes: [] };
  const t = decayTable(r, 0, 0);
  assert.ok(t && 'rows' in t);
  assert.deepEqual(t.rows[1], {
    t: { path: 'spps.point_receivers.0.bands.0.decay_curve.points.1.0', digits: 1, scale: 1000, text: '100.0' },
    level: { path: 'spps.point_receivers.0.bands.0.decay_curve.points.1.1', digits: 2, text: '-5.50' },
  });
  assert.equal(t.rows.length, 2);
  assert.equal(decayTable(r, 0, 1), null, 'no curve in that band');
});
