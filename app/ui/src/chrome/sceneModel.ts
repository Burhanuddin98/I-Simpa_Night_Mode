// The logic behind the scene package's chrome (PLAN.md 6.3): the step subs, the effective
// material of a group, the model check's rows, the Run blockers' text, the scene filter and the
// number spellings. A pure module: no store, no backend, only erasable TypeScript, tested by
// sceneModel.test.ts under `node --test` (PLAN.md 2.4, rule 8).
import type {
  CheckSummary,
  Directivity,
  Material,
  ProjectInfo,
  ProjectView,
  SceneState,
  SpectrumShape,
  UiIssue,
} from '../bindings/ipc.ts';
import { nextName } from '../ops.ts';

/** A number as the schema stores it: finite values as numbers, non-finite ones as strings. */
export type F64 = number | string;
export type Vec3 = [F64, F64, F64];

// ---- step bar --------------------------------------------------------------------------------

export type SubKey = 'geometry' | 'materials' | 'sources' | 'simulate' | 'results';

/**
 * The step bar's subs: Geometry `closed`, `refused` or `no model`; Materials
 * `<groups assigned> / <surface groups>`; Sources & receivers `<enabled sources> · <point
 * receivers>`; Simulate and Results empty (M11 and M12 fill them).
 */
/**
 * Which steps are complete, for the step bar's lights (hardware.css H1): geometry when the model check
 * passes, materials when every group has a material, sources when a source is on and a receiver exists.
 * Simulate and Results are the run's business (StepBar reads the runs store for those).
 */
export function stepsDone(scene: SceneState | null): Record<'geometry' | 'materials' | 'sources', boolean> {
  if (!scene) return { geometry: false, materials: false, sources: false };
  return {
    geometry: scene.check?.verdict === 'ok',
    materials: scene.info.surface_groups > 0 && scene.info.groups_assigned === scene.info.surface_groups,
    sources: scene.view.sources.some((s) => s.enabled) && scene.view.point_receivers.length > 0,
  };
}

export function stepSubs(scene: SceneState | null): Record<SubKey, string> {
  const check = scene?.check;
  // In words (the UI study's increment 2): "room closed", "6 of 10 set", "2 sources · 5 receivers".
  const geometry = !check ? 'no model' : check.verdict === 'ok' ? 'room closed' : 'not closed';
  if (!scene) return { geometry, materials: '', sources: '', simulate: '', results: '' };
  const enabled = scene.view.sources.filter((s) => s.enabled).length;
  const receivers = scene.view.point_receivers.length;
  return {
    geometry,
    materials: `${scene.info.groups_assigned} of ${scene.info.surface_groups} set`,
    sources: `${enabled} ${enabled === 1 ? 'source' : 'sources'} · ${receivers} ${receivers === 1 ? 'receiver' : 'receivers'}`,
    simulate: '',
    results: '',
  };
}

// ---- materials -------------------------------------------------------------------------------

/** The id of `group`'s material under the active variant: its override there, else the base. */
export function effectiveMaterialId(view: ProjectView, group: string): string | null {
  const g = view.surface_groups.find((x) => x.id === group);
  if (!g) return null;
  const variant = view.active_variant ? view.variants.find((v) => v.id === view.active_variant) : undefined;
  return variant?.overrides.find((o) => o.group === group)?.material ?? g.material;
}

export function effectiveMaterial(view: ProjectView, group: string): Material | null {
  const id = effectiveMaterialId(view, group);
  return id === null ? null : (view.materials.find((m) => m.id === id) ?? null);
}

/** The active variant's name, or `Baseline` (the base project). */
export function variantName(view: ProjectView | null | undefined): string {
  if (!view?.active_variant) return 'Baseline';
  return view.variants.find((v) => v.id === view.active_variant)?.name ?? 'Baseline';
}

/** `Variant 1`, `Variant 2`, ...: the first name no variant has. */
export function nextVariantName(view: ProjectView): string {
  return nextName('Variant ', view.variants.map((v) => v.name));
}

// ---- the model check -------------------------------------------------------------------------

export type CheckState = 'OK' | 'FAIL';
export interface CheckRow {
  key: string;
  label: string;
  value: string;
  /** Null for a row that states a fact without passing or failing (the units). */
  state: CheckState | null;
}

const has = (check: CheckSummary, code: string) => check.reasons.some((r) => r.code === code);

/** Closed: nothing open on the outer shell, and a volume is enclosed. */
export function isClosed(check: CheckSummary): boolean {
  return check.counts.open_edges === 0 && !has(check, 'open_boundary') && !has(check, 'no_enclosed_volume');
}

/** The reasons the four named rows already cover. */
const NAMED_REASONS = ['open_boundary', 'no_enclosed_volume', 'self_intersections', 'inverted_faces'];

