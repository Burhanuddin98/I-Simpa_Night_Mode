// The Sources & receivers step's properties (design:352-384). The selected source or receiver:
// its name (`data-field="name"`) and position (`data-field="position.x|y|z"`), each committed on
// Enter or blur (numbers through `parseStrictDecimal`, then a move op through the checked apply);
// inline `[data-issue-code]` messages from the refusals and the validator; a source's emission,
// read-only, inside `[data-input]` (PLAN.md 7.5, point 6). Then the sources and receivers with
// + Source, + Receiver and Place in view; Del removes and F2 renames the selection.
//
// M11 (row 22, M26): each source in the list has its on/off switch (`[data-source-toggle]`, the
// scene list's `SourceSwitch`, here with its state as text), and a disabled source reads "off"
// in the list and in its editor's head. A refused switch is shown under the sources.
//
// W1 (parity M41): the sound-level planes below the receivers (PlanesSection.tsx).
import { useEffect, useRef, useState, type Ref } from 'react';
import * as actions from '../actions';
import type { PointReceiver, SceneState, Source, UiIssue } from '../bindings/ipc';
import { fieldKey, issuesByEntity, issuesForField } from '../issues';
import { NOT_A_NUMBER, parseStrictDecimal } from '../numbers';
import { moveReceiver, moveSource, rename } from '../ops';
import { refusalStore, sceneStore, selectionStore, toolStore, useStore } from '../store';
import { PlanesSection } from './PlanesSection';
import { IssueTag, SourceSwitch, toggleRefusals } from './ScenePanel';
import {
  AXES,
  coord,
  directivityName,
  exact,
  powerText,
  roomCentre,
  sentence,
  spectrumName,
  uniqueIssues,
  withAxis,
  worstSeverity,
  type Axis,
} from './sceneModel';
import { entityKey, onEntityKey, removeSelected, renameRequestStore, selectPoint } from './sceneUi';

type Kind = 'source' | 'point_receiver';
type Point = Source | PointReceiver;

/** The F2 requests already acted on (a panel mounted after the request still takes it). */
let renamesTaken = 0;

/** One inline issue: Error or Warning, the message, then the UI code, smaller (words first, the UI study's increment 2). */
export function IssueLine({ issue, refused }: { issue: UiIssue; refused: boolean }) {
  const fail = issue.severity === 'error';
  return (
    <div className={`issue${fail ? '' : ' warning'}`} data-issue-code={issue.code} role={fail ? 'alert' : undefined}>
      <span className="state">{fail ? 'Error' : 'Warning'}</span>
      <span className="msg">
        {sentence(issue.message)}
        {refused && ' Refused; the project is unchanged.'}
        {issue.rule && <span className="rule"> ({issue.rule})</span>}
      </span>
      <span className="code">{issue.code}</span>
    </div>
  );
}

export function Issues({ refused, current }: { refused: readonly UiIssue[]; current: readonly UiIssue[] }) {
  const all = uniqueIssues(refused, current);
  if (!all.length) return null;
  return (
    <div className="issues">
      {all.map((i) => (
        <IssueLine key={`${i.code}|${i.path}|${i.message}`} issue={i} refused={refused.includes(i)} />
      ))}
    </div>
  );
}

/**
 * A text field committed on Enter or blur. `commit` resolves true when the value was taken; a
 * refused text stays in the field (marked invalid) and is not sent again on blur. Esc restores
 * the project's value.
 */
