// The simulate package's pure logic (docs/investigations/2026-09-29-m11/PLAN.md 3.3, 9.1): the
// Simulate step's sub, the Run label, the running block's head, the elapsed m:ss, the "Before
// running" rows, the settings as drawn, the last run's summary and the Results step's state.
// A pure module: no store, no backend, only erasable TypeScript, tested by model.test.ts under
// `node --test`.
//
// Numbers. A run's diagnostics (loss, limit) are the backend's own strings, printed as they
// came. The one number this module shapes is SPPS's progress, which it takes from the `#` line's
// text by string and integer operations only (`progressDisplay`), never by formatting a float.
// Settings are inputs, shown inside `[data-input]` exactly as stored (a time step in ms).
import type { Advice, CheckSummary, ProjectInfo, ReasonUi, ResultsState, RunRow, RunsView, Setting, SolversStatus } from '../../bindings/ipc.ts';
import type { BandSet, Environment, SolverSettings, Variant } from '../../bindings/schema.ts';
import { RUN_ACTIVE, statusWord } from '../../flow.ts';
import type { ActiveRun, LinePart, SolverName } from '../../store.ts';

/** A number as the schema stores it: finite values as numbers, non-finite ones as strings. */
type F64 = number | string;

/** A value exactly as stored: the shortest round-trip spelling, never rounded. */
function exact(v: F64): string {
  return typeof v === 'number' ? String(v) : v;
}

/** Joins parts into the text a reader sees (the Run button's title, a hook, a test). */
export function partsText(parts: readonly LinePart[]): string {
  return parts.map((p) => p.text).join('');
}

// ---- progress ------------------------------------------------------------------------------------

/** A plain decimal's integer and fraction digits, leading zeros of the integer dropped. */
function plainDecimal(text: string): { int: string; frac: string } | null {
  const m = /^(\d+)(?:\.(\d*))?$/.exec(text);
  if (m) return { int: m[1].replace(/^0+(?=\d)/, ''), frac: m[2] ?? '' };
  // An exponent form (`1.667e-05`, `1e+02`): the decimal point moved, by string, not by float.
  const e = /^(\d+)(?:\.(\d*))?[eE]([+-]?\d+)$/.exec(text);
  if (!e) return null;
  const digits = e[1] + (e[2] ?? '');
  const shift = Number(e[3]);
  if (!Number.isSafeInteger(shift) || Math.abs(shift) > 64) return null;
  const point = e[1].length + shift;
  let int: string;
  let frac: string;
  if (point <= 0) {
    int = '0';
    frac = '0'.repeat(-point) + digits;
  } else if (point >= digits.length) {
    int = digits + '0'.repeat(point - digits.length);
    frac = '';
  } else {
    int = digits.slice(0, point);
    frac = digits.slice(point);
  }
  return { int: int.replace(/^0+(?=\d)/, ''), frac };
}

/**
 * SPPS's progress text (after its `#`, to 4 significant digits) as the UI shows it: as printed
 * when it has at most two decimals (`25.2`, `7`, `100`); else **rounded half up to two decimals
 * in integers**, never through a float (`25.229` shows `25.23`, `0.0006667` shows `0.00`); an
 * exponent form is expanded first by moving the point (`1.667e-05` shows `0.00`). That is the
 * rule m11-h proves a `progress_pct` span by (e2e/lib/acoustic.ts `progressProven`: one of the
 * run's `#` lines rounded half up to the digits shown). `null` for text that is not a
 * percentage from 0 to 100 (`-`, empty, `101`): the caller then shows no number. The result
 * always matches the `progress_pct` grammar of M11 PLAN.md 4.2, `^\d{1,3}(\.\d{1,2})?$` before
 * its ` %`.
 */
