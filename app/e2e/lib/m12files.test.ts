import { strict as assert } from 'node:assert';
import { copyFileSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { f32, level, readCsbin, readPbin } from './m12files.ts';

const solve = path.join(import.meta.dirname, '..', '..', '..', 'tests', 'fixtures', 'results', 'outputs_spps', 'solve');

test('M12 files: the surface map fixture reads to its last byte, its header as the format says', () => {
  const c = readCsbin(path.join(solve, 'Surface receiver', '1000 Hz', 'Sound level.csbin'));
  assert.equal(c.nodes, 53);
  assert.equal(c.timeSteps, 100);
  assert.equal(c.recordType, 0);
  const records = c.faces.reduce((n, f) => n + f.records.length, 0);
  // 4456 bytes = 44 + 53 x 12 + 264 + faces x 16 + records x 8.
  assert.equal(44 + 53 * 12 + 264 + c.faces.length * 16 + records * 8, 4456);
  assert.ok(c.faces.every((f) => f.records.every(([s]) => s < 100)));
  const cut = readCsbin(path.join(solve, 'Surface receiver', '1000 Hz', 'rs_cut.csbin'));
  assert.equal(cut.nodes, 60);
});

test('M12 files: the particle fixture: 8 particles, alive counts summing to its records', () => {
  const p = readPbin(path.join(solve, 'Particles', '1000', 'particles.pbin'));
  assert.equal(p.particles, 8);
  assert.equal(p.stepsMax, 100);
  // 716 bytes = 28 + 8 x 8 + 39 x 16.
  assert.equal(p.records, 39);
  assert.equal(p.alive.reduce((a, b) => a + b, 0), 39);
  assert.ok(p.alive[0] > 0 && p.alive[0] <= 8);
});

test('M12 files: a file one byte short or one byte long is refused', () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'm12files-'));
  try {
    const src = path.join(solve, 'Particles', '1000', 'particles.pbin');
    const bytes = readFileSync(src);
    const short = path.join(dir, 'short.pbin');
    writeFileSync(short, bytes.subarray(0, bytes.length - 1));
    assert.throws(() => readPbin(short));
    const long = path.join(dir, 'long.pbin');
    writeFileSync(long, Buffer.concat([bytes, Buffer.from([0])]));
    assert.throws(() => readPbin(long));
    const csbin = path.join(dir, 'm.csbin');
    copyFileSync(path.join(solve, 'Surface receiver', '1000 Hz', 'Sound level.csbin'), csbin);
    writeFileSync(csbin, readFileSync(csbin).subarray(0, 4455));
    assert.throws(() => readCsbin(csbin));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('M12 files: f32 of bits, and the level of an energy', () => {
  assert.equal(f32(0x3f800000), 1);
  assert.equal(level(1e-12), 0);
  assert.equal(level(0), null);
});
