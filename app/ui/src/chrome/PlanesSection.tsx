// The Sources & receivers step's sound-level planes (wow list W1, parity M41;
// docs/investigations/2026-10-04-wow-w1w5/PLAN.md): `+ Ear-height plane` adds upstream's new
// plane over the model at 1.6 m above its floor; each level plane's height above the floor and
// cell size are edited here, committed through the checked apply (the core's
// `cutting_plane_invalid` refuses inline, the project unchanged), and a plane is removed with its
// button. A tilted plane (only a `.proj` import makes one) is shown, not edited. Every edit says
// what it costs: a new or moved plane has no map until SPPS runs again. M43: each plane has an
// on/off switch; off, the view hides it and the run leaves it out.
import { cubeText, REFUSE_GB, resultCube, settingsStore, WARN_GB } from '../features/simulate/runSize';
import { useState } from 'react';
import * as actions from '../actions';
import type { SceneState, SurfaceReceiver, UiIssue } from '../bindings/ipc';
import { fieldKey, issuesByEntity } from '../issues';
import { removeSurfaceReceiver, replaceSurfaceReceiver } from '../ops';
import { refusalStore, useStore } from '../store';
import { heightAboveFloor, parseHeightAboveFloor, parseResolution, planeCells, roomBox, withLevelHeight, type Box3 } from './planes';
import { EnabledRefusals, EnabledSwitch, IssueTag } from './ScenePanel';
import { CommitInput, Issues } from './SourcesPanel';
import { exact } from './sceneModel';

type Plane = SurfaceReceiver & { shape: Extract<SurfaceReceiver['shape'], { kind: 'cutting_plane' }> };
const isPlane = (r: SurfaceReceiver): r is Plane => r.shape.kind === 'cutting_plane';

/** A height for the field: to the millimetre, as the room's own numbers carry float noise (0.5 + 1.6). */
const heightText = (h: number) => String(Number(h.toFixed(3)));

function local(id: string, field: string, message: string): UiIssue {
  return { code: 'PLANE_FIELD', rule: '', severity: 'error', path: `surface_receiver:${id}:${field}`, entity: { kind: 'surface_receiver', id }, field, message };
}

