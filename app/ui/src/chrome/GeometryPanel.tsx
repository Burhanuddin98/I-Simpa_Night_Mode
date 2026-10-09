// The Geometry step's properties (design:280-310): the room model and its check as text rows
// (closed, self-intersections, flipped normals, open edges, units), the dimensions, volume and
// surface (geometry facts, inside `[data-geometry]`), and Import model…, which opens the same
// dialog as File › Open… on a mesh (unit and up axis confirmed, never guessed).
//
// M11 (M10 MINOR B-18, check m11-b18):
// - The three dimensions share one precision, two decimals, as the design writes them
//   ("10.00 m"), where M10 dropped trailing zeros and mixed `41.45 m` with `16.1 m`.
// - A refused model shows no volume: the core sends none (`air_volume_m3` null), and the
//   row says why in words, with no digit. A checked model's volume keeps the status bar's
//   spelling (`fact(v, 1)`: "180 m³"), so one number reads the same in both places.
// - The volume is the air's (backlog 85), labelled so: the inside of a closed obstacle is not in
//   it. When the faces enclose more, the row's title says how much more and why.
// - Import model… waits while a run is active (PQ4), saying why.
import { useState } from 'react';
import * as actions from '../actions';
import { repairStore, runStore, sceneStore, useStore } from '../store';
import { RUN_ACTIVE_TITLE } from './MenuBar';
import { checkRows, fact, fileLabel, repairSummary, unitsText, unrepairable } from './sceneModel';

/** Decimals of every dimension (the design's "10.00 m"). */
const DIMENSION_DECIMALS = 2;

/** A dimension with exactly `DIMENSION_DECIMALS` decimals; an em dash when not a finite number. */
function dimension(v: number | null | undefined): string {
  return typeof v === 'number' && Number.isFinite(v) ? v.toFixed(DIMENSION_DECIMALS) : '—';
}

/**
 * G8: Repair, offered while the check refuses the model (upstream's "Repair model" at import): what it
 * fixes and what it never does, where the new file goes, then the last repair's result while the
 * project still holds the geometry it made.
 */
function RepairBlock({ ok }: { ok: boolean }) {
  const scene = useStore(sceneStore);
  const report = useStore(repairStore);
  const running = useStore(runStore) !== null;
  const [busy, setBusy] = useState(false);
  const rev = scene?.info.geometry_rev;
  const shown = report && report.outcome.state.info.geometry_rev === rev ? report : null;
  if (ok && !shown) return null;
  const not = unrepairable(scene?.check);
  return (
    <div className="props-section" data-part="repair">
      <div className="label section-label">Repair model</div>
      {shown && (
        <div className="hint" data-part="repair-result" data-changed={shown.changed} data-passes={shown.passes}>
          {repairSummary(shown)} {shown.passes ? 'The model check passes.' : 'The model check still refuses it.'}
        </div>
      )}
      {!ok && (
        <>
          <div className="hint">
            Repair welds vertices closer than 1 µm, removes faces of zero area and repeated faces, and turns faces that point
            into the room outward. It never closes a hole or separates faces that intersect. The repaired model is written to
            a new file beside the original (name_repaired.obj, metres, Z up); the original is not touched. One undo step.
          </div>
          {not.length > 0 && (
            <div className="hint" data-part="repair-cannot">
              Repair alone will not make this model pass; the check also finds: {not.join('; ')}.
            </div>
          )}
          <button
            className="wide-button"
            data-part="repair-model"
            disabled={running || busy}
            title={running ? RUN_ACTIVE_TITLE : 'Repair, write the new file beside the original, and check the model again'}
            onClick={() => {
              setBusy(true);
              actions
                .repairModel()
                .catch(() => {})
                .finally(() => setBusy(false));
            }}
          >
            Repair and check again
          </button>
        </>
      )}
    </div>
  );
}

function ImportBlock() {
  const running = useStore(runStore) !== null;
  return (
    <div className="props-section">
      <button
        className="wide-button"
        data-part="import-model"
        disabled={running}
        title={running ? RUN_ACTIVE_TITLE : undefined}
        onClick={() => actions.fire(actions.openDialog())}
      >
        Import model…
      </button>
      <div className="formats">PLY · OBJ · STL</div>
    </div>
  );
}

