// The floating panels' fold (decision-log row 50): the scene list, the properties and the dock
// fold to a strip over the 3D view and back. Remembered in this browser profile only, a
// convenience: a fresh profile opens the scene list and properties unfolded and the dock folded,
// so the Console's startup lines do not cover the room (Burhan 2026-10-06); the dock opens itself
// on the Results step and on an error (`openDockFor`).
import { Store, useStore } from '../store';

export type FoldPanel = 'scene' | 'props' | 'dock';
type Folds = Record<FoldPanel, boolean>;

const KEY = 'nm-fold';
const NONE: Folds = { scene: false, props: false, dock: true };

function load(): Folds {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? 'null') as Partial<Folds> | null;
    return { ...NONE, ...(v ?? {}) };
  } catch {
    return NONE;
  }
}

export const foldStore = new Store<Folds>(load());

export function setFold(panel: FoldPanel, folded: boolean): void {
  const next = { ...foldStore.get(), [panel]: folded };
  foldStore.set(next);
  try {
    localStorage.setItem(KEY, JSON.stringify(next));
  } catch {
    // Storage refused: the fold still holds for this session.
  }
}

export function useFold(panel: FoldPanel): boolean {
  return useStore(foldStore)[panel];
}

const NAMES: Record<FoldPanel, string> = { scene: 'scene list', props: 'properties', dock: 'dock' };
/** The chevron for each panel, pointing the way the panel goes when it folds. */
const ARROW: Record<FoldPanel, [string, string]> = { scene: ['‹', '›'], props: ['›', '‹'], dock: ['⌄', '⌃'] };

/** The button that folds `panel` away, or unfolds it. */
export function FoldButton({ panel }: { panel: FoldPanel }) {
  const folded = useFold(panel);
  const label = `${folded ? 'Show' : 'Hide'} the ${NAMES[panel]}`;
  return (
    <button type="button" className="fold" data-fold={panel} aria-label={label} title={label} aria-expanded={!folded} onClick={() => setFold(panel, !folded)}>
      {ARROW[panel][folded ? 1 : 0]}
    </button>
  );
}

/** Opens the folded dock when there is something in it to see: the Results step, or an error line. */
export function openDockFor(reason: { step?: string; tag?: string }): void {
  if (!foldStore.get().dock) return;
  if (reason.step === 'results' || reason.tag === 'ERROR' || reason.tag === 'FAIL') setFold('dock', false);
}
