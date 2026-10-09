// The properties panel (design:278-471), switched by step. Geometry and Sources & receivers are
// the project package's panels; Materials is the materials package's `MaterialsPanel`, which
// draws its own head (the selected group); Simulate and Results are the simulate package's
// `SimulatePanel` and `ResultsPanel` (M11). Each panel draws its own head, as the design does
// ("Room model", "Rear wall", "S1").
import { useEffect, useRef } from 'react';
import { MaterialsPanel } from '../features/materials/MaterialsPanel';
import { ResultsPanel } from '../features/simulate/ResultsPanel';
import { SimulatePanel } from '../features/simulate/SimulatePanel';
import { STEPS } from '../steps';
import { stepStore, useStore } from '../store';
import { GeometryPanel } from './GeometryPanel';
import { FoldButton, useFold } from './fold';
import { usePanelWidth } from './panelWidth';
import { SourcesPanel } from './SourcesPanel';

export function PropertiesPanel() {
  const step = useStore(stepStore);
  const s = STEPS.find((x) => x.key === step) ?? STEPS[0];
  const folded = useFold('props');
  const sized = usePanelWidth('nm-props-width', 344, 'left');
  // Which way the step bar moved, so the new step's options slide in from that side (motion.css).
  const index = STEPS.indexOf(s);
  const last = useRef(index);
  const dir = index >= last.current ? 'next' : 'prev';
  useEffect(() => {
    last.current = index;
  }, [index]);
  return (
    <aside className="props" aria-label="Properties" data-props-step={s.key} data-step-dir={dir} data-folded={folded} style={sized.style} onScroll={sized.onScroll}>
      <FoldButton panel="props" />
      {!folded && <div {...sized.grip} />}
      {s.key === 'geometry' && <GeometryPanel />}
      {s.key === 'materials' && <MaterialsPanel />}
      {s.key === 'sources' && <SourcesPanel />}
      {s.key === 'simulate' && <SimulatePanel />}
      {s.key === 'results' && <ResultsPanel />}
    </aside>
  );
}
