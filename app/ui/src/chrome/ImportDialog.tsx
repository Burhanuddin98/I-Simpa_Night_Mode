// The import dialog: a mesh file's length unit and up axis, every choice shown, with `m` and `z`
// preselected on each opening and confirmed by the user (nothing is guessed from the extents).
// Opens when `importRequestStore` holds a path (File › Open… or Import model… on a .ply, .obj or
// .stl). G7: with `keepGroups` (the Geometry step's "Replace model, keep groups…") the file
// replaces the open project's model instead, keeping each matching face's surface group. Registers the e2e hook `openImportDialog(path)`, which opens it on a path without the
// native file dialog that WebDriver cannot drive.
import { useEffect, useRef, useState } from 'react';
import * as actions from '../actions';
import type { Unit, Up } from '../backend';
import { importRequestStore, runStore, useStore } from '../store';
import { registerHook } from '../testhooks';
import { RUN_ACTIVE_TITLE } from './MenuBar';

const UNITS: readonly Unit[] = ['m', 'cm', 'mm', 'ft', 'in'];
const UPS: readonly Up[] = ['z', 'y'];
const UNIT_NAMES: Record<Unit, string> = { m: 'metres', cm: 'centimetres', mm: 'millimetres', ft: 'feet', in: 'inches' };
type RepairChoice = 'leave' | 'repair';
const REPAIR_CHOICES: readonly RepairChoice[] = ['leave', 'repair'];
const REPAIR_LABELS: Record<RepairChoice, string> = { leave: 'Leave it', repair: 'Repair' };
const REPAIR_TITLES: Record<RepairChoice, string> = {
  leave: 'Load it as it is: the faces the check names are highlighted and Run waits',
  repair: 'Weld vertices, remove faces of zero area and repeated faces, turn inward faces out; written to a new file beside this one, never over it',
};

function Choice<T extends string>(props: {
  field: string;
  label: string;
  options: readonly T[];
  value: T;
  onChange: (v: T) => void;
  titles?: Record<T, string>;
  labels?: Record<T, string>;
}) {
  const { field, label, options, value, onChange, titles, labels } = props;
  return (
    <div className="dialog-row">
      <span id={`import-${field}`}>{label}</span>
      <div className="segmented" role="radiogroup" aria-labelledby={`import-${field}`} data-field={field} data-value={value}>
        {options.map((o) => (
          <button
            key={o}
            role="radio"
            aria-checked={o === value}
            aria-selected={o === value}
            data-option={o}
            title={titles?.[o]}
            onClick={() => onChange(o)}
          >
            {labels?.[o] ?? o}
          </button>
        ))}
      </div>
    </div>
  );
}

function Dialog({ path, keepGroups = false }: { path: string; keepGroups?: boolean }) {
  const [unit, setUnit] = useState<Unit>('m');
  const [up, setUp] = useState<Up>('z');
  // G8: upstream's "Repair model" at import, as a plain choice; leaving the model as it is stays the default.
  const [repair, setRepair] = useState<RepairChoice>('leave');
  // A run that started while the dialog was open (F5) belongs to the project the import would
  // replace: Import waits for it (M11 PQ4), as New and Open do.
  const running = useStore(runStore) !== null;
  const confirm = useRef<HTMLButtonElement>(null);
  useEffect(() => confirm.current?.focus(), []);
  const close = () => importRequestStore.set(null);
  const go = () => {
    if (runStore.get() !== null) return;
    close();
    actions.fire(keepGroups ? actions.reimportModel(path, unit, up) : actions.importModel(path, unit, up, repair === 'repair'));
  };
  const file = path.split(/[\\/]/).pop();
  return (
    <div className="dialog-backdrop" role="presentation">
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="import-title"
        data-part="import-dialog"
        onKeyDown={(e) => {
          if (e.key === 'Escape') {
            e.preventDefault();
            close();
          }
        }}
      >
        <div className="dialog-title" id="import-title">
          {keepGroups ? 'Replace model, keep groups' : 'Import model'}
        </div>
        <div className="dialog-file mono" title={path}>
          {file}
        </div>
        <Choice field="unit" label="Length unit" options={UNITS} value={unit} onChange={setUnit} titles={UNIT_NAMES} />
        <Choice field="up" label="Up axis" options={UPS} value={up} onChange={setUp} />
        <div className="dialog-note empty">
          The file's numbers are read in this unit, and this axis becomes vertical. Nothing is guessed from the model.
        </div>
        {keepGroups ? (
          <div className="dialog-note empty" data-part="reimport-note">
            This file replaces the model of the open project. Each face lying within 1 cm of a face of the old model keeps that
            face's surface group, and so its material; the other faces get new groups. Sources, receivers, materials and
            settings stay. One undo step.
          </div>
        ) : (
          <Choice field="repair" label="If the check refuses it" options={REPAIR_CHOICES} value={repair} onChange={setRepair} titles={REPAIR_TITLES} labels={REPAIR_LABELS} />
        )}
        <div className="dialog-actions">
          <button onClick={close} data-part="import-cancel">
            Cancel
          </button>
          <button
            ref={confirm}
            className="primary"
            onClick={go}
            data-part="import-confirm"
            disabled={running}
            title={running ? RUN_ACTIVE_TITLE : undefined}
          >
            {keepGroups ? 'Replace model' : 'Import'}
          </button>
        </div>
      </div>
    </div>
  );
}

/** One number per request object, so each request gets a fresh dialog (on m and z). */
const requestIds = new WeakMap<object, number>();
let lastId = 0;
function requestId(request: object): number {
  let n = requestIds.get(request);
  if (n === undefined) requestIds.set(request, (n = ++lastId));
  return n;
}

export function ImportDialog() {
  const request = useStore(importRequestStore);
  useEffect(
    () =>
      registerHook('openImportDialog', (path: string, keepGroups?: boolean) => {
        importRequestStore.set({ path, keepGroups: keepGroups === true });
        return true;
      }),
    [],
  );
  if (!request) return null;
  return <Dialog key={requestId(request)} path={request.path} keepGroups={request.keepGroups} />;
}
