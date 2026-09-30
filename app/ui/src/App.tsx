// The whole window: composition, boot and the global key map (M10 PLAN.md 2.3, M11 PLAN.md 9.0).
// The regions are slots the packages fill without touching this file: the chrome (project
// package), the 3D view (`features/viewport`), the materials panel (`features/materials`), the
// dock (`features/dock`), the Simulate and Results panels (`features/simulate`, mounted by the
// properties panel) and the save prompt (`chrome/SavePrompt`). Every region reads the stores;
// none takes props from here.
import { useEffect } from 'react';
import * as actions from './actions';
import { asCmdError, backend } from './backend';
import { ImportDialog } from './chrome/ImportDialog';
import { MenuBar } from './chrome/MenuBar';
import { PropertiesPanel } from './chrome/PropertiesPanel';
import { SavePrompt } from './chrome/SavePrompt';
import { ScenePanel } from './chrome/ScenePanel';
import { StatusBar } from './chrome/StatusBar';
import { StepBar } from './chrome/StepBar';
import { Dock } from './features/dock/Dock';
import { frameModel } from './features/viewport/engine';
import { Viewport } from './features/viewport/Viewport';
import { isReloadKey, joinBlockers } from './flow';
import { probeWebGL } from './gpu';
import { runSelftest } from './selftest';
import { log, runStore, sceneStore, solversStatusStore, solverStore, statusStore } from './store';
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
    if (info.selftest) {
      await runSelftest(gl);
      return;
    }
    // M11: the close request comes through this channel; the solvers and the library are read
    // once (the solvers again before each run), and a --project's runs are listed.
    await actions.listenAppEvents();
    actions.fire(actions.refreshSolvers());
    actions.fire(actions.loadLibrary());
    actions.fire(actions.refreshRuns());
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

/** Run (F5) when nothing blocks it, as the Run button would; Frame model (Home). */
function onPlainKey(e: KeyboardEvent): boolean {
  if (e.ctrlKey || e.altKey || e.metaKey || e.shiftKey || typing(e.target)) return false;
  if (e.key === 'F5') {
    // Never the webview's reload.
    e.preventDefault();
    const blockers = joinBlockers(sceneStore.get()?.run_blockers ?? null, solversStatusStore.get(), runStore.get() !== null);
    if (blockers !== null && blockers.length === 0) actions.fire(actions.runStart(solverStore.get()));
    return true;
  }
  if (e.key === 'Home') {
    if (!sceneStore.get()?.check) return false;
    e.preventDefault();
    frameModel();
    return true;
  }
  return false;
}

/** WebView2's own context menu offers Refresh, a reload of the page (M11 review 2, app 2): it
 * stays only in text fields, whose menu is the edit menu (cut, copy, paste). */
function onContextMenu(e: MouseEvent): void {
  if (!typing(e.target)) e.preventDefault();
}

/** The named verbs' keys (PLAN.md 7.5, point 3): undo, redo, save, save as, open. The
 * webview's reload keys never reload the page, in a text field or not. */
function onKey(e: KeyboardEvent): void {
  if (isReloadKey(e)) e.preventDefault();
  if (onPlainKey(e)) return;
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
    window.addEventListener('contextmenu', onContextMenu);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('contextmenu', onContextMenu);
    };
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
      <SavePrompt />
    </div>
  );
}
