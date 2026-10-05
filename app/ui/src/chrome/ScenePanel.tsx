// The scene list (design:69-102): a filter, then Surfaces (the effective material's swatch and
// name), Sources, Receivers and surface receivers, each row `[data-entity="<kind>:<id>"]` with a
// FAIL (or WARN) label and the UI code when the validator names it. A row click selects it;
// Del and F2 act on a selected source or receiver (PLAN.md 7.5).
//
// M11 (row 22, M26): each source has an on/off switch, `[data-source-toggle=<id>]`, here and in
// the Sources panel's list, through the checked apply (`set_source_enabled`). A disabled source
// reads "off" as text. A refused switch (the only enabled source switched off: SOURCE_NONE; a
// source switched on outside the room: SOURCE_OUTSIDE) is shown inline under the sources, in
// both places, and the project is left unchanged. The switch is a button of its own beside the
// row's button, never inside it.
//
// Scope row 15 (1), M37: a point receiver an imported `.proj` put in a receiver group shows the
// group's path, read-only (`[data-receiver-group]`), and the filter finds it by it.
import { useState } from 'react';
import * as actions from '../actions';
import type { Source, UiIssue } from '../bindings/ipc';
import { issuesByEntity, projectIssues } from '../issues';
import { refusalStore, sceneStore, selectionStore, useStore } from '../store';
import { FoldButton, useFold } from './fold';
import { Search, Trash2 } from './icons';
import { coord, effectiveMaterial, matchesFilter, receiverFolder, sentence, uniqueIssues, worstSeverity } from './sceneModel';
import { onEntityKey, removeEntity, selectGroup, selectPoint } from './sceneUi';

// ---- the source switch (M26) -------------------------------------------------------------------

/** actions.setSourceEnabled files a switch's refusals under `source:<id>:enabled`. */
const isToggleKey = (key: string) => key.startsWith('source:') && key.endsWith(':enabled');
/** Refusals put away by a later switch attempt (by identity, so a new refusal shows again). */
const dismissedToggles = new WeakSet<readonly UiIssue[]>();

/** The latest switch attempt's refusals, if it was refused. */
export function toggleRefusals(refusals: ReadonlyMap<string, UiIssue[]>): UiIssue[] {
  const out: UiIssue[][] = [];
  for (const [key, list] of refusals) if (isToggleKey(key) && !dismissedToggles.has(list)) out.push(list);
  return uniqueIssues(...out);
}

/** Switches a source on or off; the messages shown become this attempt's. */
async function toggleSource(id: string, enabled: boolean): Promise<void> {
  for (const [key, list] of refusalStore.get()) if (isToggleKey(key)) dismissedToggles.add(list);
  await actions.setSourceEnabled(id, enabled);
}

/**
 * A source's on/off switch (the design's switch, design:446-449): red with the knob right when
 * on, grey with the knob left when off, and the state as text beside it (`compact` leaves the
 * text to the row, which prints "off"). `aria-checked` and `data-state` carry the state too.
 */
export function SourceSwitch({ source, compact = false }: { source: Source; compact?: boolean }) {
  const [pending, setPending] = useState(false);
  const on = source.enabled;
  return (
    <button
      type="button"
      role="switch"
      className={`switch${compact ? ' compact' : ''}`}
      aria-checked={on}
      aria-label={`${source.name} on`}
      data-source-toggle={source.id}
      data-state={on ? 'on' : 'off'}
      disabled={pending}
      title={on ? `Switch ${source.name} off: it emits nothing and the run leaves it out` : `Switch ${source.name} on`}
      onClick={(e) => {
        e.stopPropagation();
        setPending(true);
        toggleSource(source.id, !on)
          .catch(() => {})
          .finally(() => setPending(false));
      }}
    >
      <span className="switch-track" aria-hidden>
        <span className="switch-knob" />
      </span>
      {!compact && <span className="switch-text">{on ? 'on' : 'off'}</span>}
    </button>
  );
}

