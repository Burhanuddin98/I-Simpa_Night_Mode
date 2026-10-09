// The menu bar (design:28-44): File, Edit, View, Simulate and Help work; Model and Results stay
// disabled until their milestones. Then the project as a tab with the unsaved-changes dot, the
// command palette (disabled until it is built) and Run (RunButton). Every item calls the same
// action the keys and the e2e hooks call (actions.ts); View drives the 3D view through the
// viewport package's own `setView` and `frameModel`.
//
// M11 (PLAN.md 1.2 and section 10):
// - File › Open… takes a Night Mode project, an upstream I-Simpa `.proj` (row 22, A3: opened with
//   no unit dialog, since a `.proj` carries its own units) or a room model. New and Open go
//   through the save prompt (A9, actions.confirmDiscard).
// - While a run is active, New and Open are disabled, saying why (PQ4).
// - Simulate › Run and Simulate › Cancel run (PQ2), the same actions as the Run button, F5 and
//   the Simulate step's Cancel.
// - Edit › New group from selection (scope row 15 (1), G19), the viewport's context menu entry.
// - Edit › Rename group (F2) and Merge groups (C1): one picked group, and two or more Ctrl+clicked
//   in the scene list. Edit › Add surface group and Delete group (Del, an empty group only) (G18).
// - File › Export view as PNG…, Export parameters as CSV… and as JSON… (wow list W9,
//   features/export/), each disabled with the reason where there is nothing to export.
import { blockersWithSize, settingsStore } from '../features/simulate/runSize';
import { useEffect, useRef, useState } from 'react';
import * as actions from '../actions';
import { exportParams, exportView, paramsRefusal, viewRefusal } from '../features/export/exportActions';
import { focusSelection, frameModel, hideStore, isolateWhyNot, setRoofOff, setView, toggleIsolate, viewFrom } from '../features/viewport/engine';
import { joinBlockers } from '../flow';
import { boxRoomRequestStore, groupRenameStore, reportStore, runStore, sceneStore, selectedRunStore, selectionStore, solversStatusStore, solverStore, stepStore, useStore, viewportStore } from '../store';
import { ADD_GROUP_LABEL, DELETE_GROUP_LABEL, MERGE_LABEL, mergePlan, RENAME_GROUP_LABEL, renameTarget } from './groupsModel';
import { clipboardStore, copySelected, deleteGroup, pasteCopied } from './sceneUi';
import { HELP_LINKS, HELP_MANUAL } from './helpModel';
import { Search } from './icons';
import { recentStore, reopenLastStore, setReopenLast } from './recent';
import { recentMenuLabel } from './recentModel';
import { RunButton } from './RunButton';
import { closingProps, usePresence } from './usePresence';
import { REGROUP_LABEL, regroupFaces, runTooltip } from './sceneModel';
import './chrome.css';

type Item = { id: string; label: string; keys?: string; run: () => unknown; disabled?: boolean; title?: string };
type MenuName = 'File' | 'Edit' | 'View' | 'Model' | 'Simulate' | 'Results' | 'Help';

const MENUS: readonly MenuName[] = ['File', 'Edit', 'View', 'Model', 'Simulate', 'Results', 'Help'];

/** Why New and Open wait (PQ4): a run belongs to the project the Runs tab lists. */
export const RUN_ACTIVE_TITLE = 'A run is active: cancel it first';

const OPEN_TITLE =
  'Open a Night Mode project (.simpa), an I-Simpa project (.proj, no unit to choose: it carries its own) or a room model (PLY, OBJ, STL)';

