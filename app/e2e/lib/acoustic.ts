// m11-h's checker (docs/investigations/2026-09-29-m11/PLAN.md 4.2): no solver-computed acoustic
// number on screen, with the diagnostic allowance proven number by number.
//
// Two halves. `collectSnapshot` runs in the page (browser.execute): it reads the DOM once, with
// an optional planted element for the say-NO cases, and removes the plant before it returns.
// `judge` is pure: it holds the snapshot to the four rules, with each run's manifest and logs
// read in Node (`loadRunProof`), and returns every violation it finds. `acoustic.test.ts` holds
// the rules and the say-NO cases to their verdicts on synthetic snapshots.
//
// The rules, each able to fail:
//   1. A number next to a unit (M10's ACOUSTIC_NUMBER, unchanged) in the page's text with
//      [data-input], [data-geometry], [data-diagnostic] and [data-verbatim] hidden. Hiding every
//      diagnostic and verbatim element is the same verdict as hiding only the proven ones: an
//      unproven one is a violation of rule 3 or 4 by itself.
//   2. A parameter's name followed by a number (PARAMETER_NUMBER), anywhere, nothing hidden.
//   3. A [data-diagnostic] span that is not (i) one of the four fields, (ii) inside an allowed
//      region, a leaf, (iii) its field's grammar exactly, and (iv) the manifest's own value at the
//      display rounding (a progress span: one of the run's `#` lines at its displayed digits, or
//      100 once SPPS printed its end of calculation).
//   4. A [data-verbatim="<run>:<source>"] element whose text is not a line of that run's logs.
// Plus M10's: the Acoustics panel holds no digit at all.
//
// The in-page function is serialised by WebdriverIO and run in the page, so it closes over
// nothing and names no function inside itself (tsx's keepNames would wrap a named one in a
// helper the page does not have).
import path from 'node:path';
import { ACOUSTIC_NUMBER, PARAMETER_NUMBER } from './dom.ts';
import {
  elapsedS,
  endedCalculation,
  limitPct,
  type LogSource,
  type Manifest,
  progressValues,
  readManifest,
  roundedTo,
  sourceLines,
  worstLoss,
  bandLosses,
} from './runs.ts';

/** The four diagnostic fields (PLAN.md 3.4 rule 1). */
export const DIAGNOSTIC_FIELDS = ['loss_pct', 'loss_limit_pct', 'elapsed_s', 'progress_pct'] as const;
export type DiagnosticField = (typeof DIAGNOSTIC_FIELDS)[number];

/** Each field's grammar, exactly (PLAN.md 4.2 (iii)). */
export const GRAMMAR: Record<DiagnosticField, RegExp> = {
  loss_pct: /^\d{1,3}\.\d{2} %$/,
  loss_limit_pct: /^\d+(\.\d+)? %$/,
  elapsed_s: /^\d+\.\d s$/,
  progress_pct: /^\d{1,3}(\.\d{1,2})? %$/,
};

/**
 * The regions a diagnostic may sit in (PLAN.md 3.4 rule 1): the Runs tab, the Console, the
 * Simulate panel, the Simulate sub, the status bar and the Run button. The Acoustics panel and
 * the Results step are not among them.
 */
export const ALLOWED_REGIONS: Record<string, string> = {
  runs: '[data-dock-panel="runs"]',
  console: '[data-dock-panel="console"]',
  simulate: '[data-props-step="simulate"]',
  'simulate-sub': '[data-step="simulate"] [data-part="sub"]',
  statusbar: '.statusbar',
  'run-button': '[data-part="run"]',
};

export interface SnapshotConfig {
  /** Region name to selector: a diagnostic's region is the first that contains it. */
  regions: Record<string, string>;
  /** Hidden for rule 1. */
  hide: string[];
  /** The Acoustics panel. */
  acoustics: string;
}

export const SNAPSHOT_CONFIG: SnapshotConfig = {
  regions: ALLOWED_REGIONS,
  hide: ['[data-input]', '[data-geometry]', '[data-diagnostic]', '[data-verbatim]'],
  acoustics: '[data-dock-panel="acoustics"]',
};

/** An element the say-NO cases plant before the snapshot and remove after it. */
export interface Plant {
  host: string;
  tag: string;
  attrs: Record<string, string>;
  text: string;
}

