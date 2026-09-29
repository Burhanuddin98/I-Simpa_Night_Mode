// The menu bar (design:28-44): File, Edit and View work; Model, Simulate, Results and Help stay
// disabled until their milestones. Then the project as a tab with the unsaved-changes dot, the
// command palette (disabled in M10) and Run (RunButton). Every item calls the same action the
// keys and the e2e hooks call (actions.ts); View drives the 3D view through the viewport
// package's own `setView` and `frameModel`.
import { useEffect, useRef, useState } from 'react';
import * as actions from '../actions';
import { frameModel, setView } from '../features/viewport/engine';
import { sceneStore, useStore } from '../store';
import { Search } from './icons';
import { RunButton } from './RunButton';
import './chrome.css';

type Item = { id: string; label: string; keys?: string; run: () => unknown; disabled?: boolean };
type MenuName = 'File' | 'Edit' | 'View' | 'Model' | 'Simulate' | 'Results' | 'Help';

const MENUS: readonly MenuName[] = ['File', 'Edit', 'View', 'Model', 'Simulate', 'Results', 'Help'];

export function MenuBar() {
  const scene = useStore(sceneStore);
  const info = scene?.info ?? null;
  const [open, setOpen] = useState<MenuName | null>(null);
  const bar = useRef<HTMLElement>(null);

  useEffect(() => {
    if (!open) return;
    const outside = (e: MouseEvent) => {
      if (!bar.current?.contains(e.target as Node)) setOpen(null);
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(null);
    };
    window.addEventListener('mousedown', outside);
    window.addEventListener('keydown', esc);
    return () => {
      window.removeEventListener('mousedown', outside);
      window.removeEventListener('keydown', esc);
    };
  }, [open]);

  const hasModel = !!scene?.check;
  const items: Partial<Record<MenuName, Item[]>> = {
    File: [
      { id: 'new-project', label: 'New project', run: () => actions.fire(actions.newProject()) },
      { id: 'open', label: 'Open…', keys: 'Ctrl+O', run: () => actions.fire(actions.openDialog()) },
      { id: 'save', label: 'Save', keys: 'Ctrl+S', run: () => actions.fire(actions.save()), disabled: !info },
      { id: 'save-as', label: 'Save as…', keys: 'Ctrl+Shift+S', run: () => actions.fire(actions.saveAs()), disabled: !info },
    ],
    Edit: [
      { id: 'undo', label: 'Undo', keys: 'Ctrl+Z', run: () => actions.fire(actions.undo()), disabled: !info?.can_undo },
      { id: 'redo', label: 'Redo', keys: 'Ctrl+Y', run: () => actions.fire(actions.redo()), disabled: !info?.can_redo },
    ],
    View: [
      { id: 'perspective', label: 'Perspective', run: () => setView('perspective') },
      { id: 'plan', label: 'Plan', run: () => setView('plan') },
      { id: 'frame', label: 'Frame model', run: () => frameModel(), disabled: !hasModel },
    ],
  };

  const choose = (item: Item) => () => {
    setOpen(null);
    item.run();
  };

  return (
    <header className="menubar" ref={bar}>
      {MENUS.map((m) => {
        const list = items[m];
        if (!list) {
          return (
            <button key={m} className="menu-item" data-menu={m} aria-disabled="true" title={`${m}: not built yet`}>
              {m}
            </button>
          );
        }
        return (
          <div key={m} className="menu">
            <button
              className="menu-item"
              data-menu={m}
              aria-haspopup="menu"
              aria-expanded={open === m}
              onClick={() => setOpen(open === m ? null : m)}
              onMouseEnter={() => open && open !== m && setOpen(m)}
            >
              {m}
            </button>
            {open === m && (
              <div className="dropdown" role="menu" aria-label={m} data-menu-list={m}>
                {list.map((item) => (
                  <button
                    key={item.id}
                    role="menuitem"
                    data-menu-item={item.id}
                    onClick={choose(item)}
                    disabled={item.disabled}
                  >
                    <span className="grow">{item.label}</span>
                    {item.keys && <span className="menu-keys">{item.keys}</span>}
                  </button>
                ))}
              </div>
            )}
          </div>
        );
      })}
      <div className="menu-sep" />
      <div className="project-tab" data-part="project-tab" title={info?.path ?? undefined}>
        {info ? <span className="name">{info.name}</span> : <span className="none">No project</span>}
        {info?.dirty && <span className="dirty-dot" role="img" aria-label="Unsaved changes" title="Unsaved changes" />}
      </div>
      <div className="grow" />
      <button className="commands" disabled aria-disabled="true" title="The command palette is not built yet">
        <Search />
        Commands<span className="kbd">Ctrl K</span>
      </button>
      <RunButton />
    </header>
  );
}
