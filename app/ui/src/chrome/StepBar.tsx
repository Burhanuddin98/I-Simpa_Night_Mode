import type { ProjectInfo } from '../backend';
import { STEPS, type StepKey } from '../steps';

export function StepBar({
  current,
  onPick,
  project,
}: {
  current: StepKey;
  onPick: (step: StepKey) => void;
  project: ProjectInfo | null;
}) {
  // The variant switch is a placeholder until variants are editable (M10): it shows the base
  // and the project's variants, and follows the project's active variant.
  const variants = [{ id: null as string | null, name: 'Base' }, ...(project?.variants ?? [])];
  const active = project?.active_variant ?? null;
  return (
    <div className="stepbar">
      <nav className="steps" aria-label="Workflow">
        {STEPS.map((s, i) => (
          <button
            key={s.key}
            className="step"
            data-step={s.key}
            aria-current={s.key === current ? 'step' : undefined}
            onClick={() => onPick(s.key)}
          >
            <span className="badge">{i + 1}</span>
            <span className="name" data-part="name">
              {s.name}
            </span>
          </button>
        ))}
      </nav>
      <div className="grow" />
      <div className="variants">
        <span className="label">Variant</span>
        <div className="segmented" role="tablist" aria-label="Variants">
          {variants.map((v) => (
            <button key={v.id ?? 'base'} role="tab" aria-selected={v.id === active} aria-disabled="true">
              {v.name}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
