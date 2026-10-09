// How long a run will take, before it is launched, and when it will be done (Burhan's 10-05 UI list, item 5,
// "Simulate in words": "solver choice explained, run cost by the particle slider measured not guessed, finish
// time as a clock time"; session-logs/HANDOFF-2026-10-05.md:128).
//
// Measured, not guessed: the forecast is this project's own last finished run of the same solver on the same
// device (SPPS on the CPU, SPPS on the GPU, TCR), its solver wall time (`run.json`'s `outcome.elapsed_ms`, the
// Runs row's `elapsed_s`), scaled linearly by the work SPPS is given:
//   particles per source x sources x bands computed x duration
// the run's own from its `config.xml` (`RunRow.work`), the next run's from the settings now. With no such run
// there is no forecast, only that there is no measurement yet. Linear is the forecast's one assumption: SPPS
// traces every particle of every source in every band step by step until it is absorbed or the duration ends, so
// its work grows with each factor; in an absorbing room most particles die before the duration ends, so a longer
// duration costs less than in proportion and the forecast reads long. It reads long too when bands run in
// parallel: SPPS on the CPU runs one thread per band (sppsNantes.cpp:373-386), so on a CPU with cores to spare a
// run with twice the bands takes far less than twice the time. And it reads long when a short run's fixed start-up
// (reading the mesh, writing the results) is scaled up with its particles: a 0:05 GPU run of 300,000 particles
// forecast about 50 s for 3,000,000, which took 44.6 s; a CPU run of 150,000 particles on the box took 3.4 s and
// forecast 68 s for 3,000,000, which took 56.6 s. So an SPPS run shorter than `MIN_MEASURED_S` is not scaled
// from at all: it is too short to time the next run by. The solver's time only: the meshing before it is not
// counted. TCR has no particles: its time is its last run's, unscaled.
//
// Numbers. No acoustic number is shown before a run (M10 rule 6, the self-test's no_acoustic_numbers), and no
// number next to s, ms or % (m10-h's ACOUSTIC_NUMBER): a duration is said in minutes, a measured time as m:ss (as
// the running block's Elapsed), the finish as a clock time.
import type { RunRow, RunsView, SceneState } from '../../bindings/ipc.ts';
import type { ActiveRun, SolverName, SppsDevice } from '../../store.ts';
import { elapsedText, groupedInt, type ProjectSettings } from './model.ts';

/** What SPPS is given to do: the factors its run time is scaled by. */
export interface RunWork {
  particlesPerSource: number;
  sources: number;
  bands: number;
  durationS: number;
}

/** The run a forecast is measured on. */
export interface Measurement {
  run: string;
  /** Its number among this project's runs (`Run 3`). */
  number: number;
  /** The solver's wall time, s. */
  seconds: number;
  /** Its work; null for TCR, which has none to scale by. */
  work: RunWork | null;
  /** An SPPS run under `MIN_MEASURED_S`: named, never scaled from. */
  tooShort: boolean;
}

export interface RunTimeForecast {
  /** The solver's wall time forecast, s. */
  seconds: number;
  from: Measurement;
}

/** The shortest SPPS run a forecast is scaled from, s. Below it the solver's fixed start-up is most of the run,
 * and scaling that by the particles reads long. `elapsed_s` is rounded to 0.1 s, so the rounded time is compared. */
export const MIN_MEASURED_S = 2;

/** `RunWork`'s product, the units a run's time is scaled by. */
export function workUnits(w: RunWork): number {
  return w.particlesPerSource * w.sources * w.bands * w.durationS;
}

/** A finite, positive work, or null. */
function checked(w: RunWork): RunWork | null {
  const u = workUnits(w);
  return Number.isFinite(u) && u > 0 ? w : null;
}

/** The newest run of this project a forecast for `solver` on `device` may be measured on: finished OK, the same
 * solver on the same device (a GPU run carries `gpu_device`), its solver time read, and for SPPS its work and at
 * least `MIN_MEASURED_S`. With only shorter SPPS runs, the newest of them, `tooShort`, so the line can say why
 * there is no forecast. */
