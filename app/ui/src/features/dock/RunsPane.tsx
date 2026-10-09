// The Runs tab (design:259-274, M11 PLAN.md 3.3): the open project's runs, from `runs_list` and
// their run.json manifests, the newest first. Columns as drawn: Run, Variant, Solver, Status,
// Check.
//
// - Status is text (PQ5): OK, FAIL, CRASH, Cancelled, Running, Interrupted, in `[data-part=status]`.
// - Check: "Particles lost x.xx %", the worst band, then the limit; each reason as its UI code and
//   core code (`data-reason-code`); the warnings; the solver time.
// - R2: the person's label for a run, from notes.json beside its run.json, leads the Check column
//   (`[data-part=label]`); the selected row's detail edits it (`[data-field=run-label]`: Enter or
//   leaving the field keeps it, Esc puts it back, empty clears it).
// - R4: "Open folder" in the detail (`[data-action=run-open-folder]`): Explorer on the run's folder,
//   started by the core, so the page holds no shell permission.
// - R3: "Delete…" (`[data-action=run-delete]`), then a confirm in place (`[data-part=delete-confirm]`)
//   naming the exports noted as citing the run (`[data-part=delete-cites]`) and the later runs it
//   renumbers; "Move to Recycle Bin" (`[data-choice=delete]`) sends the folder there.
// - A click selects the run (the Results step shows it). The selected row opens to show the
//   per-band loss, the exe's sha256 and the solver build's verdict (the core's, `buildMark`,
//   backlog 38), the mesh's sha256, the line counts, the exit code and the folder.
//
// Every number with a unit is a leaf diagnostic span holding Rust's string from run.json
// (PLAN.md 2.3, 3.4 rule 1); a core detail that quotes one is left to run.json (model.detailView).
import { type ReactNode, useEffect, useRef, useState } from 'react';
import * as actions from '../../actions';
import type { ReasonUi, RunRow } from '../../bindings/ipc';
import { statusWord, WITHHELD_DETAIL } from '../../flow';
import { type ActiveRun, runsStore, runStore, sceneStore, selectedRunStore, useStore } from '../../store';
import {
  baseName,
  buildMark,
  detailView,
  exitText,
  hasManifest,
  newestFirst,
  progressPct,
  shortSha,
  solverLabel,
  stageLabel,
  statusTone,
  variantLabel,
  warningsText,
} from './model';

const SEP = ' · ';

function DetailText({ text }: { text: string | null | undefined }) {
  const d = detailView(text);
  if (d.kind === 'none') return null;
  if (d.kind === 'withheld') {
    return (
      <span
        className="reason-detail withheld"
        data-part="detail-withheld"
        title={WITHHELD_DETAIL}
      >
        : in run.json
      </span>
    );
  }
  return <span className="reason-detail">: {d.text}</span>;
}

function Reason({ r }: { r: ReasonUi }) {
  return (
    <span className="reason">
      <span className="reason-code" data-reason-code={r.ui_code} data-core-code={r.code}>
        {r.ui_code} ({r.code})
      </span>
      <DetailText text={r.detail} />
    </span>
  );
}

/** The Check column: what decided the row, in one line that wraps. */
function Check({ row, active }: { row: RunRow; active: ActiveRun | null }) {
  const pieces: ReactNode[] = [];
  if (row.label) {
    pieces.push(
      <span key="label" className="run-label" data-part="label" title={row.label}>
        {row.label}
      </span>,
    );
  }
  if (row.status === 'RUNNING') {
    const mine = active?.run === row.run ? active : null;
    const progress = mine?.progressText ? progressPct(mine.progressText) : null;
    pieces.push(
      <span key="stage" data-part="stage">
        {mine?.status === 'cancelling' ? 'Cancelling' : stageLabel(mine?.stage)}
        {progress !== null && (
          <>
            {SEP}
            <span data-diagnostic="progress_pct" data-run={row.run}>{`${progress} %`}</span>
          </>
        )}
      </span>,
    );
  }
  if (row.loss) {
    pieces.push(
      <span key="loss" className="loss">
        <span data-part="loss">
          Particles lost{' '}
          <span data-diagnostic="loss_pct" data-run={row.run}>{`${row.loss.worst_pct} %`}</span>
        </span>
        {row.loss.worst_pct !== '0.00' && <span data-part="worst-band">{` at ${row.loss.worst_band_hz} Hz`}</span>}
        <span data-part="loss-limit">
          {' / '}
          <span data-diagnostic="loss_limit_pct" data-run={row.run}>{`${row.loss.limit_pct} %`}</span>
          {' limit'}
        </span>
      </span>,
    );
  }
  for (const [i, r] of row.reasons.entries()) pieces.push(<Reason key={`r${i}`} r={r} />);
  if (hasManifest(row)) pieces.push(<span key="warn" data-part="warnings">{warningsText(row.warnings.length)}</span>);
  if (row.elapsed_s) {
    pieces.push(
      <span key="elapsed" className="elapsed">
        {'solver time '}
        <span data-part="elapsed" data-diagnostic="elapsed_s" data-run={row.run}>{`${row.elapsed_s} s`}</span>
      </span>,
    );
  }
  return (
    <span className="c-check" data-part="check" role="cell">
      {pieces.map((p, i) => (
        <span key={i} className="piece">
          {i > 0 && SEP}
          {p}
        </span>
      ))}
    </span>
  );
}

