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
//
// M12 P3: on the Results step, `ResultsOverlay` adds the surface map's controls, its legend and
// the timeline the map and the particles share; the engine draws them (resultsLayer.ts).
import { useEffect, useRef, useState, type JSX } from 'react';
import * as actions from '../../actions';
import { MeasureTool, OrbitTool, ReceiverTool, SectionTool, SelectTool } from '../../chrome/icons';
import { MOVE_TO_LABEL, moveTargets } from '../../chrome/groupsModel';
import { displayName, REGROUP_LABEL, regroupFaces } from '../../chrome/sceneModel';
import { onWindowEntityKey } from '../../chrome/sceneUi';
import { sceneStore, selectionStore, stepStore, toolStore, useStore, type Tool } from '../../store';
import { parseStrictDecimal } from '../../numbers';
import { attachViewport, focusSelection, frameModel, hideStore, isolateWhyNot, presentStore, setIsolate, setPresent, setRoofOff, setTurntable, setView, toggleIsolate, viewportUi, type ViewMode } from './engine';
import { groupList } from './hide';
import { VIEWPORT_LIBRARIES } from './libraries';
import { liveStore, setLiveLook } from './liveView';
import { PARTICLE_LOOKS } from './rays';
import { ResultsOverlay } from './ResultsOverlay';
import { ViewStyleMenu } from './ViewStyleMenu';
import { closingProps, usePresence } from '../../chrome/usePresence';
import './viewport.css';

/** Place source: the source marker's dot and glow, in the toolbar's line style. */
const SourceTool = () => (
  <svg width="16" height="16" viewBox="0 0 20 20" fill="none" aria-hidden>
    <circle cx="10" cy="7" r="2.2" fill="currentColor" />
    <circle cx="10" cy="7" r="4.6" stroke="currentColor" strokeWidth="1.2" opacity="0.55" />
    <path d="M10 11.6V17M7 17h6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
  </svg>
);

/** Present: a screen with nothing over it, in the toolbar's line style. */
const PresentTool = () => (
  <svg width="16" height="16" viewBox="0 0 20 20" fill="none" aria-hidden>
    <rect x="2.5" y="4" width="15" height="10" rx="1" stroke="currentColor" strokeWidth="1.4" />
    <path d="M7 17h6M10 14v3" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
  </svg>
);

/** Turntable: an arc around a point. */
const TurntableTool = () => (
  <svg width="16" height="16" viewBox="0 0 20 20" fill="none" aria-hidden>
    <ellipse cx="10" cy="12" rx="7" ry="3" stroke="currentColor" strokeWidth="1.3" />
    <path d="M15.5 7.5l1.5 2.5-2.8.6" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
    <circle cx="10" cy="7" r="1.8" fill="currentColor" />
  </svg>
);

/** H: the presentation view on and off; Esc leaves it; O: the turntable while presenting. Not while typing. */
function onPresentKey(e: KeyboardEvent): void {
  if (e.ctrlKey || e.altKey || e.metaKey || e.shiftKey) return;
  const t = e.target;
  if (t instanceof HTMLElement && (t.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(t.tagName))) return;
  const p = presentStore.get();
  if (e.key === 'h' || e.key === 'H') setPresent(!p.on);
  else if (e.key === 'Escape' && p.on) setPresent(false);
  else if ((e.key === 'o' || e.key === 'O') && p.on) setTurntable(!p.turntable);
}

/** Item 7: R puts the roof off and back (View style > Hide > Roof off); item 8: I isolates the selection and shows
 * everything again. Not while typing, and only with a model. */
function onHideKey(e: KeyboardEvent): void {
  if (e.ctrlKey || e.altKey || e.metaKey || e.shiftKey || !viewportUi.get().hasModel) return;
  const t = e.target;
  if (t instanceof HTMLElement && (t.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(t.tagName))) return;
  if (e.key === 'r' || e.key === 'R') setRoofOff(!hideStore.get().roof);
  else if (e.key === 'i' || e.key === 'I') toggleIsolate();
}

/** The view's right-click menu opens with something picked (Focus, Isolate; New group and Move to group on faces) or while isolated (Show everything). */
function menuOffered(kind: string, isolated: boolean): boolean {
  return kind !== 'none' || isolated;
}

