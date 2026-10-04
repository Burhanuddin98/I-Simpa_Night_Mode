// The response window (Burhan 2026-10-04 11:45): the selected receiver's energy echogram, from
// the selected source (or the sources summed), as a time x octave-band map in the Dockyard
// black-red-white map, with its colour bar, and the bands summed as a decay strip under it.
// Opened from the Acoustics tab's Decay card, floating over the app on the glass of decision-log
// row 50, pinned at the centre; closed by its button or Escape. Rendered into <body>, so the
// tab's own scans (m12-a) never see it; m12.response.e2e.ts reads it.
//
// What response.ts decides is drawn and printed here, with the tab's DOM contract: every number
// a `[data-num][data-json]` path of the report (band labels, time ticks, the emission), every
// name a `[data-str]`, the colour bar's span `[data-label="span"]`. No canvas (M10 PLAN rule 3):
// the map is a PNG of one pixel per bin (`mapPixels`, png.ts), scaled up by CSS, so a pixel
// decoded from it is the bin's colour; the decay strip and its marks are SVG, the colour bar a
// CSS gradient of the same stops.
import { useEffect, useMemo, useState } from 'react';
import { createPortal } from 'react-dom';
import type { Report } from '../../bindings/ipc';
import { N, S } from './Marked';
import type { SourceSel } from './model';
import { pngDataUrl } from './png';
import { colourBarCss, mapPixels, RESPONSE_NOTE, RESPONSE_TITLE, responseView, SPAN_DB, SPAN_LABELS, type ResponseView } from './response';

/** What the window shows, for the `responseView` test hook; null while it is closed. */
export interface ResponseHookView {
  receiver: number;
  source: string | null;
  k0: number;
  cols: number;
  /** The steps drawn: `cols`, or the crop's when the map is cut short. */
  shown: number;
  /** The crop's steps (null: none, the map is always the full run), and whether "Full run" is on. */
  crop: number | null;
  full: boolean;
  paths: string[];
  max: { band: number; col: number };
}
let hookView: ResponseHookView | null = null;
export const responseHookView = (): ResponseHookView | null => hookView;

/** The map as an image (M10 PLAN rule 3, one canvas: the 3D view's is the only one): one pixel
 * per bin (`mapPixels`), a PNG in a data URL (png.ts), scaled up by CSS with `pixelated`, so a
 * pixel decoded from it is the bin's colour. */
function MapImage({ v, cols }: { v: ResponseView; cols: number }) {
  const rows = v.map.db.length;
  const src = useMemo(() => pngDataUrl(mapPixels(v.map, SPAN_DB, cols), cols, rows), [v, cols, rows]);
  return <img className="rw-map" data-part="response-map" src={src} width={cols} height={rows} alt="Level per band and time step" draggable={false} />;
}

/** The colour bar: the map's stops as a CSS gradient, the span's top at the top. */
function ColourBar() {
  return <div className="rw-bar-img" data-part="response-bar" style={{ backgroundImage: colourBarCss() }} />;
}

/** The bands summed, dB re its maximum, as a line over the same time axis as the map, with the
 * span's thirds and the time ticks' marks, in SVG: x in steps (0 to `db.length`), y in dB below
 * the maximum (0 to `SPAN_DB`), stretched to the box; strokes keep their width. */
function Strip({ db, ticks }: { db: number[]; ticks: readonly number[] }) {
  const n = db.length;
  const points = db.map((d, c) => `${c + 0.5},${-d}`).join(' ');
  return (
    <svg className="rw-strip" data-part="response-strip" viewBox={`0 0 ${n} ${SPAN_DB}`} preserveAspectRatio="none" aria-hidden="true">
      {[1, 2].map((k) => (
        <line key={k} className="rw-strip-grid" x1={0} x2={n} y1={(k * SPAN_DB) / 3} y2={(k * SPAN_DB) / 3} vectorEffect="non-scaling-stroke" />
      ))}
      {ticks.map((c) => (
        <line key={`t${c}`} className="rw-strip-tick" x1={c} x2={c} y1={SPAN_DB * 0.93} y2={SPAN_DB} vectorEffect="non-scaling-stroke" />
      ))}
      <polyline className="rw-strip-line" points={points} vectorEffect="non-scaling-stroke" />
    </svg>
  );
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

/** The span's bottom as a word in a sentence, marked as the colour bar's words are. */
function SpanWord() {
  const l = SPAN_LABELS[SPAN_LABELS.length - 1];
  return (
    <span className="rw-span-word" data-label="span">
      {l.text}
    </span>
  );
}

export function ResponseWindow({ report, receiver, source, onClose }: { report: Report; receiver: number; source: SourceSel; onClose: () => void }) {
  const v = useMemo(() => responseView(report, receiver, source), [report, receiver, source]);
  // The time axis ends shortly after the last bin above the floor (response.ts `cropCols`);
  // "Full run" shows the whole duration. The choice holds across receivers and sources.
  const [fullRun, setFullRun] = useState(false);
  const cropped = !!v?.crop && !fullRun;
  const shown = v ? (cropped && v.crop ? v.crop.cols : v.map.db[0].length) : 1;
  const ticks = v ? (cropped && v.crop ? v.crop.ticks : v.ticks) : [];
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [onClose]);
  hookView = v
    ? { receiver, source, k0: v.k0, cols: v.map.db[0].length, shown, crop: v.crop?.cols ?? null, full: !cropped, paths: v.bands.map((b) => b.path), max: { band: v.map.at.row, col: v.map.at.col } }
    : null;
  useEffect(
    () => () => {
      hookView = null;
    },
    [],
  );
  const cols = shown;
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
          <MapImage v={v} cols={shown} />
          <div className="rw-bar">
            <ColourBar />
            <div className="rw-bar-labels">
              <SpanLabels />
            </div>
          </div>

          <div className="rw-y rw-y-strip">
            <SpanLabels ends />
          </div>
          {v.broadband ? <Strip db={v.broadband.slice(0, shown)} ticks={ticks.map((t) => t.col)} /> : <div className="ac-none">No broadband decay.</div>}
          <div className="rw-strip-key">bands summed</div>

          <div />
          <div className="rw-x" data-part="response-ticks">
            {ticks.map((t) => (
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

          <div />
          <div className="rw-shown" data-part="response-shown" data-shown={cropped ? 'crop' : 'full'}>
            <span>
              {cropped && v.crop ? (
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
                  ; from <N n={v.floor} unit="s" /> every band is at <SpanWord /> or lower
                </>
              ) : (
                <>
                  ; a band is still above <SpanWord /> at its end
                </>
              )}
            </span>
            {v.crop ? (
              <button
                type="button"
                className={`rw-full${fullRun ? ' on' : ''}`}
                data-action="response-full"
                aria-pressed={fullRun}
                title={fullRun ? 'Show the map up to where every band has reached the floor' : 'Show the whole duration of the run'}
                onClick={() => setFullRun((f) => !f)}
              >
                Full run
              </button>
            ) : null}
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
