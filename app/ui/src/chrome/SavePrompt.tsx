// The save prompt before New, Open and Exit (row 22, A9; upstream's Yes, No and Cancel,
// `main.cpp:1015-1045, 1080-1114`): "Save changes to <name>?", with Save, Don't save and Cancel.
// Open while `promptStore` is set (actions.ts `confirmDiscard` sets it and waits, and decides
// what each answer does); the choice is the dialog's only output. Built by the project package
// (docs/investigations/2026-09-29-m11/PLAN.md 9.3) on the foundation's working dialog.
//
// - It names the file the changes would go to, or says that Save asks where (a project never
//   saved opens Save as; cancelling that cancels the action).
// - Save has the page focus when it opens, as upstream's Yes has; the focus comes back to where
//   it was when it closes. Page focus only: nothing here asks the OS for the window's focus.
// - While it is open, no app key reaches the project underneath: Ctrl+Z, Ctrl+S, F5, Home and
//   Delete are held back, so the answer is about the project as the prompt found it. Esc is
//   Cancel; Tab and the arrow keys stay among its three buttons.
//
// DOM contract (PLAN.md 3.6): `[data-prompt]`, `[data-choice="save|discard|cancel"]`.
import { useEffect, useRef } from 'react';
import { promptStore, sceneStore, useStore, type PromptChoice } from '../store';

const CHOICES: readonly PromptChoice[] = ['cancel', 'discard', 'save'];

export function SavePrompt() {
  const prompt = useStore(promptStore);
  const path = useStore(sceneStore)?.info.path ?? null;
  const dialog = useRef<HTMLDivElement>(null);
  const save = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!prompt) return;
    const before = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    save.current?.focus();
    const buttons = () => CHOICES.map((c) => dialog.current?.querySelector<HTMLButtonElement>(`[data-choice="${c}"]`) ?? null);
    const move = (step: number) => {
      const list = buttons().filter((b): b is HTMLButtonElement => b !== null);
      if (!list.length) return;
      const at = list.findIndex((b) => b === document.activeElement);
      list[(at < 0 ? list.length - 1 : at + step + list.length) % list.length].focus();
    };
    // Capture phase on the window: this runs before any other key handler of the page.
    const onKey = (e: KeyboardEvent) => {
      const inside = e.target instanceof Node && !!dialog.current?.contains(e.target);
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        prompt.resolve('cancel');
        return;
      }
      if (e.key === 'Tab' || e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
        e.preventDefault();
        e.stopPropagation();
        move(e.key === 'ArrowLeft' || (e.key === 'Tab' && e.shiftKey) ? -1 : 1);
        return;
      }
      e.stopPropagation();
      // Enter and Space on a focused button press it (the browser's own action); every other
      // key, the webview's reload among them, does nothing while the prompt is open.
      if (inside && !e.ctrlKey && !e.altKey && !e.metaKey && (e.key === 'Enter' || e.key === ' ')) return;
      e.preventDefault();
    };
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('keydown', onKey, true);
      if (before?.isConnected) before.focus({ preventScroll: true });
    };
  }, [prompt]);

  if (!prompt) return null;
  const answer = (c: PromptChoice) => () => prompt.resolve(c);
  return (
    <div className="dialog-backdrop" role="presentation">
      <div
        ref={dialog}
        className="dialog prompt"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="prompt-title"
        aria-describedby="prompt-note"
        data-prompt=""
      >
        <div className="dialog-title" id="prompt-title" data-part="prompt-title">
          Save changes to {prompt.name}?
        </div>
        <div className="dialog-file mono" data-part="prompt-file" title={path ?? undefined}>
          {path ?? 'Not saved yet: Save asks where to keep it'}
        </div>
        <div className="dialog-note empty" id="prompt-note">
          Don't save discards the unsaved changes. Cancel goes back to the project as it is.
        </div>
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
