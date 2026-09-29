// The step bar (design:46-65): the five steps, each with its sub (`data-part="sub"`), and the
// variant switch. `[data-step]` and `[data-part="name"]` are what the M9 self-test reads; the sub
// sits outside the name so the names stay exactly the five.
import { simulateSub } from '../features/simulate/model';
import { STEPS } from '../steps';
import { runsStore, runStore, sceneStore, stepStore, useStore } from '../store';
import { stepSubs } from './sceneModel';
import { VariantSwitch } from './VariantSwitch';

export function StepBar() {
  const current = useStore(stepStore);
  const subs: Record<string, string> = { ...stepSubs(useStore(sceneStore)) };
  // The Simulate sub is the simulate package's (M11 PLAN.md 3.3): "<p> %", "run <n> <status>".
  const simulate = simulateSub(useStore(runStore), useStore(runsStore));
  if (simulate) subs.simulate = simulate;
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
              {subs[s.key]}
            </span>
          </button>
        ))}
      </nav>
      <div className="grow" />
      <VariantSwitch />
    </div>
  );
}
