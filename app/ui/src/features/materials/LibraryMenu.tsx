// "+ From library" beside + Material (M11 PLAN.md 1.2, row 22 item M1): upstream's 11 reference
// materials, "100% absorbing" to "0% absorbing", as the core gives them (`libraryStore`, read once
// at boot from `material_library`). A choice is one `add_material` through the checked apply
// (actions.addFromLibrary: the core's f32-widened absorption in every band, scattering 0,
// specular), so one undo step; a refusal lands under the grid like any other material edit.
// An entry the project already holds (by name) reads "in project" and is not added twice.
//
// Material names such as "30% absorbing" are inputs, not results: the list sits inside
// `[data-input]`, as the grid's rows do, so the no-acoustic-number checks read them as such.
//
// Registers the e2e hook `materialLibrary()`: the entries exactly as the core sent them.
import { useEffect, useRef, useState } from 'react';
import * as actions from '../../actions';
import type { LibraryMaterial } from '../../bindings/ipc';
import { libraryStore, sceneStore, useStore } from '../../store';
import { registerHook } from '../../testhooks';
import { closingProps, usePresence } from '../../chrome/usePresence';
import { errorOf } from './inline';
import { addedMaterial, libraryRows } from './library';

export function LibraryMenu(props: {
  /** A new attempt: the messages under the grid become this one's. */
  onAttempt: () => void;
  /** The material the add appended. */
  onAdded: (id: string) => void;
  /** The command failed (the Console has it too). */
  onFailed: (code: string, message: string) => void;
}) {
  const { onAttempt, onAdded, onFailed } = props;
  const library = useStore(libraryStore);
  const materials = useStore(sceneStore)?.view.materials ?? [];
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const listed = usePresence(open);

  useEffect(() => registerHook('materialLibrary', () => libraryStore.get().map((e) => ({ ...e }))), []);

  useEffect(() => {
    if (!open) return;
    list.current?.scrollIntoView({ block: 'nearest' });
    const outside = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.preventDefault();
      e.stopPropagation();
      setOpen(false);
    };
    window.addEventListener('mousedown', outside);
    window.addEventListener('keydown', esc, true);
    return () => {
      window.removeEventListener('mousedown', outside);
      window.removeEventListener('keydown', esc, true);
    };
  }, [open]);

  const add = (entry: LibraryMaterial) => {
    setOpen(false);
    onAttempt();
    const before = sceneStore.get()?.view.materials ?? [];
    setBusy(true);
    actions
      .addFromLibrary(entry)
      .then((out) => {
        if (!out.applied) return;
        const id = addedMaterial(before, out.state.view.materials);
        if (id) onAdded(id);
      })
      .catch((e: unknown) => {
        const err = errorOf(e);
        onFailed(err.code, err.message);
      })
      .finally(() => setBusy(false));
  };

  const rows = libraryRows(library, materials);
  const why = library.length === 0 ? 'The reference library could not be read: the Console has the reason' : undefined;
  return (
    <div className="mg-lib-menu" ref={root}>
      <button
        type="button"
        data-action="library"
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={library.length === 0 || busy}
        title={why ?? "Add from library: I-Simpa's reference materials, with the solver's own values"}
        onClick={() => setOpen((o) => !o)}
      >
        + From library
      </button>
      {listed.shown && (
        <div
          className="mg-lib-list"
          role="menu"
          aria-label="Reference materials"
          data-part="library-list"
          data-input=""
          ref={list}
          {...closingProps(listed.closing)}
        >
          {rows.map(({ entry, inProject }) => (
            <button
              key={entry.reference_id}
              type="button"
              role="menuitem"
              data-library-add={entry.reference_id}
              data-in-project={inProject ?? undefined}
              disabled={inProject !== null}
              title={
                inProject !== null
                  ? `${entry.name} is in this project already: assign it from the Material list`
                  : `Add ${entry.name} (I-Simpa reference material ${entry.reference_id}), the same α in every band`
              }
              onClick={() => add(entry)}
            >
              <span className="mat-swatch" style={{ background: entry.color }} />
              <span className="grow">{entry.name}</span>
              {inProject !== null && <span className="mg-lib-in">in project</span>}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