export interface DiagnosticEl {
  field: string;
  run: string | null;
  band: string | null;
  text: string;
  leaf: boolean;
  region: string | null;
  sayNo: boolean;
}

export interface VerbatimEl {
  key: string;
  text: string;
  sayNo: boolean;
}

export interface DomSnapshot {
  /** Set when a plant's host was missing: nothing else was read. */
  error?: string;
  diagnostics: DiagnosticEl[];
  verbatim: VerbatimEl[];
  /** document.body.innerText, nothing hidden (rule 2). */
  wholeText: string;
  /** document.body.innerText with `hide` hidden (rule 1). */
  hiddenText: string;
  /** The Acoustics panel's text; null when it is not shown. */
  acousticsText: string | null;
}

/**
 * Reads the page for the checker (run it with `browser.execute(collectSnapshot, cfg, plant)`).
 * With a `plant`, the element is appended to its host first, marked `data-say-no`, and removed
 * before this returns, whatever happens.
 */
export function collectSnapshot(cfg: SnapshotConfig, plant: Plant | null): DomSnapshot {
  let planted: HTMLElement | null = null;
  if (plant) {
    const host = document.querySelector(plant.host);
    if (!host) {
      return { error: `no ${plant.host} to plant in`, diagnostics: [], verbatim: [], wholeText: '', hiddenText: '', acousticsText: null };
    }
    planted = document.createElement(plant.tag);
    for (const [k, v] of Object.entries(plant.attrs)) planted.setAttribute(k, v);
    planted.setAttribute('data-say-no', '');
    planted.textContent = plant.text;
    host.appendChild(planted);
  }
  try {
    const diagnostics = [...document.querySelectorAll<HTMLElement>('[data-diagnostic]')].map((el) => ({
      field: el.getAttribute('data-diagnostic') ?? '',
      run: el.getAttribute('data-run'),
      band: el.getAttribute('data-band'),
      text: el.textContent ?? '',
      leaf: el.children.length === 0,
      region: Object.keys(cfg.regions).find((k) => el.closest(cfg.regions[k]) !== null) ?? null,
      sayNo: el.hasAttribute('data-say-no'),
    }));
    const verbatim = [...document.querySelectorAll<HTMLElement>('[data-verbatim]')].map((el) => ({
      key: el.getAttribute('data-verbatim') ?? '',
      text: el.textContent ?? '',
      sayNo: el.hasAttribute('data-say-no'),
    }));
    const wholeText = document.body.innerText;
    const hidden = [...document.querySelectorAll<HTMLElement>(cfg.hide.join(', '))];
    const before = hidden.map((el) => el.style.display);
    hidden.forEach((el) => {
      el.style.display = 'none';
    });
    let hiddenText = '';
    try {
      hiddenText = document.body.innerText;
    } finally {
      hidden.forEach((el, i) => {
        el.style.display = before[i];
      });
    }
    const acoustics = document.querySelector(cfg.acoustics);
    return {
      diagnostics,
      verbatim,
      wholeText,
      hiddenText,
      acousticsText: acoustics ? (acoustics.textContent ?? '') : null,
    };
  } finally {
    planted?.remove();
  }
}

// ---- the judge ---------------------------------------------------------------------------------

/** What a run folder proves: its manifest and its logs, read once. */
export interface RunProof {
  dir: string;
  manifest: Manifest | null;
  lines: Record<LogSource, Set<string>>;
  /** The percentages of its `#` lines. */
  progress: string[];
  /** SPPS printed "End of calculation." */
  ended: boolean;
}

/** Reads a run folder's proof. */
export function loadRunProof(dir: string): RunProof {
  return {
    dir,
    manifest: readManifest(dir),
    lines: { solver: new Set(sourceLines(dir, 'solver')), mesh: new Set(sourceLines(dir, 'mesh')) },
    progress: progressValues(dir),
    ended: endedCalculation(dir),
  };
}

/** Finds run folders by name under the runs roots given, reading each once. */
export function runResolver(roots: string[], folders: (root: string) => string[]): (run: string) => RunProof | null {
  const where = new Map<string, string>();
  for (const root of roots) for (const f of folders(root)) if (!where.has(f)) where.set(f, path.join(root, f));
  const cache = new Map<string, RunProof>();
  return (run) => {
    const dir = where.get(run);
    if (!dir) return null;
    let p = cache.get(run);
    if (!p) {
      p = loadRunProof(dir);
      cache.set(run, p);
    }
    return p;
  };
}

