// Parity G50: a source's, a receiver's or a fitting zone's Display, upstream's render properties: the
// colour the 3D view draws it in (the system colour picker, sent once when it closes; "Default" gives
// the view's own colour back) and Show name, its name beside it in the view. Each change sends the
// element back whole through the checked apply (one undo step) with its display normalised
// (markerDisplay.ts), so a property set back to its default writes the element as it was. Display only:
// no solver reads it.
import { useEffect, useRef } from 'react';
import * as actions from '../actions';
import type { MarkerDisplay } from '../bindings/schema';
import { DEFAULT_MARKER_COLOR, hasOwnColor, markerColor, nameShown, withDisplay, type MarkerKind } from './markerDisplay';

type Displayed = { id: string; name: string; display?: MarkerDisplay | null };

/** A colour input that reports a pick once, when the picker closes (the native `change` event). */
function ColourPick({ value, label, field, onPick }: { value: string; label: string; field: string; onPick: (hex: string) => void }) {
  const ref = useRef<HTMLInputElement>(null);
  const pick = useRef(onPick);
  pick.current = onPick;
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const changed = () => pick.current(el.value);
    el.addEventListener('change', changed);
    return () => el.removeEventListener('change', changed);
  }, [value]);
  return <input ref={ref} key={value} type="color" className="marker-colour" data-field={field} aria-label={label} defaultValue={value} />;
}

export function MarkerDisplayFields<T extends Displayed>({ kind, element, send }: { kind: MarkerKind; element: T; send: (next: T) => Promise<unknown> }) {
  const own = hasOwnColor(element);
  const colour = markerColor(element, kind);
  const shown = nameShown(element, kind);
  const what = kind === 'zone' ? 'outline' : 'marker';
  const go = (patch: { color?: string | null; show_name?: boolean }) => {
    const next = withDisplay(element, kind, patch);
    if (JSON.stringify(next.display ?? null) === JSON.stringify(element.display ?? null)) return;
    actions.fire(send(next));
  };
  return (
    <div className="props-section" data-part="marker-display" data-display-kind={kind}>
      <div className="label section-label">Display</div>
      <div className="field-row" title={`The colour the 3D view draws its ${what} in; display only, no solver reads it`}>
        <span className="label">Colour</span>
        <span className="marker-colour-cell">
          <ColourPick value={colour} label={`${element.name}: ${what} colour`} field="display.color" onPick={(hex) => go({ color: hex.toLowerCase() === DEFAULT_MARKER_COLOR[kind] && !own ? null : hex })} />
          <span className="mono" data-part="display-color">
            {own ? colour : 'Default'}
          </span>
          {own && (
            <button className="small-button" data-action="display-color-default" title="Draw it in the view's own colour again" onClick={() => go({ color: null })}>
              Default
            </button>
          )}
        </span>
      </div>
      <label className="field-row check-row" title={`Upstream's Show name: the name beside it in the 3D view${kind === 'zone' ? ' (off by default for a fitting zone, as upstream has it)' : ''}`}>
        <span className="label">Show name</span>
        <input type="checkbox" data-field="display.show_name" checked={shown} onChange={(e) => go({ show_name: e.target.checked })} />
      </label>
    </div>
  );
}
