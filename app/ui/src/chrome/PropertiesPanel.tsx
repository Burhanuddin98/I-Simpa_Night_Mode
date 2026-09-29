// The properties panel (design:278-471), switched by step. The three M10 steps mount their
// panels here: Geometry and Sources & receivers (scene package), Materials (the materials
// package's `features/materials/MaterialsPanel`). Simulate and Results keep M9's hints.
// Hand-over stub from the M10 foundation; the scene package owns it from here (PLAN.md 6.3).
import { MaterialsPanel } from '../features/materials/MaterialsPanel';
import { STEPS } from '../steps';
import { stepStore, useStore } from '../store';
import { GeometryPanel } from './GeometryPanel';
import { SourcesPanel } from './SourcesPanel';

export function PropertiesPanel() {
  const step = useStore(stepStore);
  const s = STEPS.find((x) => x.key === step) ?? STEPS[0];
  return (
    <aside className="props" aria-label="Properties" data-props-step={s.key}>
      <div className="props-head">
        <div className="title">{s.name}</div>
        <div className="sub">{s.sub}</div>
      </div>
      {s.key === 'geometry' && <GeometryPanel />}
      {s.key === 'materials' && <MaterialsPanel />}
      {s.key === 'sources' && <SourcesPanel />}
      {(s.key === 'simulate' || s.key === 'results') && <div className="props-body empty">{s.hint}</div>}
    </aside>
  );
}
