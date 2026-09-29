import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { RunRow } from '../../bindings/ipc.ts';
import { emptyLog } from '../../flow.ts';
import type { ActiveRun, ConsoleLine, RunLog } from '../../store.ts';
import {
  consoleBadge,
  type ConsoleItem,
  consoleItems,
  countsEqual,
  detailView,
  exitText,
  hasManifest,
  newestFirst,
  progressPct,
  runsBadge,
  shortSha,
  solverLabel,
  solversMark,
  stageLabel,
  statusTone,
  variantLabel,
  warningsText,
} from './model.ts';

const A = '20260929-202553-613-spps';
const B = '20260929-203000-000-spps';

const line = (tag: ConsoleLine['tag'], text: string, extra: Partial<ConsoleLine> = {}): ConsoleLine => ({
  time: '20:00:00',
  tag,
  text,
  ...extra,
});

function log(counts: Partial<RunLog['counts']> = {}, extra: Partial<RunLog> = {}): RunLog {
  const l = emptyLog();
  return { ...l, counts: { ...l.counts, ...counts }, ...extra };
}

function row(run: string, over: Partial<RunRow> = {}): RunRow {
  return {
    run,
    number: 1,
    status: 'OK',
    reasons: [],
    warnings: [],
    solver: 'spps',
    lines: { progress: 3, info: 2, ok: 1, warn: 0, fail: 0, unclassified: 0 },
    ...over,
  };
}

const active = (over: Partial<ActiveRun> = {}): ActiveRun => ({
  run: B,
  solver: 'spps',
  variant: null,
  stage: 'solve',
  progress: 25.22,
  progressText: '25.22',
  startedAt: 0,
  status: 'running',
  ...over,
});

const kinds = (items: ConsoleItem[]) =>
  items.map((i) => (i.kind === 'line' ? `line:${i.line.text}` : `${i.kind}:${i.run}`));

test('a run with lines gets its counts strip right after its last line, checked against run.json', () => {
  const lines = [
    line('INFO', 'opened'),
    line('INFO', 'SPPS banner', { run: A, source: 'solver', verbatim: true }),
    line('OK', 'end of calculation', { run: A, source: 'solver', verbatim: true }),
    line('OK', 'Run #1 finished', { run: A, source: 'app' }),
    line('INFO', 'later'),
  ];
  const logs = new Map([[A, log({ PROGRESS: 3, INFO: 2, OK: 1 })]]);
  const items = consoleItems(lines, logs, null, [row(A)]);
  assert.deepEqual(kinds(items), [
    'line:opened',
    'line:SPPS banner',
    'line:end of calculation',
    'line:Run #1 finished',
    `counts:${A}`,
    'line:later',
  ]);
  const strip = items[4];
  assert.equal(strip.kind, 'counts');
  if (strip.kind !== 'counts') return;
  assert.equal(strip.label, 'Run #1');
  assert.deepEqual(strip.counts, { PROGRESS: 3, INFO: 2, OK: 1, WARN: 0, FAIL: 0 });
  assert.deepEqual(strip.check, { match: true, manifest: { PROGRESS: 3, INFO: 2, OK: 1, WARN: 0, FAIL: 0 } });
});

test('counts that differ from run.json are flagged, and a run not yet listed is named by its folder', () => {
  const lines = [line('OK', 'end', { run: A, source: 'solver', verbatim: true })];
  const logs = new Map([[A, log({ PROGRESS: 2, INFO: 2, OK: 1 })]]);
  const flagged = consoleItems(lines, logs, null, [row(A)])[1];
  assert.equal(flagged.kind, 'counts');
  if (flagged.kind === 'counts') assert.equal(flagged.check?.match, false);
  const unlisted = consoleItems(lines, logs, null, null)[1];
  assert.equal(unlisted.kind, 'counts');
  if (unlisted.kind === 'counts') {
    assert.equal(unlisted.label, A);
    assert.equal(unlisted.check, null);
  }
});

