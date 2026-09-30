// The Materials step's properties (design:312-350): the selected surface group, its material as
// a radio list with α mini-bars, and the library grid (TSV paste and copy, row fill, natural
// sort, inline validator messages). PLAN.md 2.3 and 6.2. It reads the stores and writes only
// through `actions.apply` with `ops`.
import { useState } from 'react';
import * as actions from '../../actions';
import { fieldKey } from '../../issues';
import { assignMaterial, batch } from '../../ops';
import { STEPS } from '../../steps';
import { refusalStore, sceneStore, selectionStore, useStore } from '../../store';
import { dismiss, errorOf, IssueLines, lineOf, visibleRefusals, type Line } from './inline';
import { MaterialsGrid } from './MaterialsGrid';
import { activeVariant, alphaBars, effectiveMaterial, formatArea, selectedGroupIds, transmissionText } from './model';
import './materials.css';

export function MaterialsPanel() {
  const scene = useStore(sceneStore);
  if (!scene) {
    // No project: the step's own head and M9's hint (the panel draws its head, as the others do).
    return (
      <div data-part="materials">
        <div className="props-head">
          <div className="title">{STEPS[1].name}</div>
          <div className="sub">{STEPS[1].sub}</div>
        </div>
        <div className="props-body empty">{STEPS[1].hint}</div>
      </div>
    );
  }
  return (
    <div className="mat-panel" data-part="materials">
      <GroupSection />
      <MaterialsGrid />
      <div className="mat-hint empty">Double-click a face to take its whole flat surface.</div>
    </div>
  );
}

/** The selected surface group (or the groups of selected faces), and the material it has. */
function GroupSection() {
  const scene = useStore(sceneStore);
  const selection = useStore(selectionStore);
  const refusals = useStore(refusalStore);
  const [failed, setFailed] = useState<{ ids: string; code: string; message: string } | null>(null);
  if (!scene) return null;
  const view = scene.view;
  const ids = selectedGroupIds(selection, view.surface_groups);

  if (ids.length === 0) {
    return (
      <section className="mat-sec mat-group" data-part="material-group">
        <div className="mat-title">No surface selected</div>
        <div className="mat-sub">Pick a surface in the scene list or in the view to set its material.</div>
      </section>
    );
  }

  const variant = activeVariant(view);
  const groups = ids.map((id) => view.surface_groups.find((g) => g.id === id)).filter((g) => g !== undefined);
  const stats = ids.map((id) => scene.groups.find((s) => s.id === id)).filter((s) => s !== undefined);
  const effective = groups.map((g) => effectiveMaterial(g, variant));
  const checked = effective.every((m) => m === effective[0]) ? effective[0] : null;
  const checkedMaterial = view.materials.find((m) => m.id === checked);
  const area = stats.reduce((a, s) => a + (typeof s.area_m2 === 'number' ? s.area_m2 : NaN), 0);
  const faces = stats.reduce((a, s) => a + s.faces, 0);
  const unassigned = stats.filter((s) => !s.assigned).length;
  const title = groups.length === 1 ? groups[0].name : `${groups.length} surface groups`;
  const idsKey = ids.join(',');
  const ownKey = (key: string) => ids.some((id) => key === fieldKey('surface_group', id, 'material'));
  const many = view.bands.frequencies_hz.length > 12;

  const pick = (material: string) => {
    dismiss(refusalStore.get(), ownKey);
    setFailed(null);
    const ops = groups.filter((_, i) => effective[i] !== material).map((g) => assignMaterial(g.id, material, view.active_variant));
    if (ops.length === 0) return;
    actions
      .apply(ops.length === 1 ? ops[0] : batch(ops), fieldKey('surface_group', ids[0], 'material'))
      .catch((e: unknown) => setFailed({ ids: idsKey, ...errorOf(e) }));
  };

  const lines: Line[] = [];
  if (failed && failed.ids === idsKey) lines.push({ label: 'FAIL', code: failed.code, message: failed.message });
  for (const r of visibleRefusals(refusals, ownKey)) lines.push(lineOf(r));
  for (const i of scene.issues) if (i.entity?.kind === 'surface_group' && ids.includes(i.entity.id)) lines.push(lineOf(i));

  return (
    <>
      <section className="mat-sec mat-group" data-part="material-group" data-group-ids={idsKey}>
        <div className="mat-title">{title}</div>
        <div className="mat-sub" data-geometry="">
          {groups.length === 1 ? 'Surface group' : groups.map((g) => g.name).join(', ')} · {formatArea(area)} m² · {faces}{' '}
          {faces === 1 ? 'face' : 'faces'}
        </div>
        {variant && (
          <div className="mat-sub">
            Variant <span className="mat-strong">{variant.name}</span>: a pick here is the variant's override.
          </div>
        )}
        {unassigned > 0 && (
          <div className="mat-flag" data-part="unassigned">
            <span className="mat-tag">UNASSIGNED</span>
            <span>
              {unassigned === 1 && groups.length === 1 ? 'This group has' : `${unassigned} of these groups have`} the import
              placeholder 'Default'. Pick a material; Run stays blocked until every group has one.
            </span>
          </div>
        )}
      </section>
      <section className="mat-sec">
        <div className="label mat-sec-title">Material</div>
        <div role="radiogroup" aria-label={`${title} material`} className="mat-options">
          {view.materials.map((m) => {
            const on = m.id === checked;
            return (
              <button
                key={m.id}
                type="button"
                role="radio"
                aria-checked={on}
                className="mat-option"
                data-material-option={m.id}
                onClick={() => pick(m.id)}
              >
                <span className="radio">
                  <span className="dot" />
                </span>
                <span className="mat-swatch" style={{ background: m.color }} />
                {/* A material's name is an input ("30% absorbing" is a name, not a result). */}
                <span className="name" title={m.name} data-input="">
                  {m.name}
                </span>
                <span className={many ? 'mat-bars many' : 'mat-bars'} aria-hidden="true">
                  {alphaBars(m.absorption).map((h, i) => (
                    <span key={i} style={{ height: `${h}px` }} />
                  ))}
                </span>
              </button>
            );
          })}
        </div>
        <div className="mat-row" title="Transmission is read-only in this version">
          <span>Transmission</span>
          <span className="mono">{checkedMaterial ? transmissionText(checkedMaterial) : 'mixed'}</span>
        </div>
        <IssueLines lines={lines} part="group-issues" />
      </section>
    </>
  );
}
