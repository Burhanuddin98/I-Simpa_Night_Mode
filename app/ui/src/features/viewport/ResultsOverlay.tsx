// The Results step's controls over the 3D view (M12 P3; design:109-113, :164-170, :436-450): the
// map's kind and band (R41), the difference from a baseline run (R52), the legend (R50), and the
// one timeline's transport (R39, R55) that moves the map and the particles together (R53). MQ4:
// a run that saved no particles says so here, with how to turn it on and what it costs.
//
// Every element that can hold a number is `[data-results-region]` (m10-h and m11-h read them only
// while the Results step is current), and nothing renders on any other step. The legend's
// numbers are the map's own range, from the `.csbin` (mapData.ts), not report parameters.
//
// The test hooks of gate (c) and (d) are registered here, on mount, on every step.
//
// W5 (wow list; mapView.ts): smooth colour, contours on it and a fixed colour range in the map
// panel, and the value probe: the map face under the pointer, its own `.csbin` record with its
// level, band and time, in a card by the pointer (`[data-part="map-probe"]`, a results region).
import { useEffect, useState } from 'react';
import { planesNotInRun, rerunText } from '../../chrome/planes';
import { sceneStore, stepStore, useStore } from '../../store';
import { registerHook } from '../../testhooks';
import { Animator, animatorStore } from './animator';
import { framePixels, mapFacePoint, mapPixels, mapPointerStore, offMapPoint, resultsLayer } from './engine';
import { CONTOUR_STEPS_DB, contourText, parseRange, probeOf, stepTime, type ProbeView } from './mapView';
import { bandLabel, bandName, resultsView, resultsViewStore, shownMaps, startResultsView } from './resultsView';

const PlayIcon = () => (
  <svg width="14" height="14" viewBox="0 0 20 20" fill="none" aria-hidden>
    <path d="M6 4l10 6-10 6z" fill="currentColor" />
  </svg>
);
const PauseIcon = () => (
  <svg width="14" height="14" viewBox="0 0 20 20" fill="none" aria-hidden>
    <path d="M6 4h3v12H6zM11 4h3v12h-3z" fill="currentColor" />
  </svg>
);

function registerM12Hooks(): () => void {
  const layer = resultsLayer();
  const offs = [
    registerHook('m12Map', () => {
      const meta = layer.mapMeta;
      if (!meta) return null;
      const s = layer.sizes();
      const legend = resultsViewStore.get().map?.legend ?? null;
      return {
        run: meta.run,
        path: meta.path,
        bandHz: meta.bandHz,
        kind: meta.kind,
        faces: s.faces,
        steps: s.steps,
        step: layer.mapStep(),
        texture: s.texture,
        legend,
        baseline: meta.baseline,
        error: null,
      };
    }),
    registerHook('m12MapPixels', () => mapPixels()),
    // W5: how the map is drawn, the node means the shader computes, a point over a face, the probe.
    registerHook('wowLook', () => ({ ...layer.look(), smoothRefusal: layer.smoothRefusal, fixed: resultsViewStore.get().fixed })),
    registerHook('wowNodeEnergies', (samples: [number, number][]) => layer.readNodes(samples)),
    registerHook('wowMapFacePoint', (face: number) => mapFacePoint(face)),
    registerHook('wowProbe', () => currentProbe()),
    registerHook('wowOffMapPoint', () => offMapPoint()),
    registerHook('wowFramePixels', () => framePixels()),
    registerHook('m12Texels', (samples: [number, number][]) => layer.readTexels(samples, 'texel')),
    registerHook('m12DiffTexels', (samples: [number, number][]) => layer.readTexels(samples, 'diff')),
    registerHook('m12SetStep', (step: number) => {
      Animator.pause();
      Animator.setStep(step);
      return animatorStore.get().step;
    }),
    registerHook('m12Particles', () => {
      const meta = layer.particleMeta;
      if (!meta) return null;
      return {
        run: meta.run,
        bandHz: meta.bandHz,
        step: layer.particleStep(),
        mapStep: layer.mapStep(),
        rendered: layer.countParticles(),
        particles: meta.particles,
        records: meta.records,
        bufferBytes: layer.sizes().particleBytes,
      };
    }),
  ];
  return () => offs.forEach((off) => off());
}

/** `120 ms`: the step's start time, from the run's own float32 time step. */
const timeText = stepTime;

/** The probe of the face under the pointer at the timeline's step: the shown map's own record. */
function currentProbe(): (ProbeView & { x: number; y: number }) | null {
  const at = mapPointerStore.get();
  const s = shownMaps();
  const v = resultsViewStore.get();
  if (!at || !s || !v.map || at.face >= s.map.faceCount || stepStore.get() !== 'results') return null;
  const step = Math.min(animatorStore.get().step, s.map.timeStepCount - 1);
  const dt = v.data?.time_step_s ?? v.data?.surfaces[0]?.time_step_s;
  const base = v.map.kind === 'diff' ? s.base : null;
  return { ...probeOf(s.map, at.face, step, { what: s.what, band: bandName(v.bandHz), dtS: dt, smooth: v.smooth, base }), x: at.x, y: at.y };
}

