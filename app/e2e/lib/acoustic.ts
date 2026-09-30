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
//      [data-diagnostic] and [data-verbatim] hidden, and [data-input] and [data-geometry] hidden
//      only inside their own regions (dom.ts EXEMPT_REGIONS). Hiding every diagnostic and
//      verbatim element is the same verdict as hiding only the proven ones: an unproven one is a
//      violation of rule 3 or 4 by itself.
//   1x. A [data-input] or [data-geometry] element outside its regions (M11 review F2: the
//      exemptions covered the whole page).
//   1t. A tooltip (`title`) in a region that shows runs (the diagnostic regions and the Results
//      step) with a number next to a unit or a parameter's name followed by a number: a tooltip
//      is shown to the user and cannot be proven (M11 review 2, app 4).
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
import { ACOUSTIC_NUMBER, EXEMPT_REGIONS, PARAMETER_NUMBER } from './dom.ts';
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
  /** Region name to selector: the regions whose tooltips rule 1t reads. */
  titleRegions: Record<string, string>;
  /** Hidden for rule 1, wherever they are. */
  hide: string[];
  /** Hidden for rule 1 only inside their regions: attribute, then region name to selector. */
  exempt: Record<string, Record<string, string>>;
  /** The Acoustics panel. */
  acoustics: string;
}

/**
 * Where rule 1t reads tooltips: every region that shows runs, the diagnostic regions and the
 * Results step (M11 review 2, app 4: the Simulate and Results steps put the core's raw details,
 * numbers and units included, in `title`s, where no text check could see them). A tooltip
 * elsewhere, such as an input field's validator message, is the M10 checks' business.
 */
export const TITLE_REGIONS: Record<string, string> = {
  ...ALLOWED_REGIONS,
  results: '[data-props-step="results"]',
};

