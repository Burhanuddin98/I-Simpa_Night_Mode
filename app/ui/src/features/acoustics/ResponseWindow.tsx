// The response window (Burhan 2026-10-04 11:45): the selected receiver's energy echogram, from
// the selected source (or the sources summed), as a time x octave-band map in the Dockyard
// black-red-white map, with its colour bar, and the bands summed as a decay strip under it.
// Opened from the Acoustics tab's Decay card, floating over the app on the glass of decision-log
// row 50, pinned at the centre; closed by its button or Escape. Rendered into <body>, so the
// tab's own scans (m12-a) never see it; m12.response.e2e.ts reads it.
//
// What response.ts decides is drawn and printed here, with the tab's DOM contract: every number
// a `[data-num][data-json]` path of the report (band labels, time ticks, the emission), every
// name a `[data-str]`, the colour bar's span `[data-label="span"]`. The map's canvas holds one
// pixel per bin (`mapPixels`), scaled up by CSS, so a pixel read back is the bin's colour.
import { useEffect, useMemo, useRef } from 'react';
import { createPortal } from 'react-dom';
import type { Report } from '../../bindings/ipc';
import { N, S } from './Marked';
import type { SourceSel } from './model';
import { colourAt, mapPixels, RESPONSE_NOTE, RESPONSE_TITLE, responseView, SPAN_DB, SPAN_LABELS, type ResponseView } from './response';

/** What the window shows, for the `responseView` test hook; null while it is closed. */
export interface ResponseHookView {
  receiver: number;
  source: string | null;
  k0: number;
  cols: number;
  paths: string[];
  max: { band: number; col: number };
}
let hookView: ResponseHookView | null = null;
export const responseHookView = (): ResponseHookView | null => hookView;

const css = (name: string, fallback: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;

function MapCanvas({ v }: { v: ResponseView }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const rows = v.map.db.length;
  const cols = v.map.db[0].length;
  useEffect(() => {
    const ctx = ref.current?.getContext('2d');
    if (!ctx) return;
    ctx.putImageData(new ImageData(mapPixels(v.map) as Uint8ClampedArray<ArrayBuffer>, cols, rows), 0, 0);
  }, [v, cols, rows]);
  return <canvas ref={ref} className="rw-map" data-part="response-map" width={cols} height={rows} />;
}

const BAR_PX = 120;
function ColourBar() {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const ctx = ref.current?.getContext('2d');
    if (!ctx) return;
    const img = ctx.createImageData(1, BAR_PX);
    for (let y = 0; y < BAR_PX; y++) {
      const [r, g, b] = colourAt(1 - (y + 0.5) / BAR_PX);
      img.data.set([r, g, b, 255], y * 4);
    }
    ctx.putImageData(img, 0, 0);
  }, []);
  return <canvas ref={ref} className="rw-bar-img" data-part="response-bar" width={1} height={BAR_PX} />;
}

/** The bands summed, dB re its maximum, as a line over the same time axis as the map. */
function Strip({ db }: { db: number[] }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const draw = () => {
      const dpr = devicePixelRatio || 1;
      const w = Math.max(1, Math.round(el.clientWidth * dpr));
      const h = Math.max(1, Math.round(el.clientHeight * dpr));
      el.width = w;
      el.height = h;
      const ctx = el.getContext('2d');
      if (!ctx) return;
      ctx.clearRect(0, 0, w, h);
      ctx.strokeStyle = css('--line', '#1d1d21');
      ctx.lineWidth = 1;
      for (const k of [1, 2]) {
        const y = Math.round((k / 3) * h) + 0.5;
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(w, y);
        ctx.stroke();
      }
      ctx.strokeStyle = css('--red', '#e0202e');
      ctx.lineWidth = 1.5 * dpr;
      ctx.beginPath();
      db.forEach((d, c) => {
        const x = ((c + 0.5) / db.length) * w;
        const y = (-d / SPAN_DB) * (h - dpr) + dpr / 2;
        if (c === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      });
      ctx.stroke();
    };
    draw();
    const ro = new ResizeObserver(draw);
    ro.observe(el);
    return () => ro.disconnect();
  }, [db]);
  return <canvas ref={ref} className="rw-strip" data-part="response-strip" />;
}

function SpanLabels({ ends }: { ends?: boolean }) {
  return (
    <>
      {SPAN_LABELS.filter((l) => !ends || l.db === 0 || l.db === -SPAN_DB).map((l) => (
        <span key={l.db} className="rw-span" style={{ top: `${(-l.db / SPAN_DB) * 100}%` }} data-label="span">
          {l.text}
        </span>
      ))}
    </>
  );
}

export function ResponseWindow({ report, receiver, source, onClose }: { report: Report; receiver: number; source: SourceSel; onClose: () => void }) {
  const v = useMemo(() => responseView(report, receiver, source), [report, receiver, source]);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [onClose]);
  hookView = v
    ? { receiver, source, k0: v.k0, cols: v.map.db[0].length, paths: v.bands.map((b) => b.path), max: { band: v.map.at.row, col: v.map.at.col } }
    : null;
  useEffect(
    () => () => {
      hookView = null;
    },
    [],
  );
  const cols = v ? v.map.db[0].length : 1;
  const emitted = v?.emission && (report.spps?.sources.length ?? 0) > 1 && source === null;

  return createPortal(
    <div className="rw glass" role="dialog" aria-label={RESPONSE_TITLE} data-response-window data-receiver={receiver} data-source={source ?? ''}>
      <div className="rw-head">
        <span className="rw-title" data-part="response-title">
          {RESPONSE_TITLE}
        </span>
        <span className="rw-sub">
          {v ? <S s={v.receiver} /> : null}
          {' · '}
          {v?.source ? <S s={v.source} /> : (report.spps?.sources.length ?? 0) > 1 ? 'all sources summed' : v?.emission ? <S s={v.emission.source} /> : null}
        </span>
        <button type="button" className="rw-close" data-action="close-response" aria-label="Close" title="Close (Esc)" onClick={onClose}>
          ×
        </button>
      </div>
      <div className="rw-note" data-part="response-note">
        {RESPONSE_NOTE}
      </div>
      {v ? (
        <div className="rw-grid" style={{ ['--rw-rows' as string]: String(v.bands.length) }}>
          <div className="rw-y" data-part="response-bands">
            {v.bands.map((b) => (
              <span key={b.path} className="rw-band">
                <N n={b.label.num} unit={b.label.unit} />
              </span>
            ))}
          </div>
          <MapCanvas v={v} />
          <div className="rw-bar">
            <ColourBar />
            <div className="rw-bar-labels">
              <SpanLabels />
            </div>
          </div>

          <div className="rw-y rw-y-strip">
            <SpanLabels ends />
          </div>
          {v.broadband ? <Strip db={v.broadband} /> : <div className="ac-none">No broadband decay.</div>}
          <div className="rw-strip-key">bands summed</div>

          <div />
          <div className="rw-x" data-part="response-ticks">
            {v.ticks.map((t) => (
              <span key={t.col} className="rw-tick" style={{ left: `${(t.col / cols) * 100}%` }}>
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
