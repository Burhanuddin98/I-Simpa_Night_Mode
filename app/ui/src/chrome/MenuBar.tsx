import { useEffect, useRef, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { asCmdError, backend, type ProjectInfo } from '../backend';
import { log, projectStore } from '../store';
import { Play, Search } from './icons';

const MENUS = ['File', 'Edit', 'View', 'Model', 'Simulate', 'Results', 'Help'] as const;

async function openProject(): Promise<void> {
  const path = await open({
    multiple: false,
    directory: false,
    filters: [{ name: 'Night Mode project', extensions: ['simpa'] }],
  });
  if (typeof path !== 'string') return;
  try {
    const info = await backend.projectOpen(path);
    projectStore.set(info);
    log('OK', `Opened project "${info.name}"`);
  } catch (e) {
    const err = asCmdError(e);
    log('FAIL', `Could not open ${path}: ${err.message} (${err.code})`);
  }
}

async function newProject(): Promise<void> {
  try {
    projectStore.set(await backend.projectNew('Untitled'));
    log('INFO', 'New project');
  } catch (e) {
    log('FAIL', `Could not create a project: ${asCmdError(e).message}`);
  }
}

export function MenuBar({ project }: { project: ProjectInfo | null }) {
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
  const run = (action: () => Promise<void>) => () => {
    setFileOpen(false);
    void action();
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
          <button role="menuitem" onClick={run(newProject)}>
            New project
          </button>
          <button role="menuitem" onClick={run(openProject)}>
            Open project…
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
      <button className="run" disabled title="Runs arrive with the Simulate step">
        <Play />
        Run<span className="key">F5</span>
      </button>
    </header>
  );
}
