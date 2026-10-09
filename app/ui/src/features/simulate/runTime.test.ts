import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import type { RunRow, RunsView, SceneState } from '../../bindings/ipc.ts';
import { ACOUSTIC_NUMBER_RE, type ProjectSettings } from './model.ts';
import {
  clockText,
  durationWords,
  forecastRunTime,
  liveFinishMs,
  logProgress,
  measuredRun,
  MIN_MEASURED_S,
  type ProgressPoint,
  RATE_WINDOW_MIN_MS,
  RATE_WINDOW_SHARE,
  remainingText,
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
    exports: [],
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
  const t = runTimeText(f, 'spps', 'cpu', new Date(2026, 9, 9, 4, 0, 0).getTime(), m);
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
  const t = runTimeText(null, 'spps', 'cpu', Date.now(), null);
  assert.equal(t.forecast, null);
  assert.equal(t.finish, null);
  assert.equal(t.basis, 'No measurement yet: this project has no finished SPPS run to time the next one by. Its first run is the measurement.');
  assert.match(runTimeText(null, 'spps', 'gpu', Date.now(), null).basis, /no finished SPPS on the GPU run/);
  assert.match(runTimeText(null, 'tcr', 'cpu', Date.now(), null).basis, /no finished TCR run/);
  assert.doesNotMatch(t.basis, /\d/, 'not a digit in it');
});

