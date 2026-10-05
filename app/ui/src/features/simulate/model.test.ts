import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { RunRow, RunsView } from '../../bindings/ipc.ts';
import type { Environment } from '../../bindings/schema.ts';
import { joinBlockers, RUN_ACTIVE } from '../../flow.ts';
import type { ActiveRun } from '../../store.ts';
import {
  airText,
  bandsText,
  elapsedText,
  groupedInt,
  hzText,
  lastRunView,
  latestRun,
  PREFLIGHT,
  preflightRows,
  progressDisplay,
  progressFill,
  projectSettings,
  resultsCodes,
  resultsStateName,
  runLabel,
  runningHead,
  runVariantName,
  settingsRows,
  simulateSub,
  simulateSubParts,
  timeStepText,
} from './model.ts';

const active = (over: Partial<ActiveRun> = {}): ActiveRun => ({
  id: 1,
  run: '20260929-202553-613-spps',
  solver: 'spps',
  variant: null,
  stage: 'solve',
  progress: 25.22,
  progressText: '25.22',
  startedAt: 0,
  status: 'running',
  ...over,
});

const row = (number: number, status: RunRow['status'], over: Partial<RunRow> = {}): RunRow => ({
  run: `2026092${number}-000000-000-spps`,
  number,
  status,
  reasons: [],
  warnings: [],
  ...over,
});

const view = (rows: RunRow[]): RunsView => ({ root: 'C:/p/runs', rows, other_projects: 0 });

// The grammar M11 PLAN.md 4.2 (iii) holds a progress_pct span to, before its " %".
const PROGRESS_GRAMMAR = /^\d{1,3}(\.\d{1,2})?$/;

test('progress: shown as printed up to two decimals, else rounded half up in integers', () => {
  assert.equal(progressDisplay('25.22'), '25.22');
  assert.equal(progressDisplay('25.2'), '25.2');
  assert.equal(progressDisplay('7'), '7');
  assert.equal(progressDisplay('100'), '100');
  assert.equal(progressDisplay('12.50'), '12.50', 'digits as printed');
  assert.equal(progressDisplay('25.229'), '25.23');
  assert.equal(progressDisplay('25.225'), '25.23', 'an exact half goes up');
  assert.equal(progressDisplay('25.2249'), '25.22');
  assert.equal(progressDisplay('99.999'), '100.00');
  assert.equal(progressDisplay('0.06667'), '0.07');
  assert.equal(progressDisplay('0.0006667'), '0.00');
  assert.equal(progressDisplay(' 12.5 '), '12.5');
  assert.equal(progressDisplay('007.5'), '7.5');
  // A float would give 1.005 * 100 = 100.49999999999999 and round it down: integers cannot.
  assert.equal(progressDisplay('1.005'), '1.01');
});

test('progress: exponent forms are expanded by moving the point, not by a float', () => {
  assert.equal(progressDisplay('1.667e-05'), '0.00');
  assert.equal(progressDisplay('1e+02'), '100');
  assert.equal(progressDisplay('1.234e+01'), '12.34');
  assert.equal(progressDisplay('9.9999e+01'), '100.00');
  assert.equal(progressDisplay('9.9994e+01'), '99.99');
  assert.equal(progressDisplay('5E-1'), '0.5');
});

/**
 * An independent oracle, the rule m11-h proves a progress span by (e2e/lib/runs.ts
 * `roundedTo`): the decimal as an exact n / 10^k, rounded half up to d decimals.
 */
function roundedTo(text: string, d: number): bigint | null {
  const m = /^([+-]?)(\d*)(?:\.(\d*))?(?:[eE]([+-]?\d+))?$/.exec(text.trim());
  if (!m || (m[2] === '' && (m[3] ?? '') === '') || m[1] === '-') return null;
  let k = (m[3] ?? '').length - Number(m[4] ?? '0');
  let n = BigInt(`${m[2]}${m[3] ?? ''}` || '0');
  if (k < 0) {
    n *= 10n ** BigInt(-k);
    k = 0;
  }
  if (k <= d) return n * 10n ** BigInt(d - k);
  const div = 10n ** BigInt(k - d);
  return (n * 2n + div) / (2n * div);
}

