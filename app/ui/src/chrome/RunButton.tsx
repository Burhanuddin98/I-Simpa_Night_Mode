// The Run button (design:43). Always disabled in M10: Run is wired in M11. `data-blockers` lists
// the reasons as UI codes (`SceneState.run_blockers`, which always holds M11_PENDING), so the
// gate can tell "disabled because the geometry is refused" from "disabled because unwired".
// Hand-over stub from the M10 foundation; the scene package owns it from here (PLAN.md 6.3).
import { sceneStore, useStore } from '../store';
import { Play } from './icons';

export function RunButton() {
  const scene = useStore(sceneStore);
  const blockers = scene?.run_blockers ?? [];
  return (
    <button
      className="run"
      data-part="run"
      data-blockers={blockers.join(' ')}
      disabled
      title={scene ? `Run is blocked: ${blockers.join(', ')}` : 'Open a project first'}
    >
      <Play />
      Run<span className="key">F5</span>
    </button>
  );
}