/** Frame model: the model's box inside four corner marks, in the toolbar's line style. */
const FrameTool = () => (
  <svg width="16" height="16" viewBox="0 0 20 20" fill="none" aria-hidden>
    <path d="M3 7V3h4M13 3h4v4M17 13v4h-4M7 17H3v-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    <rect x="7" y="7" width="6" height="6" rx="0.5" stroke="currentColor" strokeWidth="1.4" />
  </svg>
);

/** G48: the distance a placement click puts the receiver or source from the face, edited in the view (Enter or blur). */
function PlaceOffset({ kind }: { kind: 'receiver' | 'source' }) {
  const offsets = useStore(actions.placeOffsetStore);
  const value = String(offsets[kind]);
  const [draft, setDraft] = useState<string | null>(null);
  const [bad, setBad] = useState<string | null>(null);
  const commit = () => {
    if (draft === null) return;
    const p = parseStrictDecimal(draft);
    if (!p.ok || !(p.value > 0) || p.value > PLACE_OFFSET_MAX_M) {
      setBad(`"${draft}" is not a distance from 0 to ${PLACE_OFFSET_MAX_M} m: the ${kind} is placed ${value} m off the face`);
      return;
    }
    setBad(null);
    setDraft(null);
    actions.placeOffsetStore.set({ ...actions.placeOffsetStore.get(), [kind]: p.value });
  };
  return (
    <label className="vp-chip place-offset" data-part="place-offset" title={bad ?? `How far the ${kind} goes from the face clicked: straight up from a floor, along the normal from any other face`}>
      <span>Distance from the face</span>
      <input
        className="mono"
        data-field="place-offset"
        aria-label={`Distance of the ${kind} from the face, metres`}
        aria-invalid={bad !== null}
        spellCheck={false}
        value={draft ?? value}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === 'Enter') commit();
          else if (e.key === 'Escape') {
            setDraft(null);
            setBad(null);
          }
        }}
        onBlur={commit}
      />
      <span>m</span>
      {bad && <span className="vp-fail" data-part="place-offset-error">Not a distance</span>}
    </label>
  );
}

/** G48: the largest distance from a face the place hint takes, metres. */
const PLACE_OFFSET_MAX_M = 100;

const TOOLS: { key: Tool; label: string; title: string; Icon: () => JSX.Element; needsModel: boolean }[] = [
  { key: 'select', label: 'Select', title: 'Select: click a face; Ctrl+click adds or removes one, Shift+click adds; Shift+drag a box adds every face seen in it; double-click takes its whole flat surface', Icon: SelectTool, needsModel: false },
  { key: 'orbit', label: 'Orbit', title: 'Orbit: drag to turn the view; clicks select nothing', Icon: OrbitTool, needsModel: false },
  { key: 'place-receiver', label: 'Place receiver', title: 'Place receiver: click any face; it goes above a floor, or off a wall or ceiling into the room', Icon: ReceiverTool, needsModel: true },
  { key: 'place-source', label: 'Place source', title: 'Place source: click any face; it goes above a floor, or off a wall or ceiling into the room', Icon: SourceTool, needsModel: true },
];
const NOT_YET = [
  ['Section plane', SectionTool],
  ['Measure', MeasureTool],
] as const;