export type Rule = '1' | '2' | '3i' | '3ii' | '3iii' | '3iv' | '4' | 'acoustics';

export interface Violation {
  rule: Rule;
  /** What was found, with a little context. */
  text: string;
  detail: string;
  /** The element was a say-NO plant. */
  sayNo: boolean;
}

function matchesWithContext(text: string, re: RegExp): string[] {
  const g = new RegExp(re.source, re.flags.includes('g') ? re.flags : `${re.flags}g`);
  const out: string[] = [];
  for (const m of text.matchAll(g)) {
    const at = m.index ?? 0;
    out.push(text.slice(Math.max(0, at - 24), at + m[0].length + 24).replace(/\s+/g, ' '));
  }
  return out;
}

/** The value a diagnostic span must show, from the run's manifest; a reason when there is none. */
export function expectedDiagnostic(field: DiagnosticField, band: string | null, m: Manifest): string | { none: string } {
  switch (field) {
    case 'loss_pct': {
      if (band !== null) {
        const b = bandLosses(m).find((x) => String(x.freq_hz) === band);
        return b ? `${b.pct} %` : { none: `run.json has no band ${band} Hz in its statistics` };
      }
      const w = worstLoss(m);
      return w ? `${w.pct} %` : { none: 'run.json has no particle statistics' };
    }
    case 'loss_limit_pct':
      return `${limitPct(m.loss_limit)} %`;
    case 'elapsed_s':
      return m.outcome ? `${elapsedS(m.outcome.elapsed_ms)} s` : { none: 'run.json has no outcome (the solver never ran)' };
    case 'progress_pct':
      return { none: 'progress is proven from the log' };
  }
}

/** Whether a progress span's text (`25.22 %`) is one of the run's `#` lines at its displayed
 * digits, or 100 once SPPS ended its calculation. */
export function progressProven(text: string, p: RunProof): boolean {
  const shown = text.replace(/ %$/, '');
  const d = (shown.split('.')[1] ?? '').length;
  const v = roundedTo(shown, d);
  if (v === null) return false;
  if (p.ended && v === 100n * 10n ** BigInt(d)) return true;
  return p.progress.some((line) => roundedTo(line, d) === v);
}

/**
 * Holds `snap` to the rules. `proof(run)` gives a run's manifest and logs, or null when there is
 * no such run folder. Returns every violation; an empty list is a pass.
 */
export function judge(snap: DomSnapshot, proof: (run: string) => RunProof | null): Violation[] {
  const out: Violation[] = [];
  if (snap.error) return [{ rule: '1', text: '', detail: snap.error, sayNo: false }];
  for (const t of matchesWithContext(snap.hiddenText, ACOUSTIC_NUMBER)) {
    out.push({ rule: '1', text: t, detail: 'a number next to a unit outside [data-input], [data-geometry], diagnostics and verbatim lines', sayNo: false });
  }
  for (const t of matchesWithContext(snap.wholeText, PARAMETER_NUMBER)) {
    out.push({ rule: '2', text: t, detail: "a room-acoustic parameter's name followed by a number", sayNo: false });
  }
  if (snap.acousticsText !== null && /\d/.test(snap.acousticsText)) {
    out.push({ rule: 'acoustics', text: snap.acousticsText.slice(0, 120), detail: 'a digit in the Acoustics panel', sayNo: false });
  }
  for (const d of snap.diagnostics) {
    const at = `data-diagnostic="${d.field}" data-run="${d.run ?? ''}"${d.band !== null ? ` data-band="${d.band}"` : ''} "${d.text}"`;
    const v = (rule: Rule, detail: string) => out.push({ rule, text: at, detail, sayNo: d.sayNo });
    if (!(DIAGNOSTIC_FIELDS as readonly string[]).includes(d.field)) {
      v('3i', `'${d.field}' is not one of ${DIAGNOSTIC_FIELDS.join(', ')}`);
      continue;
    }
    const field = d.field as DiagnosticField;
    if (d.region === null) v('3ii', 'outside the Runs tab, the Console, the Simulate panel and sub, the status bar and the Run button');
    if (!d.leaf) v('3ii', 'not a leaf span: it holds elements');
    if (!GRAMMAR[field].test(d.text)) {
      v('3iii', `does not match ${GRAMMAR[field]}`);
      continue;
    }
    const p = d.run ? proof(d.run) : null;
    if (!p) {
      v('3iv', d.run ? `no run folder '${d.run}' to prove it against` : 'no data-run to prove it against');
      continue;
    }
    if (field === 'progress_pct') {
      if (!progressProven(d.text, p)) v('3iv', `no '#' line of ${d.run} reads ${d.text.replace(/ %$/, '')} at the digits shown${p.ended ? ', and it is not 100' : ''}`);
      continue;
    }
    if (!p.manifest) {
      v('3iv', `${d.run} has no run.json`);
      continue;
    }
    const want = expectedDiagnostic(field, d.band, p.manifest);
    if (typeof want !== 'string') v('3iv', want.none);
    else if (want !== d.text) v('3iv', `run.json gives '${want}'`);
  }
  for (const e of snap.verbatim) {
    const at = `data-verbatim="${e.key}" "${e.text}"`;
    const i = e.key.lastIndexOf(':');
    const run = i > 0 ? e.key.slice(0, i) : '';
    const source = i > 0 ? e.key.slice(i + 1) : '';
    if (source !== 'solver' && source !== 'mesh') {
      out.push({ rule: '4', text: at, detail: `source '${source}' is neither solver nor mesh`, sayNo: e.sayNo });
      continue;
    }
    const p = run ? proof(run) : null;
    if (!p) {
      out.push({ rule: '4', text: at, detail: `no run folder '${run}' to prove it against`, sayNo: e.sayNo });
      continue;
    }
    if (!p.lines[source].has(e.text)) {
      out.push({ rule: '4', text: at, detail: `not a line of ${run}'s ${source} logs`, sayNo: e.sayNo });
    }
  }
  return out;
}

