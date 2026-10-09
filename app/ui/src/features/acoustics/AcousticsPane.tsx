// The Acoustics tab of the dock (M12 P2, docs/investigations/2026-10-03-m12/PLAN.md; design
// README "a bottom panel with three tabs: Acoustics (RT against the DIN 18041 target, a
// Sabine/Eyring table, an absorption breakdown)"). It shows the selected run's report, read by
// `run_report` (the CLI's report, `simpa results --json`), and only while the Results step is the
// current one: on every other step it holds no number (m10-h, m11-h).
//
// What model.ts decides is mapped here one to one onto elements, with the DOM contract the gate
// reads (app/e2e/lib/acoustics.ts):
//   [data-num][data-json][data-digits][data-scale]  a number of the report, at its decimals
//   [data-str][data-json]                           a string of the report
//   [data-label]                                    a word that is not the report's
//   [data-param]                                    every element of one parameter (gate (b))
//   [data-cell]                                     a value with its range and status, or its
//                                                   refusal (`[data-refusal]`), or STI's note;
//                                                   `[data-advice]` names the advice item that
//                                                   explains a refused, wide or warned one
//   [data-part="advice-card"]                       "Why values are missing" (backlog 80): each
//                                                   item of `report.advice`, `[data-advice=<code>]`,
//                                                   its words and numbers paths into the report
// R36: a TCR run adds "Receiver levels", each receiver's direct level and its total level with
// Sabine's and Eyring's reverberant field as TCR computed them, `[data-part="tcr-levels-table"]`,
// every number a `[data-num]` of the report like the rest of the tab, so gate (a) reads it and it
// is shown only where the tab shows numbers at all (results that load, on the Results step, an
// unverified solver build tagged). The bed has no row for these levels (it checks TCR's Eyring
// time, D), so they carry no per-parameter status and the card says they are not bed-checked.
// The charts (uPlot) are drawn from the same arrays the numbers are read from (`rtSeries`,
// `decay`), and the `acousticsView` test hook returns those arrays.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import uPlot from 'uplot';
import 'uplot/dist/uPlot.min.css';
import * as actions from '../../actions';
import { displayName } from '../../chrome/sceneModel';
import type { ApplyConflict, ReportView } from '../../bindings/ipc';
import { reportStore, runsStore, sceneStore, selectedRunStore, stepStore, useStore } from '../../store';
import { registerHook } from '../../testhooks';
import { runSolverText, runVariantName } from '../simulate/model';
import {
  type AdviceCard,
  adviceCards,
  applyConflict,
  absorption,
  bandNum,
  bandText,
  type BandSel,
  type Cell,
  cell,
  classical,
  decay,
  din,
  dinGroups,
  dinTarget,
  LOST_REFUSED,
  LOST_WARNING,
  MQ2_WORDING,
  paramMarks,
  receiverRows,
  receivers,
  type Refusal,
  rtSeries,
  runForVariant,
  type Series,
  shownParams,
  sourceLabel,
  sourceNote,
  sources,
  spectrumSeries,
  type SourceSel,
  tcrLevels,
  type Str,
} from './model';
import './acoustics.css';
import { CopyTable } from './CopyTable';
import { N, S } from './Marked';
import { auralHookOpen, auralHookSave, auralHookView, AuralWindow } from './AuralWindow';
import { responseHookView, ResponseWindow } from './ResponseWindow';

function Band({ report, index }: { report: NonNullable<ReportView['report']>; index: number }) {
  const b = bandNum(report, index);
  return b ? <N n={b.num} unit={b.unit} /> : null;
}

function RefusalView({ r }: { r: Refusal }) {
  return (
    <span className="ac-refusal" data-refusal={r.code.text}>
      <span className="ac-tag fail">REFUSED</span> <S s={r.code} />
      {r.why ? (
        <>
          {' · '}
          <S s={r.why} />
        </>
      ) : null}
      {r.lost ? (
        <span className="ac-note" data-lost="refused">
          <N n={r.lost} unit="%" /> {LOST_REFUSED}
        </span>
      ) : null}
    </span>
  );
}

function CellView({ c, unit }: { c: Cell; unit: string }) {
  return (
    <td
      data-cell
      data-param={c.param}
      data-receiver={c.receiver}
      data-band={c.band}
      data-status={c.status}
      data-advice={c.advice}
      className={`ac-cell ${c.status}`}
    >
      {c.status === 'refused' && c.refusal ? (
        <RefusalView r={c.refusal} />
      ) : (
        <>
          <span className="ac-value">
            <N n={c.value} unit={unit} />
          </span>
          {c.lo && c.hi ? (
            <span className="ac-range">
              <N n={c.lo} />
              {' – '}
              <N n={c.hi} />
              <span className={`ac-tag ${c.status}`}>{c.status}</span>
            </span>
          ) : null}
          {c.note ? (
            <span className="ac-note" data-note={c.note}>
              {c.note}
            </span>
          ) : null}
          {c.lost ? (
            <span className="ac-note" data-lost="warning">
              <N n={c.lost} unit="%" /> {LOST_WARNING}
            </span>
          ) : null}
        </>
      )}
    </td>
  );
}

