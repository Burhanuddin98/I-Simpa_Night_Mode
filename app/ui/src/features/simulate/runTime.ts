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
// duration costs less than in proportion and the forecast reads long. The solver's time only: the meshing before
// it is not counted. TCR has no particles: its time is its last run's, unscaled.
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
}

export interface RunTimeForecast {
  /** The solver's wall time forecast, s. */
  seconds: number;
  from: Measurement;
}

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
 * solver on the same device (a GPU run carries `gpu_device`), its solver time read, and for SPPS its work. */
export function measuredRun(runs: RunsView | null, solver: SolverName, device: SppsDevice): Measurement | null {
  let best: { row: RunRow; m: Measurement } | null = null;
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
    if (!best || r.number > best.row.number) best = { row: r, m: { run: r.run, number: r.number, seconds, work } };
  }
  return best?.m ?? null;
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
 * or (SPPS) no work now. */
export function forecastRunTime(from: Measurement | null, solver: SolverName, now: RunWork | null): RunTimeForecast | null {
  if (!from) return null;
  if (solver === 'tcr') return { seconds: from.seconds, from };
  if (!from.work || !now) return null;
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

export function runTimeText(f: RunTimeForecast | null, solver: SolverName, device: SppsDevice, nowMs: number, hasMeasurement: boolean): RunTimeText {
  const name = solverWords(solver, device);
  if (!f) {
    return {
      forecast: null,
      finish: null,
      basis: hasMeasurement
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

/** While SPPS solves: when it will be done, from its progress so far (the time from the solve's start to the last
 * progress line, over the share done). Null before the solve, before a progress line, or below 1 % done. */
export function liveFinishMs(run: Pick<ActiveRun, 'stage' | 'progress' | 'solveAt' | 'progressAt'>): number | null {
  const p = run.progress;
  if (run.stage !== 'solve' || typeof p !== 'number' || !Number.isFinite(p) || p < 1 || p > 100) return null;
  if (run.solveAt === undefined || run.progressAt === undefined || !(run.progressAt >= run.solveAt)) return null;
  return run.solveAt + ((run.progressAt - run.solveAt) * 100) / p;
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