test('the active run has one live PROGRESS line, verbatim, and its counts at the bottom, never checked', () => {
  const lines = [
    line('INFO', 'banner', { run: B, source: 'solver', verbatim: true }),
    line('INFO', 'a run is active: cancel it first'),
  ];
  const logs = new Map([[B, log({ PROGRESS: 2522, INFO: 1 })]]);
  const items = consoleItems(lines, logs, active(), [row(B, { status: 'RUNNING', lines: null })]);
  assert.deepEqual(kinds(items), ['line:banner', 'line:a run is active: cancel it first', `live:${B}`, `counts:${B}`]);
  const live = items[2];
  assert.equal(live.kind, 'live');
  if (live.kind === 'live') {
    assert.equal(live.verbatim, '#25.22');
    assert.equal(live.solver, 'SPPS');
    assert.equal(live.cancelling, false);
  }
  const strip = items[3];
  if (strip.kind === 'counts') assert.equal(strip.check, null);
  // Thousands of PROGRESS lines stay one item: the Console renders counts, not lines.
  assert.equal(items.filter((i) => i.kind === 'live').length, 1);
});

test('before its first # line the live line shows the stage, and a run without its folder yet has none', () => {
  const meshing = consoleItems([], new Map([[B, log()]]), active({ stage: 'mesh', progress: null, progressText: '' }), null);
  const live = meshing.find((i) => i.kind === 'live');
  assert.ok(live && live.kind === 'live');
  assert.equal(live.verbatim, '');
  assert.equal(stageLabel(live.stage), 'Meshing');
  assert.deepEqual(kinds(consoleItems([], new Map(), active({ run: undefined, status: 'starting' }), null)), []);
  const cancelling = consoleItems([], new Map([[B, log()]]), active({ status: 'cancelling' }), null)[0];
  assert.ok(cancelling.kind === 'live' && cancelling.cancelling);
});

test('a run with a log but no Console line gets its strip at the end; stream losses are carried', () => {
  const logs = new Map([
    [A, log({ PROGRESS: 1 }, { gaps: 1, dupes: 2 })],
    [B, log()],
  ]);
  const items = consoleItems([line('INFO', 'x')], logs, null, null);
  assert.deepEqual(kinds(items), ['line:x', `counts:${A}`, `counts:${B}`]);
  const a = items[1];
  if (a.kind === 'counts') assert.deepEqual([a.gaps, a.dupes], [1, 2]);
});

test('the Console badge reads live during a run, else its FAIL count, else nothing', () => {
  const lines = [line('FAIL', 'a'), line('INFO', 'b'), line('FAIL', 'c')];
  assert.deepEqual(consoleBadge(lines, true), { kind: 'live', text: 'live' });
  assert.deepEqual(consoleBadge(lines, false), { kind: 'fail', text: '2 fail' });
  assert.equal(consoleBadge([line('OK', 'fine')], false), null);
  assert.equal(runsBadge(null), null);
  assert.equal(runsBadge({ rows: [1, 2, 3] }), '3');
  assert.equal(runsBadge({ rows: [] }), '0');
});

test('a row names its variant, solver and status in words, and says what it does not know', () => {
  const variants = [{ id: 'v1', name: 'Treated rear wall' }];
  assert.equal(variantLabel(row(A), variants, null), 'Baseline');
  assert.equal(variantLabel(row(A, { variant: 'v1' }), variants, null), 'Treated rear wall');
  assert.equal(variantLabel(row(A, { variant: 'gone' }), variants, null), 'unknown variant');
  assert.equal(variantLabel(row(A, { status: 'INTERRUPTED', variant: null }), variants, null), '—');
  assert.equal(variantLabel(row(B, { status: 'RUNNING' }), variants, active({ variant: 'v1' })), 'Treated rear wall');
  assert.equal(variantLabel(row(A, { status: 'FAIL', manifest_error: 'bad' }), variants, null), '—');
  assert.deepEqual([solverLabel('spps'), solverLabel('tcr'), solverLabel(null)], ['SPPS', 'TCR', '—']);
  assert.deepEqual(
    (['OK', 'FAIL', 'CRASH', 'CANCELLED', 'INTERRUPTED', 'RUNNING'] as const).map(statusTone),
    ['ok', 'fail', 'fail', 'muted', 'warn', 'live'],
  );
  assert.deepEqual([warningsText(0), warningsText(1), warningsText(2)], ['0 warnings', '1 warning', '2 warnings']);
  assert.equal(hasManifest(row(A)), true);
  assert.equal(hasManifest(row(A, { status: 'RUNNING' })), false);
  assert.equal(hasManifest(row(A, { status: 'INTERRUPTED' })), false);
  assert.deepEqual(
    newestFirst([row(A, { number: 1 }), row(B, { number: 2 })]).map((r) => r.number),
    [2, 1],
  );
});

