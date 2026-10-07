// The response window (Burhan 2026-10-04 11:45): the selected receiver's energy echogram, from
// the selected source (or the sources summed), as a time x octave-band map in the Dockyard
// black-red-white map, with its colour bar, and a decay strip under it (the bands summed, or one
// band). Opened from the Acoustics tab's Decay card, floating over the app on the glass of
// decision-log row 50; closed by its button or Escape. Rendered into <body>, so the tab's own
// scans (m12-a) never see it; m12.response.e2e.ts reads it.
//
// Under the user's hand (Burhan 2026-10-06 18:20, "legitimately manipulated and controlled"):
// the receiver and the source, picked in the window; the time range, by dragging a span on the
// map, the wheel over it (about the cursor), Shift+drag or the arrow keys to pan, start and end
// in seconds typed in, "Fit energy" (the map cut where every band has reached the floor) and
// "Full run" as presets, double-click to fit; the level span (30 to 100 dB below the maximum);
// the time bin; the strip's band; a readout of the bin under the cursor. The window moves by its
// title bar and resizes by its corner; the map fills what it is given, and the band labels thin
// out when their rows are shorter than a line (`labelEvery`), so 27 third-octave bands read.
//
// What response.ts decides is drawn and printed here, with the tab's DOM contract: every number
// of the report a `[data-num][data-json]` path (band labels, time ticks, the emission, the time
// bins' widths), every name a `[data-str]`, the colour bar's span `[data-label="span"]`, the span
// chips the window's own choices `[data-label="control"]`; the readout's numbers are derived from
// the map, marked `[data-label="readout"]`; the pickers' options are checked on their own, as the
// tab's are (m12.response.e2e.ts resp-numbers). No canvas (M10 PLAN rule 3): the map is a
// PNG of one pixel per bin (`mapPixels`, png.ts), scaled up by CSS, so a pixel decoded from it is
// the bin's colour; the strip and its marks are SVG, the colour bar a CSS gradient of the stops.
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import type { Report } from '../../bindings/ipc';
import { N, S } from './Marked';
import type { SourceSel } from './model';
import { pngDataUrl } from './png';
import {
  clampRange,
  type ColRange,
  colourBarCss,
  decayDb,
  labelEvery,
  mapPixels,
  panRange,
  rangeTicks,
  RESPONSE_BINS,
  RESPONSE_NOTE,
  RESPONSE_TITLE,
  responseView,
  type ResponseView,
  SPAN_DB,
  spanLabels,
  SPANS,
  zoomRange,
} from './response';

/** What the window shows, for the `responseView` test hook; null while it is closed. */
export interface ResponseHookView {
  receiver: number;
  source: string | null;
  k0: number;
  cols: number;
  /** The steps drawn: `cols`, the crop's when the map is cut short, or the zoom's. */
  shown: number;
  /** The crop's steps (null: none, the map is always the full run), and whether "Full run" is on. */
  crop: number | null;
  full: boolean;
  paths: string[];
  max: { band: number; col: number };
  /** The columns drawn, `[c0, c1)`, and the span, dB. */
  range: ColRange;
  span: number;
}
let hookView: ResponseHookView | null = null;
export const responseHookView = (): ResponseHookView | null => hookView;

/** The map as an image (M10 PLAN rule 3, one canvas: the 3D view's is the only one): one pixel
 * per bin (`mapPixels`), a PNG in a data URL (png.ts), scaled up by CSS with `pixelated`, so a
 * pixel decoded from it is the bin's colour. */
function MapImage({ v, range, span }: { v: ResponseView; range: ColRange; span: number }) {
  const rows = v.map.db.length;
  const cols = range.c1 - range.c0;
  const src = useMemo(() => pngDataUrl(mapPixels(v.map, span, cols, range.c0), cols, rows), [v, cols, range.c0, rows, span]);
  return <img className="rw-map" data-part="response-map" src={src} width={cols} height={rows} alt="Level per band and time step" draggable={false} />;
}

/** The colour bar: the map's stops as a CSS gradient, the span's top at the top. */
function ColourBar() {
  return <div className="rw-bar-img" data-part="response-bar" style={{ backgroundImage: colourBarCss() }} />;
}

/** The strip's line over the window's columns: x in columns from `c0` (0 to `n`), y in dB below
 * the maximum (0 to `span`), stretched to the box; strokes keep their width. */