const LIVE_LOOK_LABELS = { dots: 'Dots', glow: 'Glow', rays: 'Rays' } as const;

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
  // Item 14: the closed menu plays its way out where it stood (motion.css).
  const { shown: shownMenu, closing: menuGoing } = usePresence(menu);
  const groups = useStore(sceneStore)?.view.surface_groups ?? [];
  const targets = selection.kind === 'faces' ? moveTargets(groups, selection.groups) : [];
  const live = useStore(liveStore);
  const step = useStore(stepStore);
  const rightDown = useRef<{ x: number; y: number } | null>(null);
  const hidden = useStore(hideStore);
  const offsets = useStore(actions.placeOffsetStore);

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
  // Round 2: the presentation view's keys.
  useEffect(() => {
    window.addEventListener('keydown', onPresentKey);
    return () => window.removeEventListener('keydown', onPresentKey);
  }, []);
  // Items 7 and 8: Roof off's and Isolate's keys.
  useEffect(() => {
    window.addEventListener('keydown', onHideKey);
    return () => window.removeEventListener('keydown', onHideKey);
  }, []);
  // Del and F2 on a source or receiver picked in the view (the panels handle their own).
  useEffect(() => {
    window.addEventListener('keydown', onWindowEntityKey);
    return () => window.removeEventListener('keydown', onWindowEntityKey);
  }, []);
  // What the menu acts on went away (a new mesh, nothing picked, Isolate ended): so does the menu.
  useEffect(() => {
    if (!menuOffered(selection.kind, hidden.isolate !== null)) setMenu(null);
  }, [selection, hidden.isolate]);

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
        if (!still || !box || (e.target as HTMLElement).tagName !== 'CANVAS' || !menuOffered(selectionStore.get().kind, hideStore.get().isolate !== null)) return;
        setMenu({ x: e.clientX - box.left, y: e.clientY - box.top });
      }}
    >
      <div className="viewport-host" ref={hostRef} />
      {shownMenu && (
        <div
          className="dropdown vp-menu"
          role="menu"
          aria-label="Selection"
          data-part="viewport-menu"
          style={{ left: shownMenu.x, top: shownMenu.y }}
          {...closingProps(menuGoing)}
          onMouseDown={(e) => e.stopPropagation()}
        >
          {/* Focus and Isolate, as View › Focus on selection (F) and View style › Hide › Isolate (I). */}
          <button
            role="menuitem"
            data-menu-item="focus"
            title="Frame what is picked from where you look now; with nothing picked, the model"
            onClick={() => {
              setMenu(null);
              focusSelection();
            }}
          >
            <span className="grow">Focus on selection</span>
            <span className="menu-keys">F</span>
          </button>
          <button
            role="menuitem"
            data-menu-item="isolate"
            disabled={!hidden.isolate && isolateWhyNot() !== null}
            title={hidden.isolate ? 'Show the faces Isolate hid' : (isolateWhyNot() ?? 'Show only the picked faces or surface groups')}
            onClick={() => {
              setMenu(null);
              toggleIsolate();
            }}
          >
            <span className="grow">{hidden.isolate ? 'Show everything' : 'Isolate selection'}</span>
            <span className="menu-keys">I</span>
          </button>
          {regroupFaces(selection) && (
            <>
              <div className="vp-menu-sep" role="separator" />
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
            </>
          )}
          {/* C1: Move selection to group, one entry per group that would change. */}
          {targets.length > 0 && (
            <div className="vp-menu-head" role="presentation">
              {MOVE_TO_LABEL}
            </div>
          )}
          <div className="vp-menu-list">
            {targets.map((g) => (
              <button
                key={g.id}
                role="menuitem"
                data-menu-item={`move-to-group:${g.id}`}
                title={`Move the picked faces into ${g.name}: they take its material`}
                onClick={() => {
                  setMenu(null);
                  actions.fire(actions.moveSelectionToGroup(g.id));
                }}
              >
                <span className="grow">{displayName(g.name)}</span>
              </button>
            ))}
          </div>
        </div>
      )}
      <div className="vp-labels" ref={labelsRef} aria-hidden />

      {(ui.error || !ui.hasModel) && (
        <div className="viewport-empty">
          {ui.error ? (
            <>
              <div className="title">
                <span className="vp-fail">Error</span> The 3D view cannot start
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

      {live && step === 'simulate' && (
        // B3: below the view bar, so a long caption never runs under it.
        <div className="vp-live">
          <div className="vp-chip live" data-part="live-caption" role="status" title="The particles the run saves (particles saved per source), drawn as each is traced; the Results step replays the same ones">
            <span className="live-dot" aria-hidden />
            <span data-part="live-caption-text">{live.caption}</span>
            <span className="live-looks" role="radiogroup" aria-label="Live particles">
              {PARTICLE_LOOKS.map((l) => (
                <button key={l} className="vp-chip-btn" role="radio" data-live-look={l} aria-checked={live.look === l} onClick={() => setLiveLook(l)}>
                  {LIVE_LOOK_LABELS[l]}
                </button>
              ))}
            </span>
            {live.lookNote && <span className="live-note">{live.lookNote}</span>}
          </div>
        </div>
      )}

      <div className="vp-chips">
        {n > 0 && (
          <div className="vp-chip fail" data-part="check-chip" role="status">
            <span className="swatch" aria-hidden />
            <span className="vp-fail">Problem</span> · {n} {n === 1 ? 'face' : 'faces'} highlighted
          </div>
        )}
        {placing && ui.hasModel && (
          <div className="vp-chip hint" data-part="place-hint" role="status">
            {ui.notice ?? `Click any face: the ${placeKind} goes ${offsets[placeKind]} m above a floor, or ${offsets[placeKind]} m off a wall or ceiling into the room. Esc ends.`}
          </div>
        )}
        {placing && ui.hasModel && <PlaceOffset kind={placeKind} />}
      </div>

      {/* Items 7 and 8: what the view leaves out, under the view bar (a long list would run under it at the top). */}
      {ui.hasModel && (hidden.roof || hidden.isolate) && (
        <div className={`vp-hide-chips${live && step === 'simulate' ? ' under-live' : ''}`}>
          {hidden.isolate && (
            <div className="vp-chip hidden-faces" data-part="isolate-chip" role="status" title={hidden.isolate.groups.map(displayName).join(', ')}>
              <span>
                Isolated · {hidden.isolate.faces} {hidden.isolate.faces === 1 ? 'face' : 'faces'} of {groupList(hidden.isolate.groups.map(displayName))}, the rest hidden
              </span>
              <button className="vp-chip-btn" data-action="isolate-off" title="Show everything again (I)" onClick={() => setIsolate(false)}>
                Show all
              </button>
            </div>
          )}
          {hidden.roof && (
            <div className="vp-chip hidden-faces" data-part="roof-off-chip" role="status" title={hidden.roofGroups.map(displayName).join(', ')}>
              <span>
                Roof off · {hidden.roofFaces === 0 ? 'no face closes this room from above' : `${hidden.roofFaces} ${hidden.roofFaces === 1 ? 'face' : 'faces'} hidden: ${groupList(hidden.roofGroups.map(displayName))}`}
              </span>
              <button className="vp-chip-btn" data-action="roof-on" title="Put the roof back (R)" onClick={() => setRoofOff(false)}>
                Show
              </button>
            </div>
          )}
        </div>
      )}

      <div className="overlay-tl view-bar">
      <div className="segmented view float-panel" role="tablist" aria-label="View">
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
      <ViewStyleMenu />
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
        <button
          className="tool"
          data-tool="present"
          aria-label="Present"
          title={ui.hasModel ? 'Present (H): the view alone, every panel hidden; H or Esc brings them back' : 'Present (H): no model to show'}
          disabled={!ui.hasModel}
          onClick={() => setPresent(true)}
        >
          <PresentTool />
        </button>
        <button
          className="tool"
          data-tool="turntable"
          aria-label="Turntable"
          title={ui.hasModel ? 'Turntable (O while presenting): the camera circles the room slowly; a drag stops it' : 'Turntable: no model to show'}
          disabled={!ui.hasModel}
          onClick={() => {
            setPresent(true);
            setTurntable(true);
          }}
        >
          <TurntableTool />
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

      {/* M12 P3: the Results step's map controls, legend and timeline (renders nothing elsewhere). */}
      <ResultsOverlay />

      {/* The axis gizmo (x red, y grey, z white), turned by the engine to follow the camera. */}
      <svg className="gizmo" ref={gizmoRef} width="60" height="60" viewBox="0 0 64 64" aria-hidden>
        <path data-axis="y" d="M32 32l12.3 8.4" stroke="#A1A1AA" strokeWidth="1.6" strokeLinecap="round" />
        <path data-axis="x" d="M32 32l16-6.6" stroke="#E0202E" strokeWidth="1.6" strokeLinecap="round" />
        <path data-axis="z" d="M32 32V15" stroke="#EDEDEF" strokeWidth="1.6" strokeLinecap="round" />
        <text data-axis-label="x" x="51" y="26" fontSize="9" fill="#E0202E" fontFamily="JetBrains Mono Variable, Cascadia Mono, Consolas, monospace">
          x
        </text>
        <text data-axis-label="y" x="46" y="48" fontSize="9" fill="#A1A1AA" fontFamily="JetBrains Mono Variable, Cascadia Mono, Consolas, monospace">
          y
        </text>
        <text data-axis-label="z" x="29" y="11" fontSize="9" fill="#EDEDEF" fontFamily="JetBrains Mono Variable, Cascadia Mono, Consolas, monospace">
          z
        </text>
      </svg>
    </div>
  );
}
