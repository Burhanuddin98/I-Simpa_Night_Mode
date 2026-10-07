// The status bar (design:474-481): the app's status, the model as a geometry fact ("Model closed
// · 6 surfaces · air 180 m³", inside `[data-geometry]`), the active variant, the solvers, the units.
// The volume is the air's (backlog 85): the inside of a closed obstacle, a radiator or a stage
// panel, is not in it (BRAS CR4: air 8656.6 m³, where the faces enclose 8695.7).
// While a run is active the status reads "Simulating", with SPPS's own last percentage in a
// diagnostic span (M11 PLAN.md 3.3, 3.4 rule 1): SPPS's text after its `#`, as the simulate
// package's `progressDisplay` shows it everywhere (as printed up to two decimals, else rounded
// half up in integers). SPPS prints 4 significant digits, so its raw text below 10 % ("1.667e-05",
// "0.0006667") would break the progress_pct grammar m11-h proves.
import { progressDisplay } from '../features/simulate/model';
import { gpuStatusStore, runStore, sceneStore, statusStore, useStore } from '../store';
import { fact, variantName } from './sceneModel';

export function StatusBar() {
  const status = useStore(statusStore);
  const scene = useStore(sceneStore);
  const active = useStore(runStore);
  // A5: the GPU solver is listed once the session's probe found a device.
  const gpu = useStore(gpuStatusStore);
  const check = scene?.check ?? null;
  const groups = scene?.info.surface_groups ?? 0;
  const progress = active && active.status !== 'cancelling' ? progressDisplay(active.progressText) : null;
  return (
    <footer className="statusbar">
      {active ? (
        <span className="status busy" data-part="run-status">
          <i className="led" data-on="warn" aria-hidden="true" />
          <span className="dot" />
          {active.status === 'cancelling' ? 'Cancelling' : 'Simulating'}
          {progress !== null && (
            <>
              {' · '}
              <span data-diagnostic="progress_pct" data-run={active.run ?? ''}>{`${progress} %`}</span>
            </>
          )}
        </span>
      ) : (
        <span className={`status ${status.kind === 'ready' ? '' : status.kind}`}>
          <i className="led" data-on={status.kind === 'ready' ? 'ok' : status.kind === 'busy' ? 'warn' : 'fail'} aria-hidden="true" />
          <span className="dot" />
          {status.text}
        </span>
      )}
      {scene &&
        (check ? (
          <span data-geometry data-part="model-fact">
            {check.verdict === 'ok'
              ? `Model closed · ${groups} surfaces · ${check.air_volume_m3 == null ? '' : `air ${fact(check.air_volume_m3, 1)} m³`}`
              : `Model refused · ${groups} surfaces`}
          </span>
        ) : (
          <span data-part="model-fact">No model</span>
        ))}
      {scene && <span data-part="variant">{variantName(scene.view)}</span>}
      <span className="grow" />
      <span data-part="solvers" title={gpu?.available ? (gpu.device ?? undefined) : undefined}>
        {`Solvers: I-Simpa 1.4.0 · SPPS, TCR${gpu?.available ? ' · SPPS on the GPU' : ''}`}
      </span>
      <span>Units: m</span>
    </footer>
  );
}