export function measuredRun(runs: RunsView | null, solver: SolverName, device: SppsDevice): Measurement | null {
  let best: { row: RunRow; m: Measurement } | null = null;
  let short: { row: RunRow; m: Measurement } | null = null;
  for (const r of runs?.rows ?? []) {
    if (r.status !== 'OK' || r.solver !== solver) continue;
    if (solver === 'spps' && (device === 'gpu') !== !!r.gpu_device) continue;
    const seconds = Number(r.elapsed_s ?? NaN);
    if (!Number.isFinite(seconds) || seconds < 0) continue;
    let work: RunWork | null = null;
    if (solver === 'spps') {
      const w = r.work;
      if (!w) continue;
      work = checked({ particlesPerSource: w.particles_per_source, sources: w.sources, bands: w.bands, durationS: w.duration_s });
      if (!work || !(seconds > 0)) continue;
    }
    const tooShort = solver === 'spps' && seconds < MIN_MEASURED_S;
    const m = { run: r.run, number: r.number, seconds, work, tooShort };
    if (tooShort) {
      if (!short || r.number > short.row.number) short = { row: r, m };
    } else if (!best || r.number > best.row.number) best = { row: r, m };
  }
  return best?.m ?? short?.m ?? null;
}

/** The work of an SPPS run on `scene` as set now, `particles` overriding the stored count (the field as typed);
 * null for TCR, or settings that do not read. */
export function workNow(
  scene: Pick<SceneState, 'view'> | null,
  settings: ProjectSettings | null,
  particles: number | null = null,
): RunWork | null {
  if (!scene || !settings) return null;
  const s = settings.solvers.spps;
  if (typeof s.duration_s !== 'number') return null;
  return checked({
    particlesPerSource: particles ?? s.particles_per_source,
    sources: scene.view.sources.filter((x) => x.enabled).length,
    bands: s.bands_computed.filter(Boolean).length,
    durationS: s.duration_s,
  });
}

/** The forecast: the measured run's time, for SPPS scaled by `now`'s work over its own; null with no measurement,
 * a measurement `tooShort`, or (SPPS) no work now. */
export function forecastRunTime(from: Measurement | null, solver: SolverName, now: RunWork | null): RunTimeForecast | null {
  if (!from) return null;
  if (solver === 'tcr') return { seconds: from.seconds, from };
  if (!from.work || !now || from.tooShort) return null;
  return { seconds: (from.seconds * workUnits(now)) / workUnits(from.work), from };
}

/** `under a minute`, `about 6 min`, `about 2 h 05 min`: a forecast in minutes, never next to `s`. */
export function durationWords(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return '—';
  if (seconds < 60) return 'under a minute';
  const min = Math.round(seconds / 60);
  if (min < 60) return `about ${min} min`;
  return `about ${Math.floor(min / 60)} h ${String(min % 60).padStart(2, '0')} min`;
}

const DAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

/** A local clock time to the minute, `04:31`; on another day than `nowMs`, with its weekday, `Sat 04:31`. */
export function clockText(atMs: number, nowMs: number): string {
  const at = new Date(Math.round(atMs / 60_000) * 60_000);
  const now = new Date(nowMs);
  const hm = `${String(at.getHours()).padStart(2, '0')}:${String(at.getMinutes()).padStart(2, '0')}`;
  const sameDay = at.getFullYear() === now.getFullYear() && at.getMonth() === now.getMonth() && at.getDate() === now.getDate();
  return sameDay ? hm : `${DAYS[at.getDay()]} ${hm}`;
}

/** The solver as the panel names it (`solverChoices`). */
function solverWords(solver: SolverName, device: SppsDevice): string {
  return solver === 'tcr' ? 'TCR' : device === 'gpu' ? 'SPPS on the GPU' : 'SPPS';
}

/** What the run-time line says: the forecast and the finish clock, and what it was measured on; or that there is
 * no measurement yet. */
export interface RunTimeText {
  /** `about 6 min`, the solver's time; null with no measurement. */
  forecast: string | null;
  /** `done about 04:31 if it starts now`; null with no measurement. */
  finish: string | null;
  /** What it was measured on, or why there is no forecast. */
  basis: string;
}