test('progress: every shown value is its # line at the digits shown, as m11-h proves it', () => {
  const lines = ['0', '0.1', '0.0001234', '1.2e-3', '1.667e-05', '6.667e-05', '0.06667', '12.35', '25.225', '33.33', '50', '66.67', '99.99', '99.995', '99.999', '100', '1e+02', '3.3333', '9.9999e+01'];
  for (const line of lines) {
    const shown = progressDisplay(line);
    assert.ok(shown !== null && PROGRESS_GRAMMAR.test(shown), `${line} -> ${shown}`);
    const d = (shown.split('.')[1] ?? '').length;
    assert.equal(roundedTo(line, d), roundedTo(shown, d), `${line} shown as ${shown}`);
  }
});

test('progress: what is not a percentage of a run shows no number', () => {
  for (const t of ['', '-', 'abc', '101', '100.5', '1000', '1.01e+02', '-5', '1e+999', '12%', '.5', '100.006']) {
    assert.equal(progressDisplay(t), null, JSON.stringify(t));
  }
});

test('the Simulate sub: the percentage while running, "run n status" when idle, else empty', () => {
  assert.equal(simulateSub(null, null), '');
  assert.equal(simulateSub(null, view([])), '');
  assert.equal(simulateSub(active(), null), '25.22 %');
  assert.deepEqual(simulateSubParts(active(), null), [
    { text: '25.22 %', diagnostic: 'progress_pct', run: '20260929-202553-613-spps' },
  ]);
  assert.equal(simulateSub(active({ progressText: '', progress: null }), null), 'running');
  assert.equal(simulateSub(active({ status: 'cancelling' }), null), 'cancelling');
  const rows = view([row(1, 'FAIL'), row(3, 'OK'), row(2, 'CANCELLED')]);
  assert.equal(simulateSub(null, rows), 'run 3 OK');
  assert.equal(simulateSub(null, view([row(1, 'OK'), row(2, 'CANCELLED')])), 'run 2 Cancelled');
  assert.equal(simulateSub(null, view([row(1, 'INTERRUPTED')])), 'run 1 Interrupted');
  // A Running row is the active run's: the sub never names it as the last result.
  assert.equal(simulateSub(null, view([row(1, 'OK'), row(2, 'RUNNING')])), 'run 1 OK');
});

test('the newest run is the highest number that is not running', () => {
  assert.equal(latestRun(null), null);
  assert.equal(latestRun(view([row(2, 'RUNNING')])), null);
  assert.equal(latestRun(view([row(4, 'FAIL'), row(2, 'OK'), row(5, 'RUNNING')]))?.number, 4);
});

test('the Run label: Run SPPS or Run TCR; Running <p> % with the percentage a diagnostic part', () => {
  assert.deepEqual(runLabel(null, 'spps'), [{ text: 'Run SPPS' }]);
  assert.deepEqual(runLabel(null, 'tcr'), [{ text: 'Run TCR' }]);
  assert.deepEqual(runLabel(active(), 'tcr'), [
    { text: 'Running ' },
    { text: '25.22 %', diagnostic: 'progress_pct', run: '20260929-202553-613-spps' },
  ]);
  assert.deepEqual(runLabel(active({ progressText: '' }), 'spps'), [{ text: 'Running…' }]);
  assert.deepEqual(runLabel(active({ progressText: '-' }), 'spps'), [{ text: 'Running…' }]);
  assert.deepEqual(runLabel(active({ status: 'cancelling' }), 'spps'), [{ text: 'Cancelling…' }]);
});

test('the running block says what the run is doing, stage by stage', () => {
  const head = (over: Partial<ActiveRun>) => runningHead(active(over)).map((p) => p.text).join('');
  assert.equal(head({ status: 'starting', stage: null, run: undefined, progressText: '' }), 'Starting…');
  assert.equal(head({ stage: 'solvers', progressText: '' }), 'Checking the solvers…');
  assert.equal(head({ stage: 'validate', progressText: '' }), 'Checking the project…');
  assert.equal(head({ stage: 'mesh', progressText: '' }), 'Meshing…');
  assert.equal(head({ stage: 'pre_launch', progressText: '' }), 'Preparing the solver…');
  assert.equal(head({ stage: 'solve', progressText: '' }), 'Solving…');
  assert.equal(head({ stage: 'solve' }), 'Solving · 25.22 %');
  assert.equal(head({ status: 'cancelling' }), 'Cancelling…');
  assert.deepEqual(runningHead(active())[1], { text: '25.22 %', diagnostic: 'progress_pct', run: '20260929-202553-613-spps' });
});

