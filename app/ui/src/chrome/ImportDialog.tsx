// The import dialog: a mesh file's length unit and up axis, every choice shown, with `m` and `z`
// preselected on each opening and confirmed by the user (nothing is guessed from the extents).
// Opens when `importRequestStore` holds a path (File › Open… or Import model… on a .ply, .obj or
// .stl). Registers the e2e hook `openImportDialog(path)`, which opens it on a path without the
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

function Choice<T extends string>(props: {
  field: string;
  label: string;
  options: readonly T[];
  value: T;
  onChange: (v: T) => void;
  titles?: Record<T, string>;
}) {
  const { field, label, options, value, onChange, titles } = props;
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
            {o}
          </button>
        ))}
      </div>
    </div>
  );
}

function Dialog({ path }: { path: string }) {
  const [unit, setUnit] = useState<Unit>('m');
  const [up, setUp] = useState<Up>('z');
  // A run that started while the dialog was open (F5) belongs to the project the import would
  // replace: Import waits for it (M11 PQ4), as New and Open do.
  const running = useStore(runStore) !== null;
  const confirm = useRef<HTMLButtonElement>(null);
  useEffect(() => confirm.current?.focus(), []);
  const close = () => importRequestStore.set(null);
  const go = () => {
    if (runStore.get() !== null) return;
    close();
    actions.fire(actions.importModel(path, unit, up));
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
          Import model
        </div>
        <div className="dialog-file mono" title={path}>
          {file}
        </div>
        <Choice field="unit" label="Length unit" options={UNITS} value={unit} onChange={setUnit} titles={UNIT_NAMES} />
        <Choice field="up" label="Up axis" options={UPS} value={up} onChange={setUp} />
        <div className="dialog-note empty">
          The file's numbers are read in this unit, and this axis becomes vertical. Nothing is guessed from the model.
        </div>
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
            Import
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
      registerHook('openImportDialog', (path: string) => {
        importRequestStore.set({ path });
        return true;
      }),
    [],
  );
  if (!request) return null;
  return <Dialog key={requestId(request)} path={request.path} />;
}
