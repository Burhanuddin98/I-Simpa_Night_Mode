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
//
// C1 (docs/investigations/2026-10-07-blank-geometry/SPEC.md): each surface row shows its face
// count (`[data-group-faces]`); F2 on a selected group (or a double-click on its row) edits its
// name in the row (`[data-part="group-name-input"]`, Enter commits, Esc cancels); Ctrl+click picks
// several groups, and the bar under the Surfaces head merges them into the first picked
// (`[data-action="merge-groups"]`). Each is one checked edit and one undo step; a refusal is shown
// under the surfaces.
//
// G18: "+ Group" in the Surfaces head (`[data-action="add-group"]`) adds an empty group with the
// placeholder material; each group row's remove button (`[data-part="group-remove"]`, or Del on a
// selected group) deletes it when it is empty, and otherwise says why not and what to do, under
// that group's own row (`[data-part="group-issues"][data-group]`), scrolled into view: under the
// last row it read as a refusal of whichever group happened to be last.
//
// G16: after the receivers, upstream's other nodes as far as the project holds them: Volumes (the
// room's air, from the model check, read-only), Fitting zones (from the project file), Environment
// (the air; a click opens Simulate, where it is edited) and Display (the 3D view's View style
// menu). Each says plainly what cannot be done here.
//
// M43: each surface receiver, cutting plane and fitting zone has an on/off switch,
// `[data-enabled-toggle="<kind>:<id>"]`, as a source has (EnabledSwitch); off leaves it out of the
// solver's input.
import { Fragment, useEffect, useRef, useState, type ReactNode } from 'react';
import * as actions from '../actions';
import type { Source, UiIssue } from '../bindings/ipc';
import { issuesByEntity, projectIssues } from '../issues';
import { fittingZonesStore, groupRenameStore, refusalStore, sceneStore, selectionStore, stepStore, useStore } from '../store';
import { settingsStore } from '../features/simulate/runSize';
import { ADD_GROUP_LABEL, groupPicked, mergePlan, renameProblem, toggleGroup } from './groupsModel';
import { FoldButton, useFold } from './fold';
import { usePanelWidth } from './panelWidth';
import { Search, Trash2 } from './icons';
import { coord, displayName, effectiveMaterial, environmentText, matchesFilter, receiverFolder, sentence, uniqueIssues, volumeRow, worstSeverity } from './sceneModel';
import { deleteGroup, type GroupProblem, groupProblemStore, onEntityKey, removeEntity, selectGroup, selectPoint } from './sceneUi';

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

/**
 * M43: the on/off switch of a cutting plane, a surface receiver or a fitting zone, drawn as the
 * source's: through the checked apply (the receiver or zone replaced whole, one undo step), its
 * refusals filed under `<kind>:<id>:enabled` and shown by `EnabledRefusals`. Off leaves it out of
 * the solver's input; `off` says what that costs.
 */