function PlaneRow({ scene, plane, box }: { scene: SceneState; plane: Plane; box: Box3 }) {
  const refusals = useStore(refusalStore);
  const [mine, setMine] = useState<Readonly<Record<string, UiIssue>>>({});
  const id = plane.id;
  const key = fieldKey('surface_receiver', id, 'shape');
  const setField = (f: string, i: UiIssue | null) =>
    setMine((m) => {
      const n = { ...m };
      if (i) n[f] = i;
      else delete n[f];
      return n;
    });
  const settings = useStore(settingsStore);
  const current = (): Plane | undefined => scene.view.surface_receivers.find((r): r is Plane => r.id === id && isPlane(r));
  const send = async (shape: Plane['shape']) => {
    const now = current();
    if (!now) return false;
    const out = await actions.apply(replaceSurfaceReceiver({ ...now, shape }), key);
    return out.applied;
  };
  const commitHeight = async (text: string) => {
    const p = parseHeightAboveFloor(text, box);
    if (!p.ok) {
      setField('height', local(id, 'height', p.message));
      return false;
    }
    setField('height', null);
    return send(withLevelHeight(plane.shape, box.min[2] + p.value));
  };
  const commitResolution = async (text: string) => {
    const p = parseResolution(text);
    if (!p.ok) {
      setField('resolution', local(id, 'resolution', p.message));
      return false;
    }
    setField('resolution', null);
    return send({ ...plane.shape, resolution_m: p.value });
  };

  const h = heightAboveFloor(plane.shape, box);
  const grid = planeCells(plane.shape.a, plane.shape.b, plane.shape.c, plane.shape.resolution_m);
  const issues = (issuesByEntity(scene.issues).get(`surface_receiver:${id}`) ?? []);
  const refused = [...Object.values(mine), ...(refusals.get(key) ?? []), ...(refusals.get(fieldKey('surface_receiver', id, '')) ?? [])];
  return (
    <div className="plane-row" data-plane={id} data-plane-name={plane.name}>
      <div className="section-head plane-head">
        <span className="point-name mono">{plane.name}</span>
        <IssueTag issues={issues} />
        <EnabledSwitch kind="surface_receiver" id={id} name={plane.name} on={plane.enabled} off="the view hides it and the run makes no map for it" />
        <button
          className="small-button"
          data-part="remove-plane"
          title={`Remove ${plane.name}`}
          onClick={() => actions.fire(actions.apply(removeSurfaceReceiver(id), fieldKey('surface_receiver', id, '')))}
        >
          Remove
        </button>
      </div>
      <div className="fact-grid plane-grid" data-input>
        {h === null ? (
          <div className="fact-cell" data-part="plane-tilted">
            <span className="k">Tilted plane</span>
            <span className="unit">Its corners come from the imported project; it is not edited here.</span>
          </div>
        ) : (
          <label className="fact-cell">
            <span className="k">Height above floor</span>
            <span className="axis-field">
              <CommitInput
                field="plane.height"
                label={`${plane.name} height above the floor in metres`}
                className="mono"
                value={heightText(h)}
                invalid={!!mine.height}
                commit={commitHeight}
                onRevert={() => setField('height', null)}
              />
              <span className="unit">m</span>
            </span>
          </label>
        )}
        <label className="fact-cell">
          <span className="k">Cell size</span>
          <span className="axis-field">
            <CommitInput
              field="plane.resolution"
              label={`${plane.name} cell size in metres`}
              className="mono"
              value={exact(plane.shape.resolution_m)}
              invalid={!!mine.resolution}
              commit={commitResolution}
              onRevert={() => setField('resolution', null)}
            />
            <span className="unit">m</span>
          </span>
        </label>
        <div className="fact-cell">
          <span className="k">Cells</span>
          <span className="mono" data-part="plane-cells">
            {grid ? `${grid.u} × ${grid.v}` : '—'}
          </span>
        </div>
        {(() => {
          // This plane's share of what the solver holds in memory (runSize.ts): the trap of 2026-10-06, a 0.1 m cell
          // over a whole hall, 149 GB, shows here as it is typed.
          const own = resultCube({ ...scene, view: { ...scene.view, surface_receivers: [plane] } }, 'spps', settings);
          if (!own || !grid) return null;
          const level = own.gb >= REFUSE_GB ? 'fail' : own.gb >= WARN_GB ? 'warn' : '';
          return (
            <div className={`fact-cell${level ? ` ${level}` : ''}`} data-part="plane-memory" data-level={level || 'ok'}>
              <span className="k">Memory per run</span>
              <span className="mono">{cubeText(own)}</span>
            </div>
          );
        })()}
      </div>
      <Issues refused={refused} current={[]} />
      <EnabledRefusals kind="surface_receiver" id={id} />
    </div>
  );
}

export function PlanesSection({ scene }: { scene: SceneState }) {
  const refusals = useStore(refusalStore);
  const box = roomBox(scene.check);
  const planes = scene.view.surface_receivers.filter(isPlane);
  return (
    <div className="props-section" data-part="planes">
      <div className="section-head">
        <span className="label">Sound-level planes</span>
        <button
          className="small-button"
          data-part="add-plane"
          disabled={!box}
          title={box ? 'Add a level plane over the whole room at ear height, 1.6 m above the floor' : 'Import a room model first'}
          onClick={() => actions.fire(actions.addEarPlane())}
        >
          + Ear-height plane
        </button>
      </div>
      {box && planes.map((p) => <PlaneRow key={p.id} scene={scene} plane={p} box={box} />)}
      {!planes.length && <div className="empty">No planes yet.</div>}
      <Issues refused={refusals.get(fieldKey('surface_receiver', 'new', 'shape')) ?? []} current={[]} />
      <div className="hint" data-part="plane-rerun-hint">
        A plane maps the sound level across the room, cell by cell. A new or moved plane has no map until SPPS
        runs again.
      </div>
    </div>
  );
}
