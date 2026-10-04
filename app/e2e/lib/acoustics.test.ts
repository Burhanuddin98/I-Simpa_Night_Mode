// acoustics.ts's rules held to hand-made cases, each with the case it must refuse.
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import {
  at,
  cellLabelMismatch,
  labelPattern,
  type LabelledCell,
  MARKS,
  notPassed,
  numberMismatch,
  pathCell,
  rtSeries,
  seriesMismatches,
  strayDigits,
  stringMismatch,
  T30_MARK,
} from './acoustics.ts';

const json = {
  bands_hz: [500, 1000],
  room: { volume_m3: 179.99999999999991, din18041: [{ group: 'A3', target_s: { value: 0.5517246 } }] },
  spps: {
    point_receivers: [
      {
        label: 'R1',
        bands: [
          { parameters: { t30_s: { value: 1.2349, status: 'ok', lo: 1.2, hi: 1.27 }, spl_db: { value: -0.04, status: 'ok', lo: -0.1, hi: 0.02 } }, g_db: { value: 3.1, status: 'ok', lo: 3, hi: 3.2 } },
          { parameters: { t30_s: { not_evaluable: { code: 'params_not_evaluable' } } } },
        ],
        aggregate: { parameters: { spl_db: { value: 2.5, status: 'ok', lo: 2.4, hi: 2.6 } }, dba: { level_db: { value: 50.1, status: 'ok', lo: 50, hi: 50.2 } } },
        sti: { shown: 'male', male: { value: 0.6 } },
      },
      {
        label: 'R2',
        bands: [
          // A value the tab does not show: no range (row 37 (3)); and a status it does not know.
          { parameters: { t30_s: { value: 1.1, status: 'ok' }, spl_db: { value: 1.5, status: 'odd', lo: 1.4, hi: 1.6 } } },
          { parameters: { t30_s: { value: 1.05, status: 'wide', lo: 0.9, hi: 1.2 } } },
        ],
      },
    ],
  },
};

test('at reads dot paths through objects and arrays', () => {
  assert.equal(at(json, 'room.din18041.0.group'), 'A3');
  assert.equal(at(json, 'spps.point_receivers.0.label'), 'R1');
  assert.equal(at(json, 'room.nope.x'), undefined);
});

test('a number equals its JSON value at the displayed precision, and only then', () => {
  const n = (text: string, path: string, digits = '2', scale: string | null = null) => ({ text, path, digits, scale });
  assert.equal(numberMismatch(n('0.55', 'room.din18041.0.target_s.value'), json), null);
  assert.equal(numberMismatch(n('180', 'room.volume_m3', '0'), json), null);
  assert.equal(numberMismatch(n('1.23', 'spps.point_receivers.0.bands.0.parameters.t30_s.value'), json), null);
  assert.equal(numberMismatch(n('0.5', 'bands_hz.0', '1', '0.001'), json), null);
  assert.equal(numberMismatch(n('-0.0', 'spps.point_receivers.0.bands.0.parameters.spl_db.value', '1'), json), null);
  // Says no: a digit off, the wrong precision, the wrong path, a refusal read as a number,
  // a unit inside the number, the scale left out.
  assert.match(numberMismatch(n('0.56', 'room.din18041.0.target_s.value'), json) ?? '', /shown 0.56/);
  assert.match(numberMismatch(n('0.552', 'room.din18041.0.target_s.value'), json) ?? '', /3 decimals/);
  assert.match(numberMismatch(n('1.24', 'spps.point_receivers.0.bands.0.parameters.t30_s.value'), json) ?? '', /shown 1.24/);
  assert.match(numberMismatch(n('1.23', 'spps.point_receivers.0.bands.1.parameters.t30_s.value'), json) ?? '', /not a number/);
  assert.match(numberMismatch(n('0.55 s', 'room.din18041.0.target_s.value'), json) ?? '', /plain decimal/);
  assert.notEqual(numberMismatch(n('0.5', 'bands_hz.0', '1'), json), null);
  assert.match(numberMismatch(n('0.55', 'room.din18041.0.target_s.value', null as unknown as string), json) ?? '', /data-digits/);
});

test('a string equals its JSON string exactly', () => {
  assert.equal(stringMismatch({ path: 'room.din18041.0.group', text: 'A3' }, json), null);
  assert.match(stringMismatch({ path: 'room.din18041.0.group', text: 'A3 ' }, json) ?? '', /shown/);
  assert.match(stringMismatch({ path: 'room.volume_m3', text: '180' }, json) ?? '', /not a string/);
});

test('a stray digit is found, and text without one passes', () => {
  assert.equal(strayDigits('Reverberation time against DIN target'), null);
  assert.notEqual(strayDigits('T30 at R1'), null);
});

test('the parameters not PASS are named from the summary', () => {
  assert.deepEqual(notPassed({ parameters: { spl_db: { status: 'PASS' }, t30_s: { status: 'FAIL' }, dba: { status: 'FAIL' } } }), ['t30_s', 'dba']);
});

