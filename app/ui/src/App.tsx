// The whole window: composition, boot and the global key map (M10 PLAN.md 2.3, M11 PLAN.md 9.0).
// The regions are slots the packages fill without touching this file: the chrome (project
// package), the 3D view (`features/viewport`), the materials panel (`features/materials`), the
// dock (`features/dock`), the Simulate and Results panels (`features/simulate`, mounted by the
// properties panel) and the save prompt (`chrome/SavePrompt`). Every region reads the stores;
// none takes props from here. While no project is open the landing page (`chrome/Landing`) covers
// the step bar and the work area, which are inert behind it (still laid out: the self-test reads
// the step bar's boxes).
import { blockersWithSize, settingsStore } from './features/simulate/runSize';
import { projectSettings } from './features/simulate/model';
import { fittingZonesOf } from './chrome/sceneModel';
import { useEffect } from 'react';
import * as actions from './actions';
import { asCmdError, backend } from './backend';
import { BoxRoomDialog } from './chrome/BoxRoomDialog';
import { ImportDialog } from './chrome/ImportDialog';
import { Landing } from './chrome/Landing';
import { landingShown } from './chrome/landingModel';
import { MenuBar } from './chrome/MenuBar';
import { PropertiesPanel } from './chrome/PropertiesPanel';
import { SavePrompt } from './chrome/SavePrompt';
import { ScenePanel } from './chrome/ScenePanel';
import { clipboardStore, copySelected, pasteCopied } from './chrome/sceneUi';
import { StatusBar } from './chrome/StatusBar';
import { StepBar } from './chrome/StepBar';
import { Dock } from './features/dock/Dock';
import { focusSelection, frameModel } from './features/viewport/engine';
import { Viewport } from './features/viewport/Viewport';
import { isReloadKey, joinBlockers } from './flow';
import { probeWebGL } from './gpu';
import { runSelftest } from './selftest';
import { fittingZonesStore, log, runStore, sceneStore, solversStatusStore, solverStore, statusStore, useStore } from './store';
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
    actions.fire(actions.refreshGpu());
    actions.fire(actions.loadLibrary());
    actions.fire(actions.refreshRuns());
  } catch (e) {
    const err = asCmdError(e);
    log('FAIL', `Startup failed: ${err.message} (${err.code})`);
    statusStore.set({ kind: 'bad', text: 'Startup failed' });
  }
}

// The run-size check (runSize.ts) needs the project's settings, which live in the project file's text: read
// them again on every scene change, so Run's blockers and the Simulate step see the cube as set now.
sceneStore.subscribe(() => {
  if (!sceneStore.get()) {
    fittingZonesStore.set(null);
    return void settingsStore.set(null);
  }
  actions
    .projectJson()
    .then((json) => {
      settingsStore.set(projectSettings(json));
      fittingZonesStore.set(fittingZonesOf(json));
    })
    .catch(() => {
      settingsStore.set(null);
      fittingZonesStore.set(null);
    });
});

/** Keys that stay with a text field while it has focus. */
function typing(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName);
}

/** Run (F5) when nothing blocks it, as the Run button would; Frame model (Home); Focus on selection (F). */
function onPlainKey(e: KeyboardEvent): boolean {
  if (e.ctrlKey || e.altKey || e.metaKey || e.shiftKey || typing(e.target)) return false;
  if (e.key === 'F5') {
    // Never the webview's reload.
    e.preventDefault();
    const blockers = joinBlockers(blockersWithSize(sceneStore.get(), solverStore.get()), solversStatusStore.get(), runStore.get() !== null);
    if (blockers !== null && blockers.length === 0) actions.fire(actions.runStart(solverStore.get()));
    return true;
  }
  if (e.key === 'Home') {
    if (!sceneStore.get()?.check) return false;
    e.preventDefault();
    frameModel();
    return true;
  }
  // Item 6: Focus on selection, flown on the arc (View › Focus on selection).
  if (e.key === 'f' || e.key === 'F') {
    if (!sceneStore.get()?.check) return false;
    return focusSelection();
  }
  return false;
}

/** WebView2's own context menu offers Refresh, a reload of the page (M11 review 2, app 2): it
 * stays only in text fields, whose menu is the edit menu (cut, copy, paste). */
function onContextMenu(e: MouseEvent): void {
  if (!typing(e.target)) e.preventDefault();
}

/** The named verbs' keys (PLAN.md 7.5, point 3): undo, redo, save, save as, open; A39, upstream's
 * Ctrl+N (new project) and Ctrl+C / Ctrl+V on a source or receiver (copy, paste a copy). The
 * webview's reload keys never reload the page, in a text field or not; a text field keeps its own
 * Ctrl+C and Ctrl+V, and Ctrl+C with no source or receiver picked copies what the page has
 * selected, as it always did. */
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
  else if (key === 'n' && !e.shiftKey) action = () => actions.newProject();
  else if (key === 'c' && !e.shiftKey) {
    if (copySelected()) e.preventDefault();
    return;
  } else if (key === 'v' && !e.shiftKey && clipboardStore.get()) action = pasteCopied;
  if (!action) return;
  e.preventDefault();
  actions.fire(action());
}

export function App() {
  const landing = landingShown(useStore(sceneStore));
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
      <Landing />
      <div className="behind-landing" inert={landing}>
        <StepBar />
        <div className="work">
          <ScenePanel />
          <main className="center">
            <Viewport />
            <Dock />
          </main>
          <PropertiesPanel />
        </div>
      </div>
      <StatusBar />
      <ImportDialog />
      <BoxRoomDialog />
      <SavePrompt />
    </div>
  );
}
