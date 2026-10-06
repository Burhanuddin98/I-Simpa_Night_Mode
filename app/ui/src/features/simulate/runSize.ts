// How much the solver would hold in memory for a run, before it is launched (Burhan 2026-10-06 06:39: "HOW CAN
// ALL THE HEAVIEST FEATURES WORK WITHOUT KILLING THE FUCKING APP AGAIN AND AGAIN BECAUSE OF FUCKING MEMORY
// ISSUES"; 15:44: "its literally just data man, raw data"; docs/investigations/2026-10-06-third-octave-bug/
// MEMORY.md, COMPACT.md, SPARSE.md, BED-0002.md).
//
// Our SPPS build (patches 0001 and 0002) keeps a sound-map cell's time series sparse: only the (step, energy)
// records a particle left, 9 bytes each with the vectors' growth, switching a cell to a dense float32 array once
// more than an eighth of its steps hold energy. So a band's maps cost the smaller of two bounds:
//   the grid:  cells x bins x 4 bytes            (every cell dense; what upstream's solver always allocated)
//   the data:  particles x sources x crossings x 9 bytes   (a record for every cell-step a particle crossed)
// Crossings a particle makes over the run: 32, realised on CR4 (0.1 m plane, 18 bands, 300,000 particles a
// source, 10 s): 19 million cell-steps a band for 600,000 particles; that run's peak was 3.06 GB against this
// model's 3.1 GB (BED-0002.md). One room, one duration: a forecast calibrated once, not a law. cells are every
// enabled plane's cells plus every enabled surface-group receiver's faces; bins = ceil(steps / ratio) with ratio
// the sound-map time step in particle steps (patch 0001, 1 when unset); bands = every band computed (the
// allocation does not look at "sound maps per band"). The old solver's 74 GB for the 0.1 m whole-room plane is
// the grid bound; the data bound for it is about 1.5 GB, which is what the sparse solver holds.
// The limits are a stop-gap against this machine's 32 GB until the app can read the machine's memory: refuse
// from REFUSE_GB, warn from WARN_GB.
import type { SceneState } from '../../bindings/ipc.ts';
import { planeCells } from '../../chrome/planes.ts';
import { projectBlockers } from '../../flow.ts';
import { Store, type SolverName } from '../../store.ts';
import type { ProjectSettings } from './model.ts';

/** The project's settings as last read (App.tsx refreshes it on every scene change): the cube needs them. */
export const settingsStore = new Store<ProjectSettings | null>(null);

export const RESULTS_TOO_BIG = 'RESULTS_TOO_BIG';
export const REFUSE_GB = 24;
export const WARN_GB = 8;
/** A dense cell-step: `t_cell` is `decimal` = `float` (`coreTypes.h:255`, `mathlib.h:54`). */
export const BYTES_PER_VALUE = 4;
/** A sparse record: a `uint16_t` step and a `float` energy, with the vectors' growth (`SparseTimeSeries`, patch 0002). */
export const BYTES_PER_RECORD = 9;
/** Cell-steps one particle crosses over a run: realised on CR4 at 10 s (BED-0002.md), the model's one calibration. */
export const CROSSINGS_PER_PARTICLE = 32;
/** A cell goes dense past this share of its steps (`SparseTimeSeries::denseThreshold`). */
export const DENSE_SHARE = 1 / 8;

export interface ResultCube {
  cells: number;
  /** Particle steps, ceil(duration / time step). */
  steps: number;
  /** Particle steps per sound-map time bin, as the solver rounds it (1: every step its own bin). */
  ratio: number;
  /** Sound-map time bins, ceil(steps / ratio): the time axis the maps are stored on. */
  bins: number;
  bands: number;
  /** Cell-steps with energy forecast per band: the smaller of the grid and the data bounds. */
  records: number;
  /** Whether the forecast is the sparse form (records x 9 bytes) or the dense one (cells x bins x 4). */
  sparse: boolean;
  bytes: number;
  gb: number;
}