export function CommitInput(props: {
  field: string;
  label: string;
  value: string;
  invalid: boolean;
  commit: (text: string) => Promise<boolean>;
  onRevert: () => void;
  inputRef?: Ref<HTMLInputElement>;
  className?: string;
  /** Called with the text being typed, or null when the field shows the project's value again. */
  onDraft?: (text: string | null) => void;
}) {
  const { field, label, value, invalid, commit, onRevert, inputRef, className, onDraft } = props;
  const [draft, setDraftState] = useState<string | null>(null);
  const setDraft = (text: string | null) => {
    setDraftState(text);
    onDraft?.(text);
  };
  const tried = useRef<string | null>(null);
  const busy = useRef(false);
  const go = async (why: 'enter' | 'blur') => {
    if (busy.current) return;
    if (draft === null || draft === value) {
      if (draft !== null) {
        setDraft(null);
        onRevert();
      }
      return;
    }
    if (why === 'blur' && draft === tried.current) return;
    busy.current = true;
    try {
      const taken = await commit(draft);
      if (taken) {
        setDraft(null);
        tried.current = null;
      } else tried.current = draft;
    } catch {
      // A command error is already a FAIL line in the Console; the text stays for another try.
      tried.current = draft;
    } finally {
      busy.current = false;
    }
  };
  return (
    <input
      ref={inputRef}
      className={className}
      data-field={field}
      aria-label={label}
      aria-invalid={invalid}
      spellCheck={false}
      value={draft ?? value}
      onChange={(e) => setDraft(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          void go('enter');
        } else if (e.key === 'Escape') {
          e.preventDefault();
          setDraft(null);
          tried.current = null;
          onRevert();
        }
      }}
      onBlur={() => void go('blur')}
    />
  );
}

function notANumber(text: string, kind: Kind, id: string, field: string): UiIssue {
  return {
    code: NOT_A_NUMBER,
    rule: '',
    severity: 'error',
    path: `${kind}:${id}:${field}`,
    entity: { kind, id },
    field,
    message: `"${text}" is not a number: write digits with a decimal point, like 4.5`,
  };
}

/** The point as it is now (a commit reads the latest state, not the render's). */
function current(kind: Kind, id: string): Point | undefined {
  const view = sceneStore.get()?.view;
  return kind === 'source' ? view?.sources.find((s) => s.id === id) : view?.point_receivers.find((r) => r.id === id);
}

