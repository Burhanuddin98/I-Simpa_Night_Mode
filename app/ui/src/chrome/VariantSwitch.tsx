// The variant switch (design:57-64). Hand-over stub from the M10 foundation (M9's read-only
// switch); the scene package makes it editable (set_active_variant, "+", rename; PLAN.md 6.3).
import { sceneStore, useStore } from '../store';

export function VariantSwitch() {
  const view = useStore(sceneStore)?.view;
  const variants = [{ id: null as string | null, name: 'Base' }, ...(view?.variants ?? [])];
  const active = view?.active_variant ?? null;
  return (
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
  );
}
