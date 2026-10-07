import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { RunRow } from '../../bindings/ipc.ts';
import { emptyLog } from '../../flow.ts';
import type { ActiveRun, ConsoleLine, RunLog } from '../../store.ts';
import {
  buildMark,
  clampHeight,
  consoleBadge,
  type ConsoleItem,
  consoleItems,
  countsEqual,
  detailView,
  DOCK_DEFAULT,
  DOCK_MIN,
  type DockPlace,
  dockHeight,
  dockHeightText,
  dockTall,
  dragTo,
  escapeMax,
  exitText,
  hasManifest,
  KEY_STEP,
  keyTo,
  newestFirst,
  parseDockHeight,
  progressPct,
  runsBadge,
  shortSha,
  solverLabel,
  solversMark,
  stageLabel,
  statusTone,
  toggleMax,
  variantLabel,
  VIEW_MIN,
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
  id: 1,
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
  // A5: a run of SPPS on the GPU says so; TCR has no GPU build.
  assert.deepEqual([solverLabel('spps', true), solverLabel('tcr', true)], ['SPPS on the GPU', 'TCR']);
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
  // The reverberation time's other spellings, with no unit to give them away (M11 review F2).
  for (const t of ['Sabine 1.52', 'Eyring: 1.4', 'RT60 2', 'T60=1.9', 'reverberation time · 1.5']) {
    assert.deepEqual(detailView(t), { kind: 'withheld' }, t);
  }
  // Counts, frequencies, exit codes and file names stay.
  for (const ok of ['125 Hz: 9999', 'exit 0xC0000005: access violation', '2 of 5 expected files: a.gabe (missing)', '3 solvers', 'Sabine and Eyring, 1/1 octave']) {
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

// T38-8 (backlog 38, docs/investigations/2026-09-30-b38-39/PLAN.md): the Runs row's solver mark is
// the core's verdict as runs_list sends it (`RunRow.solver_build`, from `results::solver_build`),
// never one worked out here from the checks (C5). A run whose checks cover only TetGen, or that
// records none, is not shown as verified.
test('t38_8 the Runs row marks the solver build by the verdict runs_list sends, not by its checks', () => {
  const check = (name: string, matches: boolean) => ({ name, path: `C:\\s\\${name}`, matches });
  const reason = (code: string) => ({ code, ui_code: code.toUpperCase(), detail: '' });
  const unchecked = reason('solver_build_unchecked');
  const onlyTetgen = buildMark(
    row(A, { solvers: [check('tetgen.exe', true)], solver_build: { status: 'unverified', reason: unchecked } }),
  );
  assert.notEqual(onlyTetgen.kind, 'verified', 'checks that cover only TetGen are not a verified build');
  assert.deepEqual(onlyTetgen, { kind: 'unverified', names: ['tetgen.exe'], reason: unchecked });
  // No record: unrecorded, with the core's code.
  const unrecorded = reason('solver_build_unrecorded');
  assert.deepEqual(buildMark(row(A, { solvers: null, solver_build: { status: 'unverified', reason: unrecorded } })), {
    kind: 'unrecorded',
    names: [],
    reason: unrecorded,
  });
  // A row with no run.json that reads carries no verdict: nothing is recorded.
  assert.deepEqual(buildMark(row(A, { status: 'INTERRUPTED', solvers: null, solver_build: null })), {
    kind: 'unrecorded',
    names: [],
    reason: null,
  });
  // A failing check: unverified, naming the file that failed.
  const mismatch = reason('solver_build_mismatch');
  assert.deepEqual(
    buildMark(
      row(A, {
        solvers: [check('spps.exe', true), check('tetgen.exe', false)],
        solver_build: { status: 'unverified', reason: mismatch },
      }),
    ),
    { kind: 'unverified', names: ['tetgen.exe'], reason: mismatch },
  );
  // Verified only when the core says so.
  assert.deepEqual(
    buildMark(row(A, { solvers: [check('spps.exe', true), check('tetgen.exe', true)], solver_build: { status: 'verified' } })),
    { kind: 'verified', names: ['spps.exe', 'tetgen.exe'], reason: null },
  );
  // A run made under $SIMPA_SOLVER_MANIFEST's override: every check matches (the override
  // registers its own executables' hashes), but the core still says unverified, so the row must
  // never show 'verified' for it. `solver_manifest_override` is not `solver_build_unrecorded`, so
  // it buckets as 'unverified', not 'unrecorded', with the core's own code and text shown.
  const override_ = reason('solver_manifest_override');
  assert.deepEqual(
    buildMark(
      row(A, {
        solvers: [check('spps.exe', true), check('tetgen.exe', true)],
        solver_build: { status: 'unverified', reason: override_ },
      }),
    ),
    { kind: 'unverified', names: ['spps.exe', 'tetgen.exe'], reason: override_ },
  );
});

// ---- the dock's height (C2) -------------------------------------------------------------------------

const ROOM = 700;
const at = (height: number, max = false, folded = false): DockPlace => ({ size: { height, max }, folded });

test('c2 a dragged height is clamped: never below DOCK_MIN, never leaving the 3D view less than VIEW_MIN', () => {
  assert.equal(clampHeight(300, ROOM), 300);
  assert.equal(clampHeight(10, ROOM), DOCK_MIN);
  assert.equal(clampHeight(5_000, ROOM), ROOM - VIEW_MIN);
  // A window too short for both: the dock keeps its minimum.
  assert.equal(clampHeight(300, 100), DOCK_MIN);
  // Maximised, it takes the whole room, whatever height it will come back to.
  assert.equal(dockHeight({ height: 300, max: true }, ROOM), ROOM);
  assert.equal(dockHeight({ height: 300, max: false }, ROOM), 300);
  // The stored height outlives a smaller window: drawn clamped, kept as it was.
  assert.equal(dockHeight({ height: 650, max: false }, 500), 500 - VIEW_MIN);
});

test('c2 a drag folds below DOCK_MIN, maximises past the 3D view minimum, and keeps the start height for both', () => {
  const start = { height: 250, max: false };
  assert.deepEqual(dragTo(start, 400.4, ROOM), at(400));
  assert.deepEqual(dragTo(start, DOCK_MIN, ROOM), at(DOCK_MIN));
  assert.deepEqual(dragTo(start, DOCK_MIN - 1, ROOM), at(250, false, true));
  assert.deepEqual(dragTo(start, ROOM - VIEW_MIN, ROOM), at(ROOM - VIEW_MIN));
  assert.deepEqual(dragTo(start, ROOM - VIEW_MIN + 1, ROOM), at(250, true));
  assert.deepEqual(dragTo(start, ROOM + 50, ROOM), at(250, true));
  // Dragged down from maximised: the height under the pointer, no longer maximised.
  assert.deepEqual(dragTo({ height: 250, max: true }, 480, ROOM), at(480));
  // From maximised straight to the strip: folded, and it comes back at 250.
  assert.deepEqual(dragTo({ height: 250, max: true }, 20, ROOM), at(250, false, true));
});

test('c2 maximise toggles between the last height and full window; Escape only restores', () => {
  assert.deepEqual(toggleMax(at(320)), at(320, true));
  assert.deepEqual(toggleMax(at(320, true)), at(320));
  // A folded dock opens maximised.
  assert.deepEqual(toggleMax(at(320, false, true)), at(320, true));
  assert.deepEqual(escapeMax(at(320, true)), at(320));
  assert.equal(escapeMax(at(320)), null);
  assert.equal(escapeMax(at(320, false, true)), null);
});

test('c2 the handle\'s keys move by a step, fold, maximise and toggle, with the drag\'s limits', () => {
  assert.deepEqual(keyTo(at(300), 'ArrowUp', false, ROOM), at(300 + KEY_STEP));
  assert.deepEqual(keyTo(at(300), 'ArrowDown', true, ROOM), at(300 - 4 * KEY_STEP));
  assert.deepEqual(keyTo(at(DOCK_MIN), 'ArrowDown', false, ROOM), at(DOCK_MIN, false, true));
  assert.deepEqual(keyTo(at(ROOM - VIEW_MIN), 'ArrowUp', false, ROOM), at(ROOM - VIEW_MIN, true));
  assert.deepEqual(keyTo(at(300), 'Home', false, ROOM), at(300, false, true));
  assert.deepEqual(keyTo(at(300), 'End', false, ROOM), at(300, true));
  assert.deepEqual(keyTo(at(300), 'Enter', false, ROOM), at(300, true));
  assert.deepEqual(keyTo(at(300, true), ' ', false, ROOM), at(300));
  assert.deepEqual(keyTo(at(300, true), 'ArrowDown', false, ROOM), at(300));
  assert.deepEqual(keyTo(at(300, false, true), 'ArrowUp', false, ROOM), at(300));
  // Nothing to do: null, so the key is left to the page.
  assert.equal(keyTo(at(300, true), 'ArrowUp', false, ROOM), null);
  assert.equal(keyTo(at(300, false, true), 'ArrowDown', false, ROOM), null);
  assert.equal(keyTo(at(300, false, true), 'Home', false, ROOM), null);
  assert.equal(keyTo(at(300, true), 'End', false, ROOM), null);
  assert.equal(keyTo(at(300), 'a', false, ROOM), null);
});

test('c2 the stored height round-trips; anything else opens at the default; maximised is never stored', () => {
  assert.equal(parseDockHeight(dockHeightText(412.6)), 413);
  assert.equal(parseDockHeight(null), DOCK_DEFAULT);
  for (const bad of ['', 'abc', '-40', '12.5', '1e3', '20', '999999', '{"height":300}']) {
    assert.equal(parseDockHeight(bad), DOCK_DEFAULT, bad);
  }
  assert.equal(dockHeightText(5), String(DOCK_MIN));
  // The Acoustics tab's rows: from DOCK_TALL, or maximised; never folded.
  assert.equal(dockTall(at(439)), false);
  assert.equal(dockTall(at(440)), true);
  assert.equal(dockTall(at(250, true)), true);
  assert.equal(dockTall(at(600, true, true)), false);
});
