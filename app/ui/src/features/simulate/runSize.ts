// How much the solver would hold in memory for a run, before it is launched (Burhan 2026-10-06 06:39: "HOW CAN
// ALL THE HEAVIEST FEATURES WORK WITHOUT KILLING THE FUCKING APP AGAIN AND AGAIN BECAUSE OF FUCKING MEMORY
// ISSUES"; docs/investigations/2026-10-06-third-octave-bug/MEMORY.md and COMPACT.md). SPPS allocates, at the
// start of the run, one float32 per sound-map cell per time bin per computed band, for the whole run
// (`coreTypes.h:255-305` `r_SurfCut::Init`, `coreinitialisation.cpp:266`; the surface-group receivers' faces the
// same, `r_Surf_Face::InitFreq`). A sound-level plane of 33 x 33 m at 0.1 m (110,888 cells) over 10 s at 1 ms and
// 18 bands is 74 GB, and the solver aborts 15 s in while zero-filling it.
//
// The model: cube = cells x bins x bands x 4 bytes, where cells are every enabled plane's cells plus every enabled
// surface-group receiver's faces; bins = ceil(steps / ratio) with ratio the sound-map time step in particle steps
// (patch 0001: `recepteurs_surfaciques_pas_temps`, 1 when unset); bands = every band computed (the allocation
// does not look at "sound maps per band"). Sources do not multiply it: one cube serves every source. The 14:45
// correction: the 07:30 reading of "double" was the point receivers' accumulator (`l_decimal`), not the maps'.
// The cube alone, with no solver overhead: a floor, not a measurement (no sampled run has calibrated it yet). The
// limits are a stop-gap against this machine's 32 GB until the app can read the machine's memory: refuse from
// REFUSE_GB, warn from WARN_GB.
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
/** `t_cell` is `decimal` = `float` (`coreTypes.h:255`, `mathlib.h:54`). */
export const BYTES_PER_VALUE = 4;

export interface ResultCube {
  cells: number;
  /** Particle steps, ceil(duration / time step). */
  steps: number;
  /** Particle steps per sound-map time bin, as the solver rounds it (1: every step its own bin). */
  ratio: number;
  /** Sound-map time bins, ceil(steps / ratio): the time axis the maps are stored on. */
  bins: number;
  bands: number;
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
  const bytes = cells * bins * bands * BYTES_PER_VALUE;
  return { cells, steps, ratio, bins, bands, bytes, gb: bytes / 2 ** 30 };
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
