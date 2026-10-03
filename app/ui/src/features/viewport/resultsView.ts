// What the Results step's viewport shows, and the loads behind it (M12 P3): the selected run's
// surface maps (by kind and band, R41), its difference from a baseline run (R52), and its
// particles (R53), all through P1's IPC (`run_data`, `run_surface_map`, `run_particles`), which
// serve only results that load and verify. The drawing is resultsLayer.ts's, through the engine.
//
// Nothing loads off the Results step. A run whose results are refused or unverified shows no map
// (the panel says why); a run that saved no particles shows MQ4's notice instead of playback.
import { asCmdError, backend } from '../../backend';
import type { RunData, SurfaceMapInfo } from '../../bindings/ipc';
import { decodeParticles, decodeSurfaceMap, type SurfaceMap } from '../../resultsData';
import { runsStore, sceneStore, selectedRunStore, stepStore, Store } from '../../store';
import { Animator } from './animator';
import { showMap, showParticles } from './engine';
import { diffRange, legendGradient, legendLabels, levelRange, surfaceMismatch, type Range } from './mapData';
import { noParticlesText } from './particles';

export interface MapGroup {
  key: string;
  label: string;
}

export type ParticlesView =
  | { state: 'none'; title: string; how: string }
  | { state: 'loading' | 'shown'; bandHz: number; particles?: number }
  | { state: 'error'; message: string }
  | { state: 'off' };

export interface ResultsView {
  run: string | null;
  /** `ready` once the run's index is read and verified. */
  status: 'off' | 'loading' | 'ready' | 'refused' | 'unverified' | 'error';
  message: string | null;
  data: RunData | null;
  groups: MapGroup[];
  group: string | null;
  /** The map's bands: a band in Hz, or null for all bands together (`Global`). */
  bands: (number | null)[];
  bandHz: number | null;
  diff: boolean;
  /** The run the difference is taken from, and why it cannot be, when it cannot. */
  baseline: string | null;
  baselineLabel: string | null;
  baselineReason: string | null;
  map: { path: string; kind: 'level' | 'diff'; range: Range; legend: { lo: string; mid: string; hi: string; gradient: string; title: string } } | null;
  mapMessage: string | null;
  particles: ParticlesView;
}

const OFF: ResultsView = {
  run: null,
  status: 'off',
  message: null,
  data: null,
  groups: [],
  group: null,
  bands: [],
  bandHz: null,
  diff: false,
  baseline: null,
  baselineLabel: null,
  baselineReason: null,
  map: null,
  mapMessage: null,
  particles: { state: 'off' },
};

export const resultsViewStore = new Store<ResultsView>(OFF);

const set = (patch: Partial<ResultsView>) => resultsViewStore.set({ ...resultsViewStore.get(), ...patch });

const groupKey = (s: SurfaceMapInfo) => `${s.cutting_plane ? 'plane' : 'surface'}|${s.field ?? ''}`;
const groupLabel = (s: SurfaceMapInfo) => `${s.cutting_plane ? 'Cutting planes' : 'Surface receivers'}${s.field ? ` · ${s.field}` : ''}`;
const bandName = (b: number | null) => (b === null ? 'all bands' : b >= 1000 ? `${b / 1000} kHz` : `${b} Hz`);
export const bandLabel = (b: number | null) => (b === null ? 'All' : b >= 1000 ? `${b / 1000}k` : String(b));

function groupsOf(d: RunData): MapGroup[] {
  const out: MapGroup[] = [];
  for (const s of d.surfaces) if (!out.some((g) => g.key === groupKey(s))) out.push({ key: groupKey(s), label: groupLabel(s) });
  return out;
}

function bandsOf(d: RunData, group: string | null): (number | null)[] {
  const b = d.surfaces.filter((s) => groupKey(s) === group).map((s) => s.band_hz ?? null);
  return [...b.filter((x): x is number => x !== null).sort((x, y) => x - y), ...(b.includes(null) ? [null] : [])];
}

const defaultBand = (bands: (number | null)[]): number | null => (bands.includes(1000) ? 1000 : (bands.find((b) => b !== null) ?? null));

