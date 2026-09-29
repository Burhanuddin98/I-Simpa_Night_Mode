// The step bar (design:46-65): the five steps and the variant switch.
// Hand-over stub from the M10 foundation (M9's step bar, the step now in `stepStore`); the scene
// package owns it from here and adds the subs (`data-part="sub"`, PLAN.md 6.3).
import { STEPS } from '../steps';
import { stepStore, useStore } from '../store';
import { VariantSwitch } from './VariantSwitch';

export function StepBar() {
  const current = useStore(stepStore);
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
          </button>
        ))}
      </nav>
      <div className="grow" />
      <VariantSwitch />
    </div>
  );
}
