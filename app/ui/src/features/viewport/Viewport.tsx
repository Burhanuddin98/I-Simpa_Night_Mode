// The 3D view (design:106-173): the one canvas, the view tabs, the tools, the plan inset frame,
// the check chip, the marker labels and the axis gizmo. The drawing is engine.ts's; this
// component is the DOM around it, and mounts the engine into it once (PLAN.md 6.1).
// `data-part="viewport"` stays on the root for the M9 self-test.
import { useEffect, useRef, type JSX } from 'react';
import * as actions from '../../actions';
import { MeasureTool, OrbitTool, ReceiverTool, SectionTool, SelectTool } from '../../chrome/icons';
import { sceneStore, toolStore, useStore, type Tool } from '../../store';
import { attachViewport, setView, viewportUi, type ViewMode } from './engine';
import { VIEWPORT_LIBRARIES } from './libraries';
import './viewport.css';

/** Place source: the source marker's dot and glow, in the toolbar's line style. */
const SourceTool = () => (
  <svg width="16" height="16" viewBox="0 0 20 20" fill="none" aria-hidden>
    <circle cx="10" cy="7" r="2.2" fill="currentColor" />
    <circle cx="10" cy="7" r="4.6" stroke="currentColor" strokeWidth="1.2" opacity="0.55" />
    <path d="M10 11.6V17M7 17h6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);

const TOOLS: { key: Tool; label: string; title: string; Icon: () => JSX.Element; needsModel: boolean }[] = [
  { key: 'select', label: 'Select', title: 'Select: click a face; double-click takes its whole flat surface', Icon: SelectTool, needsModel: false },
  { key: 'orbit', label: 'Orbit', title: 'Orbit: drag to turn the view; clicks select nothing', Icon: OrbitTool, needsModel: false },
  { key: 'place-receiver', label: 'Place receiver', title: 'Place receiver: click a floor', Icon: ReceiverTool, needsModel: true },
  { key: 'place-source', label: 'Place source', title: 'Place source: click a floor', Icon: SourceTool, needsModel: true },
];
const NOT_YET = [
  ['Section plane', SectionTool],
  ['Measure', MeasureTool],
] as const;

const VIEWS: { key: ViewMode | 'section'; label: string }[] = [
  { key: 'perspective', label: 'Perspective' },
  { key: 'plan', label: 'Plan' },
  { key: 'section', label: 'Section' },
];

export function Viewport() {
  const rootRef = useRef<HTMLDivElement>(null);
  const hostRef = useRef<HTMLDivElement>(null);
  const insetRef = useRef<HTMLDivElement>(null);
  const labelsRef = useRef<HTMLDivElement>(null);
  const gizmoRef = useRef<SVGSVGElement>(null);
  const ui = useStore(viewportUi);
  const tool = useStore(toolStore);
  const info = useStore(sceneStore)?.info ?? null;

  useEffect(() => {
    const [root, host, inset, labels, gizmo] = [rootRef.current, hostRef.current, insetRef.current, labelsRef.current, gizmoRef.current];
    if (!root || !host || !inset || !labels || !gizmo) return;
    return attachViewport({ root, host, inset, labels, gizmo });
  }, []);

  const placing = tool === 'place-receiver' || tool === 'place-source';
  const placeKind = tool === 'place-source' ? 'source' : 'receiver';
  const n = ui.highlightCount;

  return (
    <div
      className="viewport"
      data-part="viewport"
      data-view-mode={ui.view}
      data-active-tool={tool}
      data-libraries={VIEWPORT_LIBRARIES}
      ref={rootRef}
    >
      <div className="viewport-host" ref={hostRef} />
      <div className="vp-labels" ref={labelsRef} aria-hidden />

      {(ui.error || !ui.hasModel) && (
        <div className="viewport-empty">
          {ui.error ? (
            <>
              <div className="title">
                <span className="vp-fail">FAIL</span> The 3D view cannot start
              </div>
              <div className="empty">{ui.error}</div>
            </>
          ) : info ? (
            <>
              <div className="title">No room model in this project</div>
              <div className="empty">Geometry, then Import model…</div>
            </>
          ) : (
            <>
              <div className="title">No model loaded</div>
              <div className="empty">File, then Open…</div>
            </>
          )}
        </div>
      )}

      <div className="vp-chips">
        {n > 0 && (
          <div className="vp-chip fail" data-part="check-chip" role="status">
            <span className="swatch" aria-hidden />
            <span className="vp-fail">FAIL</span> · {n} {n === 1 ? 'face' : 'faces'} highlighted
          </div>
        )}
        {placing && ui.hasModel && (
          <div className="vp-chip hint" data-part="place-hint" role="status">
            {ui.notice ?? `Click a floor: the ${placeKind} goes ${actions.PLACE_HEIGHT_M[placeKind]} m above it. Esc ends.`}
          </div>
        )}
      </div>

      <div className="overlay-tl segmented view float-panel" role="tablist" aria-label="View">
        {VIEWS.map((v) =>
          v.key === 'section' ? (
            <button key={v.key} role="tab" data-view={v.key} aria-selected={false} disabled aria-disabled="true" title="Section: not in this version">
              {v.label}
            </button>
          ) : (
            <button
              key={v.key}
              role="tab"
              data-view={v.key}
              aria-selected={ui.view === v.key}
              onClick={() => setView(v.key as ViewMode)}
            >
              {v.label}
            </button>
          ),
        )}
      </div>

      <div className="tools float-panel" role="toolbar" aria-label="Tools">
        {TOOLS.map(({ key, label, title, Icon, needsModel }) => (
          <button
            key={key}
            className="tool"
            data-tool={key}
            aria-label={label}
            title={title}
            aria-pressed={tool === key}
            disabled={needsModel && !ui.hasModel}
            onClick={() => toolStore.set(key)}
          >
            <Icon />
          </button>
        ))}
        {NOT_YET.map(([label, Icon]) => (
          <button key={label} className="tool" aria-label={label} title={`${label}: not in this version`} aria-pressed={false} disabled aria-disabled="true">
            <Icon />
          </button>
        ))}
      </div>

      <div
        className="plan-inset"
        data-part="plan-inset"
        style={ui.hasModel && ui.view === 'perspective' ? undefined : { display: 'none' }}
        aria-label={ui.planLabel ? `Plan · ${ui.planLabel}` : 'Plan'}
      >
        <div className="plan-head">
          <span className="label">Plan</span>
          <span className="dims" data-geometry>
            {ui.planLabel}
          </span>
        </div>
        <div className="plan-box" ref={insetRef} role="img" aria-label="Plan view from above" />
      </div>

      {/* The axis gizmo (x red, y grey, z white), turned by the engine to follow the camera. */}
      <svg className="gizmo" ref={gizmoRef} width="60" height="60" viewBox="0 0 64 64" aria-hidden>
        <path data-axis="y" d="M32 32l12.3 8.4" stroke="#A1A1AA" strokeWidth="1.6" strokeLinecap="round" />
        <path data-axis="x" d="M32 32l16-6.6" stroke="#E0202E" strokeWidth="1.6" strokeLinecap="round" />
        <path data-axis="z" d="M32 32V15" stroke="#EDEDEF" strokeWidth="1.6" strokeLinecap="round" />
        <text data-axis-label="x" x="51" y="26" fontSize="9" fill="#E0202E" fontFamily="JetBrains Mono, monospace">
          x
        </text>
        <text data-axis-label="y" x="46" y="48" fontSize="9" fill="#A1A1AA" fontFamily="JetBrains Mono, monospace">
          y
        </text>
        <text data-axis-label="z" x="29" y="11" fontSize="9" fill="#EDEDEF" fontFamily="JetBrains Mono, monospace">
          z
        </text>
      </svg>
    </div>
  );
}
