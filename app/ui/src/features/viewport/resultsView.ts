// What the Results step's viewport shows, and the loads behind it (M12 P3): the selected run's
// surface maps (by kind and band, R41), its difference from a baseline run (R52), and its
// particles (R53), all through P1's IPC (`run_data`, `run_surface_map`, `run_particles`), which
// serve only results that load and verify. The drawing is resultsLayer.ts's, through the engine.
//
// W5 (wow list; mapView.ts): smooth colour, contours and a fixed colour range are view choices
// kept here across bands and runs, applied to the GPU without a reload; the probe reads the map
// shown (`shownMaps`), the CPU's copy of the file's float32s.
//
// W2 (cumulative.ts): the cumulative map, a view choice kept like W5's, reloads the map with each
// face's running sum in the texture; a difference is not offered cumulative. W3 (particles.ts):
// trails of a chosen length over the particles shown, refused without particles or past the budget.
//
// The time window (window.ts, Burhan 2026-10-05): each face's mean over the last few steps, a view
// choice kept like W5's (default 10 ms), applied by one uniform without a reload; the legend's
// title and the probe say so. A cumulative map takes none (the switch says why); a difference
// averages both runs alike.
//
// Nothing loads off the Results step. A run whose results are refused or unverified shows no map
// (the panel says why); a run that saved no particles shows MQ4's notice instead of playback.
import * as actions from '../../actions';
import { asCmdError } from '../../backend';
import type { RunData, SurfaceMapInfo } from '../../bindings/ipc';
import { decodeParticles, decodeSurfaceMap, type SurfaceMap } from '../../resultsData';
import { runsStore, sceneStore, selectedRunStore, stepStore, Store } from '../../store';
import { Animator } from './animator';
import { resultsLayer, renderNow, setGlow, setMapWhilePlaying, showMap, showParticles, type Glow } from './engine';
import { cumulativeRange, cumulativeRefusal } from './cumulative';
import { diffRange, legendGradient, legendLabels, levelRange, surfaceMismatch, type Range } from './mapData';
import { emissionStep, noParticlesText } from './particles';
import type { ParticleLook } from './rays';
import { DEFAULT_WINDOW_MS, windowChoice, WINDOW_CUMULATIVE_REFUSAL, windowLabel } from './window';

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
  map: {
    path: string;
    kind: 'level' | 'diff';
    cumulative: boolean;
    /** The window the map is drawn with, steps (1: none), and the map's time step. */
    windowSteps: number;
    dtS: number | null;
    range: Range;
    legend: { lo: string; mid: string; hi: string; gradient: string; title: string };
  } | null;
  mapMessage: string | null;
  /** W5: smooth colour (R46), contours every `isoDb` dB on it, 0 for none (R48). */
  smooth: boolean;
  isoDb: number;
  /** Why smooth colour is not drawn for this map, or null. */
  smoothRefusal: string | null;
  /** W5: a fixed colour range for level maps (R47), or null for the map's own. */
  fixed: Range | null;
  /** W2: the cumulative map (sound building up) for level maps. */
  cumulative: boolean;
  /** Why the shown map has no cumulative view, or null. */
  cumulativeRefusal: string | null;
  /** The time window chosen, ms of sound (0: each step on its own); why it is not drawn as chosen, or null. */
  windowMs: number;
  windowRefusal: string | null;
  /** W3: trail length in steps, 0 for none; and why trails cannot be drawn, or null. */
  trails: number;
  trailRefusal: string | null;
  /** B2 (decision 68 (b)): how particles are drawn, and why the GPU looks cannot be, or null. */
  look: ParticleLook;
  /** Why each GPU look cannot be drawn, or null (rays need WebGPU; glow runs on the WebGL2 fallback too). */
  lookRefusal: { glow: string | null; rays: string | null };
  /** Decision 69: how strongly the particles' light blooms. */
  glow: Glow;
  /** The warm ramp's span for the band shown, dB below its loudest particle (the legend's floor). */
  warmDepth: number;
  /** Round 2: the map kept at full strength while the light plays (else faded to 25 %). */
  mapFull: boolean;
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
  smooth: false,
  isoDb: 0,
  smoothRefusal: null,
  fixed: null,
  cumulative: false,
  cumulativeRefusal: null,
  windowMs: DEFAULT_WINDOW_MS,
  windowRefusal: null,
  trails: 0,
  trailRefusal: null,
  look: 'dots',
  lookRefusal: { glow: null, rays: null },
  glow: 'soft',
  warmDepth: 60,
  mapFull: false,
  particles: { state: 'off' },
};