function PointEditor({ scene, kind, point }: { scene: SceneState; kind: Kind; point: Point }) {
  const refusals = useStore(refusalStore);
  const renameRequest = useStore(renameRequestStore);
  const id = point.id;
  const nameRef = useRef<HTMLInputElement>(null);
  /** Refusals put away by Esc or by typing the old value back; shown again on the next refusal. */
  const [hidden, setHidden] = useState<ReadonlySet<string>>(new Set());
  const [local, setLocal] = useState<Readonly<Record<string, UiIssue>>>({});

  useEffect(() => {
    if (renameRequest > renamesTaken) {
      renamesTaken = renameRequest;
      nameRef.current?.focus();
      nameRef.current?.select();
    }
  }, [renameRequest]);

  const keyOf = (field: string) => fieldKey(kind, id, field);
  const hide = (key: string, on: boolean) =>
    setHidden((h) => {
      const n = new Set(h);
      if (on) n.add(key);
      else n.delete(key);
      return n;
    });
  const setLocalIssue = (field: string, issue: UiIssue | null) =>
    setLocal((l) => {
      const n = { ...l };
      if (issue) n[field] = issue;
      else delete n[field];
      return n;
    });
  const refusedFor = (key: string) => (hidden.has(key) ? [] : (refusals.get(key) ?? []));

  const commitName = async (text: string) => {
    const out = await actions.apply(rename(kind, id, text), keyOf('name'));
    if (!out.applied) hide(keyOf('name'), false);
    return out.applied;
  };
  const commitAxis = (axis: Axis) => async (text: string) => {
    const field = `position.${axis}`;
    const parsed = parseStrictDecimal(text);
    if (!parsed.ok) {
      setLocalIssue(field, notANumber(text, kind, id, field));
      return false;
    }
    setLocalIssue(field, null);
    const now = current(kind, id);
    if (!now) return false;
    // `5.0` typed over `5` is the same number: nothing to edit, and no empty undo step.
    if (Object.is(now.position[AXES.indexOf(axis)], parsed.value)) {
      hide(keyOf('position'), true);
      return true;
    }
    const position = withAxis(now.position, axis, parsed.value);
    const op = kind === 'source' ? moveSource(id, position) : moveReceiver(id, position);
    const out = await actions.apply(op, keyOf('position'));
    if (!out.applied) hide(keyOf('position'), false);
    return out.applied;
  };

  const nameRefused = refusedFor(keyOf('name'));
  const nameIssues = issuesForField(scene.issues, kind, id, 'name');
  const posRefused = refusedFor(keyOf('position'));
  const posIssues = issuesForField(scene.issues, kind, id, 'position');
  const localPos = AXES.map((a) => local[`position.${a}`]).filter((x): x is UiIssue => !!x);
  const entityIssues = (issuesByEntity(scene.issues).get(`${kind}:${id}`) ?? []).filter(
    (i) => i.field !== 'name' && i.field !== 'position' && !i.field.startsWith('position/') && !i.field.startsWith('name/'),
  );
  const removeRefused = refusedFor(entityKey(kind, id));
  const errors = worstSeverity(scene.issues.filter((i) => i.entity?.kind === kind && i.entity.id === id)) === 'error';
  const source = kind === 'source' ? (point as Source) : null;
  // A disabled source is not checked against the room (the validator reads enabled ones only).
  const where = source && !source.enabled ? ' · off' : scene.check?.verdict === 'ok' && !errors ? ' · inside the room' : '';

  return (
    <>
      <div className="props-head">
        <div className="head-row">
          <div className="title">{point.name}</div>
          <button className="small-button" data-part="remove" onClick={() => actions.fire(removeSelected())} title="Remove (Del)">
            Remove<span className="kbd-hint">Del</span>
          </button>
        </div>
        <div className="sub">
          {kind === 'source' ? 'Point source' : 'Point receiver'}
          {where}
        </div>
        <Issues refused={removeRefused} current={entityIssues} />
      </div>

      <div className="props-section">
        <label className="field-row">
          <span className="label">Name</span>
          <CommitInput
            field="name"
            label="Name"
            className="name-input"
            value={point.name}
            invalid={nameRefused.length > 0}
            inputRef={nameRef}
            commit={commitName}
            onRevert={() => hide(keyOf('name'), true)}
          />
        </label>
        <Issues refused={nameRefused} current={nameIssues} />
      </div>

      <div className="props-section">
        <div className="label section-label">Position</div>
        <div className="fact-grid" data-input>
          {AXES.map((axis, i) => (
            <label key={axis} className={`fact-cell axis-${axis}`}>
              <span className="k">{axis.toUpperCase()}</span>
              <span className="axis-field">
                <CommitInput
                  field={`position.${axis}`}
                  label={`Position ${axis.toUpperCase()} in metres`}
                  className="mono"
                  value={exact(point.position[i])}
                  invalid={posRefused.length > 0 || !!local[`position.${axis}`]}
                  commit={commitAxis(axis)}
                  onRevert={() => {
                    setLocalIssue(`position.${axis}`, null);
                    hide(keyOf('position'), true);
                  }}
                />
                <span className="unit">m</span>
              </span>
            </label>
          ))}
        </div>
        <Issues refused={[...localPos, ...posRefused]} current={posIssues} />
      </div>

      {source && (
        <div className="props-section" data-input>
          <div className="label section-label">Emission</div>
          <div className="kv">
            <span>Sound power</span>
            <span className="mono">{powerText(source.power.global_db)}</span>
          </div>
          <div className="kv">
            <span>Spectrum</span>
            <span>{spectrumName(source.power.shape)}</span>
          </div>
          <div className="kv">
            <span>Directivity</span>
            <span>{directivityName(source.directivity)}</span>
          </div>
          <div className="hint">Read-only in this build. The switch in the list below turns the source on or off.</div>
        </div>
      )}
    </>
  );
}

function PointRow({ scene, kind, point, on }: { scene: SceneState; kind: Kind; point: Point; on: boolean }) {
  const issues = issuesByEntity(scene.issues).get(`${kind}:${point.id}`) ?? [];
  const [x, y, z] = point.position;
  const source = kind === 'source' ? (point as Source) : null;
  const off = source !== null && !source.enabled;
  const row = (
    <button
      className="point-row"
      data-point-row={`${kind}:${point.id}`}
      aria-pressed={on}
      onClick={() => selectPoint(kind === 'source' ? 'source' : 'receiver', point.id)}
    >
      <span className="point-name mono">{point.name}</span>
      <span className="point-pos mono">
        ({coord(x, 2)}, {coord(y, 2)}, {coord(z, 2)})
      </span>
      {off && (
        <span className="point-state off" data-part="source-off">
          off
        </span>
      )}
      {issues.length ? (
        <IssueTag issues={issues} />
      ) : (
        !off && <span className="point-state">{scene.check?.verdict === 'ok' ? 'inside' : ''}</span>
      )}
    </button>
  );
  if (!source) return row;
  return (
    <div className="point-line">
      {row}
      <SourceSwitch source={source} />
    </div>
  );
}

