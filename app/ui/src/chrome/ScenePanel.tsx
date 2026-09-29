// The scene list (design:69-102): filter, Surfaces, Sources, Receivers.
// Hand-over stub from the M10 foundation (M9's counts); the scene package owns it from here
// (rows with `data-entity`, FAIL labels, selection; PLAN.md 6.3).
import { sceneStore, useStore } from '../store';
import { Search } from './icons';

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

export function ScenePanel() {
  const project = useStore(sceneStore)?.info ?? null;
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