export function progressDisplay(text: string): string | null {
  const d = plainDecimal(text.trim());
  if (!d) return null;
  let { int, frac } = d;
  if (frac.length > 2) {
    // Hundredths, half up: the third decimal decides, since the rest only adds less than one.
    const h = BigInt(int + frac.slice(0, 2)) + (frac[2] >= '5' ? 1n : 0n);
    int = String(h / 100n);
    frac = String(h % 100n).padStart(2, '0');
  }
  // Above 100 is not a percentage of a run; `100.5` would be SPPS misprinting, not progress.
  if (int.length > 3 || (int.length === 3 && (int !== '100' || /[1-9]/.test(frac)))) return null;
  return frac ? `${int}.${frac}` : int;
}

/** The progress of the active run as a diagnostic part, when SPPS has printed one. */
function progressPart(run: ActiveRun): LinePart | null {
  const p = progressDisplay(run.progressText);
  if (p === null) return null;
  return { text: `${p} %`, diagnostic: 'progress_pct', run: run.run ?? '' };
}

// ---- the Simulate sub and the Run label -----------------------------------------------------------

/** The newest run of the project that is not running: the one the idle block and the sub show. */
export function latestRun(runs: RunsView | null): RunRow | null {
  let best: RunRow | null = null;
  for (const r of runs?.rows ?? []) {
    if (r.status === 'RUNNING') continue;
    if (!best || r.number > best.number) best = r;
  }
  return best;
}

/**
 * The Simulate step's sub as parts: while running, the progress (`25.22 %`, in a `progress_pct`
 * diagnostic part), else `running` or `cancelling`; when idle, `run <n> <status>` of the newest
 * run (`run 3 OK`, `run 2 Cancelled`); else nothing.
 */
export function simulateSubParts(run: ActiveRun | null, runs: RunsView | null): LinePart[] {
  if (run) {
    if (run.status === 'cancelling') return [{ text: 'cancelling' }];
    const p = progressPart(run);
    return [p ?? { text: 'running' }];
  }
  const last = latestRun(runs);
  return last ? [{ text: `run ${last.number} ${statusWord(last.status)}` }] : [];
}

/**
 * The Simulate step's sub as text (the step bar's slot, M11 PLAN.md 3.3): `simulateSubParts`
 * joined. The step bar should render the parts, so the percentage sits in its diagnostic span.
 */
export function simulateSub(run: ActiveRun | null, runs: RunsView | null): string {
  return partsText(simulateSubParts(run, runs));
}

/** `Run SPPS`, `Run TCR`. */
export function solverLabel(solver: SolverName): string {
  return solver === 'tcr' ? 'TCR' : 'SPPS';
}

/**
 * The Run button's label (design:43, `runLabel`): `Run SPPS` or `Run TCR` when idle; while a
 * run is active `Running <p> %` (the percentage a diagnostic part), `Running…` before SPPS has
 * printed one (and for TCR, which prints none), `Cancelling…` once Cancel was pressed.
 */
export function runLabel(run: ActiveRun | null, solver: SolverName): LinePart[] {
  if (!run) return [{ text: `Run ${solverLabel(solver)}` }];
  if (run.status === 'cancelling') return [{ text: 'Cancelling…' }];
  const p = progressPart(run);
  return p ? [{ text: 'Running ' }, p] : [{ text: 'Running…' }];
}

// ---- the running block ----------------------------------------------------------------------------

const STAGE_WORDS: Record<string, string> = {
  solvers: 'Checking the solvers…',
  geometry: 'Checking the project…',
  validate: 'Checking the project…',
  mesh: 'Meshing…',
  export: 'Preparing the solver…',
  pre_launch: 'Preparing the solver…',
};

/**
 * The running block's head (design:411): what the run is doing, from its stage. During the
 * solve: `Solving · <p> %` once SPPS has printed a percentage, else `Solving…`.
 */
