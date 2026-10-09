// emission.ts under `node --test` (C1).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  bandLevels,
  bandNumber,
  energySum,
  entryFrom,
  entryUsers,
  entryWithLevel,
  linkedTo,
  powerFor,
  powerKey,
  powerOptions,
  shapeFor,
  spectrumKey,
  spectrumOptions,
  unlinked,
  withBandLevel,
  withDirectivity,
} from './emission.ts';

const OCT = [125, 250, 500, 1000, 2000, 4000, 8000];
const near = (a: number, b: number, eps = 1e-9) => assert.ok(Math.abs(a - b) < eps, `${a} vs ${b}`);

test('band numbers are the exact third-octave steps', () => {
  assert.deepEqual([50, 63, 800, 1000, 1250, 3150, 20000].map(bandNumber), [-13, -12, -1, 0, 1, 5, 13]);
});

test('band levels sum to the global level; pink is flat, white rises 3 dB per octave', () => {
  const pink = bandLevels({ global_db: 85, shape: { kind: 'pink' } }, OCT)!;
  near(energySum(pink), 85);
  pink.forEach((l) => near(l, 85 - 10 * Math.log10(7)));
  const white = bandLevels({ global_db: 85, shape: { kind: 'white' } }, OCT)!;
  near(energySum(white), 85);
  near(white[1] - white[0], 3);
  assert.equal(bandLevels({ global_db: 85, shape: { kind: 'custom', relative_db: [0, 0] } }, OCT), null, 'a shape of another length');
});

test('a level typed in one band keeps the others and becomes the custom shape', () => {
  const before = bandLevels({ global_db: 85, shape: { kind: 'pink' } }, OCT)!;
  const p = withBandLevel({ global_db: 85, shape: { kind: 'pink' } }, OCT, 3, 90)!;
  assert.equal(p.shape.kind, 'custom');
  const after = bandLevels(p, OCT)!;
  near(after[3], 90, 1e-9);
  for (const i of [0, 1, 2, 4, 5, 6]) near(after[i], before[i], 1e-9);
  near(Number(p.global_db), energySum(after));
  assert.equal(withBandLevel({ global_db: 85, shape: { kind: 'pink' } }, OCT, 7, 90), null);
});

test('the spectrum select names a library shape, and falls back to typed', () => {
  const lib = [
    { reference_id: 1, name: 'Pink noise', shape: { kind: 'pink' as const } },
    { reference_id: 0, name: 'White noise', shape: { kind: 'white' as const } },
    { reference_id: 2, name: 'ES_VL', shape: { kind: 'custom' as const, relative_db: [1, 2, 3, 4, 5, 6, 7] } },
  ];
  assert.equal(spectrumKey({ kind: 'pink' }, lib), 'pink');
  assert.equal(spectrumKey({ kind: 'custom', relative_db: [1, 2, 3, 4, 5, 6, 7] }, lib), 'ref:2');
  assert.equal(spectrumKey({ kind: 'custom', relative_db: [1, 2, 3, 4, 5, 6, 8] }, lib), 'custom');
  assert.deepEqual(
    spectrumOptions(lib, 'pink').map((o) => o.value),
    ['pink', 'white', 'ref:2'],
  );
  assert.deepEqual(spectrumOptions(lib, 'custom').map((o) => o.value).at(-1), 'custom');
  assert.deepEqual(shapeFor('ref:2', lib), lib[2].shape);
  assert.deepEqual(shapeFor('white', lib), { kind: 'white' });
  assert.equal(shapeFor('custom', lib), null);
});

test('a unidirectional source keeps its direction; the planes and omni carry none', () => {
  assert.deepEqual(withDirectivity({ kind: 'omni' }, 'unidirectional'), { kind: 'unidirectional', direction: [1, 0, 0] });
  assert.deepEqual(withDirectivity({ kind: 'unidirectional', direction: [0, 1, 0] }, 'unidirectional'), { kind: 'unidirectional', direction: [0, 1, 0] });
  assert.deepEqual(withDirectivity({ kind: 'unidirectional', direction: [0, 1, 0] }, 'plane_xy'), { kind: 'plane_xy' });
  assert.equal(withDirectivity({ kind: 'omni' }, 'balloon'), null, 'a balloon is not offered');
});

