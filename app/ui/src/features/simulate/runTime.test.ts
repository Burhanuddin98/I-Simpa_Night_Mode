import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { RunRow, RunsView, SceneState } from '../../bindings/ipc.ts';
import { ACOUSTIC_NUMBER_RE, type ProjectSettings } from './model.ts';
import {
  clockText,
  durationWords,
  forecastRunTime,
  liveFinishMs,
  measuredRun,
  runTimeText,
  solverSentence,
  workNow,
  workUnits,
} from './runTime.ts';

// CR4 as the GUI audit ran it: 300,000 particles a source, 2 sources, 6 octave bands, 10 s; a CPU run of 2:48.
const WORK = { particles_per_source: 300_000, sources: 2, bands: 6, duration_s: 10 };

function row(number: number, extra: Partial<RunRow> = {}): RunRow {
  return {
    run: `run-${number}`,
    number,
    status: 'OK',
    solver: 'spps',
    reasons: [],
    warnings: [],
    elapsed_s: '168.0',
    work: WORK,
    ...extra,
  } as RunRow;
}

const runs = (...rows: RunRow[]): RunsView => ({ root: 'r', rows, other_projects: 0 });

function project(particles: number, bands = 6, duration: number | string = 10, sources = 2): [SceneState, ProjectSettings] {
  const sc = { view: { sources: Array.from({ length: sources + 1 }, (_, i) => ({ enabled: i < sources })) } } as unknown as SceneState;
  const spps = { particles_per_source: particles, duration_s: duration, bands_computed: Array.from({ length: 8 }, (_, i) => i < bands) };
  return [sc, { bands: {}, environment: {}, solvers: { spps, tcr: {} } } as unknown as ProjectSettings];
}

// m10-h's PARAMETER_NUMBER (app/e2e/lib/dom.ts), copied: a parameter's name and a number.
const PARAMETER_NUMBER =
  /(?:\b(?:T15|T20|T30|T60|RT60|EDT|RT|C50|C80|D50|Ts|STI|SPL|LF|LFC|G)\b|\b(?:[Ss]abine|[Ee]yring|[Rr]everberation time)\b)\s*[:=·]?\s*[-+]?\d/;

test('the forecast is the measured run scaled by particles: twice the particles, twice the 2:48', () => {
  const m = measuredRun(runs(row(1)), 'spps', 'cpu');
  assert.ok(m && m.number === 1 && m.seconds === 168);
  const [sc, st] = project(600_000);
  const f = forecastRunTime(m, 'spps', workNow(sc, st));
  assert.equal(f?.seconds, 336);
  const t = runTimeText(f, 'spps', 'cpu', new Date(2026, 9, 9, 4, 0, 0).getTime(), true);
  assert.equal(t.forecast, 'about 6 min');
  assert.equal(t.finish, 'done about 04:06 if it starts now');
  assert.equal(
    t.basis,
    'Measured on your run at 300,000 particles: Run 1 took 2:48 (m:ss) with 2 sources and 6 bands, scaled by particles × sources × bands × duration. Meshing comes on top.',
  );
});

test('each factor scales it linearly: sources, bands, duration', () => {
  const m = measuredRun(runs(row(1)), 'spps', 'cpu')!;
  const at = (p: number, b: number, d: number, s: number) => forecastRunTime(m, 'spps', workNow(...project(p, b, d, s)))!.seconds;
  assert.equal(at(300_000, 6, 10, 2), 168, 'the same settings: the measured time');
  assert.equal(at(300_000, 3, 10, 2), 84, 'half the bands');
  assert.equal(at(300_000, 6, 20, 2), 336, 'twice the duration');
  assert.equal(at(300_000, 6, 10, 1), 84, 'one source of two');
  assert.equal(at(150_000, 6, 10, 2), 84, 'half the particles');
  assert.equal(workUnits({ particlesPerSource: 300_000, sources: 2, bands: 6, durationS: 10 }), 36_000_000);
});

test('the particles as typed override the stored count', () => {
  const [sc, st] = project(300_000);
  assert.equal(workNow(sc, st, 1_000_000)?.particlesPerSource, 1_000_000);
  assert.equal(workNow(sc, st)?.particlesPerSource, 300_000);
  assert.equal(workNow(...project(300_000, 6, 'NaN')), null, 'a non-finite duration: no work, no forecast');
  assert.equal(workNow(...project(0)), null, 'no particles: no work');
  assert.equal(workNow(null, st), null);
});

test('no previous run: no number, only that there is no measurement yet', () => {
  for (const rs of [null, runs(), runs(row(1, { status: 'FAIL' })), runs(row(1, { work: null })), runs(row(1, { elapsed_s: null }))]) {
    assert.equal(measuredRun(rs, 'spps', 'cpu'), null, JSON.stringify(rs?.rows[0] ?? null));
  }
  const [sc, st] = project(300_000);
  assert.equal(forecastRunTime(null, 'spps', workNow(sc, st)), null);
  const t = runTimeText(null, 'spps', 'cpu', Date.now(), false);
  assert.equal(t.forecast, null);
  assert.equal(t.finish, null);
  assert.equal(t.basis, 'No measurement yet: this project has no finished SPPS run to time the next one by. Its first run is the measurement.');
  assert.match(runTimeText(null, 'spps', 'gpu', Date.now(), false).basis, /no finished SPPS on the GPU run/);
  assert.match(runTimeText(null, 'tcr', 'cpu', Date.now(), false).basis, /no finished TCR run/);
  assert.doesNotMatch(t.basis, /\d/, 'not a digit in it');
});

