// The menu bar (design:28-44): the menus, the project as a tab, Commands and Run.
// Hand-over stub from the M10 foundation (M9's menu, routed through actions.ts); the scene
// package owns it from here (PLAN.md 6.3).
import { useEffect, useRef, useState } from 'react';
import * as actions from '../actions';
import { sceneStore, useStore } from '../store';
import { Search } from './icons';
import { RunButton } from './RunButton';
import './chrome.css';

const MENUS = ['File', 'Edit', 'View', 'Model', 'Simulate', 'Results', 'Help'] as const;

export function MenuBar() {
  const project = useStore(sceneStore)?.info ?? null;
  const [fileOpen, setFileOpen] = useState(false);
  const bar = useRef<HTMLElement>(null);
  useEffect(() => {
    if (!fileOpen) return;
    const close = (e: MouseEvent) => {
      if (!bar.current?.contains(e.target as Node)) setFileOpen(false);
    };
    window.addEventListener('mousedown', close);
    return () => window.removeEventListener('mousedown', close);
  }, [fileOpen]);
  const run = (action: () => Promise<unknown>) => () => {
    setFileOpen(false);
    actions.fire(action());
  };
  return (
    <header className="menubar" ref={bar}>
      {MENUS.map((m) =>
        m === 'File' ? (
          <button
            key={m}
            className="menu-item"
            data-menu={m}
            aria-haspopup="menu"
            aria-expanded={fileOpen}
            onClick={() => setFileOpen(!fileOpen)}
          >
            {m}
          </button>
        ) : (
          <button key={m} className="menu-item" data-menu={m} aria-disabled="true">
            {m}
          </button>
        ),
      )}
      {fileOpen && (
        <div className="dropdown" role="menu">
          <button role="menuitem" onClick={run(() => actions.newProject())}>
            New project
          </button>
          <button role="menuitem" onClick={run(actions.openDialog)}>
            Open…
          </button>
          <button role="menuitem" onClick={run(actions.save)} disabled={!project}>
            Save
          </button>
          <button role="menuitem" onClick={run(() => actions.saveAs())} disabled={!project}>
            Save as…
          </button>
        </div>
      )}
      <div className="menu-sep" />
      <div className="project-tab" data-part="project-tab">
        {project ? <span className="name">{project.name}</span> : <span className="none">No project</span>}
      </div>
      <div className="grow" />
      <button className="commands" aria-disabled="true">
        <Search />
        Commands<span className="kbd">Ctrl K</span>
      </button>
      <RunButton />
    </header>
  );
}