test('the bar fill is the core value clamped to 0-100, and 0 before any', () => {
  assert.equal(progressFill(active({ progress: null })), 0);
  assert.equal(progressFill(active({ progress: 42.5 })), 42.5);
  assert.equal(progressFill(active({ progress: 180 })), 100);
  assert.equal(progressFill(active({ progress: -3 })), 0);
  assert.equal(progressFill(active({ progress: Number.NaN })), 0);
});

test('elapsed time is m:ss, h:mm:ss from an hour on, with no unit', () => {
  assert.equal(elapsedText(0), '0:00');
  assert.equal(elapsedText(999), '0:00');
  assert.equal(elapsedText(42_000), '0:42');
  assert.equal(elapsedText(61_500), '1:01');
  assert.equal(elapsedText(3_599_999), '59:59');
  assert.equal(elapsedText(3_600_000), '1:00:00');
  assert.equal(elapsedText(3_725_000), '1:02:05');
  assert.equal(elapsedText(-5), '0:00');
  assert.equal(elapsedText(Number.NaN), '0:00');
  // No unit a reader, or the no-acoustic-number check, could take for a computed value.
  assert.doesNotMatch(elapsedText(125_000), /\d\s*(dB|s|ms|%)(?![\p{L}\p{N}])/u);
});

const solvers = (blockers: string[]) => ({ checks: [], blockers });
const info = (surface_groups: number, groups_assigned: number) => ({ surface_groups, groups_assigned });
const okCheck = { verdict: 'ok' } as never;

test('Before running: all OK on a project that may run', () => {
  const rows = preflightRows({ blockers: [], solvers: solvers([]), check: okCheck, info: info(6, 6) });
  assert.ok(rows);
  assert.deepEqual(
    rows.map((r) => [r.key, r.state]),
    PREFLIGHT.map((r) => [r.key, 'OK']),
  );
  assert.equal(preflightRows({ blockers: null, solvers: null, check: null, info: null }), null, 'no project');
});

test('Before running: the placeholder hall fails its materials and its source, with the codes', () => {
  const rows = preflightRows({
    blockers: ['MATERIALS_UNASSIGNED', 'SOURCE_NONE'],
    solvers: solvers([]),
    check: okCheck,
    info: info(10, 0),
  });
  assert.ok(rows);
  const by = new Map(rows.map((r) => [r.key, r]));
  assert.equal(by.get('materials')?.state, 'FAIL');
  assert.deepEqual(by.get('materials')?.codes, ['MATERIALS_UNASSIGNED']);
  assert.equal(by.get('materials')?.detail, '10 of 10 surface groups have none');
  assert.equal(by.get('source')?.state, 'FAIL');
  assert.deepEqual(by.get('source')?.codes, ['SOURCE_NONE']);
  assert.equal(by.get('model')?.state, 'OK');
  assert.equal(by.get('receivers')?.state, 'OK');
  assert.equal(by.has('other'), false);
});

test('Before running: the solvers row is its own, and UNCHECKED until the status is known', () => {
  const unchecked = preflightRows({ blockers: [], solvers: null, check: okCheck, info: info(1, 1) });
  assert.equal(unchecked?.find((r) => r.key === 'solvers')?.state, 'UNCHECKED');
  const bad = preflightRows({ blockers: [], solvers: solvers(['SOLVER_UNVERIFIED']), check: okCheck, info: info(1, 1) });
  const s = bad?.find((r) => r.key === 'solvers');
  assert.deepEqual([s?.state, s?.codes], ['FAIL', ['SOLVER_UNVERIFIED']]);
});

test('Before running: no model fails the model row; RUN_ACTIVE is no row; any other blocker gets one', () => {
  const rows = preflightRows({
    blockers: ['SOURCE_NONE', 'TIME_STEP_INVALID', RUN_ACTIVE, 'PARTICLE_COUNT_INVALID'],
    solvers: solvers([]),
    check: null,
    info: info(0, 0),
  });
  assert.ok(rows);
  const by = new Map(rows.map((r) => [r.key, r]));
  assert.deepEqual([by.get('model')?.state, by.get('model')?.detail], ['FAIL', 'no model in the project']);
  assert.deepEqual(by.get('other'), {
    key: 'other',
    label: 'Every other check passes',
    state: 'FAIL',
    codes: ['TIME_STEP_INVALID', 'PARTICLE_COUNT_INVALID'],
    detail: '',
  });
  assert.equal(rows.some((r) => r.codes.includes(RUN_ACTIVE)), false);
});

