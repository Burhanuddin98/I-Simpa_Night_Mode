// Parity M45: a material's display colour, edited (upstream's material render colour,
// `e_scene_bdd_materiaux_rendermateriau.h:56`). The swatch is the system colour picker's input: the
// choice is sent once, when the picker closes (the native `change` event), not at every step of a drag
// through it (React's onChange is the `input` event), so a pick is one checked edit and one undo step
// (`actions.setMaterialColor`). The colour is display only: the 3D view's tint and the swatches.
import { useEffect, useRef } from 'react';
import * as actions from '../../actions';
import type { Material } from '../../bindings/schema';

export function ColourInput({ material, where }: { material: Material; where: 'panel' | 'grid' }) {
  const ref = useRef<HTMLInputElement>(null);
  const id = material.id;
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const changed = () => actions.fire(actions.setMaterialColor(id, el.value));
    el.addEventListener('change', changed);
    return () => el.removeEventListener('change', changed);
  }, [id, material.color]);
  return (
    <input
      ref={ref}
      type="color"
      className={`mat-swatch mat-colour ${where}`}
      data-material-color={id}
      aria-label={`${material.name}: display colour`}
      title={`${material.name}'s colour in the 3D view and the lists: click to choose another. Display only; no solver reads it`}
      // Uncontrolled between picks: the picker's drag moves it freely, and a new project value resets it.
      key={material.color}
      defaultValue={material.color}
      onMouseDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
    />
  );
}
