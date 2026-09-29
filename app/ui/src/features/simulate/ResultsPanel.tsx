// The Results step's properties panel (design:428-470), for the selected run (a Runs row click,
// the Simulate step's "Run <n>" link, else the newest run). M11 shows only whether the run's
// results verify (`run_results`), never a value: only M12 may show one (M11 PLAN.md 1.4, 3.3).
//
// `[data-results-state]` is `none`, `running`, `checking`, `verified`, `refused` or `error`. A
// refusal reads "FAIL · Results refused" with its UI code and core code, then the codes of the
// run's own reasons. There is no `[data-result]` element, and the panel's text has no digit
// outside `[data-run-label]` ("Run <n> · <variant>"): a reason's detail may hold numbers, so it
// goes in a title, never in the text (gate (e), m11-e-results).
import { useEffect, useState } from 'react';
import * as actions from '../../actions';
import type { ReasonUi } from '../../bindings/ipc';
import { resultsStore, runsStore, sceneStore, selectedRunStore, useStore } from '../../store';
import { registerHook } from '../../testhooks';
import { resultsStateName, runVariantName, solverLabel } from './model';
import './simulate.css';

function Codes({ reasons, part }: { reasons: readonly ReasonUi[]; part: string }) {
  return (
    <div className="res-codes" data-part={part}>
      {reasons.map((r, i) => (
        <div key={`${r.code}-${i}`} className="res-code" data-code={r.ui_code} title={r.detail}>
          <span className="code">{r.ui_code}</span>
          <span className="core">{r.code}</span>
        </div>
      ))}
    </div>
  );
}

export function ResultsPanel() {
  const selected = useStore(selectedRunStore);
  const runs = useStore(runsStore);
  const results = useStore(resultsStore);
  const scene = useStore(sceneStore);
  const [error, setError] = useState<{ run: string; code: string; message: string } | null>(null);

  const row = selected ? (runs?.rows.find((r) => r.run === selected) ?? null) : null;
  const answer = selected ? (results.get(selected) ?? null) : null;
  const errored = error !== null && error.run === selected;
  const state = resultsStateName(selected, row, answer, errored);

  // Ask once per run, when it has ended (a running run has no results to check yet).
  const rowStatus = row?.status ?? null;
  useEffect(() => {
    if (!selected || rowStatus === 'RUNNING' || resultsStore.get().has(selected)) return;
    let live = true;
    actions.resultsFor(selected).catch((e) => {
      if (!live) return;
      const err = actions.asCmdError(e);
      setError({ run: selected, code: err.code, message: err.message });
    });
    return () => {
      live = false;
    };
  }, [selected, rowStatus]);

  useEffect(
    () =>
      registerHook('resultsPanel', () => {
        const el = document.querySelector('[data-props-step="results"] [data-results-state]');
        return el ? { state: el.getAttribute('data-results-state'), run: el.getAttribute('data-run') } : null;
      }),
    [],
  );

  const variants = scene?.view.variants ?? [];
  const label = row ? `Run ${row.number} · ${runVariantName(row.variant, variants)}` : selected;
  const solver = row?.solver === 'tcr' || row?.solver === 'spps' ? solverLabel(row.solver) : 'Run';

  return (
    <div data-part="results-panel">
      <div className="props-head">
        <div className="title" data-run-label>
          {label ?? 'Results'}
        </div>
        <div className="sub">{selected ? `${solver} · results checked before any value is shown` : 'Checked values only'}</div>
      </div>
      <div className="props-section res-state" data-results-state={state} data-run={selected ?? ''}>
        {state === 'none' && (
          <div className="empty">No run selected. Run the simulation, or pick a run in the Runs tab.</div>
        )}
        {state === 'running' && (
          <div className="empty">This run is still running. Its results are checked when it ends.</div>
        )}
        {state === 'checking' && <div className="empty">Checking this run’s results…</div>}
        {state === 'verified' && (
          <>
            <div className="res-verdict ok">
              <span className="res-label">OK</span>
              Results verified
            </div>
            <div className="res-text">
              Results verified: run.json, inputs and outputs re-checked. Values appear here once the physics checks
              behind them pass.
            </div>
          </>
        )}
        {state === 'refused' && (
          <>
            <div className="res-verdict fail">
              <span className="res-label">FAIL</span>
              Results refused
            </div>
            {answer?.refusal && <Codes reasons={[answer.refusal]} part="refusal" />}
            {row && row.reasons.length > 0 && (
              <>
                <div className="label res-section-label">The run’s reasons</div>
                <Codes reasons={row.reasons} part="run-reasons" />
              </>
            )}
            <div className="res-text">A run that did not verify shows no value.</div>
          </>
        )}
        {state === 'error' && error && (
          <>
            <div className="res-verdict fail">
              <span className="res-label">FAIL</span>
              The results could not be checked
            </div>
            <div className="res-codes" data-part="error">
              <div className="res-code" data-code={error.code} title={error.message}>
                <span className="code">{error.code}</span>
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
