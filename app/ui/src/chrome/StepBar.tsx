// The step bar (design:46-65): the five steps, each with its sub (`data-part="sub"`), and the
// variant switch. `[data-step]` and `[data-part="name"]` are what the M9 self-test reads; the sub
// sits outside the name so the names stay exactly the five.
import { Fragment } from 'react';
import { simulateSubParts } from '../features/simulate/model';
import { STEPS } from '../steps';
import { type LinePart, runsStore, runStore, sceneStore, stepStore, useStore } from '../store';
import { stepSubs } from './sceneModel';
import { VariantSwitch } from './VariantSwitch';

/** A sub's parts; a diagnostic part is a leaf `data-diagnostic` span (M11 PLAN.md 3.4 rule 1). */
function SubParts({ parts }: { parts: readonly LinePart[] }) {
  return (
    <>
      {parts.map((p, i) =>
        'diagnostic' in p ? (
          <span key={i} data-diagnostic={p.diagnostic} data-run={p.run}>
            {p.text}
          </span>
        ) : (
          <Fragment key={i}>{p.text}</Fragment>
        ),
      )}
    </>
  );
}

export function StepBar() {
  const current = useStore(stepStore);
  const subs: Record<string, string> = { ...stepSubs(useStore(sceneStore)) };
  // The Simulate sub is the simulate package's (M11 PLAN.md 3.3): "<p> %" while running, its
  // percentage in a progress_pct diagnostic span, else "run <n> <status>".
  const simulate = simulateSubParts(useStore(runStore), useStore(runsStore));
  return (
    <div className="stepbar">
      <nav className="steps" aria-label="Workflow">
        {STEPS.map((s, i) => (
          <button
            key={s.key}
            className="step"
            data-step={s.key}
            aria-current={s.key === current ? 'step' : undefined}
            onClick={() => stepStore.set(s.key)}
          >
            <span className="badge">{i + 1}</span>
            <span className="name" data-part="name">
              {s.name}
            </span>
            <span className={`sub${subs[s.key] === 'refused' ? ' fail' : ''}`} data-part="sub">
              {s.key === 'simulate' && simulate.length > 0 ? <SubParts parts={simulate} /> : subs[s.key]}
            </span>
          </button>
        ))}
      </nav>
      <div className="grow" />
      <VariantSwitch />
    </div>
  );
}
