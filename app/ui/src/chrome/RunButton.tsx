// The Run button (design:43). Always disabled in M10: Run is wired in M11. `data-blockers` lists
// the reasons as UI codes (`SceneState.run_blockers`, which always holds M11_PENDING), so the
// gate can tell "disabled because the geometry is refused" from "disabled because unwired", and
// the tooltip spells each one out as text (a disabled control does not explain itself).
import { sceneStore, useStore } from '../store';
import { Play } from './icons';
import { runTooltip } from './sceneModel';

export function RunButton() {
  const scene = useStore(sceneStore);
  const blockers = scene?.run_blockers ?? null;
  const tip = runTooltip(blockers);
  return (
    <span className="run-wrap" title={tip}>
      <button className="run" data-part="run" data-blockers={(blockers ?? []).join(' ')} disabled title={tip}>
        <Play />
        Run<span className="key">F5</span>
      </button>
    </span>
  );
}
