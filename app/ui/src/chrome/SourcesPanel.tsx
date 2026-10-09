// The Sources & receivers step's properties (design:352-384). The selected source or receiver:
// its name (`data-field="name"`) and position (`data-field="position.x|y|z"`), each committed on
// Enter or blur (numbers through `parseStrictDecimal`, then a move op through the checked apply);
// inline `[data-issue-code]` messages from the refusals and the validator; a source's emission
// inside `[data-input]`, edited since C1 (EmissionEditor.tsx: power overall and per band,
// spectrum, directivity). Then the sources and receivers with
// + Source, + Receiver and Place in view; Del removes and F2 renames the selection.
//
// M11 (row 22, M26): each source in the list has its on/off switch (`[data-source-toggle]`, the
// scene list's `SourceSwitch`, here with its state as text), and a disabled source reads "off"
// in the list and in its editor's head. A refused switch is shown under the sources.
//
// W1 (parity M41): the sound-level planes below the receivers (PlanesSection.tsx).
//
// M27: a source's group (`data-field="source.group"`), typed as a path (`Stage / Left`) and
// committed through the checked apply (`replace_source`); empty is the top level. It reaches no
// solver. The list shows it beside the name (`[data-source-group]`).
import { useEffect, useRef, useState, type Ref } from 'react';
import * as actions from '../actions';
import type { PointReceiver, SceneState, Source, UiIssue } from '../bindings/ipc';
import { fieldKey, issuesByEntity, issuesForField } from '../issues';
import { NOT_A_NUMBER, parseStrictDecimal } from '../numbers';
import { moveReceiver, moveSource, rename, replaceReceiver, replaceSource, type Vec3 } from '../ops';
import { refusalStore, sceneStore, selectionStore, toolStore, useStore } from '../store';
import { EmissionEditor } from './EmissionEditor';
import { PlanesSection } from './PlanesSection';
import { IssueTag, SourceSwitch, toggleRefusals } from './ScenePanel';
import {
  AXES,
  coord,
  directionTo,
  exact,
  groupPath,
  roomCentre,
  sourceSpot,
  sentence,
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
  /** Shown while the field is empty: what an empty value means (GUI audit 2026-10-09 C6). */
  placeholder?: string;
}) {
  const { field, label, value, invalid, commit, onRevert, inputRef, className, onDraft, placeholder } = props;
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
      placeholder={placeholder}
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

  // M27: a source's group, typed as a path (`Stage / Left`); empty is the top level.
  const commitGroup = async (text: string) => {
    const now = current('source', id) as Source | undefined;
    if (!now) return false;
    const group = groupPath(text);
    if (group === now.group) {
      hide(keyOf('group'), true);
      return true;
    }
    const out = await actions.apply(replaceSource({ ...now, group }), keyOf('group'));
    if (!out.applied) hide(keyOf('group'), false);
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
  // G48: the face a placement click put it on, while it still stands at the point written.
  const placement = useStore(actions.placementStore).get(id);
  const placed = placement && placement.point.every((v, i) => v === point.position[i]) ? placement : null;
  const errors = worstSeverity(scene.issues.filter((i) => i.entity?.kind === kind && i.entity.id === id)) === 'error';
  const source = kind === 'source' ? (point as Source) : null;
  const receiver = kind === 'point_receiver' ? (point as PointReceiver) : null;
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
        {source && (
          <>
            <label className="field-row" title="The source group it sits in, as upstream groups sources: names from the outermost, joined by /. It reaches no solver; names need be unique only within one group">
              <span className="label">Group</span>
              <CommitInput
                field="source.group"
                label="Source group"
                className="name-input"
                value={source.group ?? ''}
                placeholder="None (top level)"
                invalid={refusedFor(keyOf('group')).length > 0}
                commit={commitGroup}
                onRevert={() => hide(keyOf('group'), true)}
              />
            </label>
            <Issues refused={refusedFor(keyOf('group'))} current={[]} />
          </>
        )}
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
        {placed && (
          <div className="hint" data-part="placed-on" title="Where the 3D view's placement click put it; a move clears this">
            Placed {actions.placementText(placed)}.
          </div>
        )}
      </div>

      {receiver && <OrientationSection scene={scene} receiver={receiver} />}

      {source && <EmissionEditor scene={scene} source={source} />}
    </>
  );
}

/**
 * M32: a point receiver's orientation, upstream's u, v, w (`e_scene_recepteursp_recepteur_proprietes.h:46-48`:
 * Direction X, Y, Z, a direction with no unit), each committed on Enter or blur through the checked apply
 * (`replace_point_receiver`, one undo step), and "Face" a source, upstream's Orientation point: the
 * direction of length one toward it. Written to the solver as `u`, `v`, `w` as stored; the solver
 * normalises it. A zero direction is refused by the validator.
 */