/** R2: the run's label, edited in place: Enter or leaving the field keeps it, Esc puts it back. */
function LabelField({ row }: { row: RunRow }) {
  const [text, setText] = useState<string | null>(null);
  // Esc blurs the field after putting the label back; the blur must not keep the edit.
  const dropped = useRef(false);
  const shown = text ?? row.label ?? '';
  const keep = () => {
    const edited = dropped.current ? null : text;
    dropped.current = false;
    setText(null);
    if (edited !== null && edited.trim() !== (row.label ?? '')) actions.fire(actions.labelRun(row.run, edited));
  };
  const running = row.status === 'RUNNING';
  return (
    <div className="detail-line" data-part="label-line">
      <span className="k">Label</span>
      <input
        className="run-label-input"
        data-field="run-label"
        value={shown}
        maxLength={80}
        disabled={running}
        placeholder={running ? 'A run is labelled once it has ended' : 'Name this run (Enter to keep)'}
        aria-label={`Label of run ${row.number}`}
        onClick={(e) => e.stopPropagation()}
        onChange={(e) => setText(e.target.value)}
        onBlur={keep}
        onKeyDown={(e) => {
          // The row's own keys (Enter and Space select it) stay out of the field.
          e.stopPropagation();
          if (e.key === 'Enter') e.currentTarget.blur();
          else if (e.key === 'Escape') {
            dropped.current = true;
            e.currentTarget.blur();
          }
        }}
      />
      {row.notes_error && (
        <span data-part="notes-error">
          <span className="flag">FAIL</span> notes.json does not read: the label is not shown or changed
        </span>
      )}
    </div>
  );
}

