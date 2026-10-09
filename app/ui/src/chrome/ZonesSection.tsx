// G28/G29: the Geometry step's fitting zones, upstream's encombrements: a volume of the room filled
// with scattering objects (seats, a stage set, machinery), which SPPS treats as a scattering and
// absorbing medium. `+ Box zone` adds upstream's box, 1 m on a side, centred in the room's box (zones.ts); each zone's name, its box (From and To, each axis), its absorption and mean
// free path in every band at once or band by band, and its diffusion law are edited here, each
// through the checked apply as one undo step (the core's `fitting_parameters_invalid` refuses a
// value, inline, the project unchanged). A box with no volume, or reaching out of the room's box,
// is refused here first, in words. A zone of surfaces (only a `.proj` import makes one) shows its
// surfaces and its seed point, not edited. Each zone has its on/off switch (M43) and a remove
// button. The 3D view draws every enabled box zone's edges (engine.ts, zones.ts `zoneEdges`).
import { useState } from 'react';
import * as actions from '../actions';
import type { SceneState, UiIssue } from '../bindings/ipc';
import type { DiffusionLaw, FittingZone } from '../bindings/schema';
import { bandLabel } from '../features/materials/bands';
import { parseStrictDecimal } from '../numbers';
import { fieldKey, issuesByEntity } from '../issues';
import { removeFittingZone, rename, replaceFittingZone, setFittingBand } from '../ops';
import { fittingZonesStore, refusalStore, useStore } from '../store';
import { roomBox, type Box3 } from './planes';
import { EnabledRefusals, EnabledSwitch, IssueTag } from './ScenePanel';
import { CommitInput, Issues } from './SourcesPanel';
import { exact } from './sceneModel';
import { boxProblem, DIFFUSION_LAWS, isBoxZone, parseZoneValue, sameInEveryBand, withBound, withEveryBand, type BoxZone } from './zones';

const AXES = ['x', 'y', 'z'] as const;

function local(id: string, field: string, message: string): UiIssue {
  return { code: 'ZONE_FIELD', rule: '', severity: 'error', path: `fitting_zone:${id}:${field}`, entity: { kind: 'fitting_zone', id }, field, message };
}

/** The zone as it is now (a commit reads the latest state, not the render's). */
const currentZone = (id: string): FittingZone | undefined => fittingZonesStore.get()?.find((z) => z.id === id);

