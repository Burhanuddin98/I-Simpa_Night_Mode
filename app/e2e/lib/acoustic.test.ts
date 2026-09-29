// node --test suite for m11-h's checker (PLAN.md 4.2): each rule on synthetic snapshots, the
// five say-NO cases flagged under their own rule, and honest diagnostics and verbatim lines let
// through. The snapshots are what `collectSnapshot` returns; the proofs are what
// `loadRunProof` reads from a run folder.
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import {
  type DiagnosticEl,
  type DomSnapshot,
  expectedDiagnostic,
  flagged,
  GRAMMAR,
  judge,
  progressProven,
  type RunProof,
  sayNoCases,
  type VerbatimEl,
} from './acoustic.ts';
import type { Manifest } from './runs.ts';

const RUN = '20260929-202553-613-spps';

const manifest = {
  stage: 'solve',
  outcome: { exit_code: 0, cancelled: false, elapsed_ms: 1_405 },
  loss_limit: 0.01,
  particles: {
    bands: [
      { freq_hz: 125, lost_by_infinite_loops: 0, lost_by_meshing_problems: 0, total: 150_000 },
      { freq_hz: 4000, lost_by_infinite_loops: 0, lost_by_meshing_problems: 1, total: 800 },
    ],
  },
} as unknown as Manifest;

const proofOf = (m: Manifest | null): RunProof => ({
  dir: `C:\\work\\p\\box\\runs\\${RUN}`,
  manifest: m,
  lines: {
    solver: new Set(['SPPS version 1.4.0', '#0.01', '#25.22', 'End of calculation.']),
    mesh: new Set(['Opening scene_mesh.poly.']),
  },
  progress: ['0.01', '25.22', '99.99'],
  ended: true,
});
const proof = (run: string) => (run === RUN ? proofOf(manifest) : null);

const diag = (field: string, text: string, over: Partial<DiagnosticEl> = {}): DiagnosticEl => ({
  field,
  run: RUN,
  band: null,
  text,
  leaf: true,
  region: 'runs',
  sayNo: false,
  ...over,
});
const verb = (key: string, text: string, sayNo = false): VerbatimEl => ({ key, text, sayNo });
const snap = (over: Partial<DomSnapshot> = {}): DomSnapshot => ({
  diagnostics: [],
  verbatim: [],
  exempt: [],
  titles: [],
  wholeText: 'Geometry\nMaterials\nRun #1 · SPPS finished · OK · Particles lost (limit )',
  hiddenText: 'Geometry\nMaterials\nRun #1 · SPPS finished · OK · Particles lost (limit )',
  acousticsText: null,
  ...over,
});
const rules = (s: DomSnapshot) => judge(s, proof).map((v) => v.rule);

test('a clean page with proven diagnostics and verbatim lines passes', () => {
  const s = snap({
    diagnostics: [
      diag('loss_pct', '0.13 %'),
      diag('loss_pct', '0.00 %', { band: '125' }),
      diag('loss_pct', '0.13 %', { band: '4000' }),
      diag('loss_limit_pct', '1 %', { region: 'console' }),
      diag('elapsed_s', '1.4 s', { region: 'runs' }),
      diag('progress_pct', '25.22 %', { region: 'statusbar' }),
      diag('progress_pct', '100 %', { region: 'simulate-sub' }),
    ],
    verbatim: [verb(`${RUN}:solver`, 'End of calculation.'), verb(`${RUN}:mesh`, 'Opening scene_mesh.poly.')],
    // What innerText gives with the spans shown: the checker reads rule 1 on the hidden text.
    wholeText: 'Particles lost 0.13 % (limit 1 %) 1.4 s',
    acousticsText: 'Room acoustics appear here once the physics checks behind them pass.',
  });
  assert.deepEqual(judge(s, proof), []);
});

