// The Simulate step's properties panel (design:386-426), top to bottom (M11 PLAN.md 3.3):
// - the head, "Simulation · Runs in the background · the app stays usable";
// - the solver choice, SPPS or TCR (`[data-solver]`, `aria-checked`), session state only;
// - the chosen solver's settings as fields (PQ3, SettingsEditor.tsx): each edit an op through the
//   checked apply, its refusals inline; Run is blocked for the chosen solver's own errors too
//   (`flow.projectBlockers`: every band off, `NO_BAND_COMPUTED`);
// - "Before running": one row per check with an OK or FAIL text label (never colour alone) and
//   the UI codes that fail it, from the same blockers the Run button's `data-blockers` joins;
// - "Run quality" (backlog 80): the run-quality advisor's items (`SceneState.advice`), each its
//   cause, the setting that addresses it and an Apply (one checked edit, one undo step). It never
//   blocks Run, and holds no number next to s, ms, dB or % (m10-h): a duration's or a time
//   step's new value shows in its own field once applied;
// - while a run is active, the running block: what it is doing ("Meshing…", "Solving · <p> %"),
//   the bar, the elapsed m:ss, Cancel (`[data-part="cancel-run"]`, PQ2);
// - when idle, the last run: "Run <n> · <variant>" (a link to the Results step), its status as
//   text, "Particles lost <x> % / <l> % limit", "Solver warnings <n>", and the big Run button.
//
// Every number next to a unit outside `[data-input]` is a run diagnostic in a leaf
// `data-diagnostic` span (PLAN.md 3.4 rule 1): the progress, the loss and its limit, each the
// solver's or the backend's own string. No solver-computed acoustic number is shown (only M12
// may). The settings come from the project file (`actions.projectJson`), since the scene view
// carries no solver settings; see `useProjectSettings`.
import { Fragment, useEffect, useRef, useState, type KeyboardEvent } from 'react';
import * as actions from '../../actions';
import type { SceneState } from '../../bindings/ipc';
import { runTooltip } from '../../chrome/sceneModel';
import { detailTitle, joinBlockers, projectBlockers } from '../../flow';
import { Issues } from '../../chrome/SourcesPanel';
import {
  type ActiveRun,
  type LinePart,
  refusalStore,
  runsStore,
  runStore,
  sceneStore,
  type SolverName,
  solversStatusStore,
  solverStore,
  stepStore,
  useStore,
} from '../../store';
import { registerHook } from '../../testhooks';
import {
  type AdviceRow,
  adviceRows,
  elapsedText,
  lastRunView,
  latestRun,
  type LastRunView,
  partsText,
  preflightRows,
  type PreflightRow,
  progressFill,
  projectSettings,
  type ProjectSettings,
  runLabel,
  runningHead,
  settingsRows,
  solverLabel,
} from './model';
import { SettingsEditor } from './SettingsEditor';
import { reasonWords } from './reasonWords';
import './simulate.css';

/**
 * Parts as text; a diagnostic part as its leaf span, `data-diagnostic=<field> data-run=<run>`,
 * whose text is exactly the field's value and unit (M11 PLAN.md 3.4 rule 1, 4.2).
 */
export function Parts({ parts }: { parts: readonly LinePart[] }) {
  return (
    <>
      {parts.map((p, i) =>
        'diagnostic' in p ? (
          <span key={i} className="diag" data-diagnostic={p.diagnostic} data-run={p.run}>
            {p.text}
          </span>
        ) : (
          <Fragment key={i}>{p.text}</Fragment>
        ),
      )}
    </>
  );
}

const SOLVERS: readonly { key: SolverName; what: string }[] = [
  { key: 'spps', what: 'Particle tracing' },
  { key: 'tcr', what: 'Classical theory' },
];

/**
 * The open project's solver settings, read from its file form. Refetched when the scene state
 * changes, and only while this panel is shown. The previous answer stays on screen during a
 * refetch of the same project; another project reads em dashes until its own answer lands.
 */
function useProjectSettings(scene: SceneState | null): ProjectSettings | null {
  const [got, setGot] = useState<{ project: string; settings: ProjectSettings | null } | null>(null);
  const project = scene ? `${scene.info.id}\u0000${scene.info.path ?? ''}` : null;
  useEffect(() => {
    if (!scene || project === null) return;
    let live = true;
    actions
      .projectJson()
      .then((json) => live && setGot({ project, settings: projectSettings(json) }))
      .catch(() => live && setGot({ project, settings: null }));
    return () => {
      live = false;
    };
  }, [scene, project]);
  return got && got.project === project ? got.settings : null;
}

/** The wall clock, ticking once a second while `on`. */
function useNow(on: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!on) return;
    setNow(Date.now());
    const t = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(t);
  }, [on]);
  return now;
}