export function EnabledSwitch({ kind, id, name, on, off, compact = false }: { kind: 'surface_receiver' | 'fitting_zone'; id: string; name: string; on: boolean; off: string; compact?: boolean }) {
  const [pending, setPending] = useState(false);
  return (
    <button
      type="button"
      role="switch"
      className={`switch${compact ? ' compact' : ''}`}
      aria-checked={on}
      aria-label={`${name} on`}
      data-enabled-toggle={`${kind}:${id}`}
      data-state={on ? 'on' : 'off'}
      disabled={pending}
      title={on ? `Switch ${name} off: ${off}` : `Switch ${name} on`}
      onClick={(e) => {
        e.stopPropagation();
        setPending(true);
        (kind === 'surface_receiver' ? actions.setSurfaceReceiverEnabled(id, !on) : actions.setFittingZoneEnabled(id, !on))
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

/** M43: a refused on/off switch of one surface receiver or fitting zone, under its row. */
export function EnabledRefusals({ kind, id }: { kind: 'surface_receiver' | 'fitting_zone'; id: string }) {
  const refusals = useStore(refusalStore);
  return <ToggleRefusalLines issues={refusals.get(`${kind}:${id}:enabled`) ?? []} part="enabled-toggle-issues" />;
}

/** A refused switch, inline: FAIL and the UI code as text, then the message. */
export function ToggleRefusalLines({ issues, part = 'source-toggle-issues' }: { issues: readonly UiIssue[]; part?: string }) {
  if (!issues.length) return null;
  return (
    <div className="issues toggle-issues" data-part={part}>
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
      <span className="state">{worst === 'error' ? 'Error' : 'Warning'}</span>
      {more && <span className="code">{more}</span>}
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

/** C1: a group's name, edited in its row (F2). Enter or blur commits, Esc cancels. */
function GroupNameInput({ id, name, onProblem }: { id: string; name: string; onProblem: (p: { code: string; message: string } | null) => void }) {
  const ref = useRef<HTMLInputElement>(null);
  const done = useRef(false);
  const [text, setText] = useState(name);
  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);
  const finish = (commit: boolean) => {
    if (done.current) return;
    done.current = true;
    groupRenameStore.set(null);
    if (!commit || text === name) return;
    const groups = sceneStore.get()?.view.surface_groups ?? [];
    const problem = renameProblem(text, id, groups);
    onProblem(problem);
    if (problem) return;
    actions.renameGroup(id, text.trim()).catch((e: unknown) => onProblem(actions.asCmdError(e)));
  };
  return (
    <input
      ref={ref}
      className="group-name-input"
      data-part="group-name-input"
      aria-label={`Rename ${name}`}
      value={text}
      spellCheck={false}
      onChange={(e) => setText(e.target.value)}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === 'Enter') {
          e.preventDefault();
          finish(true);
        } else if (e.key === 'Escape') {
          e.preventDefault();
          finish(false);
        }
      }}
      onBlur={() => finish(true)}
    />
  );
}

/** A group row's remove button (G18): an empty group goes; one that holds faces says why not. */
function GroupRemoveButton({ id, name, faces }: { id: string; name: string; faces: number }) {
  return (
    <button
      className="row-remove"
      data-part="group-remove"
      aria-label={`Delete ${name}`}
      title={
        faces > 0
          ? `${name} holds ${faces} ${faces === 1 ? 'face' : 'faces'}: only an empty group can be deleted`
          : `Delete ${name} (Del; Ctrl+Z brings it back)`
      }
      onClick={() => actions.fire(deleteGroup(id))}
    >
      <Trash2 size={12} />
    </button>
  );
}

/** A group edit's own problem (`groupProblemStore`), said in full; `group` when it sits under that group's row. */
function GroupProblemLine({ problem, group }: { problem: GroupProblem; group?: string }) {
  return (
    <div className="issues toggle-issues" data-part="group-issues" data-group={group}>
      <div className="issue" data-issue-code={problem.code} role="alert">
        <span className="code">FAIL {problem.code}</span>
        <span className="msg">{problem.message} The project is unchanged.</span>
      </div>
    </div>
  );
}

function Head({ title, shown, total, children }: { title: string; shown: number; total: number; children?: ReactNode }) {
  return (
    <div className="scene-head label">
      <span>{title}</span>
      <span className="scene-head-end">
        {children}
        <span className="count">{shown === total ? total : `${shown} of ${total}`}</span>
      </span>
    </div>
  );
}

export function ScenePanel() {
  const scene = useStore(sceneStore);
  const selection = useStore(selectionStore);
  const refusals = useStore(refusalStore);
  const [query, setQuery] = useState('');
  const renaming = useStore(groupRenameStore);
  const groupProblem = useStore(groupProblemStore);
  const setGroupProblem = (p: GroupProblem | null) => groupProblemStore.set(p);
  const listRef = useRef<HTMLDivElement>(null);
  const view = scene?.view ?? null;
  const byEntity = issuesByEntity(scene?.issues ?? []);
  const issuesOf = (kind: string, id: string) => byEntity.get(`${kind}:${id}`) ?? [];
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
  const zoneList = useStore(fittingZonesStore);
  const settings = useStore(settingsStore);
  const zonesKnown = zoneList !== null;
  const zones = (zoneList ?? []).filter((z) => matchesFilter(query, z.name));
  const volume = volumeRow(scene?.check);
  const envText = environmentText(settings?.environment);
  const merge = view ? mergePlan(selection, view.surface_groups) : null;
  // The latest group edit's refusals (rename, merge, move), from the checked apply.
  const groupRefusals = uniqueIssues(
    ...[...refusals].filter(([k]) => k.startsWith('surface_group:') && /:(name|merge|faces|delete)$/.test(k)).map(([, v]) => v),
  );
  const folded = useFold('scene');
  const sized = usePanelWidth('nm-scene-width', 248, 'right');
  // A problem that concerns one group shows under that group's row, when the row is listed; else under the Surfaces.
  const problemRow =
    groupProblem?.group !== undefined && renaming !== groupProblem.group && shownSurfaces.some(({ g }) => g.id === groupProblem.group)
      ? groupProblem.group
      : null;
  // ...and is scrolled to, the row and its sentence both: the group may be far out of view (Edit > Delete group).
  useEffect(() => {
    if (problemRow === null) return;
    const list = listRef.current;
    const line = list?.querySelector(`[data-part="group-issues"][data-group="${CSS.escape(problemRow)}"]`);
    const row = list?.querySelector(`[data-entity="surface_group:${CSS.escape(problemRow)}"]`);
    line?.scrollIntoView({ block: 'nearest' });
    row?.scrollIntoView({ block: 'nearest' });
  }, [groupProblem, problemRow]);

  return (
    <aside className="scene" aria-label="Scene" data-folded={folded} style={sized.style} onScroll={sized.onScroll}>
      <FoldButton panel="scene" />
      {!folded && <div {...sized.grip} />}
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
      <div className="scene-list" data-part="scene-list" onKeyDown={onEntityKey} ref={listRef}>
        {!view ? (
          <div className="scene-empty empty">No project. File, then Open…</div>
        ) : (
          <>
            <Head title="Surfaces" shown={shownSurfaces.length} total={surfaces.length}>
              <button
                className="scene-head-add"
                data-action="add-group"
                title={`${ADD_GROUP_LABEL}: an empty group, its material to choose; move faces into it from the 3D view (right-click, Move to group)`}
                onClick={() => {
                  setGroupProblem(null);
                  actions.addEmptyGroup().catch((e: unknown) => setGroupProblem(actions.asCmdError(e)));
                }}
              >
                + Group
              </button>
            </Head>
            {merge && (
              <div className="group-merge" data-part="group-merge">
                <span className="grow">{merge.from.length + 1} groups picked</span>
                <button
                  className="small-button"
                  data-action="merge-groups"
                  title={`Merge ${merge.from.map((g) => g.name).join(', ')} into ${merge.into.name}, which keeps its name and material (one undo step)`}
                  onClick={() => {
                    setGroupProblem(null);
                    actions.mergeSelectedGroups().catch((e: unknown) => setGroupProblem(actions.asCmdError(e)));
                  }}
                >
                  Merge into {displayName(merge.into.name)}
                </button>
              </div>
            )}
            {shownSurfaces.map(({ g, m, assigned }) => {
              const on = groupPicked(selection, g.id) || pickedGroupNames.has(g.name);
              const faces = stats.get(g.id)?.faces ?? 0;
              if (renaming === g.id) {
                return (
                  <div key={g.id} className="scene-row static editing" data-entity={`surface_group:${g.id}`} aria-pressed={on}>
                    <span className={`swatch${assigned ? '' : ' unassigned'}`} style={assigned && m ? { background: m.color } : undefined} />
                    <GroupNameInput id={g.id} name={g.name} onProblem={setGroupProblem} />
                  </div>
                );
              }
              return (
                <Fragment key={g.id}>
                <div className="scene-line">
                <button
                  className="scene-row group-row"
                  data-entity={`surface_group:${g.id}`}
                  aria-pressed={on}
                  title="Click to set its material; Ctrl+click to pick several, to merge them; F2 to rename"
                  onClick={(e) => {
                    setGroupProblem(null);
                    if (e.ctrlKey || e.metaKey) selectionStore.set(toggleGroup(selectionStore.get(), g.id) as typeof selection);
                    else selectGroup(g.id);
                  }}
                  onDoubleClick={() => {
                    selectGroup(g.id);
                    groupRenameStore.set(g.id);
                  }}
                >
                  <span
                    className={`swatch${assigned ? '' : ' unassigned'}`}
                    style={assigned && m ? { background: m.color } : undefined}
                  />
                  {/* C1 audit: the name has the row's whole width (renaming is the feature); the face
                      count and the material go on a second line, and shorten first. */}
                  <span className="row-main">
                    <span className="row-name" title={g.name}>
                      {displayName(g.name)}
                    </span>
                    <span className="row-sub">
                      <span className="row-count mono" data-group-faces={faces} title={`${faces} ${faces === 1 ? 'face' : 'faces'}`}>
                        {faces} {faces === 1 ? 'face' : 'faces'}
                      </span>
                      {/* The material, unless it only repeats the surface's own name (BRAS names both alike). */}
                      {!(assigned && m && displayName(m.name) === displayName(g.name)) && (
                        <span
                          className="row-detail"
                          data-input
                          title={assigned ? m?.name : `${m?.name ?? 'No material'}: the import placeholder, not assigned yet`}
                        >
                          {assigned ? (m ? displayName(m.name) : '—') : 'unassigned'}
                        </span>
                      )}
                    </span>
                  </span>
                  <IssueTag issues={issuesOf('surface_group', g.id)} />
                </button>
                <GroupRemoveButton id={g.id} name={displayName(g.name)} faces={faces} />
                </div>
                {problemRow === g.id && groupProblem && <GroupProblemLine problem={groupProblem} group={g.id} />}
                </Fragment>
              );
            })}
            {!surfaces.length && <div className="scene-empty empty">No surfaces</div>}
            {groupProblem && problemRow === null && <GroupProblemLine problem={groupProblem} />}
            {groupRefusals.length > 0 && (
              <div className="issues toggle-issues" data-part="group-issues">
                {groupRefusals.map((i) => (
                  <div key={`${i.code}|${i.path}`} className="issue" data-issue-code={i.code} role="alert">
                    <span className="code">FAIL {i.code}</span>
                    <span className="msg">{sentence(i.message)} Refused; the project is unchanged.</span>
                  </div>
                ))}
              </div>
            )}

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
              <Fragment key={r.id}>
                <div className="scene-line">
                  <div className="scene-row static" data-entity={`surface_receiver:${r.id}`}>
                    <span className={`marker grid${r.enabled ? '' : ' off'}`} />
                    <span className="row-name">{r.name}</span>
                    <IssueTag issues={issuesOf('surface_receiver', r.id)} />
                    {!r.enabled && <span className="row-detail row-off">off</span>}
                    <span className="row-detail">{r.shape.kind === 'scene' ? 'surface map' : 'cutting plane'}</span>
                  </div>
                  <EnabledSwitch kind="surface_receiver" id={r.id} name={r.name} on={r.enabled} off="the run leaves it out and makes no map for it" compact />
                </div>
                <EnabledRefusals kind="surface_receiver" id={r.id} />
              </Fragment>
            ))}
            {!view.point_receivers.length && !view.surface_receivers.length && (
              <div className="scene-empty empty">No receivers</div>
            )}

            {/* G16: upstream's other scene nodes, as far as the project holds them; what cannot be edited here says so. */}
            {(!query || matchesFilter(query, 'Volumes', volume.text)) && (
              <>
                <Head title="Volumes" shown={scene?.check?.verdict === 'ok' ? 1 : 0} total={scene?.check?.verdict === 'ok' ? 1 : 0} />
                <div className="scene-row static" data-scene-node="volume" title={`${volume.detail}. Volumes are found by the model check; naming one or making it a fitting zone is not in this version.`}>
                  <span className="marker volume" aria-hidden />
                  <span className="row-name">{volume.text}</span>
                  {scene?.check?.verdict === 'ok' && <span className="row-detail">read-only</span>}
                </div>
              </>
            )}

            {(!query || zones.length > 0) && (
              <>
            <Head title="Fitting zones" shown={zones.length} total={zoneList?.length ?? 0} />
            {zones.map((z) => (
              <Fragment key={z.id}>
                <div className="scene-line">
                  <div className="scene-row static" data-entity={`fitting_zone:${z.id}`} title="As the project holds it; it is switched on or off here">
                    <span className={`marker zone${z.enabled ? '' : ' off'}`} aria-hidden />
                    <span className="row-name">{z.name}</span>
                    <IssueTag issues={issuesOf('fitting_zone', z.id)} />
                    {!z.enabled && <span className="row-detail row-off">off</span>}
                    <span className="row-detail">{z.shape.kind === 'box' ? 'box' : 'scene volume'}</span>
                  </div>
                  <EnabledSwitch kind="fitting_zone" id={z.id} name={z.name} on={z.enabled} off="the run leaves its fittings out, as if the space were empty" compact />
                </div>
                <EnabledRefusals kind="fitting_zone" id={z.id} />
              </Fragment>
            ))}
            <div className="scene-empty empty" data-scene-node="fitting-zones-note">
              {zonesKnown && zones.length === 0 ? 'No fitting zones. ' : ''}
              A zone is switched on or off here; adding or editing one is not in this version, and an imported project keeps its own.
            </div>
              </>
            )}

            {(!query || matchesFilter(query, 'Environment', envText)) && (
              <>
                <Head title="Environment" shown={1} total={1} />
                <button
                  className="scene-row"
                  data-scene-node="environment"
                  title="Temperature, humidity and pressure: edited in Simulate, under the solver's settings"
                  onClick={() => stepStore.set('simulate')}
                >
                  <span className="marker env" aria-hidden />
                  <span className="row-name">Air</span>
                  {/* A project input with its units (the scene list is an input region, m10-h). */}
                  <span className="row-detail mono" data-input>
                    {envText}
                  </span>
                </button>
              </>
            )}

            {(!query || matchesFilter(query, 'Display', 'View style')) && (
              <>
                <Head title="Display" shown={1} total={1} />
                <div className="scene-row static" data-scene-node="display" title="Faces, lines and what is hidden are set in the 3D view's View style menu; per-element colour and Show name are not in this version">
                  <span className="marker display" aria-hidden />
                  <span className="row-name">View style</span>
                  <span className="row-detail">in the 3D view</span>
                </div>
              </>
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
