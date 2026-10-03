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
//                                                   refusal (`[data-refusal]`), or STI's note
// The charts (uPlot) are drawn from the same arrays the numbers are read from (`rtSeries`,
// `decay`), and the `acousticsView` test hook returns those arrays.
import { useEffect, useMemo, useRef, useState } from 'react';
import uPlot from 'uplot';
import 'uplot/dist/uPlot.min.css';
import * as actions from '../../actions';
import type { ReportView } from '../../bindings/ipc';
import { reportStore, runsStore, sceneStore, selectedRunStore, stepStore, useStore } from '../../store';
import { registerHook } from '../../testhooks';
import { runVariantName, solverLabel } from '../simulate/model';
import {
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
  EDT_MARKS,
  MQ2_WORDING,
  type Num,
  receiverRows,
  receivers,
  type Refusal,
  rtSeries,
  runForVariant,
  type Series,
  shownParams,
  STI_NOTE,
  type Str,
} from './model';
import './acoustics.css';

function N({ n, unit }: { n: Num | null; unit?: string }) {
  if (!n) return <span className="ac-none">–</span>;
  return (
    <span className="ac-n">
      <span data-num data-json={n.path} data-digits={n.digits} data-scale={n.scale}>
        {n.text}
      </span>
      {unit ? <span className="ac-unit"> {unit}</span> : null}
    </span>
  );
}

function S({ s, className }: { s: Str | null; className?: string }) {
  if (!s) return null;
  return (
    <span data-str data-json={s.path} className={className}>
      {s.text}
    </span>
  );
}

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
    </span>
  );
}

function CellView({ c, unit }: { c: Cell; unit: string }) {
  return (
    <td data-cell data-param={c.param} data-receiver={c.receiver} data-band={c.band} data-status={c.status} className={`ac-cell ${c.status}`}>
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
        </>
      )}
    </td>
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

/** RT per band against the DIN target: the shown reverberation times (gaps where refused), the
 * target line, and a shaded fifth either side of it. */
function RtChart({ report, series, target }: { report: NonNullable<ReportView['report']>; series: Series[]; target: number | null }) {
  const { opts, data } = useMemo(() => {
    const x = report.bands_hz.map((_, i) => i);
    const t = target ?? null;
    const flat = (k: number) => x.map(() => (t === null ? null : t * k));
    const data: uPlot.AlignedData = [x, ...series.map((s) => s.values), flat(1), flat(0.8), flat(1.2)];
    const n = series.length;
    const opts: Omit<uPlot.Options, 'width' | 'height'> = {
      legend: { show: false },
      cursor: { show: false },
      scales: { x: { time: false, range: [-0.5, x.length - 0.5] }, y: { range: (_u, _lo, hi) => [0, Math.max(hi ?? 1, (t ?? 0) * 1.3) * 1.1 || 1] } },
      axes: axes('Band', 'Time (s)', (_u, splits) => splits.map((v) => (Number.isInteger(v) && report.bands_hz[v] !== undefined ? bandText(report.bands_hz[v]) : ''))),
      series: [
        {},
        ...series.map((s) => ({ label: s.label, stroke: SERIES_COLOURS[s.param] ?? '#a1a1aa', width: 2, points: { size: 8, fill: SERIES_COLOURS[s.param] ?? '#a1a1aa' }, spanGaps: false })),
        { label: 'DIN target', stroke: css('--text-2', '#a1a1aa'), width: 1, dash: [4, 4], points: { show: false } },
        { label: 'lower', stroke: 'transparent', points: { show: false } },
        { label: 'upper', stroke: 'transparent', points: { show: false } },
      ],
      bands: [{ series: [n + 3, n + 2], fill: 'rgba(161,161,170,0.12)' }],
    };
    return { opts, data };
  }, [report, series, target]);
  return <Chart opts={opts} data={data} part="rt-chart" />;
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
  din: string;
  series: Series[];
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
  const [group, setGroup] = useState('A3');
  const [error, setError] = useState<{ run: string; code: string } | null>(null);

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
  const series = useMemo(() => (report ? rtSeries(report, r) : []), [report, r]);
  const target = report ? dinTarget(report, group) : null;
  const curve = useMemo(() => (report ? decay(report, r, b) : null), [report, r, b]);

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
  paneView = { state, run: selected, receiver: r, band: String(b), din: group, series };
  useEffect(
    () => () => {
      paneView = null;
    },
    [],
  );

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
  const rows = receiverRows(report, b);
  const d = din(report, group);
  const groupNames = new Map((view?.surface_groups ?? []).map((g) => [g.material_id, g.names]));
  const abs = absorption(report, groupNames);
  const cls = classical(report);
  const variants = scene?.view.variants ?? [];
  const label = row ? `Run ${row.number} · ${runVariantName(row.variant, variants)}` : selected;
  const hasEdt = specs.some((s) => s.name === 'edt_s');
  const hasSti = specs.some((s) => s.name === 'sti');

  return (
    <div data-acoustics data-acoustics-state="ready" data-run={selected} className="ac">
      <div className="ac-head">
        <span className="ac-title" data-run-label>
          {label}
        </span>
        <span className="ac-solver">{solverLabel(report.solver === 'tcr' ? 'tcr' : 'spps')}</span>
        <span className="ac-wording" data-label="wording" data-part="wording">
          {MQ2_WORDING}
        </span>
        {view?.state.unverified ? <span className="ac-tag warn">UNVERIFIED solver build</span> : null}
      </div>
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
                      const c = cell(report, spec, r, i);
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
          </div>
          {hasEdt ? (
            <div className="ac-marks" data-param="edt_s">
              {EDT_MARKS.map((m) => (
                <span key={m} className="ac-mark">
                  {m}
                </span>
              ))}
            </div>
          ) : null}
          {hasSti ? (
            <div className="ac-marks" data-param="sti">
              <span className="ac-mark">STI: {STI_NOTE}</span>
            </div>
          ) : null}
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

        <section className="ac-card ac-decay" aria-label="Decay">
          <div className="ac-card-head">
            <span className="ac-card-title">Decay</span>
            <span className="ac-sub">
              {names[r] !== undefined ? <S s={{ path: `${report.solver === 'tcr' ? 'tcr' : 'spps'}.point_receivers.${r}.label`, text: names[r] }} /> : null}
              {' · '}
              {b === 'sum' ? 'bands summed' : <Band report={report} index={b} />}
            </span>
          </div>
          {curve ? <DecayChart curve={curve} /> : <div className="ac-none">No decay curve for this receiver and band.</div>}
        </section>

        <section className="ac-card ac-classical" aria-label="Sabine and Eyring">
          <div className="ac-card-head">
            <span className="ac-card-title">Sabine / Eyring</span>
            <span className="ac-sub">{report.solver === 'tcr' ? 'TCR’s own results' : 'classical formulas on the run’s own room, beside the simulation'}</span>
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
                        <span data-label="group">{row.names.join(', ')}</span>
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
      </div>
    </div>
  );
}