export function runningHead(run: ActiveRun): LinePart[] {
  if (run.status === 'cancelling') return [{ text: 'Cancelling…' }];
  if (run.status === 'starting' || run.stage === null) return [{ text: 'Starting…' }];
  if (run.stage === 'solve') {
    const p = progressPart(run);
    return p ? [{ text: 'Solving · ' }, p] : [{ text: 'Solving…' }];
  }
  return [{ text: STAGE_WORDS[run.stage] ?? `${run.stage}…` }];
}

/** The progress bar's fill, 0 to 100, from the core's parsed value; 0 before any. */
export function progressFill(run: ActiveRun): number {
  const p = run.progress;
  if (typeof p !== 'number' || !Number.isFinite(p)) return 0;
  return Math.min(100, Math.max(0, p));
}

/**
 * Elapsed wall time as `m:ss` (`h:mm:ss` from an hour on): no unit, so it is never read as a
 * computed acoustic number, and it is the UI's own clock, not a manifest value (the Runs tab
 * shows `run.json`'s solver time once the run has ended).
 */
export function elapsedText(ms: number): string {
  const total = Math.max(0, Math.floor((Number.isFinite(ms) ? ms : 0) / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const ss = String(s).padStart(2, '0');
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
}

// ---- "Before running" -----------------------------------------------------------------------------

export type PreflightState = 'OK' | 'FAIL' | 'UNCHECKED';

export interface PreflightRow {
  key: string;
  label: string;
  state: PreflightState;
  /** The UI codes that fail this row, as the Run button's `data-blockers` spells them. */
  codes: string[];
  /** A short fact beside the codes (`3 of 10 surface groups have none`), or empty. */
  detail: string;
}

/**
 * The design's five rows (design:676-682), then the solvers (M11 PLAN.md 3.3), each with the UI
 * codes that fail it. A project blocker no row names goes to a last row, "Every other check
 * passes", shown only when it fails: the list never reads all OK while Run is blocked.
 */
export const PREFLIGHT: readonly { key: string; label: string; codes: readonly string[] }[] = [
  { key: 'materials', label: 'Every surface has a material', codes: ['MATERIALS_UNASSIGNED', 'MATERIAL_UNASSIGNED'] },
  { key: 'model', label: 'Model closed, no self-intersections', codes: ['GEOMETRY_REFUSED'] },
  { key: 'source', label: 'Source inside the room', codes: ['SOURCE_NONE', 'SOURCE_OUTSIDE', 'SOURCE_NEAR_SURFACE'] },
  {
    key: 'receivers',
    label: 'All receivers inside the room',
    codes: ['RECEIVER_OUTSIDE', 'RECEIVER_ON_SURFACE', 'RECEIVER_SPHERE_CROSSES'],
  },
  { key: 'air', label: 'Air absorption set', codes: ['ATMOSPHERE_INVALID', 'ABSATMO_INVALID'] },
  { key: 'solvers', label: 'Solvers are the verified build', codes: ['SOLVER_NOT_FOUND', 'SOLVER_UNVERIFIED'] },
];

export interface PreflightInput {
  /** `SceneState.run_blockers`; null with no project. */
  blockers: readonly string[] | null;
  solvers: SolversStatus | null;
  check: CheckSummary | null | undefined;
  info: Pick<ProjectInfo, 'surface_groups' | 'groups_assigned'> | null;
}

/**
 * The "Before running" rows, each OK or FAIL as text, from the same codes the Run button's
 * `data-blockers` joins (`flow.joinBlockers`): the project's own blockers, then the solvers'.
 * The solvers' row is UNCHECKED until `solvers_status` has answered. `RUN_ACTIVE` is no row: the
 * running block says so. `null` with no project.
 */
export function preflightRows(input: PreflightInput): PreflightRow[] | null {
  if (input.blockers === null) return null;
  const project = input.blockers.filter((b) => b !== RUN_ACTIVE);
  const named = new Set(PREFLIGHT.flatMap((r) => r.codes));
  const rows: PreflightRow[] = PREFLIGHT.map((r) => {
    if (r.key === 'solvers') {
      if (!input.solvers) return { key: r.key, label: r.label, state: 'UNCHECKED', codes: [], detail: 'not checked yet' };
      const codes = r.codes.filter((c) => input.solvers?.blockers.includes(c));
      return { key: r.key, label: r.label, state: codes.length ? 'FAIL' : 'OK', codes, detail: '' };
    }
    const codes = r.codes.filter((c) => project.includes(c));
    let detail = '';
    let fail = codes.length > 0;
    if (r.key === 'materials' && input.info && input.info.groups_assigned < input.info.surface_groups) {
      const missing = input.info.surface_groups - input.info.groups_assigned;
      detail = `${missing} of ${input.info.surface_groups} surface groups have none`;
    }
    if (r.key === 'model' && !input.check) {
      // No faces: the core refuses the run at its geometry stage.
      fail = true;
      detail = 'no model in the project';
    }
    return { key: r.key, label: r.label, state: fail ? 'FAIL' : 'OK', codes, detail };
  });
  const other = project.filter((b) => !named.has(b));
  if (other.length) rows.push({ key: 'other', label: 'Every other check passes', state: 'FAIL', codes: other, detail: '' });
  return rows;
}

// ---- settings, read-only as drawn (design:666-675; editing them is PQ3) ------------------------------

/** What the settings rows read from the project file: its bands, environment and solvers. */
export interface ProjectSettings {
  bands: BandSet;
  environment: Environment;
  solvers: SolverSettings;
}

/**
 * The settings out of `project_json`'s text (the canonical project file). `null` when the text
 * does not hold them: the rows then read an em dash, never a guess.
 */
export function projectSettings(json: string): ProjectSettings | null {
  try {
    const p = JSON.parse(json) as Partial<ProjectSettings> | null;
    if (!p || typeof p !== 'object' || !p.bands || !p.environment || !p.solvers?.spps || !p.solvers.tcr) return null;
    return { bands: p.bands, environment: p.environment, solvers: p.solvers };
  } catch {
    return null;
  }
}

export interface SettingRow {
  key: string;
  label: string;
  value: string;
}

/** `125`, `4k`, `1.25k`: a band's nominal centre frequency as the design spells it. */
export function hzText(hz: number): string {
  if (hz < 1000) return String(hz);
  // Nominal frequencies are integers (`band_frequency_not_integer`); the decimal point moves by
  // string, so 1250 is `1.25k`, never a float's spelling.
  const s = String(hz);
  const int = s.slice(0, -3);
  const frac = s.slice(-3).replace(/0+$/, '');
  return frac ? `${int}.${frac}k` : `${int}k`;
}

/** `1/1 oct · 125–4k`, and how many are computed when not all of them are. */
export function bandsText(bands: BandSet, computed: readonly boolean[]): string {
  const f = bands.frequencies_hz;
  if (f.length === 0) return 'none';
  const kind = bands.kind === 'octave' ? '1/1 oct' : '1/3 oct';
  const range = f.length === 1 ? hzText(f[0]) : `${hzText(f[0])}–${hzText(f[f.length - 1])}`;
  const on = computed.filter(Boolean).length;
  return on === f.length ? `${kind} · ${range}` : `${kind} · ${range} · ${on} of ${f.length} computed`;
}

/**
 * A time step as stored, in ms below a second (`0.01` is `10 ms`). The product is re-spelled
 * at 12 significant digits, so a stored `0.0035` reads `3.5 ms`, not `3.5000000000000004 ms`.
 */
export function timeStepText(v: F64): string {
  if (typeof v !== 'number') return `${v} s`;
  if (v > 0 && v < 1) return `${Number((v * 1000).toPrecision(12))} ms`;
  return `${v} s`;
}

/** `20 °C · 50 %` (ISO 9613), `0.01 dB/m` (user defined), or `off`. */
export function airText(env: Environment, on: boolean): string {
  if (!on) return 'off';
  if (env.air_absorption.kind === 'user_defined') {
    const unit = env.air_absorption.unit === 'per_metre' ? '/m' : 'dB/m';
    return `${exact(env.air_absorption.value)} ${unit}`;
  }
  return `${exact(env.temperature_c)} °C · ${exact(env.relative_humidity_percent)} %`;
}

/** `150,000`: an integer grouped by thousands, the same on every machine (no locale). */
export function groupedInt(n: number): string {
  return String(n).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

/**
 * The settings rows of the chosen solver, as drawn (design:666-675): SPPS its particles,
 * duration, time step, bands and air; TCR its method, bands and air. Values are the project's
 * own; with no settings read yet each reads an em dash.
 */
export function settingsRows(s: ProjectSettings | null, solver: SolverName): SettingRow[] {
  const dash = '—';
  if (solver === 'tcr') {
    return [
      { key: 'method', label: 'Method', value: 'Sabine · Eyring' },
      { key: 'bands', label: 'Bands', value: s ? bandsText(s.bands, s.solvers.tcr.bands_computed) : dash },
      { key: 'air', label: 'Air absorption', value: s ? airText(s.environment, s.solvers.tcr.air_absorption) : dash },
    ];
  }
  const spps = s?.solvers.spps;
  return [
    { key: 'particles', label: 'Particles per source and band', value: spps ? groupedInt(spps.particles_per_source) : dash },
    { key: 'duration', label: 'Duration', value: spps ? `${exact(spps.duration_s)} s` : dash },
    { key: 'time_step', label: 'Time step', value: spps ? timeStepText(spps.time_step_s) : dash },
    { key: 'bands', label: 'Bands', value: s && spps ? bandsText(s.bands, spps.bands_computed) : dash },
    { key: 'air', label: 'Air absorption', value: s && spps ? airText(s.environment, spps.air_absorption) : dash },
  ];
}

// ---- the last run (the idle block, design:419-425) ------------------------------------------------

/** A run's variant by its id: `Baseline` for the base project, its name, or that it is gone. */
export function runVariantName(id: string | null | undefined, variants: readonly Pick<Variant, 'id' | 'name'>[]): string {
  if (!id) return 'Baseline';
  return variants.find((v) => v.id === id)?.name ?? 'Deleted variant';
}

export interface LastRunView {
  run: string;
  /** `Run 3`: the run's number among this project's runs. */
  label: string;
  variant: string;
  status: RunRow['status'];
  statusText: string;
  /** SPPS's worst band and the limit, the backend's own strings; null for TCR, or no table. */
  loss: { pct: string; limit: string } | null;
  /** The solver's WARN lines (`run.json` `lines.warn`); an em dash when it never ran. */
  solverWarnings: string;
  reasons: ReasonUi[];
  /** The verdict's warnings (one per WARN line), each code once, in first-seen order. */
  warnings: ReasonUi[];
}

/** Reasons with each code once, the first of each kept, in order. */
export function distinctCodes(reasons: readonly ReasonUi[]): ReasonUi[] {
  const seen = new Set<string>();
  return reasons.filter((r) => !seen.has(r.code) && (seen.add(r.code), true));
}

export function lastRunView(row: RunRow, variants: readonly Pick<Variant, 'id' | 'name'>[]): LastRunView {
  return {
    run: row.run,
    label: `Run ${row.number}`,
    variant: runVariantName(row.variant, variants),
    status: row.status,
    statusText: statusWord(row.status),
    loss: row.loss ? { pct: row.loss.worst_pct, limit: row.loss.limit_pct } : null,
    solverWarnings: row.lines ? String(row.lines.warn) : '—',
    reasons: row.status === 'OK' ? [] : row.reasons,
    warnings: distinctCodes(row.warnings),
  };
}

// ---- the Results step (design:428-470; M11 shows its state, never a value) ------------------------

export type ResultsStateName = 'none' | 'running' | 'checking' | 'verified' | 'unverified' | 'refused' | 'error';

/**
 * What the Results step shows for the selected run: `none` (no run), `running` (not ended),
 * `checking` (asked, no answer yet), `verified`, `unverified` (the results load but the run's
 * solver build was not verified, backlog 38) or `refused` (`run_results`'s answer), or `error`
 * (the question itself failed, e.g. `RUN_NOT_FOUND`).
 */
export function resultsStateName(
  selected: string | null,
  row: Pick<RunRow, 'status'> | null,
  results: ResultsState | null,
  errored: boolean,
): ResultsStateName {
  if (!selected) return 'none';
  if (row?.status === 'RUNNING') return 'running';
  if (results) {
    if (results.verified) return 'verified';
    return results.unverified && !results.refusal ? 'unverified' : 'refused';
  }
  return errored ? 'error' : 'checking';
}

/**
 * The codes the Results step shows for `results`: the refusal's, or the reason a run whose
 * results load is still unverified (`ResultsState.unverified`, backlog 38); none for a verified
 * run. The panel renders these, so what the step shows is what this returns.
 */
export function resultsCodes(results: ResultsState | null): ReasonUi[] {
  if (results?.refusal) return [results.refusal];
  return results?.unverified ? [results.unverified] : [];
}

// ---- the run-quality advisor before a run (backlog 80) ------------------------------------------------

/** m10-h's scanner (app/e2e/lib/dom.ts `ACOUSTIC_NUMBER`), copied for the unit test: a digit next
 * to dB, s, ms or %. The advisor's rows hold none on the Simulate step. */
export const ACOUSTIC_NUMBER_RE = /\d\s*(dB|s|ms|%)(?![\p{L}\p{N}])/u;

/** What "Apply" sends: the setting, the value the advice was given for, and the value it sets. */
export interface AdviceApply {
  setting: Setting;
  from: number | boolean;
  to: number | boolean;
}

/** One advice item as the Simulate step shows it (`[data-advice=<key>]`). */
export interface AdviceRow {
  /** The advisor's code (`simpa_core::advise::CODES`). */
  key: string;
  /** The core's words, as they came: no number. */
  cause: string;
  words: string;
  /** The setting's field label, or null when no setting addresses it. */
  label: string | null;
  /** `0.31 m → 0.6 m`, `off → on`; null for a duration or a time step, whose value would read as
   * a number next to s or ms (m10-h): it shows in its own field once applied. */
  change: string | null;
  apply: AdviceApply | null;
  why: string | null;
  note: string | null;
}

/** A setting's value as the Simulate step may print it, or null when it may not (s, ms). */
export function settingValueText(setting: Setting, v: number | boolean): string | null {
  if (typeof v === 'boolean') return v ? 'on' : 'off';
  switch (setting) {
    case 'receiver_radius':
      return `${v} m`;
    case 'particles_per_source':
      return groupedInt(v);
    case 'extinction_exponent':
      return String(v);
    case 'duration':
    case 'time_step':
    case 'preserve_boundary':
    case 'echogram_per_source':
      return null;
  }
}

/** The advisor's rows (`SceneState.advice`), in the core's order; `null` with no project. */
export function adviceRows(advice: readonly Advice[] | null | undefined): AdviceRow[] | null {
  if (!advice) return null;
  return advice.map((a) => {
    const f = a.fix;
    const setting = f.setting ?? null;
    const from = f.from ?? null;
    const to = f.to ?? null;
    const apply = setting !== null && from !== null && to !== null ? { setting, from, to } : null;
    const a2 = apply ? settingValueText(apply.setting, apply.from) : null;
    const b2 = apply ? settingValueText(apply.setting, apply.to) : null;
    return {
      key: a.code,
      cause: a.cause,
      words: f.words,
      label: f.label ?? null,
      change: a2 !== null && b2 !== null ? `${a2} → ${b2}` : null,
      apply,
      why: f.why_no_apply ?? null,
      note: f.note ?? null,
    };
  });
}
