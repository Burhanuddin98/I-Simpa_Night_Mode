// The Materials step's properties (design:312-350): the selected group, the material list, and
// the grid (TSV paste and copy, row fill, natural band sort, inline validator messages).
// Hand-over stub from the M10 foundation (M9's hint); the materials package owns
// `features/materials/**` from here (PLAN.md 6.2). It reads the stores and writes only through
// `actions.apply` with `ops`.
import { STEPS } from '../../steps';

export function MaterialsPanel() {
  return (
    <div className="props-body empty" data-part="materials">
      {STEPS[1].hint}
    </div>
  );
}
