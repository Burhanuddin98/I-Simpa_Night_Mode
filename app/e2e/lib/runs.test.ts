// node --test suite for runs.ts: the integer rules of PLAN.md 2.3, on the values runs.rs's own
// unit test (`the_loss_and_time_rules_round_half_up_in_integers`) uses, and on exact halves where
// a float rounding would differ.
import { strict as assert } from 'node:assert';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import {
  bandLosses,
  decimalAbove,
  elapsedS,
  endedCalculation,
  limitPct,
  logLines,
  lossHundredths,
  lossPct,
  type Manifest,
  parseDecimal,
  progressValues,
  roundedTo,
  runFolders,
  sourceLines,
  worstLoss,
} from './runs.ts';

test('a band loss rounds half up in integers', () => {
  assert.equal(lossPct(0n, 150_000n), '0.00');
  assert.equal(lossPct(1n, 10_000n), '0.01');
  // 1 of 800 is 0.125 %: exactly half a hundredth, rounded up.
  assert.equal(lossHundredths(1n, 800n), 13n);
  assert.equal(lossPct(1n, 800n), '0.13');
  // 1 of 1,600 is 0.0625 %: below half, down.
  assert.equal(lossPct(1n, 1_600n), '0.06');
  // 1 of 400,000 is 0.00025 %: half of 0.0005, still 0.00.
  assert.equal(lossPct(1n, 400_000n), '0.00');
  // 1 of 20,000 is exactly 0.005 %: half a hundredth, up to 0.01.
  assert.equal(lossPct(1n, 20_000n), '0.01');
  assert.equal(lossPct(3n, 7n), '42.86');
  assert.equal(lossPct(7n, 7n), '100.00');
  assert.equal(lossPct(0n, 0n), '0.00');
  // Far past a double's 53 bits: still exact.
  assert.equal(lossPct(2n ** 60n, 2n ** 61n), '50.00');
});

test('the limit prints as its shortest decimal, as Rust does', () => {
  assert.equal(limitPct(0.01), '1');
  assert.equal(limitPct(0.005), '0.5');
  assert.equal(limitPct(0.05), '5');
  assert.equal(limitPct(0), '0');
  assert.equal(limitPct('inf'), 'inf');
  assert.equal(limitPct('NaN'), 'NaN');
});

test('elapsed time is floor(ms / 100 + 0.5) / 10', () => {
  assert.equal(elapsedS(1_405), '1.4');
  assert.equal(elapsedS(1_450), '1.5');
  assert.equal(elapsedS(1_449.999), '1.4');
  assert.equal(elapsedS(64_269), '64.3');
  assert.equal(elapsedS(0), '0.0');
  assert.equal(elapsedS(49.9), '0.0');
  assert.equal(elapsedS(50), '0.1');
  assert.equal(elapsedS(-5), '0.0');
  assert.equal(elapsedS(Number.NaN), '0.0');
  assert.equal(elapsedS(3_600_000), '3600.0');
});

const manifest = (bands: [number, number, number, number][]): Manifest =>
  ({
    particles: {
      bands: bands.map(([freq_hz, loops, meshing, total]) => ({
        freq_hz,
        lost_by_infinite_loops: loops,
        lost_by_meshing_problems: meshing,
        total,
      })),
    },
  }) as unknown as Manifest;

test('the worst band is the largest loss, the first on a tie', () => {
  const m = manifest([
    [125, 0, 0, 150_000],
    [250, 1, 0, 150_000],
    [500, 0, 1, 150_000],
    [1000, 0, 0, 150_000],
  ]);
  assert.deepEqual(
    bandLosses(m).map((b) => b.pct),
    ['0.00', '0.00', '0.00', '0.00'],
  );
  // All four round to 0.00: the first band is the worst on the tie of hundredths.
  assert.equal(worstLoss(m)?.freq_hz, 125);
  const w = worstLoss(manifest([
    [125, 0, 0, 800],
    [250, 1, 0, 800],
    [500, 0, 1, 800],
  ]));
  assert.equal(w?.freq_hz, 250);
  assert.equal(w?.pct, '0.13');
  assert.equal(worstLoss({ particles: null } as unknown as Manifest), null);
});

test('decimals parse exactly and round half up at the displayed digits', () => {
  assert.deepEqual(parseDecimal('25.22'), { n: 2522n, k: 2 });
  assert.deepEqual(parseDecimal('100'), { n: 100n, k: 0 });
  assert.deepEqual(parseDecimal('1.667e-05'), { n: 1667n, k: 8 });
  assert.deepEqual(parseDecimal('1e2'), { n: 100n, k: 0 });
  assert.equal(parseDecimal('abc'), null);
  assert.equal(parseDecimal('.'), null);
  assert.equal(roundedTo('25.22', 2), 2522n);
  assert.equal(roundedTo('25.225', 2), 2523n);
  assert.equal(roundedTo('25.2', 2), 2520n);
  assert.equal(roundedTo('99.99', 0), 100n);
  assert.equal(roundedTo('1.667e-05', 2), 0n);
});

test('one progress figure is above another, compared exactly', () => {
  assert.equal(decimalAbove('0.11', '0.01'), true);
  assert.equal(decimalAbove('0.1', '0.09'), true);
  assert.equal(decimalAbove('1.667e-05', '0.0006667'), false);
  assert.equal(decimalAbove('0.0006667', '1.667e-05'), true);
  // Equal values, however written, are not above: a frozen readout fails.
  assert.equal(decimalAbove('0.01', '0.01'), false);
  assert.equal(decimalAbove('0.10', '0.1'), false);
  assert.equal(decimalAbove('x', '0.1'), null);
});

test('logs read as the core streams them, from a run folder', (t) => {
  const dir = mkdtempSync(path.join(tmpdir(), 'm11-runs-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const run = path.join(dir, '20260929-120000-000-spps');
  mkdirSync(path.join(run, 'mesh', 'diag'), { recursive: true });
  mkdirSync(path.join(dir, '20260929-120000-000-spps-2'));
  mkdirSync(path.join(dir, '20260929-110000-000-tcr'));
  mkdirSync(path.join(dir, 'not-a-run'));
  writeFileSync(path.join(run, 'solver.stdout.txt'), 'SPPS version 1.4.0\r\n#0.01\r\n#25.22\r\nEnd of calculation.\r\n');
  writeFileSync(path.join(run, 'solver.stderr.txt'), 'Warning 1 particles has been in error on 800 particles.');
  writeFileSync(path.join(run, 'mesh', 'tetgen.stdout.txt'), 'Opening scene_mesh.poly.\n');
  writeFileSync(path.join(run, 'mesh', 'diag', 'tetgen.stdout.txt'), 'Detecting self-intersections.\n');
  writeFileSync(path.join(run, 'mesh', 'scene_mesh.poly'), 'not a log\n');
  assert.deepEqual(runFolders(dir), ['20260929-110000-000-tcr', '20260929-120000-000-spps', '20260929-120000-000-spps-2']);
  assert.deepEqual(runFolders(path.join(dir, 'absent')), []);
  assert.deepEqual(logLines(path.join(run, 'solver.stdout.txt')), ['SPPS version 1.4.0', '#0.01', '#25.22', 'End of calculation.']);
  assert.deepEqual(sourceLines(run, 'solver').slice(-1), ['Warning 1 particles has been in error on 800 particles.']);
  assert.deepEqual(sourceLines(run, 'mesh').sort(), ['Detecting self-intersections.', 'Opening scene_mesh.poly.']);
  assert.deepEqual(progressValues(run), ['0.01', '25.22']);
  assert.equal(endedCalculation(run), true);
});
