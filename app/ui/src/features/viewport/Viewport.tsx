// The 3D view (design:106-173): the one canvas, the view tabs, the tools, the plan inset frame,
// the check chip, the marker labels and the axis gizmo. The drawing is engine.ts's; this
// component is the DOM around it, and mounts the engine into it once (PLAN.md 6.1).
// `data-part="viewport"` stays on the root for the M9 self-test.
//
// M11 (row 22, G42): Frame model at the foot of the tools, `data-tool="frame"`, the same
// `frameModel` as View › Frame model and the Home key. An action, not a mode: it has no pressed
// state and leaves the tool as it was.
//
// Scope row 15 (1), G19: a right click on the model that does not drag (a drag pans) opens the
// context menu, `data-part="viewport-menu"`, while faces are picked: New group from selection,
// the same action as Edit › New group from selection.
import { useEffect, useRef, useState, type JSX } from 'react';
import * as actions from '../../actions';
import { MeasureTool, OrbitTool, ReceiverTool, SectionTool, SelectTool } from '../../chrome/icons';
import { REGROUP_LABEL, regroupFaces } from '../../chrome/sceneModel';
import { sceneStore, selectionStore, toolStore, useStore, type Tool } from '../../store';
import { attachViewport, frameModel, setView, viewportUi, type ViewMode } from './engine';
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

/** Frame model: the model's box inside four corner marks, in the toolbar's line style. */
const FrameTool = () => (
  <svg width="16" height="16" viewBox="0 0 20 20" fill="none" aria-hidden>
    <path d="M3 7V3h4M13 3h4v4M17 13v4h-4M7 17H3v-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    <rect x="7" y="7" width="6" height="6" rx="0.5" stroke="currentColor" strokeWidth="1.4" />
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
  const selection = useStore(selectionStore);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const rightDown = useRef<{ x: number; y: number } | null>(null);

  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    const esc = (e: KeyboardEvent) => e.key === 'Escape' && close();
    window.addEventListener('mousedown', close);
    window.addEventListener('keydown', esc);
    return () => {
      window.removeEventListener('mousedown', close);
      window.removeEventListener('keydown', esc);
    };
  }, [menu]);
  // The picked faces went away (a new mesh, another pick): so does the menu.
  useEffect(() => {
    if (!regroupFaces(selection)) setMenu(null);
  }, [selection]);

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
      onPointerDown={(e) => {
        if (e.button === 2) rightDown.current = { x: e.clientX, y: e.clientY };
      }}
      onContextMenu={(e) => {
        const down = rightDown.current;
        rightDown.current = null;
        const still = !down || Math.hypot(e.clientX - down.x, e.clientY - down.y) <= 4;
        const box = rootRef.current?.getBoundingClientRect();
        if (!still || !box || (e.target as HTMLElement).tagName !== 'CANVAS' || !regroupFaces(selectionStore.get())) return;
        setMenu({ x: e.clientX - box.left, y: e.clientY - box.top });
      }}
    >
      <div className="viewport-host" ref={hostRef} />
      {menu && (
        <div
          className="dropdown vp-menu"
          role="menu"
          aria-label="Selection"
          data-part="viewport-menu"
          style={{ left: menu.x, top: menu.y }}
          onMouseDown={(e) => e.stopPropagation()}
        >
          <button
            role="menuitem"
            data-menu-item="new-group-from-selection"
            onClick={() => {
              setMenu(null);
              actions.fire(actions.regroupSelection());
            }}
          >
            <span className="grow">{REGROUP_LABEL}</span>
            <span className="menu-keys">
              {regroupFaces(selection)?.length ?? 0} {regroupFaces(selection)?.length === 1 ? 'face' : 'faces'}
            </span>
          </button>
        </div>
      )}
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
        <span className="tool-sep" aria-hidden />
        <button
          className="tool"
          data-tool="frame"
          aria-label="Frame model"
          title={ui.hasModel ? 'Frame model (Home)' : 'Frame model (Home): no model to frame'}
          disabled={!ui.hasModel}
          onClick={() => frameModel()}
        >
          <FrameTool />
        </button>
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