export const SNAPSHOT_CONFIG: SnapshotConfig = {
  regions: ALLOWED_REGIONS,
  titleRegions: TITLE_REGIONS,
  hide: ['[data-diagnostic]', '[data-verbatim]'],
  exempt: EXEMPT_REGIONS,
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

/** A `[data-input]` or `[data-geometry]` element, and the region that makes it exempt. */
export interface ExemptEl {
  attr: string;
  /** The first of its attribute's regions that contains it; null outside all of them. */
  region: string | null;
  text: string;
  sayNo: boolean;
}

/** A tooltip (`title`) in a region where runs are shown, and that region. */
export interface TitleEl {
  text: string;
  region: string;
  sayNo: boolean;
}

export interface DomSnapshot {
  /** Set when a plant's host was missing: nothing else was read. */
  error?: string;
  diagnostics: DiagnosticEl[];
  verbatim: VerbatimEl[];
  exempt: ExemptEl[];
  /** Every `title` inside the diagnostic regions and the Results step (rule 1t). */
  titles: TitleEl[];
  /** document.body.innerText, nothing hidden (rule 2). */
  wholeText: string;
  /** document.body.innerText with `hide` hidden, and the exempt elements inside their regions
   * (rule 1). */
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
      return { error: `no ${plant.host} to plant in`, diagnostics: [], verbatim: [], exempt: [], titles: [], wholeText: '', hiddenText: '', acousticsText: null };
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
    const exempt: ExemptEl[] = [];
    const inRegion: HTMLElement[] = [];
    for (const attr of Object.keys(cfg.exempt)) {
      for (const el of document.querySelectorAll<HTMLElement>(`[${attr}]`)) {
        const region = Object.keys(cfg.exempt[attr]).find((k) => el.closest(cfg.exempt[attr][k]) !== null) ?? null;
        if (region !== null) inRegion.push(el);
        exempt.push({ attr, region, text: (el.textContent ?? '').slice(0, 120), sayNo: el.hasAttribute('data-say-no') });
      }
    }
    const titles: TitleEl[] = [];
    for (const el of document.querySelectorAll<HTMLElement>('[title]')) {
      const region = Object.keys(cfg.titleRegions).find((k) => el.closest(cfg.titleRegions[k]) !== null) ?? null;
      if (region !== null) titles.push({ text: el.getAttribute('title') ?? '', region, sayNo: el.hasAttribute('data-say-no') });
    }
    const wholeText = document.body.innerText;
    const hidden = [...document.querySelectorAll<HTMLElement>(cfg.hide.join(', ')), ...inRegion];
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
      exempt,
      titles,
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

export type Rule = '1' | '1x' | '1t' | '2' | '3i' | '3ii' | '3iii' | '3iv' | '4' | 'acoustics';

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
    out.push({ rule: '1', text: t, detail: 'a number next to a unit outside diagnostics, verbatim lines, and [data-input] and [data-geometry] in their regions', sayNo: false });
  }
  for (const e of snap.exempt) {
    if (e.region !== null) continue;
    const regions = Object.keys(EXEMPT_REGIONS[e.attr as keyof typeof EXEMPT_REGIONS] ?? {}).join(', ');
    out.push({ rule: '1x', text: `[${e.attr}] "${e.text}"`, detail: `a [${e.attr}] element outside its regions (${regions || 'none'})`, sayNo: e.sayNo });
  }
  for (const t of matchesWithContext(snap.wholeText, PARAMETER_NUMBER)) {
    out.push({ rule: '2', text: t, detail: "a room-acoustic parameter's name followed by a number", sayNo: false });
  }
  for (const t of snap.titles) {
    const hits = [...matchesWithContext(t.text, ACOUSTIC_NUMBER), ...matchesWithContext(t.text, PARAMETER_NUMBER)];
    if (hits.length === 0) continue;
    out.push({
      rule: '1t',
      text: `[title] in ${t.region}: ${hits[0]}`,
      detail: "a tooltip with a number next to a unit, or a parameter's name followed by a number: a tooltip is shown to the user, and cannot be a proven diagnostic",
      sayNo: t.sayNo,
    });
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

/** The host of the say-NO plant inside an allowed `[data-geometry]` region: the status bar's
 * model fact, shown whenever a project with a checked model is open. */
export const MODEL_FACT = EXEMPT_REGIONS['data-geometry']['statusbar-model-fact'];

/**
 * The plants the checker must flag, given a run whose manifest is known (`run`, its elapsed
 * time as displayed, `elapsed`, and its worst loss, `lossPct`). The elapsed plant reads 1.8 s
 * unless the manifest says exactly that, when it reads 2.8 s: it must differ.
 *
 * H6 to H8 hold the exemptions to their regions (M11 review F2): a `[data-geometry]` and a
 * `[data-input]` element outside every region of theirs, and the review's mutation M13 inside an
 * allowed region, which only rule 2's wider names can see. H9 is a tooltip holding the verdict's
 * loss quote (review 2, app 4), which no text rule can see.
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
    {
      name: 'H6: a [data-geometry] span reading 1.52 s in the status bar, outside the model fact',
      plant: { host: ALLOWED_REGIONS.statusbar, tag: 'span', attrs: { 'data-geometry': '' }, text: '1.52 s' },
      rule: '1x',
    },
    {
      name: 'H7: a [data-input] span reading 85 dB in the Runs tab',
      plant: { host: ALLOWED_REGIONS.runs, tag: 'span', attrs: { 'data-input': '' }, text: '85 dB' },
      rule: '1x',
    },
    {
      name: "H8: 'Reverberation time · Sabine 1.52 s' in a [data-geometry] span inside the model fact",
      plant: { host: MODEL_FACT, tag: 'span', attrs: { 'data-geometry': '' }, text: 'Reverberation time · Sabine 1.52 s' },
      rule: '2',
      marker: 'Sabine 1.52',
    },
    {
      name: "H9: a reason's tooltip in the Runs tab quoting the verdict's loss detail, which the row withholds",
      plant: {
        host: ALLOWED_REGIONS.runs,
        tag: 'span',
        attrs: { title: '1 band(s) lost more than 1 % of their particles to loops and meshing: 500 Hz: 1600 of 150000 (1.0667 %)' },
        text: '',
      },
      rule: '1t',
    },
  ];
}

/** Whether the checker flagged a say-NO case: a violation of its rule, on the plant. */
export function flagged(c: SayNo, vs: Violation[]): boolean {
  return vs.some((x) => x.rule === c.rule && (c.marker ? x.text.includes(c.marker) : x.sayNo));
}
