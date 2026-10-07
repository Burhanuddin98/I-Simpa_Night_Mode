// The Run button (design:43). Its label is the design's `runLabel`: "Run SPPS" or "Run TCR" for
// the solver chosen on the Simulate step; while a run is active "Running <p> %" (SPPS's own
// percentage in a `progress_pct` diagnostic span, M11 PLAN.md 3.4 rule 1), "Running…" before
// SPPS has printed one, "Cancelling…" once Cancel was pressed, and disabled throughout.
// `data-blockers` lists why it is disabled, as UI codes: the project's own
// (`SceneState.run_blockers`), the solvers' and `RUN_ACTIVE`, joined by `flow.joinBlockers`, so
// the gate can tell one reason from another; the tooltip spells each one out as text (a
// disabled control does not explain itself). With none, a click (or F5, App.tsx) runs the chosen
// solver on the saved project (actions.runStart, which saves first, PQ1).
import { blockersWithSize, settingsStore } from '../features/simulate/runSize';
import { useEffect } from 'react';
import * as actions from '../actions';
import { Parts } from '../features/simulate/SimulatePanel';
import { runLabel } from '../features/simulate/model';
import '../features/simulate/simulate.css';
import { joinBlockers } from '../flow';
import { deviceStore, runStore, sceneStore, type SolverName, solversStatusStore, solverStore, useStore } from '../store';
import { registerHook } from '../testhooks';
import { Play } from './icons';
import { runTooltip } from './sceneModel';

export function RunButton() {
  const scene = useStore(sceneStore);
  const solvers = useStore(solversStatusStore);
  const active = useStore(runStore);
  const solver = useStore(solverStore);
  const device = useStore(deviceStore);
  // The solver the next run uses, for a spec that does not test the choice itself (the Simulate
  // step's radios are the real control; this is the same store they write).
  useEffect(
    () =>
      registerHook('setSolver', (s: SolverName) => {
        if (s !== 'spps' && s !== 'tcr') throw new Error(`setSolver: no solver '${String(s)}'`);
        solverStore.set(s);
        return true;
      }),
    [],
  );
  // SPPS on the GPU or the CPU for the next run (A5), as the Simulate step's GPU entry sets it.
  useEffect(
    () =>
      registerHook('setDevice', (d: 'cpu' | 'gpu') => {
        if (d !== 'cpu' && d !== 'gpu') throw new Error(`setDevice: no device '${String(d)}'`);
        deviceStore.set(d);
        return true;
      }),
    [],
  );
  const blockers = joinBlockers(blockersWithSize(scene, solver, useStore(settingsStore)), solvers, active !== null);
  const tip = runTooltip(blockers);
  const disabled = blockers === null || blockers.length > 0;
  return (
    <span className="run-wrap" title={tip}>
      <button
        className="run"
        data-part="run"
        data-blockers={(blockers ?? []).join(' ')}
        data-running={active ? 'true' : undefined}
        disabled={disabled}
        title={tip}
        onClick={() => actions.fire(actions.runStart(solver))}
      >
        <Play />
        <span className="run-label" data-part="run-label">
          <Parts parts={runLabel(active, solver, device)} />
        </span>
        <span className="key">F5</span>
      </button>
    </span>
  );
}
