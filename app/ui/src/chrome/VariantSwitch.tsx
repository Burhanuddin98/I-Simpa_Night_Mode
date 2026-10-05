// The variant switch (design:57-64): Baseline (the base project) and the project's variants. A
// click makes one active (`set_active_variant`); "+" adds one and makes it active in one undo
// step (`batch(add_variant, set_active_variant)`); a double-click renames a variant in place
// (PLAN.md 7.5, point 2); "−" removes the active variant, back to Baseline first, in one undo step
// (`batch(set_active_variant(null), remove_variant)`: the core refuses to remove the active one).
// Every change is an edit through the checked apply.
import { useEffect, useRef, useState } from 'react';
import * as actions from '../actions';
import type { UiIssue } from '../bindings/ipc';
import { fieldKey } from '../issues';
import { addVariant, batch, removeVariant, rename, setActiveVariant } from '../ops';
import { sceneStore, useStore } from '../store';
import { nextVariantName } from './sceneModel';

function RenameField({ id, name, onDone }: { id: string; name: string; onDone: () => void }) {
  const [text, setText] = useState(name);
  const [refused, setRefused] = useState<UiIssue[]>([]);
  const input = useRef<HTMLInputElement>(null);
  const busy = useRef(false);
  const done = useRef(false);
  const finish = () => {
    done.current = true;
    onDone();
  };
  useEffect(() => {
    input.current?.focus();
    input.current?.select();
  }, []);
  const commit = async () => {
    if (busy.current || done.current) return;
    if (text === name) return finish();
    busy.current = true;
    try {
      const out = await actions.apply(rename('variant', id, text), fieldKey('variant', id, 'name'));
      if (out.applied) finish();
      else setRefused(out.refusals);
    } catch {
      // The failure is in the Console; the field stays open.
    } finally {
      busy.current = false;
    }
  };
  return (
    <span className="variant-rename">
      <input
        ref={input}
        value={text}
        aria-label="Variant name"
        data-part="variant-name"
        aria-invalid={refused.length > 0}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            void commit();
          } else if (e.key === 'Escape') {
            e.preventDefault();
            finish();
          }
        }}
        onBlur={() => (refused.length ? finish() : void commit())}
      />
      {refused.map((r) => (
        <span key={r.code + r.path} className="issue" data-issue-code={r.code} title={r.message}>
          <span className="code">FAIL {r.code}</span>
        </span>
      ))}
    </span>
  );
}

export function VariantSwitch() {
  const scene = useStore(sceneStore);
  const view = scene?.view ?? null;
  const active = view?.active_variant ?? null;
  const [renaming, setRenaming] = useState<string | null>(null);
  const tabs = [{ id: null as string | null, name: 'Baseline' }, ...(view?.variants ?? [])];

  const pick = (id: string | null) => {
    if (!view || id === active) return;
    actions.fire(actions.apply(setActiveVariant(id)));
  };
  const add = () => {
    if (!view) return;
    const id = crypto.randomUUID();
    const op = batch([addVariant(view.variants.length, { id, name: nextVariantName(view), overrides: [] }), setActiveVariant(id)]);
    actions.fire(actions.apply(op, fieldKey('variant', 'new')));
  };
  const remove = () => {
    if (!view || active === null) return;
    actions.fire(actions.apply(batch([setActiveVariant(null), removeVariant(active)]), fieldKey('variant', active)));
  };
  const activeName = view?.variants.find((v) => v.id === active)?.name;

  return (
    <div className="variants">
      <span className="label">Variant</span>
      <div className="segmented" role="tablist" aria-label="Variants" data-part="variants">
        {tabs.map((v) =>
          v.id !== null && renaming === v.id ? (
            <RenameField key={v.id} id={v.id} name={v.name} onDone={() => setRenaming(null)} />
          ) : (
            <button
              key={v.id ?? 'baseline'}
              role="tab"
              data-variant={v.id ?? 'baseline'}
              aria-selected={v.id === active}
              aria-disabled={!view}
              title={v.id ? 'Click to make active; double-click to rename' : 'The base project'}
              onClick={() => pick(v.id)}
              onDoubleClick={() => v.id && setRenaming(v.id)}
            >
              {v.name}
            </button>
          ),
        )}
      </div>
      <button
        className="variant-add"
        data-part="variant-add"
        aria-label="Add a variant"
        title="Add a variant and make it active"
        disabled={!view}
        onClick={add}
      >
        +
      </button>
      <button
        className="variant-add variant-remove"
        data-part="variant-remove"
        aria-label={activeName ? `Remove the variant ${activeName}` : 'Remove a variant'}
        title={activeName ? `Remove "${activeName}" and go back to Baseline (Ctrl+Z brings it back)` : 'Pick a variant to remove it'}
        disabled={!view || active === null}
        onClick={remove}
      >
        −
      </button>
    </div>
  );
}