function Strip({ db, range, span, ticks }: { db: number[]; range: ColRange; span: number; ticks: readonly number[] }) {
  const n = range.c1 - range.c0;
  const points = db
    .slice(range.c0, range.c1)
    .map((d, c) => `${c + 0.5},${-d}`)
    .join(' ');
  return (
    <svg className="rw-strip" data-part="response-strip" viewBox={`0 0 ${n} ${span}`} preserveAspectRatio="none" aria-hidden="true">
      {[1, 2].map((k) => (
        <line key={k} className="rw-strip-grid" x1={0} x2={n} y1={(k * span) / 3} y2={(k * span) / 3} vectorEffect="non-scaling-stroke" />
      ))}
      {ticks.map((c) => (
        <line key={`t${c}`} className="rw-strip-tick" x1={c - range.c0} x2={c - range.c0} y1={span * 0.93} y2={span} vectorEffect="non-scaling-stroke" />
      ))}
      <polyline className="rw-strip-line" points={points} vectorEffect="non-scaling-stroke" />
    </svg>
  );
}

function SpanLabels({ span, ends }: { span: number; ends?: boolean }) {
  return (
    <>
      {spanLabels(span)
        .filter((l) => !ends || l.db === 0 || l.db === -span)
        .map((l) => (
          <span key={l.db} className="rw-span" style={{ top: `${(-l.db / span) * 100}%` }} data-label="span">
            {l.text}
          </span>
        ))}
    </>
  );
}

/** The span's bottom as a word in a sentence, marked as the colour bar's words are. */
function SpanWord({ span }: { span: number }) {
  const l = spanLabels(span)[3];
  return (
    <span className="rw-span-word" data-label="span">
      {l.text}
    </span>
  );
}

/** The window's frame: moved by its title bar, resized by its corner (CSS `resize`), kept on
 * screen. Centred until first moved. */
function useFrame() {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ x: number; y: number } | null>(null);
  const drag = useRef<{ dx: number; dy: number } | null>(null);
  const onHeadDown = (e: React.PointerEvent) => {
    if (e.button !== 0 || (e.target as HTMLElement).closest('button, select, input')) return;
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    drag.current = { dx: e.clientX - r.left, dy: e.clientY - r.top };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  };
  const onHeadMove = (e: React.PointerEvent) => {
    const d = drag.current;
    const el = ref.current;
    if (!d || !el) return;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const x = Math.max(0, Math.min(window.innerWidth - Math.min(w, 200), e.clientX - d.dx));
    const y = Math.max(0, Math.min(window.innerHeight - 40, e.clientY - d.dy));
    void h;
    setPos({ x, y });
  };
  const onHeadUp = () => {
    drag.current = null;
  };
  return { ref, pos, onHeadDown, onHeadMove, onHeadUp };
}