/** The view choices a new run keeps (W5). */
const kept = (v: ResultsView) => ({ smooth: v.smooth, isoDb: v.isoDb, fixed: v.fixed, cumulative: v.cumulative, windowMs: v.windowMs, trails: v.trails, look: v.look, glow: v.glow, mapFull: v.mapFull });

/** The map on screen and its baseline, as decoded: the probe reads its values here. */
let shown: { map: SurfaceMap; base: SurfaceMap | null; what: string; cumulative: boolean; windowSteps: number } | null = null;
export const shownMaps = () => shown;

export const resultsViewStore = new Store<ResultsView>(OFF);

const set = (patch: Partial<ResultsView>) => resultsViewStore.set({ ...resultsViewStore.get(), ...patch });

const groupKey = (s: SurfaceMapInfo) => `${s.cutting_plane ? 'plane' : 'surface'}|${s.field ?? ''}`;
const groupLabel = (s: SurfaceMapInfo) => `${s.cutting_plane ? 'Cutting planes' : 'Surface receivers'}${s.field ? ` · ${s.field}` : ''}`;
export const bandName = (b: number | null) => (b === null ? 'all bands' : b >= 1000 ? `${b / 1000} kHz` : `${b} Hz`);
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
  set({ ...OFF, ...kept(resultsViewStore.get()), run, status: 'loading' });
  shown = null;
  showMap(null);
  showParticles(null);
  try {
    const idx = await actions.runData(run);
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
    const dtS = data.time_step_s ?? data.surfaces[0]?.time_step_s ?? null;
    Animator.reset(data.steps ?? Math.max(1, ...data.surfaces.map((s) => s.time_step_count)), dtS ? dtS * 1000 : null);
    set({ status: 'ready', data, groups, group, bands, bandHz: defaultBand(bands), baseline: defaultBaseline(run) });
  } catch (e) {
    if (!fresh(g)) return;
    const err = asCmdError(e);
    set({ status: 'error', message: `${err.message} (${err.code})` });
  }
}

/**
 * Texels (faces x steps) a map may ask of the GPU: what one texture holds (`mapLayout`), and no more
 * than 64 Mi, 256 MB of texture and as much again in the array it is filled from. A denser map
 * arrives at a coarser time bin (the backend merges steps; the header says by how much), so the
 * densest run still draws: CR4 at 27 bands with its 0.1 m plane is 87,860 faces x 10,000 steps.
 */
export const MAP_TEXELS_MAX = 64 * 1024 * 1024;

function texelBudget(): number {
  const side = resultsLayer().maxTextureSize();
  return Math.min(side * side, MAP_TEXELS_MAX);
}

async function mapBytes(run: string, path: string): Promise<SurfaceMap> {
  return decodeSurfaceMap(await actions.runSurfaceMap(run, path, texelBudget()));
}