// ---- Parity M17 and M48: the project's spectrum library ---------------------------------------

const REF = [
  { reference_id: 1, name: 'Pink noise', shape: { kind: 'pink' as const } },
  { reference_id: 2, name: 'ES_VL', shape: { kind: 'custom' as const, relative_db: [1, 2, 3, 4, 5, 6, 7] } },
];
const SPEECH = { id: 'e1', name: 'Speech', levels_db: [60, 64, 68, 70, 66, 60, 52] };

test('M17: a source saved to the library keeps its band levels; linked, it writes the same levels', () => {
  const power = { global_db: 85, shape: { kind: 'white' as const } };
  const e = entryFrom('e2', 'Mine', power, OCT)!;
  assert.equal(e.name, 'Mine');
  const was = bandLevels(power, OCT)!;
  e.levels_db.forEach((l, i) => near(Number(l), was[i]));
  const p = linkedTo(power, e);
  assert.equal(p.library, 'e2');
  assert.equal(p.global_db, 85, 'its own level kept');
  bandLevels(p, OCT)!.forEach((l, i) => near(l, was[i], 1e-9));
  assert.equal(entryFrom('x', 'x', { global_db: 85, shape: { kind: 'custom', relative_db: [0] } }, OCT), null);
});

test('M17: the select lists the reference spectra, then the library, then typed per band only while it is', () => {
  const opts = powerOptions(REF, [SPEECH], 'custom');
  assert.deepEqual(
    opts.map((o) => [o.value, o.group]),
    [
      ['pink', 'reference'],
      ['ref:2', 'reference'],
      ['lib:e1', 'library'],
      ['custom', 'typed'],
    ],
  );
  assert.equal(powerOptions(REF, [SPEECH], 'pink').length, 3);
  assert.equal(powerKey({ global_db: 80, shape: { kind: 'custom', relative_db: SPEECH.levels_db }, library: 'e1' }, REF), 'lib:e1');
  assert.equal(powerKey({ global_db: 80, shape: { kind: 'pink' } }, REF), 'pink');
});

test('M48: choosing a library entry links the source; a reference or a typed band unlinks it', () => {
  const pink = { global_db: 80, shape: { kind: 'pink' as const } };
  const linked = powerFor('lib:e1', pink, REF, [SPEECH])!;
  assert.deepEqual(linked, { global_db: 80, shape: { kind: 'custom', relative_db: SPEECH.levels_db }, library: 'e1' });
  assert.equal(powerFor('lib:e1', linked, REF, [SPEECH]), null, 'the same entry again is no change');
  assert.equal(powerFor('lib:gone', pink, REF, [SPEECH]), null);
  const back = powerFor('pink', linked, REF, [SPEECH])!;
  assert.deepEqual(back, { global_db: 80, shape: { kind: 'pink' } });
  assert.equal('library' in back, false, 'no link is left');
  const typed = withBandLevel(linked, OCT, 2, 75)!;
  assert.equal('library' in typed, false, 'a band typed by hand is no longer the entry');
  assert.deepEqual(unlinked(linked), { global_db: 80, shape: linked.shape });
});

test('M17: an entry edited in one band keeps the others; the same level is no edit', () => {
  const e = entryWithLevel(SPEECH, 1, 61.5)!;
  assert.deepEqual(e.levels_db, [60, 61.5, 68, 70, 66, 60, 52]);
  assert.equal(e.id, 'e1');
  assert.equal(entryWithLevel(SPEECH, 1, 64), null);
  assert.equal(entryWithLevel(SPEECH, 7, 1), null);
});

test('M48: the users of an entry are its linked sources and receivers', () => {
  const view = {
    sources: [
      { name: 'S1', power: { global_db: 80, shape: { kind: 'pink' as const } } },
      { name: 'S2', power: { global_db: 80, shape: { kind: 'custom' as const, relative_db: SPEECH.levels_db }, library: 'e1' } },
    ],
    point_receivers: [{ name: 'R1', background_noise: { global_db: 30, shape: { kind: 'custom' as const, relative_db: SPEECH.levels_db }, library: 'e1' } }],
  };
  assert.deepEqual(entryUsers('e1', view), ['S2', 'R1 (background noise)']);
  assert.deepEqual(entryUsers('e9', view), []);
});