/** A refused switch, inline: FAIL and the UI code as text, then the message. */
export function ToggleRefusalLines({ issues }: { issues: readonly UiIssue[] }) {
  if (!issues.length) return null;
  return (
    <div className="issues toggle-issues" data-part="source-toggle-issues">
      {issues.map((i) => (
        <div key={`${i.code}|${i.path}|${i.message}`} className="issue" data-issue-code={i.code} role="alert">
          <span className="code">FAIL {i.code}</span>
          <span className="msg">{sentence(i.message)} Refused; the project is unchanged.</span>
        </div>
      ))}
    </div>
  );
}

/** FAIL or WARN and the first code, as text: a failure never relies on colour alone. */
export function IssueTag({ issues }: { issues: readonly UiIssue[] }) {
  const worst = worstSeverity(issues);
  if (!worst) return null;
  const first = issues.find((i) => i.severity === worst) ?? issues[0];
  const more = issues.length > 1 ? ` +${issues.length - 1}` : '';
  return (
    <span
      className={`issue-tag ${worst}`}
      data-row-issue={first.code}
      title={issues.map((i) => `${i.code} (${i.rule}): ${i.message}`).join('\n')}
    >
      <span className="state">{worst === 'error' ? 'FAIL' : 'WARN'}</span>
      <span className="code">
        {first.code}
        {more}
      </span>
    </span>
  );
}

/** A row's own remove button: one click removes it (Ctrl+Z brings it back). */
function RemoveButton({ kind, id, name }: { kind: 'source' | 'receiver'; id: string; name: string }) {
  return (
    <button
      className="row-remove"
      data-part="row-remove"
      aria-label={`Remove ${name}`}
      title={`Remove ${name} (Del; Ctrl+Z brings it back)`}
      onClick={() => actions.fire(removeEntity(kind, id))}
    >
      <Trash2 size={12} />
    </button>
  );
}

function Head({ title, shown, total }: { title: string; shown: number; total: number }) {
  return (
    <div className="scene-head label">
      <span>{title}</span>
      <span className="count">{shown === total ? total : `${shown} of ${total}`}</span>
    </div>
  );
}

