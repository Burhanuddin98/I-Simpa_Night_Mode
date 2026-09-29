// The Sources & receivers step's properties (design:352-384): the selected source or receiver
// (`data-field="name"`, `data-field="position.x|y|z"`), inline `[data-issue-code]` messages,
// emission read-only inside `[data-input]`, Place in view.
// Hand-over stub from the M10 foundation (M9's hint); the scene package builds it (PLAN.md 6.3).
import { STEPS } from '../steps';

export function SourcesPanel() {
  return <div className="props-body empty">{STEPS[2].hint}</div>;
}