test('Before running never reads all OK while the Run button is blocked: every blocker is on a FAIL row', () => {
  const cases: [string[], string[]][] = [
    [['GEOMETRY_REFUSED', 'MATERIALS_UNASSIGNED'], []],
    [['RECEIVER_OUTSIDE', 'LABEL_UNSAFE'], ['SOLVER_NOT_FOUND']],
    [['ABSATMO_INVALID', 'MESH_OUT_OF_DATE', 'SOURCE_OUTSIDE'], ['SOLVER_UNVERIFIED', 'SOLVER_NOT_FOUND']],
    [['MATERIAL_UNASSIGNED', 'BAND_SET_EMPTY', 'RECEIVER_ON_SURFACE', 'ATMOSPHERE_INVALID'], []],
  ];
  for (const [project, solverBlockers] of cases) {
    const status = solvers(solverBlockers);
    const joined = joinBlockers(project, status, true) ?? [];
    const rows = preflightRows({ blockers: project, solvers: status, check: okCheck, info: info(3, 3) }) ?? [];
    const onFail = new Set(rows.filter((r) => r.state === 'FAIL').flatMap((r) => r.codes));
    for (const b of joined) if (b !== RUN_ACTIVE) assert.ok(onFail.has(b), `${b} is on no FAIL row`);
    for (const r of rows) if (r.state === 'OK') assert.deepEqual(r.codes, [], `${r.key} OK with codes`);
  }
});

const env = (over: Partial<Environment> = {}): Environment => ({
  air_absorption: { kind: 'iso9613' },
  celerity_gradient_lin: 0,
  celerity_gradient_log: 0,
  ground_roughness_m: 0,
  pressure_pa: 101325,
  relative_humidity_percent: 50,
  temperature_c: 20,
  ...over,
});

test('settings: the frequencies, bands, time step, air and particle count as the design spells them', () => {
  assert.deepEqual([125, 250, 500, 1000, 2000, 4000, 1250, 12500, 16000, 20].map(hzText), [
    '125', '250', '500', '1k', '2k', '4k', '1.25k', '12.5k', '16k', '20',
  ]);
  const oct = { kind: 'octave' as const, frequencies_hz: [125, 250, 500, 1000, 2000, 4000] };
  assert.equal(bandsText(oct, [true, true, true, true, true, true]), '1/1 oct · 125–4k');
  assert.equal(bandsText(oct, [true, false, true, true, true, true]), '1/1 oct · 125–4k · 5 of 6 computed');
  assert.equal(bandsText({ kind: 'third_octave', frequencies_hz: [100, 125] }, [true, true]), '1/3 oct · 100–125');
  assert.equal(bandsText({ kind: 'octave', frequencies_hz: [] }, []), 'none');
  assert.equal(timeStepText(0.01), '10 ms');
  assert.equal(timeStepText(0.0035), '3.5 ms');
  assert.equal(timeStepText(0.001), '1 ms');
  assert.equal(timeStepText(1), '1 s');
  assert.equal(timeStepText('NaN'), 'NaN s');
  assert.equal(airText(env(), true), '20 °C · 50 %');
  assert.equal(airText(env({ temperature_c: 21.5 }), true), '21.5 °C · 50 %');
  assert.equal(airText(env(), false), 'off');
  assert.equal(airText(env({ air_absorption: { kind: 'user_defined', unit: 'decibel_per_metre', value: 0.01 } }), true), '0.01 dB/m');
  assert.equal(airText(env({ air_absorption: { kind: 'user_defined', unit: 'per_metre', value: 0.002 } }), true), '0.002 /m');
  assert.deepEqual([0, 7, 999, 1000, 150000, 1000000, 12345678].map(groupedInt), [
    '0', '7', '999', '1,000', '150,000', '1,000,000', '12,345,678',
  ]);
});

const projectFile = {
  format_version: 1,
  name: 'Teaching room',
  bands: { kind: 'octave', frequencies_hz: [125, 250, 500, 1000, 2000, 4000] },
  environment: env(),
  solvers: {
    meshing: {},
    spps: {
      air_absorption: true,
      bands_computed: [true, true, true, true, true, true],
      duration_s: 1.5,
      particles_per_source: 150000,
      time_step_s: 0.01,
    },
    tcr: { air_absorption: false, bands_computed: [true, true, true, true, true, true] },
  },
};

