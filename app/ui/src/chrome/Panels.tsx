// The Concept B panels, empty until M10 fills them: the scene list, the 3D view, the dock and
// the properties panel. No acoustic number appears anywhere (physics-before-pixels rule).
import { useState } from 'react';
import type { ProjectInfo } from '../backend';
import { STEPS, type StepKey } from '../steps';
import { consoleStore, useStore } from '../store';
import { AxisGizmo, MeasureTool, OrbitTool, ReceiverTool, Search, SectionTool, SelectTool } from './icons';

function SceneSection({ title, count, empty }: { title: string; count: number | null; empty: string }) {
  return (
    <>
      <div className="scene-head label">
        <span>{title}</span>
        {count !== null && <span className="count">{count}</span>}
      </div>
      {!count && <div className="scene-empty empty">{empty}</div>}
    </>
  );
}

export function ScenePanel({ project }: { project: ProjectInfo | null }) {
  const n = (x: number | undefined) => (project ? (x ?? 0) : null);
  return (
    <aside className="scene" aria-label="Scene">
      <div className="filter">
        <Search size={12} />
        Filter scene
      </div>
      <div className="scene-list">
        <SceneSection title="Surfaces" count={n(project?.surface_groups)} empty="No surfaces" />
        <SceneSection title="Sources" count={n(project?.sources)} empty="No sources" />
        <SceneSection
          title="Receivers"
          count={project ? project.point_receivers + project.surface_receivers : null}
          empty="No receivers"
        />
      </div>
    </aside>
  );
}

const VIEWS = ['Perspective', 'Plan', 'Section'] as const;
const TOOLS = [
  ['Select', SelectTool],
  ['Orbit', OrbitTool],
  ['Place receiver', ReceiverTool],
  ['Section plane', SectionTool],
  ['Measure', MeasureTool],
] as const;

export function Viewport({ project }: { project: ProjectInfo | null }) {
  const [view, setView] = useState<(typeof VIEWS)[number]>('Perspective');
  const hasModel = !!project && project.faces > 0;
  return (
    <div className="viewport" data-part="viewport">
      <div className="viewport-empty">
        <div className="title">{hasModel ? 'The 3D view arrives with the Geometry step' : 'No model loaded'}</div>
        <div className="empty">{hasModel ? project.name : 'File, then Open project…'}</div>
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

const DOCK_TABS = [
  { key: 'acoustics', name: 'Acoustics' },
  { key: 'console', name: 'Console' },
  { key: 'runs', name: 'Runs' },
] as const;
type DockKey = (typeof DOCK_TABS)[number]['key'];

function ConsolePane() {
  const lines = useStore(consoleStore);
  return (
    <div className="console" role="log" aria-live="polite">
      {lines.map((l, i) => (
        <div key={i} className={`console-line ${l.tag}`}>
          <span className="time">{l.time}</span>
          <span className="tag">{l.tag}</span>
          <span className="text">{l.text}</span>
        </div>
      ))}
    </div>
  );
}

export function Dock() {
  const [tab, setTab] = useState<DockKey>('console');
  const fails = useStore(consoleStore).filter((l) => l.tag === 'FAIL').length;
  return (
    <section className="dock" aria-label="Analysis dock">
      <div className="dock-tabs" role="tablist" aria-label="Dock">
        {DOCK_TABS.map((d) => (
          <button
            key={d.key}
            className="dock-tab"
            role="tab"
            data-dock-tab={d.key}
            aria-selected={d.key === tab}
            onClick={() => setTab(d.key)}
          >
            {d.name}
            {d.key === 'console' && fails > 0 && <span className="tab-badge">{fails} fail</span>}
          </button>
        ))}
      </div>
      <div className="dock-body" role="tabpanel">
        {tab === 'acoustics' && (
          <div className="dock-empty empty">
            Room acoustics appear here once the physics checks behind them pass.
          </div>
        )}
        {tab === 'console' && <ConsolePane />}
        {tab === 'runs' && (
          <div className="runs">
            <div className="runs-head label">
              <span>Run</span>
              <span>Variant</span>
              <span>Solver</span>
              <span>Status</span>
              <span>Check</span>
            </div>
            <div className="runs-empty empty">No runs yet.</div>
          </div>
        )}
      </div>
    </section>
  );
}

export function PropertiesPanel({ step }: { step: StepKey }) {
  const s = STEPS.find((x) => x.key === step) ?? STEPS[0];
  return (
    <aside className="props" aria-label="Properties">
      <div className="props-head">
        <div className="title">{s.name}</div>
        <div className="sub">{s.sub}</div>
      </div>
      <div className="props-body empty">{s.hint}</div>
    </aside>
  );
}
