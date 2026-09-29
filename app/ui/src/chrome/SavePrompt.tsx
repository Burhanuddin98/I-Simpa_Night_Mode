// The save prompt before New, Open and Exit (row 22, A9; upstream's Yes, No and Cancel,
// `main.cpp:1015-1045, 1080-1114`): "Save changes to <name>?", with Save, Don't save and Cancel.
// Open while `promptStore` is set (actions.ts `confirmDiscard` sets it and waits); the choice is
// the dialog's only output. The M11 foundation's minimal working dialog, in the import dialog's
// styles; owned by the project package from here (docs/investigations/2026-09-29-m11/PLAN.md 9.3).
import { useEffect, useRef } from 'react';
import { promptStore, useStore, type PromptChoice } from '../store';

export function SavePrompt() {
  const prompt = useStore(promptStore);
  const save = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (prompt) save.current?.focus();
  }, [prompt]);
  if (!prompt) return null;
  const answer = (c: PromptChoice) => () => prompt.resolve(c);
  return (
    <div className="dialog-backdrop" role="presentation">
      <div
        className="dialog"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="prompt-title"
        data-prompt
        onKeyDown={(e) => {
          if (e.key === 'Escape') {
            e.preventDefault();
            prompt.resolve('cancel');
          }
        }}
      >
        <div className="dialog-title" id="prompt-title" data-part="prompt-title">
          Save changes to {prompt.name}?
        </div>
        <div className="dialog-note empty">Your changes are lost if you don't save them.</div>
        <div className="dialog-actions">
          <button onClick={answer('cancel')} data-choice="cancel">
            Cancel
          </button>
          <button onClick={answer('discard')} data-choice="discard">
            Don't save
          </button>
          <button ref={save} className="primary" onClick={answer('save')} data-choice="save">
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
