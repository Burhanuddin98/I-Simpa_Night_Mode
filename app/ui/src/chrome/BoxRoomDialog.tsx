// Parity G11, upstream's File > New scene ("Please enter scene dimensions (m)": Width (x), Length (y),
// Height (z), 5 m each by default, `i_simpa_main.cpp:903-906`): a new project whose model is a box room
// of that size, one corner at the origin (the core's `geometry::shoebox`). Opens from File › New box room…
// and the landing page. Each side is read as a strict decimal and must be above 0 before anything is
// sent; the core refuses the rest (`BOX_ROOM_INVALID`), shown here. Creating it goes through the save
// prompt and the run guard as New project does. Registers the e2e hook `openBoxRoomDialog()`.
import { useEffect, useRef, useState } from 'react';
import * as actions from '../actions';
import { parseStrictDecimal } from '../numbers';
import { boxRoomRequestStore, runStore, useStore } from '../store';
import { registerHook } from '../testhooks';
import { RUN_ACTIVE_TITLE } from './MenuBar';

const SIDES = [
  { key: 'width', label: 'Width (x)' },
  { key: 'length', label: 'Length (y)' },
  { key: 'height', label: 'Height (z)' },
] as const;
type Side = (typeof SIDES)[number]['key'];

/** Upstream's default for each side, metres. */
export const BOX_ROOM_DEFAULT_M = 5;

function Dialog() {
  const [text, setText] = useState<Record<Side, string>>({ width: String(BOX_ROOM_DEFAULT_M), length: String(BOX_ROOM_DEFAULT_M), height: String(BOX_ROOM_DEFAULT_M) });
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const running = useStore(runStore) !== null;
  const first = useRef<HTMLInputElement>(null);
  useEffect(() => {
    first.current?.focus();
    first.current?.select();
  }, []);
  const close = () => boxRoomRequestStore.set(false);
  const go = async () => {
    const sizes: number[] = [];
    for (const s of SIDES) {
      const p = parseStrictDecimal(text[s.key]);
      if (!p.ok || !(p.value > 0)) {
        setProblem(`${s.label}: "${text[s.key]}" is not a length. Write the metres above 0 with a decimal point, like 4.5.`);
        return;
      }
      sizes.push(p.value);
    }
    setProblem(null);
    setBusy(true);
    try {
      const st = await actions.newBoxRoom(sizes[0], sizes[1], sizes[2]);
      if (st) close();
    } catch (e) {
      const err = actions.asCmdError(e);
      setProblem(`${err.message} (${err.code})`);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="dialog-backdrop" role="presentation">
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="box-room-title"
        data-part="box-room-dialog"
        onKeyDown={(e) => {
          if (e.key === 'Escape') {
            e.preventDefault();
            close();
          } else if (e.key === 'Enter') {
            e.preventDefault();
            actions.fire(go());
          }
        }}
      >
        <div className="dialog-title" id="box-room-title">
          New box room
        </div>
        {SIDES.map((s, i) => (
          <label key={s.key} className="dialog-row">
            <span>{s.label}</span>
            <span className="axis-field box-room-side">
              <input
                ref={i === 0 ? first : undefined}
                className="mono"
                data-field={`box.${s.key}`}
                aria-label={`${s.label} in metres`}
                spellCheck={false}
                value={text[s.key]}
                onChange={(e) => setText({ ...text, [s.key]: e.target.value })}
              />
              <span className="unit">m</span>
            </span>
          </label>
        ))}
        <div className="dialog-note empty">
          A closed box with one corner at the origin: its floor, walls and ceiling become three surface groups. Choose their
          materials, then add a source and a receiver; Run waits for both. Nothing is saved until you save.
        </div>
        {problem && (
          <div className="issue" role="alert" data-part="box-room-problem">
            <span className="state">Error</span>
            <span className="msg">{problem}</span>
          </div>
        )}
        <div className="dialog-actions">
          <button onClick={close} data-part="box-room-cancel">
            Cancel
          </button>
          <button className="primary" onClick={() => actions.fire(go())} data-part="box-room-create" disabled={running || busy} title={running ? RUN_ACTIVE_TITLE : undefined}>
            {busy ? 'Creating…' : 'Create'}
          </button>
        </div>
      </div>
    </div>
  );
}

export function BoxRoomDialog() {
  const open = useStore(boxRoomRequestStore);
  useEffect(
    () =>
      registerHook('openBoxRoomDialog', () => {
        boxRoomRequestStore.set(true);
        return true;
      }),
    [],
  );
  return open ? <Dialog /> : null;
}
