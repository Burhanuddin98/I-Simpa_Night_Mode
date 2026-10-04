// The menu bar (design:28-44): File, Edit, View and Simulate work; Model, Results and Help stay
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
// - File › Export view as PNG…, Export parameters as CSV… and as JSON… (wow list W9,
//   features/export/), each disabled with the reason where there is nothing to export.
import { useEffect, useRef, useState } from 'react';
import * as actions from '../actions';
import { exportParams, exportView, paramsRefusal, viewRefusal } from '../features/export/exportActions';
import { frameModel, setView } from '../features/viewport/engine';
import { joinBlockers, projectBlockers } from '../flow';
import { reportStore, runStore, sceneStore, selectedRunStore, selectionStore, solversStatusStore, solverStore, stepStore, useStore, viewportStore } from '../store';
import { Search } from './icons';
import { RunButton } from './RunButton';
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
  // W9: what the export items say depends on the step, the run and its report, and the view.
  useStore(stepStore);
  useStore(selectedRunStore);
  useStore(reportStore);
  useStore(viewportStore);
  const paramsWhy = paramsRefusal();
  const viewWhy = viewRefusal();
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
  const running = active !== null;
  const blockers = joinBlockers(projectBlockers(scene, solver), solvers, running);
  const items: Partial<Record<MenuName, Item[]>> = {
    File: [
      {
        id: 'new-project',
        label: 'New project',
        run: () => actions.fire(actions.newProject()),
        disabled: running,
        title: running ? RUN_ACTIVE_TITLE : undefined,
      },
      {
        id: 'open',
        label: 'Open…',
        keys: 'Ctrl+O',
        run: () => actions.fire(actions.openDialog()),
        disabled: running,
        title: running ? RUN_ACTIVE_TITLE : OPEN_TITLE,
      },
      { id: 'save', label: 'Save', keys: 'Ctrl+S', run: () => actions.fire(actions.save()), disabled: !info },
      { id: 'save-as', label: 'Save as…', keys: 'Ctrl+Shift+S', run: () => actions.fire(actions.saveAs()), disabled: !info },
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
      {
        id: 'new-group-from-selection',
        label: REGROUP_LABEL,
        run: () => actions.fire(actions.regroupSelection()),
        disabled: !regroupFaces(selection),
        title: regroupFaces(selection)
          ? 'Send the faces picked in the 3D view to a new surface group'
          : 'Pick faces in the 3D view first: click a face, or double-click for its flat surface',
      },
    ],
    View: [
      { id: 'perspective', label: 'Perspective', run: () => setView('perspective') },
      { id: 'plan', label: 'Plan', run: () => setView('plan') },
      { id: 'frame', label: 'Frame model', keys: 'Home', run: () => frameModel(), disabled: !hasModel },
    ],
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
            {open === m && (
              <div className="dropdown" role="menu" aria-label={m} data-menu-list={m}>
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