/** `from` is `measuredRun`'s answer, which says why there is no forecast when `f` is null. */
export function runTimeText(f: RunTimeForecast | null, solver: SolverName, device: SppsDevice, nowMs: number, from: Measurement | null): RunTimeText {
  const name = solverWords(solver, device);
  if (!f) {
    return {
      forecast: null,
      finish: null,
      basis: from?.tooShort
        ? `Run ${from.number} took ${elapsedText(from.seconds * 1000)} (m:ss), too short to time the next run by: its start-up is most of it, and scaling that by the particles would read long. A longer run is the measurement.`
        : from
          ? 'No forecast until the settings read.'
          : `No measurement yet: this project has no finished ${name} run to time the next one by. Its first run is the measurement.`,
    };
  }
  const m = f.from;
  const took = `Run ${m.number} took ${elapsedText(m.seconds * 1000)}`;
  const basis =
    m.work === null
      ? `Measured on your last TCR run: ${took} (m:ss). TCR has no particles, so its time does not scale.`
      : `Measured on your run at ${groupedInt(m.work.particlesPerSource)} particles: ${took} (m:ss) with ${m.work.sources} ${m.work.sources === 1 ? 'source' : 'sources'} and ${m.work.bands} ${m.work.bands === 1 ? 'band' : 'bands'}, scaled by particles × sources × bands × duration. Meshing comes on top.`;
  return { forecast: durationWords(f.seconds), finish: `done about ${clockText(nowMs + f.seconds * 1000, nowMs)} if it starts now`, basis };
}

/** A PROGRESS line as the live finish reads it: its share, %, and when it arrived, ms on this page's clock. */
export interface ProgressPoint {
  progress: number;
  at: number;
}

/** The live finish's rate is measured over the last tenth of the solve so far, and never over less than 20 s
 * (`liveFinishMs`; backlog row 18's replay picked it, runTime.test.ts). */
export const RATE_WINDOW_SHARE = 0.1;
export const RATE_WINDOW_MIN_MS = 20_000;

/** Where the rate's window starts for a last line at `lastAt`: it only moves forward as lines come. */
function windowStart(lastAt: number, solveAt: number): number {
  return lastAt - Math.max(RATE_WINDOW_SHARE * (lastAt - solveAt), RATE_WINDOW_MIN_MS);
}

/**
 * The lines the live finish projects from, after a PROGRESS line of `progress` arriving at `at`: that line appended
 * when its share is above every one before it, else the log unchanged, so the log only rises and its last point is
 * the highest share yet and when it came. A share that falls (a solver printing per band, or starting over) would
 * otherwise throw the finish out by the whole run: from 100 to 5 % it would read hours late. Only the points the
 * rate's window can still reach are kept (the newest at or before its start, and every one after), so a run of
 * hours holds a tenth of its lines, not all of them.
 *
 * What upstream prints (I-Simpa-upstream, src/spps/sppsNantes.cpp:202, 363-380, and
 * src/lib_interface/input_output/progressionInfo.h): one progress tree for the whole run. Its root counts the bands
 * computed, each band (one thread per band on the CPU, all started at once) its sources, each source its particles;
 * every thread pushes its particle into the same tree under one mutex, and `#<share>` is the whole run's share,
 * printed only when it has risen by 0.01 since the last line (`OutputCurrentProgression`) and never at 100. So the
 * CPU solver's line neither resets nor goes per band, and with bands in parallel it is the mean of their shares.
 * The guard costs nothing there; it holds against anything else that prints PROGRESS lines.
 */
export function logProgress(
  log: readonly ProgressPoint[] | undefined,
  solveAt: number | undefined,
  progress: number | null | undefined,
  at: number,
): readonly ProgressPoint[] | undefined {
  if (typeof progress !== 'number' || !Number.isFinite(progress)) return log;
  const last = log?.[log.length - 1];
  if (last !== undefined && !(progress > last.progress)) return log;
  const next = [...(log ?? []), { progress, at }];
  if (solveAt === undefined) return next;
  const start = windowStart(at, solveAt);
  let i = 0;
  while (i + 1 < next.length && next[i + 1].at <= start) i++;
  return i > 0 ? next.slice(i) : next;
}