/** The probe card by the pointer. */
function Probe() {
  useStore(mapPointerStore);
  useStore(animatorStore);
  useStore(resultsViewStore);
  const p = currentProbe();
  if (!p) return null;
  return (
    <div
      className="vp-probe float-panel"
      data-part="map-probe"
      data-results-region
      data-probe-face={p.face}
      data-probe-step={p.step}
      data-probe-bits={p.bits === null ? '' : p.bits.toString(16)}
      style={{ left: p.x + 16, top: p.y + 16 }}
    >
      <div className="vp-probe-title" data-probe="title">
        {p.title}
      </div>
      {p.level !== null && (
        <div className="vp-probe-level mono" data-probe="level">
          {p.level}
        </div>
      )}
      <div className="vp-probe-value mono" data-probe="value">
        {p.value}
      </div>
      <div className="vp-probe-note" data-probe="face">
        Face {p.face} of the map{p.note ? `. ${p.note}` : ''}
      </div>
    </div>
  );
}

/** W5: the fixed range's two ends, committed together on Enter or blur; a refusal stays inline. Mounted
 * once per switch-on: the typed text is its own, so a commit of one end never resets the other. */
function RangeFields({ lo, hi }: { lo: number; hi: number }) {
  const [text, setText] = useState({ lo: String(lo), hi: String(hi) });
  const [refusal, setRefusal] = useState<string | null>(null);
  const commit = (next: { lo: string; hi: string }) => {
    const r = parseRange(next.lo, next.hi);
    if (!r.ok) {
      setRefusal(r.message);
      return;
    }
    setRefusal(null);
    resultsView.setFixedRange(r.range);
  };
  const field = (k: 'lo' | 'hi', label: string) => (
    <input
      className="vp-range-input mono"
      data-field={`range.${k}`}
      aria-label={label}
      aria-invalid={refusal !== null}
      spellCheck={false}
      value={text[k]}
      onChange={(e) => setText({ ...text, [k]: e.target.value })}
      onKeyDown={(e) => {
        if (e.key === 'Enter') commit(text);
      }}
      onBlur={() => commit(text)}
    />
  );
  return (
    <>
      <div className="vp-row" data-part="map-range">
        <span className="vp-row-label">From</span>
        {field('lo', 'Colour range low end in dB')}
        <span className="vp-row-label">to</span>
        {field('hi', 'Colour range high end in dB')}
        <span className="vp-row-label">dB</span>
      </div>
      {refusal && (
        <div className="vp-diff-note vp-refused" data-part="range-refused" role="alert">
          {refusal}. Not applied.
        </div>
      )}
    </>
  );
}