export function ScenePanel() {
  const scene = useStore(sceneStore);
  const selection = useStore(selectionStore);
  const refusals = useStore(refusalStore);
  const [query, setQuery] = useState('');
  const view = scene?.view ?? null;
  const byEntity = issuesByEntity(scene?.issues ?? []);
  const issuesOf = (kind: string, id: string) => byEntity.get(`${kind}:${id}`) ?? [];
  const pickedGroup = selection.kind === 'group' ? selection.id : null;
  const pickedGroupNames = new Set(selection.kind === 'faces' ? selection.groups : []);

  const stats = new Map((scene?.groups ?? []).map((s) => [s.id, s]));
  const surfaces = view
    ? view.surface_groups.map((g) => ({ g, m: effectiveMaterial(view, g.id), assigned: stats.get(g.id)?.assigned ?? true }))
    : [];
  const shownSurfaces = surfaces.filter(({ g, m }) => matchesFilter(query, g.name, m?.name ?? ''));
  const sources = (view?.sources ?? []).filter((s) => matchesFilter(query, s.name));
  const receivers = (view?.point_receivers ?? []).filter((r) => matchesFilter(query, r.name, receiverFolder(r)));
  const grids = (view?.surface_receivers ?? []).filter((r) => matchesFilter(query, r.name));
  const general = projectIssues(scene?.issues ?? []);
  const folded = useFold('scene');

  return (
    <aside className="scene" aria-label="Scene" data-folded={folded}>
      <FoldButton panel="scene" />
      <label className="filter">
        <Search size={12} />
        <input
          type="text"
          placeholder="Filter scene"
          aria-label="Filter scene"
          data-part="scene-filter"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => e.key === 'Escape' && setQuery('')}
          spellCheck={false}
        />
      </label>
      <div className="scene-list" data-part="scene-list" onKeyDown={onEntityKey}>
        {!view ? (
          <div className="scene-empty empty">No project. File, then Open…</div>
        ) : (
          <>
            <Head title="Surfaces" shown={shownSurfaces.length} total={surfaces.length} />
            {shownSurfaces.map(({ g, m, assigned }) => {
              const on = pickedGroup === g.id || pickedGroupNames.has(g.name);
              return (
                <button
                  key={g.id}
                  className="scene-row"
                  data-entity={`surface_group:${g.id}`}
                  aria-pressed={on}
                  onClick={() => selectGroup(g.id)}
                >
                  <span
                    className={`swatch${assigned ? '' : ' unassigned'}`}
                    style={assigned && m ? { background: m.color } : undefined}
                  />
                  <span className="row-name">{g.name}</span>
                  <IssueTag issues={issuesOf('surface_group', g.id)} />
                  <span
                    className="row-detail"
                    data-input
                    title={assigned ? m?.name : `${m?.name ?? 'No material'}: the import placeholder, not assigned yet`}
                  >
                    {assigned ? (m?.name ?? '—') : 'unassigned'}
                  </span>
                </button>
              );
            })}
            {!surfaces.length && <div className="scene-empty empty">No surfaces</div>}

            <Head title="Sources" shown={sources.length} total={view.sources.length} />
            {sources.map((s) => (
              <div key={s.id} className="scene-line">
                <button
                  className="scene-row"
                  data-entity={`source:${s.id}`}
                  aria-pressed={selection.kind === 'source' && selection.id === s.id}
                  onClick={() => selectPoint('source', s.id)}
                >
                  <span className={`marker source${s.enabled ? '' : ' off'}`} />
                  <span className="row-name">{s.name}</span>
                  <IssueTag issues={issuesOf('source', s.id)} />
                  {!s.enabled && (
                    <span className="row-detail row-off" data-part="source-off">
                      off
                    </span>
                  )}
                  <span className="row-detail" data-input>
                    {s.directivity.kind === 'omni' ? 'Omni' : 'Directional'} · {String(s.power.global_db)} dB
                  </span>
                </button>
                <SourceSwitch source={s} compact />
                <RemoveButton kind="source" id={s.id} name={s.name} />
              </div>
            ))}
            {!view.sources.length && <div className="scene-empty empty">No sources</div>}
            <ToggleRefusalLines issues={toggleRefusals(refusals)} />

            <Head
              title="Receivers"
              shown={receivers.length + grids.length}
              total={view.point_receivers.length + view.surface_receivers.length}
            />
            {receivers.map((r) => (
              <div key={r.id} className="scene-line">
              <button
                className="scene-row"
                data-entity={`point_receiver:${r.id}`}
                aria-pressed={selection.kind === 'receiver' && selection.id === r.id}
                onClick={() => selectPoint('receiver', r.id)}
              >
                <span className="marker receiver" />
                <span className="row-name">{r.name}</span>
                {receiverFolder(r) && (
                  <span
                    className="row-folder"
                    data-receiver-group={receiverFolder(r)}
                    title={`In the receiver group ${receiverFolder(r)}, as the imported project has it`}
                  >
                    {receiverFolder(r)}
                  </span>
                )}
                <IssueTag issues={issuesOf('point_receiver', r.id)} />
                <span className="row-detail mono">
                  {coord(r.position[0], 1)}, {coord(r.position[1], 1)}
                </span>
              </button>
              <RemoveButton kind="receiver" id={r.id} name={r.name} />
              </div>
            ))}
            {grids.map((r) => (
              <div key={r.id} className="scene-row static" data-entity={`surface_receiver:${r.id}`}>
                <span className="marker grid" />
                <span className="row-name">{r.name}</span>
                <IssueTag issues={issuesOf('surface_receiver', r.id)} />
                <span className="row-detail">{r.shape.kind === 'scene' ? 'surface map' : 'cutting plane'}</span>
              </div>
            ))}
            {!view.point_receivers.length && !view.surface_receivers.length && (
              <div className="scene-empty empty">No receivers</div>
            )}

            {general.length > 0 && (
              <>
                <div className="scene-head label">
                  <span>Project</span>
                  <span className="count">{general.length}</span>
                </div>
                {general.map((i) => (
                  <div key={i.code + i.path} className="scene-row static" data-project-issue={i.code}>
                    <IssueTag issues={[i]} />
                    <span className="row-name empty" title={`${i.rule}: ${i.message}`}>
                      {sentence(i.message)}
                    </span>
                  </div>
                ))}
              </>
            )}
          </>
        )}
      </div>
    </aside>
  );
}