// Each load carries the generation it started in; a later choice makes it stale.
let gen = 0;
let indexed: string | null = null;
const fresh = (g: number) => g === gen;

function runLabel(run: string): string {
  const row = runsStore.get()?.rows.find((r) => r.run === run);
  return row ? `Run ${row.number}` : run;
}

/** The default baseline: the newest other run of this project started before this one, else the newest other. */
function defaultBaseline(run: string): string | null {
  const rows = (runsStore.get()?.rows ?? []).filter((r) => r.run !== run && r.status !== 'RUNNING');
  const me = runsStore.get()?.rows.find((r) => r.run === run);
  const earlier = rows.filter((r) => me && r.number < me.number).sort((a, b) => b.number - a.number);
  const any = [...rows].sort((a, b) => b.number - a.number);
  return (earlier[0] ?? any[0])?.run ?? null;
}

async function loadIndex(run: string, g: number): Promise<void> {
  set({ ...OFF, run, status: 'loading' });
  showMap(null);
  showParticles(null);
  try {
    const idx = await backend.runData(run);
    if (!fresh(g)) return;
    if (idx.state.refusal || !idx.data) {
      set({ status: 'refused', message: 'Results refused: no map is drawn for this run.' });
      return;
    }
    if (!idx.state.verified) {
      set({ status: 'unverified', message: 'Results unverified: no map is drawn for this run.' });
      return;
    }
    const data = idx.data;
    const groups = groupsOf(data);
    const group = groups.find((x) => x.key.startsWith('surface|'))?.key ?? groups[0]?.key ?? null;
    const bands = bandsOf(data, group);
    indexed = run;
    Animator.reset(data.steps ?? Math.max(1, ...data.surfaces.map((s) => s.time_step_count)));
    set({ status: 'ready', data, groups, group, bands, bandHz: defaultBand(bands), baseline: defaultBaseline(run) });
  } catch (e) {
    if (!fresh(g)) return;
    const err = asCmdError(e);
    set({ status: 'error', message: `${err.message} (${err.code})` });
  }
}

async function mapBytes(run: string, path: string): Promise<SurfaceMap> {
  return decodeSurfaceMap(await backend.runSurfaceMap(run, path));
}

async function loadMap(g: number): Promise<void> {
  const v = resultsViewStore.get();
  const d = v.data;
  if (!d || !v.run) return;
  const info = d.surfaces.find((s) => groupKey(s) === v.group && (s.band_hz ?? null) === v.bandHz);
  if (!info) {
    showMap(null);
    set({ map: null, mapMessage: d.surfaces.length ? 'No map for this choice.' : 'This run has no surface receivers or cutting planes.' });
    return;
  }
  try {
    const m = await mapBytes(v.run, info.path);
    if (!fresh(g)) return;
    let base: SurfaceMap | null = null;
    let reason: string | null = null;
    if (v.diff) {
      if (!v.baseline) reason = 'No other run to compare with.';
      else {
        const bi = await backend.runData(v.baseline);
        if (!fresh(g)) return;
        if (!bi.data || !bi.state.verified) reason = `${runLabel(v.baseline)}: results not verified.`;
        else if (!bi.data.surfaces.some((s) => s.path === info.path)) reason = `${runLabel(v.baseline)} has no ${groupLabel(info).toLowerCase()} map at ${bandName(v.bandHz)}.`;
        else {
          const b = await mapBytes(v.baseline, info.path);
          if (!fresh(g)) return;
          const why = surfaceMismatch(m, b);
          if (why) reason = `${runLabel(v.baseline)} cannot be subtracted: ${why}.`;
          else base = b;
        }
      }
    }
    const kind: 'level' | 'diff' = base ? 'diff' : 'level';
    const range = kind === 'diff' && base ? diffRange(m, base) : levelRange(m);
    const what = groupLabel(info);
    const title = kind === 'diff'
      ? `Difference from ${runLabel(v.baseline as string)} · ${bandName(v.bandHz)}`
      : `${what} · level · ${bandName(v.bandHz)}`;
    const shownRange = range ?? { lo: 0, hi: 1 };
    const err = showMap(m, { run: v.run, path: info.path, bandHz: v.bandHz, kind, range: shownRange, baseline: base ? v.baseline : null }, base);
    if (!fresh(g)) return;
    set({
      map: err ? null : { path: info.path, kind, range: shownRange, legend: { ...legendLabels(shownRange, kind), gradient: legendGradient(kind), title } },
      mapMessage: err ?? (range ? null : `No energy reached the ${what.toLowerCase()} at ${bandName(v.bandHz)}.`),
      baselineLabel: v.baseline ? runLabel(v.baseline) : null,
      baselineReason: reason,
    });
  } catch (e) {
    if (!fresh(g)) return;
    const err = asCmdError(e);
    showMap(null);
    set({ map: null, mapMessage: `${err.message} (${err.code})` });
  }
}

