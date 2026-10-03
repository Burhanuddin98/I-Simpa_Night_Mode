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
import { useEffect } from 'react';
import { stepStore, useStore } from '../../store';
import { registerHook } from '../../testhooks';
import { Animator, animatorStore } from './animator';
import { mapPixels, resultsLayer } from './engine';
import { bandLabel, resultsView, resultsViewStore, startResultsView } from './resultsView';

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
function timeText(step: number, dt: number | null | undefined): string {
  if (!dt) return `step ${step}`;
  const ms = step * dt * 1000;
  return `${ms < 100 ? ms.toFixed(1) : Math.round(ms)} ms`;
}

export function ResultsOverlay() {
  const step = useStore(stepStore);
  const v = useStore(resultsViewStore);
  const anim = useStore(animatorStore);

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
        {v.mapMessage && (
          <div className="vp-diff-note" data-part="map-message">
            {v.mapMessage}
          </div>
        )}
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
        </div>
      )}

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
