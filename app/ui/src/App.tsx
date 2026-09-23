import { useEffect, useState } from 'react';
import { asCmdError, backend } from './backend';
import { MenuBar } from './chrome/MenuBar';
import { Dock, PropertiesPanel, ScenePanel, Viewport } from './chrome/Panels';
import { StepBar } from './chrome/StepBar';
import { probeWebGL } from './gpu';
import { runSelftest } from './selftest';
import type { StepKey } from './steps';
import { log, projectStore, statusStore, useStore } from './store';

let booted = false;

async function boot(): Promise<void> {
  if (booted) return;
  booted = true;
  try {
    const info = await backend.startup();
    if (info.project) projectStore.set(info.project);
    if (info.project_error) {
      log('FAIL', `Could not open the project: ${info.project_error.message} (${info.project_error.code})`);
    }
    log('INFO', `${info.webview.engine} ${info.webview.version ?? '(version unknown)'} · Tauri ${info.tauri_version}`);
    const gl = probeWebGL();
    if (gl.renderer) log('INFO', `WebGL2: ${gl.renderer}`);
    else log('FAIL', `WebGL2 unavailable: ${gl.error}`);
    if (info.selftest) await runSelftest(gl);
  } catch (e) {
    const err = asCmdError(e);
    log('FAIL', `Startup failed: ${err.message} (${err.code})`);
    statusStore.set({ kind: 'bad', text: 'Startup failed' });
  }
}

function StatusBar() {
  const status = useStore(statusStore);
  const project = useStore(projectStore);
  return (
    <footer className="statusbar">
      <span className={`status ${status.kind === 'ready' ? '' : status.kind}`}>
        <span className="dot" />
        {status.text}
      </span>
      <span>{project ? project.name : 'No project'}</span>
      <span className="grow" />
      <span>Solvers: I-Simpa 1.4.0 · SPPS, TCR</span>
      <span>Units: m</span>
    </footer>
  );
}

export function App() {
  const [step, setStep] = useState<StepKey>('geometry');
  const project = useStore(projectStore);
  useEffect(() => {
    void boot();
  }, []);
  return (
    <div className="shell">
      <MenuBar project={project} />
      <StepBar current={step} onPick={setStep} project={project} />
      <div className="work">
        <ScenePanel project={project} />
        <main className="center">
          <Viewport project={project} />
          <Dock />
        </main>
        <PropertiesPanel step={step} />
      </div>
      <StatusBar />
    </div>
  );
}