test('rule 1: a number next to a unit outside the exempt elements', () => {
  assert.deepEqual(rules(snap({ hiddenText: 'Reverberation 1.8 s' })), ['1']);
  assert.deepEqual(rules(snap({ hiddenText: 'Level 85 dB' })), ['1']);
  assert.deepEqual(rules(snap({ hiddenText: 'Clarity 12 %' })), ['1']);
  // Every match is reported, not only the first.
  assert.deepEqual(rules(snap({ hiddenText: '1 s and 2 ms' })), ['1', '1']);
  // A unit glued to a word is no unit (M10's lookahead): "3 steps", "5 sources".
  assert.deepEqual(rules(snap({ hiddenText: '3 steps, 5 sources' })), []);
});

test('rule 2: a parameter name followed by a number, whatever the unit, nothing hidden', () => {
  assert.deepEqual(rules(snap({ wholeText: 'STI 0.62 · D50 0.45' })), ['2', '2']);
  assert.deepEqual(rules(snap({ wholeText: 'T30: 1.8' })), ['2']);
  assert.deepEqual(rules(snap({ wholeText: 'EDT=-0.2' })), ['2']);
  assert.deepEqual(rules(snap({ wholeText: 'T30 and EDT appear in M12' })), []);
  // The reverberation time's other spellings (M11 review F2, mutation M13), with a `·` allowed.
  for (const t of ['Reverberation time · Sabine 1.52 s', 'Eyring: 1.4', 'RT60 1.9', 'T60 = 2.0', 'reverberation time · 1.5', 'sabine 1.5']) {
    assert.deepEqual(rules(snap({ wholeText: t })), ['2'], t);
  }
  // Names with no number after them, and the app's own text, stay clean.
  for (const t of ['Live · Sabine and Eyring, 1/1 octave', 'Run 3 · Baseline', 'Model closed · 6 surfaces', 'rt 3 g 2', 'Start 3']) {
    assert.deepEqual(rules(snap({ wholeText: t })), [], t);
  }
});

test('rule 1x: an exempt attribute holds only inside its regions', () => {
  const ex = (attr: string, region: string | null, sayNo = false) => ({ attr, region, text: '1.52 s', sayNo });
  assert.deepEqual(rules(snap({ exempt: [ex('data-geometry', 'geometry-panel'), ex('data-input', 'simulate-settings')] })), []);
  assert.deepEqual(rules(snap({ exempt: [ex('data-geometry', null)] })), ['1x']);
  assert.deepEqual(rules(snap({ exempt: [ex('data-input', null), ex('data-geometry', null)] })), ['1x', '1x']);
  // The detail names the regions it should have been in.
  const v = judge(snap({ exempt: [ex('data-geometry', null, true)] }), proof)[0];
  assert.ok(v.sayNo && v.detail.includes('statusbar-model-fact') && v.detail.includes('geometry-panel'), v.detail);
});

test('the Acoustics panel holds no digit', () => {
  assert.deepEqual(rules(snap({ acousticsText: 'Run 1' })), ['acoustics']);
  assert.deepEqual(rules(snap({ acousticsText: 'Nothing yet' })), []);
});

test('rule 3 (i): only the four fields', () => {
  assert.deepEqual(rules(snap({ diagnostics: [diag('t30', '1.8 s')] })), ['3i']);
});

test('rule 3 (ii): inside an allowed region, as a leaf', () => {
  assert.deepEqual(rules(snap({ diagnostics: [diag('loss_pct', '0.13 %', { region: null })] })), ['3ii']);
  assert.deepEqual(rules(snap({ diagnostics: [diag('loss_pct', '0.13 %', { leaf: false })] })), ['3ii']);
});

test("rule 3 (iii): each field's grammar, exactly", () => {
  for (const [field, text] of [
    ['loss_pct', '0.1 %'],
    ['loss_pct', '0.13%'],
    ['loss_pct', '0.13 % '],
    ['loss_limit_pct', '1'],
    ['elapsed_s', '1.40 s'],
    ['elapsed_s', '1 s'],
    ['progress_pct', '25.225 %'],
    ['progress_pct', '1.667e-05 %'],
  ]) {
    assert.deepEqual(rules(snap({ diagnostics: [diag(field, text)] })), ['3iii'], `${field} '${text}'`);
  }
  assert.ok(GRAMMAR.loss_pct.test('100.00 %'));
  assert.ok(GRAMMAR.loss_limit_pct.test('0.5 %'));
  assert.ok(GRAMMAR.elapsed_s.test('3600.0 s'));
  assert.ok(GRAMMAR.progress_pct.test('0.01 %'));
});

