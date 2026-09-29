// The import dialog: a mesh file's unit and up axis, `m` and `z` preselected and shown, confirmed
// by the user (nothing is guessed from the extents). Opens when `importRequestStore` holds a
// path (File › Open… on a .ply, .obj or .stl).
// Hand-over stub from the M10 foundation (plain, working); the scene package owns it from here
// and gives it the design's look and themed scrollbars (PLAN.md 6.3).
import { useState } from 'react';
import * as actions from '../actions';
import type { Unit, Up } from '../backend';
import { importRequestStore, useStore } from '../store';

const UNITS: Unit[] = ['m', 'cm', 'mm', 'ft', 'in'];
const UPS: Up[] = ['z', 'y'];

export function ImportDialog() {
  const request = useStore(importRequestStore);
  const [unit, setUnit] = useState<Unit>('m');
  const [up, setUp] = useState<Up>('z');
  if (!request) return null;
  const close = () => importRequestStore.set(null);
  const go = () => {
    close();
    actions.fire(actions.importModel(request.path, unit, up));
  };
  const file = request.path.split(/[\\/]/).pop();
  return (
    <div className="dialog-backdrop" role="presentation">
      <div className="dialog" role="dialog" aria-modal="true" aria-label="Import model" data-part="import-dialog">
        <div className="dialog-title">Import {file}</div>
        <label className="dialog-row">
          <span>Length unit</span>
          <select value={unit} onChange={(e) => setUnit(e.target.value as Unit)} data-field="unit">
            {UNITS.map((u) => (
              <option key={u} value={u}>
                {u}
              </option>
            ))}
          </select>
        </label>
        <label className="dialog-row">
          <span>Up axis</span>
          <select value={up} onChange={(e) => setUp(e.target.value as Up)} data-field="up">
            {UPS.map((a) => (
              <option key={a} value={a}>
                {a}
              </option>
            ))}
          </select>
        </label>
        <div className="dialog-actions">
          <button onClick={close}>Cancel</button>
          <button className="primary" onClick={go}>
            Import
          </button>
        </div>
      </div>
    </div>
  );
}