async function loadParticles(g: number): Promise<void> {
  const v = resultsViewStore.get();
  const d = v.data;
  if (!d || !v.run) return;
  if (d.solver !== 'spps') {
    showParticles(null);
    set({ particles: { state: 'off' } });
    return;
  }
  if (d.particle_files.length === 0) {
    showParticles(null);
    const sources = (sceneStore.get()?.view.sources ?? []).filter((s) => s.enabled).length;
    set({ particles: { state: 'none', ...noParticlesText(d.steps ?? 0, sources) } });
    return;
  }
  const band = d.particle_files.some((f) => f.freq_hz === v.bandHz) ? (v.bandHz as number) : d.particle_files[0].freq_hz;
  set({ particles: { state: 'loading', bandHz: band } });
  try {
    const p = decodeParticles(await backend.runParticles(v.run, band));
    if (!fresh(g)) return;
    showParticles(p, { run: v.run, bandHz: band, particles: p.particleCount, records: p.recordCount });
    set({ particles: { state: 'shown', bandHz: band, particles: p.particleCount } });
  } catch (e) {
    if (!fresh(g)) return;
    const err = asCmdError(e);
    showParticles(null);
    set({ particles: { state: 'error', message: `${err.message} (${err.code})` } });
  }
}

/** Reloads what the current choices need. */
async function refresh(): Promise<void> {
  const g = ++gen;
  const run = selectedRunStore.get();
  if (stepStore.get() !== 'results' || !run) return;
  if (run !== indexed || resultsViewStore.get().run !== run) {
    indexed = null;
    await loadIndex(run, g);
    if (!fresh(g)) return;
  }
  if (resultsViewStore.get().status !== 'ready') return;
  await Promise.all([loadMap(g), loadParticles(g)]);
}

const fire = () => {
  void refresh();
};

export const resultsView = {
  setGroup(key: string): void {
    const d = resultsViewStore.get().data;
    if (!d) return;
    const bands = bandsOf(d, key);
    const cur = resultsViewStore.get().bandHz;
    set({ group: key, bands, bandHz: bands.includes(cur) ? cur : defaultBand(bands) });
    fire();
  },
  setBand(b: number | null): void {
    set({ bandHz: b });
    fire();
  },
  setDiff(on: boolean): void {
    set({ diff: on });
    fire();
  },
  setBaseline(run: string): void {
    set({ baseline: run });
    fire();
  },
};

let started = false;
/** Follows the step and the selected run (once; the overlay calls it on mount). */
export function startResultsView(): void {
  if (started) return;
  started = true;
  let step = stepStore.get();
  let run = selectedRunStore.get();
  stepStore.subscribe(() => {
    const s = stepStore.get();
    if (s === step) return;
    step = s;
    if (s === 'results') fire();
    else Animator.pause();
  });
  selectedRunStore.subscribe(() => {
    const r = selectedRunStore.get();
    if (r === run) return;
    run = r;
    indexed = null;
    if (r === null) {
      gen++;
      showMap(null);
      showParticles(null);
      resultsViewStore.set(OFF);
    } else fire();
  });
  fire();
}