test("a parameter's label is found as a word, not inside another", () => {
  assert.ok(labelPattern('t30_s').test('T30 per band'));
  assert.ok(!labelPattern('t30_s').test('T300'));
  assert.ok(labelPattern('dba').test('level dB(A)'));
  assert.ok(labelPattern('g_db').test('G per band'));
  assert.ok(!labelPattern('g_db').test('Glass window'));
  assert.ok(!labelPattern('edt_s').test('EDTs'));
  assert.throws(() => labelPattern('nope'));
});


test("a drawn RT series must be the JSON's, path for path and value for value", () => {
  const want = rtSeries(json, 0, 't30_s');
  assert.deepEqual(want.values, [1.2349, null]);
  assert.deepEqual(want.lo, [1.2, null]);
  assert.deepEqual(want.hi, [1.27, null]);
  assert.deepEqual(seriesMismatches(want, json, 0, 't30_s'), []);
  // Says no: the other run's value, a refusal drawn as zero, a missing band, the wrong receiver's path.
  assert.equal(seriesMismatches({ ...want, values: [1.3, null] }, json, 0, 't30_s').length, 1);
  assert.equal(seriesMismatches({ ...want, values: [1.2349, 0] }, json, 0, 't30_s').length, 1);
  assert.ok(seriesMismatches({ ...want, paths: want.paths.slice(0, 1), values: [1.2349] }, json, 0, 't30_s').length >= 1);
  assert.equal(seriesMismatches({ ...want, paths: want.paths.map((p) => p.replace('receivers.0', 'receivers.1')) }, json, 0, 't30_s').length, 2);
  // Says no: a range drawn wrong, or not drawn at all.
  assert.equal(seriesMismatches({ ...want, lo: [1.1, null] }, json, 0, 't30_s').length, 1);
  assert.ok(seriesMismatches({ paths: want.paths, values: want.values } as typeof want, json, 0, 't30_s').length >= 1);
});

test("the RT series the JSON gives is the tables' filter: a value without a range or with another status is a gap", () => {
  // Assay (M12, LOW): the chart drew `.value` wherever it was a number.
  const r2 = rtSeries(json, 1, 't30_s');
  assert.deepEqual(r2.values, [null, 1.05], 'no range at band 0: not drawn');
  assert.deepEqual(r2.lo, [null, 0.9]);
  assert.deepEqual(rtSeries(json, 1, 'spl_db').values, [null, null], "status 'odd': not drawn");
  // Says no: the raw value drawn where the table shows none.
  assert.equal(seriesMismatches({ ...r2, values: [1.1, 1.05] }, json, 1, 't30_s').length, 1);
});

test("a cell's visible row and column labels name the receiver, parameter and band of its JSON paths", () => {
  const p = (r: number, rest: string) => `spps.point_receivers.${r}.${rest}`;
  const recv = (row: string, col: string, band: string, paths: string[]): LabelledCell => ({ table: 'receivers-table', rowHead: row, colHead: col, band, receiver: '', paths });
  const rt = (row: string, col: string, receiver: string, paths: string[]): LabelledCell => ({ table: 'rt-table', rowHead: row, colHead: col, band: '', receiver, paths });
  const t30 = [p(0, 'bands.0.parameters.t30_s.value'), p(0, 'bands.0.parameters.t30_s.lo'), p(0, 'bands.0.parameters.t30_s.hi')];
  assert.equal(cellLabelMismatch(recv('R1', 'T30', '500 Hz', t30), json), null);
  assert.equal(cellLabelMismatch(rt('500 Hz', 'T30', 'R1', t30), json), null);
  assert.equal(cellLabelMismatch(recv('R1', 'T30', '1 kHz', [p(0, 'bands.1.parameters.t30_s.not_evaluable.code')]), json), null);
  assert.equal(cellLabelMismatch(recv('R1', 'SPL', 'bands summed', [p(0, 'aggregate.parameters.spl_db.value')]), json), null);
  assert.equal(cellLabelMismatch(recv('R1', 'G', '500 Hz', [p(0, 'bands.0.g_db.value')]), json), null);
  // Receiver-wide parameters: any band may be selected.
  assert.equal(cellLabelMismatch(recv('R1', 'dB(A)', '1 kHz', [p(0, 'aggregate.dba.level_db.value')]), json), null);
  assert.equal(cellLabelMismatch(recv('R1', 'STI', '500 Hz', [p(0, 'sti.male.value')]), json), null);
  // Says no, the deliberately mislabelled cases: the row names another receiver, the column
  // another parameter, the band select another band; in the RT table the row another band and
  // the receiver select another receiver; a cell whose paths disagree; a path of no cell.
  assert.match(cellLabelMismatch(recv('R2', 'T30', '500 Hz', t30), json) ?? '', /receiver/);
  assert.match(cellLabelMismatch(recv('R1', 'T20', '500 Hz', t30), json) ?? '', /parameter/);
  assert.match(cellLabelMismatch(recv('R1', 'T30', '1 kHz', t30), json) ?? '', /band/);
  assert.match(cellLabelMismatch(recv('R1', 'SPL', '500 Hz', [p(0, 'aggregate.parameters.spl_db.value')]), json) ?? '', /band/);
  assert.match(cellLabelMismatch(rt('1 kHz', 'T30', 'R1', t30), json) ?? '', /band/);
  assert.match(cellLabelMismatch(rt('500 Hz', 'T30', 'R2', t30), json) ?? '', /receiver/);
  assert.match(cellLabelMismatch(recv('R1', 'T30', '500 Hz', [t30[0], p(1, 'bands.0.parameters.t30_s.lo')]), json) ?? '', /disagree/);
  assert.match(cellLabelMismatch(recv('R1', 'T30', '500 Hz', ['room.volume_m3']), json) ?? '', /names no/);
  assert.match(cellLabelMismatch(recv('R1', 'T30', '500 Hz', []), json) ?? '', /no path/);
  // Two cells' paths swapped (the e2e's control does this on the page): both are caught.
  const a = recv('R1', 'T30', '500 Hz', t30);
  const b = recv('R1', 'G', '500 Hz', [p(0, 'bands.0.g_db.value')]);
  assert.notEqual(cellLabelMismatch({ ...a, paths: b.paths }, json), null);
  assert.notEqual(cellLabelMismatch({ ...b, paths: a.paths }, json), null);
});