test('hashes, exit codes and the solver check read as the record holds them', () => {
  const sha = '3f2a9c1b04de5566778899aabbccddeeff00112233445566778899aabbccddee';
  assert.equal(shortSha(sha), '3f2a9c1b04de');
  assert.equal(exitText(null), 'no exit code');
  assert.equal(exitText(0), 'exit 0');
  assert.equal(exitText(3221225477), 'exit 3221225477 (0xC0000005)');
  const check = (name: string, matches: boolean) => ({ name, path: `C:\\s\\${name}`, matches });
  assert.deepEqual(solversMark(null), { kind: 'unrecorded', names: [] });
  assert.deepEqual(solversMark([]), { kind: 'unrecorded', names: [] });
  assert.deepEqual(solversMark([check('spps.exe', true), check('tetgen.exe', true)]), {
    kind: 'verified',
    names: ['spps.exe', 'tetgen.exe'],
  });
  assert.deepEqual(solversMark([check('spps.exe', true), check('tetgen.exe', false)]), {
    kind: 'unverified',
    names: ['tetgen.exe'],
  });
});

test('a core detail that quotes a number with a unit, or a parameter with a number, is not shown', () => {
  assert.deepEqual(detailView('no run.json'), { kind: 'shown', text: 'no run.json' });
  assert.deepEqual(detailView('  '), { kind: 'none' });
  assert.deepEqual(detailView(null), { kind: 'none' });
  // particle_loss_excess's own wording (run/verdict.rs).
  assert.deepEqual(
    detailView('1 band(s) lost more than 1 % of their particles to loops and meshing: 2000 Hz: 3 of 150 (2.0000 %)'),
    { kind: 'withheld' },
  );
  assert.deepEqual(detailView('solver ran 12.5 s'), { kind: 'withheld' });
  assert.deepEqual(detailView('level 85 dB'), { kind: 'withheld' });
  assert.deepEqual(detailView('STI 0.62'), { kind: 'withheld' });
  assert.deepEqual(detailView('T30: 1.8'), { kind: 'withheld' });
  // Counts, frequencies, exit codes and file names stay.
  for (const ok of ['125 Hz: 9999', 'exit 0xC0000005: access violation', '2 of 5 expected files: a.gabe (missing)', '3 solvers']) {
    assert.equal(detailView(ok).kind, 'shown', ok);
  }
});

test("a progress diagnostic shows SPPS's percentage as printed, or rounded half up to two decimals", () => {
  // As printed: at most two decimals.
  for (const t of ['25.22', '100', '0.1', '0', '99.9']) assert.equal(progressPct(t), t);
  // Four significant digits below 10 % (std::cout.precision(4)): rounded, in integers.
  assert.equal(progressPct('1.234'), '1.23');
  assert.equal(progressPct('1.235'), '1.24');
  assert.equal(progressPct('9.995'), '10.00');
  assert.equal(progressPct('0.06667'), '0.07');
  assert.equal(progressPct('0.005'), '0.01');
  assert.equal(progressPct('0.004999'), '0.00');
  assert.equal(progressPct('1e-05'), '0.00');
  assert.equal(progressPct('1.5e+01'), '15.00');
  assert.equal(progressPct('1e+02'), '100.00');
  for (const bad of ['', '.', '-1', 'abc', '1.2.3']) assert.equal(progressPct(bad), null, bad);
  // Every output fits the field's grammar (PLAN.md 4.2 (iii)).
  for (const t of ['25.22', '1.234', '0.06667', '1e-05', '9.995', '100']) {
    assert.match(`${progressPct(t)} %`, /^\d{1,3}(\.\d{1,2})? %$/);
  }
});

test('counts compare class by class', () => {
  const a = { PROGRESS: 1, INFO: 2, OK: 3, WARN: 4, FAIL: 5 };
  assert.equal(countsEqual(a, { ...a }), true);
  assert.equal(countsEqual(a, { ...a, WARN: 0 }), false);
});