export function SourcesPanel() {
  const scene = useStore(sceneStore);
  const selection = useStore(selectionStore);
  const refusals = useStore(refusalStore);
  const tool = useStore(toolStore);

  if (!scene) {
    return (
      <div data-part="sources-panel">
        <div className="props-head">
          <div className="title">Sources &amp; receivers</div>
          <div className="sub">No project open</div>
        </div>
        <div className="props-section empty">Open a project to place sources and receivers inside its room.</div>
      </div>
    );
  }

  const view = scene.view;
  const selected: { kind: Kind; point: Point } | null =
    selection.kind === 'source'
      ? (() => {
          const p = view.sources.find((s) => s.id === selection.id);
          return p ? { kind: 'source' as const, point: p } : null;
        })()
      : selection.kind === 'receiver'
        ? (() => {
            const p = view.point_receivers.find((r) => r.id === selection.id);
            return p ? { kind: 'point_receiver' as const, point: p } : null;
          })()
        : null;

  const canPlace = !!scene.check;
  const add = async (kind: 'source' | 'receiver') => {
    const p = roomCentre(scene.check, actions.PLACE_HEIGHT_M[kind]);
    if (!p) return;
    const out = await actions.placeAt(kind, p);
    if (!out.applied) return;
    const list = kind === 'source' ? out.state.view.sources : out.state.view.point_receivers;
    const last = list[list.length - 1];
    if (last) selectPoint(kind, last.id);
  };
  const placing = tool === 'place-receiver';

  return (
    <div data-part="sources-panel" onKeyDown={onEntityKey}>
      {selected ? (
        <PointEditor key={`${selected.kind}:${selected.point.id}`} scene={scene} kind={selected.kind} point={selected.point} />
      ) : (
        <div className="props-head">
          <div className="title">Sources &amp; receivers</div>
          <div className="sub">Select a source or a receiver to edit it</div>
        </div>
      )}

      <div className="props-section">
        <div className="section-head">
          <span className="label">Sources</span>
          <button
            className="small-button"
            data-part="add-source"
            disabled={!canPlace}
            title={canPlace ? 'Add a source in the middle of the room' : 'Import a room model first'}
            onClick={() => actions.fire(add('source'))}
          >
            + Source
          </button>
        </div>
        {view.sources.map((s) => (
          <PointRow key={s.id} scene={scene} kind="source" point={s} on={selection.kind === 'source' && selection.id === s.id} />
        ))}
        {!view.sources.length && <div className="empty">No sources yet.</div>}
        <Issues refused={refusals.get(fieldKey('source', 'new', 'position')) ?? []} current={[]} />
        <Issues refused={toggleRefusals(refusals)} current={[]} />
      </div>

      <div className="props-section">
        <div className="section-head">
          <span className="label">Receivers</span>
          <span className="head-buttons">
            <button
              className="small-button"
              data-part="place-in-view"
              aria-pressed={placing}
              disabled={!canPlace}
              title="Click a floor in the 3D view to place a receiver there"
              onClick={() => toolStore.set(placing ? 'select' : 'place-receiver')}
            >
              Place in view
            </button>
            <button
              className="small-button"
              data-part="add-receiver"
              disabled={!canPlace}
              title={canPlace ? 'Add a receiver in the middle of the room' : 'Import a room model first'}
              onClick={() => actions.fire(add('receiver'))}
            >
              + Receiver
            </button>
          </span>
        </div>
        {view.point_receivers.map((r) => (
          <PointRow
            key={r.id}
            scene={scene}
            kind="point_receiver"
            point={r}
            on={selection.kind === 'receiver' && selection.id === r.id}
          />
        ))}
        {!view.point_receivers.length && <div className="empty">No receivers yet.</div>}
        <Issues refused={refusals.get(fieldKey('point_receiver', 'new', 'position')) ?? []} current={[]} />
        <div className="hint">
          Receivers snap to {actions.PLACE_HEIGHT_M.receiver} m ear height. A point outside the room is refused,
          and the project is left unchanged.
        </div>
      </div>

      <PlanesSection scene={scene} />
    </div>
  );
}