function OrientationSection({ scene, receiver }: { scene: SceneState; receiver: PointReceiver }) {
  const refusals = useStore(refusalStore);
  const kind: Kind = 'point_receiver';
  const id = receiver.id;
  const key = fieldKey(kind, id, 'orientation');
  const [local, setLocal] = useState<Readonly<Record<string, UiIssue>>>({});
  const [hidden, setHidden] = useState(false);
  const [faceProblem, setFaceProblem] = useState<string | null>(null);
  const setLocalIssue = (field: string, issue: UiIssue | null) =>
    setLocal((l) => {
      const n = { ...l };
      if (issue) n[field] = issue;
      else delete n[field];
      return n;
    });
  const send = async (orientation: Vec3): Promise<boolean> => {
    const now = current(kind, id) as PointReceiver | undefined;
    if (!now) return false;
    if (orientation.every((v, i) => Object.is(v, now.orientation[i]))) return true;
    const out = await actions.apply(replaceReceiver({ ...now, orientation }), key);
    setHidden(out.applied);
    return out.applied;
  };
  const commitAxis = (axis: Axis) => async (text: string) => {
    const field = `orientation.${axis}`;
    const parsed = parseStrictDecimal(text);
    if (!parsed.ok) {
      setLocalIssue(field, notANumber(text, kind, id, field));
      return false;
    }
    setLocalIssue(field, null);
    const now = current(kind, id) as PointReceiver | undefined;
    if (!now) return false;
    return send(withAxis(now.orientation, axis, parsed.value));
  };
  const refused = hidden ? [] : (refusals.get(key) ?? []);
  const issues = issuesForField(scene.issues, kind, id, 'orientation');
  const localIssues = AXES.map((a) => local[`orientation.${a}`]).filter((x): x is UiIssue => !!x);
  const sources = scene.view.sources;
  return (
    <div className="props-section" data-part="receiver-orientation">
      <div className="label section-label">Orientation</div>
      <div className="fact-grid" data-input>
        {AXES.map((axis, i) => (
          <label key={axis} className={`fact-cell axis-${axis}`}>
            <span className="k">{axis.toUpperCase()}</span>
            <span className="axis-field">
              <CommitInput
                field={`orientation.${axis}`}
                label={`Direction ${axis.toUpperCase()}, no unit`}
                className="mono"
                value={exact(receiver.orientation[i])}
                invalid={refused.length > 0 || !!local[`orientation.${axis}`]}
                commit={commitAxis(axis)}
                onRevert={() => {
                  setLocalIssue(`orientation.${axis}`, null);
                  setHidden(true);
                }}
              />
            </span>
          </label>
        ))}
      </div>
      {sources.length > 0 && (
        <label className="field-row" title="Upstream's Orientation point: the direction toward it, of length one">
          <span className="label">Face</span>
          <select
            data-field="orientation-face"
            aria-label="Turn the receiver to face a source"
            value=""
            onChange={(e) => {
              const s = sources.find((x) => x.id === e.target.value);
              const now = current(kind, id) as PointReceiver | undefined;
              if (!s || !now) return;
              const d = directionTo(now.position, s.position);
              if (!d) {
                setFaceProblem(`${now.name} and ${s.name} are at the same point: there is no direction between them`);
                return;
              }
              setFaceProblem(null);
              actions.fire(send(d));
            }}
          >
            <option value="">a source…</option>
            {sources.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
        </label>
      )}
      {faceProblem && <div className="hint" data-part="orientation-face-problem">{faceProblem}.</div>}
      <Issues refused={[...localIssues, ...refused]} current={issues} />
      <div className="hint">
        A direction, no unit (upstream's u, v, w); the solver makes it length one. It feeds only SPPS's
        lateral-energy columns (LF, LFC), which this version does not show: no level or parameter shown here
        depends on it.
      </div>
    </div>
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
      {source?.group && (
        <span className="row-folder" data-source-group={source.group} title={`In the source group ${source.group}`}>
          {source.group}
        </span>
      )}
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
    const p = kind === 'source' ? sourceSpot(scene.check, actions.PLACE_HEIGHT_M.source) : roomCentre(scene.check, actions.PLACE_HEIGHT_M.receiver);
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
              title="Click any face in the 3D view to place a receiver: above a floor, or off a wall or ceiling into the room"
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
          + Receiver puts one at {actions.PLACE_HEIGHT_M.receiver} m ear height; Place in view puts it the distance the
          view's hint shows from the face clicked. A point outside the room is refused, and the project is left unchanged.
        </div>
      </div>

      <PlanesSection scene={scene} />
    </div>
  );
}
