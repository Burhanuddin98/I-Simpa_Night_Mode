// The scene list (design:69-102): a filter, then Surfaces (the effective material's swatch and
// name), Sources, Receivers and surface receivers, each row `[data-entity="<kind>:<id>"]` with a
// FAIL (or WARN) label and the UI code when the validator names it. A row click selects it;
// Del and F2 act on a selected source or receiver (PLAN.md 7.5).
import { useState } from 'react';
import type { UiIssue } from '../bindings/ipc';
import { issuesByEntity, projectIssues } from '../issues';
import { sceneStore, selectionStore, useStore } from '../store';
import { Search } from './icons';
import { coord, effectiveMaterial, matchesFilter, sentence, worstSeverity } from './sceneModel';
import { onEntityKey, selectGroup, selectPoint } from './sceneUi';

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
  const receivers = (view?.point_receivers ?? []).filter((r) => matchesFilter(query, r.name));
  const grids = (view?.surface_receivers ?? []).filter((r) => matchesFilter(query, r.name));
  const general = projectIssues(scene?.issues ?? []);

  return (
    <aside className="scene" aria-label="Scene">
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
              <button
                key={s.id}
                className="scene-row"
                data-entity={`source:${s.id}`}
                aria-pressed={selection.kind === 'source' && selection.id === s.id}
                onClick={() => selectPoint('source', s.id)}
              >
                <span className={`marker source${s.enabled ? '' : ' off'}`} />
                <span className="row-name">{s.name}</span>
                <IssueTag issues={issuesOf('source', s.id)} />
                {!s.enabled && <span className="row-detail">off</span>}
                <span className="row-detail" data-input>
                  {s.directivity.kind === 'omni' ? 'Omni' : 'Directional'} · {String(s.power.global_db)} dB
                </span>
              </button>
            ))}
            {!view.sources.length && <div className="scene-empty empty">No sources</div>}

            <Head
              title="Receivers"
              shown={receivers.length + grids.length}
              total={view.point_receivers.length + view.surface_receivers.length}
            />
            {receivers.map((r) => (
              <button
                key={r.id}
                className="scene-row"
                data-entity={`point_receiver:${r.id}`}
                aria-pressed={selection.kind === 'receiver' && selection.id === r.id}
                onClick={() => selectPoint('receiver', r.id)}
              >
                <span className="marker receiver" />
                <span className="row-name">{r.name}</span>
                <IssueTag issues={issuesOf('point_receiver', r.id)} />
                <span className="row-detail mono">
                  {coord(r.position[0], 1)}, {coord(r.position[1], 1)}
                </span>
              </button>
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