test("rule 3 (iv): the manifest's value at the display rounding", () => {
  // The worst band is 4 kHz, 1 of 800: 0.125 % rounds half up to 0.13.
  assert.equal(expectedDiagnostic('loss_pct', null, manifest), '0.13 %');
  assert.deepEqual(rules(snap({ diagnostics: [diag('loss_pct', '0.12 %')] })), ['3iv']);
  assert.deepEqual(rules(snap({ diagnostics: [diag('loss_pct', '0.13 %', { band: '125' })] })), ['3iv']);
  assert.deepEqual(rules(snap({ diagnostics: [diag('loss_pct', '0.00 %', { band: '250' })] })), ['3iv']);
  assert.deepEqual(rules(snap({ diagnostics: [diag('loss_limit_pct', '5 %')] })), ['3iv']);
  assert.deepEqual(rules(snap({ diagnostics: [diag('elapsed_s', '1.8 s')] })), ['3iv']);
  assert.deepEqual(rules(snap({ diagnostics: [diag('elapsed_s', '1.4 s', { run: 'nope' })] })), ['3iv']);
  assert.deepEqual(rules(snap({ diagnostics: [diag('elapsed_s', '1.4 s', { run: null })] })), ['3iv']);
  // A progress figure no '#' line printed.
  assert.deepEqual(rules(snap({ diagnostics: [diag('progress_pct', '42.42 %')] })), ['3iv']);
  // A run that never launched has no elapsed time and no statistics.
  const bare = (run: string) => (run === RUN ? proofOf({ outcome: null, particles: null, loss_limit: 0.01 } as unknown as Manifest) : null);
  assert.equal(judge(snap({ diagnostics: [diag('elapsed_s', '0.0 s')] }), bare)[0]?.rule, '3iv');
  assert.equal(judge(snap({ diagnostics: [diag('loss_pct', '0.00 %')] }), bare)[0]?.rule, '3iv');
  // No run.json at all.
  const none = (run: string) => (run === RUN ? proofOf(null) : null);
  assert.equal(judge(snap({ diagnostics: [diag('loss_limit_pct', '1 %')] }), none)[0]?.rule, '3iv');
});

test("a progress span is a '#' line at its displayed digits, or 100 once the calculation ended", () => {
  const p = proofOf(manifest);
  assert.equal(progressProven('25.22 %', p), true);
  assert.equal(progressProven('25.2 %', p), true);
  assert.equal(progressProven('25 %', p), true);
  assert.equal(progressProven('0.01 %', p), true);
  assert.equal(progressProven('100 %', p), true);
  assert.equal(progressProven('100.00 %', p), true);
  assert.equal(progressProven('25.23 %', p), false);
  assert.equal(progressProven('100 %', { ...p, ended: false, progress: ['99.99'] }), true, "99.99 shown at 0 decimals is 100");
  assert.equal(progressProven('100.00 %', { ...p, ended: false, progress: ['99.99'] }), false);
});

test('rule 4: a verbatim element is a line of its run and source', () => {
  assert.deepEqual(rules(snap({ verbatim: [verb(`${RUN}:solver`, 'End of calculation. ')] })), ['4']);
  assert.deepEqual(rules(snap({ verbatim: [verb(`${RUN}:mesh`, 'End of calculation.')] })), ['4']);
  assert.deepEqual(rules(snap({ verbatim: [verb(`${RUN}:app`, 'End of calculation.')] })), ['4']);
  assert.deepEqual(rules(snap({ verbatim: [verb('nope:solver', 'End of calculation.')] })), ['4']);
});

test('a plant whose host is missing fails the snapshot', () => {
  assert.deepEqual(rules(snap({ error: 'no .statusbar to plant in' })), ['1']);
});

