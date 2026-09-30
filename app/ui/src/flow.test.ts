import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { RunRow, RunStreamEvent } from './bindings/ipc.ts';
import {
  detailTitle,
  emptyLog,
  endLine,
  foldEvent,
  isReloadKey,
  joinBlockers,
  needsSavePrompt,
  progressText,
  RUN_ACTIVE,
  WITHHELD_DETAIL,
} from './flow.ts';

const solvers = (blockers: string[]) => ({ checks: [], blockers });

test('the blockers join the project, the solvers and the run slot, each once', () => {
  assert.equal(joinBlockers(null, solvers([]), false), null);
  assert.deepEqual(joinBlockers([], solvers([]), false), []);
  assert.deepEqual(joinBlockers([], null, false), []);
  assert.deepEqual(joinBlockers(['GEOMETRY_REFUSED'], solvers(['SOLVER_NOT_FOUND']), true), [
    'GEOMETRY_REFUSED',
    'SOLVER_NOT_FOUND',
    RUN_ACTIVE,
  ]);
  assert.deepEqual(joinBlockers([RUN_ACTIVE], solvers([]), true), [RUN_ACTIVE]);
});

test('the save prompt asks only when the user changed something not on disk', () => {
  const info = (dirty: boolean, path: string | null, undo_depth: number) => ({ dirty, path, undo_depth });
  assert.equal(needsSavePrompt(null), false);
  assert.equal(needsSavePrompt(info(false, 'a.simpa', 0)), false, 'clean');
  assert.equal(needsSavePrompt(info(true, 'a.simpa', 1)), true, 'an edit to a saved project');
  assert.equal(needsSavePrompt(info(true, 'a.simpa', 0)), true, 'undone past the save');
  assert.equal(needsSavePrompt(info(true, null, 0)), false, 'a new or imported project, untouched');
  assert.equal(needsSavePrompt(info(true, null, 1)), true, 'a new or imported project, edited');
});

const line = (seq: number, cls: 'PROGRESS' | 'INFO' | 'OK', solverSeq: number | null, text: string, source: 'solver' | 'mesh' = 'solver'): RunStreamEvent => ({
  kind: 'line',
  seq,
  t_ms: seq,
  source,
  stream: 'stdout',
  class: cls,
  rule: '',
  solver_seq: solverSeq,
  progress: cls === 'PROGRESS' ? 1 : null,
  continuation: false,
  text,
});

test('folding counts solver lines by class, shows all but PROGRESS, and sees gaps and repeats', () => {
  let log = emptyLog();
  const events: RunStreamEvent[] = [
    { kind: 'started', seq: 0, t_ms: 0, run: 'r', folder: 'C:/r' },
    line(1, 'INFO', null, 'TetGen says hi', 'mesh'),
    line(2, 'INFO', 0, 'SPPS version 2.2.1'),
    line(3, 'PROGRESS', 1, '#0.01'),
    line(4, 'PROGRESS', 2, '#0.02'),
    line(5, 'OK', 3, 'End of calculation.'),
  ];
  const shown: string[] = [];
  for (const e of events) {
    const f = foldEvent(log, 'r', e);
    log = f.log;
    if (f.line) shown.push(`${f.line.tag} ${f.line.source} ${f.line.text}`);
  }
  assert.deepEqual(log.counts, { PROGRESS: 2, INFO: 1, OK: 1, WARN: 0, FAIL: 0 });
  assert.deepEqual([...log.seqs].sort(), [0, 1, 2, 3]);
  assert.equal(log.dupes, 0);
  assert.equal(log.gaps, 0);
  assert.deepEqual(shown, ['INFO mesh TetGen says hi', 'INFO solver SPPS version 2.2.1', 'OK solver End of calculation.']);
  // A repeated solver line and a skipped event are both seen.
  log = foldEvent(log, 'r', line(7, 'PROGRESS', 2, '#0.02')).log;
  assert.equal(log.dupes, 1);
  assert.equal(log.gaps, 1);
});

test('progress text is what follows the #', () => {
  assert.equal(progressText('#25.22'), '25.22');
  assert.equal(progressText('#0.1501'), '0.1501');
  assert.equal(progressText('no hash'), 'no hash');
});

const row = (over: Partial<RunRow>): RunRow => ({
  run: '20260929-201500-000-spps',
  number: 3,
  status: 'OK',
  reasons: [],
  warnings: [],
  solver: 'spps',
  ...over,
});

test("a run's end line names its status and keeps its numbers in diagnostic parts", () => {
  const ok = endLine(
    row({ loss: { worst_pct: '0.00', worst_band_hz: 125, limit_pct: '1', bands: [] } }),
  );
  assert.equal(ok.tag, 'OK');
  assert.equal(ok.text, 'Run #3 · SPPS finished · OK · Particles lost 0.00 % (limit 1 %)');
  const diag = (ok.parts ?? []).filter((p) => 'diagnostic' in p);
  assert.deepEqual(
    diag.map((p) => [p.text, 'diagnostic' in p ? p.diagnostic : '']),
    [
      ['0.00 %', 'loss_pct'],
      ['1 %', 'loss_limit_pct'],
    ],
  );
  const fail = endLine(
    row({ status: 'FAIL', reasons: [{ code: 'tetgen_skipped_facets', ui_code: 'MESH_TETGEN_SKIPPED', detail: '' }] }),
  );
  assert.equal(fail.tag, 'FAIL');
  assert.ok(fail.text.endsWith('FAIL · MESH_TETGEN_SKIPPED (tetgen_skipped_facets)'), fail.text);
  const cancelled = endLine(row({ status: 'CANCELLED', reasons: [{ code: 'cancelled', ui_code: 'CANCELLED', detail: '' }] }));
  assert.equal(cancelled.tag, 'INFO');
  assert.ok(cancelled.text.includes('· Cancelled ·'), cancelled.text);
  assert.equal(endLine(row({ status: 'CRASH' })).tag, 'FAIL');
});

test("the webview's reload keys are known, and ordinary keys are not (review 2, app 2)", () => {
  const k = (key: string, mods: { ctrlKey?: boolean; metaKey?: boolean; altKey?: boolean } = {}) => ({
    key,
    ctrlKey: mods.ctrlKey ?? false,
    metaKey: mods.metaKey ?? false,
    altKey: mods.altKey ?? false,
  });
  for (const e of [k('F5'), k('F5', { ctrlKey: true }), k('r', { ctrlKey: true }), k('R', { ctrlKey: true }), k('r', { metaKey: true })]) {
    assert.equal(isReloadKey(e), true, JSON.stringify(e));
  }
  for (const e of [k('r'), k('R'), k('F4'), k('s', { ctrlKey: true }), k('F5', { altKey: true }), k('Home')]) {
    assert.equal(isReloadKey(e), false, JSON.stringify(e));
  }
});

test("a tooltip carries a reason's detail only when the row could show it (review 2, app 4)", () => {
  // The verdict's particle_loss_excess detail, which the Runs row withholds.
  const loss = '1 band(s) lost more than 1 % of their particles to loops and meshing: 500 Hz: 1600 of 150000 (1.0667 %)';
  assert.equal(detailTitle(loss), WITHHELD_DETAIL);
  assert.equal(detailTitle('the solver reported T30 = 1.52'), WITHHELD_DETAIL);
  assert.equal(detailTitle('no run.json'), 'no run.json');
  assert.equal(detailTitle('  '), undefined);
  assert.equal(detailTitle(null), undefined);
  assert.ok(!/\d/.test(WITHHELD_DETAIL), 'the explanation holds no digit');
});