export function ResultsOverlay() {
  const step = useStore(stepStore);
  const v = useStore(resultsViewStore);
  const anim = useStore(animatorStore);
  const scene = useStore(sceneStore);

  useEffect(() => {
    startResultsView();
    return registerM12Hooks();
  }, []);

  if (step !== 'results' || v.status === 'off') return null;
  if (v.status !== 'ready') {
    return (
      <div className="vp-results-note float-panel" data-part="results-viewport" data-results-region data-results-view={v.status}>
        {v.status === 'loading' ? 'Reading the run…' : v.message}
      </div>
    );
  }
  const dt = v.data?.time_step_s ?? v.data?.surfaces[0]?.time_step_s;
  // W1: a plane added or renamed after this run has no map in it until SPPS runs again.
  const rerun = v.data?.solver === 'spps' ? rerunText(planesNotInRun(scene?.view.surface_receivers ?? [], v.data)) : null;
  const p = v.particles;
  return (
    <>
      <div className="vp-map-panel float-panel" data-part="results-viewport" data-results-region data-results-view={v.status}>
        {v.groups.length > 1 && (
          <div className="vp-row" role="radiogroup" aria-label="Map">
            {v.groups.map((g) => (
              <button key={g.key} className="vp-chip-btn" data-map-kind={g.key} aria-checked={v.group === g.key} role="radio" onClick={() => resultsView.setGroup(g.key)}>
                {g.label}
              </button>
            ))}
          </div>
        )}
        {v.groups.length === 1 && <div className="vp-map-title">{v.groups[0].label}</div>}
        {v.bands.length > 0 && (
          <div className="vp-row" role="radiogroup" aria-label="Band">
            <span className="vp-row-label">Band</span>
            {v.bands.map((b) => (
              <button
                key={String(b)}
                className="vp-chip-btn mono"
                data-map-band={b === null ? 'global' : String(b)}
                aria-checked={v.bandHz === b}
                role="radio"
                title={b === null ? 'All bands together (the solver’s Global map)' : `${b} Hz`}
                onClick={() => resultsView.setBand(b)}
              >
                {bandLabel(b)}
              </button>
            ))}
          </div>
        )}
        <button className="vp-switch" role="switch" aria-checked={v.diff} data-part="map-diff" onClick={() => resultsView.setDiff(!v.diff)}>
          <span>Difference from baseline</span>
          <span className="track" aria-hidden>
            <span className="knob" />
          </span>
        </button>
        {v.diff && (
          <div className="vp-diff-note" data-part="map-diff-note">
            {v.baselineReason ?? (v.baselineLabel ? `This run minus ${v.baselineLabel}, in dB. Blue is quieter, red louder.` : '')}
          </div>
        )}
        {rerun && (
          <div className="vp-diff-note vp-rerun" data-part="plane-rerun">
            {rerun}
          </div>
        )}
        {v.mapMessage && (
          <div className="vp-diff-note" data-part="map-message">
            {v.mapMessage}
          </div>
        )}

        <button className="vp-switch" role="switch" aria-checked={v.smooth} data-part="map-smooth" disabled={v.smoothRefusal !== null} onClick={() => resultsView.setSmooth(!v.smooth)}>
          <span>Smooth colour</span>
          <span className="track" aria-hidden>
            <span className="knob" />
          </span>
        </button>
        {v.smoothRefusal && (
          <div className="vp-diff-note" data-part="smooth-refused">
            Drawn flat: {v.smoothRefusal}.
          </div>
        )}
        <div className="vp-row" role="radiogroup" aria-label="Contours">
          <span className="vp-row-label">Contours</span>
          {[0, ...CONTOUR_STEPS_DB].map((db) => (
            <button
              key={db}
              className="vp-chip-btn mono"
              role="radio"
              data-map-contours={db}
              aria-checked={v.isoDb === db}
              disabled={!v.smooth && db > 0}
              title={db === 0 ? 'No contour lines' : v.smooth ? `A line every ${db} dB on the smoothed levels` : 'Contours follow the smooth colour: switch it on first'}
              onClick={() => resultsView.setContours(db)}
            >
              {db === 0 ? 'Off' : `${db} dB`}
            </button>
          ))}
        </div>
        <button
          className="vp-switch"
          role="switch"
          aria-checked={v.fixed !== null}
          data-part="map-fixed"
          disabled={v.map?.kind !== 'level' && v.fixed === null}
          title={v.map?.kind === 'diff' ? 'A difference keeps its own range, symmetric about 0 dB' : 'Hold the colour range while switching bands and runs'}
          onClick={() => resultsView.setFixedRange(v.fixed ? null : (v.map?.range ?? null))}
        >
          <span>Fixed colour range</span>
          <span className="track" aria-hidden>
            <span className="knob" />
          </span>
        </button>
        {v.fixed && <RangeFields lo={v.fixed.lo} hi={v.fixed.hi} />}
      </div>

      {v.map && (
        <div className="vp-legend float-panel" data-part="map-legend" data-results-region data-map-kind-shown={v.map.kind}>
          <div className="vp-legend-title">{v.map.legend.title}</div>
          <div className="vp-legend-bar" style={{ background: v.map.legend.gradient }} />
          <div className="vp-legend-labels">
            <span data-legend="lo">{v.map.legend.lo}</span>
            <span data-legend="mid">{v.map.legend.mid}</span>
            <span data-legend="hi">{v.map.legend.hi}</span>
          </div>
          {v.smooth && !v.smoothRefusal && (
            <div className="vp-legend-note" data-part="legend-note">
              Smoothed between faces; the probe reads each face's own value.{v.isoDb > 0 ? ` ${contourText(v.isoDb)}.` : ''}
            </div>
          )}
        </div>
      )}
      <Probe />

      <div className="vp-transport float-panel" data-part="animator" data-results-region>
        {p.state === 'none' && (
          <div className="vp-particles-none" data-part="particles-none">
            <div data-part="particles-none-title" className="title">
              {p.title}
            </div>
            <div data-part="particles-none-how">{p.how}</div>
          </div>
        )}
        {p.state === 'error' && <div className="vp-particles-none">{p.message}</div>}
        <div className="vp-row">
          <button
            className="tool"
            data-part="anim-play"
            aria-label={anim.playing ? 'Pause' : 'Play'}
            aria-pressed={anim.playing}
            disabled={anim.steps <= 1}
            onClick={() => (anim.playing ? Animator.pause() : Animator.play())}
          >
            {anim.playing ? <PauseIcon /> : <PlayIcon />}
          </button>
          <input
            type="range"
            className="vp-step"
            data-part="anim-step"
            aria-label="Time step"
            min={0}
            max={Math.max(0, anim.steps - 1)}
            step={1}
            value={anim.step}
            onChange={(e) => {
              Animator.pause();
              Animator.setStep(Number(e.target.value));
            }}
          />
          <span className="vp-time mono" data-part="anim-time">
            {timeText(anim.step, dt)}
          </span>
          {p.state === 'shown' && (
            <span className="vp-row-label" data-part="particles-band">
              Particles {bandLabel(p.bandHz)}
            </span>
          )}
        </div>
      </div>
    </>
  );
}