test('settings: read from the project file, SPPS and TCR each as drawn', () => {
  const s = projectSettings(JSON.stringify(projectFile));
  assert.ok(s);
  assert.deepEqual(
    settingsRows(s, 'spps').map((r) => [r.label, r.value]),
    [
      ['Particles per source and band', '150,000'],
      ['Duration', '1.5 s'],
      ['Time step', '10 ms'],
      ['Bands', '1/1 oct · 125–4k'],
      ['Air absorption', '20 °C · 50 %'],
    ],
  );
  assert.deepEqual(
    settingsRows(s, 'tcr').map((r) => [r.label, r.value]),
    [
      ['Method', 'Sabine · Eyring'],
      ['Bands', '1/1 oct · 125–4k'],
      ['Air absorption', 'off'],
    ],
  );
});

test('settings: an unreadable file gives em dashes, never a guess', () => {
  assert.equal(projectSettings('not json'), null);
  assert.equal(projectSettings('null'), null);
  assert.equal(projectSettings(JSON.stringify({ bands: projectFile.bands })), null);
  for (const solver of ['spps', 'tcr'] as const) {
    for (const r of settingsRows(null, solver)) if (r.key !== 'method') assert.equal(r.value, '—', r.key);
  }
});

test('the last run: its number, variant, status word, loss strings and solver warnings', () => {
  const variants = [{ id: 'v1', name: 'Treated rear wall' }];
  assert.equal(runVariantName(null, variants), 'Baseline');
  assert.equal(runVariantName('v1', variants), 'Treated rear wall');
  assert.equal(runVariantName('gone', variants), 'Deleted variant');
  const ok = lastRunView(
    row(3, 'OK', {
      variant: 'v1',
      loss: { worst_pct: '0.00', limit_pct: '1', worst_band_hz: 125, bands: [] },
      lines: { progress: 9999, info: 2, ok: 1, warn: 0, fail: 0, unclassified: 0 },
      reasons: [{ code: 'x', ui_code: 'X', detail: '' }],
    }),
    variants,
  );
  assert.deepEqual(
    [ok.label, ok.variant, ok.statusText, ok.loss, ok.solverWarnings, ok.reasons],
    ['Run 3', 'Treated rear wall', 'OK', { pct: '0.00', limit: '1' }, '0', []],
  );
  const reason = { code: 'tetgen_skipped_facets', ui_code: 'MESH_TETGEN_SKIPPED', detail: '' };
  const fail = lastRunView(row(2, 'FAIL', { reasons: [reason] }), variants);
  assert.deepEqual([fail.statusText, fail.loss, fail.solverWarnings, fail.reasons], ['FAIL', null, '—', [reason]]);
  const tcr = lastRunView(row(4, 'OK', { solver: 'tcr', lines: { progress: 0, info: 3, ok: 1, warn: 2, fail: 0, unclassified: 0 } }), []);
  assert.deepEqual([tcr.loss, tcr.solverWarnings], [null, '2'], 'TCR has no loss line');
  assert.equal(lastRunView(row(5, 'CANCELLED'), []).statusText, 'Cancelled');
  const w = (code: string, detail: string) => ({ code, ui_code: code.toUpperCase(), detail });
  const warned = lastRunView(row(6, 'OK', { warnings: [w('a', '1'), w('b', '2'), w('a', '3')] }), []);
  assert.deepEqual(warned.warnings, [w('a', '1'), w('b', '2')], 'each warning code once, first kept');
});

test('the Results step state: none, running, checking, verified, refused, error', () => {
  const verified = { run: 'r', verified: true };
  const refused = { run: 'r', verified: false, refusal: { code: 'results_run_failed', ui_code: 'RESULTS_RUN_FAILED', detail: '' } };
  assert.equal(resultsStateName(null, null, null, false), 'none');
  assert.equal(resultsStateName('r', { status: 'RUNNING' }, verified, false), 'running');
  assert.equal(resultsStateName('r', { status: 'OK' }, null, false), 'checking');
  assert.equal(resultsStateName('r', null, null, false), 'checking');
  assert.equal(resultsStateName('r', { status: 'OK' }, verified, false), 'verified');
  assert.equal(resultsStateName('r', { status: 'FAIL' }, refused, false), 'refused');
  assert.equal(resultsStateName('r', { status: 'OK' }, null, true), 'error');
});