/**
 * The Geometry panel's model check list, as text: Closed volume, Self-intersections, Flipped
 * normals, Open edges and Units, then one row per other reason the check refused with.
 * `units` is the Units row's text.
 */
export function checkRows(check: CheckSummary, units: string): CheckRow[] {
  const c = check.counts;
  const closed = isClosed(check);
  const selfInt = has(check, 'self_intersections') || c.self_intersecting_pairs > 0;
  const flipped = has(check, 'inverted_faces') || c.inverted_faces > 0;
  const open = has(check, 'open_boundary') || c.open_edges > 0;
  const rows: CheckRow[] = [
    { key: 'closed', label: 'Closed volume', value: closed ? 'yes' : 'no', state: closed ? 'OK' : 'FAIL' },
    {
      key: 'self_intersections',
      label: 'Self-intersections',
      value: c.self_intersecting_pairs > 0 ? `${c.self_intersecting_pairs} pairs · ${c.self_intersecting_faces} faces` : '0',
      state: selfInt ? 'FAIL' : 'OK',
    },
    { key: 'flipped', label: 'Flipped normals', value: String(c.inverted_faces), state: flipped ? 'FAIL' : 'OK' },
    { key: 'open_edges', label: 'Open edges', value: String(c.open_edges), state: open ? 'FAIL' : 'OK' },
    { key: 'units', label: 'Units', value: units, state: null },
  ];
  for (const r of check.reasons) {
    if (NAMED_REASONS.includes(r.code)) continue;
    rows.push({ key: r.code, label: r.code, value: `${r.count} · ${r.faces} faces`, state: 'FAIL' });
  }
  return rows;
}

/**
 * The Units row. A project is stored in metres; an imported one that has never been saved had
 * its file's unit chosen in the import dialog (never guessed).
 */
export function unitsText(info: Pick<ProjectInfo, 'path'>): string {
  return info.path ? 'm (project file)' : 'm (chosen at import)';
}

/** The file a project came from: the saved path's last part, else the project's name. */
export function fileLabel(info: Pick<ProjectInfo, 'path' | 'name'>): string {
  return info.path ? (info.path.split(/[\\/]/).pop() ?? info.name) : info.name;
}

// ---- Run -------------------------------------------------------------------------------------

const BLOCKERS: Record<string, string> = {
  GEOMETRY_REFUSED: 'the model check refused the geometry',
  MATERIALS_UNASSIGNED: 'some surface groups have no material yet',
  SOLVER_NOT_FOUND: 'a solver executable was not found (set SIMPA_SOLVERS_DIR to the solver build)',
  SOLVER_UNVERIFIED: 'a solver executable is not the verified build (solvers/manifest.json)',
  RUN_ACTIVE: 'a run is active: cancel it first',
  RESULTS_TOO_BIG: 'the results would not fit in memory: fewer cells on the sound-level planes, fewer bands, or a coarser time step',
};

/** One blocker as a line of text: its code, then what it means. */
export function blockerText(code: string): string {
  const why = BLOCKERS[code];
  return why ? `${code}: ${why}` : code;
}

/** The Run button's tooltip: every blocker as text, or what Run does when there is none. */
export function runTooltip(blockers: readonly string[] | null): string {
  if (blockers === null) return 'Run is disabled: open a project first';
  if (blockers.length === 0) return 'Run the solver on the saved project (F5)';
  return ['Run is disabled:', ...blockers.map(blockerText)].join('\n');
}

// ---- issues ----------------------------------------------------------------------------------

/** `error` if any issue is an error, `warning` if all are warnings, null if there are none. */
export function worstSeverity(issues: readonly UiIssue[]): 'error' | 'warning' | null {
  if (issues.some((i) => i.severity === 'error')) return 'error';
  return issues.length ? 'warning' : null;
}

/** The lists joined, each issue once (the same code at the same pointer is the same issue). */
export function uniqueIssues(...lists: (readonly UiIssue[])[]): UiIssue[] {
  const seen = new Set<string>();
  const out: UiIssue[] = [];
  for (const list of lists) {
    for (const i of list) {
      const k = `${i.code}\u0000${i.path}\u0000${i.message}`;
      if (seen.has(k)) continue;
      seen.add(k);
      out.push(i);
    }
  }
  return out;
}

/** A validator message as a sentence: capitalised, with a closing full stop. */
export function sentence(text: string): string {
  const t = text.trim();
  if (!t) return t;
  const s = t[0].toUpperCase() + t.slice(1);
  return /[.!?]$/.test(s) ? s : `${s}.`;
}

// ---- groups (scope row 15 (1)) -----------------------------------------------------------------

/** The menu entry of G19, in the viewport's context menu and the Edit menu. */
export const REGROUP_LABEL = 'New group from selection';

/**
 * The faces "New group from selection" sends (G19): a viewport pick or fill, each face once and
 * ascending; `null` for any other selection (a whole group is already a group), so the entry is
 * disabled.
 */