function ZoneEditor({ scene, zone, room }: { scene: SceneState; zone: FittingZone; room: Box3 | null }) {
  const refusals = useStore(refusalStore);
  const [mine, setMine] = useState<Readonly<Record<string, UiIssue>>>({});
  const id = zone.id;
  const key = (f: string) => fieldKey('fitting_zone', id, f);
  const setField = (f: string, i: UiIssue | null) =>
    setMine((m) => {
      const n = { ...m };
      if (i) n[f] = i;
      else delete n[f];
      return n;
    });
  const freqs = scene.view.bands.frequencies_hz;

  const commitName = async (text: string) => (await actions.apply(rename('fitting_zone', id, text), key('name'))).applied;
  const commitBound = (which: 'min' | 'max', axis: 0 | 1 | 2) => async (text: string) => {
    const f = `${which}.${AXES[axis]}`;
    const p = parseStrictDecimal(text);
    const now = currentZone(id);
    if (!now || !isBoxZone(now)) return false;
    if (!p.ok) {
      setField(f, local(id, f, `"${text}" is not a number: write digits with a decimal point, like 4.5`));
      return false;
    }
    const next = withBound(now, which, axis, p.value);
    const why = boxProblem(next.shape.min, next.shape.max, room);
    if (why) {
      setField(f, local(id, f, `The box is not changed: ${why}.`));
      return false;
    }
    setField(f, null);
    return (await actions.apply(replaceFittingZone(next as FittingZone), key('shape'))).applied;
  };
  const commitAll = (quantity: 'absorption' | 'mean_free_path_m') => async (text: string) => {
    const f = `all.${quantity}`;
    const p = parseZoneValue(text, quantity);
    if (!p.ok) {
      setField(f, local(id, f, p.message));
      return false;
    }
    setField(f, null);
    const now = currentZone(id);
    if (!now) return false;
    return (await actions.apply(replaceFittingZone(withEveryBand(now, quantity, p.value)), key('bands'))).applied;
  };
  const commitBand = (quantity: 'absorption' | 'mean_free_path_m', band: number) => async (text: string) => {
    const f = `${quantity}.${band}`;
    const p = parseZoneValue(text, quantity);
    if (!p.ok) {
      setField(f, local(id, f, p.message));
      return false;
    }
    setField(f, null);
    const op = setFittingBand(id, quantity === 'absorption' ? 'absorption' : 'mean_free_path', band, p.value);
    return (await actions.apply(op, key('bands'))).applied;
  };
  const chooseLaw = (law: DiffusionLaw) => {
    const now = currentZone(id);
    if (now) actions.fire(actions.apply(replaceFittingZone(withEveryBand(now, 'diffusion_law', law)), key('bands')));
  };

  const issues = issuesByEntity(scene.issues).get(`fitting_zone:${id}`) ?? [];
  const refusedOf = (...fields: string[]) => fields.flatMap((f) => refusals.get(key(f)) ?? []);
  const mineOf = (prefix: string) => Object.entries(mine).filter(([k]) => k.startsWith(prefix)).map(([, v]) => v);
  const alpha = sameInEveryBand(zone.absorption);
  const lambda = sameInEveryBand(zone.mean_free_path_m);
  const law = sameInEveryBand(zone.diffusion_law);
  const box = isBoxZone(zone) ? (zone as BoxZone) : null;
  const groupName = (g: string) => scene.view.surface_groups.find((x) => x.id === g)?.name ?? g;

  return (
    <div className="plane-row zone-row" data-zone={id} data-zone-name={zone.name}>
      <div className="section-head plane-head">
        <span className="point-name mono">{zone.name}</span>
        <IssueTag issues={issues} />
        <EnabledSwitch kind="fitting_zone" id={id} name={zone.name} on={zone.enabled} off="the run leaves its fittings out, as if the space were empty" />
        <button
          className="small-button"
          data-part="remove-zone"
          title={`Remove ${zone.name} (Ctrl+Z brings it back)`}
          onClick={() => actions.fire(actions.apply(removeFittingZone(id), key('')))}
        >
          Remove
        </button>
      </div>
      <EnabledRefusals kind="fitting_zone" id={id} />
      <label className="field-row">
        <span className="label">Name</span>
        <CommitInput field="zone.name" label={`${zone.name} name`} className="name-input" value={zone.name} invalid={refusedOf('name').length > 0} commit={commitName} onRevert={() => {}} />
      </label>
      <Issues refused={refusedOf('name')} current={[]} />

      {box ? (
        <div className="fact-grid zone-box" data-input data-part="zone-box">
          {(['min', 'max'] as const).map((which) =>
            AXES.map((a, axis) => (
              <label key={`${which}${a}`} className={`fact-cell axis-${a}`}>
                <span className="k">
                  {which === 'min' ? 'From' : 'To'} {a.toUpperCase()}
                </span>
                <span className="axis-field">
                  <CommitInput
                    field={`zone.${which}.${a}`}
                    label={`${zone.name} ${which === 'min' ? 'from' : 'to'} ${a.toUpperCase()} in metres`}
                    className="mono"
                    value={exact(box.shape[which][axis])}
                    invalid={!!mine[`${which}.${a}`]}
                    commit={commitBound(which, axis as 0 | 1 | 2)}
                    onRevert={() => setField(`${which}.${a}`, null)}
                  />
                  <span className="unit">m</span>
                </span>
              </label>
            )),
          )}
        </div>
      ) : (
        zone.shape.kind === 'surfaces' && (
          <div className="hint" data-part="zone-surfaces">
            The volume closed by {zone.shape.groups.map(groupName).join(', ') || 'no surface'}, seeded at (
            {zone.shape.inside_point.map((v) => exact(v)).join(', ')}) m, as the imported project has it; it is not edited here.
          </div>
        )
      )}
      <Issues refused={[...mineOf('min.'), ...mineOf('max.'), ...refusedOf('shape')]} current={[]} />

      <div className="fact-grid" data-input data-part="zone-all-bands">
        <label className="fact-cell">
          <span className="k">Absorption, all bands</span>
          <CommitInput
            field="zone.all.absorption"
            label={`${zone.name} absorption in every band, 0 to 1`}
            className="mono"
            value={alpha === null ? '' : exact(alpha)}
            placeholder="varies"
            invalid={!!mine['all.absorption']}
            commit={commitAll('absorption')}
            onRevert={() => setField('all.absorption', null)}
          />
        </label>
        <label className="fact-cell">
          <span className="k">Mean free path, all bands</span>
          <span className="axis-field">
            <CommitInput
              field="zone.all.mean_free_path"
              label={`${zone.name} mean free path in every band, metres`}
              className="mono"
              value={lambda === null ? '' : exact(lambda)}
              placeholder="varies"
              invalid={!!mine['all.mean_free_path_m']}
              commit={commitAll('mean_free_path_m')}
              onRevert={() => setField('all.mean_free_path_m', null)}
            />
            <span className="unit">m</span>
          </span>
        </label>
        <label className="fact-cell">
          <span className="k">Diffusion law</span>
          <select data-field="zone.law" aria-label={`${zone.name} diffusion law, every band`} value={law ?? ''} onChange={(e) => chooseLaw(e.target.value as DiffusionLaw)}>
            {law === null && (
              <option value="" disabled>
                Varies by band
              </option>
            )}
            {DIFFUSION_LAWS.map((l) => (
              <option key={l.key} value={l.key}>
                {l.label}
              </option>
            ))}
          </select>
        </label>
      </div>
      {(['absorption', 'mean_free_path_m'] as const).map((q) => (
        <div key={q}>
          <div className="label section-label">{q === 'absorption' ? 'Absorption by band (0 to 1)' : 'Mean free path by band (m)'}</div>
          <div className="band-levels" data-part={`zone-${q}`}>
            {zone[q].map((v, i) => (
              <label key={freqs[i]} className="band-level">
                <span className="k">{bandLabel(freqs[i])}</span>
                <CommitInput
                  field={`zone.${q}.${i}`}
                  label={`${zone.name} ${q === 'absorption' ? 'absorption' : 'mean free path'} at ${freqs[i]} Hz`}
                  className="mono"
                  value={exact(v)}
                  invalid={!!mine[`${q}.${i}`]}
                  commit={commitBand(q, i)}
                  onRevert={() => setField(`${q}.${i}`, null)}
                />
              </label>
            ))}
          </div>
        </div>
      ))}
      <Issues refused={[...mineOf('all.'), ...mineOf('absorption.'), ...mineOf('mean_free_path_m.'), ...refusedOf('bands', '')]} current={[]} />
    </div>
  );
}

export function ZonesSection({ scene }: { scene: SceneState }) {
  const zones = useStore(fittingZonesStore) ?? [];
  const refusals = useStore(refusalStore);
  const room = roomBox(scene.check);
  return (
    <div className="props-section" data-part="zones">
      <div className="section-head">
        <span className="label">Fitting zones</span>
        <button
          className="small-button"
          data-part="add-zone"
          disabled={!room}
          title={room ? 'Add a box fitting zone, 1 m on a side, in the middle of the room; then set its corners' : 'Import a room model first'}
          onClick={() => actions.fire(actions.addBoxZone())}
        >
          + Box zone
        </button>
      </div>
      {zones.map((z) => (
        <ZoneEditor key={z.id} scene={scene} zone={z} room={room} />
      ))}
      {!zones.length && <div className="empty">No fitting zones.</div>}
      <Issues refused={refusals.get(fieldKey('fitting_zone', 'new', 'shape')) ?? []} current={[]} />
      <div className="hint" data-part="zone-hint">
        A fitting zone is a part of the room full of objects that scatter sound (seats, a stage set): SPPS scatters and absorbs
        particles inside it by its mean free path and absorption. A new or changed zone is meshed again at the next run.
      </div>
    </div>
  );
}