/** A setting's value on the card: a number of the report, or on/off. */
function SettingValueView({ n, word, unit }: { n: AdviceCard['from']; word: string | null; unit: string }) {
  if (word !== null) return <span className="ac-word">{word}</span>;
  return <N n={n} unit={unit || undefined} />;
}

/** The cards' names, each a jump to its card (GUI audit 2026-10-09, A2): the dock shows two or three cards side by
 * side, and the rest were found only by scrolling sideways. "Why values are missing" comes last, after the numbers. */
const CARDS: readonly { label: string; card: string }[] = [
  { label: 'Reverberation time', card: 'Reverberation time against DIN 18041' },
  { label: 'Receivers', card: 'Receivers' },
  { label: 'Spectrum', card: 'Spectrum' },
  { label: 'Decay', card: 'Decay' },
  { label: 'Sabine / Eyring', card: 'Sabine and Eyring' },
  { label: 'Absorption', card: 'Absorption by surface group' },
];
const ADVICE_CARD = { label: 'Why values are missing', card: 'Why values are missing' };

function CardJumps({ advice }: { advice: boolean }) {
  const jump = (e: React.MouseEvent<HTMLButtonElement>, card: string) => {
    const el = e.currentTarget.closest('.ac')?.querySelector<HTMLElement>(`.ac-body > section[aria-label="${card}"]`);
    el?.scrollIntoView({ inline: 'start', block: 'nearest', behavior: 'smooth' });
  };
  return (
    <nav className="ac-jumps" aria-label="Cards" data-part="card-jumps">
      {(advice ? [...CARDS, ADVICE_CARD] : CARDS).map((c) => (
        <button key={c.card} type="button" className={c === ADVICE_CARD ? 'ac-jump warn' : 'ac-jump'} data-jump={c.card} onClick={(e) => jump(e, c.card)}>
          {c.label}
        </button>
      ))}
    </nav>
  );
}

/** "Why values are missing" (backlog 80): the run-quality advisor's items for this run, each its
 * cause, the setting that addresses it and, where the core offers one, "Apply and re-run" (one
 * checked edit, one undo step, then a run of the same solver). */
function AdviceCardView({ cards, solver, conflicts }: { cards: AdviceCard[]; solver: 'spps' | 'tcr'; conflicts: readonly ApplyConflict[] }) {
  if (!cards.length) return null;
  return (
    <section className="ac-card ac-advice-card" aria-label="Why values are missing" data-part="advice-card">
      <div className="ac-card-head">
        <span className="ac-card-title">Why values are missing</span>
        <span className="ac-sub">each cause once; the values it explains are marked in the tables</span>
      </div>
      {cards.map((c) => (
        <div key={c.index} className="ac-advice" data-advice={c.code.text}>
          <div className="ac-advice-cause">
            <S s={c.cause} />
          </div>
          <div className="ac-advice-fix">
            <S s={c.words} />
            {c.label && (c.from || c.fromWord !== null) && (c.to || c.toWord !== null) ? (
              <span className="ac-advice-change" data-part="advice-change">
                {' '}
                <S s={c.label} />
                {': '}
                <SettingValueView n={c.from} word={c.fromWord} unit={c.unit} />
                {' → '}
                <SettingValueView n={c.to} word={c.toWord} unit={c.unit} />
              </span>
            ) : null}
          </div>
          {c.note ? <S s={c.note} className="ac-note block" /> : null}
          {c.apply && applyConflict(c.apply, conflicts) !== null ? (
            <span className="ac-note block" data-part="advice-conflict">
              Not offered: {applyConflict(c.apply, conflicts)}
            </span>
          ) : c.apply && solver === 'spps' ? (
            <button
              type="button"
              className="small-button"
              data-part="advice-apply-rerun"
              data-setting={c.apply.setting}
              onClick={() => c.apply && actions.fire(actions.adviceApplyAndRerun(c.apply, solver))}
            >
              Apply and re-run
            </button>
          ) : c.why ? (
            <S s={c.why} className="ac-note block" />
          ) : null}
        </div>
      ))}
    </section>
  );
}