export function ResponseWindow({
  report,
  receiver,
  source,
  receivers,
  sources,
  onReceiver,
  onSource,
  onClose,
}: {
  report: Report;
  receiver: number;
  source: SourceSel;
  /** The receivers' labels, by index (the pane's `receivers`). */
  receivers: string[];
  /** The sources with their own echogram (the pane's `sources`); empty: the sources summed only. */
  sources: string[];
  onReceiver: (r: number) => void;
  onSource: (s: SourceSel) => void;
  onClose: () => void;
}) {
  // The time bin shown: steps a column (display only; response.ts `binSum`).
  const [bin, setBin] = useState<number>(1);
  // The level span below the maximum, dB.
  const [span, setSpan] = useState<number>(SPAN_DB);
  const v = useMemo(() => responseView(report, receiver, source, bin, span), [report, receiver, source, bin, span]);
  const stepS = report.spps?.time_step_s ?? 0;
  const stepMs = stepS * 1000;
  // A time bin's width, the report's time step times the steps per column, in ms: a number of the
  // report (`[data-num]`, m12.response's resp-numbers), at the decimals it needs (up to 3).
  const binNum = (b: number) => {
    const text = String(+(stepMs * b).toFixed(3));
    return { path: 'spps.time_step_s', digits: (text.split('.')[1] ?? '').length, scale: 1000 * b, text };
  };
  const cols = v ? v.map.db[0].length : 1;
  // The range: a preset ("Fit energy" where the map has a crop, else the full run; "Full run" on
  // the toggle), or the user's zoom. The preset holds across receivers, sources, bins and spans;
  // a zoom is dropped when the columns change under it (another bin), kept otherwise.
  const [fullRun, setFullRun] = useState(false);
  const [zoom, setZoom] = useState<ColRange | null>(null);
  useEffect(() => setZoom(null), [bin]);
  const cropped = !!v?.crop && !fullRun && !zoom;
  const range: ColRange = useMemo(() => {
    if (!v) return { c0: 0, c1: 1 };
    if (zoom) return clampRange(zoom, cols);
    if (cropped && v.crop) return { c0: 0, c1: v.crop.cols };
    return { c0: 0, c1: cols };
  }, [v, zoom, cropped, cols]);
  const shown = range.c1 - range.c0;
  const ticks = useMemo(() => {
    if (!v) return [];
    const colStep = stepS * bin;
    return rangeTicks(colStep, range).map((t) => ({ col: t.j, num: { ...v.run, digits: t.digits, scale: t.j * bin, text: (stepS * t.j * bin).toFixed(t.digits) } }));
  }, [v, range, stepS, bin]);
  // The strip: the bands summed, or one band's own decay.
  const [stripBand, setStripBand] = useState<number | 'sum'>('sum');
  const stripDb = useMemo(() => {
    if (!v) return null;
    if (stripBand === 'sum' || stripBand >= v.energy.length) return v.broadband;
    return decayDb(v.energy[stripBand], span);
  }, [v, stripBand, span]);

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement | null)?.closest('input, select, textarea')) return;
      if (e.key === 'Escape') onClose();
      else if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
        const by = Math.max(1, Math.round(shown * 0.1)) * (e.key === 'ArrowLeft' ? -1 : 1);
        setZoom(panRange(range, by, cols));
      } else if (e.key === '+' || e.key === '=') setZoom(zoomRange(range, 1.5, 0.5, cols));
      else if (e.key === '-' || e.key === '_') setZoom(zoomRange(range, 1 / 1.5, 0.5, cols));
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [onClose, range, shown, cols]);

  hookView = v
    ? {
        receiver,
        source,
        k0: v.k0,
        cols,
        shown,
        crop: v.crop?.cols ?? null,
        full: !cropped && !zoom,
        paths: v.bands.map((b) => b.path),
        max: { band: v.map.at.row, col: v.map.at.col },
        range,
        span,
      }
    : null;
  useEffect(
    () => () => {
      hookView = null;
    },
    [],
  );
  const emitted = v?.emission && (report.spps?.sources.length ?? 0) > 1 && source === null;

  // ---- the map's hand: drag a span to zoom, Shift+drag to pan, wheel to zoom, double-click to fit;
  // the readout follows the cursor.
  const mapBox = useRef<HTMLDivElement>(null);
  const [sel, setSel] = useState<{ x0: number; x1: number } | null>(null);
  const gesture = useRef<{ x0: number; pan: boolean; start: ColRange } | null>(null);
  const [hover, setHover] = useState<{ col: number; band: number } | null>(null);
  const colAt = useCallback(
    (clientX: number) => {
      const el = mapBox.current;
      if (!el) return range.c0;
      const r = el.getBoundingClientRect();
      const f = Math.min(1, Math.max(0, (clientX - r.left) / Math.max(1, r.width)));
      return range.c0 + f * shown;
    },
    [range.c0, shown],
  );
  const onMapDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    const el = mapBox.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    gesture.current = { x0: e.clientX - r.left, pan: e.shiftKey, start: range };
    el.setPointerCapture(e.pointerId);
  };
  const onMapMove = (e: React.PointerEvent) => {
    const el = mapBox.current;
    if (!el || !v) return;
    const r = el.getBoundingClientRect();
    const x = e.clientX - r.left;
    const g = gesture.current;
    if (g) {
      if (g.pan) {
        const by = -((x - g.x0) / Math.max(1, r.width)) * (g.start.c1 - g.start.c0);
        setZoom(panRange(g.start, by, cols));
      } else {
        setSel({ x0: Math.min(g.x0, x), x1: Math.max(g.x0, x) });
      }
      return;
    }
    const rows = v.map.db.length;
    const fy = Math.min(1, Math.max(0, (e.clientY - r.top) / Math.max(1, r.height)));
    const band = rows - 1 - Math.min(rows - 1, Math.floor(fy * rows));
    const col = Math.min(cols - 1, Math.floor(colAt(e.clientX)));
    setHover({ col, band });
  };
  const onMapUp = (e: React.PointerEvent) => {
    const g = gesture.current;
    gesture.current = null;
    const el = mapBox.current;
    if (!g || !el) return;
    const r = el.getBoundingClientRect();
    const x = e.clientX - r.left;
    setSel(null);
    if (g.pan) return;
    if (Math.abs(x - g.x0) >= 4) {
      const a = g.start.c0 + (Math.min(g.x0, x) / Math.max(1, r.width)) * (g.start.c1 - g.start.c0);
      const b = g.start.c0 + (Math.max(g.x0, x) / Math.max(1, r.width)) * (g.start.c1 - g.start.c0);
      setZoom(clampRange({ c0: Math.floor(a), c1: Math.ceil(b) }, cols));
    }
  };
  const onMapWheel = (e: React.WheelEvent) => {
    const el = mapBox.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const anchor = (e.clientX - r.left) / Math.max(1, r.width);
    const factor = e.deltaY < 0 ? 1.25 : 1 / 1.25;
    setZoom(zoomRange(range, factor, anchor, cols));
  };
  const fit = () => {
    setZoom(null);
    setFullRun(false);
  };
  const toggleFull = () => {
    setZoom(null);
    setFullRun((f) => !f);
  };
  // Start and end typed in, seconds from the emission; applied on Enter or blur.
  const colStepS = stepS * bin;
  const [edit, setEdit] = useState<{ start: string; end: string } | null>(null);
  const startText = edit?.start ?? (range.c0 * colStepS).toFixed(3);
  const endText = edit?.end ?? (range.c1 * colStepS).toFixed(3);
  const applyRange = () => {
    if (!edit) return;
    const a = Number(edit.start);
    const b = Number(edit.end);
    setEdit(null);
    if (!Number.isFinite(a) || !Number.isFinite(b) || !(colStepS > 0) || b <= a) return;
    setZoom(clampRange({ c0: Math.floor(a / colStepS), c1: Math.ceil(b / colStepS) }, cols));
  };

  // The band labels thin out when their rows are shorter than a line.
  const [every, setEvery] = useState(1);
  const rows = v?.bands.length ?? 1;
  useLayoutEffect(() => {
    const el = mapBox.current;
    if (!el) return;
    const measure = () => setEvery(labelEvery(el.getBoundingClientRect().height / rows));
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [rows, v]);

  const frame = useFrame();
  const hoverDb = hover && v ? v.map.db[hover.band]?.[hover.col] : undefined;
  const hoverStrip = hover && stripDb ? stripDb[hover.col] : undefined;
  const fmtDb = (d: number | undefined) => (d === undefined ? '' : d === -Infinity ? 'no energy' : `${d.toFixed(1).replace('-', '−')} dB`);

  return createPortal(
    <div
      ref={frame.ref}
      className={`rw glass${frame.pos ? ' rw-moved' : ''}`}
      style={frame.pos ? { left: frame.pos.x, top: frame.pos.y } : undefined}
      role="dialog"
      aria-label={RESPONSE_TITLE}
      data-response-window
      data-receiver={receiver}
      data-source={source ?? ''}
    >
      <div className="rw-head" onPointerDown={frame.onHeadDown} onPointerMove={frame.onHeadMove} onPointerUp={frame.onHeadUp} onPointerCancel={frame.onHeadUp}>
        <span className="rw-title" data-part="response-title">
          {RESPONSE_TITLE}
        </span>
        <span className="rw-sub">
          {v ? <S s={v.receiver} /> : null}
          {' · '}
          {v?.source ? <S s={v.source} /> : (report.spps?.sources.length ?? 0) > 1 ? 'all sources summed' : v?.emission ? <S s={v.emission.source} /> : null}
        </span>
        <span className="rw-pick">
          <label>
            Receiver
            <select data-part="response-receiver" value={receiver} onChange={(e) => onReceiver(Number(e.target.value))}>
              {receivers.map((name, i) => (
                <option key={i} value={i}>
                  {name}
                </option>
              ))}
            </select>
          </label>
          {sources.length ? (
            <label>
              Source
              <select data-part="response-source" value={source ?? ''} onChange={(e) => onSource(e.target.value === '' ? null : e.target.value)}>
                <option value="">all sources summed</option>
                {sources.map((s) => (
                  <option key={s} value={s}>
                    {s}
                  </option>
                ))}
              </select>
            </label>
          ) : null}
        </span>
        <button type="button" className="rw-close" data-action="close-response" aria-label="Close" title="Close (Esc)" onClick={onClose}>
          ×
        </button>
      </div>
      <div className="rw-note" data-part="response-note">
        {RESPONSE_NOTE}
      </div>
      <div className="rw-controls" data-part="response-controls">
        <div className="rw-bins segmented" role="radiogroup" aria-label="Time bin" data-part="response-bins">
          <span className="rw-bins-label">Time bin</span>
          {RESPONSE_BINS.map((b) => (
            <button
              key={b}
              type="button"
              role="radio"
              aria-checked={bin === b}
              aria-selected={bin === b}
              data-bin={b}
              title={b === 1 ? 'Every time step of the run' : `${b} time steps summed per column: smoother, less detail in the early reflections`}
              onClick={() => setBin(b)}
            >
              <N n={binNum(b)} unit="ms" />
            </button>
          ))}
        </div>
        <div className="rw-bins segmented" role="radiogroup" aria-label="Level span" data-part="response-spans">
          <span className="rw-bins-label">Span</span>
          {SPANS.map((s) => (
            <button key={s} type="button" role="radio" aria-checked={span === s} aria-selected={span === s} data-span={s} title={`Colours and the strip run ${s} dB below the maximum`} onClick={() => setSpan(s)}>
              <span data-label="control">{`${s} dB`}</span>
            </button>
          ))}
        </div>
        <div className="rw-range" data-part="response-range">
          <span className="rw-bins-label">Time</span>
          <input
            className="rw-num"
            type="number"
            inputMode="decimal"
            step={colStepS || 0.001}
            min={0}
            value={startText}
            aria-label="Start, s"
            data-field="start"
            onChange={(e) => setEdit({ start: e.target.value, end: endText })}
            onBlur={applyRange}
            onKeyDown={(e) => e.key === 'Enter' && applyRange()}
          />
          <span className="rw-dash">to</span>
          <input
            className="rw-num"
            type="number"
            inputMode="decimal"
            step={colStepS || 0.001}
            min={0}
            value={endText}
            aria-label="End, s"
            data-field="end"
            onChange={(e) => setEdit({ start: startText, end: e.target.value })}
            onBlur={applyRange}
            onKeyDown={(e) => e.key === 'Enter' && applyRange()}
          />
          <span className="rw-unit">s</span>
          <button type="button" className={`rw-full${cropped ? ' on' : ''}`} data-action="response-fit" aria-pressed={cropped} title="Cut the map where every band has reached the floor" onClick={fit} disabled={!v?.crop}>
            Fit energy
          </button>
          <button
            type="button"
            className={`rw-full${!cropped && !zoom ? ' on' : ''}`}
            data-action="response-full"
            aria-pressed={!cropped && !zoom}
            title={fullRun || !v?.crop ? 'The whole duration of the run' : 'Show the whole duration of the run'}
            onClick={toggleFull}
          >
            Full run
          </button>
          <button type="button" className="rw-full" data-action="response-zoom-in" title="Zoom in (+)" aria-label="Zoom in" onClick={() => setZoom(zoomRange(range, 1.5, 0.5, cols))}>
            +
          </button>
          <button type="button" className="rw-full" data-action="response-zoom-out" title="Zoom out (−)" aria-label="Zoom out" onClick={() => setZoom(zoomRange(range, 1 / 1.5, 0.5, cols))} disabled={shown >= cols}>
            −
          </button>
        </div>
      </div>
      {v ? (
        <div className="rw-grid" style={{ ['--rw-rows' as string]: String(v.bands.length) }}>
          <div className="rw-y" data-part="response-bands">
            {v.bands.map((b, i) => (
              <span key={b.path} className={`rw-band${i % every ? ' rw-band-thin' : ''}`}>
                <N n={b.label.num} unit={b.label.unit} />
              </span>
            ))}
          </div>
          <div
            ref={mapBox}
            className="rw-map-box"
            data-part="response-map-box"
            onPointerDown={onMapDown}
            onPointerMove={onMapMove}
            onPointerUp={onMapUp}
            onPointerCancel={onMapUp}
            onPointerLeave={() => setHover(null)}
            onWheel={onMapWheel}
            onDoubleClick={fit}
            title="Drag to zoom to a span, wheel to zoom, Shift+drag or ← → to pan, double-click to fit"
          >
            <MapImage v={v} range={range} span={span} />
            {sel ? <div className="rw-sel" style={{ left: sel.x0, width: Math.max(1, sel.x1 - sel.x0) }} /> : null}
            {hover && !sel ? <div className="rw-cursor" style={{ left: `${((hover.col - range.c0 + 0.5) / shown) * 100}%` }} /> : null}
          </div>
          <div className="rw-bar">
            <ColourBar />
            <div className="rw-bar-labels">
              <SpanLabels span={span} />
            </div>
          </div>

          <div className="rw-y rw-y-strip">
            <SpanLabels span={span} ends />
          </div>
          {stripDb ? <Strip db={stripDb} range={range} span={span} ticks={ticks.map((t) => t.col)} /> : <div className="ac-none">No decay.</div>}
          <div className="rw-strip-key">
            <select className="rw-strip-pick" data-part="response-strip-band" value={stripBand === 'sum' ? 'sum' : String(stripBand)} onChange={(e) => setStripBand(e.target.value === 'sum' ? 'sum' : Number(e.target.value))} title="The strip: the bands summed, or one band's own decay">
              <option value="sum">bands summed</option>
              {v.bands.map((b, i) => (
                <option key={b.path} value={i}>
                  {`${b.label.num.text} ${b.label.unit}`}
                </option>
              ))}
            </select>
          </div>

          <div />
          <div className="rw-x" data-part="response-ticks">
            {ticks.map((t) => (
              <span key={t.col} className="rw-tick" style={{ left: `${((t.col - range.c0) / shown) * 100}%` }}>
                <N n={t.num} />
              </span>
            ))}
          </div>
          <div />

          <div />
          <div className="rw-x-label">
            Time from {emitted ? 'the first emission' : v.emission ? <><S s={v.emission.source} />’s emission</> : 'the start'}, s
            {v.emission && v.k0 > 0 ? (
              <>
                {' '}
                (at <N n={v.emission.at} unit="s" />)
              </>
            ) : null}
            {' · colour: level re the map’s maximum'}
          </div>
          <div />

          <div />
          <div className="rw-shown" data-part="response-shown" data-shown={zoom ? 'zoom' : cropped ? 'crop' : 'full'}>
            <span>
              {zoom ? (
                <>
                  Shown: <span data-label="readout">{(range.c0 * colStepS).toFixed(3)}</span> to <span data-label="readout">{(range.c1 * colStepS).toFixed(3)}</span> s of the <N n={v.run} unit="s" /> run
                </>
              ) : cropped && v.crop ? (
                <>
                  Shown: to <N n={v.crop.end} unit="s" /> of the <N n={v.run} unit="s" /> run
                </>
              ) : (
                <>
                  Shown: the full run, <N n={v.run} unit="s" />
                </>
              )}
              {v.floor ? (
                <>
                  ; from <N n={v.floor} unit="s" /> every band is at <SpanWord span={span} /> or lower
                </>
              ) : (
                <>
                  ; a band is still above <SpanWord span={span} /> at its end
                </>
              )}
            </span>
            <span className="rw-readout" data-part="response-readout" data-label="readout">
              {hover && v ? (
                <>
                  {`t ${(hover.col * colStepS).toFixed(3)} s · ${v.bands[hover.band].label.num.text} ${v.bands[hover.band].label.unit} · ${fmtDb(hoverDb)}`}
                  {hoverStrip !== undefined ? ` · ${stripBand === 'sum' ? 'summed' : 'strip'} ${fmtDb(hoverStrip)}` : ''}
                </>
              ) : (
                'Point at the map for the bin under the cursor'
              )}
            </span>
          </div>
          <div />
        </div>
      ) : (
        <div className="ac-none" data-part="response-none">
          This receiver has no echogram with energy for this source.
        </div>
      )}
    </div>,
    document.body,
  );
}
