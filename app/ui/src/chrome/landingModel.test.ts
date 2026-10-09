// node --test suite for the landing page's model (landingModel.ts): every example card says only what
// its shipped file holds, the cards and the core's list (src-tauri/src/examples.rs) name the same
// rooms in the same order, and no string on the page is a number with an acoustic unit.
import { strict as assert } from 'node:assert';
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import type { SceneState } from '../bindings/ipc.ts';
import { counted, EXAMPLES, EXAMPLES_HEAD, EXAMPLES_NOTE, exampleLine, LANDING_LEAD, LANDING_TITLE, landingShown, listed, TUTORIALS_HEAD, TUTORIALS_NOTE } from './landingModel.ts';

const TAURI = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', '..', '..', 'src-tauri');
const SHIPPED = path.join(TAURI, 'examples');

/** The self-test's rule (selftest.ts, `ACOUSTIC`): a number next to an acoustic unit. */
const ACOUSTIC = /\d(?:[\d.,]*\d)?\s*(?:dB|Hz|kHz|ms|s|%|m²|m³|m|°C)(?![\p{L}\p{N}])/gu;

interface Simpa {
  sources: unknown[];
  point_receivers: unknown[];
  surface_receivers: unknown[];
  variants: unknown[];
}

test('landing: each card counts what its shipped file holds', () => {
  for (const e of EXAMPLES) {
    const p = JSON.parse(readFileSync(path.join(SHIPPED, e.file), 'utf8')) as Simpa;
    assert.deepEqual(
      [e.sources, e.receivers, e.planes, e.variants],
      [p.sources.length, p.point_receivers.length, p.surface_receivers.length, p.variants.length],
      e.id,
    );
  }
});

test('landing: the cards and examples.rs name the same rooms, files and order; every shipped file is offered', () => {
  const rs = readFileSync(path.join(TAURI, 'src', 'examples.rs'), 'utf8');
  const rows = [...rs.matchAll(/id: "([^"]+)",\s*file_stem: "[^"]+",\s*bytes: include_bytes!\("\.\.\/examples\/([^"]+)"\)/g)].map((m) => [m[1], m[2]]);
  assert.deepEqual(rows, EXAMPLES.map((e) => [e.id, e.file]));
  const files = readdirSync(SHIPPED).filter((f) => f.endsWith('.simpa')).sort();
  assert.deepEqual(files, EXAMPLES.map((e) => e.file).sort());
});

test('landing: the lines read as plain sentences', () => {
  assert.deepEqual(
    EXAMPLES.map(exampleLine),
    [
      'A concert hall, I-Simpa’s own tutorial room: 3 sources, 6 receivers, 2 sound-level planes and every surface’s material — ready to run.',
      'A factory hall, I-Simpa’s third tutorial room, with two milling machines among the machinery: 6 sources, 5 receivers, a sound-level plane and every surface’s material — ready to run.',
      'Coupled rooms from the BRAS benchmark, a laboratory opening onto a reverberation chamber: 2 sources, 2 receivers and every surface’s material — ready to run.',
      'A seminar room from the BRAS benchmark: 2 sources, 5 receivers and every surface’s material — ready to run.',
      'A chamber music hall from the BRAS benchmark: 2 sources, 5 receivers and every surface’s material — ready to run.',
      'An auditorium from the BRAS benchmark: 2 sources, 5 receivers, a sound-level plane, every surface’s material and one variant set — ready to run.',
      'I-Simpa’s first tutorial, a box-shaped classroom for TCR and SPPS: 1 source, 2 receivers, a sound-level plane and every surface’s material — ready to run.',
      'I-Simpa’s second tutorial, the Elmia hall of the second Round Robin, at the tutorial’s SPPS settings: 3 sources, 6 receivers, 2 sound-level planes and every surface’s material — ready to run.',
      'I-Simpa’s third tutorial, a factory hall with two machines, fitting zones and walls that transmit: 6 sources, 5 receivers, a sound-level plane and every surface’s material — ready to run.',
    ],
  );
  assert.equal(counted(1, 'source', 'sources'), '1 source');
  assert.equal(counted(1, 'plane', 'planes', true), 'a plane');
  assert.equal(counted(0, 'plane', 'planes'), '0 planes');
  assert.equal(listed(['a']), 'a');
  assert.equal(listed(['a', 'b', 'c']), 'a, b and c');
});

test('landing: no string on the page is a number with an acoustic unit (no_acoustic_numbers)', () => {
  const shown = [LANDING_TITLE, LANDING_LEAD, EXAMPLES_HEAD, EXAMPLES_NOTE, TUTORIALS_HEAD, TUTORIALS_NOTE, ...EXAMPLES.flatMap((e) => [e.name, exampleLine(e)])];
  for (const s of shown) assert.deepEqual(s.match(ACOUSTIC), null, s);
  // Say-NO: the same check finds a number with a unit.
  assert.ok('Reverberation 1.2 s'.match(ACOUSTIC));
  assert.ok('a plane 1.6 m above the floor'.match(ACOUSTIC));
});

test('landing (A43): the tutorials are the last three cards, numbered 1 to 3, under their own heading', () => {
  assert.deepEqual(EXAMPLES.filter((e) => e.tutorial).map((e) => [e.id, e.tutorial]), [['tutorial-1', 1], ['tutorial-2', 2], ['tutorial-3', 3]]);
  assert.deepEqual(EXAMPLES.slice(-3).map((e) => e.tutorial), [1, 2, 3]);
  assert.equal(TUTORIALS_HEAD, 'I-Simpa tutorials');
});

test('landing: shown exactly while no project is open', () => {
  assert.equal(landingShown(null), true);
  assert.equal(landingShown({ info: { name: 'x' } } as unknown as SceneState), false);
});