const num = (v: unknown): number | null => (typeof v === 'number' && Number.isFinite(v) ? v : null);

/**
 * The solver's bin ratio: `floor(surfaceStep / timeStep + 0.5)` in float, at least 1, and 1 when the sound-map
 * step is unset or not above the time step (patch 0001, `base_core_configuration.cpp`).
 */
export function mapStepRatio(timeStepS: unknown, mapStepS: unknown): number {
  const dt = num(timeStepS);
  const map = num(mapStepS);
  if (dt === null || map === null || !(dt > 0) || !(map > dt)) return 1;
  const r = Math.floor(Math.fround(Math.fround(map) / Math.fround(dt)) + 0.5);
  return r >= 1 && Number.isFinite(r) ? r : 1;
}

/** The result cube of an SPPS run on `scene` as set now; null for TCR (no time steps) or settings that do not parse. */
export function resultCube(
  scene: Pick<SceneState, 'view' | 'groups'> | null,
  solver: SolverName,
  settings: ProjectSettings | null = settingsStore.get(),
): ResultCube | null {
  if (!scene || solver !== 'spps' || !settings) return null;
  const v = scene.view;
  const s = settings.solvers.spps;
  const dt = num(s.time_step_s);
  const dur = num(s.duration_s);
  if (dt === null || dur === null || !(dt > 0) || !(dur > 0)) return null;
  const steps = Math.ceil(dur / dt);
  const ratio = mapStepRatio(dt, s.map_time_step_s ?? null);
  const bins = Math.ceil(steps / ratio);
  const bands = s.bands_computed.filter(Boolean).length;
  const facesOf = new Map(scene.groups.map((g) => [g.id, g.faces] as const));
  let cells = 0;
  for (const r of v.surface_receivers) {
    if (!r.enabled) continue;
    if (r.shape.kind === 'cutting_plane') {
      const g = planeCells(r.shape.a, r.shape.b, r.shape.c, r.shape.resolution_m);
      if (g) cells += g.cells;
    } else {
      for (const id of r.shape.groups) cells += facesOf.get(id) ?? 0;
    }
  }
  const particles = num(s.particles_per_source) ?? 0;
  const sources = v.sources.filter((x) => x.enabled).length;
  const grid = cells * bins;
  const data = particles * sources * CROSSINGS_PER_PARTICLE;
  const records = Math.min(grid, data);
  // The cells go dense when their share of steps with energy passes an eighth: then the grid bound is the cost.
  const sparse = cells > 0 && records / cells < bins * DENSE_SHARE;
  const perBand = sparse ? records * BYTES_PER_RECORD : grid * BYTES_PER_VALUE;
  const bytes = perBand * bands;
  return { cells, steps, ratio, bins, bands, records, sparse, bytes, gb: bytes / 2 ** 30 };
}

/** "about 149 GB", "about 2.4 GB", "about 120 MB". */
export function cubeText(c: ResultCube): string {
  if (c.gb >= 10) return `about ${Math.round(c.gb)} GB`;
  if (c.gb >= 1) return `about ${c.gb.toFixed(1)} GB`;
  const mb = c.bytes / 2 ** 20;
  return mb >= 1 ? `about ${Math.round(mb)} MB` : 'under 1 MB';
}

/** The run's blockers with the size one added when the cube is over REFUSE_GB; null with no project, as projectBlockers. */
export function blockersWithSize(scene: SceneState | null, solver: SolverName, settings: ProjectSettings | null = settingsStore.get()): string[] | null {
  const out = projectBlockers(scene, solver);
  if (out === null) return null;
  const c = resultCube(scene, solver, settings);
  if (c && c.gb >= REFUSE_GB && !out.includes(RESULTS_TOO_BIG)) out.push(RESULTS_TOO_BIG);
  return out;
}