/** A uPlot chart over `data`, redrawn when `data` changes; sized to its box. */
function Chart({ opts, data, part }: { opts: Omit<uPlot.Options, 'width' | 'height'>; data: uPlot.AlignedData; part: string }) {
  const box = useRef<HTMLDivElement>(null);
  const plot = useRef<uPlot | null>(null);
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const size = () => ({ width: Math.max(160, el.clientWidth), height: Math.max(100, el.clientHeight) });
    plot.current = new uPlot({ ...opts, ...size() }, data, el);
    const ro = new ResizeObserver(() => plot.current?.setSize(size()));
    ro.observe(el);
    return () => {
      ro.disconnect();
      plot.current?.destroy();
      plot.current = null;
    };
  }, [opts, data]);
  return <div className="ac-chart" data-part={part} ref={box} />;
}

const css = (name: string, fallback: string) => {
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return v || fallback;
};
/** The two series colours, validated together on the dark panel (dataviz validate_palette.js:
 * lightness band, chroma, CVD and normal-vision separation, contrast all pass). */
const SERIES_COLOURS: Record<string, string> = { t30_s: '#e0202e', t20_s: '#3d8fd1', edt_s: '#d6a033' };

function axes(xLabel: string, yLabel: string, xValues?: (u: uPlot, splits: number[]) => string[]): uPlot.Axis[] {
  const ink = css('--text-3', '#85858e');
  const grid = { stroke: css('--line', '#1d1d21'), width: 1 };
  return [
    { label: xLabel, stroke: ink, grid, ticks: grid, ...(xValues ? { values: xValues } : {}) },
    { label: yLabel, stroke: ink, grid, ticks: grid },
  ];
}

/** R9: the spectrum's bars, the first series colour on the dark panel; their whiskers in the panel's ink. */
const SPECTRUM_COLOUR = '#e0202e';

/** A draw hook: each drawn value's range (lo to hi) as a whisker with caps, as the tables print it
 * beside the value; nothing where the value is a gap. */
function rangeWhiskers(series: readonly Series[], colour: (param: string) => string): (u: uPlot) => void {
  return (u: uPlot) => {
    const ctx = u.ctx;
    const cap = 4 * devicePixelRatio;
    series.forEach((s) => {
      ctx.save();
      ctx.strokeStyle = colour(s.param);
      ctx.lineWidth = 1.5 * devicePixelRatio;
      s.lo.forEach((lo, i) => {
        const hi = s.hi[i];
        if (lo === null || hi === null || s.values[i] === null) return;
        const px = u.valToPos(i, 'x', true);
        const y0 = u.valToPos(lo, 'y', true);
        const y1 = u.valToPos(hi, 'y', true);
        ctx.beginPath();
        ctx.moveTo(px, y0);
        ctx.lineTo(px, y1);
        ctx.moveTo(px - cap, y0);
        ctx.lineTo(px + cap, y0);
        ctx.moveTo(px - cap, y1);
        ctx.lineTo(px + cap, y1);
        ctx.stroke();
      });
      ctx.restore();
    });
  };
}

/** RT per band against the DIN target: the shown reverberation times, each with its range as a
 * whisker (`rtSeries`: the tables' filter, so a value the tables do not show is a gap), the target
 * line, and a shaded fifth either side of it. */
function RtChart({ report, series, target }: { report: NonNullable<ReportView['report']>; series: Series[]; target: number | null }) {
  const { opts, data } = useMemo(() => {
    const x = report.bands_hz.map((_, i) => i);
    const t = target ?? null;
    const flat = (k: number) => x.map(() => (t === null ? null : t * k));
    const data: uPlot.AlignedData = [x, ...series.map((s) => s.values), flat(1), flat(0.8), flat(1.2)];
    const n = series.length;
    const top = Math.max(0, ...series.flatMap((s) => s.hi.filter((v): v is number => v !== null)));
    const whiskers = rangeWhiskers(series, (p) => SERIES_COLOURS[p] ?? '#a1a1aa');
    const opts: Omit<uPlot.Options, 'width' | 'height'> = {
      legend: { show: false },
      cursor: { show: false },
      scales: { x: { time: false, range: [-0.5, x.length - 0.5] }, y: { range: (_u, _lo, hi) => [0, Math.max(hi ?? 1, top, (t ?? 0) * 1.3) * 1.1 || 1] } },
      axes: axes('Band', 'Time (s)', (_u, splits) => splits.map((v) => (Number.isInteger(v) && report.bands_hz[v] !== undefined ? bandText(report.bands_hz[v]) : ''))),
      series: [
        {},
        ...series.map((s) => ({ label: s.label, stroke: SERIES_COLOURS[s.param] ?? '#a1a1aa', width: 2, points: { size: 8, fill: SERIES_COLOURS[s.param] ?? '#a1a1aa' }, spanGaps: false })),
        { label: 'DIN target', stroke: css('--text-2', '#a1a1aa'), width: 1, dash: [4, 4], points: { show: false } },
        { label: 'lower', stroke: 'transparent', points: { show: false } },
        { label: 'upper', stroke: 'transparent', points: { show: false } },
      ],
      bands: [{ series: [n + 3, n + 2], fill: 'rgba(161,161,170,0.12)' }],
      hooks: { draw: [whiskers] },
    };
    return { opts, data };
  }, [report, series, target]);
  return <Chart opts={opts} data={data} part="rt-chart" />;
}

