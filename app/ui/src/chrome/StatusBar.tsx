// The status bar (design:474-481): the app's status, the model as a geometry fact ("Model closed
// · 6 surfaces · 180 m³", inside `[data-geometry]`), the active variant, the solvers, the units.
// While a run is active the status reads "Simulating", with SPPS's own last percentage in a
// diagnostic span (M11 PLAN.md 3.3, 3.4 rule 1): the progress text exactly as the solver printed
// it, never a number the UI formatted.
import { runStore, sceneStore, statusStore, useStore } from '../store';
import { fact, variantName } from './sceneModel';

export function StatusBar() {
  const status = useStore(statusStore);
  const scene = useStore(sceneStore);
  const active = useStore(runStore);
  const check = scene?.check ?? null;
  const groups = scene?.info.surface_groups ?? 0;
  return (
    <footer className="statusbar">
      {active ? (
        <span className="status busy" data-part="run-status">
          <span className="dot" />
          {active.status === 'cancelling' ? 'Cancelling' : 'Simulating'}
          {active.progressText && (
            <>
              {' · '}
              <span data-diagnostic="progress_pct" data-run={active.run ?? ''}>{`${active.progressText} %`}</span>
            </>
          )}
        </span>
      ) : (
        <span className={`status ${status.kind === 'ready' ? '' : status.kind}`}>
          <span className="dot" />
          {status.text}
        </span>
      )}
      {scene &&
        (check ? (
          <span data-geometry data-part="model-fact">
            {check.verdict === 'ok'
              ? `Model closed · ${groups} surfaces · ${check.enclosed_volume_m3 === null ? '' : `${fact(check.enclosed_volume_m3, 1)} m³`}`
              : `Model refused · ${groups} surfaces`}
          </span>
        ) : (
          <span data-part="model-fact">No model</span>
        ))}
      {scene && <span data-part="variant">{variantName(scene.view)}</span>}
      <span className="grow" />
      <span>Solvers: I-Simpa 1.4.0 · SPPS, TCR</span>
      <span>Units: m</span>
    </footer>
  );
}