/** One line per violation, for an assertion message. */
export function describeViolations(vs: Violation[]): string {
  return vs.map((x) => `rule ${x.rule}${x.sayNo ? ' (say-NO)' : ''}: ${x.detail} :: ${x.text}`).join('\n');
}

// ---- the say-NO cases (PLAN.md 4.2) --------------------------------------------------------------

export interface SayNo {
  name: string;
  plant: Plant;
  /** The rule the checker must flag it under. */
  rule: Rule;
  /** For rules 1 and 2, which see text, not elements: the flagged text must hold this. */
  marker?: string;
}

/**
 * The five plants the checker must flag, given a run whose manifest is known (`run`, its
 * elapsed time as displayed, `elapsed`, and its worst loss, `lossPct`). The elapsed plant reads
 * 1.8 s unless the manifest says exactly that, when it reads 2.8 s: it must differ.
 */
export function sayNoCases(run: string, elapsed: string, lossPctText: string): SayNo[] {
  const wrong = elapsed === '1.8' ? '2.8 s' : '1.8 s';
  return [
    { name: 'H1: T30 1.8 s in the status bar', plant: { host: '.statusbar', tag: 'span', attrs: {}, text: 'T30 1.8 s' }, rule: '1', marker: 'T30 1.8 s' },
    { name: 'H2: STI 0.62 · D50 0.45', plant: { host: '.statusbar', tag: 'span', attrs: {}, text: 'STI 0.62 · D50 0.45' }, rule: '2', marker: 'STI 0' },
    {
      name: `an elapsed_s span reading ${wrong} for ${run}, whose run.json says ${elapsed} s`,
      plant: { host: ALLOWED_REGIONS.runs, tag: 'span', attrs: { 'data-diagnostic': 'elapsed_s', 'data-run': run }, text: wrong },
      rule: '3iv',
    },
    {
      name: 'a loss_pct span inside the Acoustics panel',
      plant: { host: SNAPSHOT_CONFIG.acoustics, tag: 'span', attrs: { 'data-diagnostic': 'loss_pct', 'data-run': run }, text: `${lossPctText} %` },
      rule: '3ii',
    },
    {
      name: 'a verbatim line that is not in the logs',
      plant: { host: ALLOWED_REGIONS.console, tag: 'div', attrs: { 'data-verbatim': `${run}:solver` }, text: 'Reverberation estimate written by nobody' },
      rule: '4',
    },
  ];
}

/** Whether the checker flagged a say-NO case: a violation of its rule, on the plant. */
export function flagged(c: SayNo, vs: Violation[]): boolean {
  return vs.some((x) => x.rule === c.rule && (c.marker ? x.text.includes(c.marker) : x.sayNo));
}