function SolverChoice({ solver }: { solver: SolverName }) {
  const refs = useRef<Record<SolverName, HTMLButtonElement | null>>({ spps: null, tcr: null });
  const onKey = (e: KeyboardEvent) => {
    if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(e.key)) return;
    e.preventDefault();
    const next: SolverName = solver === 'spps' ? 'tcr' : 'spps';
    solverStore.set(next);
    refs.current[next]?.focus();
  };
  return (
    <div className="sim-solvers" role="radiogroup" aria-label="Solver" onKeyDown={onKey}>
      {SOLVERS.map((s) => (
        <button
          key={s.key}
          ref={(el) => {
            refs.current[s.key] = el;
          }}
          className="sim-solver"
          role="radio"
          aria-checked={solver === s.key}
          tabIndex={solver === s.key ? 0 : -1}
          data-solver={s.key}
          onClick={() => solverStore.set(s.key)}
        >
          <span className="name">{solverLabel(s.key)}</span>
          <span className="what">{s.what}</span>
        </button>
      ))}
    </div>
  );
}

function PreflightList({ rows }: { rows: PreflightRow[] | null }) {
  if (rows === null) return <div className="empty">Open a project to check it before a run.</div>;
  return (
    <div className="sim-checks">
      {rows.map((r) => (
        <div key={r.key} className="sim-check" data-preflight={r.key} data-state={r.state}>
          <span className={`sim-state ${r.state.toLowerCase()}`} title={r.state === 'UNCHECKED' ? 'Not checked yet' : undefined}>
            {r.state === 'UNCHECKED' ? '—' : r.state === 'OK' ? 'Ready' : 'Blocked'}
          </span>
          <span className="sim-check-body">
            <span className="k">{r.label}</span>
            {(r.codes.length > 0 || r.detail) && (
              <span className="sim-check-why">
                {r.codes.map((c) => (
                  <span key={c} className="code" data-code={c}>
                    {c}
                  </span>
                ))}
                {r.detail && <span className="detail">{r.detail}</span>}
              </span>
            )}
          </span>
        </div>
      ))}
    </div>
  );
}

/** The run-quality advisor before a run (backlog 80): advice, never a blocker. */
function AdviceList({ rows }: { rows: AdviceRow[] | null }) {
  const refusals = useStore(refusalStore);
  if (rows === null) return null;
  if (rows.length === 0)
    return (
      <div className="empty" data-part="advice-none">
        Nothing in the settings is known to make values noisy or refused.
      </div>
    );
  return (
    <div className="sim-advice" data-part="advice">
      {rows.map((r, i) => (
        <div key={`${r.key}-${i}`} className="sim-advice-row" data-advice={r.key}>
          <div className="sim-advice-cause">{r.cause}</div>
          <div className="sim-advice-fix">
            <span className="sim-advice-words">{r.words}</span>
            {r.change ? (
              <span className="sim-advice-change mono" data-part="advice-change">
                {' '}
                {r.change}
              </span>
            ) : null}
          </div>
          {r.note ? <div className="sim-note">{r.note}</div> : null}
          {r.apply ? (
            <button
              className="small-button"
              data-part="advice-apply"
              data-setting={r.apply.setting}
              onClick={() => r.apply && actions.fire(actions.adviceApply(r.apply))}
            >
              Apply
            </button>
          ) : r.why ? (
            <div className="sim-note" data-part="advice-why">
              {r.why}
            </div>
          ) : null}
          {r.apply ? <Issues refused={refusals.get(actions.adviceKey(r.apply)) ?? []} current={[]} /> : null}
        </div>
      ))}
    </div>
  );
}

function RunningBlock({ active }: { active: ActiveRun }) {
  const now = useNow(true);
  const cancelling = active.status === 'cancelling';
  return (
    <div className="sim-running" data-part="running" data-run={active.run ?? ''} data-stage={active.stage ?? ''}>
      <div className="sim-running-head" data-part="running-head">
        <Parts parts={runningHead(active)} />
      </div>
      <div className="sim-bar" aria-hidden="true">
        <div className="sim-bar-fill" style={{ width: `${progressFill(active)}%` }} />
      </div>
      <div className="sim-running-meta">
        <span>
          Elapsed{' '}
          <span className="mono" data-part="elapsed-clock">
            {elapsedText(now - active.startedAt)}
          </span>
        </span>
        <button
          className="sim-cancel"
          data-part="cancel-run"
          disabled={cancelling}
          title="Stop the solver and record the run as Cancelled"
          onClick={() => actions.fire(actions.runCancel())}
        >
          {cancelling ? 'Cancelling…' : 'Cancel run'}
        </button>
      </div>
      <div className="sim-note">Solver output is read line by line in the Console tab.</div>
    </div>
  );
}

const STATUS_CLASS: Record<LastRunView['status'], string> = {
  OK: 'ok',
  FAIL: 'fail',
  CRASH: 'fail',
  CANCELLED: 'muted',
  INTERRUPTED: 'muted',
  RUNNING: 'muted',
};