test('an SPPS run under 2 s is too short to time the next run by: never scaled, said so', () => {
  assert.equal(MIN_MEASURED_S, 2);
  const [sc, st] = project(3_000_000);
  // Only a 1.9 s run (elapsed_s is rounded to 0.1 s): no forecast, and the line says why.
  const short = measuredRun(runs(row(1, { elapsed_s: '1.9' })), 'spps', 'cpu');
  assert.ok(short && short.tooShort && short.number === 1);
  assert.equal(forecastRunTime(short, 'spps', workNow(sc, st)), null);
  const t = runTimeText(null, 'spps', 'cpu', Date.now(), short);
  assert.equal(t.forecast, null);
  assert.equal(t.finish, null);
  assert.equal(
    t.basis,
    'Run 1 took 0:01 (m:ss), too short to time the next run by: its start-up is most of it, and scaling that by the particles would read long. A longer run is the measurement.',
  );
  // 2.0 s is long enough; an older long run is measured on before a newer short one.
  assert.equal(measuredRun(runs(row(1, { elapsed_s: '2.0' })), 'spps', 'cpu')?.tooShort, false);
  const m = measuredRun(runs(row(1), row(2, { elapsed_s: '0.8' })), 'spps', 'cpu');
  assert.ok(m && m.number === 1 && !m.tooShort);
  // TCR is not scaled: its 1.2 s run is its forecast (the TCR test below).
  assert.equal(measuredRun(runs(row(3, { solver: 'tcr', work: null, elapsed_s: '1.2' })), 'tcr', 'cpu')?.tooShort, false);
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
  const t = runTimeText(f, 'tcr', 'cpu', new Date(2026, 9, 9, 4, 0, 0).getTime(), m);
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

/** The log as the run's stream builds it, from (ms after the solve's start, share) lines. */
function logOf(solveAt: number, lines: [number, number][]): readonly ProgressPoint[] | undefined {
  let log: readonly ProgressPoint[] | undefined;
  for (const [ms, p] of lines) log = logProgress(log, solveAt, p, solveAt + ms);
  return log;
}

test('while SPPS solves: the finish from the progress so far', () => {
  const solveAt = 1_000_000;
  const live = (lines: [number, number][], stage = 'solve') => liveFinishMs({ stage, solveAt, progressLog: logOf(solveAt, lines) });
  // A quarter done one minute into the solve, at a steady rate: four minutes in all.
  assert.equal(live([[20_000, 25 / 3], [40_000, 50 / 3], [60_000, 25]]), solveAt + 240_000);
  // Shorter than the window: measured from the solve's start at 0 %.
  assert.equal(live([[5_000, 25]]), solveAt + 20_000);
  assert.equal(live([[60_000, 100]]), solveAt + 60_000);
  assert.equal(live([[1_000, 0.5]]), null, 'under 1 %: too early to say');
  assert.equal(live([[60_000, 25]], 'mesh'), null);
  assert.equal(live([]), null);
  assert.equal(liveFinishMs({ stage: 'solve', solveAt: undefined, progressLog: logOf(solveAt, [[60_000, 25]]) }), null);
});

test('a slowing share moves the finish out: the rate is the last tenth of the solve, at least 20 s', () => {
  assert.equal(RATE_WINDOW_SHARE, 0.1);
  assert.equal(RATE_WINDOW_MIN_MS, 20_000);
  const solveAt = 0;
  // 0.5 % a second for 180 s (90 %), then 0.1 % a second for 20 s: 92 % at 200 s, 80 s left at the new rate.
  const lines: [number, number][] = [];
  for (let s = 1; s <= 180; s++) lines.push([s * 1000, s * 0.5]);
  for (let s = 181; s <= 200; s++) lines.push([s * 1000, 90 + (s - 180) * 0.1]);
  const log = logOf(solveAt, lines)!;
  assert.equal(Math.round(liveFinishMs({ stage: 'solve', solveAt, progressLog: log })!), 280_000);
  // The whole-run rate would have said 17 s left.
  assert.equal(Math.round((200_000 * 100) / 92 - 200_000), 17_391);
  // Only what the window can reach is kept: the point at its start (180 s) and the 20 after it.
  assert.equal(log.length, 21);
  assert.deepEqual(log[0], { progress: 90, at: 180_000 });
});

test('the live finish keeps the highest share seen: a share that falls never throws the clock late', () => {
  const solveAt = 1_000_000;
  let log = logOf(solveAt, [[30_000, 25], [60_000, 50]]);
  const before = liveFinishMs({ stage: 'solve', solveAt, progressLog: log });
  assert.equal(before, solveAt + 120_000);
  // From 50 down to 5 (a per-band share, or a solver starting over): the log holds, the finish does not move.
  const after = logProgress(log, solveAt, 5, solveAt + 61_000);
  assert.equal(after, log);
  assert.equal(liveFinishMs({ stage: 'solve', solveAt, progressLog: after }), before);
  // The same share again keeps its first time; a higher one is appended; no number keeps what it had.
  assert.equal(logProgress(log, solveAt, 50, solveAt + 62_000), log);
  log = logProgress(log, solveAt, 60, solveAt + 70_000);
  assert.deepEqual(log?.at(-1), { progress: 60, at: solveAt + 70_000 });
  assert.equal(logProgress(log, solveAt, null, solveAt + 71_000), log);
  assert.equal(logProgress(log, solveAt, Number.NaN, solveAt + 71_000), log);
});

// Backlog row 18's done-when, replayed: the two runs' PROGRESS lines as the GUI got them (c36-timelines.json, from
// .out/v1q-p0a/c36-*/bed-run.json), fed in at the times they were sampled, and the time left the running block would
// have said at each sample of the second half, against the time the run then really had left.
interface Timeline {
  endMs: number;
  samplesMs: number[];
  lines: [number, number][];
}
const TIMELINES = JSON.parse(readFileSync(new URL('./c36-timelines.json', import.meta.url), 'utf8')) as { runs: Record<'box' | 'hall', Timeline> };

/** The worst relative error of the time left over the second half's samples, and where. */
function replay(tl: Timeline, finish: (log: readonly ProgressPoint[] | undefined) => number | null) {
  let log: readonly ProgressPoint[] | undefined;
  let next = 0;
  let worst = { error: 0, atS: 0, leftS: 0, saidS: 0 };
  for (const now of tl.samplesMs) {
    while (next < tl.lines.length && tl.lines[next][0] <= now) {
      log = logProgress(log, 0, tl.lines[next][1], tl.lines[next][0]);
      next++;
    }
    if (now < tl.endMs / 2) continue;
    const f = finish(log);
    const left = tl.endMs - now;
    const error = f === null ? Number.POSITIVE_INFINITY : (f - now - left) / left;
    if (Math.abs(error) > Math.abs(worst.error)) worst = { error, atS: now / 1000, leftS: left / 1000, saidS: f === null ? NaN : (f - now) / 1000 };
  }
  return worst;
}

const pct = (x: number) => Math.round(x * 1000) / 10;
// %, signed. The box's is luck: its share stops at 59.25 % at 33 s (upstream's float sum, runTime.ts), so its second
// half is one projection held from that line, which happens to land near its end. The hall's is the slowing near
// its end that no rate sees before it comes: 13.4 s said with 32.1 s left, 283.6 s into the solve.
const [BOX_WORST, HALL_WORST, BOX_WHOLE_WORST, HALL_WHOLE_WORST] = [1.7, -58.3, -117.4, -277.9];

test('backlog 18, replayed: the time left on the box and the hall, every sample of the second half', () => {
  const recent = (log: readonly ProgressPoint[] | undefined) => liveFinishMs({ stage: 'solve', solveAt: 0, progressLog: log });
  // What 4d0b970 shipped, the share over the whole time, for comparison.
  const whole = (log: readonly ProgressPoint[] | undefined) => {
    const last = log?.at(-1);
    return last && last.progress >= 1 ? (last.at * 100) / last.progress : null;
  };
  const box = replay(TIMELINES.runs.box, recent);
  const hall = replay(TIMELINES.runs.hall, recent);
  const hallWhole = replay(TIMELINES.runs.hall, whole);
  const boxWhole = replay(TIMELINES.runs.box, whole);
  // The measured worst errors, held so a change that makes them worse fails here.
  assert.equal(pct(box.error), BOX_WORST, JSON.stringify(box));
  assert.equal(pct(hall.error), HALL_WORST, JSON.stringify(hall));
  assert.equal(pct(boxWhole.error), BOX_WHOLE_WORST, JSON.stringify(boxWhole));
  assert.equal(pct(hallWhole.error), HALL_WHOLE_WORST, JSON.stringify(hallWhole));
  // Row 18 wants every sample within 30 %: the hall is not, so the row stays open.
  assert.ok(Math.abs(hall.error) > 0.3, 'if this passes now, close backlog row 18 with these numbers');
});

test('C36: the time left in words and the finish clock, from the progress so far', () => {
  const now = new Date(2026, 9, 9, 4, 0, 0).getTime();
  assert.equal(remainingText(now + 6 * 60_000, now), 'About 6 min left, done about 04:06, from the progress so far');
  assert.equal(remainingText(now + 40_000, now), 'Under a minute left, done about 04:01, from the progress so far');
  assert.equal(remainingText(now + 125 * 60_000, now), 'About 2 h 05 min left, done about 06:05, from the progress so far');
  assert.equal(remainingText(now - 5_000, now), 'Should finish any moment: the progress so far gave done about 04:00', 'never below zero');
  assert.equal(remainingText(null, now), null, 'no projection yet');
  assert.equal(remainingText(Number.NaN, now), null);
});

test('no acoustic number in anything this module says', () => {
  const m = measuredRun(runs(row(1)), 'spps', 'cpu');
  const now = new Date(2026, 9, 9, 4, 0, 0).getTime();
  const texts = [
    solverSentence('spps', 'cpu', null),
    solverSentence('spps', 'gpu', 'NVIDIA GeForce RTX 5070'),
    solverSentence('tcr', 'cpu', null),
    ...[1, 59, 600, 7200, 200_000].flatMap((s) => {
      const t = runTimeText({ seconds: s, from: m! }, 'spps', 'cpu', now, m);
      return [t.forecast ?? '', t.finish ?? '', t.basis];
    }),
    runTimeText(null, 'spps', 'cpu', now, null).basis,
    runTimeText(null, 'spps', 'cpu', now, measuredRun(runs(row(1, { elapsed_s: '1.4' })), 'spps', 'cpu')).basis,
    ...[-5, 1, 59, 600, 7200].map((s) => remainingText(now + s * 1000, now) ?? ''),
  ];
  for (const t of texts) {
    assert.doesNotMatch(t, ACOUSTIC_NUMBER_RE, t);
    assert.doesNotMatch(t, PARAMETER_NUMBER, t);
  }
});