async function loadMap(g: number): Promise<void> {
  const v = resultsViewStore.get();
  const d = v.data;
  if (!d || !v.run) return;
  const info = d.surfaces.find((s) => groupKey(s) === v.group && (s.band_hz ?? null) === v.bandHz);
  if (!info) {
    shown = null;
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
        const bi = await actions.runData(v.baseline);
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
    const now = resultsViewStore.get();
    // W2: cumulative for a level map only; a difference says why not.
    const cumRefusal = now.cumulative ? cumulativeRefusal(v.diff ? 'diff' : kind) : null;
    const cumulative = now.cumulative && cumRefusal === null;
    const win = windowOf(now.windowMs, m.timeStepS, cumulative);
    const range = kind === 'diff' && base ? diffRange(m, base) : cumulative ? cumulativeRange(m) : levelRange(m);
    const what = groupLabel(info);
    // W5: a fixed range applies to level maps; a difference keeps its own symmetric range.
    const fixed = kind === 'level' ? now.fixed : null;
    const shownRange = fixed ?? range ?? { lo: 0, hi: 1 };
    // A map stored in time bins longer than the particle step (patch 0001) is shown at bin floor(step / ratio).
    const runDt = d.time_step_s ?? null;
    const stepRatio = runDt && m.timeStepS > runDt ? Math.max(1, Math.round(m.timeStepS / runDt)) : 1;
    const err = showMap(m, { run: v.run, path: info.path, bandHz: v.bandHz, kind, range: shownRange, baseline: base ? v.baseline : null, cumulative, windowSteps: win.steps, stepRatio }, base);
    if (!fresh(g)) return;
    const layer = resultsLayer();
    layer.setLook({ smooth: now.smooth, isoDb: now.isoDb });
    renderNow();
    shown = err ? null : { map: m, base, what, cumulative, windowSteps: win.steps };
    const dtS = m.timeStepS || null;
    set({
      map: err ? null : { path: info.path, kind, cumulative, windowSteps: win.steps, dtS, range: shownRange, legend: legendOf(shownRange, kind, v.bandHz, fixed !== null, cumulative, windowLabel(win.steps, dtS)) },
      cumulativeRefusal: cumRefusal,
      windowRefusal: win.refusal,
      mapMessage: err ?? (range ? null : `No energy reached the ${what.toLowerCase()} at ${bandName(v.bandHz)}.`),
      smoothRefusal: err ? null : layer.smoothRefusal,
      baselineLabel: v.baseline ? runLabel(v.baseline) : null,
      baselineReason: reason,
    });
  } catch (e) {
    if (!fresh(g)) return;
    const err = asCmdError(e);
    shown = null;
    showMap(null);
    set({ map: null, mapMessage: `${err.message} (${err.code})` });
  }
}

/** The window a map is drawn with: the chosen ms in the map's steps, none on a cumulative map; and why, when not as chosen. */
function windowOf(ms: number, dtS: number | null | undefined, cumulative: boolean): { steps: number; refusal: string | null } {
  if (cumulative) return { steps: 1, refusal: ms > 0 ? WINDOW_CUMULATIVE_REFUSAL : null };
  return windowChoice(ms, dtS);
}

/** The legend of a map shown with `range` (a fixed one says so in its title, a window too). */
function legendOf(range: Range, kind: 'level' | 'diff', bandHz: number | null, fixed: boolean, cumulative = false, window: string | null = null) {
  const v = resultsViewStore.get();
  const what = shown?.what ?? groupLabel(v.data?.surfaces.find((s) => groupKey(s) === v.group) ?? ({ cutting_plane: false } as SurfaceMapInfo));
  const title = kind === 'diff'
    ? `Difference from ${runLabel(v.baseline as string)} · ${bandName(bandHz)}${window ? ` · ${window}` : ''}`
    : `${what} · ${cumulative ? 'cumulative level from 0 ms' : 'level'} · ${bandName(bandHz)}${window ? ` · ${window}` : ''}${fixed ? ' · fixed range' : ''}`;
  return { ...legendLabels(range, kind), gradient: legendGradient(kind), title };
}

