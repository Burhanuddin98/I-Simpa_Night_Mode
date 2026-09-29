// The status bar (design:474-481). Hand-over stub from the M10 foundation (M9's, moved out of
// App.tsx); the scene package owns it from here (PLAN.md 6.3).
import { sceneStore, statusStore, useStore } from '../store';

export function StatusBar() {
  const status = useStore(statusStore);
  const project = useStore(sceneStore)?.info ?? null;
  return (
    <footer className="statusbar">
      <span className={`status ${status.kind === 'ready' ? '' : status.kind}`}>
        <span className="dot" />
        {status.text}
      </span>
      <span>{project ? project.name : 'No project'}</span>
      <span className="grow" />
      <span>Solvers: I-Simpa 1.4.0 · SPPS, TCR</span>
      <span>Units: m</span>
    </footer>
  );
}
