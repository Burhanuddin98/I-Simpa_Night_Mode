// The Results step's properties panel (design:428-470), for the selected run (a Runs row click,
// the Simulate step's "Run <n>" link, else the newest run). M11 shows only whether the run's
// results verify (`run_results`), never a value: only M12 may show one (M11 PLAN.md 1.4, 3.3).
//
// `[data-results-state]` is `none`, `running`, `checking`, `verified`, `unverified`, `refused` or
// `error`. A refusal reads "FAIL · Results refused" with its UI code and core code, then the codes
// of the run's own reasons. A run whose results load but whose solver build was not verified
// (backlog 38) reads "UNVERIFIED · Results unverified" with its reason's codes, never "Results
// verified". The codes shown are `resultsCodes`'. There is no `[data-result]` element, and
// neither the panel's text nor its tooltips hold a digit outside `[data-run-label]` ("Run <n> ·
// <variant>"): a reason's detail may hold numbers, even a solver-computed one
// (`results_value_invalid` quotes the value it refused), so it is not shown here at all, not even
// in a title, which a user reads as surely as the text (M11 review 2, app 4). The Runs tab shows
// it, withheld when it quotes a number with a unit.
import { useEffect, useState } from 'react';
import * as actions from '../../actions';
import type { ReasonUi } from '../../bindings/ipc';
import { reportStore, resultsStore, runsStore, sceneStore, selectedRunStore, useStore } from '../../store';
import { registerHook } from '../../testhooks';
import { resultsCodes, resultsStateName, runVariantName, solverLabel } from './model';
import { reasonWords } from './reasonWords';
import './simulate.css';

/** A reason's title on this step: where its detail is, never the detail. */
const DETAIL_ON_RUNS_TAB = "The reason's detail is on the Runs tab";

function Codes({ reasons, part }: { reasons: readonly ReasonUi[]; part: string }) {
  return (
    <div className="res-codes" data-part={part}>
      {reasons.map((r, i) => (
        <div key={`${r.code}-${i}`} className="res-code" data-code={r.ui_code} title={DETAIL_ON_RUNS_TAB}>
          <span className="words">{reasonWords(r.code)}</span>
          <span className="core">{r.ui_code}</span>
        </div>
      ))}
    </div>
  );
}

export function ResultsPanel() {
  const selected = useStore(selectedRunStore);
  const runs = useStore(runsStore);
  const results = useStore(resultsStore);
  const reports = useStore(reportStore);
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
              <i className="led" data-on="ok" aria-hidden="true" />
              <span className="res-label">Verified</span>
              Results verified
            </div>
            <div className="res-text">
              Results verified: run.json, inputs and outputs re-checked. The values are in the Acoustics tab below.
            </div>
            {(selected ? (reports.get(selected)?.report?.advice.length ?? 0) : 0) > 0 && (
              <div className="res-text" data-part="advice-pointer">
                Some values are refused or wide: “Why values are missing” in the Acoustics tab names each cause and the
                setting to change.
              </div>
            )}
          </>
        )}
        {state === 'unverified' && (
          <>
            <div className="res-verdict warn">
              <i className="led" data-on="warn" aria-hidden="true" />
              <span className="res-label">Not verified</span>
              Results unverified
            </div>
            <Codes reasons={resultsCodes(answer)} part="unverified" />
            <div className="res-text">
              The results load and re-check, but the solvers that made them were not verified against the manifest, so
              they are marked unverified.
            </div>
          </>
        )}
        {state === 'refused' && (
          <>
            <div className="res-verdict fail">
              <i className="led" data-on="fail" aria-hidden="true" />
              <span className="res-label">Withheld</span>
              Results refused
            </div>
            <Codes reasons={resultsCodes(answer)} part="refusal" />
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
              <i className="led" data-on="fail" aria-hidden="true" />
              <span className="res-label">Not checked</span>
              The results could not be checked
            </div>
            <div className="res-codes" data-part="error">
              <div className="res-code" data-code={error.code} title="The message is in the Console">
                <span className="words">{reasonWords(error.code)}</span>
                <span className="core">{error.code}</span>
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
