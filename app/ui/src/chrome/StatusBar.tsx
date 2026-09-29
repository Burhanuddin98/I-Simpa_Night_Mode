// The status bar (design:474-481): the app's status, the model as a geometry fact ("Model closed
// · 6 surfaces · 180 m³", inside `[data-geometry]`), the active variant, the solvers, the units.
import { sceneStore, statusStore, useStore } from '../store';
import { fact, variantName } from './sceneModel';

export function StatusBar() {
  const status = useStore(statusStore);
  const scene = useStore(sceneStore);
  const check = scene?.check ?? null;
  const groups = scene?.info.surface_groups ?? 0;
  return (
    <footer className="statusbar">
      <span className={`status ${status.kind === 'ready' ? '' : status.kind}`}>
        <span className="dot" />
        {status.text}
      </span>
      {scene &&
        (check ? (
          <span data-geometry data-part="model-fact">
            {check.verdict === 'ok'
              ? `Model closed · ${groups} surfaces · ${fact(check.enclosed_volume_m3, 1)} m³`
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