test("the marks the tab may show: EDT's two, T30's (decision 46), STI's note; none says validated", () => {
  assert.equal(T30_MARK, 'Ranges on noise-limited T30 values may be slightly narrow: 1 of 833 checked values fell 1.5 ms outside its range.');
  assert.ok(MARKS.includes(T30_MARK));
  assert.ok(MARKS.includes('STI: noise range not computed'));
  for (const m of MARKS) assert.ok(!/validated/i.test(m), m);
});

test("backlog 77: a per-source cell's labels name its source too, and a summed cell shows no source", () => {
  const two = {
    bands_hz: [500],
    spps: {
      point_receivers: [
        {
          label: 'R1',
          bands: [{ parameters: { t30_s: { not_evaluable: { code: 'params_not_evaluable' } } } }],
          per_source: [
            { source: 'S01', bands: [{ parameters: { t30_s: { value: 1.97, status: 'ok', lo: 1.92, hi: 2.01 } } }] },
            { source: 'S02', bands: [{ parameters: { t30_s: { value: 1.98, status: 'ok', lo: 1.93, hi: 2.03 } } }] },
          ],
        },
      ],
    },
  };
  const cellOf = (source: string, rest: string, row = 'R1'): LabelledCell => ({
    table: 'receivers-table',
    rowHead: row,
    colHead: 'T30',
    band: '500 Hz',
    receiver: '',
    source,
    paths: [`spps.point_receivers.0.${rest}`],
  });
  assert.deepEqual(pathCell('spps.point_receivers.0.per_source.1.bands.0.parameters.t30_s.value'), { receiver: 0, source: 1, param: 't30_s', band: 0 });
  assert.deepEqual(pathCell('spps.point_receivers.0.bands.0.parameters.t30_s.value'), { receiver: 0, source: null, param: 't30_s', band: 0 });
  assert.equal(cellLabelMismatch(cellOf('S02', 'per_source.1.bands.0.parameters.t30_s.value'), two), null);
  assert.equal(cellLabelMismatch(cellOf('all sources summed', 'bands.0.parameters.t30_s.not_evaluable.code'), two), null);
  assert.equal(cellLabelMismatch(cellOf('', 'bands.0.parameters.t30_s.not_evaluable.code'), two), null, 'no source picker');
  // Says no: S01's value under S02, a source's value with the sources summed selected, the summed
  // value under a source, and two sources' paths in one cell.
  assert.match(cellLabelMismatch(cellOf('S02', 'per_source.0.bands.0.parameters.t30_s.value'), two) ?? '', /source/);
  assert.match(cellLabelMismatch(cellOf('all sources summed', 'per_source.0.bands.0.parameters.t30_s.value'), two) ?? '', /source/);
  assert.match(cellLabelMismatch(cellOf('S01', 'bands.0.parameters.t30_s.not_evaluable.code'), two) ?? '', /source/);
  const mixed = { ...cellOf('S01', 'per_source.0.bands.0.parameters.t30_s.value'), paths: ['spps.point_receivers.0.per_source.0.bands.0.parameters.t30_s.value', 'spps.point_receivers.0.per_source.1.bands.0.parameters.t30_s.lo'] };
  assert.match(cellLabelMismatch(mixed, two) ?? '', /disagree/);
});