/**
 * While SPPS solves: when it will be done, from its progress so far. The share left over the rate of the last tenth
 * of the solve, at least its last 20 s (`RATE_WINDOW_SHARE`, `RATE_WINDOW_MIN_MS`), measured from the newest line at
 * or before the window's start, or from the solve's start at 0 % while the solve is shorter than the window. Null
 * before the solve, before a progress line, or below 1 % done.
 *
 * Why a recent rate and not the share over the whole time (until backlog row 18's replay): on the hall the share
 * slows near the end (0.34 % a second until 255 s, 0.29 to 286 s, 0.12 after: with its 6 bands on 6 threads that
 * reads as one band done at 255 s and three more at 286 s, the share being their mean), and the whole-run rate then
 * put the finish up to 0.45 s in the past with 0.25 s to go, while a recent rate follows the slowing. Neither sees
 * the slowing before it comes: 32 s from the hall's end the recent rate said 13 s left and the whole-run rate 12,
 * and row 18 stays open for it (runTime.test.ts has the numbers).
 *
 * Upstream's share can also stop short: the box at 3,000,000 particles and 27 bands printed 59.25 % at 33 s and
 * nothing more until it ended at 57 s, in both runs at that count (solver.stdout.txt). 16/27 is 59.26 %, where the
 * `float` the root of progressionInfo.h sums into reaches 16 and its spacing doubles, which is most likely why. The
 * projection then stays where the last line put it.
 */
export function liveFinishMs(run: Pick<ActiveRun, 'stage' | 'solveAt' | 'progressLog'>): number | null {
  const log = run.progressLog ?? [];
  const last = log[log.length - 1];
  const s0 = run.solveAt;
  if (run.stage !== 'solve' || last === undefined || s0 === undefined) return null;
  if (!(last.progress >= 1 && last.progress <= 100) || !(last.at >= s0)) return null;
  const start = windowStart(last.at, s0);
  let ref: ProgressPoint = { progress: 0, at: s0 };
  for (const pt of log) if (pt !== last && pt.at <= start) ref = pt;
  const rate = (last.progress - ref.progress) / (last.at - ref.at);
  if (!(rate > 0) || !Number.isFinite(rate)) return s0 + ((last.at - s0) * 100) / last.progress;
  return last.at + (100 - last.progress) / rate;
}

/** The sentence under the solver choice: which solver will run and what it is for. */
export function solverSentence(solver: SolverName, device: SppsDevice, gpuName: string | null): string {
  if (solver === 'tcr')
    return 'TCR will run: classical theory, from the room’s volume and its surfaces’ absorption, band by band. It traces no particles, so there is no particle count to set.';
  if (device === 'gpu')
    return `SPPS on the GPU will run: the same particle tracing as SPPS, on ${gpuName ?? 'the CUDA device'}. Its time grows with the particles, sources, bands and duration.`;
  return 'SPPS will run on the CPU: it traces sound particles through the room for the echograms at the receivers and the sound maps. Its time grows with the particles, sources, bands and duration.';
}

/**
 * C36: the running block's line while SPPS solves, from `liveFinishMs`: the time left in words and the finish as a
 * clock time, `About 6 min left, done about 04:31, from the progress so far`. Past the projected finish it says so
 * rather than count below zero. Null when there is no projection yet (`liveFinishMs` null). Never a number next to s.
 */
export function remainingText(finishMs: number | null, nowMs: number): string | null {
  if (finishMs === null || !Number.isFinite(finishMs)) return null;
  const left = (finishMs - nowMs) / 1000;
  const done = `done about ${clockText(finishMs, nowMs)}`;
  if (left <= 0) return `Should finish any moment: the progress so far gave ${done}`;
  const words = durationWords(left);
  return `${words.charAt(0).toUpperCase()}${words.slice(1)} left, ${done}, from the progress so far`;
}