/** The selected row's detail: what a run's record holds beyond its verdict. */
function Detail({ row, root, total }: { row: RunRow; root: string | null; total: number }) {
  const mark = buildMark(row);
  const counts = row.lines;
  return (
    <div className="run-detail" data-part="detail">
      <LabelField row={row} />
      {row.loss && (
        <div className="detail-line" data-part="bands">
          <span className="k">Particle loss per band</span>
          {row.loss.bands.map((b) => (
            <span key={b.freq_hz} className="band" data-part="band-loss" data-band={b.freq_hz}>
              {`${b.freq_hz} Hz `}
              <span className="mono">{`${b.lost} of ${b.total}`}</span>{' '}
              <span data-diagnostic="loss_pct" data-band={b.freq_hz} data-run={row.run}>{`${b.pct} %`}</span>
            </span>
          ))}
        </div>
      )}
      <div className="detail-line" data-part="hashes">
        <span className="k">Solver</span>
        {row.exe ? (
          <span>
            <span title={row.exe.path}>{baseName(row.exe.path)}</span>
            {' sha256 '}
            <span className="mono" data-part="exe-sha" data-sha256={row.exe.sha256} title={row.exe.sha256}>
              {shortSha(row.exe.sha256)}
            </span>
          </span>
        ) : (
          <span>not recorded</span>
        )}
        {SEP}
        <span
          className={`verified ${mark.kind}`}
          data-part="verified"
          data-verified={mark.kind === 'verified' ? 'yes' : mark.kind === 'unverified' ? 'no' : 'unrecorded'}
          data-build-code={mark.reason?.ui_code}
          title={mark.names.join(', ')}
        >
          {mark.kind === 'verified' && 'solvers verified against the manifest'}
          {mark.kind === 'unverified' && mark.reason && (
            <>
              <span className="flag">FAIL</span>
              {' solver build not verified: '}
              <span className="reason-code">{`${mark.reason.ui_code} (${mark.reason.code})`}</span>
              <DetailText text={mark.reason.detail} />
            </>
          )}
          {mark.kind === 'unrecorded' && 'solver check not recorded (a command-line run, or one made before M11)'}
        </span>
        {SEP}
        {'mesh sha256 '}
        {row.mesh_sha256 ? (
          <span className="mono" data-part="mesh-sha" data-sha256={row.mesh_sha256} title={row.mesh_sha256}>
            {shortSha(row.mesh_sha256)}
          </span>
        ) : (
          <span>none</span>
        )}
      </div>
      {counts && (
        <div className="detail-line" data-part="line-counts">
          <span className="k">run.json lines</span>
          {(
            [
              ['PROGRESS', counts.progress],
              ['INFO', counts.info],
              ['OK', counts.ok],
              ['WARN', counts.warn],
              ['FAIL', counts.fail],
            ] as const
          ).map(([c, n]) => (
            <span key={c} className={`count ${c}`}>
              <span className="lbl">{c}</span> <span data-manifest-count={c}>{n}</span>
            </span>
          ))}
          {counts.unclassified > 0 && <span className="count">{`(${counts.unclassified} WARN unclassified)`}</span>}
        </div>
      )}
      {row.warnings.length > 0 && (
        <div className="detail-line" data-part="warning-list">
          <span className="k">Warnings</span>
          {row.warnings.map((w, i) => (
            <span key={i} className="reason">
              <span className="reason-code" data-warning-code={w.ui_code} data-core-code={w.code}>
                {w.ui_code} ({w.code})
              </span>
              <DetailText text={w.detail} />
            </span>
          ))}
        </div>
      )}
      <div className="detail-line" data-part="meta">
        <span className="k">Run</span>
        <span>{row.stage ? `stage ${row.stage}` : 'stage not recorded'}</span>
        {SEP}
        <span data-part="exit">{exitText(row.exit_code)}</span>
        {row.started && (
          <>
            {SEP}
            <span>{`started ${row.started}`}</span>
          </>
        )}
        {SEP}
        <span data-part="folder" title={root ? `${root}\\${row.run}` : undefined}>
          {`folder ${row.run}`}
        </span>
      </div>
      {row.manifest_error && (
        <div className="detail-line" data-part="manifest-error">
          <span className="flag">FAIL</span> run.json does not read
          <DetailText text={row.manifest_error} />
        </div>
      )}
      <div className="detail-line run-actions" data-part="run-actions">
        <span className="k">Folder</span>
        <button
          type="button"
          className="small-button"
          data-action="run-open-folder"
          title={root ? `Open ${root}\\${row.run} in Explorer` : 'Open the run folder in Explorer'}
          onClick={(e) => {
            e.stopPropagation();
            actions.fire(actions.openRunFolder(row.run));
          }}
        >
          Open folder
        </button>
        <DeleteRun row={row} total={total} />
      </div>
    </div>
  );
}

/**
 * R3: "Delete…", then a confirm in place that names what the delete takes and what cites the run
 * (the exports noted beside it), and how the later runs' numbers move; "Move to Recycle Bin" sends
 * the folder there, where it can be restored. Not offered for a run still running.
 */
function DeleteRun({ row, total }: { row: RunRow; total: number }) {
  const [asking, setAsking] = useState(false);
  const running = row.status === 'RUNNING';
  const later = Math.max(0, total - row.number);
  const stop = (e: React.SyntheticEvent) => e.stopPropagation();
  if (!asking) {
    return (
      <button
        type="button"
        className="small-button"
        data-action="run-delete"
        disabled={running}
        title={running ? 'A run is deleted once it has ended' : 'Move this run to the Recycle Bin (asks first)'}
        onClick={(e) => {
          e.stopPropagation();
          setAsking(true);
        }}
      >
        Delete…
      </button>
    );
  }
  return (
    <div className="run-delete-confirm" role="alertdialog" aria-label={`Delete run ${row.number}`} data-part="delete-confirm" onClick={stop} onKeyDown={stop}>
      <div className="q" data-part="delete-question">
        {`Move run ${row.number}${row.label ? ` "${row.label}"` : ''} to the Recycle Bin? Its folder ${row.run} goes with everything in it; it can be restored from the Recycle Bin.`}
      </div>
      <div data-part="delete-shown">It is the run the Results step and the Acoustics tab show now; they let it go.</div>
      {row.exports.length > 0 ? (
        <div data-part="delete-cites" data-cites={row.exports.length}>
          <span className="flag warn">CITED</span>
          {` ${row.exports.length} exported file${row.exports.length === 1 ? '' : 's'} cite this run. They stay where they are, naming a run that is gone:`}
          <ul className="cites">
            {row.exports.map((x) => (
              <li key={x.path} className="mono" data-cite={x.path} title={`${x.kind.toUpperCase()}, written ${x.at}`}>
                {x.path}
              </li>
            ))}
          </ul>
        </div>
      ) : row.notes_error ? (
        <div data-part="delete-cites" data-cites="unknown">
          <span className="flag">FAIL</span> notes.json does not read, so which exports cite this run is not known.
        </div>
      ) : (
        <div data-part="delete-cites" data-cites={0}>
          No export from this app is noted as citing this run.
        </div>
      )}
      {later > 0 && (
        <div data-part="delete-renumber">
          {`The ${later} later run${later === 1 ? '' : 's'} move${later === 1 ? 's' : ''} down one number (run ${row.number + 1} becomes run ${row.number}), so a file named by its run number may then name another run.`}
        </div>
      )}
      <div className="run-delete-actions">
        <button type="button" className="small-button" data-choice="cancel" autoFocus onClick={() => setAsking(false)}>
          Cancel
        </button>
        <button
          type="button"
          className="small-button danger"
          data-choice="delete"
          onClick={() => {
            setAsking(false);
            actions.fire(actions.deleteRun(row.run));
          }}
        >
          Move to Recycle Bin
        </button>
      </div>
    </div>
  );
}