test('a forecast is measured on the same device: a GPU run never times a CPU run, nor the reverse', () => {
  const gpu = row(2, { gpu_device: 'NVIDIA GeForce RTX 5070, sm_120', elapsed_s: '5.0' });
  const rs = runs(row(1), gpu, row(3, { solver: 'tcr', work: null, elapsed_s: '1.2' }));
  assert.equal(measuredRun(rs, 'spps', 'cpu')?.number, 1);
  assert.equal(measuredRun(rs, 'spps', 'gpu')?.number, 2);
  assert.equal(measuredRun(runs(row(1)), 'spps', 'gpu'), null);
  // The newest of several, by number.
  assert.equal(measuredRun(runs(row(1), row(4, { elapsed_s: '200.0' }), row(3)), 'spps', 'cpu')?.seconds, 200);
});

test('TCR: its last run’s time, unscaled, said so', () => {
  const rs = runs(row(1), row(3, { solver: 'tcr', work: null, elapsed_s: '1.2' }));
  const m = measuredRun(rs, 'tcr', 'cpu');
  assert.ok(m && m.work === null && m.seconds === 1.2);
  const f = forecastRunTime(m, 'tcr', null);
  assert.equal(f?.seconds, 1.2);
  const t = runTimeText(f, 'tcr', 'cpu', new Date(2026, 9, 9, 4, 0, 0).getTime(), true);
  assert.equal(t.forecast, 'under a minute');
  assert.equal(t.finish, 'done about 04:00 if it starts now');
  assert.equal(t.basis, 'Measured on your last TCR run: Run 3 took 0:01 (m:ss). TCR has no particles, so its time does not scale.');
});

test('durations in minutes, never next to s', () => {
  assert.equal(durationWords(0), 'under a minute');
  assert.equal(durationWords(59.9), 'under a minute');
  assert.equal(durationWords(60), 'about 1 min');
  assert.equal(durationWords(89), 'about 1 min');
  assert.equal(durationWords(90), 'about 2 min');
  assert.equal(durationWords(3569), 'about 59 min');
  assert.equal(durationWords(3570), 'about 1 h 00 min');
  assert.equal(durationWords(3600 + 300), 'about 1 h 05 min');
  assert.equal(durationWords(NaN), '—');
});

test('the finish as a clock time, to the minute, with the weekday on another day', () => {
  const now = new Date(2026, 9, 9, 4, 0, 0).getTime(); // Friday 2026-10-09 04:00, local
  assert.equal(clockText(now + 31 * 60_000, now), '04:31');
  assert.equal(clockText(now + 31 * 60_000 + 29_000, now), '04:31', 'rounded to the minute');
  assert.equal(clockText(now + 31 * 60_000 + 31_000, now), '04:32');
  assert.equal(clockText(now + 20 * 3600_000 + 10 * 60_000, now), 'Sat 00:10');
});

test('while SPPS solves: the finish from the progress so far', () => {
  const solveAt = 1_000_000;
  // A quarter done one minute into the solve: four minutes in all.
  assert.equal(liveFinishMs({ stage: 'solve', progress: 25, solveAt, progressAt: solveAt + 60_000 }), solveAt + 240_000);
  assert.equal(liveFinishMs({ stage: 'solve', progress: 100, solveAt, progressAt: solveAt + 60_000 }), solveAt + 60_000);
  assert.equal(liveFinishMs({ stage: 'solve', progress: 0.5, solveAt, progressAt: solveAt + 1_000 }), null, 'under 1 %: too early to say');
  assert.equal(liveFinishMs({ stage: 'mesh', progress: 25, solveAt, progressAt: solveAt + 60_000 }), null);
  assert.equal(liveFinishMs({ stage: 'solve', progress: null, solveAt, progressAt: undefined }), null);
  assert.equal(liveFinishMs({ stage: 'solve', progress: 25, solveAt: undefined, progressAt: solveAt }), null);
});

test('no acoustic number in anything this module says', () => {
  const m = measuredRun(runs(row(1)), 'spps', 'cpu');
  const now = new Date(2026, 9, 9, 4, 0, 0).getTime();
  const texts = [
    solverSentence('spps', 'cpu', null),
    solverSentence('spps', 'gpu', 'NVIDIA GeForce RTX 5070'),
    solverSentence('tcr', 'cpu', null),
    ...[1, 59, 600, 7200, 200_000].flatMap((s) => {
      const t = runTimeText({ seconds: s, from: m! }, 'spps', 'cpu', now, true);
      return [t.forecast ?? '', t.finish ?? '', t.basis];
    }),
    runTimeText(null, 'spps', 'cpu', now, false).basis,
  ];
  for (const t of texts) {
    assert.doesNotMatch(t, ACOUSTIC_NUMBER_RE, t);
    assert.doesNotMatch(t, PARAMETER_NUMBER, t);
  }
});
