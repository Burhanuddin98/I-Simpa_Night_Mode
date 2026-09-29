import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { ReflectionLaw } from '../../bindings/schema.ts';
import { changesLaw, LAWS, lawLabel, lawOf, lawState, lawTitle, lawValue, PER_BAND, usesLaw } from './law.ts';

test("the laws are the core's seven, in ReflectionLaw::ALL order, each labelled", () => {
  // crates/simpa-core/src/schema/model.rs: Specular 0, Uniform 1, Lambert 2, W2 3, W3 4, W4 5,
  // SemiDiffuse 6; the schema's serde names.
  const all: ReflectionLaw[] = ['specular', 'uniform', 'lambert', 'w2', 'w3', 'w4', 'semi_diffuse'];
  assert.deepEqual(
    LAWS.map((o) => o.law),
    all,
  );
  assert.deepEqual(
    LAWS.map((o) => o.label),
    ['Specular', 'Uniform', 'Lambert', 'W2', 'W3', 'W4', 'Semi-diffuse'],
  );
  assert.equal(new Set(LAWS.map((o) => o.label)).size, 7);
  // The one law whose name is not what SPPS does says so.
  assert.deepEqual(
    LAWS.filter((o) => o.note).map((o) => o.law),
    ['semi_diffuse'],
  );
});

test('a single law and a per-band law read apart; the select value is the law or per_band', () => {
  const one = lawState({ reflection_law: 'lambert' });
  assert.deepEqual(one, { kind: 'all', law: 'lambert' });
  assert.equal(lawValue(one), 'lambert');
  const bands = lawState({ reflection_law: ['lambert', 'lambert', 'specular'] });
  assert.deepEqual(bands, { kind: 'per_band', laws: ['lambert', 'lambert', 'specular'] });
  assert.equal(lawValue(bands), PER_BAND);
  // Even a per-band law with one law in every band stays per band: it is stored so.
  assert.equal(lawValue(lawState({ reflection_law: ['w2', 'w2'] })), PER_BAND);
});

test('a choice maps back to a law; the per-band placeholder and junk do not', () => {
  for (const o of LAWS) assert.equal(lawOf(o.law), o.law);
  assert.equal(lawOf(PER_BAND), null);
  assert.equal(lawOf(''), null);
  assert.equal(lawOf('Lambert'), null, 'labels are not values');
});

test('the same single law is no change (no empty undo step); a per-band law always changes', () => {
  assert.equal(changesLaw({ reflection_law: 'specular' }, 'specular'), false);
  assert.equal(changesLaw({ reflection_law: 'specular' }, 'lambert'), true);
  assert.equal(changesLaw({ reflection_law: ['lambert', 'lambert'] }, 'lambert'), true);
});

test('a law is used when it is the single law, or the law of any band', () => {
  assert.equal(usesLaw({ reflection_law: 'semi_diffuse' }, 'semi_diffuse'), true);
  assert.equal(usesLaw({ reflection_law: 'specular' }, 'semi_diffuse'), false);
  assert.equal(usesLaw({ reflection_law: ['specular', 'semi_diffuse'] }, 'semi_diffuse'), true);
  assert.equal(usesLaw({ reflection_law: ['specular', 'lambert'] }, 'semi_diffuse'), false);
});

test('labels: known laws by name, an unknown spelling as itself', () => {
  assert.equal(lawLabel('semi_diffuse'), 'Semi-diffuse');
  assert.equal(lawLabel('w4'), 'W4');
  assert.equal(lawLabel('w9'), 'w9');
});

test('the tooltip names every band of a per-band law, and the note of semi-diffuse', () => {
  const t = lawTitle(lawState({ reflection_law: ['lambert', 'specular', 'w3'] }), [125, 250]);
  assert.deepEqual(t.split('\n'), [
    'Reflection law per band (from an I-Simpa project):',
    '125 Hz Lambert',
    '250 Hz Specular',
    'band 3 W3',
    'Choosing a law sets it for every band.',
  ]);
  const s = lawTitle(lawState({ reflection_law: 'semi_diffuse' }), [125]);
  assert.ok(s.startsWith('Reflection law: Semi-diffuse in every band\nSPPS has no semi-diffuse case'), s);
  assert.ok(s.includes('refused at export (EXPORT_FAILED)') && !s.includes('specularly'), s);
  assert.ok(s.endsWith('it has no effect in SPPS.'), s);
});