/** R9: a receiver's spectrum, SPL per band as bars (`spectrumSeries`: the receivers table's cells,
 * so a level the table does not show is a gap), each with its range as a whisker. */
function SpectrumChart({ report, series }: { report: NonNullable<ReportView['report']>; series: Series[] }) {
  const { opts, data } = useMemo(() => {
    const x = report.bands_hz.map((_, i) => i);
    const data: uPlot.AlignedData = [x, ...series.map((s) => s.values)];
    const vals = series.flatMap((s) => [...s.lo, ...s.hi]).filter((v): v is number => v !== null);
    const lo = vals.length ? Math.min(...vals) : 0;
    const hi = vals.length ? Math.max(...vals) : 1;
    // Bars stand on a floor 10 dB under the lowest level shown, so the differences between bands read.
    const floor = Math.floor((lo - 10) / 5) * 5;
    const whiskers = rangeWhiskers(series, () => css('--text', '#ececee'));
    const opts: Omit<uPlot.Options, 'width' | 'height'> = {
      legend: { show: false },
      cursor: { show: false },
      scales: { x: { time: false, range: [-0.5, x.length - 0.5] }, y: { range: [floor, Math.ceil((hi + 3) / 5) * 5] } },
      axes: axes('Band', 'Level (dB)', (_u, splits) => splits.map((v) => (Number.isInteger(v) && report.bands_hz[v] !== undefined ? bandText(report.bands_hz[v]) : ''))),
      series: [
        {},
        ...series.map((s) => ({
          label: s.label,
          stroke: SPECTRUM_COLOUR,
          fill: 'rgba(224,32,46,0.35)',
          width: 1.5,
          points: { show: false },
          paths: uPlot.paths.bars!({ size: [0.6, 48] }),
        })),
      ],
      hooks: { draw: [whiskers] },
    };
    return { opts, data };
  }, [report, series]);
  return <Chart opts={opts} data={data} part="spectrum-chart" />;
}

function DecayChart({ curve }: { curve: { t: number[]; db: number[] } }) {
  const { opts, data } = useMemo(() => {
    const data: uPlot.AlignedData = [curve.t, curve.db];
    const opts: Omit<uPlot.Options, 'width' | 'height'> = {
      legend: { show: false },
      cursor: { show: false },
      scales: { x: { time: false } },
      axes: axes('Time (s)', 'Level (dB)'),
      series: [{}, { label: 'Decay', stroke: '#e0202e', width: 2, points: { show: false } }],
    };
    return { opts, data };
  }, [curve]);
  return <Chart opts={opts} data={data} part="decay-chart" />;
}

/** What the tab shows, for the `acousticsView` test hook; null while the tab is not mounted. */
interface HookView {
  state: string;
  run: string | null;
  receiver: number;
  band: string;
  /** The source shown, or null for the sources summed (backlog 77). */
  source: string | null;
  din: string;
  series: Series[];
  /** R9: the spectrum chart's series (SPL per band), as drawn. */
  spectrum: Series[];
}
let paneView: HookView | null = null;

/**
 * The dock's part of the tab, mounted with the dock (always there): the `acousticsView` test
 * hook (gate (f) reads the RT series drawn), and the variant follow: when the variant switch
 * changes the active variant, the newest OK run of that variant is selected (`runForVariant`),
 * so the Results step and this tab show it.
 */
export function useAcousticsDock(): void {
  useEffect(() => registerHook('acousticsView', () => paneView), []);
  useEffect(() => registerHook('responseView', () => responseHookView()), []);
  useEffect(() => registerHook('auralView', () => auralHookView()), []);
  useEffect(() => registerHook('auralSave', ((what: 'ir' | 'aural', path: string) => auralHookSave(what, path)) as never), []);
  useEffect(() => registerHook('auralOpen', ((path: string) => auralHookOpen(path)) as never), []);
  useVariantRunFollow();
}