export function MenuBar() {
  const scene = useStore(sceneStore);
  const active = useStore(runStore);
  const solvers = useStore(solversStatusStore);
  const solver = useStore(solverStore);
  const selection = useStore(selectionStore);
  const hidden = useStore(hideStore);
  const copied = useStore(clipboardStore);
  const recent = useStore(recentStore);
  const reopen = useStore(reopenLastStore);
  // W9: what the export items say depends on the step, the run and its report, and the view.
  useStore(stepStore);
  useStore(selectedRunStore);
  useStore(reportStore);
  useStore(viewportStore);
  const paramsWhy = paramsRefusal();
  const viewWhy = viewRefusal();
  const info = scene?.info ?? null;
  const [open, setOpen] = useState<MenuName | null>(null);
  // Item 14: the closed menu plays its way out (motion.css); switching menus is instant.
  const listed = usePresence(open);
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
  const running = active !== null;
  const blockers = joinBlockers(blockersWithSize(scene, solver, useStore(settingsStore)), solvers, running);
  const items: Partial<Record<MenuName, Item[]>> = {
    File: [
      {
        id: 'new-project',
        label: 'New project',
        keys: 'Ctrl+N',
        run: () => actions.fire(actions.newProject()),
        disabled: running,
        title: running ? RUN_ACTIVE_TITLE : undefined,
      },
      {
        id: 'new-box-room',
        label: 'New box room…',
        run: () => boxRoomRequestStore.set(true),
        disabled: running,
        title: running ? RUN_ACTIVE_TITLE : 'A new project whose model is a closed box of the width, length and height you give',
      },
      {
        id: 'open',
        label: 'Open…',
        keys: 'Ctrl+O',
        run: () => actions.fire(actions.openDialog()),
        disabled: running,
        title: running ? RUN_ACTIVE_TITLE : OPEN_TITLE,
      },
      // A7: upstream's Recent projects, five, newest first, numbered as upstream's file history.
      ...recent.map((p, i) => ({
        id: `recent-${i}`,
        label: `${i + 1}  ${recentMenuLabel(p, recent)}`,
        run: () => actions.fire(actions.openPath(p)),
        disabled: running,
        title: running ? RUN_ACTIVE_TITLE : p,
      })),
      // A33: a view of the start, said as what choosing it does (as Roof off / Put the roof back).
      {
        id: 'reopen-last',
        label: reopen ? 'Start on the landing page' : 'Start on the last project',
        run: () => setReopenLast(!reopen),
        title: reopen
          ? 'Night Mode now opens the last project when it starts; choose this to start on the landing page again'
          : 'Open the last project when Night Mode starts, as upstream does',
      },
      { id: 'save', label: 'Save', keys: 'Ctrl+S', run: () => actions.fire(actions.save()), disabled: !info },
      { id: 'save-as', label: 'Save as…', keys: 'Ctrl+Shift+S', run: () => actions.fire(actions.saveAs()), disabled: !info },
      {
        id: 'save-copy',
        label: 'Save a copy…',
        run: () => actions.fire(actions.saveCopy()),
        disabled: !info,
        title: info ? 'Write the project as it is now to another file; this window stays on its own file' : 'Open a project first',
      },
      {
        id: 'export-view',
        label: 'Export view as PNG…',
        run: () => actions.fire(exportView()),
        disabled: viewWhy !== null,
        title: viewWhy ?? 'The 3D view as drawn, with the shown map’s legend below it',
      },
      {
        id: 'export-csv',
        label: 'Export parameters as CSV…',
        run: () => actions.fire(exportParams('csv')),
        disabled: paramsWhy !== null,
        title: paramsWhy ?? 'The parameters the Acoustics tab shows, each with its range or refusal and its report path',
      },
      {
        id: 'export-json',
        label: 'Export parameters as JSON…',
        run: () => actions.fire(exportParams('json')),
        disabled: paramsWhy !== null,
        title: paramsWhy ?? 'The parameters the Acoustics tab shows, with the words and marks shown with them',
      },
    ],
    Edit: [
      { id: 'undo', label: 'Undo', keys: 'Ctrl+Z', run: () => actions.fire(actions.undo()), disabled: !info?.can_undo },
      { id: 'redo', label: 'Redo', keys: 'Ctrl+Y', run: () => actions.fire(actions.redo()), disabled: !info?.can_redo },
      // A39: upstream's copy and paste of an element, here a source or a receiver.
      {
        id: 'copy',
        label: 'Copy source or receiver',
        keys: 'Ctrl+C',
        run: () => copySelected(),
        disabled: selection.kind !== 'source' && selection.kind !== 'receiver',
        title: selection.kind === 'source' || selection.kind === 'receiver' ? 'Copy the picked source or receiver' : 'Pick a source or receiver first',
      },
      {
        id: 'paste',
        label: copied ? `Paste a copy of ${copied.item.name}` : 'Paste',
        keys: 'Ctrl+V',
        run: () => actions.fire(pasteCopied()),
        disabled: !copied || !scene,
        title: copied ? 'Add a copy at the same position, named "… copy"' : 'Copy a source or receiver first (Ctrl+C)',
      },
      {
        id: 'new-group-from-selection',
        label: REGROUP_LABEL,
        run: () => actions.fire(actions.regroupSelection()),
        disabled: !regroupFaces(selection),
        title: regroupFaces(selection)
          ? 'Send the faces picked in the 3D view to a new surface group'
          : 'Pick faces in the 3D view first: click a face, or double-click for its flat surface',
      },
      {
        id: 'add-group',
        label: ADD_GROUP_LABEL,
        run: () => actions.fire(actions.addEmptyGroup()),
        disabled: !scene,
        title: scene ? 'An empty surface group; choose its material, then move faces into it' : 'Open a project first',
      },
      {
        id: 'delete-group',
        label: DELETE_GROUP_LABEL,
        keys: 'Del',
        run: () => {
          const id = renameTarget(selection);
          if (id) actions.fire(deleteGroup(id));
        },
        disabled: !renameTarget(selection),
        title: renameTarget(selection) ? 'Delete the picked surface group if it holds no faces' : 'Pick one surface group in the scene list first',
      },
      {
        id: 'rename-group',
        label: RENAME_GROUP_LABEL,
        keys: 'F2',
        run: () => {
          const id = renameTarget(selection);
          if (id) groupRenameStore.set(id);
        },
        disabled: !renameTarget(selection),
        title: renameTarget(selection) ? 'Edit the picked surface group\'s name in the scene list' : 'Pick one surface group in the scene list first',
      },
      {
        id: 'merge-groups',
        label: MERGE_LABEL,
        run: () => actions.fire(actions.mergeSelectedGroups()),
        disabled: !mergePlan(selection, scene?.view.surface_groups ?? []),
        title: mergePlan(selection, scene?.view.surface_groups ?? [])
          ? 'Merge the picked surface groups into the first picked, which keeps its name and material'
          : 'Ctrl+click two or more surface groups in the scene list first',
      },
    ],
    View: [
      { id: 'perspective', label: 'Perspective', run: () => setView('perspective') },
      { id: 'plan', label: 'Plan', run: () => setView('plan') },
      { id: 'frame', label: 'Frame model', keys: 'Home', run: () => frameModel(), disabled: !hasModel },
      // Item 6: each flies on an arc around the target (arc.ts), never through the model.
      { id: 'focus', label: 'Focus on selection', keys: 'F', run: () => focusSelection(), disabled: !hasModel, title: 'Frame what is picked from where you look now; with nothing picked, the model' },
      { id: 'view-front', label: 'View from the front', run: () => viewFrom('front'), disabled: !hasModel },
      { id: 'view-side', label: 'View from the side', run: () => viewFrom('side'), disabled: !hasModel },
      { id: 'view-top', label: 'View from above', run: () => viewFrom('top'), disabled: !hasModel },
      { id: 'view-corner', label: 'View from the corner', run: () => viewFrom('corner'), disabled: !hasModel },
      // Item 7: a view state, the model untouched.
      { id: 'roof-off', label: hidden.roof ? 'Put the roof back' : 'Roof off', keys: 'R', run: () => setRoofOff(!hidden.roof), disabled: !hasModel, title: 'Leave out the faces that close the room from above, so you can look in from above' },
      // Item 8: a view state, the model untouched.
      {
        id: 'isolate',
        label: hidden.isolate ? 'Show everything' : 'Isolate selection',
        keys: 'I',
        run: () => toggleIsolate(),
        disabled: !hasModel || (!hidden.isolate && isolateWhyNot() !== null),
        title: hidden.isolate ? 'Show the faces Isolate hid' : (isolateWhyNot() ?? 'Show only the picked faces or surface groups'),
      },
    ],
    // A20: the Help menu, its pages opened by the core in the default browser (help.rs); A22: the
    // user manual first, shipped inside app.exe.
    Help: [HELP_MANUAL, ...HELP_LINKS].map((l) => ({
      id: l.id,
      label: l.label,
      title: l.title,
      run: () => actions.fire(actions.openHelp(l.topic, l.what)),
    })),
    Simulate: [
      {
        id: 'run',
        label: `Run ${solver.toUpperCase()}`,
        keys: 'F5',
        run: () => actions.fire(actions.runStart(solver)),
        disabled: blockers === null || blockers.length > 0,
        title: runTooltip(blockers),
      },
      {
        id: 'cancel-run',
        label: 'Cancel run',
        run: () => actions.fire(actions.runCancel()),
        disabled: !running || active.status === 'cancelling',
        title: running ? (active.status === 'cancelling' ? 'Cancelling the run' : 'Stop the solver; the run is recorded as Cancelled') : 'No run is active',
      },
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
            {listed.shown === m && (
              <div className="dropdown" role="menu" aria-label={m} data-menu-list={m} {...closingProps(listed.closing)}>
                {list.map((item) => (
                  <button
                    key={item.id}
                    role="menuitem"
                    data-menu-item={item.id}
                    onClick={choose(item)}
                    disabled={item.disabled}
                    title={item.title}
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