export function regroupFaces(selection: { kind: string; faces?: readonly number[]; [key: string]: unknown }): number[] | null {
  if (selection.kind !== 'faces' || !selection.faces?.length) return null;
  return [...new Set(selection.faces)].sort((a, b) => a - b);
}

/**
 * A point receiver's folder (M37): the path of the receiver groups an imported `.proj` put it in
 * (`Stalls / Front`), or '' at the top level. Read-only: groups are made in upstream's GUI.
 */
export function receiverFolder(receiver: { group?: string | null }): string {
  return receiver.group ?? '';
}

// ---- filter ----------------------------------------------------------------------------------

/**
 * The scene filter: every whitespace-separated term of `query` appears, ignoring case, in one of
 * `texts`. An empty query matches everything.
 */
export function matchesFilter(query: string, ...texts: string[]): boolean {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (!terms.length) return true;
  const hay = texts.join('\u0000').toLowerCase();
  return terms.every((t) => hay.includes(t));
}

// ---- numbers ---------------------------------------------------------------------------------

/**
 * A geometry fact with at most `digits` decimals and no trailing zeros (`180`, `4001.8`); an
 * em dash for a value that is missing or not finite (the backend sends those as null).
 */
export function fact(v: number | null | undefined, digits: number): string {
  if (typeof v !== 'number' || !Number.isFinite(v)) return '—';
  const s = v.toFixed(digits);
  return s.includes('.') ? s.replace(/\.?0+$/, '') : s;
}

/** A coordinate for a list, with `digits` decimals; a non-finite value as the schema spells it. */
export function coord(v: F64, digits: number): string {
  return typeof v === 'number' ? v.toFixed(digits) : v;
}

/** A value for an edit field: exact, never rounded (`-0` keeps its sign). */
export function exact(v: F64): string {
  if (typeof v !== 'number') return v;
  return Object.is(v, -0) ? '-0' : String(v);
}

export const AXES = ['x', 'y', 'z'] as const;
export type Axis = (typeof AXES)[number];

/** `position` with one axis replaced; the other two are passed on exactly as stored. */
export function withAxis(position: Vec3, axis: Axis, value: number): Vec3 {
  const out: Vec3 = [position[0], position[1], position[2]];
  out[AXES.indexOf(axis)] = value;
  return out;
}

/**
 * Where + Source and + Receiver put a new point: the centre of the model's bounding box, `lift`
 * metres above its lowest point. Null without a usable bounding box. A point that falls outside
 * the room (an L-shaped hall) is refused by the checked apply, and the project is left unchanged.
 */
export function roomCentre(check: CheckSummary | null | undefined, lift: number): [number, number, number] | null {
  if (!check) return null;
  const [x0, y0, z0] = check.bbox_min;
  const [ex, ey, ez] = check.extents_m;
  const p: [number, number, number] = [x0 + ex / 2, y0 + ey / 2, z0 + Math.min(lift, ez / 2)];
  return p.every((v) => typeof v === 'number' && Number.isFinite(v)) ? p : null;
}

// ---- emission (read-only in M10, PLAN.md 7.5 point 6) ------------------------------------------

export function spectrumName(shape: SpectrumShape): string {
  switch (shape.kind) {
    case 'pink':
      return 'Pink noise';
    case 'white':
      return 'White noise';
    case 'custom':
      return 'Custom spectrum';
  }
}

export function directivityName(d: Directivity): string {
  switch (d.kind) {
    case 'omni':
      return 'Omni';
    case 'unidirectional':
      return 'Unidirectional';
    case 'plane_xy':
      return 'Plane XY';
    case 'plane_yz':
      return 'Plane YZ';
    case 'plane_xz':
      return 'Plane XZ';
    case 'balloon':
      return `Balloon (${d.file})`;
  }
}

/** A source's sound power as stored, in dB (an input, shown inside `[data-input]`). */
export function powerText(globalDb: F64): string {
  return `${exact(globalDb)} dB`;
}

/**
 * A surface group's or material's name as the scene list shows it (Burhan 2026-10-06): the import
 * prefix `mat_<room>_` dropped, underscores and camelCase split into words, sentence case
 * ("mat_CR4_whitePanels" reads "White panels", "door_room1" "Door room 1"). On screen only: the
 * project keeps its names, and the row's tooltip shows the stored one.
 */
export function displayName(raw: string): string {
  const words = raw
    .replace(/^mat_[A-Za-z0-9]+_/, '')
    .replace(/_/g, ' ')
    .replace(/([a-z])([A-Z])/g, '$1 $2')
    .replace(/([A-Za-z])(\d)/g, '$1 $2')
    .replace(/\s+/g, ' ')
    .trim()
    .toLowerCase();
  return words ? words.charAt(0).toUpperCase() + words.slice(1) : raw;
}
