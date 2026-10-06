// How much the solver would hold in memory for a run, before it is launched (Burhan 2026-10-06 06:39: "HOW CAN
// ALL THE HEAVIEST FEATURES WORK WITHOUT KILLING THE FUCKING APP AGAIN AND AGAIN BECAUSE OF FUCKING MEMORY
// ISSUES"; docs/investigations/2026-10-06-third-octave-bug/MEMORY.md). SPPS keeps every surface receiver's
// energy for every time step, band and source for the whole run: a sound-level plane of 33 x 33 m at 0.1 m
// (110,888 cells) over 10 s at 1 ms, 18 bands and 2 sources is 298 GB, and the solver aborts 15 s in.
//
// The model: cube = cells x steps x bands x sources x 8 bytes (SPPS accumulates in `l_decimal`, a double), where cells are every enabled plane's cells plus
// every enabled surface-group receiver's faces, bands count only with sound maps per band, sources only with an
// echogram per source. The cube alone, with no solver overhead: a floor, not a measurement (no sampled run
// has calibrated it yet). The limits are a stop-gap against this machine's 32 GB until the app can read the
// machine's memory: refuse from REFUSE_GB, warn from WARN_GB.
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

export interface ResultCube {
  cells: number;
  steps: number;
  bands: number;
  sources: number;
  bytes: number;
  gb: number;
}

const num = (v: unknown): number | null => (typeof v === 'number' && Number.isFinite(v) ? v : null);

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
  const bandsOn = s.bands_computed.filter(Boolean).length;
  const sourcesOn = v.sources.filter((x) => x.enabled).length;
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
  const bands = s.sound_maps_per_band ? bandsOn : 1;
  const sources = s.echogram_per_source ? sourcesOn : 1;
  const bytes = cells * steps * bands * sources * 8;
  return { cells, steps, bands, sources, bytes, gb: bytes / 2 ** 30 };
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