test('say-NO: every plant is flagged under its own rule', () => {
  const cases = sayNoCases(RUN, '1.4', '0.13');
  assert.equal(cases.length, 9);
  for (const c of cases) {
    // What collectSnapshot reads with the plant in its host.
    const text = `Ready\n${c.plant.text}`;
    const exemptAttr = ['data-geometry', 'data-input'].find((a) => a in c.plant.attrs);
    let s: DomSnapshot;
    if (c.rule === '1' || c.rule === '2') {
      // Rule 2 reads the whole text; an exempt plant in its region is hidden from rule 1.
      s = snap({ wholeText: text, hiddenText: exemptAttr ? 'Ready' : text });
    } else if (c.rule === '1t') {
      // A tooltip: no text at all, so only rule 1t can see it.
      s = snap({ titles: [{ text: c.plant.attrs.title, region: 'runs', sayNo: true }] });
      assert.deepEqual(rules(snap({ wholeText: text, hiddenText: text })), [], `${c.name}: the text rules see nothing`);
    } else if (c.rule === '1x') {
      assert.ok(exemptAttr, c.name);
      s = snap({ wholeText: text, hiddenText: text, exempt: [{ attr: exemptAttr, region: null, text: c.plant.text, sayNo: true }] });
    } else if (c.plant.attrs['data-diagnostic']) {
      s = snap({
        diagnostics: [diag(c.plant.attrs['data-diagnostic'], c.plant.text, { sayNo: true, region: c.plant.host.includes('acoustics') ? null : 'runs' })],
      });
    } else {
      s = snap({ verbatim: [verb(c.plant.attrs['data-verbatim'], c.plant.text, true)] });
    }
    const vs = judge(s, proof);
    assert.ok(flagged(c, vs), `${c.name}: not flagged under rule ${c.rule}: ${JSON.stringify(vs)}`);
  }
  // H8 sits in an allowed region: rule 1 cannot see it, so only rule 2's wider names catch it.
  const h8 = cases[7];
  assert.equal(h8.rule, '2');
  assert.deepEqual(rules(snap({ wholeText: `Ready\n${h8.plant.text}`, hiddenText: 'Ready' })), ['2']);
  // The elapsed plant always differs from the manifest.
  assert.equal(sayNoCases(RUN, '1.8', '0.00')[2].plant.text, '2.8 s');
  // And the loss plant is otherwise honest: only its region gives it away.
  const loss = sayNoCases(RUN, '1.4', '0.13')[3];
  assert.deepEqual(rules(snap({ diagnostics: [diag('loss_pct', loss.plant.text, { region: 'runs' })] })), []);
});

test('say-NO: a checker that let a plant through would be caught', () => {
  const c = sayNoCases(RUN, '1.4', '0.13')[2];
  // The same span, reading the manifest's own value, is not flagged: flagged() needs the rule.
  assert.equal(flagged(c, judge(snap({ diagnostics: [diag('elapsed_s', '1.4 s', { sayNo: true })] }), proof)), false);
});

test('rule 1t: a tooltip with a number and a unit, or a parameter and a number, is flagged; others pass (review 2, app 4)', () => {
  const t = (text: string) => ({ text, region: 'simulate', sayNo: false });
  assert.deepEqual(rules(snap({ titles: [t('500 Hz: 1600 of 150000 (1.0667 %)')] })), ['1t']);
  assert.deepEqual(rules(snap({ titles: [t('the solver reported T30 = 1.52')] })), ['1t']);
  assert.deepEqual(rules(snap({ titles: [t('elapsed 12.5 s')] })), ['1t']);
  for (const clean of [
    'C:\\tmp\\nm-target\\gates\\m11\\20260930-011500\\solvers\\spps.exe',
    'C:\\tmp\\nm-target\\gates\\m11\\20260930-011500\\p\\box\\runs\\20260929-234927-813-spps',
    'b9f1c3a2e4d5f60718293a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d',
    "The reason's detail is on the Runs tab",
    'Stop the solver and record the run as Cancelled',
  ]) {
    assert.deepEqual(rules(snap({ titles: [t(clean)] })), [], clean);
  }
});