// T38-7 (backlog 38, docs/investigations/2026-09-30-b38-39/PLAN.md): a run whose results load but
// whose solver build was never verified (T38-1's run, `results_state` as T38-5 pins it) is marked
// unverified with its reason code: never "Results verified", and not refused (C6).
test('t38_7 the Results step shows an unverified solver build as unverified with its reason code', () => {
  const reason = { code: 'solver_build_unrecorded', ui_code: 'SOLVER_BUILD_UNRECORDED', detail: '' };
  const unverified = { run: 'r', verified: false, refusal: null, unverified: reason };
  const state = resultsStateName('r', { status: 'OK' }, unverified, false);
  assert.notEqual(state, 'verified', 'an unverified build never reads "Results verified"');
  assert.equal(state, 'unverified', 'marked unverified, not refused (C6)');
  assert.deepEqual(resultsCodes(unverified), [reason], 'the reason code is shown');
  // A refusal is still a refusal, with its own code.
  const refusal = { code: 'results_run_failed', ui_code: 'RESULTS_RUN_FAILED', detail: '' };
  const refused = { run: 'r', verified: false, refusal, unverified: null };
  assert.equal(resultsStateName('r', { status: 'FAIL' }, refused, false), 'refused');
  assert.deepEqual(resultsCodes(refused), [refusal]);
  // A verified run shows no code.
  const verified = { run: 'r', verified: true, refusal: null, unverified: null };
  assert.equal(resultsStateName('r', { status: 'OK' }, verified, false), 'verified');
  assert.deepEqual(resultsCodes(verified), []);
});

// ---- the run-quality advisor before a run (backlog 80, T8) ------------------------------------------

import type { Advice } from '../../bindings/ipc.ts';
import { ACOUSTIC_NUMBER_RE, adviceRows } from './model.ts';

const advice = (code: string, setting: Advice['fix']['setting'], from: number | boolean | null, to: number | boolean | null): Advice => ({
  code,
  cause: `The cause of ${code}.`,
  fix: {
    words: 'Use larger receivers (Receiver radius).',
    setting,
    pointer: setting ? `/solvers/spps/${setting}` : null,
    label: setting ? 'Receiver radius' : null,
    from,
    to,
    why_no_apply: to === null ? 'why not' : null,
    bound: null,
    note: null,
  },
  values: [],
});

test('adviceRows: one row per item, with its Apply only where the core offers one', () => {
  assert.equal(adviceRows(null), null);
  assert.deepEqual(adviceRows([]), []);
  const rows = adviceRows([
    advice('mesh_splits_walls', 'preserve_boundary', false, true),
    advice('receivers_small', 'receiver_radius', 0.31, 0.6),
    advice('run_short', 'duration', 1.5, 10),
    advice('particles_few', 'particles_per_source', 10000, 50000),
    advice('receivers_small', 'particles_per_source', 100000, null),
  ])!;
  assert.deepEqual(
    rows.map((r) => [r.key, r.change, r.apply !== null]),
    [
      ['mesh_splits_walls', 'off → on', true],
      ['receivers_small', '0.31 m → 0.6 m', true],
      // A duration's change is not printed on the Simulate step (m10-h: a number next to s): the
      // value shows in its field after Apply.
      ['run_short', null, true],
      ['particles_few', '10,000 → 50,000', true],
      ['receivers_small', null, false],
    ],
  );
  assert.equal(rows[4].why, 'why not');
  assert.deepEqual(rows[1].apply, { setting: 'receiver_radius', from: 0.31, to: 0.6 });
});

test('adviceRows: no row text trips the m10-h number scanner', () => {
  const rows = adviceRows([
    advice('run_short', 'duration', 1.5, 10),
    advice('onset_too_coarse', 'time_step', 0.005, 0.001),
    advice('receivers_small', 'receiver_radius', 0.31, 0.6),
    advice('solver_floor', 'extinction_exponent', 5, 7),
  ])!;
  for (const r of rows) {
    for (const t of [r.cause, r.words, r.label, r.change, r.why, r.note]) {
      if (t) assert.ok(!ACOUSTIC_NUMBER_RE.test(t), `${r.key}: ${t}`);
    }
  }
});