function useVariantRunFollow(): void {
  const scene = useStore(sceneStore);
  const active = scene?.view.active_variant ?? null;
  const prev = useRef<string | null | undefined>(undefined);
  useEffect(() => {
    if (prev.current === undefined) {
      prev.current = active;
      return;
    }
    if (prev.current === active) return;
    prev.current = active;
    const run = runForVariant(runsStore.get()?.rows ?? [], active);
    if (run) actions.selectRun(run);
  }, [active]);
}

export function AcousticsPane() {
  const step = useStore(stepStore);
  const selected = useStore(selectedRunStore);
  const reports = useStore(reportStore);
  const runs = useStore(runsStore);
  const scene = useStore(sceneStore);
  const [band, setBand] = useState<BandSel>(0);
  const [receiver, setReceiver] = useState(0);
  // Undefined until picked: a run with several sources opens on the first (backlog 77).
  const [source, setSource] = useState<SourceSel | undefined>(undefined);
  const [group, setGroup] = useState('A3');
  const [error, setError] = useState<{ run: string; code: string } | null>(null);
  const [responseOpen, setResponseOpen] = useState(false);
  const closeResponse = useCallback(() => setResponseOpen(false), []);
  // C5's listening window: it closes with the run it plays, and off the Results step.
  const [auralOpen, setAuralOpen] = useState(false);
  const closeAural = useCallback(() => setAuralOpen(false), []);

  const onResults = step === 'results';
  const row = selected ? (runs?.rows.find((r) => r.run === selected) ?? null) : null;
  const view = selected ? (reports.get(selected) ?? null) : null;
  const report = view?.report ?? null;
  const rowStatus = row?.status ?? null;

  useEffect(() => {
    if (!onResults || !selected || rowStatus === 'RUNNING' || reportStore.get().has(selected)) return;
    let live = true;
    actions.reportFor(selected).catch((e) => {
      if (live) setError({ run: selected, code: actions.asCmdError(e).code });
    });
    return () => {
      live = false;
    };
  }, [onResults, selected, rowStatus]);

  const names = receivers(report ?? ({ solver: 'spps', bands_hz: [] } as unknown as NonNullable<ReportView['report']>));
  const r = Math.min(receiver, Math.max(0, names.length - 1));
  const b: BandSel = band === 'sum' || (report && band < report.bands_hz.length) ? band : 0;
  const projectSources = scene?.view.sources;
  const srcNames = useMemo(() => (report ? sources(report, (projectSources ?? []).map((s) => s.name)) : []), [report, projectSources]);
  const src: SourceSel = !srcNames.length ? null : source === undefined || (source !== null && !srcNames.includes(source)) ? srcNames[0] : source;
  const series = useMemo(() => (report ? rtSeries(report, r, src) : []), [report, r, src]);
  const spectrum = useMemo(() => (report ? spectrumSeries(report, r, src) : []), [report, r, src]);
  const target = report ? dinTarget(report, group) : null;
  const curve = useMemo(() => (report ? decay(report, r, b, src) : null), [report, r, b, src]);

  const state = !onResults
    ? 'off-step'
    : !selected
      ? 'none'
      : error?.run === selected
        ? 'error'
        : rowStatus === 'RUNNING'
          ? 'running'
          : !view
            ? 'loading'
            : report
              ? 'ready'
              : 'refused';

  // The test hook (gate (f)): what is shown and the arrays the RT chart was given.
  paneView = { state, run: selected, receiver: r, band: String(b), source: src, din: group, series, spectrum };
  useEffect(
    () => () => {
      paneView = null;
    },
    [],
  );

  // The response window holds numbers: it closes with the run it shows, and off the Results step.
  useEffect(() => {
    if (state !== 'ready') setResponseOpen(false);
    if (state !== 'ready') setAuralOpen(false);
  }, [state, selected]);

  if (state === 'off-step') {
    return (
      <div data-acoustics data-acoustics-state={state} className="dock-empty empty">
        Room acoustics are shown on the Results step.
      </div>
    );
  }
  if (state !== 'ready' || !report) {
    const text: Record<string, string> = {
      none: 'No run selected. Run the simulation, or pick a run in the Runs tab.',
      running: 'This run is still running.',
      loading: 'Reading this run’s results…',
      refused: 'This run’s results were refused: no value is shown. The Results panel says why.',
      error: 'The results could not be read. The Console says why.',
    };
    return (
      <div data-acoustics data-acoustics-state={state} data-run={selected ?? ''} className="dock-empty empty">
        {text[state]}
      </div>
    );
  }

  const specs = shownParams(report);
  const rows = receiverRows(report, b, src);
  const note = sourceNote(report);
  const d = din(report, group);
  const groupNames = new Map((view?.surface_groups ?? []).map((g) => [g.material_id, g.names]));
  const abs = absorption(report, groupNames);
  const cls = classical(report);
  // R36: a TCR run's receiver levels, in the Receivers card's band.
  const levels = tcrLevels(report, b);
  const variants = scene?.view.variants ?? [];
  const label = row ? `Run ${row.number}${row.label ? ` · ${row.label}` : ''} · ${runVariantName(row.variant, variants)}` : selected;
  // EDT's row 37 marks, T30's row 46 mark, STI's MQ3 note: each only while its parameter is shown.
  const marks = paramMarks(report);

  return (
    <div data-acoustics data-acoustics-state="ready" data-run={selected} className="ac">
      <div className="ac-head">
        <span className="ac-title" data-run-label>
          {label}
        </span>
        <span className="ac-solver" data-run-label data-part="run-solver" title={row?.gpu_device ?? undefined}>
          {runSolverText(report.solver === 'tcr' ? 'tcr' : 'spps', row?.gpu_device)}
        </span>
        <span className="ac-wording" data-label="wording" data-part="wording" title={MQ2_WORDING}>
          {MQ2_WORDING}
        </span>
        {view?.state.unverified ? <span className="ac-tag warn">UNVERIFIED solver build</span> : null}
        {srcNames.length ? (
          <label className="ac-control">
            Source
            <select data-control="source" value={src ?? ''} onChange={(e) => setSource(e.target.value === '' ? null : e.target.value)}>
              {srcNames.map((n) => (
                <option key={n} value={n}>
                  {n}
                </option>
              ))}
              <option value="">all sources summed</option>
            </select>
          </label>
        ) : null}
        <CardJumps advice={adviceCards(report).length > 0} />
      </div>
      {note ? (
        <div className="ac-note block" data-part="source-note" data-label="note">
          {note}
        </div>
      ) : null}
      <div className="ac-body">
        <section className="ac-card ac-rt" aria-label="Reverberation time against DIN 18041">
          <div className="ac-card-head">
            <span className="ac-card-title">Reverberation time</span>
            <label className="ac-control">
              Receiver
              <select data-control="receiver" value={String(r)} onChange={(e) => setReceiver(Number(e.target.value))}>
                {names.map((n, i) => (
                  <option key={i} value={String(i)}>
                    {n}
                  </option>
                ))}
              </select>
            </label>
            {series.length ? <CopyTable part="rt-table" what="reverberation time" /> : null}
          </div>
          <div className="ac-legend">
            {series.map((s) => (
              <span key={s.param} className="ac-key" data-param={s.param}>
                <span className="ac-swatch" style={{ background: SERIES_COLOURS[s.param] }} />
                <span data-label="param" data-param={s.param}>
                  {s.label}
                </span>
              </span>
            ))}
            <span className="ac-key">
              <span className="ac-swatch dash" />
              target, shaded a fifth either side
            </span>
            <span className="ac-key">whiskers: each value's range</span>
          </div>
          <div className="ac-din" data-part="din-target">
            <label className="ac-control">
              <span data-label="standard">DIN 18041</span>
              <select data-control="din-group" value={group} onChange={(e) => setGroup(e.target.value)}>
                {dinGroups(report).map((g) => (
                  <option key={g} value={g}>
                    {g}
                  </option>
                ))}
              </select>
            </label>
            {d ? (
              <span className="ac-din-text">
                <S s={d.group} /> (<S s={d.use} />
                ): T_soll {d.target ? <N n={d.target} unit="s" /> : d.refusal ? <RefusalView r={d.refusal} /> : '–'} at <N n={d.volume} unit="m³" />
              </span>
            ) : (
              <span className="ac-none">The room was not read: no target.</span>
            )}
          </div>
          {d?.note ? <S s={d.note} className="ac-note block" /> : null}
          {series.length ? <RtChart report={report} series={series} target={target} /> : <div className="ac-none">No reverberation time is shown for this run.</div>}
          {series.length ? (
            <table className="ac-table" data-part="rt-table">
              <thead>
                <tr>
                  <th>Band</th>
                  {series.map((s) => (
                    <th key={s.param} data-param={s.param}>
                      <span data-label="param" data-param={s.param}>
                        {s.label}
                      </span>
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {report.bands_hz.map((_, i) => (
                  <tr key={i}>
                    <td>
                      <Band report={report} index={i} />
                    </td>
                    {series.map((s) => {
                      const spec = specs.find((x) => x.name === s.param)!;
                      const c = cell(report, spec, r, i, src);
                      return c ? <CellView key={s.param} c={c} unit="s" /> : <td key={s.param} />;
                    })}
                  </tr>
                ))}
              </tbody>
            </table>
          ) : null}
        </section>

        <section className="ac-card ac-receivers" aria-label="Receivers">
          <div className="ac-card-head">
            <span className="ac-card-title">Receivers</span>
            <label className="ac-control">
              Band
              <select data-control="band" value={String(b)} onChange={(e) => setBand(e.target.value === 'sum' ? 'sum' : Number(e.target.value))}>
                {report.bands_hz.map((hz, i) => (
                  <option key={hz} value={String(i)}>
                    {bandText(hz)}
                  </option>
                ))}
                <option value="sum">bands summed</option>
              </select>
            </label>
            <CopyTable part="receivers-table" what="receivers" />
          </div>
          {[...new Set(marks.map((m) => m.param))].map((param) => (
            <div key={param} className="ac-marks" data-param={param}>
              {marks
                .filter((m) => m.param === param)
                .map((m) => (
                  <span key={m.text} className="ac-mark" data-label="mark">
                    {m.text}
                  </span>
                ))}
            </div>
          ))}
          <table className="ac-table" data-part="receivers-table">
            <thead>
              <tr>
                <th>Receiver</th>
                {specs.map((s) => (
                  <th key={s.name} data-param={s.name}>
                    <span data-label="param" data-param={s.name}>
                      {s.label}
                    </span>
                    {s.unit ? <span className="ac-unit"> {s.unit}</span> : null}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row, i) => (
                <tr key={i}>
                  <td>
                    <S s={row.receiver} />
                  </td>
                  {row.cells.map((c, j) => (c ? <CellView key={specs[j].name} c={c} unit={specs[j].unit} /> : <td key={specs[j].name} />))}
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        <section className="ac-card ac-spectrum" aria-label="Spectrum">
          <div className="ac-card-head">
            <span className="ac-card-title">Spectrum</span>
            <label className="ac-control">
              Receiver
              <select data-control="spectrum-receiver" value={String(r)} onChange={(e) => setReceiver(Number(e.target.value))}>
                {names.map((n, i) => (
                  <option key={i} value={String(i)}>
                    {n}
                  </option>
                ))}
              </select>
            </label>
          </div>
          {spectrum.length && spectrum.some((s) => s.values.some((v) => v !== null)) ? (
            <>
              {/* Only a shown parameter is named (gate (b)): the key exists only with the chart. */}
              <div className="ac-legend">
                {spectrum.map((s) => (
                  <span key={s.param} className="ac-key" data-param={s.param}>
                    <span className="ac-swatch" style={{ background: SPECTRUM_COLOUR }} />
                    <span data-label="param" data-param={s.param}>
                      {s.label}
                    </span>
                  </span>
                ))}
                <span className="ac-key">per band, as the Receivers table shows it; whiskers: each value's range</span>
              </div>
              <SpectrumChart report={report} series={spectrum} />
            </>
          ) : (
            <div className="ac-none" data-part="spectrum-none">
              No sound level is shown for this run, so no spectrum is drawn.
            </div>
          )}
        </section>

        {levels.length ? (
          <section className="ac-card ac-tcr-levels" aria-label="TCR receiver levels">
            <div className="ac-card-head">
              <span className="ac-card-title">Receiver levels</span>
              <span className="ac-sub" data-label="tcr-levels-sub">
                TCR’s own, read from its receiver files, dB · {b === 'sum' ? 'the bands’ energetic sum (TCR’s Global row, not a band)' : <Band report={report} index={b} />} · not checked by the test bed, which checks TCR’s Eyring time only
              </span>
              <CopyTable part="tcr-levels-table" what="receiver levels" />
            </div>
            <table className="ac-table" data-part="tcr-levels-table">
              <thead>
                <tr>
                  <th>Receiver</th>
                  <th title="The direct sound alone, from every source">
                    <span data-label="level">Direct</span>
                  </th>
                  <th title="Direct sound plus Sabine’s diffuse reverberant field">
                    <span data-label="level">Total, Sabine</span>
                  </th>
                  <th title="Direct sound plus Eyring’s diffuse reverberant field">
                    <span data-label="level">Total, Eyring</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {levels.map((row, i) => (
                  <tr key={i}>
                    <td>
                      <S s={row.receiver} />
                    </td>
                    <td>
                      <N n={row.direct} unit="dB" />
                    </td>
                    <td>
                      <N n={row.sabine} unit="dB" />
                    </td>
                    <td>
                      <N n={row.eyring} unit="dB" />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        ) : null}

        <section className="ac-card ac-decay" aria-label="Decay">
          <div className="ac-card-head">
            <span className="ac-card-title">Decay</span>
            {report.solver !== 'tcr' ? (
              <button type="button" className="small-button ac-open" data-action="open-response" aria-pressed={responseOpen} onClick={() => setResponseOpen((o) => !o)}>
                Open response
              </button>
            ) : null}
            {report.solver !== 'tcr' ? (
              <button type="button" className="small-button ac-listen" data-action="open-aural" aria-pressed={auralOpen} title="Hear the receiver's impulse response, synthesised from the echogram, and a dry recording through it" onClick={() => setAuralOpen((o) => !o)}>
                Listen
              </button>
            ) : null}
            <span className="ac-sub">
              {names[r] !== undefined ? <S s={{ path: `${report.solver === 'tcr' ? 'tcr' : 'spps'}.point_receivers.${r}.label`, text: names[r] }} /> : null}
              {' · '}
              {b === 'sum' ? 'bands summed' : <Band report={report} index={b} />}
              {srcNames.length ? ' · ' : null}
              {srcNames.length ? src === null ? 'all sources summed' : sourceLabel(report, r, src) ? <S s={sourceLabel(report, r, src) as Str} /> : null : null}
            </span>
          </div>
          {curve ? <DecayChart curve={curve} /> : <div className="ac-none">No decay curve for this receiver and band.</div>}
          {responseOpen && report.solver !== 'tcr' ? (
            <ResponseWindow report={report} receiver={r} source={src} receivers={names} sources={srcNames} onReceiver={setReceiver} onSource={setSource} onClose={closeResponse} />
          ) : null}
          {auralOpen && selected && report.solver !== 'tcr' ? (
            <AuralWindow
              run={selected}
              runNumber={row?.number ?? null}
              project={scene?.info.name ?? null}
              receiver={r}
              source={src}
              receivers={names}
              sources={srcNames}
              onReceiver={setReceiver}
              onSource={setSource}
              onClose={closeAural}
            />
          ) : null}
        </section>

        <section className="ac-card ac-classical" aria-label="Sabine and Eyring">
          <div className="ac-card-head">
            <span className="ac-card-title">Sabine / Eyring</span>
            <span className="ac-sub">{report.solver === 'tcr' ? 'TCR’s own results' : 'classical formulas on the run’s own room, beside the simulation'}</span>
            {cls.length ? <CopyTable part="classical-table" what="Sabine and Eyring" /> : null}
          </div>
          {cls.length ? (
            <table className="ac-table" data-part="classical-table">
              <thead>
                <tr>
                  <th>Band</th>
                  {cls[0].cells.map((c) => (
                    <th key={c.key}>{c.label}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {cls.map((row) => (
                  <tr key={row.band}>
                    <td>
                      <Band report={report} index={row.band} />
                    </td>
                    {row.cells.map((c) => (
                      <td key={c.key}>{c.value ? <N n={c.value} unit={c.unit} /> : c.refusal ? <RefusalView r={c.refusal} /> : '–'}</td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <div className="ac-none">The room was not read: no classical times.</div>
          )}
        </section>

        <section className="ac-card ac-absorption" aria-label="Absorption by surface group">
          <div className="ac-card-head">
            <span className="ac-card-title">Absorption by surface group</span>
            <span className="ac-sub">S·α, m²; names from the project as it is open now</span>
            {abs ? <CopyTable part="absorption-table" what="absorption" /> : null}
          </div>
          {abs ? (
            <table className="ac-table" data-part="absorption-table">
              <thead>
                <tr>
                  <th>Surface group</th>
                  <th>Area</th>
                  {report.bands_hz.map((hz, i) => (
                    <th key={hz}>
                      <Band report={report} index={i} />
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {abs.rows.map((row) => (
                  <tr key={row.materialId.path}>
                    <td>
                      {row.names.length ? (
                        <span data-label="group" title={row.names.join(', ')}>
                          {row.names.map(displayName).join(', ')}
                        </span>
                      ) : (
                        <span>
                          material <N n={row.materialId} />
                        </span>
                      )}
                    </td>
                    <td>
                      <N n={row.area} unit="m²" />
                    </td>
                    {row.bands.map((n, i) => (
                      <td key={i}>
                        <N n={n} />
                      </td>
                    ))}
                  </tr>
                ))}
                <tr className="ac-total">
                  <td>Total</td>
                  <td />
                  {abs.totals.map((n, i) => (
                    <td key={i}>
                      <N n={n} />
                    </td>
                  ))}
                </tr>
              </tbody>
            </table>
          ) : (
            <div className="ac-none">The room was not read: no absorption.</div>
          )}
        </section>
        <AdviceCardView cards={adviceCards(report)} solver={report.solver === 'tcr' ? 'tcr' : 'spps'} conflicts={scene?.advice_conflicts ?? []} />
      </div>
    </div>
  );
}