function Row({
  row,
  selected,
  active,
  variants,
  root,
  total,
}: {
  row: RunRow;
  selected: boolean;
  active: ActiveRun | null;
  variants: readonly { id: string; name: string }[] | null;
  root: string | null;
  /** How many runs the tab lists (R3: the later runs a delete renumbers). */
  total: number;
}) {
  const select = () => actions.selectRun(row.run);
  return (
    <div
      className={`run-row${selected ? ' selected' : ''}`}
      role="row"
      tabIndex={0}
      aria-selected={selected}
      data-run-row={row.run}
      data-status={row.status}
      onClick={select}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          select();
        }
      }}
    >
      <div className="run-main">
        <span className="c-run mono" role="cell" data-part="number">{`#${row.number}`}</span>
        <span className="c-variant" role="cell">{variantLabel(row, variants, active)}</span>
        <span
          className="c-solver"
          role="cell"
          data-device={row.gpu_device || (active?.run === row.run && active.device === 'gpu') ? 'gpu' : 'cpu'}
          title={row.gpu_device ?? undefined}
        >
          {solverLabel(row.solver, !!row.gpu_device || (active?.run === row.run && active.device === 'gpu'))}
        </span>
        <span className={`c-status ${statusTone(row.status)}`} role="cell" data-part="status">
          {statusWord(row.status)}
        </span>
        <Check row={row} active={active} />
      </div>
      {selected && <Detail row={row} root={root} total={total} />}
    </div>
  );
}

export function RunsPane() {
  const view = useStore(runsStore);
  const scene = useStore(sceneStore);
  const active = useStore(runStore);
  const selected = useStore(selectedRunStore);

  // The list is re-read whenever the tab is shown (PLAN.md 3.1).
  useEffect(() => {
    actions.fire(actions.refreshRuns());
  }, []);

  const rows = view ? newestFirst(view.rows) : [];
  const root = view?.root ?? null;
  const rootText = detailView(root);
  return (
    <div className="runs" data-part="runs" role="table" aria-label="Runs">
      <div className="runs-head label" role="row">
        <span role="columnheader">Run</span>
        <span role="columnheader">Variant</span>
        <span role="columnheader">Solver</span>
        <span role="columnheader">Status</span>
        <span role="columnheader">Check</span>
      </div>
      {rows.length === 0 && (
        <div className="runs-empty empty">{scene ? 'No runs yet.' : 'Open a project to list its runs.'}</div>
      )}
      {rows.map((r) => (
        <Row
          key={r.run}
          row={r}
          selected={r.run === selected}
          active={active}
          variants={scene?.view.variants ?? null}
          root={root}
          total={rows.length}
        />
      ))}
      {/* A project never saved has no runs folder yet (runs_list answers an empty root). */}
      {view && scene && view.root && (
        <div className="runs-foot empty" data-part="runs-root">
          {rootText.kind === 'shown' ? `Runs folder ${rootText.text}` : 'Runs folder: runs, beside the project file'}
          {view.other_projects > 0 &&
            ` · ${view.other_projects} run folder${view.other_projects === 1 ? '' : 's'} of other projects not listed`}
        </div>
      )}
    </div>
  );
}