async function loadParticles(g: number): Promise<void> {
  const v = resultsViewStore.get();
  const d = v.data;
  if (!d || !v.run) return;
  if (d.solver !== 'spps') {
    showParticles(null);
    set({ particles: { state: 'off' }, trailRefusal: 'Only an SPPS run has particles' });
    return;
  }
  if (d.particle_files.length === 0) {
    showParticles(null);
    const sources = (sceneStore.get()?.view.sources ?? []).filter((s) => s.enabled).length;
    set({ particles: { state: 'none', ...noParticlesText(d.steps ?? 0, sources) }, trailRefusal: 'No particles saved for this run' });
    return;
  }
  const band = d.particle_files.some((f) => f.freq_hz === v.bandHz) ? (v.bandHz as number) : d.particle_files[0].freq_hz;
  set({ particles: { state: 'loading', bandHz: band } });
  try {
    const p = decodeParticles(await actions.runParticles(v.run, band));
    if (!fresh(g)) return;
    showParticles(p, { run: v.run, bandHz: band, particles: p.particleCount, records: p.recordCount });
    // Playback starts where the sources emit: the first step a saved particle is alive.
    Animator.setStart(emissionStep(p));
    const layer = resultsLayer();
    layer.setTrails(resultsViewStore.get().trails);
    const look = layer.setParticleLook(resultsViewStore.get().look);
    renderNow();
    set({ particles: { state: 'shown', bandHz: band, particles: p.particleCount }, trailRefusal: layer.trailRefusal, look, lookRefusal: { glow: layer.particleLookRefusal('glow'), rays: layer.particleLookRefusal('rays') }, warmDepth: layer.gpu.depthDb() });
  } catch (e) {
    if (!fresh(g)) return;
    const err = asCmdError(e);
    showParticles(null);
    set({ particles: { state: 'error', message: `${err.message} (${err.code})` }, trailRefusal: 'The particles could not be read' });
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
  /** W5: smooth colour on or off; contours go with it. No reload: the GPU has the node list. */
  setSmooth(on: boolean): void {
    const isoDb = on ? resultsViewStore.get().isoDb : 0;
    set({ smooth: on, isoDb });
    resultsLayer().setLook({ smooth: on, isoDb });
    renderNow();
  },
  /** W5: contours every `db` dB (0 for none), drawn on the smoothed level only. */
  setContours(db: number): void {
    const v = resultsViewStore.get();
    if (!v.smooth && db > 0) return;
    set({ isoDb: db });
    resultsLayer().setLook({ smooth: v.smooth, isoDb: db });
    renderNow();
  },
  /** W5: a fixed colour range for level maps (null: each map's own), kept across bands and runs. */
  setFixedRange(range: Range | null): void {
    set({ fixed: range });
    const v = resultsViewStore.get();
    if (!v.map || v.map.kind !== 'level' || !shown) return;
    const r = range ?? (shown.cumulative ? cumulativeRange(shown.map) : levelRange(shown.map)) ?? { lo: 0, hi: 1 };
    resultsLayer().setRange(r);
    renderNow();
    set({ map: { ...v.map, range: r, legend: legendOf(r, 'level', v.bandHz, range !== null, shown.cumulative, windowLabel(v.map.windowSteps, v.map.dtS)) } });
  },
  /** W2: the cumulative map on or off; the map reloads (its texture is the running sum). */
  setCumulative(on: boolean): void {
    set({ cumulative: on });
    fire();
  },
  /** The time window, `ms` of sound (0: each step on its own), kept across bands and runs; no reload, one uniform. */
  setWindow(ms: number): void {
    set({ windowMs: ms });
    const v = resultsViewStore.get();
    if (!v.map || !shown) return;
    const win = windowOf(ms, v.map.dtS, v.map.cumulative);
    resultsLayer().setWindow(win.steps);
    renderNow();
    shown = { ...shown, windowSteps: win.steps };
    const label = windowLabel(win.steps, v.map.dtS);
    set({ windowRefusal: win.refusal, map: { ...v.map, windowSteps: win.steps, legend: legendOf(v.map.range, v.map.kind, v.bandHz, v.map.kind === 'level' && v.fixed !== null, v.map.cumulative, label) } });
  },
  /** B2: particles as dots (per record), glowing with fading trails, or their rays (GPU); no reload. */
  setLook(look: ParticleLook): void {
    const layer = resultsLayer();
    if (layer.particleLookRefusal(look)) return;
    set({ look: layer.setParticleLook(look) });
    renderNow();
  },
  /** Round 2: the map at full strength while the light plays, or faded to 25 %. */
  setMapFull(full: boolean): void {
    set({ mapFull: full });
    setMapWhilePlaying(full);
  },
  /** Decision 69: the light's bloom, off (no bloom pass), soft or full. */
  setGlow(g: Glow): void {
    set({ glow: g });
    setGlow(g);
  },
  /** W3: trails `steps` long (0 for none), over the particles shown; no reload. */
  setTrails(steps: number): void {
    const layer = resultsLayer();
    if (steps > 0 && layer.trailRefusal) return;
    set({ trails: steps });
    layer.setTrails(steps);
    renderNow();
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
      shown = null;
      showMap(null);
      showParticles(null);
      resultsViewStore.set({ ...OFF, ...kept(resultsViewStore.get()) });
    } else fire();
  });
  fire();
}
