// The whole window: composition, boot and the global key map (PLAN.md 2.3). The regions are
// slots the packages fill without touching this file: the chrome (scene package), the 3D view
// (`features/viewport`) and the materials panel (`features/materials`, mounted by the
// properties panel). Every region reads the stores; none takes props from here.
import { useEffect } from 'react';
import * as actions from './actions';
import { asCmdError, backend } from './backend';
import { Dock } from './chrome/Dock';
import { ImportDialog } from './chrome/ImportDialog';
import { MenuBar } from './chrome/MenuBar';
import { PropertiesPanel } from './chrome/PropertiesPanel';
import { ScenePanel } from './chrome/ScenePanel';
import { StatusBar } from './chrome/StatusBar';
import { StepBar } from './chrome/StepBar';
import { Viewport } from './features/viewport/Viewport';
import { probeWebGL } from './gpu';
import { runSelftest } from './selftest';
import { log, statusStore } from './store';
import { installTestHooks } from './testhooks';

let booted = false;

async function boot(): Promise<void> {
  if (booted) return;
  booted = true;
  try {
    const info = await backend.startup();
    if (info.e2e) installTestHooks();
    if (info.project_error) {
      log('FAIL', `Could not open the project: ${info.project_error.message} (${info.project_error.code})`);
    }
    log('INFO', `${info.webview.engine} ${info.webview.version ?? '(version unknown)'} · Tauri ${info.tauri_version}`);
    const gl = probeWebGL();
    if (gl.renderer) log('INFO', `WebGL2: ${gl.renderer}`);
    else log('FAIL', `WebGL2 unavailable: ${gl.error}`);
    // A --project opened at launch, with its check lines.
    await actions.loadInitialState();
    if (info.selftest) await runSelftest(gl);
  } catch (e) {
    const err = asCmdError(e);
    log('FAIL', `Startup failed: ${err.message} (${err.code})`);
    statusStore.set({ kind: 'bad', text: 'Startup failed' });
  }
}

/** Keys that stay with a text field while it has focus. */
function typing(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName);
}

/** The named verbs' keys (PLAN.md 7.5, point 3): undo, redo, save, save as, open. */
function onKey(e: KeyboardEvent): void {
  if (!e.ctrlKey || e.altKey || e.metaKey || typing(e.target)) return;
  const key = e.key.toLowerCase();
  let action: (() => Promise<unknown>) | null = null;
  if (key === 'z' && !e.shiftKey) action = actions.undo;
  else if ((key === 'y' && !e.shiftKey) || (key === 'z' && e.shiftKey)) action = actions.redo;
  else if (key === 's' && !e.shiftKey) action = actions.save;
  else if (key === 's' && e.shiftKey) action = () => actions.saveAs();
  else if (key === 'o' && !e.shiftKey) action = actions.openDialog;
  if (!action) return;
  e.preventDefault();
  actions.fire(action());
}

export function App() {
  useEffect(() => {
    void boot();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);
  return (
    <div className="shell">
      <MenuBar />
      <StepBar />
      <div className="work">
        <ScenePanel />
        <main className="center">
          <Viewport />
          <Dock />
        </main>
        <PropertiesPanel />
      </div>
      <StatusBar />
      <ImportDialog />
    </div>
  );
}
