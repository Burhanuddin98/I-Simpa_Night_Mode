// acoustics.ts's rules held to hand-made cases, each with the case it must refuse.
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { at, labelPattern, notPassed, numberMismatch, rtSeries, seriesMismatches, strayDigits, stringMismatch } from './acoustics.ts';

const json = {
  bands_hz: [500, 1000],
  room: { volume_m3: 179.99999999999991, din18041: [{ group: 'A3', target_s: { value: 0.5517246 } }] },
  spps: {
    point_receivers: [
      {
        label: 'R1',
        bands: [
          { parameters: { t30_s: { value: 1.2349 }, spl_db: { value: -0.04 } } },
          { parameters: { t30_s: { not_evaluable: { code: 'params_not_evaluable' } } } },
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
  assert.deepEqual(seriesMismatches(want, json, 0, 't30_s'), []);
  // Says no: the other run's value, a refusal drawn as zero, a missing band, the wrong receiver's path.
  assert.equal(seriesMismatches({ paths: want.paths, values: [1.3, null] }, json, 0, 't30_s').length, 1);
  assert.equal(seriesMismatches({ paths: want.paths, values: [1.2349, 0] }, json, 0, 't30_s').length, 1);
  assert.ok(seriesMismatches({ paths: want.paths.slice(0, 1), values: [1.2349] }, json, 0, 't30_s').length >= 1);
  assert.equal(seriesMismatches({ paths: want.paths.map((p) => p.replace('receivers.0', 'receivers.1')), values: want.values }, json, 0, 't30_s').length, 2);
});
