// The 3D view (design:106-173): the one canvas, view tabs, tools, plan inset, axis gizmo.
// Hand-over stub from the M10 foundation: M9's empty viewport, `data-part="viewport"` kept for
// the M9 self-test. The viewport package owns `features/viewport/**` from here (PLAN.md 6.1):
// one WebGLRenderer for the app's lifetime, three-mesh-bvh with `indirect: true`, and the hooks
// `highlightedFaceCount`, `selection`, `facesOfGroup`, `aimAtFace`, `frame` (testhooks.ts
// `registerHook`), with `viewportStore` set live and its drawn revision after each frame.
import { useState } from 'react';
import { AxisGizmo, MeasureTool, OrbitTool, ReceiverTool, SectionTool, SelectTool } from '../../chrome/icons';
import { sceneStore, useStore } from '../../store';
import { VIEWPORT_LIBRARIES } from './libraries';
import './viewport.css';

const VIEWS = ['Perspective', 'Plan', 'Section'] as const;
const TOOLS = [
  ['Select', SelectTool],
  ['Orbit', OrbitTool],
  ['Place receiver', ReceiverTool],
  ['Section plane', SectionTool],
  ['Measure', MeasureTool],
] as const;

export function Viewport() {
  const project = useStore(sceneStore)?.info ?? null;
  const [view, setView] = useState<(typeof VIEWS)[number]>('Perspective');
  const hasModel = !!project && project.faces > 0;
  return (
    <div className="viewport" data-part="viewport" data-libraries={VIEWPORT_LIBRARIES}>
      <div className="viewport-empty">
        <div className="title">{hasModel ? 'The 3D view arrives with the Geometry step' : 'No model loaded'}</div>
        <div className="empty">{hasModel ? project.name : 'File, then Open…'}</div>
      </div>
      <div className="overlay-tl segmented view float-panel" role="tablist" aria-label="View">
        {VIEWS.map((v) => (
          <button key={v} role="tab" aria-selected={v === view} onClick={() => setView(v)}>
            {v}
          </button>
        ))}
      </div>
      <div className="tools float-panel" role="toolbar" aria-label="Tools">
        {TOOLS.map(([label, Icon], i) => (
          <button key={label} className="tool" aria-label={label} aria-pressed={i === 0} aria-disabled="true">
            <Icon />
          </button>
        ))}
      </div>
      <div className="plan-inset float-panel">
        <div className="label">Plan</div>
        <div className="box" role="img" aria-label="Plan view, empty" />
      </div>
      <AxisGizmo />
    </div>
  );
}