function LastRun({ last }: { last: LastRunView }) {
  return (
    <>
      <div className="sim-last-head">
        <button
          className="sim-run-link"
          data-part="run-link"
          data-run={last.run}
          title="Show this run on the Results step"
          onClick={() => {
            actions.selectRun(last.run);
            stepStore.set('results');
          }}
        >
          {last.label} · {last.variant}
        </button>
        <span className={`sim-status ${STATUS_CLASS[last.status]}`} data-part="last-status" data-status={last.status}>
          {last.statusText}
        </span>
      </div>
      {last.reasons.length > 0 && (
        <div className="sim-reasons" data-part="last-reasons">
          {last.reasons.map((r, i) => (
            <div key={`${r.code}-${i}`} className="sim-reason" data-code={r.ui_code} title={detailTitle(r.detail)}>
              <span className="words">{reasonWords(r.code)}</span>
              <span className="core">{r.ui_code}</span>
            </div>
          ))}
        </div>
      )}
      {last.loss && (
        <div className="sim-kv" data-part="last-loss">
          <span className="k">Particles lost</span>
          <span className="v mono">
            <span className="diag" data-diagnostic="loss_pct" data-run={last.run}>
              {`${last.loss.pct} %`}
            </span>{' '}
            <span className="dim">
              /{' '}
              <span data-diagnostic="loss_limit_pct" data-run={last.run}>
                {`${last.loss.limit} %`}
              </span>{' '}
              limit
            </span>
          </span>
        </div>
      )}
      <div className="sim-kv" data-part="last-warnings">
        <span className="k">Solver warnings</span>
        <span className="v mono">{last.solverWarnings}</span>
      </div>
      {last.warnings.length > 0 && (
        <div className="sim-reasons" data-part="last-run-warnings">
          {last.warnings.map((w, i) => (
            <div key={`${w.code}-${i}`} className="sim-reason warn" data-code={w.ui_code} title={detailTitle(w.detail)}>
              <span className="label-warn">Warning</span>
              <span className="words">{reasonWords(w.code)}</span>
              <span className="core">{w.ui_code}</span>
            </div>
          ))}
        </div>
      )}
    </>
  );
}

export function SimulatePanel() {
  const scene = useStore(sceneStore);
  const solvers = useStore(solversStatusStore);
  const active = useStore(runStore);
  const runs = useStore(runsStore);
  const solver = useStore(solverStore);
  const settings = useProjectSettings(scene);

  const project = projectBlockers(scene, solver);
  const blockers = joinBlockers(project, solvers, active !== null);
  const preflight = preflightRows({
    blockers: project,
    solvers,
    check: scene?.check ?? null,
    info: scene?.info ?? null,
  });
  const settingRows = settingsRows(settings, solver);
  const advice = solver === 'spps' ? adviceRows(scene?.advice) : null;
  const lastRow = latestRun(runs);
  const last = lastRow ? lastRunView(lastRow, scene?.view.variants ?? []) : null;
  const tip = runTooltip(blockers);

  // What this panel shows, for the e2e (PLAN.md 3.5: packages register their own hooks).
  const view = useRef({ solver, settings: settingRows, preflight, advice, last, running: active ? partsText(runningHead(active)) : null });
  view.current = { solver, settings: settingRows, preflight, advice, last, running: active ? partsText(runningHead(active)) : null };
  useEffect(() => registerHook('simulateView', () => structuredClone(view.current)), []);

  return (
    <div data-part="simulate-panel">
      <div className="props-head">
        <div className="title">Simulation</div>
        <div className="sub">Runs in the background · the app stays usable</div>
      </div>

      <div className="props-section">
        <SolverChoice solver={solver} />
        <SettingsEditor scene={scene} settings={settings} solver={solver} />
      </div>

      <div className="props-section">
        <div className="label sim-section-label">Before running</div>
        <PreflightList rows={preflight} />
      </div>

      {advice !== null ? (
        <div className="props-section" data-part="run-quality">
          <div className="label sim-section-label">Run quality</div>
          <AdviceList rows={advice} />
        </div>
      ) : null}

      <div className="props-section">
        {active ? (
          <RunningBlock active={active} />
        ) : (
          <div className="sim-idle" data-part="last-run" data-run={last?.run ?? ''}>
            {last ? (
              <LastRun last={last} />
            ) : (
              <div className="empty" data-part="no-run">
                {scene ? 'No run yet for this project.' : 'No project open.'}
              </div>
            )}
            <button
              className="sim-run-big"
              data-part="run-panel"
              data-blockers={(blockers ?? []).join(' ')}
              disabled={blockers === null || blockers.length > 0}
              title={tip}
              onClick={() => actions.fire(actions.runStart(solver))}
            >
              <Parts parts={runLabel(null, solver)} />
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
