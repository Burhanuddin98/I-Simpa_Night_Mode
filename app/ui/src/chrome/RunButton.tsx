// The Run button (design:43). `data-blockers` lists why it is disabled, as UI codes: the
// project's own (`SceneState.run_blockers`), the solvers' and `RUN_ACTIVE`, joined by
// `flow.joinBlockers` (M11 PLAN.md 3.3), so the gate can tell one reason from another, and the
// tooltip spells each one out as text (a disabled control does not explain itself). With none,
// it runs the chosen solver on the saved project (actions.runStart, which saves first).
//
// The M11 foundation's minimal wiring; the simulate package owns this file and builds its
// label ("Run SPPS", "Running <p> %") from here (PLAN.md 9.1).
import * as actions from '../actions';
import { joinBlockers } from '../flow';
import '../features/simulate/simulate.css';
import { runStore, sceneStore, solversStatusStore, solverStore, useStore } from '../store';
import { Play } from './icons';
import { runTooltip } from './sceneModel';

export function RunButton() {
  const scene = useStore(sceneStore);
  const solvers = useStore(solversStatusStore);
  const active = useStore(runStore);
  const solver = useStore(solverStore);
  const blockers = joinBlockers(scene?.run_blockers ?? null, solvers, active !== null);
  const tip = runTooltip(blockers);
  const disabled = blockers === null || blockers.length > 0;
  return (
    <span className="run-wrap" title={tip}>
      <button
        className="run"
        data-part="run"
        data-blockers={(blockers ?? []).join(' ')}
        disabled={disabled}
        title={tip}
        onClick={() => actions.fire(actions.runStart(solver))}
      >
        <Play />
        Run<span className="key">F5</span>
      </button>
    </span>
  );
}
