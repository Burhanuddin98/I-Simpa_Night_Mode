// The properties panel (design:278-471), switched by step. Geometry and Sources & receivers are
// the scene package's panels; Materials is the materials package's `MaterialsPanel`, which draws
// its own head (the selected group); Simulate and Results keep M9's hints until M11 and M12.
// Each panel draws its own head, as the design does ("Room model", "Rear wall", "S1").
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
      {s.key === 'geometry' && <GeometryPanel />}
      {s.key === 'materials' && <MaterialsPanel />}
      {s.key === 'sources' && <SourcesPanel />}
      {(s.key === 'simulate' || s.key === 'results') && (
        <>
          <div className="props-head">
            <div className="title">{s.name}</div>
            <div className="sub">{s.sub}</div>
          </div>
          <div className="props-body empty">{s.hint}</div>
        </>
      )}
    </aside>
  );
}