export function GeometryPanel() {
  const scene = useStore(sceneStore);
  const check = scene?.check ?? null;
  if (!scene || !check) {
    return (
      <div data-part="geometry-panel">
        <div className="props-head">
          <div className="title">Room model</div>
          <div className="sub">{scene ? 'No model in this project' : 'No project open'}</div>
        </div>
        <div className="props-section empty">
          Import a room model, or open a project, to see its model check here.
        </div>
        <ImportBlock />
      </div>
    );
  }
  const info = scene.info;
  const ok = check.verdict === 'ok';
  const imported = !info.path;
  const [lx, ly, lz] = check.extents_m;
  const volume = check.air_volume_m3 ?? null;
  const enclosed = check.enclosed_volume_m3 ?? null;
  const inside = volume != null && enclosed != null && enclosed > volume ? enclosed - volume : null;
  return (
    <div data-part="geometry-panel">
      <div className="props-head">
        <div className="title">Room model</div>
        <div className="sub">
          {fileLabel(info)} · {imported ? 'imported, ' : ''}
          {ok ? 'checked' : 'refused'}
        </div>
      </div>

      <div className="props-section">
        <div className="section-head">
          <span className="label">Model check</span>
          <span className={`verdict ${ok ? 'ok' : 'fail'}`} data-part="check-verdict">
            {ok ? 'Passed' : 'FAIL · refused'}
          </span>
        </div>
        <div className="check-list" data-geometry>
          {checkRows(check, unitsText(info)).map((r) => (
            <div key={r.key} className="check-row" data-check={r.key}>
              <i className="led" data-on={r.state === 'FAIL' ? 'fail' : r.state === 'OK' ? 'ok' : undefined} aria-hidden="true" />
              <span className={`state ${r.state === 'FAIL' ? 'fail' : r.state === 'OK' ? 'ok' : 'none'}`}>
                {r.state ?? ''}
              </span>
              <span className="k">{r.label}</span>
              <span className="v">{r.value}</span>
            </div>
          ))}
        </div>
        {!ok && (
          <div className="hint">
            Run is refused until the model passes. The faces involved are highlighted in the view; the
            Console has each reason in full.
          </div>
        )}
      </div>

      <div className="props-section" data-geometry>
        <div className="label section-label">Dimensions</div>
        <div className="fact-grid">
          {(
            [
              ['Length', lx],
              ['Width', ly],
              ['Height', lz],
            ] as const
          ).map(([k, v]) => (
            <div key={k} className="fact-cell" data-dimension={k.toLowerCase()}>
              <div className="k">{k}</div>
              <div className="v mono">{dimension(v)} m</div>
            </div>
          ))}
        </div>
        <div className="fact-line">
          <span data-part="volume" data-volume={volume == null ? 'none' : 'air'}>
            Air volume{' '}
            {volume == null ? (
              <span
                className="v"
                title={ok ? 'The model check gave no volume' : 'The model check refused this model: it encloses no volume anyone should read'}
              >
                {ok ? 'not given' : 'none: the model is refused'}
              </span>
            ) : (
              <span
                className="mono v"
                title={
                  inside == null
                    ? undefined
                    : `The faces enclose ${fact(enclosed, 1)} m³; ${fact(inside, 1)} m³ of it is inside closed obstacles, not air`
                }
              >
                {fact(volume, 1)} m³
              </span>
            )}
          </span>
          <span data-part="surface">
            Surface <span className="mono v">{fact(check.area_m2, 1)} m²</span>
          </span>
        </div>
        <div className="fact-line">
          <span>
            Faces <span className="mono v">{check.counts.faces}</span>
          </span>
          <span>
            Surface groups <span className="mono v">{info.surface_groups}</span>
          </span>
        </div>
      </div>

      <RepairBlock ok={ok} />

      {info.imported_ungrouped && (
        <div className="props-section" data-part="ungrouped-note">
          <div className="label section-label">No groups in the file</div>
          <div className="hint">
            The file names no surface groups, so every face came in as one group named after the file. To give walls their
            own materials: double-click a wall in the 3D view, right-click it, then New group from selection, or Move to group.
            In the scene list, F2 renames a group and Ctrl+click picks several to merge.
          </div>
        </div>
      )}

      <ImportBlock />
    </div>
  );
}
